# Oyabaun relay protocol

The relay speaks **NIP-01 WebSocket message types** (`AUTH`, `EVENT`, `OK`) plus relay-signed `EVENT` pushes. Every **client-originated** gameplay message is a **signed Nostr event** (NIP-01 event object: `id`, `pubkey`, `created_at`, `kind`, `tags`, `content`, `sig`).

Transport: **WebSocket** (`/ws`).

## Wire format (JSON arrays)

Messages are JSON arrays, same shape as mainstream Nostr relays:

| First element | Meaning |
|---------------|---------|
| `AUTH` | Relay → client: authentication challenge (aligned with [NIP-42](https://github.com/nostr-protocol/nips/blob/master/42.md) style). |
| `EVENT` | Client → relay: `["EVENT", <signed_event_object>]` **or** relay → client: `["EVENT", "<sub_id>", <signed_event_object>]`. |
| `OK` | Relay → client: `["OK", "<event_id>", <accepted:bool>, "<message>"]` per NIP-01. |

There is **no** separate `{ "type": "auth", ... }` envelope for gameplay; the client publishes **EVENT** only.

## Event kinds (custom, ephemeral band)

| Kind | Direction | Purpose |
|-----:|-------------|---------|
| **24550** | client → relay | Auth: prove pubkey; tag `["challenge","<nonce>"]` must match the relay’s last `AUTH` challenge for this connection. |
| **24551** | client → relay | Join room: tag `["room","<room_id>"]` (e.g. `default`). |
| **20420** | client → relay | Input frame: tags `["room","<id>"]`, `["seq","<u64>"]`; `content` is compact JSON (see below). |
| **24552** | relay → client | Join accepted: `content` JSON includes `your_entity_id`, `room_id`, `tick_rate`, `relay_pubkey_hex`. |
| **20421** | relay → client | Snapshot: `content` JSON `{ "tick", "players", "you_id" }` (same fields as before). |
| **24553** | relay → client | Game notice: `content` JSON e.g. `{ "name":"kill", ... }`. |

Kinds **24550–24553** and **20420–20421** are in or adjacent to the **ephemeral** / experimental bands per [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md); this relay does not persist them to disk (only optional JSONL gameplay logging for ops).

## Session flow

1. **Relay → client:** `["AUTH","<nonce>"]`
2. **Client → relay:** `["EVENT", { kind:24550, tags:[["challenge",nonce]], ... signed }]`
3. **Relay → client:** `["OK", "<event_id>", true, ""]` on success.
4. **Client → relay:** `["EVENT", { kind:24551, tags:[["room","default"]], content:"{}", ... signed }]`
5. **Relay → client:** `["OK", "<event_id>", true, ""]` then `["EVENT","oyabaun", { kind:24552, ... relay-signed }]`
6. **Input:** client repeatedly sends `["EVENT", { kind:20420, ... signed }]`. Relay responds with `OK` per event.

**Binding:** after auth, the connection is tied to the **pubkey** that signed kind **24550**. Join and input events **must** use the same `pubkey` (signature verified).

## Kind 20420 `content` (compact)

```json
{ "f": 1, "s": 0, "y": 0.0, "p": 0.0, "sh": false, "j": false }
```

- `f` / `s`: forward / strafe −1, 0, or 1  
- `y` / `p`: yaw / pitch (radians)  
- `sh` / `j`: shoot / jump  

## Relay-signed events

Snapshots and notices are normal Nostr events signed with a **relay secp256k1 key** generated at hub startup (`relay_pubkey_hex` is sent in the join-ack `content`). Clients may verify signatures for integrity.

## Optional / future

- `chat` and other features can be added as additional signed kinds.  
- Version field: optional top-level `"proto": 1` when breaking changes are needed.

## References

- [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md) — event structure, `OK`, `EVENT`, kind ranges.  
- [NIP-42](https://github.com/nostr-protocol/nips/blob/master/42.md) — relay auth pattern (`AUTH` challenge).  
- `.cursor/skills/nostr-protocol.md` — design notes and registry.
