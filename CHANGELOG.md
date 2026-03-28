# Changelog

## 2026-03-28 — Fix glTF level rendering (player ejected from map)

### Root cause
When no explicit collision meshes (`Collider`-named nodes) exist in the GLB,
`parse_glb` used the entire level bounding box as a single collision solid.
The movement code (`resolve_xz`) detected the player was *inside* this giant
AABB and pushed them outside the map — resulting in the "nothing like Blender"
view reported by testers.

### Changes

**`client/src/gltf_level.rs`**
- Fixed no-collider fallback: creates a thin floor slab at `bounds.min.y`
  instead of the full level AABB that was trapping/ejecting the player.
- Added emissive color support: materials with black `baseColorFactor` but
  non-zero `emissiveFactor` (signs, neon lettering) now use the emissive
  color as the tint instead of rendering invisible.
- Added brightness boost (2.8x) for factor-only materials (no image texture)
  since the unlit shader has no lighting and Blender's dark base colors
  (tuned for Eevee) were nearly invisible.

**`client/src/render.rs`**
- Added height-gradient ambient light to `SHADER_WORLD_TEX` fragment shader
  so Eevee-tuned dark materials are visible in the unlit posterize renderer.

**`client/src/lib.rs`**
- Added `bootDebugJson()` wasm_bindgen endpoint returning: `level_label`,
  `vert_count`, `batch_count`, `bounds_min/max`, `spawn`, `mural_z`.
- Tracks `vert_count` and `batch_count` in `GameInit` and `OyabaunApp`.

**`client/index.html`**
- Logs `bootDebugJson()` to browser console on startup for diagnostics.

### Still needed
- All 12 materials in the GLB are solid-color only (0 images, 0 textures).
  Baking image textures in Blender would significantly improve visual fidelity.
- No explicit `Collider`-named meshes in the Blender scene; adding them would
  give proper wall collision instead of the current floor-slab-only fallback.
