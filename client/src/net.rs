use serde_json::{json, Value};

use crate::game::GameState;
use crate::input::InputState;

const ROOM: &str = "default";
const INPUT_MS: f64 = 45.0;

const KIND_JOIN_ACK: u64 = 24552;
const KIND_GAME_SNAP: u64 = 20421;
const KIND_GAME_NOTICE: u64 = 24553;

#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct SnapPlayer {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub health: i32,
    pub score: i32,
}

pub struct NetController {
    out: std::collections::VecDeque<String>,
    pub entity_id: Option<u32>,
    pub joined: bool,
    pub players: Vec<SnapPlayer>,
    pub self_health: i32,
    pub self_score: i32,
    pub toast: String,
    pub status: String,
    pub last_input_ms: f64,
    seq: u64,
}

impl NetController {
    pub fn new() -> Self {
        Self {
            out: std::collections::VecDeque::new(),
            entity_id: None,
            joined: false,
            players: Vec::new(),
            self_health: 100,
            self_score: 0,
            toast: String::new(),
            status: String::from("connecting…"),
            last_input_ms: 0.0,
            seq: 0,
        }
    }

    pub fn take_outbound(&mut self) -> Option<String> {
        self.out.pop_front()
    }

    pub fn ingest(&mut self, text: &str) {
        let v: Value = match serde_json::from_str(text) {
            Ok(x) => x,
            Err(_) => return,
        };
        if let Value::Array(arr) = &v {
            if arr
                .first()
                .and_then(|x| x.as_str())
                .is_some_and(|s| s == "EVENT")
            {
                if let Some(ev) = arr.get(2) {
                    self.ingest_relay_event(ev);
                }
                return;
            }
            if arr
                .first()
                .and_then(|x| x.as_str())
                .is_some_and(|s| s == "OK")
            {
                let ok = arr.get(2).and_then(|x| x.as_bool()).unwrap_or(false);
                if !ok {
                    let reason = arr
                        .get(3)
                        .and_then(|x| x.as_str())
                        .unwrap_or("rejected");
                    self.status = format!("relay: {}", reason);
                }
                return;
            }
        }
    }

    fn ingest_relay_event(&mut self, ev: &Value) {
        let kind = ev.get("kind").and_then(|x| x.as_u64()).unwrap_or(0);
        let content = ev.get("content").and_then(|x| x.as_str()).unwrap_or("{}");
        match kind {
            KIND_JOIN_ACK => {
                if let Ok(c) = serde_json::from_str::<Value>(content) {
                    if let Some(id) = c.get("your_entity_id").and_then(|x| x.as_u64()) {
                        self.entity_id = Some(id as u32);
                    }
                    self.joined = true;
                    self.status = String::from("in match — WSAD / mouse / click shoot");
                }
            }
            KIND_GAME_SNAP => {
                if let Ok(c) = serde_json::from_str::<Value>(content) {
                    self.apply_snap_value(&c);
                }
            }
            KIND_GAME_NOTICE => {
                if let Ok(c) = serde_json::from_str::<Value>(content) {
                    if c.get("name").and_then(|x| x.as_str()) == Some("kill") {
                        let k = c.get("killer_id").and_then(|x| x.as_u64()).unwrap_or(0);
                        let vic = c.get("victim_id").and_then(|x| x.as_u64()).unwrap_or(0);
                        self.toast = format!("kill · {} → {}", k, vic);
                    }
                }
            }
            _ => {}
        }
    }

    fn apply_snap_value(&mut self, v: &Value) {
        let Some(arr) = v.get("players").and_then(|p| p.as_array()) else {
            return;
        };
        let you = v.get("you_id").and_then(|x| x.as_u64()).map(|x| x as u32);
        self.players.clear();
        for p in arr {
            let id = p.get("id").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let x = p.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            let y = p.get("y").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            let z = p.get("z").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            let yaw = p.get("yaw").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            let health = p.get("health").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
            let score = p.get("score").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
            if Some(id) == you {
                self.self_health = health;
                self.self_score = score;
            }
            self.players.push(SnapPlayer {
                id,
                x,
                y,
                z,
                yaw,
                health,
                score,
            });
        }
    }

    pub fn pump_input(
        &mut self,
        time_ms: f64,
        game: &GameState,
        input: &InputState,
        shoot: bool,
    ) {
        if !self.joined {
            return;
        }
        if time_ms - self.last_input_ms < INPUT_MS {
            return;
        }
        self.last_input_ms = time_ms;
        self.seq += 1;
        let mut f = 0i8;
        if input.forward {
            f += 1;
        }
        if input.back {
            f -= 1;
        }
        let mut st = 0i8;
        if input.left {
            st -= 1;
        }
        if input.right {
            st += 1;
        }
        let created_at = (time_ms / 1000.0).floor() as i64;
        let inner = json!({
            "f": f,
            "s": st,
            "y": game.yaw as f64,
            "p": game.pitch as f64,
            "sh": shoot,
            "j": false,
        });
        let draft = json!({
            "unsigned_nostr_event": {
                "kind": 20420,
                "created_at": created_at,
                "tags": [["room", ROOM], ["seq", self.seq.to_string()]],
                "content": inner.to_string(),
            }
        });
        self.out.push_back(draft.to_string());
    }

    pub fn target_xz_for_self(&self) -> Option<(f32, f32)> {
        let eid = self.entity_id?;
        for p in &self.players {
            if p.id == eid {
                return Some((p.x, p.z));
            }
        }
        None
    }
}
