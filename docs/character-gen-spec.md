# Oyabaun Character Generation Spec

## Overview

Characters in Oyabaun use **8-direction pixel-art sprites** rendered on billboard
quads via the `SHADER_CHAR_TEX` pipeline. As the camera orbits a character, the
shader selects the correct directional view from a sprite atlas — creating a
pseudo-3D effect like classic games (Doom, Diablo, Final Fantasy Tactics).

All entities (boss, rival, remote players, offline demos) instance the same
`oyabaun_player.glb` mesh with per-instance model matrices.

## Pipeline Summary

```
PixelLab API  -->  8 × 64x64 RGBA PNGs (S, SE, E, NE, N, NW, W, SW)
                      |
              crop to character bbox (remove padding)
              assemble into 512×49 atlas (8 columns, native res)
                      |
              save as client/sprite1.png
                      |
    Blender script (tools/blender_make_oyabaun_character.py)
              builds quad mesh (aspect-matched to one atlas cell)
              embeds atlas texture, UVs span 0-1
                      |
              exports client/characters/oyabaun_player.glb
                      |
              include_bytes!() in Rust WASM build
                      |
              parse_character_glb() --> raster_character_gltf()
                      |
              SHADER_CHAR_TEX (vs_char / fs_char)
              BlendState::ALPHA_BLENDING, discard alpha < 0.35
              fs_char computes camera→character angle
              selects 1/8th atlas column (direction sprite)
```

## 8-Direction Sprite System

### Atlas Layout

`sprite1.png` is a horizontal strip: **8 columns × 1 row**, native resolution
(512×49 for 64px source sprites cropped to 49px character height).

| Column | Index | Direction  | Camera Position        |
|--------|-------|------------|------------------------|
| 0      | 0     | South      | Camera south of char   |
| 1      | 1     | South-East | Camera SE of char      |
| 2      | 2     | East       | Camera east of char    |
| 3      | 3     | North-East | Camera NE of char      |
| 4      | 4     | North      | Camera behind char     |
| 5      | 5     | North-West | Camera NW of char      |
| 6      | 6     | West       | Camera west of char    |
| 7      | 7     | South-West | Camera SW of char      |

### Direction Selection (Shader)

The fragment shader computes which column to sample:

```wgsl
// Camera→character angle in XZ plane
let dx = cam.x - char.x;
let dz = cam.z - char.z;
var angle = atan2(dx, dz);  // 0 = south (+Z)
if (angle < 0.0) { angle += 2π; }

// Quantize to 8 directions (45° each), half-step offset for boundaries
// +4 offset: PixelLab directions are character-facing, not camera-facing
// so camera-north should show south (front) sprite
let dir_idx = (u32((angle + π/8) / (π/4)) + 4) % 8;

// Sample 1/8th column
let atlas_u = (uv.x + f32(dir_idx)) * 0.125;
let t = textureSample(albedo, sampler, vec2(atlas_u, uv.y));
```

### Quad Geometry

The Blender export creates a vertical quad matching one atlas cell's aspect ratio:
- Height: 1.68m (character + head padding)
- Width: 1.68 × (64/49) ≈ 2.19m (full cell width — character is ~30% of this)
- Transparent pixels handle visual trimming; quad is wide to accommodate all directions
- Front + back face (0.04m offset) for rear visibility
- Cull mode: None (double-sided)

### Per-Instance Uniforms

`CharUniforms._char_params`:
- `.x` = facing yaw (reserved for future NPC heading)
- `.yz` = character world XZ position (used for angle computation)

## PixelLab Character Generation

### API: `create_character`

