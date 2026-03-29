# Blender → Oyabaun (glTF / `.glb`)

The WASM client loads **`client/levels/tokyo_alley.glb`** first (binary glTF with embedded images). If that fetch fails, it falls back to `tokyo_street.json` (vertex-color JSON), then the built-in procedural arena.

## CLI (oyabaunctl)

From the repo root (defaults to **`client/levels/tokyo_alley.blend`** if you omit `--blend`):

```bash
# Export GLB + legacy JSON (default)
python3 tools/oyabaunctl.py export-world

# Other .blend path
python3 tools/oyabaunctl.py export-world --blend /path/to/your_scene.blend

# Regenerate packed albedos, then export (Tokyo alley)
python3 tools/oyabaunctl.py export-world --enhance

# Rebuild every packed texture, then export (full level asset refresh)
python3 tools/oyabaunctl.py export-world --force-all
# equivalent: export-world --enhance --repack

# One-shot: same as --force-all; add --wasm to run wasm-pack after (embedded GLB in bundle)
python3 tools/oyabaunctl.py rebuild-level
python3 tools/oyabaunctl.py rebuild-level --wasm

# Redesign Phase 1 (shop recess + awnings + blade signs; see docs/CURSOR_LEVEL_REDESIGN.md)
python3 tools/oyabaunctl.py redesign-tokyo-phase1
python3 tools/oyabaunctl.py redesign-tokyo-phase1 --export-after

# Albedos only (no GLB write)
python3 tools/oyabaunctl.py enhance-tokyo-alley
python3 tools/oyabaunctl.py enhance-tokyo-alley --repack
```

### Tokyo alley materials (`enhance-tokyo-alley` / `export-world --enhance`)

`tools/blender_enhance_tokyo_alley.py` **removes** legacy collection **`OyabaunTokyoDetail`** (old script-generated street clutter) if present, then assigns **packed 96×96 pixel albedos** (Image Texture → Principled) on materials used by mesh objects so **glTF embeds real textures**. Purely procedural node trees **do not** export to `.glb` (they become white in-game). Re-running skips materials already using **`OyabaunPx_`*** images; use **`enhance-tokyo-alley --repack`** or **`export-world --enhance --repack`** to rebuild them. Skips **`Gun_*`** / **`FPS_*`** so first-person meshes in the same blend keep their factors. It does **not** spawn new props in the lane.

**Art direction:** compare exports against refs in repo-root **`example_images/`** (e.g. `sokes1.png`, `soke*.mp4`).

`export-world` writes **`client/levels/tokyo_alley.glb`** (and by default **`tokyo_street.json`** via the legacy script). Use `--format glb` for glTF only. Set **`BLENDER`** or **`--blender`** if `blender` is not on `PATH` (macOS: path to `Blender.app/Contents/MacOS/Blender`).

## Export from Blender (manual)

1. **Apply scale** on meshes (Ctrl+A → Scale) so transforms are baked.
2. **Materials**: use Principled BSDF with **Base Color** wired to an **Image Texture** (pixel art: small images, e.g. 64–256 px). The runtime uses **nearest** sampling and a 15-step posterize in the fragment shader.
3. **Coordinates**: File → Export → **glTF 2.0**  
   - Format: **GLB**  
   - Include: **Selected Objects** *or* full scene as needed  
   - **Transform**: +Y Up (default glTF; matches the game)
4. Save or copy the file to **`client/levels/tokyo_alley.glb`** next to your static server root (same folder as `index.html` when using `python3 -m http.server` from `client/`).

## Scene objects (names)

| Name | Purpose |
|------|---------|
| **`OyabaunSpawn`** or **`PlayerSpawn`** | Empty. Player feet spawn at this world position (+4 cm Y snap). |
| **`OyabaunCollider` / `Collider`** in node name | Mesh used only for **axis-aligned collision** bounds (per primitive). |
| Visual meshes | Any other names; rendered with textures. |

### Playable character bodies (`oyabaun_player.glb`, `oyabaun_rival.glb`)

NPCs use **authored 3D** meshes (skin-modifier style humanoids, multi-material, no image textures). The Blender Python that builds them is **not** in this repo yet — see **`docs/CHARACTER_PIPELINE_HANDOFF.md`** to recover it from the original author.

`tools/blender_make_oyabaun_character.py` **does not** overwrite these GLBs by default (prevents accidental blocky placeholder exports). Legacy atlas billboard: `OYABAUN_LEGACY_SPRITE=1` with Blender.

After replacing either GLB, run **`wasm-pack build`** from `client/` so `include_bytes!` matches.

The alley export still **removes** legacy **`Boss_*`**, **`Rival_*`**, and **`ACBody*`** blocky meshes so they are not confused with real characters. Set **`OYABAUN_KEEP_PLACEHOLDER_NPCS=1`** if you need those dummies back in Blender for layout only.

**Backdrop**: `reference.png` remains an optional **environment billboard** (mural), separate from character bodies.

If no collider nodes exist, collision falls back to a single AABB around the whole visible mesh (coarse).

## JSON export (legacy)

`tools/blender_export_oyabaun.py` still exports vertex colors to `tokyo_street.json` for fallback testing. It uses a custom **Blender Z-up → game Y-up** remap. **glTF from Blender does not use that remap**; the official exporter already outputs **Y-up** glTF space aligned with the client.

## Pipeline summary

1. **Fetch**: `fetch_bytes("./levels/tokyo_alley.glb")` in `client/src/lib.rs`.
2. **Parse**: `gltf::import_slice` in `client/src/gltf_level.rs` → positions, UVs, indices, PBR base color textures, tangents optional.
3. **GPU**: `Gpu::raster_from_gltf` in `client/src/render.rs` uploads RGBA8 textures, builds a **nearest** `wgpu::Sampler`, and one bind group per draw batch (texture + non-filtering sampler + tint uniform).
4. **Shading**: `SHADER_WORLD_TEX` samples albedo, multiplies material tint, posterizes, then applies the same fog as the flat world shader.

Rebuild the WASM package after changing levels: `wasm-pack build --target web` from `client/`.
