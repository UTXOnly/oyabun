# Character art pipeline — handoff for Claude (or whoever regenerates meshes)

## What went wrong (March 2026)

The **canonical** boss/rival meshes are **organic skin-modifier humanoids** (~1.7k verts, 11 materials), committed as binary GLBs in:

- `client/characters/oyabaun_player.glb`
- `client/characters/oyabaun_rival.glb`

The **Python source that built those meshes was never added to git.** Only the `.glb` files and the high-level description in `.claude/skills/blender-characters.md` / `CHANGELOG.md` were committed (see git `4437bd8`, `54b228e`; co-authored Claude Opus 4.6).

A later change replaced the tool script with **stacked axis-aligned boxes** and overwrote the GLBs, which reads as “block people” and does not match the intended yakuza / `example_images` look.

**Repo state fix:** GLBs were restored from git commit **`54b228e`** (“Add detailed facial features…”). `tools/blender_make_oyabaun_character.py` no longer overwrites them by default.

---

## What we need from you (Claude) to make this maintainable

Please reply in a **single paste-friendly block** (or a gist) with as many of the following as you still have:

### 1. Full Blender Python generator

- The complete script that builds **boss** and **rival** using:
  - skin modifier skeleton (joints, edges, per-joint radii),
  - subdivision + decimate,
  - material assignment (by height / face center),
  - detail meshes (glasses, gun, katana, neon, tie, hair, scar, etc.),
  - join → `export_scene.gltf(..., export_yup=True)`.

If boss and rival were two script variants or one script with a `variant` flag, show both paths.

### 2. Reference art

- Where are the **example images** the user judged against (paths or filenames)? They are **not** in this repo (no `example_images/` here). If they live only in chat, please list what each image was supposed to lock (silhouette, palette, hair, weapons).

### 3. Exact Blender version

- Blender version used when the good GLBs were exported (e.g. 4.2 / 5.1). Any exporter options beyond `export_yup=True` that mattered.

### 4. Regeneration commands

- Exact env vars and command line you used, e.g.  
  `OYABAUN_OUT=... Blender --background --python ...`

### 5. Rival vs boss differences

- Parameter deltas only (scale, radii dict diff, extra objects list) if the script is shared.

### 6. Material names / slot order

- Material names in Blender (must stay compatible with batching / debugging). Current GLB uses ~11 slots each; if names are meaningful, list them.

---

## Technical constraints (unchanged)

- Game loads glTF with `parse_character_glb` — multiple primitives / materials = multiple draw batches (keep **≤ ~15** materials).
- **No image textures required** today: Principled **base color** (+ **emission** for neon). Shader multiplies sampled tex × tint; untextured prims use a white 1×1 internally.
- **Feet at Z = 0** in Blender; **front faces −Y**; `export_yup=True` for glTF.
- `character_model()` in `client/src/lib.rs` uses **`yaw + π`** for Blender → game facing — do not rotate the mesh inconsistently with that without a coordinated Rust change.

---

## Optional improvements (after script is back in repo)

- Commit the generator as e.g. `tools/blender_build_oyabaun_characters_3d.py` and wire `OYABAUN_VARIANT=boss|rival|all`.
- Add `wasm-pack build` note to `docs/BLENDER_GLTF.md` after regenerating GLBs.
- If example images are meant to be shared, add them under `example_images/` (repo policy permitting).

---

## Git references

| Commit     | Notes |
|-----------|--------|
| `4437bd8` | Introduced 3D skin-modifier pipeline + task doc; GLBs replaced sprite quads. |
| `54b228e` | Richer detail (fingers, hair spikes, katana wrap, etc.); **last good GLBs before block replacement**. |
| `12c980f` | Blocky procedural replacement (reverted in working tree for binaries; do not use as art direction). |

---

## Questions checklist (short)

1. Paste the **full** skin-modifier build script(s) for boss and rival.  
2. Where are the **example_images** (or describe their filenames if only in chat)?  
3. **Blender version** + any non-default glTF export settings?  
4. Any **manual** steps after the script (sculpt, proportional edit, vertex paint)?  
5. Should **neon** stay emissive-only or do you want separate unlit pass later?
