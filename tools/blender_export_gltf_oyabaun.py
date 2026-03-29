"""
Run inside Blender (invoked by oyabaunctl export-world):
  blender scene.blend --background --python tools/blender_export_gltf_oyabaun.py

Requires env **OYABAUN_GLB_OUT** = absolute path to output `.glb`.

NPCs (boss, rival, arcade dummies) are drawn in-engine as **billboards** textured from
`client/*.png` (PixelLab / hand-painted), not as rigged meshes in the GLB. Before export
we delete legacy placeholder rigs so the alley is not full of blocky 3D figures.

Set **OYABAUN_KEEP_PLACEHOLDER_NPCS=1** to skip deletion (layout/debug only).
"""
from __future__ import annotations

import os
import sys

import bpy

out = os.environ.get("OYABAUN_GLB_OUT")
if not out:
    print("oyabaun: set OYABAUN_GLB_OUT to the output .glb path", file=sys.stderr)
    sys.exit(1)

out = os.path.abspath(out)
os.makedirs(os.path.dirname(out) or ".", exist_ok=True)


def _strip_placeholder_npcs() -> int:
    prefixes = ("Boss_", "Rival_", "ACBody")
    victims = [
        o
        for o in bpy.data.objects
        if o.type != "CAMERA" and any(o.name.startswith(p) for p in prefixes)
    ]
    if not victims:
        return 0
    bpy.ops.object.select_all(action="DESELECT")
    for o in victims:
        o.select_set(True)
    bpy.context.view_layer.objects.active = victims[0]
    bpy.ops.object.delete(use_global=False)
    return len(victims)


if not os.environ.get("OYABAUN_KEEP_PLACEHOLDER_NPCS"):
    n = _strip_placeholder_npcs()
    if n:
        print(f"oyabaun: removed {n} placeholder NPC mesh objects (use billboards + PNGs in client)")
    fp = bpy.data.filepath
    if fp:
        bpy.ops.wm.save_mainfile()

# Blender 3.4+ / 4.x (kwargs vary slightly; keep defaults where possible)
bpy.ops.export_scene.gltf(
    filepath=out,
    export_format="GLB",
    export_materials="EXPORT",
    export_texcoords=True,
    export_normals=True,
    export_apply=True,
    export_yup=True,
    use_selection=False,
    export_animations=False,
)

print(f"oyabaun: wrote {out}")
