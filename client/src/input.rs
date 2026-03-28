#[derive(Default)]
pub struct InputState {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub mouse_dx: f32,
    pub mouse_dy: f32,
    shoot_edge: bool,
    weapon_prev_edge: bool,
    weapon_next_edge: bool,
    reload_edge: bool,
    weapon_pick: Option<u8>,
}

impl InputState {
    pub fn key_set(&mut self, code: &str, down: bool) {
        if down {
            match code {
                "KeyQ" | "BracketLeft" => self.weapon_prev_edge = true,
                "KeyE" | "BracketRight" => self.weapon_next_edge = true,
                "KeyR" => self.reload_edge = true,
                "Digit1" => self.weapon_pick = Some(0),
                "Digit2" => self.weapon_pick = Some(1),
                "Digit3" => self.weapon_pick = Some(2),
                "Digit4" => self.weapon_pick = Some(3),
                _ => {}
            }
        }
        match code {
            "KeyW" | "ArrowUp" => self.forward = down,
            "KeyS" | "ArrowDown" => self.back = down,
            "KeyA" | "ArrowLeft" => self.left = down,
            "KeyD" | "ArrowRight" => self.right = down,
            "Space" => self.jump = down,
            _ => {}
        }
    }

    pub fn take_weapon_edges(&mut self) -> (bool, bool, Option<u8>, bool) {
        let p = self.weapon_prev_edge;
        let n = self.weapon_next_edge;
        let r = self.reload_edge;
        let pick = self.weapon_pick.take();
        self.weapon_prev_edge = false;
        self.weapon_next_edge = false;
        self.reload_edge = false;
        (p, n, pick, r)
    }

    pub fn mouse_accum(&mut self, dx: f32, dy: f32) {
        self.mouse_dx += dx;
        self.mouse_dy += dy;
    }

    pub fn take_mouse(&mut self) -> (f32, f32) {
        let d = (self.mouse_dx, self.mouse_dy);
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        d
    }

    pub fn shoot_press(&mut self) {
        self.shoot_edge = true;
    }

    pub fn take_shoot_edge(&mut self) -> bool {
        let s = self.shoot_edge;
        self.shoot_edge = false;
        s
    }
}
