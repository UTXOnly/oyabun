# Blender → glTF Export Skill

## Coordinate System
- Blender: Z-up. glTF/wgpu: Y-up.
- Export with `export_yup=True, export_apply=True`
- Blender (X, Y, Z) → glTF (X, Z, -Y)

## Export Command
```python
import bpy
bpy.ops.export_scene.gltf(
    filepath="/Users/brianhartford/Desktop/oyabaun/client/levels/tokyo_alley.glb",
    export_format='GLB',
    export_yup=True,
    export_apply=True,
    export_animations=True,
    export_skins=True,
)
```

## Critical Rules
- **NEVER** use `dpdx`/`dpdy` WGSL builtins — causes SILENT pipeline failure in this wgpu setup
- **NEVER** use `export_colors` parameter — not recognized in this Blender version
- After export, the browser caches .wasm/.glb aggressively — use `?v=timestamp` cache buster
- Materials with black baseColor but non-zero emissive use emissive as tint
- Factor-only materials (no textures) get 2.8x brightness boost in renderer
- All meshes must have vertex groups matching armature bones if skinned

## Spawn Point
- Empty named "PlayerSpawn" in Blender
- Position and rotation extracted; yaw from world transform
- Currently at Blender (0.77, -18, 1) rotated 180° to face bosses

## Collision
- No explicit colliders → thin floor slab fallback at bounds.min.y
- Add "Collider"-named meshes in Blender for proper wall collision
