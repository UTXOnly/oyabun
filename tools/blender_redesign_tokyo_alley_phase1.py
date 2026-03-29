"""
Tokyo alley redesign — Phase 1 (shop read / depth).

Adds low-poly shop-front modules along existing LeftBuilding_* / RightBuilding_* segments:
  - Recessed doorway volume (dark trim, into building)
  - Awning plane tilted toward the walkway
  - Thin vertical blade sign (classic Tokyo silhouette)

Uses existing materials (OYA_Trim, OYA_Awning, OYA_Building, ShopSign_*). No Collider in names
(geometry is mostly inset into walls; awnings stay within ~0.7 m of the façade).

Run (from repo root):

  python3 tools/oyabaunctl.py redesign-tokyo-phase1 --export-after

Or Blender only, then `export-world --force-all`:

  /path/to/Blender client/levels/tokyo_alley.blend --background \\
    --python tools/blender_redesign_tokyo_alley_phase1.py
"""
from __future__ import annotations

import math
import sys

import bpy
from mathutils import Euler, Vector

COL_NAME = "OyabaunRedesign_Phase1"


def _bounds_world(obj: bpy.types.Object) -> tuple[float, float, float, float, float, float] | None:
    if obj.type != "MESH" or not obj.data.vertices:
        return None
    mw = obj.matrix_world
    coords = [mw @ v.co for v in obj.data.vertices]
    xs = [p.x for p in coords]
    ys = [p.y for p in coords]
    zs = [p.z for p in coords]
    return (min(xs), max(xs), min(ys), max(ys), min(zs), max(zs))


def _ensure_collection(name: str) -> bpy.types.Collection:
    if name in bpy.data.collections:
        old = bpy.data.collections[name]
        bpy.ops.object.select_all(action="DESELECT")
        for o in list(old.objects):
            o.select_set(True)
        if bpy.context.selected_objects:
            bpy.ops.object.delete(use_global=False)
        bpy.data.collections.remove(old)
    col = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(col)
    return col


def _new_box(
    name: str,
    col: bpy.types.Collection,
    loc: tuple[float, float, float],
    size: tuple[float, float, float],
    rot_euler: tuple[float, float, float] | None = None,
) -> bpy.types.Object:
    import bmesh

    mesh = bpy.data.meshes.new(name + "_Mesh")
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=2.0)
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    ob = bpy.data.objects.new(name, mesh)
    ob.location = Vector(loc)
    ob.scale = (size[0] / 2.0, size[1] / 2.0, size[2] / 2.0)
    if rot_euler:
        ob.rotation_euler = Euler(rot_euler, "XYZ")
    col.objects.link(ob)
    _ensure_uv_smart_project(ob)
    return ob


def _new_awning_canopy(
    name: str,
    col: bpy.types.Collection,
    loc: tuple[float, float, float],
    size: tuple[float, float, float],
    rot_euler: tuple[float, float, float],
    u_repeat: float = 6.0,
) -> bpy.types.Object:
    """Bottom face of the old awning box only, with UVs tiled along width (avoids tiny cube islands)."""
    import bmesh

    awning_d, awning_w, thick = size
    mesh = bpy.data.meshes.new(name + "_Mesh")
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=2.0)
    bm.faces.ensure_lookup_table()
    bottom = [
        f
        for f in bm.faces
        if all(abs(v.co.z + 1.0) < 1e-5 for v in f.verts)
    ]
    if len(bottom) != 1:
        bottom = [max(bm.faces, key=lambda f: f.calc_area())]
    kill = [f for f in bm.faces if f is not bottom[0]]
    bmesh.ops.delete(bm, geom=kill, context="FACES")
    bm.faces.ensure_lookup_table()
    uvl = bm.loops.layers.uv.verify()
    f0 = bm.faces[0]
    for loop in f0.loops:
        co = loop.vert.co
        u = (co.y + 1.0) * 0.5 * u_repeat
        v = (co.x + 1.0) * 0.5
        loop[uvl].uv = (u, v)
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    ob = bpy.data.objects.new(name, mesh)
    ob.location = Vector(loc)
    ob.scale = (awning_d / 2.0, awning_w / 2.0, thick / 2.0)
    ob.rotation_euler = Euler(rot_euler, "XYZ")
    col.objects.link(ob)
    return ob


