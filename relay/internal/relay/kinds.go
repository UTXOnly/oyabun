package relay

// Nostr event kinds for Oyabaun (ephemeral band 20000–29999 and adjacent custom).
// See docs/PROTOCOL.md and .cursor/skills/nostr-protocol.md.
const (
	KindRelayAuth   = 24550 // client → relay: prove pubkey + challenge
	KindRoomJoin    = 24551 // client → relay: join a room
	KindClientInput = 20420 // client → relay: input frame
	KindJoinAck     = 24552 // relay → client: join accepted (relay-signed)
	KindGameSnap    = 20421 // relay → client: world snapshot (relay-signed)
	KindGameNotice  = 24553 // relay → client: HUD event e.g. kill (relay-signed)
)
