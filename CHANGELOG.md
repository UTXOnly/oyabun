# Changelog

## 2026-03-29 — Pixel art sprite billboards replace 3D models (PixelLab v3 pro)

### Reverted: 3D Blender skin-modifier characters → PixelLab pixel art sprites

The procedural 3D character approach could not match the neo-noir pixel art reference style despite multiple iterations (audit tool, texture tweaks, shader tuning). Characters are now **PixelLab pro-mode pixel art sprites** rendered as camera-facing billboard quads.

### New character pipeline

```
PixelLab MCP (pro mode, 64px canvas, 8 directions)
    → walk animation (6 frames × 8 dirs)
    → download ZIP → extract
    → build_game_atlas.py → 896×784 atlas (8 cols × 7 rows, 112×112 cells)
    → convert to .rgba binary (width + height header + raw RGBA)
    → embed via include_bytes! in render.rs
    → billboard quads generated per-character per-frame
    → SHADER_BILL renders with alpha discard + fog
```

### Character art repo

Dedicated repo at `~/Desktop/oyabaun-characters/` for character art production:
- `reference/` — style target images + extracted palettes
- `prompts/` — generation prompts per character
- `tools/` — atlas builder, palette extractor, comparison tool, game export
- `raw/` → `refined/` → `export/` pipeline

### Characters (v3 pro mode)

| Character | PixelLab ID | Canvas | Status |
|-----------|-------------|--------|--------|
| Boss | `d5ceb30a-0a4b-49c4-8ccb-988898cb8135` | 112×112 | ✅ Atlas in-game |
| Rival | `dabe33dd-b9d5-481c-9413-402cd0002747` | 116×116 | Rotations only, walk TODO |
| Player | `fe8d4102-8926-4267-ab1c-4600441cfcf4` | 104×104 | Rotations only, walk TODO |

### Rendering changes (render.rs)

- **Added**: Embedded sprite atlas loading (raw RGBA → GPU texture → bind group)
- **Added**: Camera-facing billboard quad generation per character
- **Added**: 8-direction selection from camera-character relative angle
- **Added**: Walk animation row selection from `anim_frame`
- **Preserved**: 3D GLB path (bypassed when sprite atlas present)
- **Shader**: Reuses `SHADER_BILL` (alpha discard at 0.35, nearest sampling)

### Files changed

- `client/src/render.rs` — char_sprite_bg fields, atlas loading, billboard quad gen, draw calls
- `client/characters/boss_v3_atlas.rgba` — Embedded sprite atlas (896×784, 2.8MB)
- `client/characters/boss_v3_atlas.png` — Visual reference copy

---

## 2026-03-29 (earlier) — 3D character overhaul: skin modifier models replace sprite billboards

### Breaking change: Deprecated PixelLab sprite pipeline

The entire billboard sprite system has been replaced with proper 3D character models. The old pipeline (PixelLab → atlas PNG → billboard quad GLB → atlas UV shader) is fully deprecated.

### New character pipeline: Blender skin modifier

Characters are now organic 3D humanoid meshes built via Blender's skin modifier technique:

1. **Skeleton definition**: Joint positions as vertices, connected by edges
2. **Skin modifier**: Inflates skeleton into smooth organic mesh with per-joint radii controlling body proportions (broad shoulders, thin waist, etc.)
3. **Subdivision + Decimate**: Subdivide level 2 for smoothness, decimate to ~30% for game performance (~1100 verts)
4. **Material assignment**: Face center Z-position determines material (shoes/suit/skin/hair)
5. **Detail meshes**: Separate objects for sunglasses, weapons, neon accents, tie
6. **Join + Export**: Single GLB per character type

### Characters

| Character | File | Verts | Materials | Features |
|-----------|------|-------|-----------|----------|
| Boss | oyabaun_player.glb | 1100 | 11 | Dark suit, sunglasses, pistol, red tie, cyan neon accents, slicked hair |
| Rival | oyabaun_rival.glb | 1186 | 11 | White suit, purple glasses, katana with glowing edge, bleached spiky hair, facial scar, purple neon accents |

### Shader overhaul (SHADER_CHAR_TEX)

- **Replaced**: Billboard vertex shader (extracted foot pos, computed camera-facing axes, positioned quad)
- **New**: Standard 3D model transform (`world_pos = model * vec4(v.pos, 1.0)`)
- **Replaced**: Atlas UV column/row selection (8 directions × 7 animation rows)
- **New**: Material tint color sampling (`textureSample * mu.tint`)
- **Added**: Directional lighting with cyberpunk ambient tones
- **Changed**: Depth write ON (solid 3D models vs transparent sprites)
- **Kept**: Hit flash (anim_frame > 100), distance fog

