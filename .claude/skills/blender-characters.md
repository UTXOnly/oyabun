# Blender Character Modeling Skill

## Current Pipeline: 3D Skin Modifier Models

Characters are built in Blender using Python scripting with the **skin modifier** technique. The old PixelLab sprite/billboard pipeline is deprecated.

### Pipeline

```
Skin modifier skeleton (joints + edges + radii)
    → Subdivision level 2 (smooth organic shape)
    → Decimate to ~30% (~1000 faces)
    → Assign materials by face center position
    → Add detail meshes (glasses, weapons, neon, tie, etc.)
    → Join all into single mesh
    → Export GLB with export_yup=True
    → client/characters/oyabaun_player.glb (boss)
    → client/characters/oyabaun_rival.glb (rival)
```

### Conventions

- Blender Z-up, character front faces -Y
- Feet at Z=0
- glTF export flips to Y-up (export_yup=True)
- `character_model()` in lib.rs adds PI to yaw for the Blender→game facing conversion
- Materials: Principled BSDF with base color (no textures). Emissive for neon glow.

### Current Characters

**Boss** (oyabaun_player.glb):
- Dark suit, broad shoulders, slicked hair
- Sunglasses, red tie, pistol in right hand
- Cyan neon accents (lapels, pocket, belt, cuffs)
- ~1100 verts, 11 materials

**Rival** (oyabaun_rival.glb):
- White/cream suit, lean athletic build, bleached spiky hair
- Purple glasses, facial scar, katana in left hand
- Purple neon accents (lapels, collar, belt, katana edge)
- ~1186 verts, 11 materials

### Shader

`SHADER_CHAR_TEX` in `render.rs`:
- Standard 3D model transform (NOT billboard)
- Material tint from GLB material base color
- Directional lighting + cyberpunk ambient
- Hit flash (anim_frame > 100 encodes flash intensity)
- Distance fog

### Key Files

| File | Purpose |
|------|---------|
| `client/src/render.rs` | Character shader, pipeline, uniform struct |
| `client/src/lib.rs` | character_model(), make_character(), NPC render loop |
| `client/src/gltf_level.rs` | parse_character_glb() — loads GLB |
| `client/src/npc.rs` | NPC AI, hitboxes, wave spawning |
| `client/characters/*.glb` | Character model files |

### DEPRECATED — Do NOT Use

- PixelLab MCP tools for character sprites
- Billboard/atlas shaders
- `tools/blender_make_oyabaun_character.py` (old sprite→quad pipeline)
- Atlas UV selection, ATLAS_ROWS, direction indices
