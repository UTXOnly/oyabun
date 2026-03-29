# Character Improvement Task Doc

## Status: PIPELINE REVERTED — now using PixelLab pixel art sprites, NOT 3D Blender models

## What Changed (2026-03-29)

The 3D Blender skin-modifier character pipeline **FAILED** to match the neo-noir pixel art reference style. After multiple iterations (procedural textures, audit tools, shader tuning), the approach was abandoned.

**Characters are now PixelLab pro-mode pixel art sprites** rendered as camera-facing billboard quads. This produces dramatically better results that match the reference images in `example_images/`.

### DEPRECATED — Do NOT use:
- `tools/blender_build_oyabaun_characters_3d.py` — old 3D generator
- `tools/character_style_audit.py` — compared renders to ref, irrelevant now
- `tools/blender_character_capture.py` — headless render for audit
- `client/characters/oyabaun_player.glb` / `oyabaun_rival.glb` — old 3D models (still embedded but bypassed)
- Any Blender-based character workflow

## Current Architecture

### Pipeline

```
PixelLab MCP (pro mode, canvas ~104–128px typical, 8 directions)
    → animate_character (walk, shoot, etc.)
    → download ZIP → extract frames
    → oyabaun-characters/tools/build_game_atlas.py
        → 8 cols (directions) × N rows (idle + anim frames) atlas PNG
    → python3 tools/export_character_atlas_to_rgba.py <atlas.png> -o client/characters/<name>_atlas.rgba
        (or PIL one-liner — 8-byte LE width/height + RGBA)
    → client/characters/<name>_atlas.rgba
    → include_bytes!() in render.rs
    → billboard quads in draw_world() with atlas UV selection
    → SHADER_BILL: alpha discard, fog, nearest sampling
```

### Rendering (render.rs)

- `char_sprite_bg`: bind group for the sprite atlas texture
- Billboard quads generated per-character in `draw_world()`:
  - Camera-facing quad at character foot position
  - Direction column (0-7) from `atan2(cam-char)` relative to `mesh_yaw`
  - Animation row from `anim_frame` (0 = idle, 1-6 = walk)
- `SHADER_BILL`: simple texture sample + alpha discard + rim
- 3D GLB path preserved but bypassed when `char_sprite_bg.is_some()`
- **Ground alignment**: `CHAR_BILLBOARD_FEET_DROP` in `render.rs` lowers the quad — atlas cells have transparent padding under the feet; increase/decrease if sprites float or sink.
- **Atlas rows**: Row count for UVs is derived from embedded atlas width/height (8 columns, square cells); no hardcoded row count.

### Key Files

| File | Purpose |
|------|---------|
| `client/src/render.rs` | Billboard sprite rendering, atlas loading, SHADER_BILL |
| `client/src/lib.rs` | character_model(), walk_anim_frame(), walk_bob_y() |
| `client/src/npc.rs` | NPC AI, hitboxes, wave spawning |
| `client/characters/boss_v3_atlas.rgba` | Boss sprite atlas (embedded, 896×784) |
| `~/Desktop/oyabaun-characters/` | Character art repo (generation, refinement, export) |

### Atlas Format

- Raw binary: 8-byte header (u32 LE width, u32 LE height) + RGBA pixel data
- Layout: 8 columns × N rows of square cells
- Columns: S, SW, W, NW, N, NE, E, SE
- Row 0: idle/standing rotation
- Rows 1+: animation frames

### PixelLab Characters (v3 pro mode)

| Character | PixelLab ID | Canvas | Animations | In-Game |
|-----------|-------------|--------|------------|---------|
| Boss | `d5ceb30a-0a4b-49c4-8ccb-988898cb8135` | 112×112 | walk (8 dirs × 6 frames) | ✅ Active |
| Boss v4 (pistol oyabun, 2026-03-29) | `38781ecc-b15b-44ab-b2a2-0c4b4ee3fb8b` | 112×112 | **Add walk** when gen completes (see below) | ⏳ Generating |
| Rival | `dabe33dd-b9d5-481c-9413-402cd0002747` | 116×116 | None yet | ❌ Uses boss atlas |
| Player | `fe8d4102-8926-4267-ab1c-4600441cfcf4` | 104×104 | None yet | ❌ Uses boss atlas |

