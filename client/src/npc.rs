use glam::Vec3;
use std::f32::consts::PI;

use crate::game::GameState;
use crate::mesh::Aabb;

/// Smoothly interpolate yaw toward a target, taking the shortest arc.
fn smooth_turn(current: f32, target: f32, max_delta: f32) -> f32 {
    let mut diff = target - current;
    // Wrap to [-PI, PI]
    while diff > PI {
        diff -= 2.0 * PI;
    }
    while diff < -PI {
        diff += 2.0 * PI;
    }
    if diff.abs() < max_delta {
        target
    } else {
        current + diff.signum() * max_delta
    }
}

/// Per-weapon damage table shared by all NPCs (M4A1-family loadout slots).
const DAMAGE: [f32; 4] = [28.0, 28.0, 26.0, 32.0];

/// NPC movement speeds.
const NPC_WALK_SPEED: f32 = 1.8;
const NPC_CHASE_SPEED: f32 = 3.2;
const NPC_CHASE_RANGE: f32 = 40.0;
/// Within this XZ distance, NPC stops and plays shoot stance (ranged).
pub const NPC_SHOOT_RANGE: f32 = 7.5;
const NPC_PATROL_PAUSE: f32 = 1.5;

/// Which character skin to render for this NPC.
#[derive(Clone, Copy, PartialEq)]
pub enum NpcSkin {
    Boss,
    Rival,
}

/// Configuration for an NPC archetype.
#[derive(Clone)]
pub struct NpcDef {
    pub label: &'static str,
    pub max_hp: f32,
    pub scale: f32,
    pub hitbox_pad: f32,
    pub hitbox_height: f32,
    pub skin: NpcSkin,
}

// ── NPC archetypes ──────────────────────────────────────────────────

pub const BOSS_DEF: NpcDef = NpcDef {
    label: "boss",
    max_hp: 220.0,
    scale: 1.22,
    hitbox_pad: 1.20,
    hitbox_height: 3.0,
    skin: NpcSkin::Boss,
};

pub const RIVAL_DEF: NpcDef = NpcDef {
    label: "rival",
    max_hp: 140.0,
    scale: 1.08,
    hitbox_pad: 1.10,
    hitbox_height: 2.9,
    skin: NpcSkin::Rival,
};

/// Thug — weaker grunt, appears in numbers.
const THUG_DEF: NpcDef = NpcDef {
    label: "thug",
    max_hp: 80.0,
    scale: 0.95,
    hitbox_pad: 1.05,
    hitbox_height: 2.7,
    skin: NpcSkin::Rival,
};

/// Enforcer — tougher mid-tier enemy.
const ENFORCER_DEF: NpcDef = NpcDef {
    label: "enforcer",
    max_hp: 160.0,
    scale: 1.12,
    hitbox_pad: 1.15,
    hitbox_height: 2.7,
    skin: NpcSkin::Boss,
};

/// Heavy — big slow tank.
const HEAVY_DEF: NpcDef = NpcDef {
    label: "heavy",
    max_hp: 280.0,
    scale: 1.30,
    hitbox_pad: 1.25,
    hitbox_height: 3.1,
    skin: NpcSkin::Boss,
};

/// Assassin — fast, fragile.
const ASSASSIN_DEF: NpcDef = NpcDef {
    label: "assassin",
    max_hp: 60.0,
    scale: 0.90,
    hitbox_pad: 1.00,
    hitbox_height: 2.6,
    skin: NpcSkin::Rival,
};

// ── Wave definitions ────────────────────────────────────────────────

/// Each wave defines which NPC types spawn and how they're spread.
struct WaveDef {
    defs: &'static [&'static NpcDef],
}

