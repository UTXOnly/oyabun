pub struct WeaponDef {
    pub name: &'static str,
    pub max_clip: u32,
}

pub const WEAPONS: [WeaponDef; 4] = [
    WeaponDef {
        name: "M9",
        max_clip: 12,
    },
    WeaponDef {
        name: "500",
        max_clip: 8,
    },
    WeaponDef {
        name: "SCAR",
        max_clip: 32,
    },
    WeaponDef {
        name: "LITE",
        max_clip: 50,
    },
];

pub struct Loadout {
    clips: [u32; 4],
    reserves: [u32; 4],
    current: usize,
    pub muzzle_flash: f32,
    /// 0.0 = idle, 1.0 = just fired, decays over time (recoil kick)
    pub recoil: f32,
    /// Reload animation progress: 0.0 = not reloading, 0.0→1.0 = lowering,
    /// 1.0→2.0 = raising back. When >= 2.0, reload completes.
    pub reload_anim: f32,
    reload_pending: bool,
}

impl Loadout {
    pub fn new() -> Self {
        Self {
            clips: [12, 8, 32, 50],
            reserves: [100, 40, 200, 150],
            current: 0,
            muzzle_flash: 0.0,
            recoil: 0.0,
            reload_anim: 0.0,
            reload_pending: false,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.muzzle_flash = (self.muzzle_flash - dt * 2.6).max(0.0);
        self.recoil = (self.recoil - dt * 4.2).max(0.0);

        if self.reload_pending || self.is_reloading() {
            self.reload_anim += dt * 2.5; // ~0.8s full cycle
            if self.reload_anim >= 1.0 && self.reload_pending {
                // At the midpoint, actually reload the ammo
                self.do_reload();
                self.reload_pending = false;
            }
            if self.reload_anim >= 2.0 {
                self.reload_anim = 0.0;
            }
        }
    }

    pub fn try_fire(&mut self) -> bool {
        if self.is_reloading() {
            return false; // can't fire while reloading
        }
        let i = self.current;
        if self.clips[i] > 0 {
            self.clips[i] -= 1;
            self.muzzle_flash = 1.0;
            self.recoil = 1.0;
            return true;
        }
        // Auto-reload when clip is empty
        if self.reserves[i] > 0 {
            self.start_reload();
        }
        false
    }

    pub fn start_reload(&mut self) {
        if self.is_reloading() {
            return; // already reloading
        }
        let i = self.current;
        let cap = WEAPONS[i].max_clip;
        if self.clips[i] >= cap || self.reserves[i] == 0 {
            return; // full or no reserves
        }
        self.reload_pending = true;
        self.reload_anim = 0.001; // start the animation
    }

    fn do_reload(&mut self) {
        let i = self.current;
        let cap = WEAPONS[i].max_clip;
        let need = cap.saturating_sub(self.clips[i]);
        let take = need.min(self.reserves[i]);
        self.clips[i] += take;
        self.reserves[i] -= take;
    }

    pub fn cycle_next(&mut self) {
        if self.is_reloading() { return; }
        self.current = (self.current + 1) % 4;
    }

    pub fn cycle_prev(&mut self) {
        if self.is_reloading() { return; }
        self.current = (self.current + 3) % 4;
    }

    pub fn select(&mut self, idx: usize) {
        if idx < 4 && !self.is_reloading() {
            self.current = idx;
        }
    }

    pub fn handle_edges(&mut self, prev: bool, next: bool, pick: Option<u8>, reload: bool) {
        if reload {
            self.start_reload();
        }
        if prev {
            self.cycle_prev();
        }
        if next {
            self.cycle_next();
        }
        if let Some(i) = pick {
            self.select(i as usize);
        }
    }

    pub fn current_idx(&self) -> usize {
        self.current
    }

    pub fn clip_for(&self, i: usize) -> u32 {
        self.clips.get(i).copied().unwrap_or(0)
    }

    pub fn reserve_for(&self, i: usize) -> u32 {
        self.reserves.get(i).copied().unwrap_or(0)
    }

    pub fn is_reloading(&self) -> bool {
        self.reload_anim > 0.0
    }
}