**Boss v4 next steps:** Poll PixelLab / MCP `get_character(character_id="38781ecc-b15b-44ab-b2a2-0c4b4ee3fb8b")` until rotations exist. Then queue **walk** (template `walk` or `walking`): use the [PixelLab](https://pixellab.ai) site if Cursor’s `animate_character` MCP call errors (some clients send invalid JSON for string args). Export ZIP → `build_game_atlas.py` → `tools/export_character_atlas_to_rgba.py` → replace `boss_v3_atlas.rgba` (or add a second atlas + bind group when per-skin wiring lands).

### Previous Characters (v2 standard, v1 deprecated)

- v2 Boss: `6d169ab6-bb02-4ef2-bf1e-6bec41553472` (64×64, standard mode)
- v2 Rival: `213e25e0-9c7a-4d71-a37f-cd199a4f9855`
- v2 Player: `ea4cdb4d-00bb-4f77-853d-843061b465f2`
- v1 Boss: `572836f2-a19f-41b5-bee5-46998f43b019`
- v1 Rival: `afd7b081-5b53-49bf-8f00-ecbd5e65f1c2`

## TODOs

### Priority 1: Characters with weapons baked in

Oyabaun is a **yakuza gangster** shooter: adults in suits or street-luxury, firearms first; katanas optional for specific roles — **not** cartoon teens or ninja archetypes.

The in-game boss atlas should show a **visible gun** in-frame. **Regenerate** with prompts like:

- [ ] **Boss / oyabun**: Pro mode, 8 dirs — e.g. *middle-aged Japanese yakuza boss, dark pinstripe suit, sunglasses, cigarette, gold ring, holding semi-automatic pistol in two-hand forward stance, stern scarred face, neo-noir crime drama, low top-down pixel art*
- [ ] **Rival / wakashu**: Pro mode — *young adult yakuza enforcer, leather or loud suit, visible pistol or submachine gun, bleached or dyed hair, aggressive stance, same noir tone*
- [ ] **Player**: Pro mode — *hardened street operator in dark coat or tactical jacket, face partially obscured, **firearm clearly visible**, urban yakuza-adjacent look, not ninja*

- [ ] **Rival with katana** (optional variant): only if you want a blade-heavy role — still reads as **gangster**, not samurai fantasy.

### Priority 2: Shooting/attack animations

PixelLab template animations available for combat:
- `cross-punch`, `fireball`, `flying-kick`, `high-kick`, `hurricane-kick`, `lead-jab`, `leg-sweep`, `roundhouse-kick`, `surprise-uppercut`, `taking-punch`

For a shooter game, custom animations may be better:
- [ ] **Boss shooting pistol**: custom animation — "firing pistol forward" (costs 20-40 generations/direction)
- [ ] **Rival katana slash**: Use template `cross-punch` or custom "slashing katana" animation
- [ ] **Player attack**: custom animation matching player's weapon

### Priority 3: Per-character atlas wiring

Currently all characters use the boss atlas. Need:

- [ ] **Rival atlas**: Queue walk animations for rival v3, build atlas, convert to .rgba, add to render.rs
- [ ] **Player atlas**: Same for player v3
- [ ] **Per-skin bind group**: Modify render.rs to store `char_sprite_bg_boss`, `char_sprite_bg_rival`, select in `draw_world()` based on `CharacterSkin`

### Priority 4: Additional animations

Template animations to consider (1 generation/direction, cheap):
- [ ] `breathing-idle` — better than static idle
- [ ] `running-6-frames` or `running-8-frames` — for sprint
- [ ] `falling-back-death` — death animation
- [ ] `taking-punch` — hit reaction
- [ ] `fight-stance-idle-8-frames` — combat idle

### Priority 5: Hit flash

- [ ] Add hit flash support to SHADER_BILL (or create SHADER_CHAR_BILL with tint uniform)
- [ ] When `anim_frame > 100.0`, mix sprite color with red (same convention as old 3D shader)

### Priority 6: Pixel art weapons as separate sprites (alternative approach)

If baking weapons into characters doesn't work well:
- [ ] Generate standalone weapon pixel art (pistol, katana, SMG) as small sprites
- [ ] Render as separate billboard quads attached to character hand positions
- [ ] Animate weapon independently of character

## Build & Test

```bash
# After changing .rgba atlas files or render.rs:
cd client && wasm-pack build --target web --no-typescript

# Serve:
python3 -m http.server 8080 --directory client

# Hard refresh browser (Cmd+Shift+R)
```

## Character Art Repo

The dedicated character art repo at `~/Desktop/oyabaun-characters/` has:

```
reference/          # Style target images + palettes
prompts/            # PixelLab generation prompts per character
raw/sprites/        # PixelLab outputs (ZIP extracts)
export/atlases/     # Game-ready atlas PNGs
tools/
    build_game_atlas.py   # Extracted ZIP → atlas PNG
    palette_extract.py    # Reference → color palette
    compare.py            # Side-by-side QA
    export_to_game.py     # Copy to oyabaun repo
```

### Atlas build workflow

```bash
cd ~/Desktop/oyabaun-characters

# 1. Extract PixelLab ZIP
unzip raw/sprites/boss_v3/boss_v3.zip -d raw/sprites/boss_v3/extracted

# 2. Build atlas
python tools/build_game_atlas.py raw/sprites/boss_v3/extracted --out export/atlases/boss_v3_atlas.png

# 3. Convert to raw RGBA for embedding
python3 ../oyabaun/tools/export_character_atlas_to_rgba.py export/atlases/boss_v3_atlas.png \\
    -o ../oyabaun/client/characters/boss_v3_atlas.rgba

# 4. Rebuild WASM
cd ../oyabaun/client && wasm-pack build --target web --no-typescript
```

## DO NOT

- Use Blender skin-modifier pipeline for characters — it's deprecated
- Use `tools/blender_build_oyabaun_characters_3d.py` — deprecated
- Create floating weapon billboards separate from characters (bake weapons into sprites instead)
- Use billboard vertex shaders with model transforms — billboard quads are pre-built in world space
- Forget that atlas .rgba files have an 8-byte header (u32 width, u32 height) before pixel data
