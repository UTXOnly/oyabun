"""
Build a simple Oyabaun player body mesh (feet at origin, +Y up) and export GLB.

PixelLab / hand-painted textures: assign in Blender to this mesh, then re-run export.
Run from repo root:
  /path/to/Blender --background --python tools/blender_make_oyabaun_character.py

Writes client/characters/oyabaun_player.glb (overwrites).
"""
from __future__ import annotations

import os
import sys

import bpy

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
OUT_DIR = os.path.join(ROOT, "client", "characters")
OUT = os.path.join(OUT_DIR, "oyabaun_player.glb")
SPRITE = os.path.join(ROOT, "client", "sprite1.png")

os.makedirs(OUT_DIR, exist_ok=True)

bpy.ops.wm.read_factory_settings(use_empty=True)

# Body: ~1.65m tall, origin between feet (Y-up Blender)
bpy.ops.mesh.primitive_cube_add(location=(0, 0, 0.55))
body = bpy.context.active_object
body.name = "OyabaunCharacter"
body.scale = (0.24, 0.2, 0.55)
bpy.ops.object.transform_apply(scale=True)

bpy.ops.mesh.primitive_cube_add(location=(0, 0, 1.22))
head = bpy.context.active_object
head.name = "OyabaunCharacterHead"
head.scale = (0.2, 0.18, 0.22)
bpy.ops.object.transform_apply(scale=True)

bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
head.select_set(True)
bpy.context.view_layer.objects.active = body
bpy.ops.object.join()
body.name = "OyabaunCharacter"

bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.mesh.select_all(action="SELECT")
bpy.ops.uv.smart_project(angle_limit=66.0)
bpy.ops.object.mode_set(mode="OBJECT")

mat = bpy.data.materials.new(name="OyabaunCharMat")
mat.use_nodes = True
nodes = mat.node_tree.nodes
links = mat.node_tree.links
nodes.clear()
out = nodes.new("ShaderNodeOutputMaterial")
prin = nodes.new("ShaderNodeBsdfPrincipled")
tex = nodes.new("ShaderNodeTexImage")
if os.path.isfile(SPRITE):
    tex.image = bpy.data.images.load(SPRITE)
else:
    tex.image = None
links.new(tex.outputs["Color"], prin.inputs["Base Color"])
links.new(prin.outputs["BSDF"], out.inputs["Surface"])
body.data.materials.append(mat)

# Face -Y as forward (matches game forward / glTF -Z camera convention after export)
body.rotation_euler = (0, 0, 0)

bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
bpy.context.view_layer.objects.active = body

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
