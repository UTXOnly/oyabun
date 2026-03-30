# Arcade props: multi-angle & animation (PixelLab MCP reality + engine path)

## What the PixelLab MCP **cannot** do for props

| Tool | What it does | Props / cars? |
|------|----------------|---------------|
| **`create_character`** | **Humanoid** (template skeleton) or **quadruped** only (`bear`, `cat`, `dog`, `horse`, `lion`). Returns 4 or 8 **directional rotations** + optional template animations. | **No.** Not for vehicles, crates, bags, or static scene clutter. Wrong rig; “describe a car as a character” produces garbage. |
| **`animate_character`** | Queues animation jobs for an existing **`character_id`** from `create_character`. | **No** standalone prop animation API. |
| **`create_map_object`** | **One** PNG per job (async map-object). | **Yes** for a **single** view only. Multi-angle = **many jobs** (or hand layout), not one MCP call. |
| **`create_isometric_tile`** / **tileset** tools | Terrain / tiles. | Wrong product for alley props. |

**Conclusion:** You **cannot** get “like characters” 8-dir + walk templates from MCP **for props** using `create_character` / `animate_character`. Those APIs are **character-specific**.

---

## Approach that **does** match how NPCs work

NPCs use:

1. An **atlas** (8 columns = compass views, multiple **rows** = idle / walk / shoot frames).
2. **CPU each frame**: camera-facing quad + **column** from `atan2` of camera vs entity **yaw** (see `push_char_sprite_quad` in `client/src/render.rs`).
3. **`SHADER_BILL`**: nearest sample, alpha discard, fog — same for props.

**For props you want the same three ideas**, with different art production:

1. **Build a prop atlas** (per prop type or one shared atlas):
   - **8 columns**: S, SE, E, NE, N, NW, W, SW (same order as PixelLab character ZIPs / `pixellab_zip_to_atlas.py`).
   - **Rows**: row `0` = static idle; extra rows only if you want **animation** (e.g. neon flicker = 2–4 duplicate-ish frames, or a short loop drawn in Aseprite).

2. **Produce the pixels** (pick one):
   - **A)** Eight separate `pixellab_v2.py map-object` calls with explicit facing in the prompt, then stitch in Aseprite / script into one PNG → `.rgba` via `export_character_atlas_to_rgba.py` (same header format as characters: width, height LE u32, then RGBA).
   - **B)** Hand-draw in Aseprite (best control, thick outlines, no photo creep).
   - **C)** Blender render ortho **8 views** → downscale/posterize → atlas (actual 3D, more work).

3. **Engine work** (not done yet — design only here):
   - Add **`PropBillboardInstance`** (or reuse a slim `CharacterInstance` with a `skin` enum value `PropCar`, etc.): `foot`/`center` position, **`mesh_yaw`** (car long axis along alley = `0` or `±π/2`), **`anim_frame`** row, **width/height** in meters (R32 wider than a trash pile).
   - In **`Gpu::draw_world`**, after character quads, call a sibling of `push_char_sprite_quad` that uses a **prop atlas bind group** (third atlas: `prop_atlas.rgba`) or a **small array of atlases** by prop type.
   - **Collision**: keep **AABB** from `arcade_level` (or JSON); rendering is decoupled from the old `wall_prop` boxes.

4. **Animation**:
   - **Simple:** increment `anim_frame` / row on a timer in `OyabaunApp` update (same as walk row for NPCs).
   - **No** PixelLab `animate_character` unless the prop is literally authored as a **character** (not recommended for a car).

---

## Migration from current `wall_prop` boxes

- **Phase 1:** Ship **one** pilot prop (e.g. R32) as 8-dir billboard + AABB only; remove its three world-texture quads from `arcade_level.rs`.
- **Phase 2:** Trash / crates / bike atlases; replace remaining `wall_prop` clusters.
- **Phase 3:** Optional **second atlas bind group** or **texture array** if atlas count grows.

---

## Alternatives if atlases are too heavy

- **Blender → GLB** small props (real geometry), exported with the rest of the level — works but fights the “pure `arcade_level.rs`” direction.
- **Pre-baked 2.5D**: one hero angle only + no rotation (cheapest, looks bad when strafing — you already rejected this).

---

## Changelog

| Date | |
|------|---|
| 2026-03-30 | Initial doc: MCP limits + billboard reuse plan. |
