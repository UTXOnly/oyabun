# 90s Arcade Tokyo Level — Vision & Pipeline Options

**Branch:** `arcade-tokyo-vision` (forked from level/shader work; safe to experiment without blocking `main` / gameplay branches).

**Primary reference (commit this folder):** `example_images/arcade_tokyo_alley_ref.png`  
A **hand-pixel illustration**: one-point alley, wet cobble with **painted** rectangular reflections, tiered/banded glow (not smooth PBR bloom), dense power lines, vertical signage as **graphic shapes**, navy shadows vs orange–red lanterns. This is closer to **Neo Geo / late 2D arcade matte painting** than to **textured low-poly GLB + fog**.

## Why Blender-First Feels “Off”

| Reference | Current stack (typical) |
|-----------|-------------------------|
| Fixed illustration camera; depth is **artistic** | Free FPS camera; depth must read from **geometry** |
| Reflections = **designed pixel clusters** | No screen-space reflection; ground is albedo + shader tricks |
| **Banding / posterized** light falloff | Smooth-ish gradients + exponential fog |
| Silhouette clutter (wires, props) authored in 2D | Same clutter = mesh + draw cost + export pain |
| Every pixel intentional | Procedural brick/detail can fight authored textures |

Blender → GLB is not “wrong” for a **PS1-style corridor FPS**, but it is a **different genre** than matching this still frame 1:1. Expecting a GLB pipeline to reproduce a **single painted keyframe** without a new rendering strategy usually disappoints.

## Goals (product)

1. **Read as 90s arcade Tokyo** at a glance: navy blacks, hot orange–yellow–red accents, readable vertical signs, wet-street impression.
2. **Stay an FPS**: movement, aim, hitscan, collision, spawn — no mode that breaks netcode or authority.
3. **Iterate with Cursor + humans**: clear phases, file ownership, and “done” checks.

Non-goals for v1 of a pivot: photorealism, dynamic weather simulation, full PBR.

---

## Strategic Options (pick one primary; others can hybrid)

### Option A — **Stay GLB, tighten the marriage**

Keep `tokyo_alley.glb` as the world. Reduce shader tricks that fight art; push contrast in **textures + emissive** in Blender; optional **lower fog / no procedural brick** on sign materials (already partially explored).

- **Pros:** Existing export, collision, spawn pipeline unchanged.
- **Cons:** Still won’t get **painted reflections** or illustration composition without heavy fakery.

**When to choose:** You want incremental improvement and a believable 3D alley, not a clone of the ref still.

### Option B — **2.5D “corridor matte” + gameplay shell** (strong match for the ref)

**Idea:** Treat the alley like classic **racing / rail** games: a **high-res authored backdrop** (or 2–3 parallax layers) defines the look; a simple **floor + invisible collision** carries physics.

- Backdrop: one (or left/right split) **orthographic-style** pixel painting, or **render-to-texture** once from Blender **without** shipping the full mesh to runtime.
- Midground props that must occlude the player: sparse **cards / thin meshes** with alpha, or baked into the sheet where possible.
- **Reflections:** painted into the ground texture, or a second UV layer / detail map with mask (still 2D authorship).

- **Pros:** Visual target can approach the reference; decouples “looks” from “every brick is geometry.”
- **Cons:** New loader path (textures + layout JSON), camera constraints or careful UV projection, art pipeline is **Aseprite / PixelLab / painter** more than daily Blender.

**When to choose:** Visual fidelity to the ref matters more than arbitrary viewing angles.

### Option C — **Pre-baked environment cubemap / skydome + simplified floor**

Single **interior-ish** dome or large cylinder with the alley painted on the inside; floor plane with reflection-ish texture; minimal side geometry.

- **Pros:** Implementation smaller than full GLB city.
- **Cons:** Distortion at glances; harder to sell as “FPS alley” unless FOV and corridor length are tuned.

### Option D — **Orthographic / fixed-angle “boomer shooter” slice**

Reduce pitch limits and treat the level as **tile- or layer-based** 2.5D (Doom-adjacent or true 2D billboards).

