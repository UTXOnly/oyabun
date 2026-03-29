use glam::{Mat4, Quat, Vec3};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

mod game;
mod gltf_level;
mod input;
mod loadout;
mod mesh;
mod net;
mod npc;
mod render;

use game::GameState;
use npc::NpcManager;
use input::InputState;
use loadout::{Loadout, WEAPONS};
use mesh::{arena_from_level_json, build_arena, mural_z_plane, vertex_bounds, LevelBoot};
use net::NetController;
use render::WeaponHudParams;
pub use render::{CharacterInstance, CharacterSkin, Gpu, Vertex};

use serde_json::json;

fn character_model(foot: Vec3, yaw: f32, scale: f32) -> Mat4 {
    // Blender models face -Y → glTF +Z. Game yaw 0 = facing -Z. Add PI to flip.
    Mat4::from_scale_rotation_translation(
        Vec3::splat(scale),
        Quat::from_rotation_y(yaw + std::f32::consts::PI),
        foot,
    )
}

fn make_character(foot: Vec3, facing_yaw: f32, scale: f32, skin: CharacterSkin) -> CharacterInstance {
    CharacterInstance {
        model: character_model(foot, facing_yaw, scale),
        mesh_yaw: facing_yaw,
        skin,
        anim_frame: 0.0,
        bill_tint: [1.0, 1.0, 1.0, 1.0],
    }
}

fn npc_sprite_billboard_tint(npc: &npc::Npc) -> [f32; 4] {
    let mut r = 1.0_f32;
    let mut g = 1.0;
    let mut b = 1.0;
    let mut a = 1.0_f32;
    if npc.alive() {
        let inj = 1.0 - npc.hp_frac();
        r += inj * 0.48;
        g -= inj * 0.26;
        b -= inj * 0.22;
    }
    if npc.hit_flash > 0.0 {
        let h = npc.hit_flash.min(1.0);
        r += h * 0.62;
        g += h * 0.12;
        b += h * 0.08;
    }
    if npc.state == npc::NpcState::Dead {
        let t = npc.death_timer;
        r = r * (0.4 + (1.0 - t) * 0.35) + t * 0.22;
        g = g * (0.22 + (1.0 - t) * 0.32);
        b = b * (0.24 + (1.0 - t) * 0.28);
        a = 0.94 - t * 0.18;
    }
    [
        r.clamp(0.12, 2.2),
        g.clamp(0.12, 2.2),
        b.clamp(0.12, 2.2),
        a.clamp(0.25, 1.0),
    ]
}

/// Compute walk animation frame (1.0–6.0) from time, or 0.0 for idle.
/// `speed` is the character's XZ movement speed; below threshold → idle.
const WALK_FRAME_COUNT: f32 = 6.0;
const WALK_FPS: f32 = 8.0;
const WALK_SPEED_THRESHOLD: f32 = 0.3;

fn walk_anim_frame(time: f32, speed: f32) -> f32 {
    if speed < WALK_SPEED_THRESHOLD {
        return 0.0; // idle
    }
    // Cycle through frames 1-6 based on time, scale rate by speed
    let rate = WALK_FPS * (speed / 3.0).max(0.6);
    let frame = (time * rate) % WALK_FRAME_COUNT;
    frame.floor() + 1.0 // rows 1-6 in the atlas
}

const RUN_FRAME_COUNT: f32 = 6.0;
const RUN_FPS: f32 = 11.0;
const RUN_SPEED_THRESHOLD: f32 = 2.25;
const SHOOT_FPS: f32 = 9.0;
const SHOOT_FRAME_COUNT: f32 = 6.0;

fn run_anim_frame(time: f32, speed: f32) -> f32 {
    let rate = RUN_FPS * (speed / 3.2).max(0.75);
    let frame = (time * rate) % RUN_FRAME_COUNT;
    7.0 + frame.floor()
}

fn shoot_anim_frame(shoot_t: f32) -> f32 {
    let frame = (shoot_t * SHOOT_FPS) % SHOOT_FRAME_COUNT;
    13.0 + frame.floor()
}