### Facing fix

- Characters faced away from player because Blender -Y front → glTF +Z, but game yaw 0 = -Z
- Fixed by adding PI offset in `character_model()`: `Quat::from_rotation_y(yaw + PI)`

### Hitbox Y-sync fix (shooting)

- NPC `foot.y` was stuck at spawn value (0.0) while visual rendering used terrain-aware `feet_draw_y()`
- Hitbox AABB was at wrong Y height, causing shots to miss
- Fixed: `foot.y` now syncs with `feet_draw_y()` every tick after NPC AI update

### Removed

- `append_char_gun_billboard()` function (dead code from old gun sprite overlay)
- All atlas-related shader code (ATLAS_ROWS, TAU, direction index computation, atlas UV mapping)

### Updated docs/rules

- `.cursor/rules/character-gen.mdc` — Updated from PixelLab sprite pipeline to 3D skin modifier pipeline
- `.claude/skills/blender-characters.md` — Updated with new pipeline, conventions, file references
- `docs/CURSOR_CHARACTER_IMPROVEMENT.md` — New task doc for further character detail improvements
- `docs/character-gen-spec.md` — Needs update (still references old sprite pipeline)

### Files changed

- `client/src/render.rs` — New SHADER_CHAR_TEX (3D transform), removed billboard shader + gun billboard function, depth write enabled
- `client/src/lib.rs` — character_model() PI offset, foot.y terrain sync
- `client/characters/oyabaun_player.glb` — New 3D boss model (51KB, was sprite quad)
- `client/characters/oyabaun_rival.glb` — New 3D rival model (155KB, was sprite quad)

---

## 2026-03-28 — Tokyo alley full redesign (Phases 1-3)

### Level overhaul: from flat walls to cyberpunk Tokyo

- **Phase 1 — Shop depth**: 84 objects added — recessed doorways (OYA_Trim), tilted awnings, vertical blade signs per building segment. Every wall section now has 3D depth instead of flat surfaces.
- **Phase 2 — Shop identities**: 120 objects added — 8 distinct shop types across 28 segments:
  - Ramen shops (noren curtains, warm glow, menu boards, step stones)
  - Pachinko parlors (neon arches in pink/cyan, fluorescent panels)
  - Yakuza offices (dark glass doors, gold kanji signs)
  - Konbini (fluorescent-lit windows, magazine rack silhouettes)
  - Tattoo parlors (dark entrances, dragon art panels)
  - Izakaya (red/white lanterns, wood sliding doors)
  - Shuttered shops (corrugated metal strips, rust, graffiti, "for rent" signs)
  - Arcades (cyan glow, cabinet silhouettes)
  - Bars, pharmacies, bookshops, noodle shops (warm glass, step stones)
- **Phase 3 — Environment detail**: 145 objects added:
  - Ground: drain channels along both walls, 4 manhole covers, 12 puddles, painted parking lines, 48 debris clusters
  - Overhead: 14 cross-cables at varying heights, 6 longitudinal cables, 12 AC units with drip stains
  - Infrastructure: 6 horizontal rusty pipes, fire escape with 8 steps + railings + landing platform
- **89 materials packed** with procedural pixel-art textures (96×96, dithered, Bayer pattern) for proper glTF export
- **36 new materials** created: noren fabric, shutters, neon arches, dark glass, lanterns, drain grates, manholes, puddles, cables, pipes, fire escapes, etc.
- **Performance**: 28,684 triangles (budget: 100k), 3.6 MB GLB (budget: 5 MB), 92 textures

---

## 2026-03-28 — Fix level textures (KHR_texture_transform), walk animation system

### Critical fix: Level textures rendering

