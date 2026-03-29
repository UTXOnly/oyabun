"""
Export an **8-direction character card** for Oyabaun.

The atlas is 8 columns × N rows (row 0 = idle, rows 1+ = walk frames).
Each cell is 64×crop_h pixels. The shader selects the right column (direction)
and row (animation frame) at runtime via ATLAS_ROWS.

Quad mesh is in Blender Z-up space; glTF export flips to Y-up.
Feet sit at Z=0 so the model matrix foot_y places them on the ground.

Run from repo root:
  /path/to/Blender --background --python tools/blender_make_oyabaun_character.py

Environment variables:
  OYABAUN_OUT     — output GLB path (default: client/characters/oyabaun_player.glb)
  OYABAUN_SPRITE  — atlas PNG path (default: client/sprite1.png)

Writes client/characters/oyabaun_player.glb (overwrites).
"""
from __future__ import annotations

import os
import sys

import bpy
import bmesh

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
OUT_DIR = os.path.join(ROOT, "client", "characters")
OUT = os.environ.get("OYABAUN_OUT", os.path.join(OUT_DIR, "oyabaun_player.glb"))
SPRITE = os.environ.get(
    "OYABAUN_SPRITE", os.path.join(ROOT, "client", "sprite1.png")
)

# Atlas layout: 8 columns × ATLAS_ROWS rows.
# Each cell is 64 × cell_h pixels; total atlas is 512 × (cell_h * ATLAS_ROWS).
# The quad aspect matches one cell (not the full atlas), since the shader
# scales UVs by 1/8 horizontally and 1/ATLAS_ROWS vertically.
ATLAS_ROWS = 7  # 1 idle + 6 walk frames
# Cell aspect at 64×50: 64/50 = 1.28. At HEIGHT=1.68m, width = 2.15m.
# Character is ~30% of cell width, so visual width ≈ 0.65m.
CELL_W = 64
CELL_H = 50  # cropped height per cell (boss atlas)
HEIGHT = 1.68
HALF_W = HEIGHT * (float(CELL_W) / float(CELL_H)) / 2.0
X_PLANE = 0.02

os.makedirs(OUT_DIR, exist_ok=True)

bpy.ops.wm.read_factory_settings(use_empty=True)

bm = bmesh.new()
uv_layer = bm.loops.layers.uv.new()

# Front face (+X normal in Blender → −Z normal in glTF Y-up)
v_bl = bm.verts.new((X_PLANE, -HALF_W, 0.0))
v_br = bm.verts.new((X_PLANE,  HALF_W, 0.0))
v_tr = bm.verts.new((X_PLANE,  HALF_W, HEIGHT))
v_tl = bm.verts.new((X_PLANE, -HALF_W, HEIGHT))
face = bm.faces.new((v_bl, v_br, v_tr, v_tl))
# UVs span entire atlas (0,0)→(1,1). Shader selects column.
uvs = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
for loop, uv in zip(face.loops, uvs):
    loop[uv_layer].uv = uv

# Back face (thin offset for rear visibility)
eps = 0.04
v2_bl = bm.verts.new((X_PLANE - eps, -HALF_W, 0.0))
v2_br = bm.verts.new((X_PLANE - eps,  HALF_W, 0.0))
v2_tr = bm.verts.new((X_PLANE - eps,  HALF_W, HEIGHT))
v2_tl = bm.verts.new((X_PLANE - eps, -HALF_W, HEIGHT))
face2 = bm.faces.new((v2_br, v2_bl, v2_tl, v2_tr))
for loop, uv in zip(face2.loops, uvs):
    loop[uv_layer].uv = uv

mesh = bpy.data.meshes.new("OyabaunCharacter")
bm.to_mesh(mesh)
bm.free()
obj = bpy.data.objects.new("OyabaunCharacter", mesh)
bpy.context.collection.objects.link(obj)
bpy.context.view_layer.objects.active = obj
obj.select_set(True)

mat = bpy.data.materials.new(name="OyabaunCharMat")
mat.use_nodes = True
nodes = mat.node_tree.nodes
links = mat.node_tree.links
nodes.clear()
out = nodes.new("ShaderNodeOutputMaterial")
prin = nodes.new("ShaderNodeBsdfPrincipled")
tex = nodes.new("ShaderNodeTexImage")
if os.path.isfile(SPRITE):
    tex.image = bpy.data.images.load(SPRITE, check_existing=True)
else:
    tex.image = None
links.new(tex.outputs["Color"], prin.inputs["Base Color"])
if tex.image:
    links.new(tex.outputs["Alpha"], prin.inputs["Alpha"])
prin.inputs["Roughness"].default_value = 0.85
mat.blend_method = "BLEND" if tex.image else "OPAQUE"
links.new(prin.outputs["BSDF"], out.inputs["Surface"])
mesh.materials.append(mat)

bpy.ops.object.select_all(action="DESELECT")
obj.select_set(True)
bpy.context.view_layer.objects.active = obj

bpy.ops.export_scene.gltf(
    filepath=OUT,
    export_format="GLB",
    export_materials="EXPORT",
    export_texcoords=True,
    export_normals=True,
    export_apply=True,
    export_yup=True,
    use_selection=True,
    export_animations=False,
)

print(f"oyabaun: wrote {OUT}", file=sys.stderr)