fn npc_billboard_anim_frame(time: f32, npc: &npc::Npc, atlas_rows: u32) -> f32 {
    if npc.hit_flash > 0.0 {
        return 100.0 + npc.hit_flash;
    }
    if !npc.alive() {
        return 0.0;
    }
    let extended_shoot = atlas_rows >= 19;
    let extended_run = atlas_rows >= 13;
    if npc.shooting_at_player() {
        if extended_shoot {
            return shoot_anim_frame(npc.shoot_anim_t);
        }
        return 0.0;
    }
    if npc.speed >= RUN_SPEED_THRESHOLD && extended_run {
        return run_anim_frame(time, npc.speed);
    }
    walk_anim_frame(time, npc.speed)
}

/// Vertical walk bob (sinusoidal bounce) to prevent floating/sliding look.
fn walk_bob_y(time: f32, speed: f32) -> f32 {
    if speed < WALK_SPEED_THRESHOLD {
        return 0.0;
    }
    let rate = WALK_FPS * (speed / 3.0).max(0.6);
    // Two bounces per walk cycle (each foot hits ground)
    let phase = time * rate * 2.0 * std::f32::consts::PI / WALK_FRAME_COUNT;
    phase.sin().abs() * 0.06 // 6cm vertical bounce
}

fn yaw_face_cam_xz(foot: Vec3, cam: Vec3) -> f32 {
    let dx = cam.x - foot.x;
    let dz = cam.z - foot.z;
    // Must match shader convention: atan2(dx, -dz)
    dx.atan2(-dz)
}

#[cfg(target_arch = "wasm32")]
async fn fetch_level_json(url: &str) -> Option<String> {
    let window = web_sys::window()?;
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_cache(web_sys::RequestCache::NoStore);
    let req = web_sys::Request::new_with_str_and_init(url, &init).ok()?;
    let v = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req))
        .await
        .ok()?;
    let resp: web_sys::Response = v.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let text_p = resp.text().ok()?;
    let text_v = wasm_bindgen_futures::JsFuture::from(text_p).await.ok()?;
    text_v.as_string()
}

#[cfg(target_arch = "wasm32")]
async fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
    let window = web_sys::window()?;
    let init = web_sys::RequestInit::new();
    init.set_method("GET");
    init.set_cache(web_sys::RequestCache::NoStore);
    let req = web_sys::Request::new_with_str_and_init(url, &init).ok()?;
    let v = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&req))
        .await
        .ok()?;
    let resp: web_sys::Response = v.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let buf = wasm_bindgen_futures::JsFuture::from(resp.array_buffer().ok()?).await.ok()?;
    let arr = js_sys::Uint8Array::new(&buf);
    let mut out = vec![0u8; arr.length() as usize];
    arr.copy_to(&mut out);
    Some(out)
}

struct GameInit {
    boot: LevelBoot,
    gltf: Option<gltf_level::GltfLevelCpu>,
    level_label: String,
    vert_count: usize,
    batch_count: usize,
}

#[cfg(target_arch = "wasm32")]
fn wasm_warn(msg: &str) {
    web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(msg));
}

#[cfg(target_arch = "wasm32")]
fn wasm_log(msg: &str) {
    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(msg));
}

fn gltf_needs_floor_slab(solids: &[mesh::Aabb], bounds: &mesh::Aabb) -> bool {
    if solids.is_empty() {
        return false;
    }
    if solids.len() == 1 {
        let s = &solids[0];
        let covers = s.min.x <= bounds.min.x + 0.02
            && s.min.z <= bounds.min.z + 0.02
            && s.max.x >= bounds.max.x - 0.02
            && s.max.z >= bounds.max.z - 0.02
            && s.min.y <= bounds.min.y + 0.5
            && s.max.y >= bounds.max.y - 0.5;
        return !covers;
    }
    true
}