static WAVE_1: WaveDef = WaveDef {
    defs: &[&BOSS_DEF, &RIVAL_DEF],
};
static WAVE_2: WaveDef = WaveDef {
    defs: &[&THUG_DEF, &THUG_DEF, &ENFORCER_DEF],
};
static WAVE_3: WaveDef = WaveDef {
    defs: &[&ASSASSIN_DEF, &ENFORCER_DEF, &THUG_DEF, &ASSASSIN_DEF],
};
static WAVE_4: WaveDef = WaveDef {
    defs: &[&HEAVY_DEF, &THUG_DEF, &THUG_DEF, &ENFORCER_DEF, &ASSASSIN_DEF],
};
static WAVE_5: WaveDef = WaveDef {
    defs: &[&HEAVY_DEF, &ENFORCER_DEF, &ENFORCER_DEF, &ASSASSIN_DEF, &ASSASSIN_DEF, &THUG_DEF],
};

static WAVES: [&WaveDef; 5] = [&WAVE_1, &WAVE_2, &WAVE_3, &WAVE_4, &WAVE_5];

// ── NPC state ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum NpcState {
    Patrol,
    Idle,
    Chase,
    Dead,
}

pub struct Npc {
    pub def: NpcDef,
    pub foot: Vec3,
    pub hp: f32,
    pub state: NpcState,
    pub yaw: f32,
    pub speed: f32,
    patrol_a: Vec3,
    patrol_b: Vec3,
    patrol_toward_b: bool,
    idle_timer: f32,
    pub death_timer: f32,
    pub hit_flash: f32,
    /// Seconds accumulated while holding shoot stance (drives shoot row cycle on client).
    pub shoot_anim_t: f32,
    /// Last planar distance to player this tick (for anim / damage).
    pub chase_dist: f32,
}

impl Npc {
    pub fn new(def: NpcDef, foot: Vec3) -> Self {
        let hp = def.max_hp;
        Self {
            def,
            foot,
            hp,
            state: NpcState::Patrol,
            yaw: 0.0,
            speed: 0.0,
            patrol_a: foot,
            patrol_b: foot,
            patrol_toward_b: true,
            idle_timer: 0.0,
            death_timer: 0.0,
            hit_flash: 0.0,
            shoot_anim_t: 0.0,
            chase_dist: 999.0,
        }
    }

