use glam::Vec3;

use crate::game::GameState;
use crate::mesh::Aabb;

const FOOT: Vec3 = Vec3::new(11.85, 0.0, -11.85);
const SCALE: f32 = 1.22;
const DAMAGE: [f32; 4] = [24.0, 42.0, 16.0, 30.0];

pub struct BossState {
    hp: f32,
    max_hp: f32,
}

impl BossState {
    pub fn new() -> Self {
        Self {
            hp: 220.0,
            max_hp: 220.0,
        }
    }

    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }

    pub fn foot(&self) -> Vec3 {
        FOOT
    }

    pub fn scale(&self) -> f32 {
        SCALE
    }

    pub fn hp_frac(&self) -> f32 {
        (self.hp / self.max_hp).clamp(0.0, 1.0)
    }

    fn hit_aabb() -> Aabb {
        let sc = SCALE;
        let pad = 0.68 * sc;
        Aabb {
            min: Vec3::new(FOOT.x - pad, 0.05, FOOT.z - pad),
            max: Vec3::new(FOOT.x + pad, 2.55 * sc, FOOT.z + pad),
        }
    }

    pub fn register_shot(&mut self, game: &GameState, weapon_idx: usize) {
        if !self.alive() {
            return;
        }
        let eye = game.eye_pos();
        let dir = game.view_forward();
        if dir.length_squared() < 1e-8 {
            return;
        }
        if let Some(t) = ray_aabb(eye, dir, &Self::hit_aabb()) {
            if t > 0.02 && t < 120.0 {
                let d = DAMAGE[weapon_idx.min(3)];
                self.hp = (self.hp - d).max(0.0);
            }
        }
    }
}

const RIVAL_FOOT: Vec3 = Vec3::new(-10.2, 0.0, -9.4);
const RIVAL_SCALE: f32 = 1.08;
const RIVAL_HP: f32 = 140.0;

pub struct RivalState {
    hp: f32,
    max_hp: f32,
}

impl RivalState {
    pub fn new() -> Self {
        Self {
            hp: RIVAL_HP,
            max_hp: RIVAL_HP,
        }
    }

    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }

    pub fn foot(&self) -> Vec3 {
        RIVAL_FOOT
    }

    pub fn scale(&self) -> f32 {
        RIVAL_SCALE
    }

    pub fn hp_frac(&self) -> f32 {
        (self.hp / self.max_hp).clamp(0.0, 1.0)
    }

    fn hit_aabb() -> Aabb {
        let sc = RIVAL_SCALE;
        let pad = 0.62 * sc;
        Aabb {
            min: Vec3::new(RIVAL_FOOT.x - pad, 0.05, RIVAL_FOOT.z - pad),
            max: Vec3::new(RIVAL_FOOT.x + pad, 2.45 * sc, RIVAL_FOOT.z + pad),
        }
    }

    pub fn register_shot(&mut self, game: &GameState, weapon_idx: usize) {
        if !self.alive() {
            return;
        }
        let eye = game.eye_pos();
        let dir = game.view_forward();
        if dir.length_squared() < 1e-8 {
            return;
        }
        if let Some(t) = ray_aabb(eye, dir, &Self::hit_aabb()) {
            if t > 0.02 && t < 120.0 {
                let d = DAMAGE[weapon_idx.min(3)];
                self.hp = (self.hp - d).max(0.0);
            }
        }
    }
}

fn ray_aabb(origin: Vec3, dir: Vec3, aabb: &Aabb) -> Option<f32> {
    let mut tmin = 0.0_f32;
    let mut tmax = f32::INFINITY;
    let o = [origin.x, origin.y, origin.z];
    let d = [dir.x, dir.y, dir.z];
    let mn = [aabb.min.x, aabb.min.y, aabb.min.z];
    let mx = [aabb.max.x, aabb.max.y, aabb.max.z];
    for i in 0..3 {
        if d[i].abs() < 1e-8 {
            if o[i] < mn[i] || o[i] > mx[i] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d[i];
        let mut t0 = (mn[i] - o[i]) * inv;
        let mut t1 = (mx[i] - o[i]) * inv;
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        tmin = tmin.max(t0);
        tmax = tmax.min(t1);
        if tmin > tmax {
            return None;
        }
    }
    if tmax < 0.0 {
        return None;
    }
    let t = if tmin >= 0.0 { tmin } else { tmax };
    Some(t)
}