#[cfg(target_arch = "wasm32")]
fn game_init_from_gltf(cpu: gltf_level::GltfLevelCpu) -> GameInit {
    let bounds = cpu.bounds();
    let spawn = cpu.spawn;
    let yaw = cpu.spawn_yaw;
    let (boss, rival) = mesh::npc_placements(spawn, yaw, &bounds);
    // Override spawn yaw to face toward NPCs (midpoint of boss & rival)
    let mid = (boss + rival) * 0.5;
    let to_dx = mid.x - spawn.x;
    let to_dz = mid.z - spawn.z;
    let yaw = if to_dx.abs() + to_dz.abs() > 0.1 {
        to_dx.atan2(-to_dz)
    } else {
        yaw
    };
    let mural_z = mesh::mural_z_plane(&bounds, spawn);
    let mut arena = mesh::empty_arena();
    let mut solids = cpu.solids.clone();
    if gltf_needs_floor_slab(&solids, &bounds) {
        solids.push(mesh::Aabb {
            min: Vec3::new(bounds.min.x - 120.0, bounds.min.y - 0.25, bounds.min.z - 120.0),
            max: Vec3::new(bounds.max.x + 120.0, bounds.min.y + 0.12, bounds.max.z + 120.0),
        });
    }
    arena.solids = solids;
    wasm_log(&format!(
        "oyabaun: glTF level {} verts, {} tri indices, {} draw batches; bounds Y [{:.2}, {:.2}] (online play snaps feet to colliders under you)",
        cpu.vertices.len(),
        cpu.indices.len() / 3,
        cpu.batches.len(),
        bounds.min.y,
        bounds.max.y
    ));
    let vert_count = cpu.vertices.len();
    let batch_count = cpu.batches.len();
    GameInit {
        boot: LevelBoot {
            arena,
            spawn,
            boss_foot: boss,
            rival_foot: rival,
            spawn_yaw: yaw,
            level_bounds: bounds,
            mural_z,
        },
        gltf: Some(cpu),
        level_label: String::from("glTF tokyo_alley"),
        vert_count,
        batch_count,
    }
}

async fn load_game_init() -> GameInit {
    #[cfg(target_arch = "wasm32")]
    {
        const EMBEDDED_GLB: &[u8] = include_bytes!("../levels/tokyo_alley.glb");

        let mut fetch_failed = false;
        let mut fetch_parse_err: Option<String> = None;
        let ts = js_sys::Date::now() as u64;
        let level_url = format!("./levels/tokyo_alley.glb?v={ts}");
        if let Some(bytes) = fetch_bytes(&level_url).await {
            match gltf_level::parse_glb(&bytes) {
                Ok(cpu) => {
                    return game_init_from_gltf(cpu);
                }
                Err(e) => fetch_parse_err = Some(e),
            }
        } else {
            fetch_failed = true;
        }

        match gltf_level::parse_glb(EMBEDDED_GLB) {
            Ok(cpu) => {
                if let Some(ref e) = fetch_parse_err {
                    wasm_warn(&format!(
                        "oyabaun: fetched tokyo_alley.glb failed to parse ({e}); using embedded copy (wasm-pack build after export-world refreshes embed)."
                    ));
                } else if fetch_failed {
                    wasm_warn(
                        "oyabaun: fetch ./levels/tokyo_alley.glb failed (check DevTools → Network). Serve from client/ (oyabaunctl launch). Using embedded .glb; run wasm-pack build after export-world to refresh it.",
                    );
                }
                return game_init_from_gltf(cpu);
            }
            Err(e) => {
                wasm_warn(&format!("oyabaun: embedded tokyo_alley.glb parse failed: {e}"));
                if fetch_failed {
                    wasm_warn(
                        "oyabaun: fetch also failed — add client/levels/tokyo_alley.glb (export-world) and wasm-pack build.",
                    );
                }
            }
        }

        if let Some(json) = fetch_level_json("./levels/tokyo_street.json").await {
            if let Ok(boot) = arena_from_level_json(&json) {
                let vc = boot.arena.vertices.len();
                return GameInit {
                    boot,
                    gltf: None,
                    level_label: String::from("vertex JSON"),
                    vert_count: vc,
                    batch_count: 1,
                };
            }
        }

        wasm_warn(
            "oyabaun: using procedural build_arena() — this is not your Blender level. Fix level files + wasm-pack build, then reload.",
        );
    }
    let arena = build_arena();
    let level_bounds = vertex_bounds(&arena);
    let spawn = Vec3::new(0.0, 0.0, 9.0);
    let mural_z = mural_z_plane(&level_bounds, spawn);
    GameInit {
        boot: LevelBoot {
            spawn,
            boss_foot: Vec3::new(11.85, 0.0, -11.85),
            rival_foot: Vec3::new(-10.2, 0.0, -9.4),
            spawn_yaw: 0.0,
            level_bounds,
            mural_z,
            arena,
        },
        gltf: None,
        level_label: String::from("procedural demo"),
        vert_count: 0,
        batch_count: 0,
    }
}