    pub fn set_patrol(&mut self, a: Vec3, b: Vec3) {
        self.patrol_a = a;
        self.patrol_b = b;
        self.patrol_toward_b = true;
        self.state = NpcState::Patrol;
    }

    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }

    pub fn scale(&self) -> f32 {
        self.def.scale
    }

    pub fn hp_frac(&self) -> f32 {
        (self.hp / self.def.max_hp).clamp(0.0, 1.0)
    }

    pub fn shooting_at_player(&self) -> bool {
        self.alive()
            && self.state == NpcState::Chase
            && self.chase_dist <= NPC_SHOOT_RANGE
    }

    fn hit_aabb(&self) -> Aabb {
        let sc = self.def.scale;
        let pad = self.def.hitbox_pad * sc;
        let f = self.foot;
        Aabb {
            min: Vec3::new(f.x - pad, f.y + 0.05, f.z - pad),
            max: Vec3::new(f.x + pad, f.y + self.def.hitbox_height * sc, f.z + pad),
        }
    }

    pub fn tick(&mut self, dt: f32, player_pos: Vec3, bounds: &Aabb) {
        self.hit_flash = (self.hit_flash - dt * 6.0).max(0.0);

        if !self.alive() {
            self.state = NpcState::Dead;
            self.speed = 0.0;
            self.death_timer = (self.death_timer + dt * 1.5).min(1.0);
            return;
        }

        let dx = player_pos.x - self.foot.x;
        let dz = player_pos.z - self.foot.z;
        let dist_to_player = (dx * dx + dz * dz).sqrt();
        self.chase_dist = dist_to_player;

        // Chase speed varies by NPC type
        let chase_speed = match self.def.label {
            "assassin" => 4.2,
            "heavy" => 2.4,
            _ => NPC_CHASE_SPEED,
        };

        // Smooth yaw turning speed (radians/sec) — NPCs don't snap-turn
        let turn_speed = 4.5;

        match self.state {
            NpcState::Patrol => {
                let target = if self.patrol_toward_b {
                    self.patrol_b
                } else {
                    self.patrol_a
                };
                let tx = target.x - self.foot.x;
                let tz = target.z - self.foot.z;
                let tdist = (tx * tx + tz * tz).sqrt();

                if tdist < 0.5 {
                    self.patrol_toward_b = !self.patrol_toward_b;
                    self.state = NpcState::Idle;
                    self.idle_timer = NPC_PATROL_PAUSE;
                    self.speed = 0.0;
                } else {
                    let dir_x = tx / tdist;
                    let dir_z = tz / tdist;
                    // Face movement direction
                    let target_yaw = dir_x.atan2(-dir_z);
                    self.yaw = smooth_turn(self.yaw, target_yaw, turn_speed * dt);
                    self.speed = NPC_WALK_SPEED;
                    self.foot.x += dir_x * NPC_WALK_SPEED * dt;
                    self.foot.z += dir_z * NPC_WALK_SPEED * dt;
                }

                if dist_to_player < NPC_CHASE_RANGE {
                    self.state = NpcState::Chase;
                }
            }
            NpcState::Idle => {
                self.speed = 0.0;
                self.idle_timer -= dt;
                // Slowly turn toward player while idle
                if dist_to_player < 50.0 {
                    let target_yaw = dx.atan2(-dz);
                    self.yaw = smooth_turn(self.yaw, target_yaw, turn_speed * 0.5 * dt);
                }
                if self.idle_timer <= 0.0 {
                    self.state = NpcState::Patrol;
                }
                if dist_to_player < NPC_CHASE_RANGE {
                    self.state = NpcState::Chase;
                }
            }
            NpcState::Chase => {
                if dist_to_player > NPC_CHASE_RANGE * 1.5 {
                    self.state = NpcState::Patrol;
                    self.shoot_anim_t = 0.0;
                } else if dist_to_player <= NPC_SHOOT_RANGE {
                    self.speed = 0.0;
                    self.shoot_anim_t += dt;
                    let target_yaw = dx.atan2(-dz);
                    self.yaw = smooth_turn(self.yaw, target_yaw, turn_speed * dt);
                } else {
                    self.shoot_anim_t = 0.0;
                    let dir_x = dx / dist_to_player;
                    let dir_z = dz / dist_to_player;
                    let target_yaw = dir_x.atan2(-dir_z);
                    self.yaw = smooth_turn(self.yaw, target_yaw, turn_speed * dt);
                    self.speed = chase_speed;
                    self.foot.x += dir_x * chase_speed * dt;
                    self.foot.z += dir_z * chase_speed * dt;
                }
            }
            NpcState::Dead => {
                self.speed = 0.0;
            }
        }

        let pad = 0.5;
        self.foot.x = self.foot.x.clamp(bounds.min.x + pad, bounds.max.x - pad);
        self.foot.z = self.foot.z.clamp(bounds.min.z + pad, bounds.max.z - pad);
    }

}

// ── NPC Manager with wave spawning ──────────────────────────────────

pub struct NpcManager {
    pub npcs: Vec<Npc>,
    /// Current wave index (0-based).
    pub wave: usize,
    /// Delay timer before next wave spawns (seconds).
    wave_delay: f32,
    /// Whether we're waiting to spawn the next wave.
    wave_pending: bool,
}

impl NpcManager {
    pub fn new(boss_foot: Vec3, rival_foot: Vec3) -> Self {
        Self {
            npcs: vec![
                Npc::new(BOSS_DEF, boss_foot),
                Npc::new(RIVAL_DEF, rival_foot),
            ],
            wave: 0,
            wave_delay: 0.0,
            wave_pending: false,
        }
    }

    /// Set up patrol routes for current NPCs.
    pub fn init_patrols(&mut self, bounds: &Aabb) {
        Self::setup_patrols(&mut self.npcs, bounds);
    }

    fn setup_patrols(npcs: &mut [Npc], bounds: &Aabb) {
        let cx = (bounds.min.x + bounds.max.x) * 0.5;
        for (i, npc) in npcs.iter_mut().enumerate() {
            let f = npc.foot;
            let patrol_range = 5.0 + (i as f32) * 1.5;
            let a = Vec3::new(
                f.x.clamp(bounds.min.x + 1.0, bounds.max.x - 1.0),
                f.y,
                (f.z - patrol_range).clamp(bounds.min.z + 1.0, bounds.max.z - 1.0),
            );
            let b = Vec3::new(
                (f.x + (cx - f.x) * 0.3).clamp(bounds.min.x + 1.0, bounds.max.x - 1.0),
                f.y,
                (f.z + patrol_range).clamp(bounds.min.z + 1.0, bounds.max.z - 1.0),
            );
            npc.set_patrol(a, b);
        }
    }

