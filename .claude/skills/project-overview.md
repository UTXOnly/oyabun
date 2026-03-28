# Oyabaun — Project Overview

## What
Retro 90s-style first-person WASM/WebGPU game set in a Tokyo alley.

## Stack
- **Rust** → WebAssembly via wasm-pack
- **wgpu** WebGPU rendering (NDC: Y-up, Z range [0,1], `Mat4::perspective_rh`)
- Static site served from `client/`
- Blender for 3D assets → glTF/GLB export
- Preview: `python3 -m http.server 8080` from `client/`

## Key Files
| File | Purpose |
|------|---------|
| `client/src/render.rs` | Shaders (SHADER_WORLD_TEX, SHADER_BILL), render pipeline |
| `client/src/gltf_level.rs` | glTF level parser, spawn extraction, collision |
| `client/src/mesh.rs` | NPC placement, geometry helpers |
| `client/src/boss.rs` | Boss/Rival state (HP, scale, hit radius) |
| `client/src/lib.rs` | WASM entry, game loop, debug endpoints |
| `client/index.html` | Entry point, sprite loading |
| `client/levels/tokyo_alley.glb` | Level + character geometry |
| `client/boss.png` | Boss billboard sprite |
| `client/sprite1.png` | Rival billboard sprite |
| `CHANGELOG.md` | Change log |

## Rendering
- Unlit posterize shader: albedo × tint → 16-level posterize → exponential fog
- Procedural brick/grime/streak detail on dark surfaces (lum < 0.45)
- Height-gradient ambient light for dark Eevee-tuned materials
- Billboard system for sprites with alpha discard + rim lighting

## Characters
- Boss: white suit, hat, dark skin, red flower — Blender Boss_Armature
- Rival: dark jacket, sunglasses, tattoos — Blender Rival_Armature
- 7-bone skeleton: Hips→Spine→Head, Spine→ArmL/ArmR, Hips→LegL/LegR
- 60-frame idle animations