#[wasm_bindgen]
pub struct OyabaunApp {
    gpu: Gpu,
    game: GameState,
    input: InputState,
    net: NetController,
    loadout: Loadout,
    npcs: NpcManager,
    last_ms: f64,
    game_time: f32,
    /// Set to true on the frame an NPC is hit; JS reads and clears it.
    last_hit: bool,
    /// Name of last killed NPC (empty if none this frame).
    last_kill: String,
    /// Screen blood overlay strength for HUD (0..1), arcade hit feedback.
    blood_splat: f32,
    clear: Vec3,
    level_bounds: mesh::Aabb,
    mural_z: f32,
    level_label: String,
    vert_count: usize,
    batch_count: usize,
}

#[wasm_bindgen]
impl OyabaunApp {
    pub fn key_set(&mut self, code: &str, down: bool) {
        self.input.key_set(code, down);
    }

    pub fn mouse_accum(&mut self, dx: f32, dy: f32) {
        self.input.mouse_accum(dx, dy);
    }

    pub fn shoot_press(&mut self) {
        self.input.shoot_press();
    }

    /// Returns true if an NPC was hit since last call (auto-clears).
    pub fn take_hit(&mut self) -> bool {
        let h = self.last_hit;
        self.last_hit = false;
        h
    }

    /// Returns the name of the last killed NPC (empty if none). Auto-clears.
    pub fn take_kill(&mut self) -> String {
        let k = self.last_kill.clone();
        self.last_kill.clear();
        k
    }

    /// Debug: force a shot and return what happened.
    pub fn debug_shoot(&mut self) -> String {
        let eye = self.game.eye_pos();
        let dir = self.game.view_forward();
        let wi = self.loadout.current_idx();
        let clip = self.loadout.clip_for(wi);
        let mut result = format!(
            "eye=({:.2},{:.2},{:.2}) dir=({:.3},{:.3},{:.3}) weapon={} clip={}",
            eye.x, eye.y, eye.z, dir.x, dir.y, dir.z, wi, clip
        );
        for (i, npc) in self.npcs.npcs.iter().enumerate() {
            let f = npc.foot;
            let aabb = format!(
                " npc{}[{}] foot=({:.2},{:.2},{:.2}) hp={:.0} hp%={:.0} alive={}",
                i,
                npc.def.label,
                f.x,
                f.y,
                f.z,
                npc.hp,
                npc.hp_frac() * 100.0,
                npc.alive()
            );
            result.push_str(&aabb);

            // Manual ray test
            let sc = npc.def.scale;
            let pad = npc.def.hitbox_pad * sc;
            let hmin = Vec3::new(f.x - pad, f.y + 0.05, f.z - pad);
            let hmax = Vec3::new(f.x + pad, f.y + npc.def.hitbox_height * sc, f.z + pad);
            result.push_str(&format!(
                " box=({:.2},{:.2},{:.2})-({:.2},{:.2},{:.2})",
                hmin.x, hmin.y, hmin.z, hmax.x, hmax.y, hmax.z
            ));
        }

        // Actually try firing
        let fired = self.loadout.try_fire();
        result.push_str(&format!(" fired={}", fired));
        if fired {
            let hp_before: Vec<f32> = self.npcs.npcs.iter().map(|n| n.hp).collect();
            self.npcs.register_shot(&self.game, wi);
            for (i, npc) in self.npcs.npcs.iter().enumerate() {
                if npc.hp < hp_before[i] {
                    result.push_str(&format!(" HIT_NPC{}! {:.0}->{:.0}", i, hp_before[i], npc.hp));
                    self.last_hit = true;
                }
            }
        }
        result
    }

