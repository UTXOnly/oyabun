# Oyabaun Level Design Skill

## Overview

End-to-end pipeline for creating and modifying levels in Oyabaun. Levels are built in Blender (Z-up), exported as glTF Binary (.glb, Y-up), and rendered by the Rust/WASM client with WebGPU.

---

## Architecture Quick Reference

### File Layout

```
client/levels/tokyo_alley.blend    — Blender source (Z-up)
client/levels/tokyo_alley.glb      — Exported binary (Y-up), embedded in WASM
client/src/gltf_level.rs           — GLB parser (images, materials, collision, spawn)
client/src/render.rs               — Shaders (SHADER_WORLD_TEX, SHADER_FLAT, SHADER_CHAR_TEX)
client/src/mesh.rs                 — Collision AABBs, NPC placement, spawn yaw
client/src/lib.rs                  — load_game_init(), game_init_from_gltf()
tools/blender_export_gltf_oyabaun.py  — Headless GLB export script
tools/blender_make_oyabaun_character.py — Character atlas → GLB
```

### Coordinate Systems

| System | Up | Forward | Right |
|--------|-----|---------|-------|
| Blender | +Z | -Y | +X |
| glTF / Game | +Y | -Z | +X |
| Conversion | `game = (blender_x, blender_z, -blender_y)` | Auto by glTF exporter |

---

## Level Creation Workflow

### Step 1: Build in Blender

1. Open `client/levels/tokyo_alley.blend`
2. Model geometry in Z-up space
3. Create materials using **Principled BSDF** only:
   - Textured: Base Color → Image Texture (PNG, nearest interpolation, packed in .blend)
   - Emissive: Emission Color + Emission Strength (for neon signs, glowing surfaces)
   - Flat color: Base Color factor only (no texture node)
4. Name collision volumes with `Collider` prefix (e.g., `Collider_Wall_Left`)
5. Place spawn Empty named `OyabaunSpawn` or `PlayerSpawn`

### Step 2: Export GLB

**Automated:**
```bash
python3 tools/oyabaunctl.py export-world
```

**Manual headless:**
```bash
OYABAUN_GLB_OUT=$(pwd)/client/levels/tokyo_alley.glb \
/Applications/Blender.app/Contents/MacOS/Blender \
  client/levels/tokyo_alley.blend \
  --background \
  --python tools/blender_export_gltf_oyabaun.py
```

**Export settings (in script):**
- Format: GLB (binary)
- Materials: EXPORT (embeds textures)
- Coordinates: Y-up (`export_yup=True`)
- TexCoords + Normals: Yes
- Animations: No

### Step 3: Rebuild WASM

```bash
cd client && wasm-pack build --target web --out-dir pkg
```

