use glam::{Mat4, Quat, Vec3};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

mod boss;
mod game;
mod gltf_level;
mod input;
mod loadout;
mod mesh;
mod net;
mod npc;
mod render;
mod street_dressing;

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
    Mat4::from_scale_rotation_translation(Vec3::splat(scale), Quat::from_rotation_y(yaw), foot)
}

fn make_character(foot: Vec3, mesh_yaw: f32, scale: f32, skin: CharacterSkin) -> CharacterInstance {
    CharacterInstance {
        model: character_model(foot, mesh_yaw, scale),
        mesh_yaw,
        skin,
    }
}

fn yaw_face_cam_xz(foot: Vec3, cam: Vec3) -> f32 {
    let dx = cam.x - foot.x;
    let dz = cam.z - foot.z;
    (-dz).atan2(dx)
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
fn game_init_from_gltf(mut cpu: gltf_level::GltfLevelCpu) -> GameInit {
    street_dressing::append_tokyo_facades(&mut cpu);
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

    pub fn ingest_server_json(&mut self, json: &str) {
        self.net.ingest(json);
        self.game.set_online(self.net.joined);
    }

    pub fn take_net_outbound(&mut self) -> Option<String> {
        self.net.take_outbound()
    }

    #[wasm_bindgen(js_name = bootDebugJson)]
    pub fn boot_debug_json(&self) -> String {
        let bf = self.npcs.boss().foot;
        let rf = self.npcs.rival().foot;
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
            "boss_alive": self.npcs.boss().alive(),
            "rival_alive": self.npcs.rival().alive(),
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
            if self.npcs.boss().alive() {
                s.push_str(&format!(" · BOSS {:.0}%", self.npcs.boss().hp_frac() * 100.0));
            } else {
                s.push_str(" · BOSS DEFEATED");
            }
            if self.npcs.rival().alive() {
                s.push_str(&format!(" · RIVAL {:.0}%", self.npcs.rival().hp_frac() * 100.0));
            } else {
                s.push_str(" · RIVAL DOWN");
            }
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
            self.npcs.register_shot(&self.game, wi);
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
                if !npc.alive() {
                    continue;
                }
                let f = npc.foot;
                let foot_y = self.game.feet_draw_y(f.x, f.z);
                let mesh_yaw = yaw_face_cam_xz(
                    Vec3::new(f.x, 0.0, f.z),
                    Vec3::new(cam.x, 0.0, cam.z),
                );
                let skin = if npc.def.label == npc::RIVAL_DEF.label {
                    CharacterSkin::Rival
                } else {
                    CharacterSkin::Boss
                };
                characters.push(make_character(
                    Vec3::new(f.x, foot_y, f.z),
                    mesh_yaw,
                    0.72 * npc.scale(),
                    skin,
                ));
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
                    let sc = 0.66 + (p.id % 3) as f32 * 0.04;
                    let mesh_yaw = yaw_face_cam_xz(
                        Vec3::new(p.x, 0.0, p.z),
                        Vec3::new(cam.x, 0.0, cam.z),
                    );
                    characters.push(make_character(
                        Vec3::new(p.x, foot_y, p.z),
                        mesh_yaw,
                        sc,
                        CharacterSkin::Remote,
                    ));
                }
            } else {
                // Offline demo: place characters at fixed world positions
                // (midway between boss and rival, plus two flanking spots)
                let bf = self.npcs.boss().foot;
                let rf = self.npcs.rival().foot;
                let mid = (bf + rf) * 0.5;
                let spots = [
                    Vec3::new(mid.x + 2.0, 0.0, mid.z),
                    Vec3::new(mid.x - 2.5, 0.0, mid.z - 3.0),
                    Vec3::new(mid.x + 0.5, 0.0, mid.z + 4.0),
                ];
                for pos in &spots {
                    let foot_y = self.game.feet_draw_y(pos.x, pos.z);
                    let mesh_yaw = yaw_face_cam_xz(
                        Vec3::new(pos.x, 0.0, pos.z),
                        Vec3::new(cam.x, 0.0, cam.z),
                    );
                    characters.push(make_character(
                        Vec3::new(pos.x, foot_y, pos.z),
                        mesh_yaw,
                        0.88,
                        CharacterSkin::Remote,
                    ));
                }
            }
            // Offline: show your own body with the same GLB/atlas (first-person otherwise has no mesh).
            if !self.net.joined {
                let gp = self.game.pos;
                let foot_y = self.game.feet_draw_y(gp.x, gp.z);
                let mesh_yaw = yaw_face_cam_xz(
                    Vec3::new(gp.x, 0.0, gp.z),
                    Vec3::new(cam.x, 0.0, cam.z),
                );
                characters.push(make_character(
                    Vec3::new(gp.x, foot_y, gp.z),
                    mesh_yaw,
                    0.64,
                    CharacterSkin::Remote,
                ));
            }
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
        npcs: NpcManager::new(boot.boss_foot, boot.rival_foot),
        last_ms: 0.0,
        clear: Vec3::new(0.045, 0.038, 0.072),
        level_bounds: boot.level_bounds,
        mural_z: boot.mural_z,
        level_label: gi.level_label,
        vert_count: gi.vert_count,
        batch_count: gi.batch_count,
    })
}