    pub fn ingest_server_json(&mut self, json: &str) {
        self.net.ingest(json);
        self.game.set_online(self.net.joined);
    }

    pub fn take_net_outbound(&mut self) -> Option<String> {
        self.net.take_outbound()
    }

    /// Live swapchain + world draw diagnostics (browser console: `JSON.parse(app.renderDebugJson())`).
    #[wasm_bindgen(js_name = renderDebugJson)]
    pub fn render_debug_json(&self) -> String {
        let gpu = self.gpu.render_diag();
        let eye = self.game.eye_pos();
        let fwd = self.game.view_forward();
        let aspect = self.gpu.config.width.max(1) as f32 / self.gpu.config.height.max(1) as f32;
        let vp = self.game.view_proj(aspect);
        let ndc = |p: Vec3| -> [f32; 4] {
            let c = vp * p.extend(1.0);
            let w = c.w;
            if w.abs() > 1e-7 {
                let t = c.truncate() / w;
                [t.x, t.y, t.z, w]
            } else {
                [f32::NAN, f32::NAN, f32::NAN, w]
            }
        };
        let level_mid = (self.level_bounds.min + self.level_bounds.max) * 0.5;
        let ahead = eye + fwd * 4.0;
        json!({
            "gpu": gpu,
            "clear_rgb": [self.clear.x, self.clear.y, self.clear.z],
            "eye": [eye.x, eye.y, eye.z],
            "forward": [fwd.x, fwd.y, fwd.z],
            "ndc_level_center_xyzw": ndc(level_mid),
            "ndc_4m_ahead_xyzw": ndc(ahead),
            "player_yaw": self.game.yaw,
            "walk_surface_y": self.game.walk_surface_y,
            "boot_vertex_count": self.vert_count,
            "boot_batch_count": self.batch_count,
            "level_bounds_min": [self.level_bounds.min.x, self.level_bounds.min.y, self.level_bounds.min.z],
            "level_bounds_max": [self.level_bounds.max.x, self.level_bounds.max.y, self.level_bounds.max.z],
            "hint": "ndc_* xy should be roughly in [-1,1] if that point is on-screen. If both are way outside, camera/proj may face away from the level. frames_skipped_no_swapchain>0 = swapchain acquire failed. Check console for wgpu uncaptured error lines.",
        })
        .to_string()
    }

    #[wasm_bindgen(js_name = bootDebugJson)]
    pub fn boot_debug_json(&self) -> String {
        let bf = self.npcs.npcs.first().map(|n| n.foot).unwrap_or(Vec3::ZERO);
        let rf = self.npcs.npcs.get(1).map(|n| n.foot).unwrap_or(Vec3::ZERO);
        json!({
            "level_label": self.level_label,
            "vert_count": self.vert_count,
            "batch_count": self.batch_count,
            "bounds_min": [self.level_bounds.min.x, self.level_bounds.min.y, self.level_bounds.min.z],
            "bounds_max": [self.level_bounds.max.x, self.level_bounds.max.y, self.level_bounds.max.z],
            "spawn": [self.game.pos.x, self.game.pos.y, self.game.pos.z],
            "mural_z": self.mural_z,
            "characters_3d_loaded": self.gpu.characters_loaded(),
            "character_rival_loaded": self.gpu.character_rival_loaded(),
            "boss_foot": [bf.x, bf.y, bf.z],
            "rival_foot": [rf.x, rf.y, rf.z],
            "wave": self.npcs.wave + 1,
            "alive_count": self.npcs.alive_count(),
            "walk_surface_y": self.game.walk_surface_y,
            "player_yaw": self.game.yaw,
            "player_pos": [self.game.pos.x, self.game.pos.y, self.game.pos.z],
        })
        .to_string()
    }

