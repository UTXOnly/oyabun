# Handoff: Blender level → WASM client (why it still looks wrong)

This document is for another engineer or AI assistant (e.g. Claude with **Blender MCP**) who needs to **debug why the exported Tokyo alley does not match Blender** or still looks like an empty / wrong scene in the browser.

## What Oyabaun is doing

- **Stack**: Rust → WebAssembly, **wgpu** WebGPU, static site served from **`client/`** (`python3 tools/oyabaunctl.py launch`).
- **Level data**: Primary path is **`client/levels/tokyo_alley.glb`** (glTF 2.0 binary). The WASM binary also **`include_bytes!`** the same file so the level loads even if `fetch()` fails.
- **Gameplay bus**: Go WebSocket relay (`relay/`); Nostr is auth only. Not relevant to *geometry* except that **when `joined == true`**, the client lerps **X/Z** from server snaps and derives **feet Y** from short collision solids under the player (see `client/src/game.rs`).

## End-to-end pipeline (files)

| Step | Location | Notes |
|------|----------|--------|
| Export from `.blend` | `tools/oyabaunctl.py export-world`, `tools/blender_export_gltf_oyabaun.py` | Runs Blender headless; `bpy.ops.export_scene.gltf`, `export_yup=True`, `export_apply=True`. |
| Copy hand export | `python3 tools/oyabaunctl.py import-glb ~/path/to/file.glb --rebuild` | Copies to `client/levels/tokyo_alley.glb` and should run **wasm-pack** (embed refreshes only after rebuild). |
| Load order (WASM) | `client/src/lib.rs` → `load_game_init` | Fetch GLB → parse; else embedded bytes; else fetch `tokyo_street.json`; else **`build_arena()`** procedural placeholder. |
| glTF parse | `client/src/gltf_level.rs` | Walks **scene root nodes** recursively; meshes → world vertices + batches; spawn empties; collider-named nodes → AABBs. |
| Render | `client/src/render.rs` | `SHADER_WORLD_TEX`: nearest albedo × tint, 15-step posterize, fog. **No** lighting normals in use beyond depth. |
| Spawn | `client/src/gltf_level.rs` | Named empties **or** heuristic: **min-Z + 14% of Z span**, center X (not AABB center-Z). |
| HUD label | `client/src/lib.rs` | Header shows **`glTF tokyo_alley`**, **`vertex JSON`**, or **`procedural demo`** — if you see **procedural demo**, the GLB path did not win. |

More detail: `docs/BLENDER_GLTF.md`, `docs/ARCHITECTURE.md`.

## What “wrong” means (reported symptoms)

Users describe the in-game view as **nothing like Blender**: a **flat / empty / simple** layout, “weird square”, few boxes, while **billboards** (reference mural, boss, etc.) still show. That can mean:

1. **Wrong level path**: Procedural `build_arena()` or JSON fallback instead of glTF (check HUD label + console warnings in `lib.rs`).
2. **Wrong place in a large map**: Camera in a sparse region while art clusters elsewhere (mitigated by min-Z spawn heuristic; override with **`OyabaunSpawn`** empty in Blender).
3. **Wrong vertical alignment**: Previously **online** mode forced **`y = 0`** every tick (fixed); relay spawns were for old demo map (partially retuned in `relay/internal/relay/room.go`).
4. **Materials don’t export**: Inspecting **`tokyo_alley.glb`** JSON chunk can show **`images` / `textures` count = 0** — only **`baseColorFactor`** solids → game looks like **flat colored low-poly**, not the textured Eevee viewport. **Fix in Blender**: Principled **Base Color** → **Image Texture** (glTF-compatible), not procedural-only.
5. **Perception**: Posterize + nearest + unlit albedo will **never** match Cycles/Eevee lighting.

## What was already tried (don’t repeat blindly)

- Embedded GLB fallback + `Cache-Control: no-cache` on `index.html`.
- `fetch` with **`RequestCache::NoStore`** for level URLs.
- **`import-glb`** oyabaunctl subcommand.
- Online: remove hard **`y = 0`**, **`y_min`** from level bounds, **`feet_y_on_solids`** (skip tall solids > ~3.2 m height for floor pick), optional **floor slab** at **`bounds.min.y`** when using mesh colliders (`lib.rs` + `gltf_needs_floor_slab`).
- **Far** clip increased (e.g. 220).
- Default **glTF spawn** moved off AABB center-Z toward **min-Z** mouth.
- Relay **spawn XZ** moved toward negative Z for the Tokyo export.

