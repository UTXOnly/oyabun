# Skill: Oyabaun Character Generation

Use this skill when creating, modifying, or adding new characters to the Oyabaun game.
Characters are 8-direction pixel-art sprites rendered on billboard quads with a direction-selecting shader.

---

## End-to-end workflow

### Step 1: Generate character with PixelLab MCP

```
create_character(
  description="<character appearance description>",
  name="<character name>",
  mode="standard",
  size=64,
  n_directions=8,
  view="low top-down",
  outline="single color black outline",
  shading="detailed shading",
  detail="high detail"
)
```

**Theme**: All characters should be **Japanese yakuza / neo-Tokyo** themed. This is a dark retro FPS set in Tokyo alleys.

**Tips for good descriptions**:
- Be specific about clothing, hair, and accessories
- Mention skin tone, build, expression
- Include thematic keywords: yakuza, Tokyo, cyberpunk, street fighter, etc.
- Keep descriptions under 200 chars for best results

Wait 3-5 minutes for generation. Check with `get_character(character_id="...")`.

### Step 2: Queue animations

After character generation completes, queue walk animation:

```
animate_character(
  character_id="<id>",
  template_animation_id="walk",
  n_directions=8,
  view="low top-down"
)
```

**Important**: PixelLab has 8 concurrent job slots. Each 8-direction animation uses all 8 slots. Queue ONE animation at a time, wait for completion, then queue the next.

Available animations:
| Template ID                  | Use case         |
|------------------------------|------------------|
| `walk`                       | Walk cycle       |
| `running-8-frames`           | Run cycle        |
| `fight-stance-idle-8-frames` | Combat idle      |
| `cross-punch`                | Melee attack     |
| `high-kick`                  | Kick attack      |
| `falling-back-death`         | Death animation  |

### Step 3: Download direction PNGs

After generation completes, `get_character` returns URLs for each direction.
Download all 8 PNGs to `client/characters/`:

```
{name}_south.png
{name}_south-east.png
{name}_east.png
{name}_north-east.png
{name}_north.png
{name}_north-west.png
{name}_west.png
{name}_south-west.png
```

### Step 4: Build atlas PNG

Run the atlas builder (Python with Pillow + NumPy):

```python
from PIL import Image
import numpy as np

name = "boss"  # character prefix
dirs = ['south','south-east','east','north-east',
        'north','north-west','west','south-west']

# Find unified crop bounds across all 8 directions
all_rmin, all_rmax = 63, 0
for d in dirs:
    img = Image.open(f'client/characters/{name}_{d}.png').convert('RGBA')
    alpha = np.array(img)[:,:,3]
    rows = np.where(np.any(alpha > 0, axis=1))[0]
    if len(rows) > 0:
        all_rmin = min(all_rmin, rows[0])
        all_rmax = max(all_rmax, rows[-1])

# Assemble atlas at native resolution (no upscale!)
crop_h = all_rmax - all_rmin + 1
atlas = Image.new('RGBA', (64 * 8, crop_h), (0, 0, 0, 0))
for i, d in enumerate(dirs):
    sprite = Image.open(f'client/characters/{name}_{d}.png').convert('RGBA')
    cropped = sprite.crop((0, all_rmin, 64, all_rmax + 1))
    atlas.paste(cropped, (i * 64, 0))

atlas.save('client/sprite1.png')
print(f"Atlas: {atlas.size[0]}x{atlas.size[1]}")
```

**Critical**: Do NOT upscale. The GPU's NEAREST sampler handles magnification. Native resolution preserves pixel art crispness.

### Step 5: Build GLB mesh

```bash
/Applications/Blender.app/Contents/MacOS/Blender \
  --background --python tools/blender_make_oyabaun_character.py
```

This reads `client/sprite1.png` and writes `client/characters/oyabaun_player.glb`.
The script creates a vertical quad with aspect ratio matching one atlas cell, embeds the atlas texture, and sets up alpha materials.