    pub fn hud_text(&self) -> String {
        let base = if self.net.joined {
            format!(
                "HP {} · SCORE {} · Q/E weapons · R reload · {}",
                self.net.self_health, self.net.self_score, self.level_label
            )
        } else {
            let mut s = self.net.status.clone();
            s.push_str(" · ");
            s.push_str(&self.level_label);
            s.push_str(&format!(" · {} · {} ENEMIES",
                self.npcs.wave_text(),
                self.npcs.alive_count(),
            ));
            s
        };
        if !self.net.toast.is_empty() {
            format!("{} · {}", base, self.net.toast)
        } else {
            base
        }
    }

    #[wasm_bindgen(js_name = hudBarJson)]
    pub fn hud_bar_json(&self) -> String {
        let mut weapons = Vec::new();
        for i in 0..4 {
            weapons.push(json!({
                "id": i,
                "name": WEAPONS[i].name,
                "clip": self.loadout.clip_for(i),
                "max": WEAPONS[i].max_clip,
                "reserve": self.loadout.reserve_for(i),
                "active": i == self.loadout.current_idx(),
            }));
        }
        json!({
            "health": self.net.self_health,
            "armor": 0,
            "weapons": weapons,
            "joined": self.net.joined,
            "blood_splat": self.blood_splat,
        })
        .to_string()
    }

