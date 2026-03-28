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
}

impl Loadout {
    pub fn new() -> Self {
        Self {
            clips: [12, 8, 32, 50],
            reserves: [100, 40, 200, 150],
            current: 0,
            muzzle_flash: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.muzzle_flash = (self.muzzle_flash - dt * 6.0).max(0.0);
    }

    pub fn try_fire(&mut self) -> bool {
        let i = self.current;
        if self.clips[i] > 0 {
            self.clips[i] -= 1;
            self.muzzle_flash = 1.0;
            return true;
        }
        false
    }

    pub fn reload(&mut self) {
        let i = self.current;
        let cap = WEAPONS[i].max_clip;
        let need = cap.saturating_sub(self.clips[i]);
        let take = need.min(self.reserves[i]);
        self.clips[i] += take;
        self.reserves[i] -= take;
    }

    pub fn cycle_next(&mut self) {
        self.current = (self.current + 1) % 4;
    }

    pub fn cycle_prev(&mut self) {
        self.current = (self.current + 3) % 4;
    }

    pub fn select(&mut self, idx: usize) {
        if idx < 4 {
            self.current = idx;
        }
    }

    pub fn handle_edges(&mut self, prev: bool, next: bool, pick: Option<u8>, reload: bool) {
        if reload {
            self.reload();
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
}
