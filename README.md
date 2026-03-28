# Oyabaun

Nostr-native multiplayer FPS (MVP): Rust + WebGPU in the browser, specialized Go relay over WebSocket.

## Repo layout

| Path | Contents |
|------|----------|
| `client/` | Rust WASM crate (`wgpu`, `wasm-bindgen`) |
| `relay/` | `oyabaun-relay` — WebSocket hub, rooms, auth, snapshots |
| `protocol/` | Schema stubs + pointers to `docs/PROTOCOL.md` |
| `docs/` | Architecture and protocol |
| `infra/` | Docker Compose for the relay |
| `example_images/` | Reference clips for character / tone — not loaded by the client yet |

## Dev CLI (`tools/oyabaunctl.py`)

From the repo root (Python 3.9+):

```bash
python3 tools/oyabaunctl.py status
python3 tools/oyabaunctl.py rebuild              # wasm-pack + go build -> .oyabaun/oyabaun-relay
python3 tools/oyabaunctl.py launch               # relay binary + http.server on client/
python3 tools/oyabaunctl.py launch --build       # rebuild then launch
python3 tools/oyabaunctl.py launch --docker      # relay via Docker Compose only
python3 tools/oyabaunctl.py stop                 # stop tracked local pids or compose down
```

Logs: `.oyabaun/relay.log`, `.oyabaun/http.log`. State: `.oyabaun/state.json`.

Relay is started as **`.oyabaun/oyabaun-relay`** (built automatically on first launch), not `go run`, so `stop` can tear down a single process cleanly. `stop` also sends **SIGTERM to the whole process group**, then **SIGKILL** if needed, and runs **`lsof`** to clear anything still listening on the saved relay/HTTP ports (or `:8765`/`:8080` if state is missing).

## Prerequisites

- **Rust** with `wasm32-unknown-unknown`: `rustup target add wasm32-unknown-unknown`
- **wasm-pack**: [install](https://rustwasm.github.io/wasm-pack/installer/), then build the client bindings
- **Go 1.22+** for local relay
- **Docker** (optional) for relay-only compose

## Run the relay (local)

```bash
cd relay && go run ./cmd/oyabaun-relay
```

- HTTP health: `http://127.0.0.1:8765/healthz`
- WebSocket: `ws://127.0.0.1:8765/ws`

## Run the relay (Docker)

From repo root:

```bash
docker compose -f infra/docker-compose.yml up --build
```

## Build the browser client

```bash
cd client
wasm-pack build --target web --out-dir pkg
```

From `client/` after `wasm-pack`:

```bash
wasm-pack build --target web --out-dir pkg
python3 -m http.server 8080
```

Open `http://127.0.0.1:8080/` (serves `index.html` and `./pkg/oyabaun_client.js`).

The page opens a **WebSocket** to `ws://<host>:8765/ws`, speaks **NIP-01** frames (`AUTH`, `EVENT`, `OK`): **kind 24550** auth and **24551** join are signed with your browser extension (NIP-07); **kind 20420** input frames are signed each tick. The relay pushes **relay-signed** snapshots (**20421**). See `docs/PROTOCOL.md`. Start the relay with `tools/oyabaunctl.py launch` (or `go run` in `relay/`).

Environment (relay): `OYABAUN_RELAY_ADDR` (listen address), `OYABAUN_GAME_LOG` (optional JSONL path for authoritative gameplay events).

## Protocol summary

1. Connect WebSocket → receive `["AUTH","<nonce>"]` (NIP-42-style).
2. Send `["EVENT", <signed kind 24550 with challenge tag>]`.
3. Receive `["OK",…,true,…]` then send `["EVENT", <signed kind 24551 join>]`.
4. Receive relay `EVENT` **24552** (join ack) and **20421** snapshots.
5. Send signed **20420** input `EVENT`s at bounded rate with monotonic `seq` (in tags).

Details: [docs/PROTOCOL.md](docs/PROTOCOL.md).

## Cursor

Project rules live in `.cursor/rules/`. The Oyabaun skill is `.cursor/skills/oyabaun/SKILL.md`.

## License

Proprietary / TBD.
