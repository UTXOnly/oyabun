package relay

import (
	"encoding/json"
	"log"
	"net/http"
	"strconv"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/gorilla/websocket"
	"github.com/nbd-wtf/go-nostr"
)

var upgrader = websocket.Upgrader{
	ReadBufferSize:  4096,
	WriteBufferSize: 4096,
	CheckOrigin: func(r *http.Request) bool {
		return true
	},
}

type Hub struct {
	mu            sync.Mutex
	nonceByConnID map[string]string
	writeLocks    map[string]*sync.Mutex // one mutex per WebSocket until disconnect
	byConn        map[string]*session
	rooms         map[string]*Room
	gameLog       *EventLog
	relayPriv     string
	relayPub      string
}

func NewHub(gameLog *EventLog) *Hub {
	sk := nostr.GeneratePrivateKey()
	pk, err := nostr.GetPublicKey(sk)
	if err != nil || pk == "" {
		log.Printf("relay keygen failed, using fresh key")
		sk = nostr.GeneratePrivateKey()
		pk, _ = nostr.GetPublicKey(sk)
	}
	h := &Hub{
		nonceByConnID: make(map[string]string),
		writeLocks:    make(map[string]*sync.Mutex),
		byConn:        make(map[string]*session),
		rooms:         make(map[string]*Room),
		gameLog:       gameLog,
		relayPriv:     sk,
		relayPub:      pk,
	}
	h.rooms["default"] = NewRoom("default", h)
	return h
}

// RelayPublicKeyHex is the pubkey that signs KindJoinAck / KindGameSnap / KindGameNotice.
func (h *Hub) RelayPublicKeyHex() string {
	return h.relayPub
}

func (h *Hub) LogGameplay(m map[string]any) {
	if h == nil || h.gameLog == nil {
		return
	}
	h.gameLog.Append(m)
}

func (h *Hub) writeOK(connID string, c *websocket.Conn, eventID string, accepted bool, message string) {
	h.mu.Lock()
	mu := h.writeLocks[connID]
	h.mu.Unlock()
	if mu == nil {
		return
	}
	mu.Lock()
	defer mu.Unlock()
	_ = c.WriteJSON([]any{"OK", eventID, accepted, message})
}

func (h *Hub) HandleWS(w http.ResponseWriter, r *http.Request) {
	c, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("upgrade: %v", err)
		return
	}
	connID := uuid.NewString()
	challenge := uuid.NewString()
	wl := &sync.Mutex{}
	h.mu.Lock()
	h.nonceByConnID[connID] = challenge
	h.writeLocks[connID] = wl
	h.mu.Unlock()
	wl.Lock()
	_ = c.WriteJSON([]any{"AUTH", challenge})
	wl.Unlock()
	go h.readLoop(c, connID)
}

func (h *Hub) readLoop(c *websocket.Conn, connID string) {
	defer func() {
		_ = c.Close()
		var pub string
		var ent int
		var roomID string
		h.mu.Lock()
		delete(h.nonceByConnID, connID)
		delete(h.writeLocks, connID)
		if s, ok := h.byConn[connID]; ok {
			if s.room != nil {
				roomID = s.room.id
				s.room.RemovePlayer(s.entityID)
			}
			pub = s.pubkey
			ent = s.entityID
			delete(h.byConn, connID)
		}
		h.mu.Unlock()
		if pub != "" {
			h.LogGameplay(map[string]any{
				"type":      "leave",
				"conn_id":   connID,
				"pubkey":    pub,
				"entity_id": ent,
				"room_id":   roomID,
			})
		}
	}()
	for {
		_, data, err := c.ReadMessage()
		if err != nil {
			return
		}
		var wire []json.RawMessage
		if err := json.Unmarshal(data, &wire); err != nil || len(wire) < 2 {
			continue
		}
		var typ string
		if err := json.Unmarshal(wire[0], &typ); err != nil {
			continue
		}
		if typ != "EVENT" {
			continue
		}
		var ev nostr.Event
		if err := json.Unmarshal(wire[1], &ev); err != nil {
			continue
		}
		h.routeEvent(c, connID, &ev)
	}
}

func (h *Hub) routeEvent(c *websocket.Conn, connID string, ev *nostr.Event) {
	okSig, err := ev.CheckSignature()
	if err != nil || !okSig {
		h.writeOK(connID, c, ev.ID, false, "invalid: bad signature")
		return
	}
	switch ev.Kind {
	case KindRelayAuth:
		h.handleAuthEvent(c, connID, ev)
	case KindRoomJoin:
		h.handleJoinEvent(c, connID, ev)
	case KindClientInput:
		h.handleInputEvent(connID, ev)
	default:
		h.writeOK(connID, c, ev.ID, false, "restricted: unknown kind")
	}
}

