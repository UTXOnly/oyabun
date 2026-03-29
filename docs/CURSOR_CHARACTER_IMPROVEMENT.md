# Character Improvement Task Doc

## Status: IN PROGRESS — generator: `tools/blender_build_oyabaun_characters_3d.py`; refs: `example_images/` + `docs/CHARACTER_PIPELINE_HANDOFF.md`

## Goal

The current 3D characters are smooth humanoid shapes (via Blender skin modifier) but lack detail. They need to look like stylized cyberpunk yakuza gangsters — menacing, detailed, with visible clothing features and weapons.

**Reference style**: Cyberpunk yakuza — dark alley setting, neon accents, suits with visible lapels/collars, sunglasses, weapons (pistols, katanas), slicked/spiky hair, facial scars. Think Yakuza game series meets cyberpunk aesthetic.

## Current Architecture

Characters are **3D meshes** (NOT billboard sprites — the old PixelLab pixel art pipeline is deprecated).

### Pipeline

```
Blender Python script (skin modifier + detail meshes)
    → join all parts into single mesh
    → export as GLB (glTF binary)
    → client/characters/oyabaun_player.glb (boss)
    → client/characters/oyabaun_rival.glb (rival)
    → include_bytes!() in Rust WASM build
    → parse_character_glb() loads verts + indices + material batches
    → SHADER_CHAR_TEX renders with standard model*position transform
    → per-material tint colors (no textures, just colored materials)
    → directional lighting + hit flash + distance fog
```

### Shader (render.rs — SHADER_CHAR_TEX)

- Standard 3D vertex transform: `world_pos = model * vec4(v.pos, 1.0)`
- Fragment: material tint * directional light + ambient + hit flash + fog
- Depth write ON, alpha = 1.0 (solid models)
- No billboard, no atlas UV selection

### Character Model Matrix

```rust
fn character_model(foot: Vec3, yaw: f32, scale: f32) -> Mat4 {
    // +PI offset because Blender -Y front → glTF +Z, game yaw 0 = -Z
    Mat4::from_scale_rotation_translation(
        Vec3::splat(scale),
        Quat::from_rotation_y(yaw + PI),
        foot,
    )
}
```

### GLB Format

- Vertex: `CharacterVertex { pos, uv, nrm }` — 32 bytes (normals from glTF or computed)
- Materials: Principled BSDF with base color (no image textures)
- Emissive materials used for neon glow effects
- Feet at Y=0 in glTF (Z=0 in Blender, exported with `export_yup=True`)
- Front of character faces -Y in Blender → +Z in glTF

### Key Files

| File | Purpose |
|------|---------|
| `client/src/render.rs` | SHADER_CHAR_TEX, CharUniforms, raster_character_gltf pipeline |
| `client/src/lib.rs` | character_model(), make_character(), NPC rendering loop |
| `client/src/gltf_level.rs` | parse_character_glb() — loads GLB vertices/indices/materials |
| `client/src/npc.rs` | NPC AI, hitboxes, wave spawning, smooth_turn |
| `client/characters/oyabaun_player.glb` | Boss character model |
| `client/characters/oyabaun_rival.glb` | Rival character model |
| `tools/blender_build_oyabaun_characters_3d.py` | Skin-modifier boss/rival (`OYABAUN_VARIANT`, optional `OYABAUN_CHAR_DECIMATE`) |
| `tools/blender_make_oyabaun_character.py` | Stub / legacy sprite quad only (`OYABAUN_LEGACY_SPRITE=1`) |
| `docs/CHARACTER_PIPELINE_HANDOFF.md` | Pipeline answers, material tables, `example_images` list |

## What Needs Improvement

### Priority 1: Character Detail (Body)

The skin modifier creates smooth organic shapes but they're featureless mannequins. Need:

- [ ] **Facial features**: Nose (wedge/pyramid), ears, eyebrow ridges, jaw definition, chin
- [ ] **Muscular definition**: Pectoral separation on chest, deltoid bumps on shoulders, forearm taper
- [ ] **Hands**: At minimum thumb separation, ideally 3-4 finger groupings
- [ ] **Feet/Shoes**: Sole separation, heel detail, pointed or rounded toe shape

