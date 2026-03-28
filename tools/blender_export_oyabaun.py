"""
Run inside Blender: Text Editor > Run Script, or:
  blender your_scene.blend --background --python tools/blender_export_oyabaun.py

Exports triangulated mesh (vertex colors from Principled base + emission tint) and
AABBs for collision. Game uses Y-up; Blender is Z-up:
  game_xyz = (blender_x, blender_z, -blender_y)

Optional: create collection "OyabaunCollision" with box meshes for manual colliders.
If empty, solids are AABBs for each mesh whose name contains "Building" or starts
with "Ground" (excludes Win_, OYA_Light_, REF_).
"""
from __future__ import annotations

import bpy
import bmesh
import json
import os
from mathutils import Vector

# Override by setting before run: OYABAUN_OUT = "/path/to/tokyo_street.json"
OUT = globals().get(
    "OYABAUN_OUT",
    os.path.normpath(
        os.path.join(
            os.path.dirname(__file__),
            "..",
            "client",
            "levels",
            "tokyo_street.json",
        )
    ),
)

# Feet on ground; camera adds 1.65 in client.
SPAWN_GAME = globals().get("OYABAUN_SPAWN", [0.0, 0.0, 4.0])


def blender_to_game(v: Vector) -> tuple[float, float, float]:
    return (v.x, v.z, -v.y)


def get_principled_rgb(mat) -> tuple[float, float, float]:
    if not mat or not mat.use_nodes:
        if mat:
            return tuple(mat.diffuse_color[:3])
        return (0.35, 0.32, 0.38)
    for n in mat.node_tree.nodes:
        if n.type == "BSDF_PRINCIPLED":
            c = n.inputs["Base Color"].default_value
            e = n.inputs["Emission Strength"].default_value
            ec = n.inputs["Emission Color"].default_value
            r, g, b = c[0], c[1], c[2]
            if e > 0.01:
                r = r * 0.65 + ec[0] * min(e * 0.08, 0.85)
                g = g * 0.65 + ec[1] * min(e * 0.08, 0.85)
                b = b * 0.65 + ec[2] * min(e * 0.08, 0.85)
            return (max(0.0, min(1.0, r)), max(0.0, min(1.0, g)), max(0.0, min(1.0, b)))
    return tuple(mat.diffuse_color[:3])


def triangulate_object_mesh(obj) -> bpy.types.Mesh:
    dg = bpy.context.evaluated_depsgraph_get()
    ev = obj.evaluated_get(dg)
    me = bpy.data.meshes.new_from_object(ev, preserve_all_data_layers=True, depsgraph=dg)
    bm = bmesh.new()
    bm.from_mesh(me)
    bmesh.ops.triangulate(bm, faces=bm.faces)
    bm.to_mesh(me)
    bm.free()
    return me


def export() -> str:
    scene = bpy.context.scene
    verts_out: list[float] = []
    indices_out: list[int] = []
    solids: list[dict] = []

    skip_prefix = ("OYA_Light_", "REF_", "Camera")
    skip_exact = ()

    export_coll = bpy.data.collections.get("OyabaunExport")
    if export_coll and any(o.type == "MESH" for o in export_coll.objects):
        mesh_objs = [
            o
            for o in export_coll.objects
            if o.type == "MESH" and o.visible_get()
        ]
    else:
        mesh_objs = [
            o
            for o in scene.objects
            if o.type == "MESH"
            and not o.name.startswith(skip_prefix)
            and o.name not in skip_exact
            and o.visible_get()
        ]

    for obj in mesh_objs:
        mw = obj.matrix_world
        me = triangulate_object_mesh(obj)
        me.calc_loop_triangles()

        for tri in me.loop_triangles:
            polygon = me.polygons[tri.polygon_index]
            mat_idx = polygon.material_index
            mat = (
                obj.data.materials[mat_idx]
                if mat_idx < len(obj.data.materials)
                else None
            )
            r, g, b = get_principled_rgb(mat)

            for vi in tri.vertices:
                co = me.vertices[vi].co
                w = mw @ co
                gx, gy, gz = blender_to_game(w)
                indices_out.append(len(verts_out) // 6)
                verts_out.extend([gx, gy, gz, r, g, b])

        bpy.data.meshes.remove(me)

    coll = bpy.data.collections.get("OyabaunCollision")
    if coll:
        for obj in coll.objects:
            if obj.type != "MESH":
                continue
            mw = obj.matrix_world
            corners = [mw @ Vector(c) for c in obj.bound_box]
            xs = [p.x for p in corners]
            ys = [p.y for p in corners]
            zs = [p.z for p in corners]
            bmin = Vector((min(xs), min(ys), min(zs)))
            bmax = Vector((max(xs), max(ys), max(zs)))
            gmin = blender_to_game(bmin)
            gmax = blender_to_game(bmax)
            solids.append(
                {
                    "min": [min(gmin[0], gmax[0]), min(gmin[1], gmax[1]), min(gmin[2], gmax[2])],
                    "max": [max(gmin[0], gmax[0]), max(gmin[1], gmax[1]), max(gmin[2], gmax[2])],
                }
            )
    else:
        for obj in mesh_objs:
            n = obj.name
            if n.startswith("Win_"):
                continue
            if not (n.startswith("Ground") or "Building" in n):
                continue
            mw = obj.matrix_world
            corners = [mw @ Vector(c) for c in obj.bound_box]
            xs = [p.x for p in corners]
            ys = [p.y for p in corners]
            zs = [p.z for p in corners]
            bmin = Vector((min(xs), min(ys), min(zs)))
            bmax = Vector((max(xs), max(ys), max(zs)))
            gmin = blender_to_game(bmin)
            gmax = blender_to_game(bmax)
            solids.append(
                {
                    "min": [min(gmin[0], gmax[0]), min(gmin[1], gmax[1]), min(gmin[2], gmax[2])],
                    "max": [max(gmin[0], gmax[0]), max(gmin[1], gmax[1]), max(gmin[2], gmax[2])],
                }
            )

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    payload = {
        "spawn": SPAWN_GAME,
        "vertices": verts_out,
        "indices": indices_out,
        "solids": solids,
    }
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(payload, f, separators=(",", ":"))
    tri_count = len(indices_out) // 3
    return f"wrote {OUT} tris={tri_count} solids={len(solids)} floats={len(verts_out)}"


if __name__ == "__main__":
    msg = export()
    print(msg)
