"""
Headless Blender: import character GLB and render a PNG for style audit.

Usage:
  /path/to/Blender --background --python tools/blender_character_capture.py -- \\
      client/characters/oyabaun_player.glb tools/_audit_out/boss_render.png

Requires Blender 5.x with glTF importer enabled (built-in).
"""
from __future__ import annotations

import math
import os
import sys

import bpy
from mathutils import Vector


def _argv_after_dd() -> list[str]:
    if "--" in sys.argv:
        return sys.argv[sys.argv.index("--") + 1 :]
    return []


def scene_world_bounds():
    min_v = Vector((1e9, 1e9, 1e9))
    max_v = Vector((-1e9, -1e9, -1e9))
    found = False
    for obj in bpy.context.scene.objects:
        if obj.type != "MESH":
            continue
        found = True
        for corner in obj.bound_box:
            w = obj.matrix_world @ Vector(corner)
            min_v.x, min_v.y, min_v.z = min(min_v.x, w.x), min(min_v.y, w.y), min(min_v.z, w.z)
            max_v.x, max_v.y, max_v.z = max(max_v.x, w.x), max(max_v.y, w.y), max(max_v.z, w.z)
    if not found:
        min_v = Vector((-0.35, -0.35, 0.0))
        max_v = Vector((0.35, 0.35, 1.85))
    center = (min_v + max_v) * 0.5
    size = max_v - min_v
    return min_v, max_v, center, size


def main():
    args = _argv_after_dd()
    if len(args) < 2:
        print("Usage: Blender --background --python tools/blender_character_capture.py -- <glb> <out.png>")
        sys.exit(2)
    glb_path = os.path.abspath(args[0])
    out_path = os.path.abspath(args[1])
    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)

    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete()

    bpy.ops.import_scene.gltf(filepath=glb_path)

    _, _, center, size = scene_world_bounds()
    span = max(size.x, size.y, size.z, 0.01)

    # Camera: three-quarter, slightly above waist (neo-noir ref framing).
    cam_data = bpy.data.cameras.new("AuditCam")
    cam_obj = bpy.data.objects.new("AuditCam", cam_data)
    bpy.context.collection.objects.link(cam_obj)
    bpy.context.scene.camera = cam_obj
    cam_data.lens = 43.0

    offset = Vector((1.15 * span, -1.55 * span, 0.55 * span))
    cam_obj.location = center + offset
    target = center + Vector((0, 0, 0.12 * span))
    direction = target - cam_obj.location
    rot_quat = direction.to_track_quat("-Z", "Y")
    cam_obj.rotation_euler = rot_quat.to_euler()

    # Key + cool fill + pink rim (approximate ref lighting, not game shader).
    def aim_light(obj, target: Vector):
        direction = target - obj.location
        obj.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()

    def add_light(name, typ, loc, energy, color, size_m=1.2):
        light = bpy.data.lights.new(name, type=typ)
        light.energy = energy
        light.color = color[:3]
        if typ == "AREA":
            light.size = size_m
            light.shape = "DISK"
        obj = bpy.data.objects.new(name, light)
        bpy.context.collection.objects.link(obj)
        obj.location = loc
        aim_light(obj, center)
        return obj

    bpy.context.scene.world.use_nodes = True
    bg = bpy.context.scene.world.node_tree.nodes.get("Background")
    if bg:
        bg.inputs[0].default_value = (0.02, 0.015, 0.04, 1.0)
        bg.inputs[1].default_value = 0.35

    add_light("Key", "AREA", center + Vector((-1.4 * span, 0.5 * span, 1.1 * span)), 780.0, (1.0, 0.82, 0.72), 2.6)
    add_light("Fill", "AREA", center + Vector((1.0 * span, 0.8 * span, 0.35 * span)), 260.0, (0.45, 0.55, 0.75), 3.0)
    add_light("RimPink", "AREA", center + Vector((0.2 * span, -1.5 * span, 0.9 * span)), 620.0, (1.0, 0.32, 0.52), 2.0)

    scene = bpy.context.scene
    try:
        scene.render.engine = "BLENDER_EEVEE_NEXT"
    except Exception:
        scene.render.engine = "BLENDER_EEVEE"

    scene.render.resolution_x = 720
    scene.render.resolution_y = 900
    scene.render.film_transparent = False
    scene.render.image_settings.file_format = "PNG"
    scene.render.filepath = out_path
    bpy.ops.render.render(write_still=True)
    print(f"Wrote {out_path}")


if __name__ == "__main__":
    main()
