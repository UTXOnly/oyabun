---
name: oyabaun
description: >-
  Nostr-native multiplayer FPS Oyabaun — Rust/wgpu WASM client, Go WebSocket relay,
  session token after signed auth, authoritative hitscan and game state. Use when
  working in this repo, changing protocol, netcode, Nostr auth, relay rooms, or WASM build.
---

# Oyabaun

## Layout

| Path | Role |
|------|------|
| `client/` | Rust game; build WASM with `wasm-pack build --target web` from `client/` |
| `relay/` | Go `oyabaun-relay` binary |
| `protocol/` | JSON Schema + field definitions (source of truth with `docs/PROTOCOL.md`) |
| `infra/` | `docker-compose.yml`, relay Dockerfile |
| `docs/` | Architecture and protocol narrative |

## Authority

- **Relay**: Spawns, health, damage, deaths, scoreboard, clock; validates inputs (rate, speed, LOS where cheap).
- **Client**: Prediction for local movement and look; reconciliation when snapshots arrive.
- **Signed**: Initial auth only (Nostr event). Gameplay uses relay-issued **session token**.
- **Transport**: Realtime match traffic is **WebSocket to Oyabaun relay only**—no P2P gameplay channels; Nostr is identity/auth (and optional out-of-band), not the tick stream.

## Auth flow (MVP)

1. Relay sends `["AUTH","<nonce>"]` (NIP-42-style).
2. Client publishes `["EVENT", <signed kind **24550**>]` with `challenge` tag; browser via NIP-07.
3. Relay `["OK",…]` then client `["EVENT", <signed kind **24551** join>]`; relay pushes **24552** join ack and **20421** snaps. Input: signed **20420**. See `docs/PROTOCOL.md`.
4. Client sends `join` with token and desired `room_id`.

## Dev

- Relay: `cd relay && go run ./cmd/oyabaun-relay` (or Docker Compose from repo root).
- Control script: `tools/oyabaunctl.py` (state under `.oyabaun/`). Export levels: `python3 tools/oyabaunctl.py export-world` (default blend `client/levels/tokyo_alley.blend`) → `client/levels/tokyo_alley.glb` (+ JSON). Tokyo packed albedos: `export-world --enhance` or `enhance-tokyo-alley`; full texture rebuild: add `--repack`.
- Client static: serve `client/pkg/` after wasm-pack, or use `python3 -m http.server` from `client/` with correct headers if needed.

## Conventions

- Protocol changes: update `docs/PROTOCOL.md` and `protocol/schemas/` together.
- Keep cheat resistance assumptions documented in `docs/ARCHITECTURE.md` when adding features.