### Step 6: Rebuild WASM

```bash
cd client && wasm-pack build --target web --out-dir pkg
```

The GLB is embedded via `include_bytes!()` at compile time. **You must rebuild after any GLB change.**

### Step 7: Verify

Hard-refresh browser (Cmd+Shift+R) or use incognito window. The static server caches aggressively and has no cache-control headers.

---

## Architecture reference

### Rendering pipeline

```
NpcManager (npc.rs)
  └─ Vec<Npc> with NpcDef (label, max_hp, scale, hitbox)
      └─ foot: Vec3, hp: f32

render_frame (lib.rs)
  └─ for each alive NPC:
      yaw = yaw_face_cam_xz(foot, cam)   // billboard faces camera
      character_models.push(character_model(foot, yaw, scale))

draw_world (render.rs)
  └─ CharUniforms per instance (view_proj, model, cam_pos, char_params)
  └─ char_params.yz = character world XZ (for shader direction calc)
  └─ SHADER_CHAR_TEX:
      angle = atan2(cam.x - char.x, cam.z - char.z)
      dir_idx = (quantized_angle + 4) % 8   // +4 offset: front-facing
      atlas_u = (uv.x + dir_idx) * 0.125    // select 1/8th column
```

### File layout

```
client/
  sprite1.png                                # Active atlas (512×~49 RGBA)
  characters/
    oyabaun_player.glb                       # Character card mesh + texture
    {name}_{direction}.png                   # Individual PixelLab outputs
  src/
    npc.rs                                   # Npc, NpcDef, NpcManager structs
    render.rs                                # SHADER_CHAR_TEX, CharUniforms
    lib.rs                                   # character_models, yaw_face_cam_xz
tools/
  blender_make_oyabaun_character.py          # Atlas → GLB builder
```

### NPC definitions (npc.rs)

To add a new NPC type, create an `NpcDef`:

```rust
pub const MINION_DEF: NpcDef = NpcDef {
    label: "minion",
    max_hp: 60.0,
    scale: 0.90,
    hitbox_pad: 0.55,
    hitbox_height: 2.20,
};
```

Then add it to `NpcManager::new()` or add a method to spawn dynamically.

### Weapon HUD

Weapons use static 512x512 RGBA PNGs in `client/fpsweapons/`. Animation is procedural:
- **Bob**: sine/cosine position offset during movement
- **Recoil**: upward kick on fire (decays 8.0/s)
- **Reload**: weapon drops below screen and raises back (2.5 speed, ~0.8s cycle)

---

## Multi-character support (TODO)

Currently all entities share one atlas/GLB. To support distinct sprites:

**Option A — Runtime texture swap** (recommended):
- Keep one GLB mesh geometry
- Upload separate atlas textures per character type
- Swap bind groups between draw calls
- Requires changes to `render.rs` character draw loop

**Option B — Combined atlas**:
- Stack multiple character atlases vertically (e.g., 512×98 for two)
- Use `char_params.x` to encode character type (row offset in shader)
- One texture, one GLB, shader selects correct row

---

## Gotchas

1. **No dpdx/dpdy** in WGSL — causes silent pipeline failure on some WebGPU
2. **glTF Y-up**: Blender exports with `export_yup=True`
3. **Alpha chain**: PNG RGBA → Blender Alpha → glTF → Rgba8UnormSrgb → ALPHA_BLENDING → `discard` if alpha < 0.35
4. **Depth write off**: Required with alpha blending or transparent pixels block geometry
5. **NEAREST filter**: Pixel art must use nearest-neighbor, never linear
6. **Foot crop**: Always crop atlas vertically to put feet at canvas bottom
7. **Cache busting**: Add `?v=timestamp` to fetch URLs. Static server has no cache headers
8. **Direction offset**: Shader uses `+4u` to flip atlas selection so characters face toward camera
