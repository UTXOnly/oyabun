"""
Add dense **Tokyo side-street** props inside Blender (signs, awnings, lanterns, vending blocks,
AC cages, conduits). Intended for `client/levels/tokyo_alley.blend` — no client-side geometry hacks.

Run (macOS example):

  /Applications/Blender.app/Contents/MacOS/Blender \\
    client/levels/tokyo_alley.blend --background \\
    --python tools/blender_enhance_tokyo_alley.py

Then export WASM level:

  python3 tools/oyabaunctl.py export-world --blend client/levels/tokyo_alley.blend

Re-run this script anytime; it replaces collection **OyabaunTokyoDetail** from scratch.
"""
from __future__ import annotations

import math
import random
import sys

import bpy
import mathutils
from mathutils import Vector

COL_NAME = "OyabaunTokyoDetail"
RNG_SEED = 42


def _world_bounds() -> tuple[Vector, Vector]:
    mn = Vector((1e9, 1e9, 1e9))
    mx = Vector((-1e9, -1e9, -1e9))
    for o in bpy.data.objects:
        if o.type != "MESH":
            continue
        for i in range(8):
            c = o.matrix_world @ Vector(o.bound_box[i])
            mn.x, mn.y, mn.z = min(mn.x, c.x), min(mn.y, c.y), min(mn.z, c.z)
            mx.x, mx.y, mx.z = max(mx.x, c.x), max(mx.y, c.y), max(mx.z, c.z)
    if mn.x > mx.x:
        print("oyabaun: no mesh bounds; abort", file=sys.stderr)
        sys.exit(1)
    return mn, mx


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


