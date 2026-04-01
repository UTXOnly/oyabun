"""
Import m4a13d/base.obj (heavy mesh), decimate, then:
  - Render 512×512 RGBA PNG for FPS HUD (client/fpsweapons/m4a1.png)
  - Export low-poly GLB (client/props/m4a1_prop.glb) for optional world props

Run from repo root (needs Blender on PATH):

  blender --background --python tools/blender_m4a1_export_assets.py

Override input:

  OYABAUN_M4_OBJ=/path/to.obj blender --background --python tools/blender_m4a1_export_assets.py
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OBJ = ROOT / "m4a13d" / "base.obj"
OUT_PNG = ROOT / "client" / "fpsweapons" / "m4a1.png"
OUT_GLB = ROOT / "client" / "props" / "m4a1_prop.glb"


def main() -> None:
    obj_path = Path(os.environ.get("OYABAUN_M4_OBJ", str(DEFAULT_OBJ))).resolve()
    if not obj_path.is_file():
        print(f"blender_m4a1: missing OBJ: {obj_path}", file=sys.stderr)
        sys.exit(1)

    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.wm.obj_import(filepath=str(obj_path))

    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    if not meshes:
        print("blender_m4a1: no mesh after import", file=sys.stderr)
        sys.exit(1)

    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()

    ob = bpy.context.active_object
    mod = ob.modifiers.new("Decimate", type="DECIMATE")
    mod.ratio = 0.018
    bpy.ops.object.modifier_apply(modifier="Decimate")

    mat = bpy.data.materials.new("M4A1_Mat")
    mat.use_nodes = True
    mat.blend_method = "OPAQUE"
    nt = mat.node_tree
    pr = nt.nodes.get("Principled BSDF")
    if pr:
        pr.inputs["Base Color"].default_value = (0.22, 0.24, 0.26, 1.0)
        pr.inputs["Roughness"].default_value = 0.45
        pr.inputs["Metallic"].default_value = 0.55
    if ob.data.materials:
        ob.data.materials[0] = mat
    else:
        ob.data.materials.append(mat)

    # Frame model
    bpy.ops.object.origin_set(type="ORIGIN_GEOMETRY", center="BOUNDS")
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    dims = ob.dimensions
    max_d = max(dims.x, dims.y, dims.z, 0.001)

    # Camera: side-ish view for first-person sprite (barrel toward +X screen right)
    cam_data = bpy.data.cameras.new("Cam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = max_d * 1.35
    cam_ob = bpy.data.objects.new("Cam", cam_data)
    bpy.context.scene.collection.objects.link(cam_ob)
    bpy.context.scene.camera = cam_ob

    center = sum((ob.matrix_world @ Vector(corner) for corner in ob.bound_box), Vector()) / 8.0
    cam_ob.location = center + Vector((max_d * 1.8, -max_d * 0.15, max_d * 0.45))
    direction = center - cam_ob.location
    cam_ob.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()

    bpy.ops.object.light_add(type="AREA", location=center + Vector((max_d * 2.0, max_d * 1.2, max_d * 1.5)))
    L = bpy.context.active_object
    L.data.energy = 800.0
    L.data.size = max_d * 3.0

    bpy.ops.object.light_add(type="AREA", location=center + Vector((-max_d * 1.2, -max_d * 1.5, max_d * 0.8)))
    L2 = bpy.context.active_object
    L2.data.energy = 120.0
    L2.data.color = (0.85, 0.92, 1.0)

    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = 512
    scene.render.resolution_y = 512
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "8"

    OUT_PNG.parent.mkdir(parents=True, exist_ok=True)
    scene.render.filepath = str(OUT_PNG.with_suffix(""))
    bpy.ops.render.render(write_still=True)
    print(f"blender_m4a1: wrote {OUT_PNG}")

    OUT_GLB.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=str(OUT_GLB),
        export_format="GLB",
        use_selection=False,
        export_apply=True,
        export_yup=True,
    )
    print(f"blender_m4a1: wrote {OUT_GLB}")


if __name__ == "__main__":
    main()