If the user still says **“still all wrong”**, assume **either** the HUD still does not say **`glTF tokyo_alley`**, **or** the GLB is loading but **looks flat / wrong place / wrong scale**, **or** a **coordinate / export** mismatch we have not proven.

## Hypotheses worth proving (priority order)

1. **Confirm which branch loaded**  
   - Browser console: `oyabaun: glTF level … verts … batches` log from `game_init_from_gltf`.  
   - HUD: **`glTF tokyo_alley`**.

2. **glTF content vs Blender**  
   - Parse GLB: node count, mesh count, `images`/`textures`, extensions (`KHR_*`, Draco — **rust `gltf` crate** may not support Draco; this export has not shown Draco in quick greps).  
   - **Blender MCP**: `get_scene_info` — object count, mesh locations; compare orders of magnitude to glTF.

3. **Axis / scale**  
   - Blender MCP object **locations** vs glTF node transforms after export.  
   - If custom rigs/collections/instance collections behave oddly, verify **Apply Transform** and what the official exporter actually puts in the default scene.

4. **Spawn and camera**  
   - With **`OyabaunSpawn`** placed in Blender at a known landmark, re-export and verify spawn in parse (debug log node name hit).  
   - Ensure **first-person eye** is `pos + (0, 1.65, 0)` (`lib.rs` `render_frame`) — feet vs eye confusion when comparing to Blender camera.

5. **Collision solids vs floor**  
   - If **`Collider`** meshes omit the walkable floor, **`feet_y_on_solids`** + slab logic may still mis-place the camera on complex geometry.

## Blender MCP (suggested checks)

If you have **Blender MCP** (`get_scene_info`, `execute_blender_code`, optional screenshot):

1. **`get_scene_info`** — note **object_count**, sample **MESH** `location` ranges (where is the alley relative to origin?).
2. **`execute_blender_code`**: list objects in view layer, check **collection** visibility, **scale**, **apply transform** state, and whether **`OyabaunSpawn`** exists.
3. Export settings: confirm **Y-up**, **+Z forward** glTF convention vs game **`look_at_rh`** / movement (`game.rs`: forward uses `sin/cos` yaw).
4. Optionally export a **minimal test GLB** (one textured plane + one cube) to `client/levels/tokyo_alley.glb` and see if the **client renders it correctly** — isolates “exporter vs parser vs shader”.

## Code touchpoints for fixes

- **Load / boot**: `client/src/lib.rs` (`load_game_init`, `game_init_from_gltf`, `level_label`, warnings).
- **Parse / spawn / colliders**: `client/src/gltf_level.rs` (`parse_glb`, `visit_node`, `is_spawn_name`, `is_collision_name`).
- **Materials / draw**: `client/src/render.rs` (`raster_from_gltf`, `SHADER_WORLD_TEX`).
- **Movement / online**: `client/src/game.rs`, `client/src/net.rs`, `relay/internal/relay/room.go`.
- **Export**: `tools/blender_export_gltf_oyabaun.py`, `tools/blender_export_oyabaun.py` (JSON legacy).

## Suggested fix strategy for the next agent

1. Add **temporary** `wasm_bindgen` **`boot_debug_json()`** (or extend HUD) returning: `level_label`, vertex count, batch count, `bounds min/max`, `spawn`, first fetch error — so testers don’t rely on console alone.  
2. In Blender, add **`OyabaunSpawn`** at a **visible landmark**, re-export, confirm spawn moves in-game.  
3. If materials are the main complaint: **bake** or **image**-based Base Color until `textures` > 0 in the GLB JSON chunk.  
4. If layout is wrong: compare **one mesh** world bounds Blender vs Python-transformed glTF (script in repo or one-off) to detect **Y/Z flip** or **scale 100×**.  
5. Only after (1)–(4): consider **vertex colors**, **double-sided** flags, **alpha**, or **second UV set** (client uses **`tex_coord()` 0** only).

## User workflow reminder

```bash
# From repo root
python3 tools/oyabaunctl.py export-world --blend /path/to/scene.blend
cd client && wasm-pack build --target web --out-dir pkg
python3 tools/oyabaunctl.py launch --force   # or stop + launch
```

Hard-refresh the browser after **`wasm-pack`** (embedded GLB lives inside the WASM).

---

*Last context: players reported persistent mismatch with Blender despite byte-identical GLB on disk vs Desktop export; MCP scene showed ~304 objects and façades near **Z ≈ −26** in Blender space while bbox spans tens of meters — spawn and material export remain the highest-signal levers.*
