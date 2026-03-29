use glam::{Mat4, Vec3};

use crate::mesh::Aabb;

const EYE: f32 = 1.65;
const RADIUS: f32 = 0.32;
const MOVE_SPEED: f32 = 5.2;
const GRAVITY: f32 = 22.0;
const JUMP_V: f32 = 8.5;
const PITCH_MAX: f32 = 1.53;

pub struct GameState {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub vel_y: f32,
    pub online: bool,
    net_tx: f32,
    net_tz: f32,
    solids: Vec<Aabb>,
    /// Lowest valid feet height (from level bounds); replaces assuming world floor at y=0.
    y_min: f32,
    /// When colliders are only tall chunks, `feet_y_on_solids` finds nothing; use spawn height for NPCs / snaps.
    pub walk_surface_y: f32,
}

impl GameState {
    pub fn new(spawn: Vec3, solids: Vec<Aabb>, spawn_yaw: f32, y_min: f32) -> Self {
        Self {
            pos: spawn,
            yaw: spawn_yaw,
            pitch: 0.0,
            vel_y: 0.0,
            online: false,
            net_tx: spawn.x,
            net_tz: spawn.z,
            solids,
            y_min,
            walk_surface_y: spawn.y,
        }
    }

    fn feet_y_on_solids(&self, x: f32, z: f32) -> f32 {
        const MAX_FLOOR_LIKE_H: f32 = 3.2;
        let mut top = self.y_min;
        for s in &self.solids {
            if x + RADIUS <= s.min.x || x - RADIUS >= s.max.x {
                continue;
            }
            if z + RADIUS <= s.min.z || z - RADIUS >= s.max.z {
                continue;
            }
            let h = s.max.y - s.min.y;
            if h > MAX_FLOOR_LIKE_H {
                continue;
            }
            top = top.max(s.max.y);
        }
        top + 0.05
    }

    /// Walkable height under `(x, z)` for drawing NPCs / remotes (not only thin floor colliders).
    pub fn ground_y_at(&self, x: f32, z: f32) -> f32 {
        let thin = self.feet_y_on_solids(x, z);
        if thin > self.y_min + 0.08 {
            thin
        } else {
            self.walk_surface_y
        }
    }

    pub fn feet_draw_y(&self, x: f32, z: f32) -> f32 {
        let g = self.ground_y_at(x, z);
        let lo = (self.walk_surface_y - 0.9).max(self.y_min + 0.02);
        let hi = self.walk_surface_y + 2.5;
        g.clamp(lo, hi)
    }

    pub fn set_online(&mut self, v: bool) {
        self.online = v;
    }

    pub fn set_net_target_xz(&mut self, x: f32, z: f32) {
        self.net_tx = x;
        self.net_tz = z;
    }

    pub fn apply_look(&mut self, input: &mut crate::input::InputState) {
        let (mdx, mdy) = input.take_mouse();
        let sens = 0.0022_f32;
        self.yaw += mdx * sens;
        self.pitch -= mdy * sens;
        self.pitch = self.pitch.clamp(-PITCH_MAX, PITCH_MAX);
    }

    pub fn tick(&mut self, dt: f32, input: &mut crate::input::InputState) {
        if self.online {
            let k = (dt * 12.0).min(0.92);
            self.pos.x += (self.net_tx - self.pos.x) * k;
            self.pos.z += (self.net_tz - self.pos.z) * k;
            self.vel_y = 0.0;
            self.resolve_xz(true, false);
            self.resolve_xz(false, true);
            self.pos.y = self.ground_y_at(self.pos.x, self.pos.z);
            self.resolve_y();
            if self.pos.y < self.y_min {
                self.pos.y = self.y_min;
            }
            return;
        }

        let sy = self.yaw.sin();
        let cy = self.yaw.cos();
        let mut wish = Vec3::ZERO;
        if input.forward {
            wish += Vec3::new(sy, 0.0, -cy);
        }
        if input.back {
            wish -= Vec3::new(sy, 0.0, -cy);
        }
        if input.left {
            wish += Vec3::new(-cy, 0.0, -sy);
        }
        if input.right {
            wish -= Vec3::new(-cy, 0.0, -sy);
        }
        if wish.length_squared() > 0.0001 {
            wish = wish.normalize() * MOVE_SPEED;
        }

        self.pos.x += wish.x * dt;
        self.resolve_xz(true, false);
        self.pos.z += wish.z * dt;
        self.resolve_xz(false, true);

        if self.grounded() {
            self.vel_y = 0.0;
            if input.jump {
                self.vel_y = JUMP_V;
            }
        } else {
            self.vel_y -= GRAVITY * dt;
        }
        self.pos.y += self.vel_y * dt;
        self.resolve_y();
        if self.pos.y < self.y_min {
            self.pos.y = self.y_min;
            self.vel_y = 0.0;
        }
    }

