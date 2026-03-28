# Oyabaun architecture

## Overview

Oyabaun is a browser-first multiplayer FPS. The **client** (Rust → WebAssembly) renders with **WebGPU** via **wgpu** and exchanges state over a **WebSocket** to a **Go relay** that speaks **NIP-01** wire messages (`AUTH`, `EVENT`, `OK`). Player identity is a **Nostr pubkey**; **auth, join, and each input frame** are **signed Nostr events** (browser NIP-07). Snapshots and notices are **relay-signed** events.

## Client (Rust / WASM)

| Module area | Responsibility |
|-------------|----------------|
| `render` | wgpu pipelines: textured glTF **level**, **3D character** meshes (per-entity `model` matrix), HUD, optional backdrop sprite. |
| `game` | Local tick: movement intent, weapon cooldown, predicted pose. |
| `net` + page script | Ingest `EVENT`/`OK` arrays; apply relay-signed snaps; WASM queues **20420** unsigned drafts; NIP-07 `signEvent` for **24550**, **24551**, **20420**. |

### Playable characters (PixelLab → Blender → Rust → relay)

- **Art**: PixelLab (or hand-painted) textures are assigned in Blender to a **rigid body** mesh named for export (see `tools/blender_make_oyabaun_character.py` and `docs/BLENDER_GLTF.md`).
- **Asset**: `client/characters/oyabaun_player.glb` — loaded at boot (`include_bytes!` + optional fetch of `./characters/oyabaun_player.glb`). Parsed with `gltf_level::parse_character_glb`.
- **Net**: Relay snapshots already carry **`x`, `y`, `z`, `yaw`** per player (`PROTOCOL.md` kind **20421**). The client builds a `Mat4` per remote entity (scale × rotation about Y × translation at feet) and draws the shared character mesh once per instance.
- **Local PvE**: Boss / rival use the same mesh at `boss_foot` / `rival_foot`, rotated to face the camera on XZ.
- **Future**: glTF **skins** and **animation clips** (walk cycles from Blender) are not in the client yet; add joint palette + sampled animation before expecting bone-animated PixelLab rigs.

**Render loop**: poll input → advance predicted simulation → draw → send aggregated input at a fixed cadence (e.g. 20–30 Hz), not per animation frame.

## Relay (Go)

| Component | Responsibility |
|-----------|----------------|
| Hub | WebSocket; `AUTH` challenge; verify client `EVENT`s; **conn → pubkey** session; relay key for outbound signed `EVENT`s. |
| Room | Match tick, authoritative transforms, hitscan traces, damage, scores, respawn timers. |
| Auth | NIP-01 `EVENT`: verify **kind 24550** + `challenge` tag + ECDSA; bind **pubkey** to WebSocket session (no opaque join token). |
| Limits | Per-IP and per-connection rate limits; movement speed / fire rate caps. |

No generic Nostr event storage for MVP; the relay is an application server that uses **Nostr-shaped events on the wire** (not a public REQ/EVENT nostr-rs-relay, but the same event object and message framing).

## Authority vs prediction

| Concern | Owner |
|---------|--------|
| Join / leave, match id | Relay |
| Health, death, respawn time, score | Relay |
| Hit confirmation, damage numbers | Relay (hitscan on server) |
| Aim + movement intent | Client sends; relay validates and applies |
| Local view latency | Client predicts own motion; reconciles from snapshots |

## Cheat resistance (MVP)

- Server validates position delta per tick (max speed, sane gravity if jump exists).
- Hitscan performed **on server** from reported aim at shoot time; optional rewind buffer can come later.
- Tokens expire; reconnect re-auths.
- Rate limit shots and messages.

Trust model: clients are untrusted except for signed auth proving pubkey ownership.

## Major risks

- **WebGPU availability**: require supported browsers; fallback (WebGL2) would be a separate renderer.
- **Latency**: aggressive prediction vs visible correction; tune snapshot rate and interpolation later.
- **Relay scale**: single process in-memory rooms for MVP; sharding later if needed.

## Libraries (choices)

- **Client**: `wgpu`, `glam`, `serde`/`serde_json`, `wasm-bindgen` (+ `web-sys`).
- **Relay**: `gorilla/websocket`, std `crypto` for verify token / optional HMAC.

See [PROTOCOL.md](PROTOCOL.md) for wire messages and [../protocol/README.md](../protocol/README.md) for schemas.
