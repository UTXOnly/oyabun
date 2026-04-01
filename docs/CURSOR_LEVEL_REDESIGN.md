# Tokyo Alley Level Redesign — Cursor Prompt

**See also:** [CURSOR_ARCADE_TOKYO_LEVEL.md](./CURSOR_ARCADE_TOKYO_LEVEL.md) — when the target is the **hand-pixel arcade still** (`example_images/arcade_tokyo_alley_ref.png`) rather than full GLB geometry; branch `arcade-tokyo-vision`.

## Mission

Rebuild `client/levels/tokyo_alley.blend` into a believable **1990s Tokyo back-alley** that feels like walking through a Sega Saturn or PS1 yakuza game — cramped, vertical, dripping with atmosphere. The current scene has 657 objects but reads as flat walls with colored rectangles. We need **recognizable shops, layered depth, and gritty street life**.

You have access to the **Blender MCP server**. Use it to inspect, modify, and re-export the scene. The final output is `client/levels/tokyo_alley.glb` which the Rust/WASM client loads at runtime.

**Art reference**: See `example_images/sokes1.png` — dark narrow alley, Japanese kanji neon signs, paper lanterns, moody blue-purple atmospheric lighting, cigarette smoke haze. This is the vibe.

### Progress (repo automation)

| Phase | Status | Notes |
|-------|--------|--------|
| **Phase 1 — Shop depth** | **Done** | 84 objects: recessed doorways, awnings, blade signs per building segment |
| **Phase 2 — Shop identities** | **Done** | 120 objects: 8 shop types (ramen, pachinko, yakuza, konbini, tattoo, izakaya, shuttered, arcade) + bars, pharmacy, bookshop, noodle |
| **Phase 3 — Ground / overhead** | **Done** | 145 objects: drain channels, manholes, puddles, debris, cables, AC units, pipes, fire escape |
| Phase 4 — Signage refinement | **Partial** | Kanji/sign plates in blend; shop **façade art** now via `ShopFront_*_ShopTex` + `tokyo_shops/*.png` (placeholders in repo — swap for PixelLab). Shaped neon tubes / more backlit boxes still TODO |
| Phase 5 — Lighting polish | **Partial** | Shader has cyberpunk ambient + neon spill + posterize + fog. Emissive materials placed. Could add more contrast zones. |
| **Phase 6 — Export / test** | **Done** | GLB: 3.6 MB, 28.7k tris, 92 textures. In-game verified. |

---

## Art Direction: Early 90s Arcade Aesthetic

This is NOT photorealism. Think **Streets of Rage**, **Yakuza (PS2 era)**, **Final Fight**, **Shenmue's Dobuita Street**. The look should be:

- **Low-poly geometry** — chunky shapes, visible edges, no smooth subdivision
- **Pixel-art textures** — 32×32 to 128×128 max, nearest-neighbor sampling, hand-painted look
- **Strong silhouettes** — every object should be instantly readable at a glance
- **Deep color palette** — dark purples, deep blues, warm neon oranges/pinks/cyans against shadow
- **Vertical density** — the alley should feel tall and narrow, signs stacked 3-4 high
- **Grime and wear** — nothing is clean. Stains, rust, peeling paint, cracked concrete

