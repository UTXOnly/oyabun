# Blender → Oyabaun (glTF / `.glb`)

The WASM client loads **`client/levels/tokyo_alley.glb`** first (binary glTF with embedded images). If that fetch fails, it falls back to `tokyo_street.json` (vertex-color JSON), then the built-in procedural arena.

## CLI (oyabaunctl)

From the repo root:

```bash
python3 tools/oyabaunctl.py export-world --blend /path/to/your_scene.blend
```

### Optional: bulk Tokyo street props (Blender-only)

`tools/blender_enhance_tokyo_alley.py` fills **`client/levels/tokyo_alley.blend`** with collection **`OyabaunTokyoDetail`** (sign boards, neon strips, awnings, wall AC blocks, conduits, vending shells, lanterns, planters, hydrants). It uses the scene’s existing mesh **world AABB** for placement. Re-running replaces that collection.

```bash
/path/to/Blender client/levels/tokyo_alley.blend --background --python tools/blender_enhance_tokyo_alley.py
python3 tools/oyabaunctl.py export-world --blend client/levels/tokyo_alley.blend
```

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

### Playable character body (`oyabaun_player.glb`)

Multiplayer and local boss/rival use a **shared 3D character mesh** (`client/characters/oyabaun_player.glb`), not rigged dolls in the alley GLB. Texture it with PixelLab output in Blender, then export GLB. Regenerate the default placeholder with:

```bash
/Applications/Blender.app/Contents/MacOS/Blender --background --python tools/blender_make_oyabaun_character.py
```

(Adjust the path to `Blender` on your OS.) The page may fetch `./characters/oyabaun_player.glb` after load; the WASM bundle also **embeds** a copy via `include_bytes!`.

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