| Parameter      | Boss Value                            | Rival Value                           |
|----------------|---------------------------------------|---------------------------------------|
| `description`  | Japanese yakuza crime boss, older man with slicked back black hair, sharp features, traditional dark suit with white dress shirt, no tie, gold pin on lapel, stern intimidating expression, scarred face, Japanese gangster lord | Young Japanese yakuza enforcer, short spiky black hair, lean muscular build, black leather jacket over red shirt, dark pants, combat boots, cocky smirk, street fighter punk, Tokyo thug |
| `name`         | Oyabaun Boss                          | Yakuza Rival                          |
| `mode`         | standard                              | standard                              |
| `size`         | 64                                    | 64                                    |
| `n_directions` | 8                                     | 8                                     |
| `view`         | low top-down                          | low top-down                          |
| `outline`      | single color black outline            | single color black outline            |
| `shading`      | detailed shading                      | detailed shading                      |
| `detail`       | high detail                           | high detail                           |

- **Output**: 64×64 RGBA PNGs with transparent backgrounds, 8 directional views
- **Processing time**: ~3-5 minutes
- **Cost**: 1 generation per character (standard mode)

### Character IDs

**Current generation (v2 — Japanese yakuza themed):**
- Oyabaun Boss: `6d169ab6-bb02-4ef2-bf1e-6bec41553472` — Japanese crime lord, dark suit, scarred face
- Yakuza Rival: `213e25e0-9c7a-4d71-a37f-cd199a4f9855` — Young enforcer, leather jacket, spiky hair
- Player Ronin: `ea4cdb4d-00bb-4f77-853d-843061b465f2` — Street ronin, hoodie + katana, face mask

**Previous generation (v1 — deprecated):**
- Boss: `572836f2-a19f-41b5-bee5-46998f43b019`
- Rival: `afd7b081-5b53-49bf-8f00-ecbd5e65f1c2`

### Animations

Queue via `animate_character` after character generation completes.

| Animation              | template_animation_id         | Cost          |
|------------------------|-------------------------------|---------------|
| Walk                   | `walk`                        | 1 gen/dir     |
| Fight idle             | `fight-stance-idle-8-frames`  | 1 gen/dir     |
| Run                    | `running-8-frames`            | 1 gen/dir     |
| Punch                  | `cross-punch`                 | 1 gen/dir     |
| Kick                   | `high-kick`                   | 1 gen/dir     |
| Death                  | `falling-back-death`          | 1 gen/dir     |
| Fireball               | `fireball`                    | 1 gen/dir     |

**Job slot limit**: 8 concurrent jobs. Each 8-direction animation uses all 8 slots.
Queue one animation at a time, wait for completion, then queue the next.

## Sprite Processing

### From PixelLab to game-ready atlas

```python
from PIL import Image
import numpy as np

dirs = ['south','south-east','east','north-east',
        'north','north-west','west','south-west']

# 1. Find unified crop bounds across all directions
all_rmin, all_rmax = 63, 0
for d in dirs:
    img = Image.open(f'{char}_{d}.png').convert('RGBA')
    alpha = np.array(img)[:,:,3]
    rows = np.where(np.any(alpha > 0, axis=1))[0]
    all_rmin = min(all_rmin, rows[0])
    all_rmax = max(all_rmax, rows[-1])

# 2. Build native-res atlas (no upscale — GPU NEAREST handles it)
crop_h = all_rmax - all_rmin + 1
atlas = Image.new('RGBA', (64 * 8, crop_h), (0,0,0,0))
for i, d in enumerate(dirs):
    sprite = Image.open(f'{char}_{d}.png').convert('RGBA')
    cropped = sprite.crop((0, all_rmin, 64, all_rmax + 1))
    atlas.paste(cropped, (i * 64, 0))
atlas.save('client/sprite1.png')
```

Key points:
- PixelLab sprites have transparent backgrounds — no chromakey needed
- Crop vertically to remove padding — puts feet at the canvas bottom
- Native resolution (no upscale) — GPU NEAREST sampler magnifies at render time
- Atlas is tiny: 512×49 ≈ 50KB compressed

### From atlas PNG to GLB

```bash
/Applications/Blender.app/Contents/MacOS/Blender \
  --background --python tools/blender_make_oyabaun_character.py
```

The script reads `client/sprite1.png` (atlas) and writes `client/characters/oyabaun_player.glb`.

## Rendering Pipeline (Rust/wgpu)

### Pipeline State

