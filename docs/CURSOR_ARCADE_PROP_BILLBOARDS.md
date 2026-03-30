# Arcade props: multi-angle & animation (PixelLab MCP reality + engine path)

## Parked cars / static props: **do not** use camera-facing billboards

A sprite quad that **yaw-bills toward the camera** (same path as NPCs) makes a parked car **spin to face the player** — it reads as a HUD sticker, not geometry. **Removed from the engine:** `PropBillboardCpu` / prop atlas draw for the R32.

**Use instead:**

- **Merged `.glb` props** (what the arcade **parked car slot** does now): `gltf_level::append_glb_transform` appends mesh + materials after procedural `GltfLevelCpu` build; ship **`client/props/arcade_parked_car_blockout.glb`** or replace it with a Blender export (pixel textures, nearest).
- **Fixed world quads** for flat signage only — not for a walk-around vehicle silhouette.
- **`wall_prop`-style boxes** for small props (trash, crates, bikes).
- **Full levels:** model in **Blender**, export **`.glb`** per `level-design.mdc`.

The sections below still apply when you **want** camera-facing sprites — e.g. **moving** entities or distant impostors — not for a static parked vehicle.

---

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

3. **Engine work** (optional — only if you add **moving** prop billboards later):
   - A sibling of `push_char_sprite_quad` + a dedicated prop atlas bind group (same UV/column math as NPCs).
   - **Do not** use this for **parked** cars in the arcade alley; keep them as **fixed meshes** or glTF.

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
| 2026-03-30 | Link parked car merge path to `arcade_r32_prop.glb` + [`CURSOR_ARCADE_PARKED_VEHICLE.md`](./CURSOR_ARCADE_PARKED_VEHICLE.md). |
| 2026-03-30 | Initial doc: MCP limits + billboard reuse plan. |
