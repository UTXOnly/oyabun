# Claude handoff — 90s arcade Tokyo alley (Oyabaun)

**Purpose:** Give Claude enough context to help on the **visual direction** and optional **pipeline pivot** without re-explaining the whole repo.

## Share these 2 files with Claude

1. **This file:** `docs/CLAUDE_ARCADE_TOKYO_HANDOFF.md`
2. **Art target:** `example_images/arcade_tokyo_alley_ref.png` (attach or ensure the repo checkout includes it)

Everything else (full option matrix, phases, palette) is expanded in `docs/CURSOR_ARCADE_TOKYO_LEVEL.md` inside the repo when Claude has the full tree.

---

## Project (one paragraph)

**Oyabaun** is a Nostr-session-auth FPS: **Rust + wgpu → WASM** (`client/`), **Go WebSocket relay** for gameplay (`relay/`). Levels today load **`client/levels/tokyo_alley.glb`** (spawn, AABB collision, textured batches). World shading lives in **`client/src/render.rs`** (`SHADER_WORLD_TEX` / fog / posterize). Do **not** add alternate realtime gameplay transports; do **not** add a generic Nostr relay.

---

## What we want

Match the **reference PNG**: hand-pixel **90s arcade** Tokyo night alley — navy/black shadows, orange–yellow–red lanterns and signs, **wet cobble reflections as painted shapes**, banded glow, dense wires. That look is **illustration-led**, not “low-poly GLB + fog approximates a photo.”

---

## Branch

Use / assume **`arcade-tokyo-vision`** for experiments that might replace or sideline the full Blender scenic export for Tokyo.

---

## Strategic choice (short)

| Track | Idea |
|--------|------|
| **A — Keep GLB** | Improve textures/emissive in Blender; tame shaders so they don’t fight art. |
| **B — 2.5D matte (recommended if matching the ref)** | Authored **large PNG** (walls/sky/floor) + **simple collision** (JSON AABBs or collider-only GLB); optional parallax layer. FPS stays; look moves toward the still. |

Details and phase checklist: `docs/CURSOR_ARCADE_TOKYO_LEVEL.md`.

---

## Implementation hints (when coding)

- After **Rust/WGSL** changes: `wasm-pack build` from `client/`.
- Level export today: `oyabaunctl.py export-world` (Blender). A new 2.5D path would add assets under e.g. `client/level_textures/` or `client/levels/` + loader changes in `gltf_level.rs` / `lib.rs` / `render.rs` — **design first**, then minimal spike (one textured plane + existing collision or stub).
- **KHR_texture_transform** is required on the `gltf` crate if GLB materials use UV transform; unrelated to PNG-backdrop path.

---

## Don’t

- Add client-side **procedural world geometry** to fake the reference (project rule: world detail belongs in authored art / Blender).
- Ship heavy **violet fog** as a stand-in for the ref’s depth (ref uses **value/hue**, not murk).
- Rewrite netcode or protocol unless the user explicitly asks.

---

## Changelog

| Date | |
|------|---|
| 2026-03-29 | Initial Claude handoff; pair with `example_images/arcade_tokyo_alley_ref.png`. |
