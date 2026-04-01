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
| `client/characters/boss_v3_atlas.rgba` | Boss sprite atlas (embedded) |
| `client/characters/rival_v3_atlas.rgba` | Rival sprite atlas (embedded); **Rival** NPCs use this bind group |
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
| Rival | `dabe33dd-b9d5-481c-9413-402cd0002747` | 116×116 | Full **8-dir walk** in ZIP → `pixellab_zip_to_atlas.py` → `rival_v3_atlas.rgba` | ✅ In-game |
| SMG rival (WIP) | `dee01186-8482-431e-ada3-3a00f1101d01` | 112×112 | `create4` Uzi wakashu — **expand 8 dirs on web**, walk, zip, rebuild atlas | ⏳ Processing |
| Player | `fe8d4102-8926-4267-ab1c-4600441cfcf4` | 104×104 | ⚠️ v2 `animate` → *Failed to start any animation jobs* (tried `walking` / `walk` + `/characters/animations`) — queue walk on **pixellab.ai** for now | ❌ Uses boss atlas |
| Extra (suit enforcer, no gun in prompt) | `ffe4c106-addf-4e53-902a-9ef73f44ea56` | 48×48 | 1 animation | — |

### PixelLab: Cursor MCP vs HTTP v2 (reliable workaround)

- **Root cause**: Some Cursor MCP HTTP clients serialize tool arguments into **invalid JSON** (string values lose quotes, e.g. `"template_animation_id": walk` instead of `"walking"`). The server then rejects the call before any job is created — so **nothing appears** in the web app or in `list_characters`.
- **Fix (recommended)**: Use the same Bearer token as MCP and call **PixelLab REST v2** from the repo:

```bash
python3 tools/pixellab_v2.py balance
python3 tools/pixellab_v2.py list --limit 20
python3 tools/pixellab_v2.py animate <character_uuid> walking --name my-walk
python3 tools/pixellab_v2.py zip <character_uuid> /tmp/char.zip
python3 tools/pixellab_zip_to_atlas.py /tmp/char.zip -o client/characters/name_atlas.rgba
python3 tools/pixellab_v2.py create8 "description here" --size 112
python3 tools/pixellab_v2.py create4 "description here" --size 112   # if create8 → bone_scaling
```

Token: `PIXELLAB_API_TOKEN` env, or omit it and the script reads `.cursor/mcp.json` (gitignored). Docs: [api.pixellab.ai/v2/llms.txt](https://api.pixellab.ai/v2/llms.txt), [MCP tools overview](https://api.pixellab.ai/mcp/docs).

- **Concurrency**: Tier 1 allows **8 concurrent background jobs**. One **8-direction walk** fills all 8 slots — wait for completion before `create8` or a second `animate`, or you’ll get a “maximum 8 concurrent jobs” error.
- **`create8` server error**: `POST /create-character-with-8-directions` can fail with `'bone_scaling'`. **Workaround**: `POST /create-character-with-4-directions` via `curl`/script (same v2 token) — succeeds (e.g. job `46d21621-…`, character `92d6cd7c-be94-41cc-82e2-f0dc4bdeaaff`, 2026-03-29). Four-dir sprites need either web expansion to 8 dirs or a future atlas/shader path — **Oyabaun billboards expect 8 columns** today.

### Previous Characters (v2 standard, v1 deprecated)

- v2 Boss: `6d169ab6-bb02-4ef2-bf1e-6bec41553472` (64×64, standard mode)
- v2 Rival: `213e25e0-9c7a-4d71-a37f-cd199a4f9855`
- v2 Player: `ea4cdb4d-00bb-4f77-853d-843061b465f2`
- v1 Boss: `572836f2-a19f-41b5-bee5-46998f43b019`
- v1 Rival: `afd7b081-5b53-49bf-8f00-ecbd5e65f1c2`

## TODOs

### Priority 1: Characters with weapons baked in (SMG / Uzi / MP5)

Oyabaun is a **yakuza gangster** shooter: **primary weapons should read as SMGs** (Uzi, MAC-10, MP5/MP5K-style compact submachine guns) — clearly visible in the sprite, two-hand or hip-fire stance. Pistols are OK for some roles; **avoid** katanas unless a deliberate blade character.

**v2 API note**: `create8` often returns `bone_scaling`; use **PixelLab web (pro, 8 dirs)** for final SMG sprites, or `python3 tools/pixellab_v2.py create4 "…"` then **expand to 8 directions on the site** before walk + ZIP.

**Queued (create4, 4-dir) — SMG rival concept**: `dee01186-8482-431e-ada3-3a00f1101d01` (Uzi-style wakashu). When rotations exist: finish on web → 8 dirs → `animate … walking` → `zip` → `pixellab_zip_to_atlas.py` → replace `rival_v3_atlas.rgba` or add a new skin row.

**Prompt templates (copy into PixelLab pro or v2 create4):**

- **Boss / oyabun + SMG**: *Middle-aged Japanese yakuza boss, dark pinstripe suit, sunglasses, cigarette, holding compact MP5-style submachine gun at chest ready stance, spare mag pouch, stern scarred face, neo-noir, low top-down pixel art*
- **Rival / wakashu + SMG**: *Young yakuza enforcer, leather jacket, bleached or dyed hair, purple sunglasses, holding Uzi-style submachine gun forward grip, aggressive stance, neo-noir pixel art*
- **Player + SMG**: *Urban operator, dark coat or tactical vest, face mask or hood, holding compact SMG MP5K style, tactical gloves, neo-noir, not ninja*

- [ ] **Rival with katana** (optional): only for a deliberate blade-heavy role; still **gangster**, not samurai fantasy.

### Priority 2: Shooting/attack animations

PixelLab template animations available for combat:
- `cross-punch`, `fireball`, `flying-kick`, `high-kick`, `hurricane-kick`, `lead-jab`, `leg-sweep`, `roundhouse-kick`, `surprise-uppercut`, `taking-punch`

For a shooter game, custom animations may be better:
- [ ] **Boss shooting pistol**: custom animation — "firing pistol forward" (costs 20-40 generations/direction)
- [ ] **Rival katana slash**: Use template `cross-punch` or custom "slashing katana" animation
- [ ] **Player attack**: custom animation matching player's weapon

### Priority 3: Per-character atlas wiring

- [x] **Rival atlas** + **per-skin bind groups** — `rival_v3_atlas.rgba`, Rival skin uses rival texture in `render.rs`
- [ ] **Player atlas**: Same pipeline for player v3 when walk export is ready
- [ ] **Boss SMG atlas**: Replace `boss_v3_atlas.rgba` after new boss sprite with MP5/SMG (web pro or fixed `create8`)

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
