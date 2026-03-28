"""
Run inside Blender (invoked by oyabaunctl export-world):
  blender scene.blend --background --python tools/blender_export_gltf_oyabaun.py

Requires env **OYABAUN_GLB_OUT** = absolute path to output `.glb`.
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
