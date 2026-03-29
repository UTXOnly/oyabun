# Character art pipeline — handoff for Claude (or whoever regenerates meshes)

## What went wrong (March 2026)

The **canonical** boss/rival meshes are **organic skin-modifier humanoids** (~1.7k verts, 11 materials), committed as binary GLBs in:

- `client/characters/oyabaun_player.glb`
- `client/characters/oyabaun_rival.glb`

The **regenerator script is now** `tools/blender_build_oyabaun_characters_3d.py` (see §1 below). Earlier history: only `.glb` binaries were committed for a while (see git `4437bd8`, `54b228e`; co-authored Claude Opus 4.6).

A later change replaced the tool script with **stacked axis-aligned boxes** and overwrote the GLBs, which reads as "block people" and does not match the intended yakuza / `example_images` look.

**Repo state fix:** GLBs were restored from git commit **`54b228e`** ("Add detailed facial features…"). `tools/blender_make_oyabaun_character.py` no longer overwrites them by default.

---

## Answers to all questions (from Claude, the original author)

### 1. Full Blender Python generator

**Recovered and committed as `tools/blender_build_oyabaun_characters_3d.py`.**

This is a single script with a `OYABAUN_VARIANT` env var (`boss`, `rival`, or `all`). It reproduces the full pipeline:

```bash
# Build both characters
Blender --background --python tools/blender_build_oyabaun_characters_3d.py

# Build one
OYABAUN_VARIANT=boss Blender --background --python tools/blender_build_oyabaun_characters_3d.py
OYABAUN_VARIANT=rival Blender --background --python tools/blender_build_oyabaun_characters_3d.py
```

The script contains:
- `build_skin_body()` — shared skeleton builder (joints → skin modifier → subsurf level 2 → decimate 0.30)
- `build_boss()` — boss-specific joints, radii, materials, detail meshes
- `build_rival()` — rival-specific joints, radii, materials, detail meshes
- All detail mesh geometry (glasses, weapons, neon, facial features, suit details)

The original meshes were built interactively via **Blender MCP** (`execute_blender_code` tool) in ~12 sequential calls per character, then refined by 4 parallel enhancement agents. The committed script unifies all of that into a single reproducible file.

### 2. Reference art

Reference images **are** in this repo under `example_images/`:

| File | What it locks |
|------|---------------|
| `example_images/ref-image.png` | **Silhouette + mood**: Side profile of suited man with sunglasses, cigarette, slicked hair. Dark suit with neon-lit alley backdrop. Red/amber color palette. Pixel-art style but the *proportions* and *details* are the target. |
| `example_images/sokes1.png` | **Face + attitude**: Front-facing yakuza with sunglasses, cigarette, neck tattoos, short dark hair, menacing expression. Neon "LOADING" sign behind. Cool blue palette. |
| `example_images/soke1.mp4`, `soke2.mp4`, `soke3.mp4` | Video references for animation style (not yet used for character gen). |

**Key style elements locked by these references:**
- Sunglasses (wrap-around or angular, reflective)
- Cigarette in mouth (with visible smoke ideally)
- Dark suit with visible lapels, collar, tie
- Slicked or short dark hair
- Menacing, confident posture
- Neon-lit cyberpunk alley atmosphere
- Pixel-art / stylized low-poly aesthetic (not photorealistic)

### 3. Exact Blender version

**Blender 5.1** — the version connected via Blender MCP when the GLBs were generated.

glTF export settings used:
```python
bpy.ops.export_scene.gltf(
    filepath=out_path,
    export_format='GLB',
    export_materials='EXPORT',
    export_texcoords=True,
    export_normals=True,
    export_apply=True,
    export_yup=True,           # Critical: Blender Z-up → glTF Y-up
    use_selection=True,
    export_animations=False,
)
```

No other non-default settings. No custom exporter plugins.

### 4. Regeneration commands

The original generation was done via **Blender MCP** (not command line). The equivalent command line is:

```bash
# Ensure Blender 5.1+ is in PATH
Blender --background --python tools/blender_build_oyabaun_characters_3d.py
```

No env vars needed beyond optional `OYABAUN_VARIANT`. No manual steps — the script is fully automated.