### Priority 2: Clothing Detail

- [ ] **Suit jacket**: Raised collar geometry, visible lapel V-panels, shoulder pads, breast pocket, jacket hem at waist, button dots, sleeve cuffs
- [ ] **Shirt**: Visible underneath jacket at V-neck, collar points
- [ ] **Pants**: Belt with buckle, slight crease definition, break at ankle
- [ ] **Tie** (boss): Proper tapered shape with knot

### Priority 3: Character-Specific Features

**Boss** (dark suit, menacing):
- [ ] Slicked-back hair with volume and side-part definition
- [ ] Sunglasses with wrap-around shape
- [ ] Cigarette in mouth
- [ ] Pistol in right hand (currently basic box shape — needs trigger, slide detail)
- [ ] Signet ring or hand tattoo
- [ ] Broader shoulders, more imposing stance

**Rival** (white suit, agile):
- [ ] Spiky bleached hair (5+ distinct spikes with height variation)
- [ ] Facial scar more prominent (longer, slightly raised)
- [ ] Purple-tinted glasses
- [ ] Katana: proper wrapped handle segments, decorative guard, curved blade with taper
- [ ] Leaner, more athletic build
- [ ] Open collar showing chain/necklace

### Priority 4: Neon Accents

Both characters should have glowing cyberpunk accents:
- [ ] Neon piping on suit edges (lapels, pockets, cuffs)
- [ ] Emissive belt buckle or jewelry
- [ ] Boss: cyan/teal glow accents
- [ ] Rival: purple/magenta glow accents
- [ ] Weapon glow effects (gun muzzle, katana edge)

## Technical Constraints

- **Vertex budget**: ~1000-2000 verts per character (current: ~1100). Can go up to 3000 if needed.
- **Material limit**: Keep under 15 materials per character (each creates a draw batch)
- **No image textures**: All coloring via material base color / emission. The GLB parser handles untextured materials.
- **Smooth shading**: Use `shade_smooth()` on organic parts, flat shading on hard-surface accessories
- **Blender coordinate system**: Z-up, front = -Y. Export with `export_yup=True` flips to Y-up for glTF.

## Approach: Skin Modifier + Detail Meshes

The current approach (and recommended path forward):

1. **Base body**: Skin modifier from skeleton joints → subdivision → decimate (~1000 faces)
2. **Material assignment**: By face center Z/X position (height determines suit vs skin vs hair vs shoes)
3. **Detail meshes**: Separate objects for accessories (glasses, gun, tie, neon strips, etc.)
4. **Join all**: `bpy.ops.object.join()` merges everything into one mesh with multiple material slots
5. **Export**: Single GLB per character type

### Skin Modifier Skeleton

Define joints as `{name: (x, y, z)}` dict, connect with edges, set radii per joint:

```python
# Example radii for broad-shouldered boss
radii = {
    'pelvis':      (0.14, 0.10),  # wide hips
    'chest':       (0.16, 0.12),  # broad chest
    'upper_chest': (0.18, 0.12),  # wide shoulders
    'neck':        (0.06, 0.06),  # thin neck
    'head':        (0.10, 0.11),  # round head
    # ... etc
}
```

## How to Test

```bash
# Rebuild WASM after GLB changes
cd client && wasm-pack build --target web --no-typescript

# Serve locally
python3 -m http.server 8080 --directory client

# Hard refresh browser (Cmd+Shift+R) — static server caches aggressively
```

## DO NOT

- Use PixelLab or any sprite/billboard approach — that pipeline is deprecated
- Use atlas UV selection in the shader — characters are solid 3D models
- Modify the Tokyo alley level (`client/levels/tokyo_alley.glb`) — it's complete
- Change the shader uniform struct layout (CharUniforms) without updating Rust side
- Forget to run `wasm-pack build` after changing GLB files (they're embedded via include_bytes!)