    pub fn tick(&mut self, time_ms: f64) {
        let dt = if self.last_ms > 0.0 {
            ((time_ms - self.last_ms) / 1000.0).clamp(0.0, 0.1) as f32
        } else {
            0.0
        };
        self.last_ms = time_ms;
        self.game_time += dt;

        self.game.set_online(self.net.joined);
        if self.net.joined {
            if let Some((x, z)) = self.net.target_xz_for_self() {
                self.game.set_net_target_xz(x, z);
            }
        }

        self.game.apply_look(&mut self.input);
        if dt > 0.0 {
            self.loadout.tick(dt);
            self.game.tick(dt, &mut self.input);
            self.blood_splat = (self.blood_splat - dt * 2.65).max(0.0);
        }

        let (wp, wn, pick, rel) = self.input.take_weapon_edges();
        self.loadout.handle_edges(wp, wn, pick, rel);

        let shoot_click = self.input.take_shoot_edge();
        let shot_fired = if shoot_click {
            self.loadout.try_fire()
        } else {
            false
        };
        let shoot_net = self.net.joined && shot_fired;
        if self.net.joined {
            self.net
                .pump_input(time_ms, &self.game, &self.input, shoot_net);
        }
        if shot_fired {
            let wi = self.loadout.current_idx();
            // Check HP before and after to detect hits and kills
            let hp_before: Vec<(f32, bool)> = self.npcs.npcs.iter()
                .map(|n| (n.hp, n.alive()))
                .collect();
            let hit = self.npcs.register_shot(&self.game, wi);
            if hit {
                self.last_hit = true;
                self.blood_splat = (self.blood_splat + 0.92).min(1.0);
            }
            // Check for kills
            for (i, npc) in self.npcs.npcs.iter().enumerate() {
                if hp_before[i].1 && !npc.alive() {
                    self.last_kill = npc.def.label.to_uppercase();
                }
            }
            // Debug log every shot
            #[cfg(target_arch = "wasm32")]
            {
                let eye = self.game.eye_pos();
                let dir = self.game.view_forward();
                let alive = self.npcs.alive_count();
                let msg = format!(
                    "SHOT wi={} hit={} alive={} wave={} eye=({:.1},{:.1},{:.1}) dir=({:.2},{:.2},{:.2})",
                    wi, hit, alive, self.npcs.wave + 1,
                    eye.x, eye.y, eye.z, dir.x, dir.y, dir.z
                );
                web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&msg));
            }
        }

        // Tick NPC AI (patrol, chase, death)
        if dt > 0.0 {
            self.npcs.tick(dt, self.game.pos, &self.level_bounds);
            // Sync NPC foot.y with terrain so hitboxes match visual positions
            for npc in &mut self.npcs.npcs {
                npc.foot.y = self.game.feet_draw_y(npc.foot.x, npc.foot.z);
            }
            if !self.net.joined {
                let d = self.npcs.offline_shoot_damage_per_tick(dt);
                if d > 0 {
                    self.net.self_health = (self.net.self_health - d).max(0);
                }
            }
        }
    }

    pub fn upload_reference_sprite(&mut self, img: web_sys::HtmlImageElement) -> Result<(), JsValue> {
        self.gpu.upload_reference_sprite(&img)
    }

    #[wasm_bindgen(js_name = uploadWeaponSprite)]
    pub fn upload_weapon_sprite(
        &mut self,
        slot: u32,
        img: web_sys::HtmlImageElement,
    ) -> Result<(), JsValue> {
        self.gpu.upload_weapon_sprite(slot, &img)
    }

    #[wasm_bindgen(js_name = uploadArmsSprite)]
    pub fn upload_arms_sprite(&mut self, img: web_sys::HtmlImageElement) -> Result<(), JsValue> {
        self.gpu.upload_arms_sprite(&img)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
    }

    pub fn render_frame(&mut self) -> Result<(), JsValue> {
        let aspect =
            self.gpu.config.width.max(1) as f32 / self.gpu.config.height.max(1) as f32;
        let vp = self.game.view_proj(aspect);
        let cam = self.game.pos + Vec3::new(0.0, 1.65, 0.0);
        let mut characters: Vec<CharacterInstance> = Vec::new();
        let you = self.net.entity_id;
        if self.gpu.characters_loaded() {
            for npc in &self.npcs.npcs {
                // Skip fully faded dead NPCs
                if npc.state == npc::NpcState::Dead && npc.death_timer >= 1.0 {
                    continue;
                }
                // Skip dead NPCs that haven't started dying (shouldn't happen but safety)
                if !npc.alive() && npc.state != npc::NpcState::Dead {
                    continue;
                }
                let f = npc.foot;
                let bob_y = if self.gpu.char_sprite_billboard_active() {
                    0.0
                } else {
                    walk_bob_y(self.game_time, npc.speed)
                };
                let foot_y = self.game.feet_draw_y(f.x, f.z) + bob_y;
                let facing_yaw = npc.yaw; // NPC's actual facing direction for 3D rotation
                let skin = match npc.def.skin {
                    npc::NpcSkin::Rival => CharacterSkin::Rival,
                    npc::NpcSkin::Boss => CharacterSkin::Boss,
                };
                let mut ch = make_character(
                    Vec3::new(f.x, foot_y, f.z),
                    facing_yaw,
                    0.78 * npc.scale(),
                    skin,
                );
                let rows = self.gpu.char_sprite_rows_for_skin(skin);
                ch.anim_frame = npc_billboard_anim_frame(self.game_time, npc, rows);
                ch.bill_tint = npc_sprite_billboard_tint(npc);
                characters.push(ch);
            }
            if self.net.joined {
                for p in &self.net.players {
                    if p.health <= 0 {
                        continue;
                    }
                    if Some(p.id) == you {
                        continue;
                    }
                    let foot_y = self.game.feet_draw_y(p.x, p.z);
                    let sc = 0.72 + (p.id % 3) as f32 * 0.04;
                    let mesh_yaw = yaw_face_cam_xz(
                        Vec3::new(p.x, 0.0, p.z),
                        Vec3::new(cam.x, 0.0, cam.z),
                    );
                    let mut ch = make_character(
                        Vec3::new(p.x, foot_y, p.z),
                        mesh_yaw,
                        sc,
                        CharacterSkin::Remote,
                    );
                    // Remote players are only visible when moving, so walk anim
                    ch.anim_frame = walk_anim_frame(self.game_time, 1.0);
                    characters.push(ch);
                }
            }
            // Offline: no demo characters or self-body in first person
            // (those were billboard sprites; 3D models clip into the camera)
        }
        let weapon_hud = WeaponHudParams {
            weapon_id: self.loadout.current_idx() as u32,
            bob: self.last_ms as f32 * 0.0028,
            flash: self.loadout.muzzle_flash,
            recoil: self.loadout.recoil,
            reload: self.loadout.reload_anim,
        };
        self.gpu.draw_world(
            vp,
            self.clear,
            cam,
            &characters,
            weapon_hud,
            &self.level_bounds,
            self.mural_z,
        );
        Ok(())
    }
}

