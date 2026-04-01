# Handoff: FPS M4 view-model + skinned NPC weapon attach

This document is for a fresh investigator (e.g. Claude). **Cursor agents iterated many times** on weapon orientation and NPC attachment; **in-game results are still wrong** (FPS rifle mis-aimed / “pointing up”, NPCs often show rifle pose with **no visible gun**, or a gun floating with wrong axes). The goal is to **fix the math and/or rendering path**, not to add another layer of guessed Euler angles.

---

## Goals

1. **First person:** Draw `client/props/m4a1_prop.glb` as a 3D view-model so the **bore points where the player looks** (same convention as `GameState::view_forward`), with sensible screen placement (lower-right). Optionally composite HUD `fpsweapons/arms.png` (already wired when `arms_ready`).

2. **Third person (skinned NPCs):** Draw the **same rigid prop** parented to the character’s **right hand** so it moves with `rifle_*` animations. Boss / rival / remote use `yakuza_shooter.glb` when skinned.

---

## Stack (relevant parts)

- **Rust / WASM** client: `client/src/`
- **wgpu** 23, **glam** matrices
- **Character shader** (`render.rs`, `SHADER_CHAR_TEX`): rigid path `world_pos = cu.model * vec4(pos,1)`; clip `cu.view_proj * world_pos`
- **Relay / protocol:** unrelated to this bug

---

## Assets

| Asset | Role |
|--------|------|
| `client/props/m4a1_prop.glb` | Rigid M4 mesh; exported by `tools/blender_m4a1_export_assets.py` from `m4a13d/base.obj`. Vertices are **baked with node world** in `gltf_level::visit_character_node` (`world.transform_point3`). Root node is identity in the shipped file. |
| `client/characters/yakuza_shooter.glb` | Skinned Mixamo body; skin joint **index 18** = `mixamorig:RightHand` (node index 15). |

**Measured mesh extent (Python on accessor):** dominant span is **Z** (~1.81 m); PCA longest axis ≈ **−Z** (treat **−Z as muzzle direction** in file space unless baking changes that).

---

## Current code paths (read these first)

### FPS world model

- `client/src/lib.rs`: if `gpu.weapon_prop_loaded()`, sets `WeaponHudParams.fps_weapon_model = Some(weapon_fps_world_model(&game))`.
- `client/src/render.rs`: `pub fn weapon_fps_world_model(game: &GameState) -> Mat4`  
  Builds `eye`, `forward = view_forward()`, `right`, `up`, `rot = from_cols(right, up, -forward, …)`, then `T(eye) * rot * local * tilt * scale`.
- `draw_world` → `draw_fps_weapon_3d`: writes one instance into weapon `CharacterDraw`’s uniform buffer; draws with **`fps_view_pipeline`** (`depth_write_enabled: false`, `depth_compare: Less`).

### NPC attach

- `gltf_level.rs`: `SkinnedCharacterMeshCpu.weapon_attach_joint` = first skin joint whose node name **`ends_with("RightHand")`** and does **not** contain **`RightHandIndex`** (avoids finger chains).
- `render.rs`: `draw_character_instances_3d` with `WeaponAttachPass { weapon_cd, hand_to_prop }` when `cd.is_skinned()` and `weapon_prop` exists.  
  Per instance: `compute_skinned_joint_palette` fills `node_world_scratch`; then  
  `wm = inst.model * node_world_scratch[jnode] * weapon_hand_to_prop_transform()`; batch draw rigid weapon with **`world_attach_pipeline`** (same shader as rigid char, **depth bias** enabled).

### Instance matrix

- `lib.rs` `character_model`: `scale * Quat::from_rotation_y(yaw + PI) * translate(foot)` — comment says Blender/glTF facing vs game yaw.

### Shader convention

- WGSL: `cu.model * vec4(v.pos, 1.0)` — **column vector, matrix on left** (matches glam column-major if layouts agree).

---

## What was tried (chronological / thematic)

1. **HUD sprite only** → user wanted **3D** prop; added rigid `CharacterDraw` for `m4a1_prop.glb` + `fps_view_pipeline`.
2. **NPC attach:** joint lookup used **`contains("righthand")`** → matched **`RightHandIndex4`** before **`RightHand`**; fixed to **`ends_with("RightHand")` + exclude `RightHandIndex`**.
3. **Assumed barrel along +Y** → added **`rotation_x(-90°)`**; **screenshots showed barrel pointing up** — inconsistent with PCA on GLB (**long axis −Z**).
4. **Removed large X-flip**, translation-only / small tilts for FPS; still wrong.
5. **`inverse(view_matrix) * local`** for FPS → user still saw **wrong aim**; replaced with **explicit camera basis** `weapon_fps_world_model` (current code).
6. **NPC depth fighting:** added **`world_attach_pipeline`** with **`DepthBiasState`** (constants tuned a few times: e.g. −40 / slope −6 / clamp −1).
7. **Hand space:** added **`rotation_x(+90°)`** in `weapon_hand_to_prop_transform()` assuming Mixamo hand **+Y** aligns grip; **still reported broken**.