The GLB is embedded via `include_bytes!()`. Without rebuilding, the embedded copy is stale (the fetched copy from the static server will work, but the fallback won't).

### Step 4: Test

Serve from `client/` directory:
```bash
python3 -m http.server 8080 --directory client
```

Check browser console for:
- `"glTF tokyo_alley"` in header = GLB loaded successfully
- `"embedded tokyo_alley.glb parse failed"` = Extension/parse error (see troubleshooting)

---

## Material & Texture Guide

### Material Types

| Type | Base Color | Texture | Emission | Example |
|------|-----------|---------|----------|---------|
| Textured surface | White (1,1,1,1) | PNG image | None | OYA_Asphalt, OYA_Building |
| Emissive sign | White | Optional PNG | Color + Strength | SignGold, Neon_Red |
| Flat color (no texture) | RGB factor | None | Optional | Gun_Dark, FPS_Skin |

### Texture Specifications

- **Size**: 32–256px (pixel art scale)
- **Format**: PNG with RGBA
- **Interpolation**: Nearest-neighbor (set in Blender texture node AND enforced by shader)
- **Packing**: Must be embedded in .blend file (Image → Pack)
- **UV Tiling**: Use Mapping node for offset/scale — exported as `KHR_texture_transform`

### How Textures Are Rendered

The `fs_tex` fragment shader (render.rs) applies:

1. **Sample**: `textureSample(albedo, nearest_sampler, uv) * tint`
2. **Detail overlay**: Procedural brick pattern on dark surfaces (luminance < 0.45)
3. **Ambient lighting**: Height-gradient warm/cool split:
   - Warm overhead: `(0.30, 0.22, 0.16)` — orange/amber, stronger at height
   - Cool bounce: `(0.12, 0.20, 0.26)` — teal, stronger at ground level
4. **Neon spill**: Sinusoidal RGB bands along alley (fake colored light from signs)
5. **Emissive boost**: Surfaces with luminance > 0.5 get extra glow
6. **Brightness**: Texture color × 1.8
7. **Posterization**: 24 color levels (arcade CRT aesthetic)
8. **Fog**: Purple haze `(0.10, 0.06, 0.16)`, density 0.015

### Fog & Atmosphere Parameters (render.rs)

```rust
fog_color: [0.10, 0.06, 0.16, 1.0],  // Purple haze
fog_params: [0.015, 0.0, 0.0, 0.0],  // Density (lower = see further)
```

Clear color (lib.rs): `Vec3::new(0.10, 0.06, 0.16)` — matches fog for seamless fade.

---

## Collision System

### AABB-Based

- Collision uses axis-aligned bounding boxes only (no per-vertex)
- Name meshes with `Collider` or `OyabaunCollision` to mark as collision
- The mesh's world-space AABB is computed and stored as a `solid`
- Auto-generates floor slab if no explicit colliders exist

### Player Movement

- `ground_y_at(x, z)` finds the floor height at any XZ position
- `feet_draw_y(x, z)` = ground height for rendering characters
- Horizontal collision pushes player outside AABB boundaries

---

## Spawn & NPC Placement

### Player Spawn

**In Blender:**
- Add Empty → name it `OyabaunSpawn`
- Position where player starts
- Rotation determines initial facing (forward = Blender -Y → game -Z)

**Fallback:** If no spawn found, defaults to alley mouth (min-Z side, 14% from edge).

**Yaw override:** Spawn yaw is overridden to face the midpoint of boss + rival NPCs.

### NPC Placement (mesh.rs)

```rust
// Boss: 11m forward, 1.8m right of spawn
// Rival: 17m forward, 2.4m left of spawn
// "Forward" = direction from spawn toward alley center (not spawn yaw)
```

---

## GLB Parsing Details (gltf_level.rs)

### Data Flow

```
GLB bytes → gltf::import_slice()
  → Document + Buffers + Images
  → images → image_data_to_rgba() → Vec<(u32, u32, Vec<u8>)>
  → scene nodes → visit_node() recursive
    → per primitive:
       - material → image_index + tint + KHR_texture_transform
       - positions → world-transformed vertices
       - UVs → texture coords (with transform baked in)
       - indices → triangle list
       - collision check (name contains "Collider")
    → spawn detection (name = "OyabaunSpawn")
  → GltfLevelCpu { vertices, indices, batches, images_rgba8, spawn, solids }
```

### KHR_texture_transform Handling

Materials with UV offset/scale/rotation have `KHR_texture_transform` extension data. This is extracted at parse time and baked into vertex UVs:

```rust
// Extract transform
let (offset, scale, rotation) = texture_transform_from_info(bct_info);

// Apply to each UV
uv_out = rotate(uv_in, rotation) * scale + offset;
```

**Critical**: The `gltf` crate feature `"KHR_texture_transform"` must be enabled in `Cargo.toml`. Without it, the entire GLB is rejected with `extensionsRequired` error.

### White Texture Fallback

If a batch references a nonexistent image index, a 1×1 white pixel texture is used. Combined with the tint, this produces flat-colored geometry.

---

## Shader Reference

### World Textured (`SHADER_WORLD_TEX` / `fs_tex`)

```
Inputs: pos (world), uv (texture coords)
Uniforms: view_proj, cam_pos, fog_color, fog_params, tint, albedo texture

1. Sample texture × tint
2. Procedural brick detail on dark surfaces
3. Height-gradient ambient (warm + cool)
4. Neon spill (sinusoidal color bands)
5. Emissive boost for bright surfaces
6. Posterize to 24 levels
7. Fog blend (exponential distance)
```

### Character Billboard (`SHADER_CHAR_TEX` / `fs_char`)

```
Atlas: 8 columns (directions) × 7 rows (1 idle + 6 walk frames)
char_params: [mesh_yaw, world_x, world_z, anim_row]

1. Compute camera-to-character bearing
2. Subtract mesh yaw → relative angle
3. Quantize to 8 directions (+4 offset for PixelLab convention)
4. Select atlas column (direction) and row (animation frame)
5. Sample + ambient + posterize + fog
```

### Flat Color (`SHADER_WORLD` / `fs_main`)

```
Inputs: pos (world), col (vertex color RGB)
1. Color boost: col * 0.90 + ambient
2. Posterize to 24 levels
3. Fog blend
```

---

## Troubleshooting

### "extensionsRequired KHR_texture_transform: Unsupported extension"

**Cause**: Blender materials use Mapping nodes (UV scale/offset), which export as `KHR_texture_transform`. The gltf crate doesn't support it without the feature flag.

**Fix**: Ensure `Cargo.toml` has:
```toml
gltf = { version = "1.4.1", features = ["import", "names", "KHR_texture_transform"] }
```

### Level appears all white / falls back to procedural arena

**Cause**: GLB failed to parse. Check browser console for error messages.

**Common issues**:
- Missing `KHR_texture_transform` feature (see above)
- GLB file corrupt or wrong path
- Blender export with unsupported extensions

### Textures show as white/untextured

**Possible causes**:
1. Material has no Image Texture node → uses white fallback + tint
2. Image not packed in .blend → missing from GLB
3. UV coordinates missing → fallback UVs generated from world position
4. `image_index` out of range → white fallback

### Characters invisible or too dark

- Check `ambient` in `fs_char`: should be `(0.25, 0.20, 0.28)`
- Check character GLB has atlas texture embedded
- Verify `anim_frame` is 0.0 for idle (not NaN)

### NPC positions wrong

- `npc_placements()` uses direction from spawn toward alley **center**, not spawn yaw
- Boss: 11m forward + 1.8m right; Rival: 17m forward - 2.4m left
- Check `bounds` are correct (debug JSON: `window._oyaApp.bootDebugJson()`)

---

## Art Direction

### Style: Early 90s Arcade / Cyberpunk Tokyo

**References**: Streets of Rage, Yakuza PS2, Shenmue, Snatcher

**Color palette**:
- Walls: Dark browns, grays, weathered concrete
- Neon: Pink, cyan, gold, crimson (high saturation)
- Ground: Wet dark asphalt with puddle reflections
- Fog: Purple haze at distance
- Ambient: Warm orange overhead, cool teal ground bounce

**Geometry guidelines**:
- Shop fronts should have **depth** (recessed doorways, awnings jutting out)
- Signs should be **3D objects** (blade signs, hanging lanterns, noren curtains)
- Ground clutter: vending machines, bikes, crates, trash bags
- Overhead: cables, hanging signs, AC units, fire escapes
- Keep walkable path ~5-6m wide (player needs 3m clear minimum)

**8 shop types** (from design brief):
1. Ramen shop (steam, noren, warm light)
2. Pachinko parlor (loud neon, glass front)
3. Yakuza office (understated, heavy door)
4. Konbini (bright fluorescent, glass walls)
5. Tattoo parlor (red/black, tiger art)
6. Izakaya (paper lanterns, wood frame)
7. Shuttered/closed shop (metal shutters)
8. Arcade (pixel art signs, blue glow)

---

## Performance Budgets

| Metric | Budget | Current |
|--------|--------|---------|
| Triangles | < 100,000 | ~28,700 |
| Mesh objects | — | 1,002 |
| Textures | < 100 | 92 |
| GLB file | < 5 MB | 3.6 MB |
| WASM binary | < 8 MB | ~4.6 MB |
