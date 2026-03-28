# Changelog

## 2026-03-28 — Enhance Tokyo alley visual fidelity

### Changes

**`client/src/render.rs`**
- Added procedural brick/block pattern shader to `SHADER_WORLD_TEX` fragment.
  Dark surfaces (lum < 0.45) now show mortar-line brick patterns, hash-based
  grime variation, and vertical water streak stains using world-space coords.
  Bright surfaces (windows, neon signs) are left untouched.
- Note: `dpdx`/`dpdy` WGSL builtins cause silent pipeline failure in this
  wgpu/WebGPU setup; face orientation is derived from `wp.x + wp.z` instead.

**`client/levels/tokyo_alley.glb`** (re-exported from Blender)
- Added 18 horizontal ledges at floor lines (Z=3, 6, 9) on both walls.
- Added 10 awnings at street level with slight tilt.
- Added 8 AC unit boxes on building walls at various heights.
- Added 4 new materials: OYA_Trim, OYA_Awning, OYA_Frame, OYA_ACUnit.
- 331 draw batches (up from 295), 32948 verts with architectural detail.

---

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