---

## Observed symptoms (from user + screenshots)

- FPS: rifle often **vertical or off-axis** relative to look direction; **not** a stable iron-sight alignment.
- NPC: often **no rifle mesh** despite rifle animation; occasionally **floating / wrong orientation** when something did draw.
- **HUD arms** sometimes absent: depends on `fpsweapons/arms.png` loading (`uploadArmsSprite`); separate from rigid weapon matrix.

---

## Hypotheses worth verifying (not all mutually exclusive)

1. **Column vs row convention mismatch** between glam `Mat4::from_cols`, `to_cols_array_2d()` into `CharUniforms`, and WGSL `mat4x4 * vec4`. If uniforms are **row-major** in the shader but filled as **column-major**, every rigid/skin matrix would look wrong — but **the level and characters mostly look correct**, so the bug may be **isolated to weapon matrices** or to **composition order** only.

2. **`weapon_fps_world_model` basis:** Sign error on **`forward` / `right` / `up`**, or wrong multiplication order (`T * R * local` vs `R * T * …`), or double-application with **`view_proj`** already containing view.

3. **Rigid prop vertices** are already **baked in a rotated frame**; treating **−Z as muzzle** in “raw” space might be false after `visit_character_node` (currently root transform is identity — re-verify after any re-export).

4. **NPC:** `inst.model * joint_node_world` might need a different composition (e.g. joint global vs mesh root, or scale interaction with Mixamo). **Depth bias** might be **unsupported or clamped** on some WebGPU backends → attach pass never wins depth.

5. **`weapon_prop` fails to load** in some deployments (fetch / `include_bytes` / parse) → `weapon_attach` never runs; user sees pose-only.

---

## Suggested investigation order

1. **Single-frame debug:** log or overlay `forward`, `right`, `up`, and the **world position of muzzle** (max −Z vertex transformed by final FPS `model`) vs `eye + forward * k`. Confirm bore direction numerically.

2. **Minimal scene:** one cube replaced by gun with **known** axis colors; confirm how `cu.model` maps object +X/+Y/+Z on screen.

3. **NPC:** temporarily draw a **unit axis triad** at `inst.model * hand * origin` (no prop) to validate **hand frame** vs animation.

4. **Depth:** try **`depth_compare: Always`** + **`depth_write: false`** only for attach pass on a dev build to see if **occlusion** is why the NPC gun vanishes (then fix properly).

5. **Compare** `Mat4::look_at_rh` output to **manual** view matrix from the same `eye` / `forward` / `up` and verify `proj * view * weapon_model` sends bore center-screen at a test pose.

---

## Key symbols (quick index)

| Symbol | File |
|--------|------|
| `weapon_fps_world_model` | `client/src/render.rs` |
| `weapon_hand_to_prop_transform` | `client/src/render.rs` |
| `draw_fps_weapon_3d`, `draw_character_instances_3d`, `WeaponAttachPass` | `client/src/render.rs` |
| `weapon_attach_joint` resolution | `client/src/gltf_level.rs` |
| `compute_skinned_joint_palette`, `node_world_scratch` | `client/src/gltf_level.rs` |
| `view_forward`, `view_matrix`, `view_proj` | `client/src/game.rs` |
| `WeaponHudParams::fps_weapon_model` | `client/src/render.rs` + `lib.rs` |
| M4 load / `weapon_prop_level` | `client/src/lib.rs` (`create_oyabaun_app`), `Gpu::new` in `render.rs` |

---

## Constraints (project rules)

- **Gameplay bus** stays the Oyabaun relay WebSocket; no new transports.
- **Levels** stay Blender → GLB; don’t add procedural world geometry for this.
- After **wire or shader** changes, update **`docs/PROTOCOL.md`** if applicable (weapon work may not touch protocol).

---

## Build

```bash
cd client && wasm-pack build
```

User workflow often only runs **`oyabaunctl launch` / `stop`**; agent is expected to run `wasm-pack` when changing the client.

---

*Written as a handoff after repeated unsuccessful iteration in Cursor on FPS + NPC weapon rendering.*