#[wasm_bindgen(js_name = createOyabaunApp)]
pub async fn create_oyabaun_app(canvas: HtmlCanvasElement) -> Result<OyabaunApp, JsValue> {
    console_error_panic_hook::set_once();
    let gi = load_game_init().await;
    let boot = gi.boot;
    let solids = boot.arena.solids.clone();
    #[cfg(target_arch = "wasm32")]
    let character_cpu = {
        const EMB_CHAR: &[u8] = include_bytes!("../characters/oyabaun_player.glb");
        let mut c = gltf_level::parse_character_glb(EMB_CHAR).ok();
        let char_url = format!("./characters/oyabaun_player.glb?v={}", js_sys::Date::now() as u64);
        if let Some(bytes) = fetch_bytes(&char_url).await {
            if let Ok(x) = gltf_level::parse_character_glb(&bytes) {
                c = Some(x);
            }
        }
        c
    };
    #[cfg(not(target_arch = "wasm32"))]
    let character_cpu = {
        const EMB_CHAR: &[u8] = include_bytes!("../characters/oyabaun_player.glb");
        gltf_level::parse_character_glb(EMB_CHAR).ok()
    };
    #[cfg(target_arch = "wasm32")]
    let character_rival_cpu = {
        const EMB_RIVAL: &[u8] = include_bytes!("../characters/oyabaun_rival.glb");
        let mut c = gltf_level::parse_character_glb(EMB_RIVAL).ok();
        let url = format!(
            "./characters/oyabaun_rival.glb?v={}",
            js_sys::Date::now() as u64
        );
        if let Some(bytes) = fetch_bytes(&url).await {
            if let Ok(x) = gltf_level::parse_character_glb(&bytes) {
                c = Some(x);
            }
        }
        c
    };
    #[cfg(not(target_arch = "wasm32"))]
    let character_rival_cpu = {
        const EMB_RIVAL: &[u8] = include_bytes!("../characters/oyabaun_rival.glb");
        gltf_level::parse_character_glb(EMB_RIVAL).ok()
    };
    let gpu = Gpu::new(
        canvas,
        &boot.arena.vertices,
        &boot.arena.indices,
        gi.gltf,
        character_cpu,
        character_rival_cpu,
    )
    .await?;
    let game = GameState::new(
        boot.spawn,
        solids,
        boot.spawn_yaw,
        boot.level_bounds.min.y,
    );
    let mut net = NetController::new();
    net.status = String::from("open page — WebSocket + Nostr extension");
    Ok(OyabaunApp {
        gpu,
        game,
        input: InputState::default(),
        net,
        loadout: Loadout::new(),
        npcs: {
            let mut nm = NpcManager::new(boot.boss_foot, boot.rival_foot);
            nm.init_patrols(&boot.level_bounds);
            nm
        },
        last_ms: 0.0,
        game_time: 0.0,
        last_hit: false,
        last_kill: String::new(),
        blood_splat: 0.0,
        clear: Vec3::new(0.14, 0.12, 0.20),
        level_bounds: boot.level_bounds,
        mural_z: boot.mural_z,
        level_label: gi.level_label,
        vert_count: gi.vert_count,
        batch_count: gi.batch_count,
    })
}