    /// Spawn the next wave of NPCs ahead of the player.
    fn spawn_wave(&mut self, player_pos: Vec3, bounds: &Aabb) {
        self.wave += 1;
        let wave_idx = self.wave.min(WAVES.len() - 1);
        let wave_def = WAVES[wave_idx];

        // Remove fully dead NPCs (keep alive ones if any)
        self.npcs.retain(|n| n.alive());

        let count = wave_def.defs.len();
        let cx = (bounds.min.x + bounds.max.x) * 0.5;
        let alley_width = bounds.max.x - bounds.min.x;

        // Spawn ahead of the player along the alley
        // Spread NPCs across the alley width and stagger depth
        for (i, def) in wave_def.defs.iter().enumerate() {
            let spread = if count <= 1 {
                0.0
            } else {
                let t = i as f32 / (count - 1) as f32; // 0..1
                (t - 0.5) * (alley_width * 0.6) // spread across 60% of alley width
            };
            let depth = 10.0 + (i as f32) * 3.0; // stagger depth 10-25m ahead
            let spawn_x = (cx + spread).clamp(bounds.min.x + 1.5, bounds.max.x - 1.5);
            // Spawn ahead of player (toward negative Z typically, but use bounds center)
            let cz = (bounds.min.z + bounds.max.z) * 0.5;
            let toward_center_z = cz - player_pos.z;
            let dir_z = if toward_center_z.abs() > 0.1 {
                toward_center_z.signum()
            } else {
                -1.0
            };
            let spawn_z = (player_pos.z + dir_z * depth)
                .clamp(bounds.min.z + 2.0, bounds.max.z - 2.0);

            let foot = Vec3::new(spawn_x, 0.0, spawn_z);
            self.npcs.push(Npc::new((*def).clone(), foot));
        }

        // Set up patrols for newly spawned NPCs
        Self::setup_patrols(&mut self.npcs, bounds);
    }

    /// Tick all NPCs + wave management.
    pub fn tick(&mut self, dt: f32, player_pos: Vec3, bounds: &Aabb) {
        // Update all NPCs
        for npc in &mut self.npcs {
            npc.tick(dt, player_pos, bounds);
        }

        // NPC-NPC separation
        let min_sep = 2.2;
        let n = self.npcs.len();
        for i in 0..n {
            for j in (i + 1)..n {
                if !self.npcs[i].alive() || !self.npcs[j].alive() {
                    continue;
                }
                let dx = self.npcs[j].foot.x - self.npcs[i].foot.x;
                let dz = self.npcs[j].foot.z - self.npcs[i].foot.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist < min_sep && dist > 0.001 {
                    let push = (min_sep - dist) * 0.5;
                    let nx = dx / dist;
                    let nz = dz / dist;
                    self.npcs[i].foot.x -= nx * push;
                    self.npcs[i].foot.z -= nz * push;
                    self.npcs[j].foot.x += nx * push;
                    self.npcs[j].foot.z += nz * push;
                } else if dist <= 0.001 {
                    self.npcs[i].foot.x -= 1.0;
                    self.npcs[j].foot.x += 1.0;
                }
            }
        }

        // Re-clamp to bounds
        let pad = 0.5;
        for npc in &mut self.npcs {
            npc.foot.x = npc.foot.x.clamp(bounds.min.x + pad, bounds.max.x - pad);
            npc.foot.z = npc.foot.z.clamp(bounds.min.z + pad, bounds.max.z - pad);
        }

        // Wave management: check if all NPCs are dead
        let all_dead = self.npcs.iter().all(|n| !n.alive());
        let all_faded = self.npcs.iter().all(|n| !n.alive() && n.death_timer >= 0.9);

        if all_dead && !self.wave_pending {
            self.wave_pending = true;
            self.wave_delay = 2.0; // 2 second pause before next wave
        }

        if self.wave_pending {
            self.wave_delay -= dt;
            if self.wave_delay <= 0.0 && all_faded {
                self.wave_pending = false;
                self.spawn_wave(player_pos, bounds);
            }
        }
    }

