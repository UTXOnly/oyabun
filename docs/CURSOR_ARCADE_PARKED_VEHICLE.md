# Arcade parked vehicle (R32): architecture & tradeoffs

This document is the decision record for how a **static parked car** is represented in the **procedural Kabukicho arcade** level (`client/src/arcade_level.rs`). It supersedes scattered notes about boxes, merged GLBs, and billboards.

**Related:** general prop / billboard rules → [`CURSOR_ARCADE_PROP_BILLBOARDS.md`](./CURSOR_ARCADE_PROP_BILLBOARDS.md). World pipeline → [`.cursor/rules/level-design.mdc`](../.cursor/rules/level-design.mdc).

---

## 1. Goals and constraints

**Goals**

- Read as a **car** from typical alley sightlines (player moves down a narrow street, car sits in the lane or at the curb).
- Match **pixel-art** presentation: **nearest** texture filtering, no photo-real sheen.
- Stay **cheap** at runtime: a few draw batches, no per-frame camera tricks for *parked* geometry unless explicitly chosen.

**Hard constraints (project)**

- **Gameplay bus** and authority boundaries are unchanged; this is purely **client visual mesh + collision**.
- **Arcade level** is built as `GltfLevelCpu` (procedural quads + optional merged assets). Full Tokyo levels use Blender → `.glb`; arcade is allowed to merge props via `gltf_level::append_glb_transform`.
- **Do not** use **camera-facing yaw billboards** for a parked car (same subsystem as NPCs): it **spins with the player** and reads as a HUD sticker. See prop billboard doc.

**Soft constraints**

- Prefer **reproducible** art steps (scripts, checked-in binaries) so CI and `include_bytes!` work without local-only secrets.
- Prefer **one place** to regenerate when `r32_*.png` sources change.

---

## 2. Current architecture (reference implementation)

**As implemented today**

1. **Source art** (hand or PixelLab `map-object`):  
   `client/level_textures/tokyo_props/r32_side.png`, `r32_front.png`, `r32_rear.png`.

2. **Build script** (Pillow):  
   `tools/build_arcade_r32_prop.py`  
   - Writes **three derived PNGs** under `client/props/generated/` (side scale, front|rear atlas, mean-color body tile).  
   - Embeds them in **`client/props/arcade_r32_prop.glb`** (Y-up box, three materials, nearest samplers).

3. **Runtime merge**:  
   `build_arcade_level()` finishes with `append_glb_transform(..., ARCADE_R32_GLB, Mat4::from_translation(...))` so the car sits at the correct world pose.

4. **Collision**:  
   A **manual AABB** in `arcade_level.rs` matches the placed mesh footprint (kept in sync with the box half-extents in the Python script).

**CLI**

- `python3 tools/oyabaunctl.py build-arcade-r32-prop`  
- Then `wasm-pack build --target web` in `client/` (embedded GLB must be refreshed after regenerating).

**Limitation (why it still “reads blocky”)**  
The **mesh is still an axis-aligned box**. Textures improve panels, but **silhouette and occlusion** are those of a rectangular prism. Expecting a box to read like a sculpted car from every angle is the recurring failure mode.

---

## 3. The core tradeoff: silhouette vs. effort

| Axis | Low effort | High effort |
|------|------------|-------------|
| **Silhouette** | Box or few quads | Low-poly modeled volume, extruded profile, or many sub-boxes |
| **Art reuse** | Slap existing PNGs on faces | Unwrap, bake, or author multi-view atlases |
| **Pipeline** | Rust / Python only | Blender (or external gen + Blender cleanup) |
| **Maintenance** | Single script or inline quads | `.blend` + export discipline |

Anything that **does not change the outer hull** beyond a **box** will tend to look like a **block** from at least some views.

---

## 4. Options considered (summary)

### A. Box / few quads + pixel textures (current and prior attempts)

**Includes:** single-texture box GLB; six-face box with per-face textures; procedural `wall_prop`-style stacks.

| Pros | Cons |
|------|------|
| Fast, scriptable, easy to merge | **Brick silhouette**; hero angle only looks “car-like” |
| Aligns with collision trivially | Temptation to fix in shader (emissive, saturation) fights symptoms |

**Verdict:** Good for **placeholder** or **distant clutter**; weak for a **hero mid-alley** prop.

---

### B. Blender (or equivalent DCC) low-poly car → `.glb`

