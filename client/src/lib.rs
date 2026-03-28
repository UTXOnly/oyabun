use glam::Vec3;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

mod boss;
mod game;
mod input;
mod loadout;
mod mesh;
mod net;
mod render;

use boss::{BossState, RivalState};
use game::GameState;
use input::InputState;
use loadout::{Loadout, WEAPONS};
use mesh::build_arena;
use net::NetController;
use render::WeaponHudParams;
pub use render::{Gpu, Vertex};

use serde_json::json;

#[wasm_bindgen]
pub struct OyabaunApp {
    gpu: Gpu,
    game: GameState,
    input: InputState,
    net: NetController,
    loadout: Loadout,
    boss: BossState,
    rival: RivalState,
    last_ms: f64,
    clear: Vec3,
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

    pub fn hud_text(&self) -> String {
        let base = if self.net.joined {
            format!(
                "HP {} · SCORE {} · Q/E weapons · R reload",
                self.net.self_health, self.net.self_score
            )
        } else {
            let mut s = self.net.status.clone();
            if self.boss.alive() {
                s.push_str(&format!(" · BOSS {:.0}%", self.boss.hp_frac() * 100.0));
            } else {
                s.push_str(" · BOSS DEFEATED");
            }
            if self.rival.alive() {
                s.push_str(&format!(" · RIVAL {:.0}%", self.rival.hp_frac() * 100.0));
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
            self.boss.register_shot(&self.game, wi);
            self.rival.register_shot(&self.game, wi);
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

    #[wasm_bindgen(js_name = uploadBossSprite)]
    pub fn upload_boss_sprite(&mut self, img: web_sys::HtmlImageElement) -> Result<(), JsValue> {
        self.gpu.upload_boss_sprite(&img)
    }

    #[wasm_bindgen(js_name = uploadRivalSprite)]
    pub fn upload_rival_sprite(&mut self, img: web_sys::HtmlImageElement) -> Result<(), JsValue> {
        self.gpu.upload_rival_sprite(&img)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
    }

    pub fn render_frame(&mut self) -> Result<(), JsValue> {
        let aspect =
            self.gpu.config.width.max(1) as f32 / self.gpu.config.height.max(1) as f32;
        let vp = self.game.view_proj(aspect);
        let cam = self.game.pos + Vec3::new(0.0, 1.65, 0.0);
        let mut bills: Vec<(Vec3, f32)> = Vec::new();
        let you = self.net.entity_id;
        if self.net.joined {
            if self.net.self_health > 0 {
                let sy = self.game.yaw.sin();
                let cy = self.game.yaw.cos();
                let bx = self.game.pos.x - sy * 1.05;
                let bz = self.game.pos.z + cy * 1.05;
                bills.push((Vec3::new(bx, 0.0, bz), 0.62));
            }
            for p in &self.net.players {
                if p.health <= 0 {
                    continue;
                }
                if Some(p.id) == you {
                    continue;
                }
                let s = 0.9 + (p.id % 3) as f32 * 0.06;
                bills.push((Vec3::new(p.x, 0.0, p.z), s));
            }
        } else {
            bills.push((Vec3::new(5.5, 0.0, -5.0), 1.0));
            bills.push((Vec3::new(-6.0, 0.0, 3.5), 0.88));
            bills.push((Vec3::new(-2.0, 0.0, -8.0), 0.92));
        }
        let weapon_hud = WeaponHudParams {
            weapon_id: self.loadout.current_idx() as u32,
            bob: self.last_ms as f32 * 0.0028,
            flash: self.loadout.muzzle_flash,
        };
        let boss_draw = if self.boss.alive() {
            Some((self.boss.foot(), self.boss.scale()))
        } else {
            None
        };
        let rival_draw = if self.rival.alive() {
            Some((self.rival.foot(), self.rival.scale()))
        } else {
            None
        };
        self.gpu
            .draw_world(vp, self.clear, cam, &bills, weapon_hud, boss_draw, rival_draw);
        Ok(())
    }
}

#[wasm_bindgen(js_name = createOyabaunApp)]
pub async fn create_oyabaun_app(canvas: HtmlCanvasElement) -> Result<OyabaunApp, JsValue> {
    console_error_panic_hook::set_once();
    let arena = build_arena();
    let solids = arena.solids.clone();
    let gpu = Gpu::new(canvas, &arena.vertices, &arena.indices).await?;
    let game = GameState::new(Vec3::new(0.0, 0.0, 9.0), solids);
    let mut net = NetController::new();
    net.status = String::from("open page — WebSocket + Nostr extension");
    Ok(OyabaunApp {
        gpu,
        game,
        input: InputState::default(),
        net,
        loadout: Loadout::new(),
        boss: BossState::new(),
        rival: RivalState::new(),
        last_ms: 0.0,
        clear: Vec3::new(0.022, 0.01, 0.045),
    })
}