- **Root cause**: Blender exported `tokyo_alley.glb` with `KHR_texture_transform` in `extensionsRequired`. The Rust `gltf` crate rejected the entire file, so the game fell back to the plain white procedural arena — no textures, no materials, no colors.
- **Fix 1 — Cargo.toml**: Added `"KHR_texture_transform"` feature to the `gltf` crate so it accepts the extension.
- **Fix 2 — gltf_level.rs**: Extract UV `offset`, `scale`, and `rotation` from `KHR_texture_transform` on each material's `baseColorTexture` and bake transforms into vertex UVs at load time (`uv' = rotate(uv) * scale + offset`). The shader samples with the pre-transformed UVs so tiling (e.g. 3×3 brick repeat) works correctly.
- All 61 texture images in the GLB (asphalt, brick, concrete, windows, neon signs, awnings, shop signs, etc.) now render properly.

### Walk animation system (all three characters)

- **Multi-frame atlas format**: Character atlases are now 8 columns × 7 rows (row 0 = idle, rows 1–6 = walk frames). Previous format was 8 columns × 1 row (idle only).
- **Shader walk frame selection**: `fs_char` reads `char_params.w` as `anim_row` and computes `atlas_v = (uv.y + anim_row) / ATLAS_ROWS` to select the correct row.
- **`CharacterInstance.anim_frame`**: New field passed through `char_params.w`. NPCs send 0.0 (idle), remote players cycle walk frames at 8 FPS.
- **`walk_anim_frame()` helper**: Computes frame index from `game_time` and movement speed.
- **All three characters animated** via PixelLab template walk (8 directions × 6 frames each):
  - Boss (`sprite1.png`, 512×350), Rival (`sprite_rival.png`, 512×357), Player (`sprite_player.png`, 512×336)
- **GLB rebuild**: `blender_make_oyabaun_character.py` updated for new atlas cell aspect ratio; accepts `OYABAUN_OUT`/`OYABAUN_SPRITE` env vars.

---

## 2026-03-28 — Tokyo alley redesign Phase 1 (shop depth) + doc/ctl alignment

### Level geometry

- **`tools/blender_redesign_tokyo_alley_phase1.py`**: Idempotent pass that recreates collection **`OyabaunRedesign_Phase1`** — per `LeftBuilding_*` / `RightBuilding_*` segment: recessed doorway volume (`OYA_Trim`), tilted awning (`OYA_Awning` / trim / building), vertical blade sign (`ShopSign_*` rotation). No `Collider` in object names; awnings ~0.6 m from façade.
- **`oyabaunctl redesign-tokyo-phase1`**: Runs that script on `client/levels/tokyo_alley.blend`; optional **`--export-after`** chains enhance+repack+GLB/JSON (same as manual `export-world --force-all` after).

### Docs

- **`docs/CURSOR_LEVEL_REDESIGN.md`**: Progress table, `oyabaunctl` export pipeline (removed invalid `export_colors` example), Phase 1 marked scripted vs still-handwork.

### Assets

- **`client/levels/tokyo_alley.blend`** / **`.glb`** / **`tokyo_street.json`**: Re-exported after Phase 1 (~+1k tris). Phases 2–5 in redesign doc remain TODO (kanji neon, ground debris, overhead density, etc.).

---

## 2026-03-30 — Walk animation system, multi-frame atlas, all characters animated

### Walk animation pipeline

- **Multi-frame atlas format**: Character atlases are now 8 columns × 7 rows (row 0 = idle, rows 1–6 = walk frames). Previous format was 8 columns × 1 row (idle only).
- **Shader walk frame selection**: `fs_char` reads `char_params.w` as `anim_row` and computes `atlas_v = (uv.y + anim_row) / ATLAS_ROWS` to select the correct row. `ATLAS_ROWS` constant (7.0) defined in WGSL.
- **`CharacterInstance.anim_frame`**: New field passed through `char_params.w` to the shader. NPCs send 0.0 (idle), remote players cycle walk frames at 8 FPS.
- **`walk_anim_frame()` helper**: Computes frame index from `game_time` and movement speed. Below 0.3 m/s → idle (row 0), above → cycles rows 1–6.
- **`game_time` accumulator**: Added to `OyabaunApp` for smooth time-based animation independent of frame rate.

### All three characters now have walk animations (PixelLab)

| Character | PixelLab ID | Atlas | Dimensions |
|-----------|-------------|-------|------------|
| Oyabaun Boss | `6d169ab6` | `sprite1.png` | 512×350 (8×7, cell 64×50) |
| Yakuza Rival | `213e25e0` | `sprite_rival.png` | 512×357 (8×7, cell 64×51) |
| Player Ronin | `ea4cdb4d` | `sprite_player.png` | 512×336 (8×7, cell 64×48) |

Each atlas contains 8 directional idle frames + 6-frame walk cycle per direction (48 walk frames total per character).

### Atlas build pipeline

Python script crops all PixelLab 64×64 frames to tight vertical bounds, arranges into 8-column grid (S, SE, E, NE, N, NW, W, SW order), idle in row 0, walk frames in rows 1–6. Output PNG fed to Blender GLB build script.

### GLB rebuild

- `tools/blender_make_oyabaun_character.py` updated for new atlas cell aspect ratio (64×50 → 1.28:1 vs old 64×49 → 1.306:1). Added `ATLAS_ROWS`, `CELL_W`, `CELL_H` constants. Accepts `OYABAUN_OUT` and `OYABAUN_SPRITE` env vars for building different characters.
- `oyabaun_player.glb` rebuilt with boss walk atlas (37KB, down from 1.1MB — old GLB had the full Blender character mesh baked in).
- `oyabaun_rival.glb` rebuilt with rival walk atlas (36KB).

### Known issues / TODO

- **Multi-character texture swap**: All non-rival entities still share the boss atlas (`oyabaun_player.glb`). Player ronin atlas built (`sprite_player.png`) but needs a third `CharacterDraw` or runtime texture swap to display in-game.
- **Player visibility desync**: Users in different browser sessions sometimes see different player sets — likely server-side relay snapshot sync issue.

---

## 2026-03-30 — Atlas vs billboard yaw, ground snap, idle bob, offline local body

- **Fixed black / wrong atlas column**: Every instance used `yaw_face_cam_xz` (mesh faces camera) while `fs_char` also picked an atlas column from camera→character angle — double-counting, often sampling empty strips. Fragment shader now uses a **fixed front column** (index 4) until we add a mode that pairs **true world yaw** with camera-relative column selection.
- **Feet height**: NPCs use `ground_y_at.max(level_foot.y)`; remotes use `ground_y_at.max(server_y)` so everyone stays on the walk mesh when either probe or authored Y is higher.
- **Idle animation placeholder**: `draw_world` takes `character_anim_t`; `vs_char` applies a small vertical bob until walk frames exist in the atlas (see 2026-03-29 TODO).
- **Offline “you”**: When not joined and the character GLB loaded, the local player gets the same instanced mesh at ground (online stays first-person only to avoid clipping through your own card).

## 2026-03-29 — Character system overhaul, yakuza characters, weapon animations

### Character rendering fixes

**Facing direction (characters showed their backs)**
- `render.rs`: Shader atlas direction selection added `+4u` offset — PixelLab direction names are character-facing (e.g. "south" = character faces south), but the shader was treating them as camera-facing. Now camera-north correctly shows the south (front) sprite.
- `lib.rs`: Billboard yaw formula fixed from `dx.atan2(-dz)` to `(-dz).atan2(dx)` — the character quad mesh faces +X at yaw=0, so the atan2 args needed to produce the correct rotation for `Quat::from_rotation_y`.

**NPC positioning (rival was past the alley wall)**
- `mesh.rs`: `npc_placements()` now computes direction from spawn toward alley center instead of using spawn yaw. The Blender spawn node pointed toward +Z (wall), pushing NPCs at Z=29 and Z=35 (past bounds max Z=31.7). Now uses `(center - spawn).normalize()` as forward.
- `lib.rs`: Spawn yaw overridden to face toward NPC midpoint so player sees characters on load.

**Remote player floating**
- `lib.rs`: Remote players now use `ground_y_at(p.x, p.z)` for foot placement instead of `(p.y).max(gy)` — server Y may be eye-level or from a different collision model.

**Offline demo characters followed the player**
- `lib.rs`: Changed from `base + fwd * 5.0 + right * 2.0` (player-relative) to fixed world positions between boss and rival.

### New architecture: NPC manager (`npc.rs`)

Unified `BossState` and `RivalState` (which were near-identical 75-line structs) into:
- `NpcDef` — configuration struct (label, max_hp, scale, hitbox dimensions)
- `Npc` — instance with def, foot position, HP, hit detection
- `NpcManager` — holds `Vec<Npc>`, loops shots across all NPCs

Constants `BOSS_DEF` and `RIVAL_DEF` define the two NPC archetypes. Adding new types is one `NpcDef` const + one `push` in `NpcManager::new()`. Render loop iterates `npcs.npcs` instead of separate boss/rival blocks.

### Weapon animations

**Recoil** (`loadout.rs` + `render.rs`):
- `recoil` field (1.0 on fire, decays at 8.0/s) — shader kicks weapon up 8% and right 1.5%
- Can't fire during reload

**Reload** (`loadout.rs` + `render.rs`):
- `reload_anim` (0→1 lower, 1→2 raise, speed 2.5 = ~0.8s cycle)
- Ammo loads at midpoint (weapon below screen)
- Can't switch weapons during reload
- `HudUniform` expanded from 16 to 32 bytes to pass recoil + reload to shader

### New Japanese yakuza characters (PixelLab MCP)

Replaced v1 characters (boss looked African American) with yakuza-themed v2:

| Character | PixelLab ID | Description |
|-----------|-------------|-------------|
| Oyabaun Boss | `6d169ab6-bb02-4ef2-bf1e-6bec41553472` | Japanese crime lord, dark suit, scarred face, slicked hair |
| Yakuza Rival | `213e25e0-9c7a-4d71-a37f-cd199a4f9855` | Young enforcer, leather jacket, spiky hair |
| Player Ronin | `ea4cdb4d-00bb-4f77-853d-843061b465f2` | Street ronin, hoodie + katana, face mask |

Boss atlas built and in-game (`sprite1.png` 512x47). Walk animations queued. Rival + player sprites downloaded to `client/characters/` but not yet atlased.

### Cursor integration docs

- `.cursor/rules/character-gen.mdc` — Pipeline rules (PixelLab params, atlas build, GLB, WASM)
- `.cursor/skills/character-generation.md` — Full skill doc: end-to-end workflow, architecture reference, NPC code patterns, multi-character TODO, gotchas list
- `docs/character-gen-spec.md` — Updated with v2 character IDs, corrected direction offset docs, fixed boss description

### Debug improvements

- `index.html`: Exposed `window._oyaApp` for runtime inspection
- `lib.rs`: Added `player_yaw` and `player_pos` to `bootDebugJson()`

### Known issues / TODO

- **Multi-character atlas**: All entities still share one atlas (boss). Need runtime texture swap or combined atlas so boss/rival/player look different. See `docs/character-gen-spec.md` "Future" section.
- **Walk animation frames**: PixelLab walk animations generated but not yet integrated into the shader (currently static single-frame sprites). Need atlas format change to include animation frames + time-based frame selection in `fs_char`.
- **Player visibility desync**: Users in different browser sessions sometimes see different player sets. Likely server-side relay snapshot sync issue.
- **Rival/player walk animations**: Need to queue after boss walk completes (8 concurrent PixelLab job slot limit).

### Files changed

- `client/src/npc.rs` — **new** (Npc, NpcDef, NpcManager)
- `client/src/lib.rs` — NpcManager integration, spawn yaw fix, billboard yaw fix, debug fields
- `client/src/render.rs` — shader +4 direction offset, HudUniform expanded, recoil/reload in vs_hud
- `client/src/loadout.rs` — recoil, reload_anim, start_reload(), fire blocking during reload
- `client/src/mesh.rs` — npc_placements uses alley center direction + bounds param
- `client/index.html` — window._oyaApp exposed, loadArmsSprite call added
- `client/sprite1.png` — new 512x47 yakuza boss atlas
- `client/characters/oyabaun_player.glb` — rebuilt from new atlas
- `client/characters/boss_new_*.png` — 8 direction PNGs (yakuza boss)
- `client/characters/rival_new_*.png` — 8 direction PNGs (yakuza rival)
- `client/characters/player_*.png` — 8 direction PNGs (player ronin)
- `.cursor/rules/character-gen.mdc` — **new**
- `.cursor/skills/character-generation.md` — **new**
- `docs/character-gen-spec.md` — updated

---

## 2026-03-28 — 90s-style characters + Tokyo street atmosphere

### Changes

**`client/levels/tokyo_alley.glb`** (rebuilt from Blender)
- Rebuilt Boss and Rival characters with 90s game-style geometry (10-seg cylinders,
  multi-ring spheres) instead of blocky cubes. ~70 mesh parts total with proper
  armature skinning via vertex groups + armature modifiers.
- Added 268 street detail objects: neon signs (14 vertical + 8 jutting), 7 emissive
  wall signs, 5 vending machines, 10 overhead power cables, 8 wall pipes, 12 paper
  lanterns, 8 door frames with noren curtains, 10 hanging banners, trash bags, beer
  crate stacks, bicycle, potted plants, puddles, drain grates, utility boxes,
  satellite dishes, umbrella stand, wall-mounted lights.
- 679 objects, 656 meshes, 290 materials. GLB: 2.85 MB.

**`.claude/skills/`** (new)
- Added `project-overview.md`, `blender-export.md`, `blender-characters.md` skill
  files for efficient cross-session workflow.

---

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