    fn grounded(&self) -> bool {
        const EPS: f32 = 0.07;
        if self.pos.y <= self.y_min + EPS {
            return true;
        }
        let px = self.pos.x;
        let pz = self.pos.z;
        for s in &self.solids {
            let top = s.max.y;
            if self.pos.y > top + EPS || self.pos.y < top - 0.22 {
                continue;
            }
            if px + RADIUS > s.min.x
                && px - RADIUS < s.max.x
                && pz + RADIUS > s.min.z
                && pz - RADIUS < s.max.z
            {
                return true;
            }
        }
        false
    }

    fn body_aabb(&self) -> Aabb {
        let p = self.pos;
        Aabb {
            min: Vec3::new(p.x - RADIUS, p.y, p.z - RADIUS),
            max: Vec3::new(p.x + RADIUS, p.y + EYE, p.z + RADIUS),
        }
    }

    fn resolve_xz(&mut self, axis_x: bool, axis_z: bool) {
        for s in &self.solids {
            let b = self.body_aabb();
            if !aabb_intersects(&b, s) {
                continue;
            }
            let dx_min = b.max.x - s.min.x;
            let dx_max = s.max.x - b.min.x;
            let dz_min = b.max.z - s.min.z;
            let dz_max = s.max.z - b.min.z;
            if axis_x {
                if dx_min < dx_max {
                    self.pos.x -= dx_min + 0.001;
                } else {
                    self.pos.x += dx_max + 0.001;
                }
            }
            if axis_z {
                if dz_min < dz_max {
                    self.pos.z -= dz_min + 0.001;
                } else {
                    self.pos.z += dz_max + 0.001;
                }
            }
        }
    }

    fn resolve_y(&mut self) {
        for s in &self.solids {
            let b = self.body_aabb();
            if !aabb_intersects(&b, s) {
                continue;
            }
            let up_pen = s.max.y - b.min.y;
            let down_pen = b.max.y - s.min.y;
            if up_pen < down_pen {
                self.pos.y += up_pen + 0.001;
                self.vel_y = 0.0;
            } else {
                self.pos.y -= down_pen + 0.001;
                self.vel_y = 0.0;
            }
        }
    }

    pub fn eye_pos(&self) -> Vec3 {
        self.pos + Vec3::new(0.0, EYE, 0.0)
    }

    pub fn view_forward(&self) -> Vec3 {
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let sy = self.yaw.sin();
        let cy = self.yaw.cos();
        Vec3::new(sy * cp, sp, -cy * cp).normalize()
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let forward = self.view_forward();
        let eye = self.eye_pos();
        let view = Mat4::look_at_rh(eye, eye + forward, Vec3::Y);
        let far = 220.0_f32;
        let proj = Mat4::perspective_rh(70_f32.to_radians(), aspect, 0.08, far);
        proj * view
    }
}

fn aabb_intersects(a: &Aabb, b: &Aabb) -> bool {
    a.min.x < b.max.x
        && a.max.x > b.min.x
        && a.min.y < b.max.y
        && a.max.y > b.min.y
        && a.min.z < b.max.z
        && a.max.z > b.min.z
}