| Setting         | Value                |
|-----------------|----------------------|
| Blend           | `ALPHA_BLENDING`     |
| Depth write     | `false`              |
| Depth compare   | `Less`               |
| Cull mode       | `None` (double-sided)|
| Texture format  | `Rgba8UnormSrgb`     |
| Mag/Min filter  | `Nearest`            |

### Instancing

Characters are drawn with dynamic uniform buffer offsets:
- `CharUniforms` struct per instance (view_proj, model, cam_pos, fog, char_params)
- `character_models: Vec<Mat4>` built per frame
- Each entity gets `character_model(foot_pos, yaw, scale)`
- Character XZ world position extracted from model matrix translation column

### Billboard Facing

The model matrix rotates the quad to always face the camera:
```rust
fn character_model(foot: Vec3, yaw: f32, scale: f32) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::splat(scale),
        Quat::from_rotation_y(yaw),  // yaw faces camera
        foot,
    )
}
```
The quad geometry faces the camera; the shader selects which directional sprite
to show based on the camera angle. This creates the Doom/Diablo pseudo-3D effect.

## File Layout

```
client/
  sprite1.png                          # 8-dir atlas (512×49 RGBA)
  characters/
    oyabaun_player.glb                 # Character card mesh + embedded atlas
    boss_south.png ... boss_south-west.png   # 8 individual direction PNGs
    rival_south.png ... rival_south-west.png
    boss_atlas.png                     # Upscaled atlas (2048×200) for preview
    rival_atlas_native.png             # Rival native atlas (512×49)
  src/
    render.rs                          # SHADER_CHAR_TEX, raster_character_gltf, draw
    gltf_level.rs                      # parse_character_glb
    lib.rs                             # character_models Vec<Mat4> per frame
    npc.rs (`Npc`, `NpcDef`)            # Entity state (HP, scale, position)
tools/
  blender_make_oyabaun_character.py    # Atlas quad mesh builder + GLB exporter
docs/
  character-gen-spec.md                # This file
```

## Regenerating Characters

1. Call `create_character` with new description via PixelLab MCP
2. Wait ~3-5 min, check with `get_character`
3. Download all 8 direction PNGs:
   ```bash
   for dir in south south-east east north-east north north-west west south-west; do
     curl --fail -o "client/characters/boss_${dir}.png" "<rotation_url>"
   done
   ```
4. Run the atlas build script (crop + assemble)
5. Rebuild GLB: `Blender --background --python tools/blender_make_oyabaun_character.py`
6. Rebuild WASM: `cd client && wasm-pack build --target web --out-dir pkg`
7. Hard-refresh browser (Cmd+Shift+R)

## Future: Multi-Character Atlases

Currently all entities share one atlas. To support distinct boss vs rival sprites:

**Option A — Runtime texture swap** (recommended):
Keep one GLB mesh. Upload boss_atlas and rival_atlas as separate textures.
Swap material bind groups between boss/rival draw calls. Minimal code change.

**Option B — Combined atlas**:
Stack boss + rival vertically (512×98). Use `char_params.x` to encode
which character row to sample. One texture, one GLB, shader selects row.

## Critical Gotchas

- **No dpdx/dpdy**: WGSL builtins cause SILENT pipeline failure on some WebGPU impls
- **glTF Y-up**: Blender exports with `export_yup=True`
- **Alpha must flow**: RGBA PNG → Blender Alpha → glTF alpha → Rgba8UnormSrgb → ALPHA_BLENDING → discard
- **Depth write off**: Alpha blending + depth writes = transparent pixels block geometry behind
- **NEAREST sampling**: Pixel art must use nearest-neighbor, not linear
- **Atlas column order**: S, SE, E, NE, N, NW, W, SW — matches clockwise from south
- **Foot cropping**: Crop atlas rows to put feet at canvas bottom — prevents floating
- **include_bytes!**: GLB is embedded at compile time. Must rebuild WASM after changing GLB
- **Cache busting**: Static server has no cache headers. Hard-refresh or `?v=Date.now()`
