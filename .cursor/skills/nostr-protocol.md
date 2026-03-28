# Skill: Nostr Protocol for retro-relay / Nostr-native games

Use this skill when designing or changing **multiplayer, relay, or Nostr event** behavior for **retro-relay**-style games (including this repo’s **Oyabaun** reference implementation). Prefer **Nostr-native** designs: WebSocket transport is fine, but **gameplay events should map to signed Nostr events and `REQ` subscriptions**, not ad-hoc JSON forever.

**Sources consulted for this file:** [NIP index / kind table](https://github.com/nostr-protocol/nips/blob/master/README.md), [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md) (fetched 2026-03-28). Re-verify the README kind table before reserving new kinds; it changes.

---

## NIP-01 kind ranges (conventions)

| Range | Behavior | Use for |
|------|----------|---------|
| `1000–9999` (and other “regular” bands per NIP-01) | **Regular** — stored | Match results, persistent chat, demos |
| `10000–19999` (and `0`, `3`) | **Replaceable** — latest per pubkey (+ kind rules) | Lobby “I’m hosting”, presence |
| `20000–29999` | **Ephemeral** — **not expected to be stored** | Input frames, live aim, fire-and-forget state |
| `30000–39999` | **Parameterized replaceable** — latest per pubkey + `d` | Session manifest, mode config, per-room metadata |

---

## Event Kind Registry

| Kind | Range type | Name | NIP basis | Frequency | Persisted? | Description |
|------|------------|------|-----------|-----------|------------|-------------|
| **24550** | ephemeral | `oyabaun_relay_auth` | custom (Oyabaun) | on connect | no | Client `EVENT`: proves pubkey + `challenge` tag; relay sends NIP-42-style `["AUTH",nonce]` then `OK`. Replaces deprecated **39001** (NIP-29 collision band). |
| **22242** | replaceable | Client Authentication | [NIP-42](https://github.com/nostr-protocol/nips/blob/master/42.md) | on challenge | latest only | Standard relay auth event; prefer for **Nostr-native** auth when relay speaks NIP-42 `AUTH`. |
| **20420** | ephemeral | `oyabaun_input_frame` | custom (Oyabaun) | ~20–30 Hz | **no** | Client-signed; tags `room`, `seq`; compact JSON `content`. |
| **24551** | ephemeral | `oyabaun_room_join` | custom (Oyabaun) | once per match | no | Client `EVENT`: tag `room`; relay `OK` + relay-signed **24552** ack. |
| **24552** | ephemeral | `oyabaun_join_ack` | custom (relay-signed) | once | no | Relay pubkey; `content` JSON with `your_entity_id`, `relay_pubkey_hex`, etc. |
| **20421** | ephemeral | `oyabaun_snap` | custom (relay-signed) | ~20 Hz | no | World snapshot `content` JSON. |
| **24553** | ephemeral | `oyabaun_game_notice` | custom (relay-signed) | rare | no | HUD events (e.g. kill). |
| **30078** | param. replaceable | Application-specific data | [NIP-78](https://github.com/nostr-protocol/nips/blob/master/78.md) | on change | latest per `d` | Session listing, rules, map name — `["d","session:<id>"]` + JSON `content`. |
| **1** | regular | Short text note | NIP-10 | low | yes | Optional public chat / kill shouts if not encrypted. |
| **14** / **15** | regular | DM / File (NIP-17) | [NIP-17](https://github.com/nostr-protocol/nips/blob/master/17.md) | low | yes | Private invites; **prefer NIP-17 over NIP-04** (NIP-04 marked unrecommended in README). |
| — | — | Encrypted payloads | [NIP-44](https://github.com/nostr-protocol/nips/blob/master/44.md) | — | — | Use for **new** encrypted content; do not specify NIP-04 for new features. |
| — | — | Protected events | [NIP-70](https://github.com/nostr-protocol/nips/blob/master/70.md) | — | — | Limit rebroadcast of sensitive game metadata where applicable. |
| — | — | Count | [NIP-45](https://github.com/nostr-protocol/nips/blob/master/45.md) | on demand | — | Lobby / player counts via `COUNT` + filters. |

Custom kinds follow [NIP-01](https://github.com/nostr-protocol/nips/blob/master/01.md) conventions; this relay does not persist hot-path events.

### Oyabaun wire summary

| Step | Message |
|------|---------|
| Auth | `["AUTH",nonce]` → client `["EVENT",24550]` → `["OK",…]` |
| Join | client `["EVENT",24551]` → `OK` + relay `EVENT` **24552** |
| Input | client `["EVENT",20420]` → `OK` |
| Snap | relay `EVENT` **20421** |
| Chat (future) | **kind 1** or **NIP-17** |

---

## Category audit (retro-relay checklist)

### A. Real-time player input (hot path)

| Item | Data | Producer → consumer | Ephemeral? | Rate | Kind approach |
|------|------|---------------------|------------|------|---------------|
| Input frames | keys, fire, seq | player → relay → peers | yes | up to 60 Hz | **20000–29999**, `#e` session |
| Mouse / aim | yaw, pitch (or delta) | player → relay → peers | yes | same | same event or separate **ephemeral** kind |
| Seq / rollback | frame id, checksum | player ↔ relay | yes | high | tags: `["seq","N"]` or JSON in `content` |

### B. Game state sync

| Item | Persist? | Suggested |
|------|----------|-----------|
| Authoritative snapshots | usually no for full rate | Server-generated **ephemeral** or non-Nostr channel until model is clear |
| Health / score deltas | optional | **Ephemeral** events or batched **regular** for match log |
| Death / respawn | record for stats | **Regular** (1000–9999) if persisted to history |

### C. Session & lobby

| Item | Suggested |
|------|-----------|
| Host announces game | **30000–39999** + `d` (session id) or NIP-78 **30078** |
| Browse / count | **NIP-45** `COUNT` + filters on kinds/tags |
| Join / ready | signed events + `p` / `e` tags |

### D. Social / meta

Chat: **kind 1** or **NIP-17**; kill feed: **ephemeral** or **regular** depending on replay needs.

### E. Anti-cheat / integrity

Commit-reveal, state hashes: **regular** or **replaceable** checkpoints; do not trust client `created_at` alone for competitive play — relay or coordinator logic.

### F. Rollback (GGPO-style)

Input prediction and rollback triggers: **ephemeral** streams; snapshots for rewind: **regular** or side-channel blob refs (e.g. blossom) if too large for events.

---

## Tag conventions

| Tag | Purpose | Example |
|-----|---------|---------|
| `e` | Session or parent game event | `["e","<session_root_event_id>"]` |
| `p` | Target player | `["p","<hex_pubkey>"]` |
| `d` | Parameterized id (NIP-33 / 30000 range) | `["d","retro-relay:session:<uuid>"]` |
| `t` | Game / mode | `["t","doom-deathmatch"]` |
| `challenge` | Relay auth nonce (Oyabaun today) | `["challenge","<nonce>"]` — keep if still using challenge-response |
| `seq` | Monotonic input sequence | `["seq","12345"]` |

---

## Event schemas (target shapes)

### Ephemeral — player input frame (proposed kind **20420**)

```json
{
  "kind": 20420,
  "tags": [
    ["e", "<session_event_id>"],
    ["seq", "1234"]
  ],
  "content": "{\"f\":0,\"s\":0,\"y\":0.0,\"p\":0.0,\"sh\":false,\"j\":false}",
  "created_at": 1710000000
}
```

`content` should stay **small** (binary-safe encoding optional later).

### Parameterized replaceable — session manifest (NIP-78 **30078** example)

```json
{
  "kind": 30078,
  "tags": [
    ["d", "retro-relay:session:abc123"],
    ["t", "oyabaun-arena"]
  ],
  "content": "{\"title\":\"default\",\"max_players\":8,\"proto\":1}",
  "created_at": 1710000000
}
```

### Oyabaun relay auth (**24550**)

```json
{
  "kind": 24550,
  "tags": [["challenge", "<nonce_from_relay>"]],
  "content": "",
  "created_at": 1710000000
}
```

---

## Relay configuration requirements

- Allow listed **custom kinds**: Oyabaun **24550–24553**, **20420–20421**; optional **22242** (NIP-42) on full relays; **30078** for session manifests if used.
- **Ephemeral:** relays MUST NOT store `20000–29999` per NIP-01 convention; game relays should **forward only** on hot path (e.g. Redis pub/sub), **no Postgres** for input frames.
- **Rate limits:** per-pubkey caps for **20–60 Hz** inputs; reject oversized `content`.
- If using **NIP-70**, configure visibility so game events are not broadly rebroadcast when undesired.

---

## Anti-patterns (NEVER DO THIS)

- **No WebRTC** for player gameplay data if the goal is Nostr-native signaling/data (prompt constraint).
- **No unsigned** gameplay events on the wire; client must sign **20420** (and auth/join kinds) or relay rejects.
- **Do not persist ephemeral kinds** to disk as the source of truth.
- **Do not hardcode** relay URLs in protocol logic — discovery / user relay lists (e.g. NIP-65) where applicable.
- **Do not use NIP-04** for new encrypted features (README marks unrecommended); use **NIP-17 / NIP-44**.
- **Do not use 39001** — conflicts with **NIP-29 `39000–39009` group metadata**; Oyabaun uses **24550** instead.

---

## Behavioral rules (for agents)

1. **Protocol-first:** if tradeoff vs “easy JSON”, choose Nostr-shaped events + subscriptions.
2. **Fetch, don’t assume:** re-read NIP README kind table before assigning numbers.
3. **Minimize custom kinds:** use **22242**, **30078**, **kind 1**, **NIP-17** when they fit.
4. **Document every kind** in this file and in `docs/PROTOCOL.md` / `protocol/README.md`.
5. **Ephemeral means ephemeral** — design no logic that requires later `REQ` of hot-path input.
6. **Subscription-first:** every consumer should use **tight `REQ` filters** (`kinds`, `#e`, `authors`).

---

## Last verified

**2026-03-28** — NIP README and NIP-01 fetched; Oyabaun wire protocol updated to NIP-01 `AUTH`/`EVENT`/`OK` + kinds **24550–24553**, **20420–20421**. Re-run README kind-table check before allocating new numbers.