def _mat_principled(name: str, rgb: tuple[float, float, float], rough: float = 0.88) -> bpy.types.Material:
    if name in bpy.data.materials:
        return bpy.data.materials[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    nt.nodes.clear()
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    pr = nt.nodes.new("ShaderNodeBsdfPrincipled")
    pr.inputs["Base Color"].default_value = (*rgb, 1.0)
    pr.inputs["Roughness"].default_value = rough
    nt.links.new(pr.outputs["BSDF"], out.inputs["Surface"])
    return m


def _mat_emission(name: str, rgb: tuple[float, float, float], strength: float) -> bpy.types.Material:
    if name in bpy.data.materials:
        return bpy.data.materials[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    nt.nodes.clear()
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    em = nt.nodes.new("ShaderNodeEmission")
    em.inputs["Color"].default_value = (*rgb, 1.0)
    em.inputs["Strength"].default_value = strength
    nt.links.new(em.outputs["Emission"], out.inputs["Surface"])
    return m


def _link(obj: bpy.types.Object, col: bpy.types.Collection) -> None:
    for c in list(obj.users_collection):
        c.objects.unlink(obj)
    col.objects.link(obj)


def _cube(name: str, loc: Vector, scale: tuple[float, float, float], mat: bpy.types.Material, col) -> None:
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=loc)
    o = bpy.context.active_object
    o.name = name
    o.scale = scale
    o.data.materials.append(mat)
    _link(o, col)


def _cylinder(name: str, loc: Vector, r: float, depth: float, mat: bpy.types.Material, col, rot_x: float = 0.0) -> None:
    bpy.ops.mesh.primitive_cylinder_add(radius=r, depth=depth, location=loc)
    o = bpy.context.active_object
    o.name = name
    o.rotation_euler[0] = rot_x
    o.data.materials.append(mat)
    _link(o, col)


def main() -> None:
    random.seed(RNG_SEED)
    mn, mx = _world_bounds()
    col = _ensure_collection(COL_NAME)

    stucco = _mat_principled("Oyabaun_Stucco", (0.14, 0.13, 0.16))
    paper = _mat_principled("Oyabaun_SignPaper", (0.92, 0.9, 0.84), 0.75)
    wood = _mat_principled("Oyabaun_WoodTrim", (0.22, 0.12, 0.08))
    metal = _mat_principled("Oyabaun_MetalDark", (0.18, 0.18, 0.2), 0.35)
    glass_dark = _mat_principled("Oyabaun_GlassDark", (0.08, 0.1, 0.12), 0.12)
    neon_r = _mat_emission("Oyabaun_NeonRed", (1.0, 0.12, 0.2), 2.8)
    neon_c = _mat_emission("Oyabaun_NeonCyan", (0.15, 0.85, 0.95), 2.2)
    neon_w = _mat_emission("Oyabaun_NeonWarm", (1.0, 0.82, 0.45), 1.8)
    vending = _mat_principled("Oyabaun_Vending", (0.2, 0.22, 0.28))

    floor_z = max(mn.z + 0.02, 0.0)
    span_y = mx.y - mn.y
    span_x = mx.x - mn.x
    y0 = mn.y + 1.2
    y1 = mx.y - 1.2
    n_bays = max(18, int(span_y / 1.45))
    ys = [y0 + (y1 - y0) * i / (n_bays - 1) for i in range(n_bays)]

    x_left = mn.x + 0.22
    x_right = mx.x - 0.22

    for yi, y in enumerate(ys):
        h_sign = 1.85 + (yi % 5) * 0.12
        w_sign = 0.55 + (yi % 3) * 0.08
        z_mid = floor_z + 1.15 + h_sign * 0.5

        # Left façade: thin sign board + paper panel + neon strip
        _cube(
            f"TokyoProp_SignL_{yi:03d}",
            Vector((x_left + 0.06, y, z_mid)),
            (0.05, w_sign, h_sign),
            stucco,
            col,
        )
        _cube(
            f"TokyoProp_SignPaperL_{yi:03d}",
            Vector((x_left + 0.11, y, z_mid + 0.15)),
            (0.04, w_sign * 0.72, h_sign * 0.35),
            paper,
            col,
        )
        neon = neon_r if yi % 2 == 0 else neon_c
        _cube(
            f"TokyoProp_NeonL_{yi:03d}",
            Vector((x_left + 0.12, y, z_mid - 0.35)),
            (0.03, w_sign * 0.9, 0.06),
            neon,
            col,
        )

        # Right façade
        _cube(
            f"TokyoProp_SignR_{yi:03d}",
            Vector((x_right - 0.06, y, z_mid)),
            (0.05, w_sign, h_sign),
            stucco,
            col,
        )
        _cube(
            f"TokyoProp_SignPaperR_{yi:03d}",
            Vector((x_right - 0.11, y, z_mid - 0.1)),
            (0.04, w_sign * 0.68, h_sign * 0.4),
            paper,
            col,
        )
        _cube(
            f"TokyoProp_NeonR_{yi:03d}",
            Vector((x_right - 0.12, y, z_mid + 0.42)),
            (0.03, w_sign * 0.85, 0.05),
            neon_w if yi % 3 == 0 else neon_c,
            col,
        )

        # Awnings (alternate sides)
        aw_z = floor_z + 2.35 + (yi % 2) * 0.08
        aw_w = min(1.85, span_x * 0.22)
        if yi % 2 == 0:
            _cube(
                f"TokyoProp_AwningL_{yi:03d}",
                Vector((x_left + aw_w * 0.45, y, aw_z)),
                (0.12, aw_w, 0.18),
                wood,
                col,
            )
        else:
            _cube(
                f"TokyoProp_AwningR_{yi:03d}",
                Vector((x_right - aw_w * 0.45, y, aw_z)),
                (0.12, aw_w, 0.18),
                wood,
                col,
            )

        # Wall AC units
        if yi % 2 == 1:
            _cube(
                f"TokyoProp_AC_L_{yi:03d}",
                Vector((x_left + 0.18, y + 0.35, floor_z + 2.1)),
                (0.22, 0.55, 0.38),
                metal,
                col,
            )
        if yi % 2 == 0:
            _cube(
                f"TokyoProp_AC_R_{yi:03d}",
                Vector((x_right - 0.18, y - 0.35, floor_z + 2.25)),
                (0.22, 0.55, 0.38),
                metal,
                col,
            )

        # Vending / kiosk blocks (street edge, toward center X)
        if yi % 4 == 2:
            vx = (mn.x + mx.x) * 0.5 + random.uniform(-1.2, 1.2)
            _cube(
                f"TokyoProp_Vend_{yi:03d}",
                Vector((vx, y, floor_z + 0.95)),
                (0.55, 0.45, 1.85),
                vending,
                col,
            )
            _cube(
                f"TokyoProp_VendGlass_{yi:03d}",
                Vector((vx + 0.2, y, floor_z + 1.15)),
                (0.04, 0.35, 0.9),
                glass_dark,
                col,
            )

        # Lantern pole + warm bulb
        if yi % 3 == 0:
            px = (mn.x + mx.x) * 0.5 + random.uniform(-0.9, 0.9)
            pole_z = floor_z + 1.45
            _cylinder(
                f"TokyoProp_LampPost_{yi:03d}",
                Vector((px, y, pole_z)),
                0.07,
                2.85,
                metal,
                col,
            )
            bpy.ops.mesh.primitive_uv_sphere_add(radius=0.16, location=(px, y, pole_z + 1.5))
            lamp = bpy.context.active_object
            lamp.name = f"TokyoProp_LampBulb_{yi:03d}"
            lamp.data.materials.append(neon_w)
            _link(lamp, col)

    # Horizontal conduit runs along Y at both walls (several levels)
    for tier, z_off in enumerate((1.4, 2.8, 4.2)):
        for side, x_wall in enumerate((x_left + 0.25, x_right - 0.25)):
            cy = (y0 + y1) * 0.5
            length = (y1 - y0) * 0.92
            _cylinder(
                f"TokyoProp_Conduit_{tier}_{side}",
                Vector((x_wall, cy, floor_z + z_off)),
                0.04,
                length,
                metal,
                col,
                rot_x=math.pi / 2,
            )

    # Alley center: low dividers / planters
    for i in range(max(6, n_bays // 3)):
        yy = y0 + (y1 - y0) * (i + 0.5) / max(6, n_bays // 3)
        _cube(
            f"TokyoProp_Planter_{i:03d}",
            Vector(((mn.x + mx.x) * 0.5 + random.uniform(-0.4, 0.4), yy, floor_z + 0.22)),
            (0.35, 1.1, 0.28),
            wood,
            col,
        )

    # Hydrants
    for hy in range(max(4, n_bays // 5)):
        yy = y0 + (y1 - y0) * (0.15 + 0.7 * hy / max(4, n_bays // 5))
        hx = (mn.x + mx.x) * 0.5 + (1.4 if hy % 2 == 0 else -1.4)
        _cylinder(f"TokyoProp_Hydrant_{hy:03d}", Vector((hx, yy, floor_z + 0.35)), 0.14, 0.55, metal, col)
        bpy.ops.mesh.primitive_cube_add(size=0.18, location=(hx, yy, floor_z + 0.62))
        cap = bpy.context.active_object
        cap.name = f"TokyoProp_HydrantCap_{hy:03d}"
        cap.data.materials.append(metal)
        _link(cap, col)

    fp = bpy.data.filepath
    if fp:
        bpy.ops.wm.save_mainfile()
        print(f"oyabaun: saved {fp} with collection {COL_NAME}")
    else:
        print("oyabaun: unsaved blend — save manually", file=sys.stderr)


if __name__ == "__main__":
    main()