func (h *Hub) handleAuthEvent(c *websocket.Conn, connID string, ev *nostr.Event) {
	ct := ev.Tags.GetFirst([]string{"challenge", ""})
	if ct == nil || ct.Value() == "" {
		h.writeOK(connID, c, ev.ID, false, "invalid: missing_challenge")
		return
	}
	challenge := ct.Value()
	h.mu.Lock()
	expect, ok := h.nonceByConnID[connID]
	h.mu.Unlock()
	if !ok || expect != challenge {
		h.writeOK(connID, c, ev.ID, false, "invalid: bad_challenge")
		return
	}
	exp := time.Now().Add(24 * time.Hour).UnixMilli()
	h.mu.Lock()
	wl := h.writeLocks[connID]
	s := &session{
		pubkey:  ev.PubKey,
		conn:    c,
		connID:  connID,
		expires: exp,
		writeMu: wl,
	}
	h.byConn[connID] = s
	delete(h.nonceByConnID, connID)
	h.mu.Unlock()
	h.writeOK(connID, c, ev.ID, true, "")
	h.LogGameplay(map[string]any{"type": "session_auth", "conn_id": connID, "pubkey": ev.PubKey})
}

func (h *Hub) handleJoinEvent(c *websocket.Conn, connID string, ev *nostr.Event) {
	h.mu.Lock()
	s, ok := h.byConn[connID]
	if !ok || time.Now().UnixMilli() > s.expires {
		h.mu.Unlock()
		h.writeOK(connID, c, ev.ID, false, "invalid: not_authenticated")
		return
	}
	if s.pubkey != ev.PubKey {
		h.mu.Unlock()
		h.writeOK(connID, c, ev.ID, false, "invalid: pubkey_mismatch")
		return
	}
	roomID := "default"
	if rt := ev.Tags.GetFirst([]string{"room", ""}); rt != nil && rt.Value() != "" {
		roomID = rt.Value()
	}
	room, ok := h.rooms[roomID]
	if !ok {
		h.mu.Unlock()
		h.writeOK(connID, c, ev.ID, false, "invalid: no_room")
		return
	}
	if s.room != nil {
		h.mu.Unlock()
		h.writeOK(connID, c, ev.ID, false, "invalid: already_joined")
		return
	}
	s.room = room
	id := room.AddPlayer(s)
	s.entityID = id
	pubkey := s.pubkey
	h.mu.Unlock()

	h.LogGameplay(map[string]any{
		"type":      "join",
		"room_id":   roomID,
		"entity_id": id,
		"pubkey":    pubkey,
	})
	h.writeOK(connID, c, ev.ID, true, "")

	ack, _ := json.Marshal(map[string]any{
		"room_id":          roomID,
		"your_entity_id":   id,
		"tick_rate":        20,
		"relay_pubkey_hex": h.relayPub,
	})
	_ = h.sendRelayEvent(s, KindJoinAck, roomID, string(ack))
}

func (h *Hub) handleInputEvent(connID string, ev *nostr.Event) {
	st := ev.Tags.GetFirst([]string{"seq", ""})
	if st == nil || st.Value() == "" {
		return
	}
	seq, err := strconv.ParseUint(st.Value(), 10, 64)
	if err != nil {
		return
	}
	var payload InputPayload
	if err := json.Unmarshal([]byte(ev.Content), &payload); err != nil {
		return
	}
	h.mu.Lock()
	s, ok := h.byConn[connID]
	if !ok {
		h.mu.Unlock()
		return
	}
	if s.room == nil || s.pubkey != ev.PubKey {
		cid := s.connID
		conn := s.conn
		h.mu.Unlock()
		h.writeOK(cid, conn, ev.ID, false, "invalid: no_session")
		return
	}
	if seq <= s.lastSeq {
		h.mu.Unlock()
		return
	}
	s.lastSeq = seq
	room := s.room
	id := s.entityID
	cid := s.connID
	conn := s.conn
	h.mu.Unlock()
	room.ApplyInput(id, payload, seq)
	h.writeOK(cid, conn, ev.ID, true, "")
}

func (h *Hub) sendRelayEvent(s *session, kind int, roomID string, content string) error {
	if s == nil || s.writeMu == nil {
		return nil
	}
	e := nostr.Event{
		PubKey:    h.relayPub,
		CreatedAt: nostr.Timestamp(time.Now().Unix()),
		Kind:      kind,
		Tags:      nostr.Tags{{"room", roomID}},
		Content:   content,
	}
	if err := e.Sign(h.relayPriv); err != nil {
		return err
	}
	s.writeMu.Lock()
	defer s.writeMu.Unlock()
	return s.conn.WriteJSON([]any{"EVENT", "oyabaun", e})
}

func (h *Hub) BroadcastRelayEvent(room *Room, kind int, content string) {
	e := nostr.Event{
		PubKey:    h.relayPub,
		CreatedAt: nostr.Timestamp(time.Now().Unix()),
		Kind:      kind,
		Tags:      nostr.Tags{{"room", room.id}},
		Content:   content,
	}
	if err := e.Sign(h.relayPriv); err != nil {
		return
	}
	data, err := json.Marshal([]any{"EVENT", "oyabaun", e})
	if err != nil {
		return
	}
	for _, s := range h.snapshotRoomSessions(room) {
		if s.writeMu == nil {
			continue
		}
		s.writeMu.Lock()
		_ = s.conn.WriteMessage(websocket.TextMessage, data)
		s.writeMu.Unlock()
	}
}

func (h *Hub) snapshotRoomSessions(room *Room) []*session {
	h.mu.Lock()
	defer h.mu.Unlock()
	out := make([]*session, 0, len(h.byConn))
	for _, s := range h.byConn {
		if s.room == room {
			out = append(out, s)
		}
	}
	return out
}
