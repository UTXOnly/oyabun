# Blender Character Modeling Skill

## Current Pipeline: 3D Skin Modifier Models

Characters are built in Blender using Python scripting with the **skin modifier** technique. The old PixelLab sprite/billboard pipeline is deprecated.

### Pipeline

```
tools/blender_build_oyabaun_characters_3d.py (OYABAUN_VARIANT=boss|rival|all)
Skin modifier skeleton (joints + edges + radii)
    → Subdivision level 1
    → Decimate (default ~46% faces kept; OYABAUN_CHAR_DECIMATE)
    → Assign materials by face center position
    → Packed 32–96px arcade albedos (vertical suit folds + hash noise, not chessboard)
    → Detail meshes (glasses, weapons, neon, tie, hair, lapels, …)
    → Join → smart UV → export GLB export_yup=True
    → client/characters/oyabaun_player.glb | oyabaun_rival.glb
```

### Style audit (reference vs render)

**No Pillow required** — stdlib PNG reader.

```bash
# Renders boss GLB with Blender, compares to ref, writes report:
python3 tools/character_style_audit.py --ref example_images/ref-image.png

# Your in-game screenshot as candidate:
python3 tools/character_style_audit.py --ref example_images/ref-pixel-yakuza.png \
  --candidate ~/Desktop/oyabaun_ingame.png

# Re-run every 30s while iterating (12 passes):
python3 tools/character_style_audit.py --ref example_images/ref-image.png --loop 12
```

Outputs **`tools/_audit_out/LAST_AUDIT_REPORT.txt`** with concrete next steps (checkerboard, saturation, palette, warmth).  
Headless render: **`tools/blender_character_capture.py`** (invoked by the audit).

Set **`OYABAUN_BLENDER`** if Blender is not at `/Applications/Blender.app/...` or on `PATH`.

### Conventions

- Blender Z-up, character front faces -Y
- Feet at Z=0
- glTF export flips to Y-up (export_yup=True)
- `character_model()` in lib.rs adds PI to yaw for the Blender→game facing conversion
- Materials: Principled + **packed pixel albedos** (nearest); solids + emission for metal/glass/neon

### Current Characters

**Boss** (oyabaun_player.glb): dark suit, broad shoulders, slicked hair, sunglasses, red tie, pistol, cyan neon.  
**Rival** (oyabaun_rival.glb): white suit, lean build, spiky blonde hair, purple glasses, katana, purple neon.

### Shader

`SHADER_CHAR_TEX` in `client/src/render.rs`: model transform, nearest albedo, cel + dithered palette, magenta rim, fog. Tuned so texture detail is not crushed by stipple.

### Key Files

| File | Purpose |
|------|---------|
| `client/src/render.rs` | Character shader, pipeline |
| `client/src/lib.rs` | character_model(), make_character() |
| `client/src/gltf_level.rs` | parse_character_glb() |
| `tools/blender_build_oyabaun_characters_3d.py` | Canonical regenerator |
| `tools/character_style_audit.py` | Ref vs render metrics + instructions |
| `tools/blender_character_capture.py` | Headless EEVEE capture for audit |

### DEPRECATED — Do NOT Use

- PixelLab MCP for character sprites
- Billboard/atlas shaders in production
- Stacked box “procedural” characters
- Atlas UV selection for 3D bodies
