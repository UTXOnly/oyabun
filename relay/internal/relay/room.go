package relay

import (
	"encoding/json"
	"math"
	"sync"
	"time"
)

const tickRate = 20

// stateLogEvery: append room_state to OYABAUN_GAME_LOG this often (ticks). 20 @ 20Hz ≈ 1/s.
const stateLogEvery uint64 = 20

// InputPayload is JSON in kind 20420 event content (compact keys for size).
type InputPayload struct {
	Forward int8    `json:"f"`
	Strafe  int8    `json:"s"`
	Yaw     float64 `json:"y"`
	Pitch   float64 `json:"p"`
	Shoot   bool    `json:"sh"`
	Jump    bool    `json:"j"`
}

type Player struct {
	ID        int
	Pubkey    string
	X, Y, Z   float64
	Yaw       float64
	Health    int
	Score     int
	DeadUntil time.Time
	LastShot  time.Time
}

type Room struct {
	mu       sync.Mutex
	id       string
	hub      *Hub
	nextID   int
	players  map[int]*Player
	stopTick chan struct{}
}

// Match exported Tokyo alley (glTF ~ z −32…+32; main façades near z ≈ −26).
var spawnXZ = [][2]float64{
	{0, -20}, {0, -14}, {2.5, -22}, {-2.5, -22}, {4, -18}, {-4, -18}, {0, -26}, {3, -16},
}

func NewRoom(id string, hub *Hub) *Room {
	r := &Room{
		id:       id,
		hub:      hub,
		players:  make(map[int]*Player),
		stopTick: make(chan struct{}),
	}
	go r.tickLoop()
	return r
}

func (r *Room) AddPlayer(s *session) int {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.nextID++
	id := r.nextID
	sp := spawnXZ[(id-1)%len(spawnXZ)]
	p := &Player{
		ID:     id,
		Pubkey: s.pubkey,
		X:      sp[0],
		Y:      1.0, // glTF alley feet ~ +1 Y; client still snaps to floor under XZ for draw
		Z:      sp[1],
		Yaw:    0,
		Health: 100,
		Score:  0,
	}
	r.players[id] = p
	return id
}

func (r *Room) RemovePlayer(id int) {
	r.mu.Lock()
	defer r.mu.Unlock()
	delete(r.players, id)
}

func (r *Room) respawn(p *Player) {
	sp := spawnXZ[(p.ID-1)%len(spawnXZ)]
	p.X, p.Y, p.Z = sp[0], 1.0, sp[1]
	p.Health = 100
	p.Yaw = 0
}

func (r *Room) ApplyInput(entityID int, msg InputPayload, seq uint64) {
	r.mu.Lock()
	p, ok := r.players[entityID]
	if !ok {
		r.mu.Unlock()
		return
	}
	if p.Health <= 0 {
		if time.Now().After(p.DeadUntil) {
			r.respawn(p)
			r.hub.LogGameplay(map[string]any{
				"type":      "respawn",
				"room_id":   r.id,
				"entity_id": p.ID,
				"pubkey":    p.Pubkey,
			})
		} else {
			r.mu.Unlock()
			return
		}
	}

	speed := 6.0 / float64(tickRate)
	f := float64(msg.Forward)
	st := float64(msg.Strafe)
	if f != 0 {
		p.X += speed * f * math.Sin(p.Yaw)
		p.Z += speed * f * (-math.Cos(p.Yaw))
	}
	if st != 0 {
		p.X += speed * st * math.Sin(p.Yaw+1.5707963267948966)
		p.Z += speed * st * (-math.Cos(p.Yaw+1.5707963267948966))
	}
	p.Yaw = msg.Yaw
	if msg.Shoot && time.Since(p.LastShot) >= 220*time.Millisecond {
		p.LastShot = time.Now()
		r.hub.LogGameplay(map[string]any{
			"type":      "shoot",
			"room_id":   r.id,
			"entity_id": p.ID,
			"pubkey":    p.Pubkey,
			"seq":       seq,
		})
		r.hitscanLocked(p)
	}
	r.mu.Unlock()
}

func (r *Room) hitscanLocked(shooter *Player) {
	if shooter.Health <= 0 {
		return
	}
	for _, t := range r.players {
		if t.ID == shooter.ID || t.Health <= 0 {
			continue
		}
		dx := t.X - shooter.X
		dz := t.Z - shooter.Z
		if dx*dx+dz*dz < 2.25 {
			t.Health -= 34
			if t.Health <= 0 {
				t.Health = 0
				t.DeadUntil = time.Now().Add(3 * time.Second)
				shooter.Score++
				r.hub.LogGameplay(map[string]any{
					"type":          "kill",
					"room_id":       r.id,
					"killer_id":     shooter.ID,
					"killer_pubkey": shooter.Pubkey,
					"victim_id":     t.ID,
					"victim_pubkey": t.Pubkey,
					"weapon":        "smg",
				})
				note, _ := json.Marshal(map[string]any{
					"name":      "kill",
					"killer_id": shooter.ID,
					"victim_id": t.ID,
					"weapon":    "smg",
				})
				r.hub.BroadcastRelayEvent(r, KindGameNotice, string(note))
			}
			return
		}
	}
}

func (r *Room) tickLoop() {
	t := time.NewTicker(time.Second / tickRate)
	defer t.Stop()
	tick := uint64(0)
	for {
		select {
		case <-r.stopTick:
			return
		case <-t.C:
			tick++
			r.broadcastSnap(tick)
		}
	}
}

func (r *Room) broadcastSnap(tick uint64) {
	r.mu.Lock()
	list := make([]map[string]any, 0, len(r.players))
	for _, p := range r.players {
		list = append(list, map[string]any{
			"id": p.ID, "pubkey": p.Pubkey, "x": p.X, "y": p.Y, "z": p.Z,
			"yaw": p.Yaw, "health": p.Health, "score": p.Score,
		})
	}
	r.mu.Unlock()

	if tick%stateLogEvery == 0 {
		r.hub.LogGameplay(map[string]any{
			"type":         "room_state",
			"room_id":      r.id,
			"tick":         tick,
			"tick_rate_hz": tickRate,
			"player_count": len(list),
			"players":      list,
		})
	}

	for _, s := range r.hub.snapshotRoomSessions(r) {
		msg, err := json.Marshal(map[string]any{
			"tick":    tick,
			"players": list,
			"you_id":  s.entityID,
		})
		if err != nil {
			continue
		}
		_ = r.hub.sendRelayEvent(s, KindGameSnap, r.id, string(msg))
	}
}