**Flow:** Model simple hood, cabin, wheel wells; assign Principled + embedded PNGs; export glTF binary; either merge with `append_glb_transform` or fold into a larger level `.glb` later.

| Pros | Cons |
|------|------|
| **Best silhouette per triangle** | Requires Blender skill and export hygiene |
| Matches **`level-design.mdc`** for “real” props | Another artifact to version (or embed) |
| KHR_texture_transform / naming conventions already documented | Slightly heavier than one Python box |

**Verdict:** **Default recommendation** when the car must read clearly from **multiple** walk-around angles.

---

### C. Fixed world-yaw 2D stacks (crossed planes, **not** camera billboards)

**Idea:** Two or three vertical quads in an **X** or **T**, parented to a **fixed yaw** along the alley (e.g. normal in XZ only). No `atan2` toward camera.

| Pros | Cons |
|------|------|
| No spinning; still **2D art** | **Paper-thin** from the wrong angle |
| Can reuse existing side art | May need **extra views** in texture for less harsh edge-on read |

**Verdict:** Reasonable **middle ground** if Blender is deferred; document accepted viewing angles.

---

### D. Python (or tool) → **many small boxes** (“voxel” / kitbash)

**Idea:** Script emits glTF with separate primitives for bumper, cabin block, wheel blocks, etc., sharing one or few materials.

| Pros | Cons |
|------|------|
| Breaks **single-slab** outline without Blender | Tuning is fiddly; still not organic curves |
| Stays in repo as code | Easy to end up “lumpy” without art direction |

**Verdict:** Upgrade path from **A** if you refuse DCC but need more than one box.

---

### E. Silhouette **extrusion** from `r32_side.png`

**Idea:** Extract alpha (or edges), build a 2D polygon, extrude along **width** to a thin solid; UV from side art.

| Pros | Cons |
|------|------|
| Hull **derived from real art** | Mesh quality depends on clean alpha; may need manual cleanup |
| Distinct from “one box” | Top/down views can look odd without a roof pass |

**Verdict:** Interesting **Python-only** experiment; validate on one PNG before committing to pipeline.

---

### F. External 3D gen (Rodin, Hunyuan, etc.) → decimate → Blender → `.glb`

| Pros | Cons |
|------|------|
| Fast first mesh | License, topology, and **pixel** look need work |
| | Almost always still passes through **Blender** for Oyabaun |

**Verdict:** Optional shortcut to **B**, not a separate end state.

---

### G. CC0 low-poly vehicle + retexture

| Pros | Cons |
|------|------|
| Immediate **non-box** silhouette | May not match **yakuza / Tokyo** tone without repaint |
| | Still export + embed like **B** |

**Verdict:** Viable if **B** timeline is long and you need a **temp** mesh.

---

### H. Eight-direction atlas + **camera** billboard (NPC-style)

**Explicitly rejected** for **parked** cars in this project: wrong motion language, breaks immersion.

**Note:** A **fixed** atlas (no camera yaw) is closer to **C** and is not the same as NPC billboards.

---

## 5. Decision guide

| If you need… | Lean toward… |
|--------------|----------------|
| **Best look, any angle** | **B** (Blender) or **G** (CC0 + retexture) |
| **No Blender yet**, acceptable compromises | **C** (fixed crossed planes) or **D** (multi-box script) |
| **Keep everything in Python** off side PNG | **E** (extrusion), accepting cleanup |
| **Placeholder only** | Keep **A**; move hero prop later |

**Collision:** For **B–G**, either keep the **current manual AABB** or add a **named collider mesh** in the exported GLB (`Collider` / `OyabaunCollision` per `gltf_level.rs`) and **remove** duplicate manual solids when verified.

---

## 6. File map (quick reference)

| Piece | Path |
|--------|------|
| Arcade level build | `client/src/arcade_level.rs` |
| GLB merge helper | `client/src/gltf_level.rs` (`append_glb_transform`) |
| R32 prop generator | `tools/build_arcade_r32_prop.py` |
| Shipped prop GLB | `client/props/arcade_r32_prop.glb` |
| Derived textures | `client/props/generated/arcade_r32_*.png` |
| Source R32 PNGs | `client/level_textures/tokyo_props/r32_*.png` |
| Prop export notes | `client/level_textures/tokyo_props/EXPORT.txt` |

---

## 7. Changelog

| Date | |
|------|---|
| 2026-03-30 | Initial doc: current pipeline, option matrix, decision guide, file map. |