**Original process** (for historical context):
1. Scene clear via MCP
2. Boss skeleton creation (31 joints, 28 edges, per-joint radii)
3. Apply skin + subdivision modifiers
4. Decimate to 30%
5. Material assignment by face center Z-position
6. Detail meshes (glasses, tie, pistol, neon)
7. Join all → export GLB
8. Repeat steps 2-7 for rival (different joints, radii, details)
9. Four parallel enhancement agents added facial features, suit details, etc.

### 5. Rival vs boss differences

Same 31-joint skeleton topology, different parameters:

| Parameter | Boss | Rival |
|-----------|------|-------|
| Height | ~1.85m (head_top Z=1.82) | ~1.78m (head_top Z=1.78) |
| Build | Broad (upper_chest rx=0.18) | Lean (upper_chest rx=0.16) |
| Pose | Right arm forward (gun) | Left arm forward (katana) |
| Suit color | Near-black (0.04, 0.04, 0.06) | White (0.85, 0.82, 0.78) |
| Hair color | Black (0.02, 0.02, 0.02) | Blonde (0.82, 0.78, 0.65) |
| Hair style | Slicked back (volumetric shell) | Spiky (5+ tapered pyramids) |
| Neon color | Cyan (0.0, 0.8, 1.0) | Purple (0.8, 0.0, 1.0) |
| Weapon | Pistol (right hand) | Katana (left hand) |
| Glasses | Black lenses, gold frames | Purple lenses, silver frames |
| Unique | Cigarette, red tie | Facial scar, collar neon |

### 6. Material names / slot order

**Boss materials (11 slots after join):**

| Slot | Name | Color (RGB) | Notes |
|------|------|-------------|-------|
| 0 | `Boss_Suit` | (0.04, 0.04, 0.06) | Near-black, roughness=0.65 |
| 1 | `Boss_Skin` | (0.72, 0.55, 0.42) | roughness=0.85 |
| 2 | `Boss_Hair` | (0.02, 0.02, 0.02) | Black, roughness=0.35 |
| 3 | `Boss_Shoe` | (0.02, 0.02, 0.02) | metallic=0.3, roughness=0.25 |
| 4 | `Boss_Shirt` | (0.12, 0.01, 0.01) | Dark red, roughness=0.5 |
| 5 | `Boss_Glasses` | (0.01, 0.01, 0.01) | metallic=0.8, roughness=0.1 |
| 6 | `Boss_Frames` | (0.6, 0.5, 0.2) | Gold, metallic=0.9 |
| 7 | `Boss_Gun` | (0.06, 0.06, 0.06) | metallic=0.7, roughness=0.25 |
| 8 | `Boss_GunGrip` | (0.35, 0.28, 0.15) | Brown, metallic=0.3 |
| 9 | `Boss_Neon` | (0.0, 0.8, 1.0) | **Emissive** cyan, strength=3.0 |
| 10 | `Boss_Tie` | (0.5, 0.02, 0.02) | Red, roughness=0.45 |
| (+) | `Boss_Cigarette` | (0.75, 0.72, 0.68) | Grey-white |
| (+) | `Boss_CigaretteTip` | (0.9, 0.4, 0.1) | **Emissive** orange, strength=2.0 |

**Rival materials (11 slots after join):**

| Slot | Name | Color (RGB) | Notes |
|------|------|-------------|-------|
| 0 | `Rival_Suit` | (0.85, 0.82, 0.78) | White/cream, roughness=0.55 |
| 1 | `Rival_Skin` | (0.65, 0.48, 0.35) | roughness=0.85 |
| 2 | `Rival_Hair` | (0.82, 0.78, 0.65) | Blonde, roughness=0.35 |
| 3 | `Rival_Shoe` | (0.80, 0.78, 0.75) | White, metallic=0.2 |
| 4 | `Rival_Shirt` | (0.08, 0.08, 0.10) | Dark, roughness=0.5 |
| 5 | `Rival_Glasses` | (0.25, 0.0, 0.35) | Purple, metallic=0.6 |
| 6 | `Rival_Frames` | (0.7, 0.7, 0.7) | Silver, metallic=0.9 |
| 7 | `Rival_Blade` | (0.85, 0.88, 0.92) | Steel, metallic=0.95 |
| 8 | `Rival_KatanaWrap` | (0.12, 0.0, 0.18) | Dark purple |
| 9 | `Rival_Neon` | (0.8, 0.0, 1.0) | **Emissive** purple, strength=3.0 |
| 10 | `Rival_Scar` | (0.85, 0.35, 0.3) | Pinkish, roughness=0.9 |

