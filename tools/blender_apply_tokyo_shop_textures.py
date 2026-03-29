"""
Place PixelLab shop PNGs on ShopFront_*_recess back walls (phase1 redesign).

Textures: client/level_textures/tokyo_shops/shop_*.png (sibling of client/levels/).

Run:

  python3 tools/oyabaunctl.py apply-tokyo-shop-textures

Then export GLB (enhance + export-world) so WASM embeds the new art.

Requires ShopFront_{L|R}_{idx}_recess meshes from blender_redesign_tokyo_alley_phase1.py.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

import bpy
from mathutils import Vector

COL_PANELS = "OyabaunShopPanels"
SHOP_FILES = (
    "shop_ramen.png",
    "shop_pachinko.png",
    "shop_konbini.png",
    "shop_shuttered.png",
    "shop_izakaya.png",
    "shop_arcade.png",
    "shop_snackbar.png",
    "shop_tattoo.png",
)


def _tex_dir() -> Path | None:
    fp = bpy.data.filepath
    if not fp:
        print("oyabaun: apply-shop-tex: save the .blend first (need filepath)", file=sys.stderr)
        return None
    d = Path(fp).resolve().parent.parent / "level_textures" / "tokyo_shops"
    if not d.is_dir():
        print(f"oyabaun: apply-shop-tex: missing directory {d}", file=sys.stderr)
        return None
    return d


def _remove_old_panels() -> int:
    n = 0
    for o in list(bpy.data.objects):
        if o.name.endswith("_ShopTex"):
            bpy.data.objects.remove(o, do_unlink=True)
            n += 1
    return n


def _ensure_collection(name: str) -> bpy.types.Collection:
    if name in bpy.data.collections:
        return bpy.data.collections[name]
    col = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(col)
    return col


def _load_image(tex_dir: Path, fname: str) -> bpy.types.Image | None:
    path = tex_dir / fname
    if not path.is_file():
        print(f"oyabaun: apply-shop-tex: missing {path}", file=sys.stderr)
        return None
    key = f"OyabaunShop_{fname.replace('.png', '')}"
    old = bpy.data.images.get(key)
    if old:
        bpy.data.images.remove(old)
    img = bpy.data.images.load(str(path), check_existing=False)
    img.name = key
    img.pack()
    try:
        img.colorspace_settings.name = "sRGB"
    except Exception:
        pass
    return img


def _ensure_material(safe: str, img: bpy.types.Image) -> bpy.types.Material:
    mat_name = f"OYA_ShopFace_{safe}"
    mat = bpy.data.materials.get(mat_name)
    if mat and mat.use_nodes and mat.node_tree:
        for node in mat.node_tree.nodes:
            if node.type == "TEX_IMAGE" and node.image:
                node.image = img
                node.interpolation = "Closest"
        return mat

    mat = bpy.data.materials.new(mat_name)
    mat.use_nodes = True
    mat.blend_method = "BLEND"
    nt = mat.node_tree
    nt.nodes.clear()
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    pr = nt.nodes.new("ShaderNodeBsdfPrincipled")
    tex = nt.nodes.new("ShaderNodeTexImage")
    uv = nt.nodes.new("ShaderNodeTexCoord")
    tex.image = img
    tex.interpolation = "Closest"
    tex.extension = "CLIP"
    nt.links.new(uv.outputs["UV"], tex.inputs["Vector"])
    nt.links.new(tex.outputs["Color"], pr.inputs["Base Color"])
    nt.links.new(tex.outputs["Alpha"], pr.inputs["Alpha"])
    pr.inputs["Roughness"].default_value = 0.86
    pr.inputs["Metallic"].default_value = 0.0
    nt.links.new(pr.outputs["BSDF"], out.inputs["Surface"])
    out.location = (300, 0)
    pr.location = (80, 0)
    tex.location = (-220, 0)
    uv.location = (-440, 0)
    return mat


def _add_panel(
    name: str,
    col: bpy.types.Collection,
    loc: Vector,
    normal: Vector,
    width: float,
    height: float,
    mat: bpy.types.Material,
) -> None:
    # Unit quad in XY, +Z normal (Blender 5+ has no bmesh.ops.create_plane).
    verts = [(-0.5, -0.5, 0.0), (0.5, -0.5, 0.0), (0.5, 0.5, 0.0), (-0.5, 0.5, 0.0)]
    faces = [(0, 1, 2, 3)]
    mesh = bpy.data.meshes.new(name + "_Mesh")
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    mesh.uv_layers.new(name="UVMap")
    uvs = ((0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0))
    uv_data = mesh.uv_layers.active.data
    for poly in mesh.polygons:
        for li in poly.loop_indices:
            vi = mesh.loops[li].vertex_index
            uv_data[li].uv = uvs[vi]
    ob = bpy.data.objects.new(name, mesh)
    col.objects.link(ob)
    ob.location = loc
    ob.dimensions = Vector((width, height, 0.0))
    z = Vector((0.0, 0.0, 1.0))
    n = normal.normalized()
    if n.length < 0.01:
        n = z
    q = z.rotation_difference(n)
    ob.rotation_euler = q.to_euler()
    ob.data.materials.append(mat)


def main() -> None:
    tex_dir = _tex_dir()
    if not tex_dir:
        sys.exit(1)

    n_del = _remove_old_panels()
    if n_del:
        print(f"oyabaun: apply-shop-tex: removed {n_del} old panel(s)")

    col = _ensure_collection(COL_PANELS)
    mat_cache: dict[str, bpy.types.Material] = {}
    created = 0

    for ob in bpy.data.objects:
        if ob.type != "MESH":
            continue
        m = re.match(r"^ShopFront_([LR])_(\d+)_recess$", ob.name)
        if not m:
            continue
        side, idx_s = m.group(1), m.group(2)
        idx = int(idx_s)
        fname = SHOP_FILES[idx % len(SHOP_FILES)]
        safe = fname.replace(".png", "")

        loc_r, rot, _sc = ob.matrix_world.decompose()
        half = Vector(
            (
                ob.dimensions[0] / 2.0,
                ob.dimensions[1] / 2.0,
                ob.dimensions[2] / 2.0,
            )
        )
        axis_x = (rot @ Vector((1.0, 0.0, 0.0))).normalized()
        if side == "L":
            back_center = loc_r - axis_x * half.x
            normal = axis_x
        else:
            back_center = loc_r + axis_x * half.x
            normal = -axis_x

        offset = normal * 0.025
        pw = max(0.15, ob.dimensions[1])
        ph = max(0.15, ob.dimensions[2])

        img = _load_image(tex_dir, fname)
        if not img:
            continue
        if safe not in mat_cache:
            mat_cache[safe] = _ensure_material(safe, img)
        mat = mat_cache[safe]

        panel_name = f"ShopFront_{side}_{idx_s}_ShopTex"
        _add_panel(panel_name, col, back_center + offset, normal, pw, ph, mat)
        created += 1
        print(f"oyabaun: apply-shop-tex: {panel_name} <- {fname}")

    fp = bpy.data.filepath
    if fp and created:
        bpy.ops.wm.save_mainfile()
        print(f"oyabaun: apply-shop-tex: saved {fp} ({created} panels)")
    elif not created:
        print(
            "oyabaun: apply-shop-tex: no ShopFront_*_recess meshes found "
            "(run redesign-tokyo-phase1 first)",
            file=sys.stderr,
        )
    else:
        print("oyabaun: apply-shop-tex: unsaved blend — panels created in memory only", file=sys.stderr)


if __name__ == "__main__":
    main()
