# Oyabaun protocol

JSON message contracts are documented in [../docs/PROTOCOL.md](../docs/PROTOCOL.md).

JSON Schema in `schemas/` describes **legacy** `type`-discriminated objects where still relevant; the live WebSocket wire format is **NIP-01 arrays** (`AUTH`, `EVENT`, `OK`) as in the markdown spec.

## Event kinds (Nostr)

| Kind | Direction | Purpose |
|-----:|-------------|---------|
| 24550 | client → relay | Auth (`challenge` tag) |
| 24551 | client → relay | Join room (`room` tag) |
| 20420 | client → relay | Input frame (`room`, `seq` tags) |
| 24552 | relay → client | Join ack (relay-signed) |
| 20421 | relay → client | Snapshot (relay-signed) |
| 24553 | relay → client | Game notice (relay-signed) |