def _ensure_uv_smart_project(ob: bpy.types.Object) -> None:
    mesh = ob.data
    if mesh.uv_layers:
        return
    import bmesh

    bm = bmesh.new()
    bm.from_mesh(mesh)
    try:
        bmesh.ops.smart_project(
            bm,
            angle_limit=math.radians(66.0),
            island_margin=0.02,
        )
    except Exception as exc:  # noqa: BLE001
        print(f"oyabaun: phase1: uv skip {ob.name}: {exc}", file=sys.stderr)
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()


def _link_mat(ob: bpy.types.Object, mat_name: str) -> None:
    mat = bpy.data.materials.get(mat_name)
    if not mat:
        mat = bpy.data.materials.new(mat_name)
        mat.use_nodes = True
    ob.data.materials.clear()
    ob.data.materials.append(mat)


def main() -> None:
    left_objs = sorted(
        [o for o in bpy.data.objects if o.type == "MESH" and o.name.startswith("LeftBuilding_")],
        key=lambda x: x.name,
    )
    right_objs = sorted(
        [o for o in bpy.data.objects if o.type == "MESH" and o.name.startswith("RightBuilding_")],
        key=lambda x: x.name,
    )
    if len(left_objs) < 4 or len(right_objs) < 4:
        print("oyabaun: phase1: need LeftBuilding_* / RightBuilding_* meshes", file=sys.stderr)
        sys.exit(1)

    col = _ensure_collection(COL_NAME)
    awning_names = ("OYA_Awning", "OYA_Trim", "OYA_Building")
    sign_names = ("ShopSign_0", "ShopSign_1", "ShopSign_2", "ShopSign_3")

    def add_for_segment(side: str, seg_ob: bpy.types.Object, idx: int) -> None:
        b = _bounds_world(seg_ob)
        if not b:
            return
        xmin, xmax, ymin, ymax, zmin, zmax = b
        yc = (ymin + ymax) * 0.5
        z0 = max(zmin, 0.05) + 0.12
        door_h = 2.35
        door_w = 1.85 + (idx % 3) * 0.15
        recess_d = 0.42
        if side == "L":
            inner_x = xmax
            recess_cx = inner_x - recess_d * 0.5
            awning_rot = (math.radians(8), 0, math.radians(2 - idx % 5))
            blade_dx = -0.14
        else:
            inner_x = xmin
            recess_cx = inner_x + recess_d * 0.5
            awning_rot = (math.radians(8), 0, math.radians(-2 + idx % 5))
            blade_dx = 0.14

        zc = z0 + door_h * 0.5
        prefix = f"ShopFront_{side}_{idx:02d}"

        recess = _new_box(
            f"{prefix}_recess",
            col,
            (recess_cx, yc, zc),
            (recess_d, door_w * 0.92, door_h),
        )
        _link_mat(recess, "OYA_Trim")

        awning_w = door_w + 0.35
        awning_d = 0.62
        awning_z = z0 + door_h - 0.15
        acx = inner_x + (0.32 if side == "L" else -0.32)
        awning = _new_awning_canopy(
            f"{prefix}_awning",
            col,
            (acx, yc, awning_z),
            (awning_d, awning_w, 0.12),
            awning_rot,
        )
        _link_mat(awning, awning_names[idx % len(awning_names)])

        blade_h = 2.1 + (idx % 4) * 0.25
        blade_w = 0.11
        blade_d = 0.38
        bcx = inner_x + blade_dx
        bzc = z0 + 1.05 + (idx % 3) * 0.35
        blade = _new_box(
            f"{prefix}_blade",
            col,
            (bcx, yc + door_w * 0.22, bzc),
            (blade_w, blade_d, blade_h),
        )
        _link_mat(blade, sign_names[idx % len(sign_names)])

    for i, ob in enumerate(left_objs):
        add_for_segment("L", ob, i)
    for i, ob in enumerate(right_objs):
        add_for_segment("R", ob, i)

    fp = bpy.data.filepath
    if fp:
        bpy.ops.wm.save_mainfile()
        print(f"oyabaun: phase1 saved {fp} ({len(col.objects)} objects in {COL_NAME})")
    else:
        print(f"oyabaun: phase1 created {len(col.objects)} objects (unsaved blend)", file=sys.stderr)


if __name__ == "__main__":
    main()