    /// Register a shot with aim assist. Returns world-space impact point for VFX (closest ray hit).
    pub fn register_shot(&mut self, game: &GameState, weapon_idx: usize) -> Option<Vec3> {
        let eye = game.eye_pos();
        let dir = game.view_forward();
        if dir.length_squared() < 1e-8 {
            return None;
        }
        let dir_n = dir.normalize();

        // First pass: precise ray-AABB — damage every intersected NPC; splat at closest intersection.
        let mut hit = false;
        let mut best_ray_t = f32::INFINITY;
        let mut impact = None;
        let d = DAMAGE[weapon_idx.min(3)];
        for npc in &mut self.npcs {
            if !npc.alive() {
                continue;
            }
            if let Some(t) = ray_aabb(eye, dir_n, &npc.hit_aabb()) {
                if t > 0.02 && t < 120.0 {
                    npc.hp = (npc.hp - d).max(0.0);
                    npc.hit_flash = 1.0;
                    hit = true;
                    if t < best_ray_t {
                        best_ray_t = t;
                        impact = Some(eye + dir_n * t);
                    }
                }
            }
        }

        if hit {
            return impact;
        }

        // Aim assist fallback: very generous cone (45 degrees)
        let aim_cos = 0.70;
        let mut best_dist = f32::INFINITY;
        let mut best_idx: Option<usize> = None;

        for (i, npc) in self.npcs.iter().enumerate() {
            if !npc.alive() {
                continue;
            }
            let center = Vec3::new(
                npc.foot.x,
                npc.foot.y + npc.def.hitbox_height * npc.def.scale * 0.5,
                npc.foot.z,
            );
            let to_npc = center - eye;
            let dist = to_npc.length();
            if dist < 0.1 || dist > 120.0 {
                continue;
            }
            let to_npc_n = to_npc / dist;
            let dot = dir_n.dot(to_npc_n);
            if dot > aim_cos && dist < best_dist {
                best_dist = dist;
                best_idx = Some(i);
            }
        }

        if let Some(idx) = best_idx {
            let d = DAMAGE[weapon_idx.min(3)];
            self.npcs[idx].hp = (self.npcs[idx].hp - d).max(0.0);
            self.npcs[idx].hit_flash = 1.0;
            let p = ray_aabb(eye, dir_n, &self.npcs[idx].hit_aabb())
                .filter(|t| *t > 0.02 && *t < 120.0)
                .map(|t| eye + dir_n * t)
                .unwrap_or_else(|| {
                    let c = Vec3::new(
                        self.npcs[idx].foot.x,
                        self.npcs[idx].foot.y
                            + self.npcs[idx].def.hitbox_height * self.npcs[idx].def.scale * 0.45,
                        self.npcs[idx].foot.z,
                    );
                    let t = (c - eye).dot(dir_n).max(0.15);
                    eye + dir_n * t
                });
            return Some(p);
        }

        None
    }

    /// Count of living NPCs.
    pub fn alive_count(&self) -> usize {
        self.npcs.iter().filter(|n| n.alive()).count()
    }

    /// When not on the relay, NPCs in shoot stance apply light hitscan-style damage (server still owns HP when joined).
    pub fn offline_shoot_damage_per_tick(&self, dt: f32) -> i32 {
        const DPS_PER_SHOOTER: f32 = 14.0;
        let n = self
            .npcs
            .iter()
            .filter(|n| n.shooting_at_player())
            .count() as f32;
        if n <= 0.0 {
            return 0;
        }
        (DPS_PER_SHOOTER * dt * n).round().max(1.0) as i32
    }

    /// Wave display text for HUD.
    pub fn wave_text(&self) -> String {
        format!("WAVE {}", self.wave + 1)
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
