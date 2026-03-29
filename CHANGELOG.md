# Changelog

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
