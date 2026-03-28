"""
Export a **front-facing character card** (single quad + optional back face) for Oyabaun.

The old two-cube rig + smart_project UVs stretched any texture into a noise blob.
This mesh is a vertical plane in Blender space (**Z up** before glTF `export_yup`),
with UVs 0–1 so `client/sprite1.png` maps as a full-body front view.

Run from repo root:
  /path/to/Blender --background --python tools/blender_make_oyabaun_character.py

Writes client/characters/oyabaun_player.glb (overwrites).
"""
from __future__ import annotations

import math
import os
import sys

import bpy
import bmesh

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
OUT_DIR = os.path.join(ROOT, "client", "characters")
OUT = os.path.join(OUT_DIR, "oyabaun_player.glb")
SPRITE = os.path.join(ROOT, "client", "sprite1.png")

# Authoring: Blender file uses Z-up (matches previous script). glTF export flips to Y-up.
HEIGHT = 1.68
HALF_W = 0.30
# Slight X offset so the sheet sits between feet; faces +X (into the level forward −Z after export).
X_PLANE = 0.02

os.makedirs(OUT_DIR, exist_ok=True)

bpy.ops.wm.read_factory_settings(use_empty=True)

bm = bmesh.new()
# Front face (+X normal): BL, BR, TR, TL — PNG feet at bottom (v=1 in wgpu sample space for bill was bottom; glTF UV v=0 is often image bottom — we match PNG bottom to mesh feet)
uv_layer = bm.loops.layers.uv.new()
v_bl = bm.verts.new((X_PLANE, -HALF_W, 0.0))
v_br = bm.verts.new((X_PLANE, HALF_W, 0.0))
v_tr = bm.verts.new((X_PLANE, HALF_W, HEIGHT))
v_tl = bm.verts.new((X_PLANE, -HALF_W, HEIGHT))
face = bm.faces.new((v_bl, v_br, v_tr, v_tl))
# UV: image bottom (feet) at v=0, top at v=1 — standard OpenGL-style used by glTF
uvs = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
for loop, uv in zip(face.loops, uvs):
    loop[uv_layer].uv = uv

# Thin back face (same UVs) so the card is visible from behind at shallow angles
eps = 0.04
v2_bl = bm.verts.new((X_PLANE - eps, -HALF_W, 0.0))
v2_br = bm.verts.new((X_PLANE - eps, HALF_W, 0.0))
v2_tr = bm.verts.new((X_PLANE - eps, HALF_W, HEIGHT))
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