- **Pros:** Extremely on-brand for arcade.
- **Cons:** Major camera and control UX change; may conflict with current design.

---

## Recommended Direction (for discussion)

For **Oyabaun** as an FPS with existing relay + prediction:

1. **Short term:** Option **A** tweaks (shader/material honesty, refs in `example_images/`) so current builds stop looking “broken” while deciding.
2. **Medium term (this branch):** Spike **Option B** in a **parallel code path**: e.g. `ArcadeAlleyEnvironment` that draws a fullscreen-ish world-space aligned quad (or two walls + floor) from **authored PNGs**, while **collision** comes from a **simplified AABB set** (JSON or minimal GLB with only colliders). Keep `GltfLevelCpu` for weapons/characters/props if needed.
3. **Art source:** Prefer **hand pixel** (Aseprite) or **PixelLab map-object** slices that match palette; avoid relying on procedural WGSL for “reflections.”

If the spike proves out, **deprecate** full-building Blender export for Tokyo only — not necessarily for all future levels.

---

## Implementation Phases (agent checklist)

| Phase | Deliverable | Done when |
|-------|-------------|-----------|
| **0 — Lock reference** | `example_images/arcade_tokyo_alley_ref.png` + palette notes in this doc | Image in repo; team agrees on target |
| **1 — Spike render** | One textured plane or 3-wall “box” with nearest filtering, no fog (or distance darken only) | Screenshot resembles ref **mood** more than current GLB |
| **2 — Collision** | `tokyo_alley_collision.json` (AABB list) or collider-only GLB; `ground_y_at` compatible | Player walks without falling through |
| **3 — Parallax (optional)** | 2nd layer texture, scrolls with `cam_pos.xz` at lower rate | Depth reads without full 3D |
| **4 — Integration** | `oyabaunctl` or feature flag: `TOKYO_MODE=arcade` / compile flag | Launch script can pick mode |
| **5 — Polish** | Posterize/banding in shader **only** if art is neutral enough; emissive quads for dynamic props | No single shader carries the whole look |

---

## Palette Anchor (from reference)

Use as **soft** targets for textures and tint — not law.

- **Shadow / sky:** deep navy → near black (`#121a35` … `#080a0f`)
- **Highlights:** orange `#ff7a2d`, yellow `#ffeb57`, red `#b81c1c`
- **Mid:** muted blue–violet (not heavy purple fog wash unless intentional)

**Depth cue:** Prefer **darkening + desat** with distance on the **backdrop**, not thick violet fog (ref is **clear**; depth is value and hue).

---

## Coordination With Other Docs

- **[CURSOR_ARCADE_PROP_BILLBOARDS.md](./CURSOR_ARCADE_PROP_BILLBOARDS.md)** — PixelLab MCP **cannot** run `create_character` / `animate_character` for cars/crates; multi-angle props should follow the **same atlas + billboard column math** as NPCs (`render.rs` `push_char_sprite_quad`).
- **`CURSOR_LEVEL_REDESIGN.md`** — Blender-forward redesign; still valid for **Option A** and for exporting **collision-only** or **prop** meshes under Option B.
- **`level-design.mdc`** — Update when a non-GLB world path is merged (document second pipeline).
- **Agents:** When touching world rendering, **read this file** if branch is `arcade-tokyo-vision` or issue mentions “arcade ref.”

---

## Open Questions (resolve before large builds)

1. **Camera:** Keep full mouselook, or cap pitch and narrow FOV for “diorama” readability?
2. **Multiplayer:** Do both clients need **bit-identical** backdrop assets (yes → ship PNGs in repo or CDN with version)?
3. **Scope:** Tokyo alley only, or is this the template for **all** levels?
4. **Who paints:** Human pixel artist vs PixelLab batch vs Blender **one-time** bake to PNG?

---

## Changelog

| Date | Note |
|------|------|
| 2026-03-29 | Initial doc; branch `arcade-tokyo-vision`; ref copied to `example_images/arcade_tokyo_alley_ref.png`. |
