# Character Art Skill — PixelLab Pixel Art Sprites

## DEPRECATED: Blender 3D skin-modifier characters (2026-03-29)

The 3D procedural approach failed to match the neo-noir pixel art reference style. Characters are now **PixelLab pro-mode pixel art sprites** rendered as camera-facing billboard quads.

## Current Pipeline

```
PixelLab MCP create_character (pro mode, ~112px canvas, 8 dirs)
    → animate walk (MCP or PixelLab web if MCP string-args break)
    → download ZIP → extract
    → ~/Desktop/oyabaun-characters/tools/build_game_atlas.py
    → atlas PNG → oyabaun tools/export_character_atlas_to_rgba.py → .rgba
    → client/characters/<name>_atlas.rgba → include_bytes!()
    → billboard quads in draw_world() → SHADER_BILL
```

## Active PixelLab Characters (v3 pro)

- **Boss** `d5ceb30a-0a4b-49c4-8ccb-988898cb8135` — 112×112, walk anim, ✅ in-game
- **Rival** `dabe33dd-b9d5-481c-9413-402cd0002747` — 116×116, rotations only (add walk on PixelLab web for a rival-specific atlas later)
- **Player** `fe8d4102-8926-4267-ab1c-4600441cfcf4` — 104×104, rotations only

## Character Art Repo

`~/Desktop/oyabaun-characters/` — reference images, prompts, atlas tools, export pipeline.

## Atlas Format

Binary `.rgba`: `[u32 LE width][u32 LE height][RGBA pixels]`
Grid: 8 cols (S/SW/W/NW/N/NE/E/SE) × N rows (row 0 = idle, rows 1+ = animation frames)

## Key Files

| File | Purpose |
|------|---------|
| `client/src/render.rs` | char_sprite_bg, billboard quad gen, SHADER_BILL |
| `client/characters/boss_v3_atlas.rgba` | Boss sprite atlas (embedded) |
| `~/Desktop/oyabaun-characters/` | Art production repo |

## Key Constraint

Characters must be generated WITH weapons visible in sprites. Do not create floating weapon sprites.