---

## Technical constraints (unchanged)

- Game loads glTF with `parse_character_glb` — multiple primitives / materials = multiple draw batches (keep **≤ ~15** materials).
- **No image textures required** today: Principled **base color** (+ **emission** for neon). Shader multiplies sampled tex × tint; untextured prims use a white 1×1 internally.
- **Feet at Z = 0** in Blender; **front faces −Y**; `export_yup=True` for glTF.
- `character_model()` in `client/src/lib.rs` uses **`yaw + π`** for Blender → game facing — do not rotate the mesh inconsistently with that without a coordinated Rust change.

---

## Remaining improvements (character quality)

The current characters are functional but still far from the reference art quality. See `docs/CURSOR_CHARACTER_IMPROVEMENT.md` for the full checklist. Key gaps:

1. **Proportions**: Skin modifier creates smooth but featureless mannequins. Need more pronounced muscular definition, hand detail, shoe detail.
2. **Facial features**: Current nose/ears/brows are minimal boxes. Need more sculpted shapes.
3. **Clothing geometry**: Collar, lapels, shoulder pads are basic boxes. Need proper V-shaped panels, raised geometry.
4. **Hair**: Boss needs volumetric slicked-back hair shell. Rival spikes need more variation.
5. **Weapons**: Pistol and katana need more detail (trigger, slide lines, wrapped handle segments).
6. **Neon**: Accents are thin boxes. Could use more prominent glowing strips.

### Possible higher-quality approaches

- **Import pre-made base meshes** (e.g., KayKit CC0 characters) and reskin for cyberpunk yakuza style
- **Enable Hyper3D/Rodin** in Blender MCP for AI-generated 3D models, then retopologize
- **Manual sculpting** in Blender (proportional edit, sculpt mode) after running the generator script
- **Higher subdivision** before decimate (level 3 instead of 2) for more geometry budget

---

## Maintenance log

- **2026-03-29**: `blender_build_oyabaun_characters_3d.py` — default decimate keep-ratio **0.38** (was 0.30) for smoother skin mesh; **smart UV** on joined mesh before glTF export; boss: hair-back/sides, lapel flaps, pocket, cuff, soles, gun mag + serration, signet ring; rival: three extra hair spikes, katana wrap segment + blade ridge, neck chain box, shoe soles. Boss may export **13** material primitives (still under the ~15 batch budget).

Reference art on disk: **`example_images/`** at repo root (`ref-image.png`, `sokes1.png`, `soke*.mp4`). Commit that folder if collaborators should see the same targets.

## Git references

| Commit     | Notes |
|-----------|--------|
| `4437bd8` | Introduced 3D skin-modifier pipeline + task doc; GLBs replaced sprite quads. |
| `54b228e` | Richer detail (fingers, hair spikes, katana wrap, etc.); **last good GLBs before block replacement**. |
| `12c980f` | Blocky procedural replacement (reverted in working tree for binaries; do not use as art direction). |

---

## Neon: emissive-only is correct

Neon accents should stay **emissive-only** (Principled BSDF with `Emission Color` + `Emission Strength`). The game shader (`SHADER_CHAR_TEX` in `render.rs`) already handles emissive materials via the tint color — the emissive color gets baked into the GLB material's base color by the exporter, so it renders as a bright solid color in-game. No separate unlit pass is needed unless bloom/glow post-processing is added later.

## Runtime lighting (client)

Characters use **`CharacterVertex`** (`pos`, `uv`, **`nrm`**) loaded from glTF normals (or smooth normals computed per primitive if missing). The fragment shader applies **per-vertex** lighting (wrap + stepped “toon” bands + rim toward camera), not a single global normal — without this, meshes read as flat floating masks regardless of Blender detail.
