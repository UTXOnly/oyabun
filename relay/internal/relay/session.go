package relay

import (
	"sync"

	"github.com/gorilla/websocket"
)

type session struct {
	pubkey   string
	conn     *websocket.Conn
	connID   string
	expires  int64
	room     *Room
	entityID int
	lastSeq  uint64
	// writeMu serializes all WebSocket writes for this connection (gorilla/websocket is not concurrent-writer safe).
	writeMu *sync.Mutex
}