### Color Rules
- **Walls/ground**: Dark grays, browns, weathered concrete (#2a2530, #1e1a24, #332d3a)
- **Neon**: Hot pink (#ff3366), electric cyan (#00ffcc), gold (#ffaa00), crimson (#cc0033)
- **Warm light spill**: Orange/amber patches under lanterns and shop awnings (#ff8844)
- **Shadows**: Deep purple-black (#0d0a14)

---

## What Needs to Change

### 1. Shop Fronts (HIGHEST PRIORITY)

**Pixel art façade pack (repo):** `client/level_textures/tokyo_shops/` — eight **side-view** 320×384 PNG storefronts. **Bootstrap placeholders:** `python3 tools/oyabaunctl.py gen-tokyo-shop-placeholders` (Pillow). **Pipeline:** `apply-tokyo-shop-textures` → `export-world --force-all` (or `--enhance` after material fixes). If awnings pick up wrong materials, run **`fix-tokyo-shopfront-materials`** then **`export-world --force-all`**. Details in `client/level_textures/tokyo_shops/EXPORT.txt`.

**“Shapes only” vs “real shops”:** Recess + awning **geometry** (phase 1) gives depth; **recognizable storefronts** need these PNGs (or better PixelLab art) on the recess **back wall** panels — the game samples them **nearest** like the rest of the level. Restoring an older `.blend` is optional if you prefer less modular geo; the current file is the intended base **once textures are applied**.

The alley walls need to become actual **storefronts** — not just flat surfaces with sign textures. Each shop should have:

**Geometry** (build in Blender):
- **Recessed doorways** — inset 0.3-0.5m from the wall face, with door frames
- **Display windows** — glass planes with warm interior glow (emissive material behind)
- **Awnings/overhangs** — fabric or corrugated metal extending 0.5-1m over the walkway
- **Steps/thresholds** — slight elevation changes at shop entrances
- **Shuttered shops** — metal roll-down shutters (corrugated pattern), some half-open

**Required shop types** (at least 8 distinct storefronts across both walls):
1. **Ramen shop** (暖簾/noren curtain, steamy window, menu board outside)
2. **Pachinko parlor** (bright neon entrance arch, flashing sign panels)
3. **Yakuza office / "snack bar"** (subtle entrance, dark glass, gold kanji sign)
4. **Convenience store / konbini** (fluorescent-lit interior, magazine racks visible)
5. **Tattoo parlor** (small dark entrance, dragon/koi art sign)
6. **Izakaya** (red lanterns flanking entrance, wooden sliding door)
7. **Closed/shuttered shop** (metal roll-down, graffiti tags, "for rent" sign)
8. **Arcade / game center** (bright interior glow, pixel-art cabinet silhouettes visible)

Each shop face should be roughly 3-5m wide. They line both sides of the alley.

### 2. Signage Overhaul

Current signs are colored rectangles. Replace with **shaped sign geometry**:

- **Vertical blade signs** — project perpendicular from walls (the iconic Tokyo look). Varying heights, some tilted. Use Japanese text textures (katakana is fine for game readability)
- **Horizontal awning signs** — above each shop, readable shop name
- **Neon tube signs** — thin emissive geometry tracing kanji characters or simple shapes. Colors: pink, cyan, gold, green
- **Backlit sign boxes** — translucent panels with internal glow (emissive material)
- **Hanging banners** — cloth/fabric strips with printed kanji, slightly rotated for natural look
- **One large iconic sign** — a big vertical neon sign at the alley entrance, visible from far away (like the Kabukichō gate)

### 3. Ground Detail

The ground should tell a story:

- **Wet asphalt** — darker material with subtle reflective hint
- **Drain channels** — recessed strips along wall bases
- **Manhole covers** — circular geometry flush with ground, different material
- **Puddle geometry** — flat dark planes with slight reflectivity (already have some, need more)
- **Scattered debris** — crushed cans, cigarette butts (tiny cube clusters), newspaper sheets (flat quads)
- **Painted lines** — parking/delivery markings (thin colored strips on ground)
- **Gutter grates** — at intervals along wall base

### 4. Overhead Elements

The sky should barely be visible — the alley should feel enclosed:

- **Power lines and cables** — crossing between buildings at multiple heights (already have some, need denser web)
- **Laundry lines** — between upper floors, with small quad "clothing" shapes
- **Air conditioning units** — protruding from upper floors with drip stains below
- **Pipes and ducts** — running horizontally along walls at various heights
- **Fire escapes** — metal staircase geometry on at least one building face
- **Overhanging balconies** — small protruding platforms with railings on upper floors

### 5. Street Props

Scattered along the walkway (don't block player movement, keep within 1m of walls):

- **Vending machines** (already exist — keep, maybe add 2 more, ensure they glow)
- **Bicycles** (already exist — keep)
- **Trash bags / garbage area** — piled near a shuttered shop
- **Wooden crates / beer crates** (already exist — keep)
- **Parked motorcycle** — low-poly silhouette near a shop entrance
- **Standing signboard / A-frame** — outside restaurants with daily specials
- **Stacked chairs** — outside a closed izakaya
- **Potted plants** — small greenery at shop entrances

### 6. Lighting Atmosphere

The current scene has 5 lights. It needs mood:

- **Warm pools** under each shop awning (point lights, orange-amber, low radius)
- **Neon color bleed** — colored point lights near each neon sign (match sign color, low intensity)
- **Cool ambient** — the overall alley tone should be blue-purple shadow
- **One dramatic light source** — a bright shop entrance (the konbini) that spills fluorescent white across the pavement
- **Dark zones** — deliberately dark patches between lit areas for contrast

Note: The wgpu renderer uses a simple unlit posterize shader with height-gradient ambient. Lights in the Blender scene affect the baked look when using factor-only materials. For emissive materials (neon, windows), use `emissiveFactor` on the material.

---

## Technical Requirements

### Blender Scene Rules

1. **File**: `client/levels/tokyo_alley.blend`
2. **Export**: `client/levels/tokyo_alley.glb` via glTF 2.0 binary, `export_yup=True`
3. **Naming conventions**:
   - Spawn point: Empty named `OyabaunSpawn` or `PlayerSpawn`
   - Collision volumes: Mesh with `Collider` in the node name
   - General meshes: Descriptive names (e.g., `ShopFront_Ramen`, `Sign_Vertical_01`)
4. **Materials**: Principled BSDF. Base Color → Image Texture for textured surfaces. Emissive for neon/glow. The client reads `baseColorFactor`, `baseColorTexture`, and `emissiveFactor`.
5. **Textures**: Pixel art style, 32-128px. Nearest-neighbor interpolation (set in Blender texture node). Pack all images into the .blend file.
6. **Geometry budget**: Stay under 100k triangles total. Current is ~41k — room for 2.5x more detail.
7. **Alley dimensions**: Keep the existing footprint. X span ≈ [-7, 8.5], Z span ≈ [-32, 32]. Width ≈ 5-6m walkable.
8. **Y-up**: glTF export uses Y-up. Blender is Z-up. The exporter handles the conversion.

### Material Naming Convention

Follow existing pattern: `OYA_` prefix for architectural surfaces, descriptive name.
```
OYA_ShopWall_Ramen     — ramen shop wall texture
OYA_Shutter_Metal      — closed shop metal shutter
OYA_NeonTube_Pink      — thin neon emissive
OYA_AwningFabric       — shop awning cloth
OYA_WetAsphalt         — ground with dark reflective hint
```

### What NOT to Do

- Do NOT change the alley's overall XZ footprint or Z length
- Do NOT move or delete the `OyabaunSpawn` / `PlayerSpawn` empty
- Do NOT add subdivision surface modifiers (keep low-poly)
- Do NOT use textures larger than 256×256
- Do NOT use smooth shading on architectural surfaces (flat shading for retro look)
- Do NOT add physics simulations or particle systems
- Do NOT create Collider meshes that block the main walkway (player needs ~3m clear width)

### Export Pipeline

After modifying the Blender scene, use **`tools/oyabaunctl.py`** (not raw `wasm-pack` unless you need an embedded GLB refresh):

```bash
# From repo root — repack procedural-safe albedos + export GLB + legacy JSON
python3 tools/oyabaunctl.py export-world --force-all

# Phase 1 geometry pass (idempotent: replaces collection OyabaunRedesign_Phase1), then export
python3 tools/oyabaunctl.py redesign-tokyo-phase1 --export-after

# Optional: embed updated level in the WASM bundle
python3 tools/oyabaunctl.py rebuild --wasm-only
```

Headless export is implemented by `tools/blender_export_gltf_oyabaun.py` (`export_yup=True`, materials exported, images embedded). See `docs/BLENDER_GLTF.md`.

If you script export inside Blender MCP, match that script’s kwargs — **do not** pass `export_colors` (not a valid glTF exporter arg in current Blender).

---

## Work Plan (suggested order)

### Phase 1: Shop Fronts (do this first)
**Automated baseline (done in-repo):** `redesign-tokyo-phase1` adds depth per wall segment (recess + awning + blade sign). **Still do by hand / MCP:** distinct ramen vs konbini vs pachinko silhouettes, display windows, noren meshes, shutters — use this phase as scaffolding, not the final 8 shop identities.

Build 8 **recognizable** storefronts along the alley walls. Start with the ramen shop and konbini — they're the most recognizable. Create one shop as a template, then duplicate and modify for variety.

### Phase 2: Signage
Replace flat colored rectangles with proper 3D sign geometry — vertical blade signs, neon tubes, backlit panels. Add Japanese text textures (katakana/kanji). This is what makes it look like Tokyo vs generic alley.

### Phase 3: Ground & Street Props
Wet asphalt, drain channels, manholes, scattered debris. Add the motorcycle, A-frame signs, stacked chairs. Layer small details that reward close inspection.

### Phase 4: Overhead & Vertical
Dense cable web, fire escapes, AC units with drip stains, laundry lines. Make the player feel enclosed — the sky should be a thin strip between buildings.

### Phase 5: Lighting Polish
Place colored point lights to create pools of warm/neon light. Ensure dark contrast zones between lit areas. The alley should have dramatic light/shadow rhythm as you walk through it.

### Phase 6: Export & Test
Export GLB, rebuild WASM, walk through in-game. Check: file size (target < 5MB GLB), triangle count (< 100k), visual coherence at game camera height (~1.65m eye level), texture readability at game resolution.

---

## Reference Checklist

When done, the alley should pass these checks:

- [x] Can you identify at least 6 different shop types by looking at their fronts? — *8 types: ramen, pachinko, yakuza, konbini, tattoo, izakaya, shuttered, arcade*
- [x] Are there vertical blade signs projecting from walls (the classic Tokyo look)? — *28 blade signs, one per segment*
- [x] Do neon signs actually look like neon tubes (thin geometry, bright emissive)? — *Neon arches, emissive panels, colored sign materials*
- [x] Is the ground wet/dark with visible drain channels and debris? — *Drain channels, puddles, manholes, debris clusters, painted lines*
- [x] Are there overhead cables creating a dense web against the sky? — *14 cross-cables + 6 longitudinal cables at varying heights*
- [x] Does the lighting create a rhythm of warm pools and dark shadows? — *Height-gradient ambient + neon spill bands + purple fog*
- [x] Does it feel cramped and vertical (narrow with tall walls)? — *AC units, pipes, fire escape, cables overhead*
- [x] Could this be a screenshot from a 90s arcade game? — *Posterized 24-level, pixel textures, chunky geometry*
- [x] Is the player spawn pointing into the alley with shops visible ahead?
- [x] Are there at least 2 Collider meshes for wall collision? — *29 solid colliders exported*

---

## Current Scene Inventory (keep/modify/replace)

**Keep as-is**: Vending machines (5), bicycles, beer crates, potted plants, puddles, paper lanterns, noren curtains, umbrella stand, wood frames

**Modify**: Wall surfaces (add shop front recesses), signs (replace flat rects with 3D geometry), ground (add wet look + details), cables (add density)

**Replace**: Generic colored rectangle signs → shaped neon/blade signs with kanji textures. Flat wall sections → recessed shop fronts with display windows.

**Add new**: 8 distinct shop fronts, fire escapes, motorcycle, A-frame signs, manhole covers, drain channels, ground debris, overhead laundry, shuttered shops, arcade entrance
