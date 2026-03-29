"""
Procedural **3D** Oyabaun characters (boss + rival) for WASM — stylized cyberpunk yakuza silhouettes.

- Multiple Principled materials (base color / emission only; no image textures — tints multiply 1×1 white in-game).
- Blender **Z-up**, feet at **Z = 0**, front faces **−Y** (glTF `export_yup` → game Y-up).
- Joined single mesh per variant; under ~15 materials, ~2k–2.8k verts.

Run (repo root):
  /path/to/Blender --background --python tools/blender_make_oyabaun_character.py

Environment:
  OYABAUN_VARIANT   — boss | rival | all   (default: boss)
  OYABAUN_OUT       — output GLB when variant is boss or rival (default: client/characters/oyabaun_player.glb)
  OYABAUN_OUT_RIVAL — used when VARIANT=all (default: client/characters/oyabaun_rival.glb)
"""
from __future__ import annotations

import math
import os
import sys
from typing import Sequence

import bpy
import bmesh
from mathutils import Euler, Vector

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
OUT_DIR = os.path.join(ROOT, "client", "characters")
DEFAULT_PLAYER = os.path.join(OUT_DIR, "oyabaun_player.glb")
DEFAULT_RIVAL = os.path.join(OUT_DIR, "oyabaun_rival.glb")

# --- materials (name -> (rgb, emissive_rgb, emit_strength)) ---
def _m(
    rgb: tuple[float, float, float],
    em: tuple[float, float, float] | None = None,
    em_str: float = 0.0,
) -> tuple[tuple[float, float, float], tuple[float, float, float], float]:
    e = em or (0.0, 0.0, 0.0)
    return (rgb, e, em_str)


BOSS_PALETTE: dict[str, tuple[tuple[float, float, float], tuple[float, float, float], float]] = {
    "Skin": _m((0.72, 0.55, 0.46)),
    "Suit": _m((0.07, 0.09, 0.12)),
    "Shirt": _m((0.92, 0.91, 0.88)),
    "Hair": _m((0.06, 0.06, 0.07)),
    "Shoe": _m((0.12, 0.1, 0.09)),
    "Metal": _m((0.22, 0.22, 0.24)),
    "NeonCyan": _m((0.02, 0.08, 0.09), (0.15, 0.85, 0.92), 2.2),
    "Glass": _m((0.05, 0.06, 0.08)),
    "Tie": _m((0.45, 0.08, 0.12)),
    "Paper": _m((0.9, 0.88, 0.82)),
    "Tip": _m((0.95, 0.35, 0.12)),
}

RIVAL_PALETTE: dict[str, tuple[tuple[float, float, float], tuple[float, float, float], float]] = {
    "Skin": _m((0.82, 0.68, 0.58)),
    "Suit": _m((0.88, 0.87, 0.9)),
    "Shirt": _m((0.25, 0.22, 0.28)),
    "Hair": _m((0.92, 0.9, 0.82)),
    "Shoe": _m((0.18, 0.16, 0.2)),
    "Blade": _m((0.75, 0.78, 0.82)),
    "Handle": _m((0.15, 0.12, 0.1)),
    "Wrap": _m((0.08, 0.06, 0.14)),
    "NeonPurple": _m((0.1, 0.04, 0.12), (0.75, 0.2, 0.95), 2.0),
    "GlassPurp": _m((0.22, 0.08, 0.35)),
    "Gold": _m((0.85, 0.65, 0.25)),
    "Scar": _m((0.45, 0.32, 0.28)),
}


def _ensure_mat(name: str, spec: tuple[tuple[float, float, float], tuple[float, float, float], float]) -> bpy.types.Material:
    rgb, em, em_str = spec
    mat = bpy.data.materials.get(name)
    if mat is None:
        mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    if nt is None:
        return mat
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    pr = nt.nodes.new("ShaderNodeBsdfPrincipled")
    pr.inputs["Base Color"].default_value = (*rgb, 1.0)
    pr.inputs["Roughness"].default_value = 0.78
    pr.inputs["Metallic"].default_value = 0.0
    if em_str > 0.0 and (em[0] + em[1] + em[2]) > 1e-4:
        pr.inputs["Emission Color"].default_value = (*em, 1.0)
        pr.inputs["Emission Strength"].default_value = em_str
    nt.links.new(pr.outputs["BSDF"], out.inputs["Surface"])
    return mat


def _cube(name: str, loc: Vector, size: Vector, mat: bpy.types.Material) -> bpy.types.Object:
    bm = bmesh.new()
    bmesh.ops.create_cube(bm, size=2.0)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    ob = bpy.data.objects.new(name, mesh)
    ob.location = loc
    ob.scale = (size.x / 2.0, size.y / 2.0, size.z / 2.0)
    ob.data.materials.append(mat)
    return ob


def _cylinder(
    name: str,
    loc: Vector,
    radius: float,
    depth: float,
    mat: bpy.types.Material,
    rot: Euler | None = None,
    segments: int = 8,
) -> bpy.types.Object:
    bm = bmesh.new()
    bmesh.ops.create_cone(
        bm,
        cap_ends=True,
        cap_tris=False,
        segments=segments,
        radius1=radius,
        radius2=radius,
        depth=depth,
    )
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    ob = bpy.data.objects.new(name, mesh)
    ob.location = loc
    if rot:
        ob.rotation_euler = rot
    ob.data.materials.append(mat)
    return ob


def _join(objects: Sequence[bpy.types.Object], result_name: str) -> bpy.types.Object:
    bpy.ops.object.select_all(action="DESELECT")
    root = None
    for o in objects:
        if o.type != "MESH":
            continue
        o.select_set(True)
        if root is None:
            root = o
    if root is None:
        raise RuntimeError("no mesh objects to join")
    bpy.context.view_layer.objects.active = root
    bpy.ops.object.join()
    root.name = result_name
    return root


def _build_palette_mats(pal: dict[str, tuple]) -> dict[str, bpy.types.Material]:
    return {k: _ensure_mat(f"OYA_{k}", v) for k, v in pal.items()}


def build_boss(m: dict[str, bpy.types.Material]) -> list[bpy.types.Object]:
    sx = 1.06
    objs: list[bpy.types.Object] = []

    # Shoes
    objs.append(_cube("shoe_L", Vector((-0.11, 0.02, 0.045)), Vector((0.14, 0.26, 0.09)), m["Shoe"]))
    objs.append(_cube("shoe_R", Vector((0.11, 0.02, 0.045)), Vector((0.14, 0.26, 0.09)), m["Shoe"]))

    # Legs (pants)
    objs.append(_cube("leg_L", Vector((-0.11, 0.02, 0.42)), Vector((0.2, 0.22, 0.78)), m["Suit"]))
    objs.append(_cube("leg_R", Vector((0.11, 0.02, 0.42)), Vector((0.2, 0.22, 0.78)), m["Suit"]))

    # Pelvis / hips
    objs.append(_cube("pelvis", Vector((0, 0.02, 0.82)), Vector((0.38 * sx, 0.28, 0.22)), m["Suit"]))

    # Torso (jacket)
    objs.append(_cube("torso", Vector((0, -0.04, 1.05)), Vector((0.5 * sx, 0.32, 0.48)), m["Suit"]))
    # Shirt V at chest (slightly forward -Y)
    objs.append(_cube("shirt", Vector((0, -0.12, 1.08)), Vector((0.32 * sx, 0.08, 0.22)), m["Shirt"]))

    # Lapel panels (V)
    objs.append(_cube("lapel_L", Vector((-0.14, -0.11, 1.12)), Vector((0.12, 0.06, 0.32)), m["Suit"]))
    objs.append(_cube("lapel_R", Vector((0.14, -0.11, 1.12)), Vector((0.12, 0.06, 0.32)), m["Suit"]))
    # Neon piping on lapels (thin)
    objs.append(_cube("neon_lap_L", Vector((-0.19, -0.12, 1.12)), Vector((0.03, 0.04, 0.34)), m["NeonCyan"]))
    objs.append(_cube("neon_lap_R", Vector((0.19, -0.12, 1.12)), Vector((0.03, 0.04, 0.34)), m["NeonCyan"]))

    # Shoulder pads
    objs.append(_cube("shoulder_L", Vector((-0.32 * sx, -0.02, 1.28)), Vector((0.22, 0.26, 0.16)), m["Suit"]))
    objs.append(_cube("shoulder_R", Vector((0.32 * sx, -0.02, 1.28)), Vector((0.22, 0.26, 0.16)), m["Suit"]))

    # Upper arms
    objs.append(_cube("uarm_L", Vector((-0.38 * sx, 0.02, 1.15)), Vector((0.16, 0.16, 0.38)), m["Suit"]))
    objs.append(_cube("uarm_R", Vector((0.38 * sx, 0.02, 1.15)), Vector((0.16, 0.16, 0.38)), m["Suit"]))
    # Forearms + mitts
    objs.append(_cube("farm_L", Vector((-0.4 * sx, 0.02, 0.82)), Vector((0.14, 0.14, 0.34)), m["Suit"]))
    objs.append(
        _cube("hand_L", Vector((-0.42 * sx, 0.02, 0.62)), Vector((0.12, 0.1, 0.14)), m["Skin"])
    )
    objs.append(_cube("farm_R", Vector((0.4 * sx, 0.02, 0.82)), Vector((0.14, 0.14, 0.34)), m["Suit"]))
    objs.append(
        _cube("hand_R", Vector((0.42 * sx, 0.02, 0.62)), Vector((0.13, 0.11, 0.15)), m["Skin"])
    )
    objs.append(_cube("thumb_R", Vector((0.46 * sx, 0.06, 0.6)), Vector((0.06, 0.05, 0.1)), m["Skin"]))

    # Tie
    objs.append(_cube("tie_knot", Vector((0, -0.16, 1.14)), Vector((0.1, 0.08, 0.12)), m["Tie"]))
    objs.append(_cube("tie_body", Vector((0, -0.15, 0.98)), Vector((0.08, 0.06, 0.38)), m["Tie"]))

    # Belt + buckle glow
    objs.append(_cube("belt", Vector((0, 0.02, 0.78)), Vector((0.44 * sx, 0.26, 0.1)), m["Suit"]))
    objs.append(_cube("buckle", Vector((0, 0.14, 0.78)), Vector((0.1, 0.04, 0.08)), m["NeonCyan"]))

    # Neck + head base
    objs.append(_cube("neck", Vector((0, -0.02, 1.38)), Vector((0.16, 0.14, 0.14)), m["Skin"]))
    objs.append(_cube("head", Vector((0, -0.06, 1.58)), Vector((0.22 * sx, 0.26, 0.32)), m["Skin"]))
    # Jaw / chin block
    objs.append(_cube("jaw", Vector((0, -0.1, 1.48)), Vector((0.2 * sx, 0.12, 0.14)), m["Skin"]))
    # Nose wedge
    objs.append(_cube("nose", Vector((0, -0.2, 1.52)), Vector((0.08, 0.1, 0.1)), m["Skin"]))
    # Ears
    objs.append(_cube("ear_L", Vector((-0.24 * sx, -0.02, 1.52)), Vector((0.06, 0.04, 0.12)), m["Skin"]))
    objs.append(_cube("ear_R", Vector((0.24 * sx, -0.02, 1.52)), Vector((0.06, 0.04, 0.12)), m["Skin"]))

    # Hair — slicked volume (several slabs)
    for i, (dz, sy) in enumerate([(0.18, 0.28), (0.26, 0.24), (0.32, 0.2)]):
        yo = -0.04 - i * 0.05
        objs.append(
            _cube(f"hair_{i}", Vector((0, yo, 1.72 + dz * 0.3)), Vector((0.24 * sx, sy, 0.12)), m["Hair"])
        )

    # Sunglasses
    objs.append(_cube("glass_L", Vector((-0.1, -0.22, 1.54)), Vector((0.1, 0.04, 0.06)), m["Glass"]))
    objs.append(_cube("glass_R", Vector((0.1, -0.22, 1.54)), Vector((0.1, 0.04, 0.06)), m["Glass"]))
    objs.append(_cube("glass_bridge", Vector((0, -0.22, 1.54)), Vector((0.06, 0.03, 0.05)), m["Metal"]))

    # Cigarette
    objs.append(
        _cylinder(
            "cig",
            Vector((0.08, -0.24, 1.46)),
            0.015,
            0.09,
            m["Paper"],
            rot=Euler((math.radians(90), 0, math.radians(20)), "XYZ"),
            segments=6,
        )
    )
    objs.append(_cube("cig_tip", Vector((0.1, -0.27, 1.46)), Vector((0.04, 0.04, 0.04)), m["Tip"]))

    # Pistol (right hand, forward -Y)
    objs.append(_cube("gun_grip", Vector((0.44 * sx, 0.02, 0.66)), Vector((0.06, 0.12, 0.14)), m["Metal"]))
    objs.append(_cube("gun_slide", Vector((0.44 * sx, -0.1, 0.7)), Vector((0.08, 0.22, 0.1)), m["Metal"]))
    objs.append(_cube("gun_barrel", Vector((0.44 * sx, -0.22, 0.72)), Vector((0.05, 0.2, 0.05)), m["Metal"]))
    objs.append(_cube("gun_trigger", Vector((0.42 * sx, -0.05, 0.64)), Vector((0.04, 0.06, 0.05)), m["Metal"]))
    objs.append(_cube("gun_neon", Vector((0.44 * sx, -0.18, 0.72)), Vector((0.04, 0.08, 0.04)), m["NeonCyan"]))

    # Breast pocket hint
    objs.append(_cube("pocket", Vector((-0.16, -0.08, 1.02)), Vector((0.1, 0.04, 0.08)), m["Suit"]))

    return objs


def build_rival(m: dict[str, bpy.types.Material]) -> list[bpy.types.Object]:
    sx = 0.94
    objs: list[bpy.types.Object] = []

    objs.append(_cube("shoe_L", Vector((-0.1, 0.02, 0.045)), Vector((0.13, 0.24, 0.09)), m["Shoe"]))
    objs.append(_cube("shoe_R", Vector((0.1, 0.02, 0.045)), Vector((0.13, 0.24, 0.09)), m["Shoe"]))

    objs.append(_cube("leg_L", Vector((-0.1, 0.02, 0.42)), Vector((0.18, 0.2, 0.78)), m["Suit"]))
    objs.append(_cube("leg_R", Vector((0.1, 0.02, 0.42)), Vector((0.18, 0.2, 0.78)), m["Suit"]))

    objs.append(_cube("pelvis", Vector((0, 0.02, 0.82)), Vector((0.34 * sx, 0.26, 0.2)), m["Suit"]))
    objs.append(_cube("torso", Vector((0, -0.04, 1.06)), Vector((0.44 * sx, 0.3, 0.46)), m["Suit"]))
    # Open collar / shirt
    objs.append(_cube("shirt_open", Vector((0, -0.1, 1.1)), Vector((0.28 * sx, 0.1, 0.2)), m["Shirt"]))
    objs.append(_cube("lapel_L", Vector((-0.12, -0.1, 1.12)), Vector((0.1, 0.05, 0.28)), m["Suit"]))
    objs.append(_cube("lapel_R", Vector((0.12, -0.1, 1.12)), Vector((0.1, 0.05, 0.28)), m["Suit"]))
    objs.append(_cube("neon_l", Vector((-0.17, -0.11, 1.1)), Vector((0.025, 0.04, 0.3)), m["NeonPurple"]))
    objs.append(_cube("neon_r", Vector((0.17, -0.11, 1.1)), Vector((0.025, 0.04, 0.3)), m["NeonPurple"]))

    objs.append(_cube("shoulder_L", Vector((-0.28 * sx, -0.02, 1.26)), Vector((0.18, 0.22, 0.14)), m["Suit"]))
    objs.append(_cube("shoulder_R", Vector((0.28 * sx, -0.02, 1.26)), Vector((0.18, 0.22, 0.14)), m["Suit"]))

    objs.append(_cube("uarm_L", Vector((-0.34 * sx, 0.02, 1.12)), Vector((0.14, 0.14, 0.36)), m["Suit"]))
    objs.append(_cube("uarm_R", Vector((0.34 * sx, 0.02, 1.12)), Vector((0.14, 0.14, 0.36)), m["Suit"]))
    objs.append(_cube("farm_L", Vector((-0.36 * sx, 0.02, 0.8)), Vector((0.12, 0.12, 0.32)), m["Suit"]))
    objs.append(_cube("hand_L", Vector((-0.38 * sx, 0.02, 0.6)), Vector((0.1, 0.09, 0.12)), m["Skin"]))
    objs.append(_cube("farm_R", Vector((0.36 * sx, 0.02, 0.8)), Vector((0.12, 0.12, 0.32)), m["Suit"]))
    objs.append(_cube("hand_R", Vector((0.38 * sx, 0.02, 0.6)), Vector((0.1, 0.09, 0.12)), m["Skin"]))

    objs.append(_cube("belt", Vector((0, 0.02, 0.78)), Vector((0.38 * sx, 0.24, 0.09)), m["Suit"]))
    objs.append(_cube("buckle", Vector((0, 0.13, 0.78)), Vector((0.08, 0.04, 0.07)), m["NeonPurple"]))

    objs.append(_cube("neck", Vector((0, -0.02, 1.36)), Vector((0.14, 0.12, 0.12)), m["Skin"]))
    objs.append(_cube("head", Vector((0, -0.05, 1.55)), Vector((0.2 * sx, 0.24, 0.3)), m["Skin"]))
    objs.append(_cube("jaw", Vector((0, -0.09, 1.46)), Vector((0.18 * sx, 0.1, 0.12)), m["Skin"]))
    objs.append(_cube("nose", Vector((0, -0.19, 1.5)), Vector((0.07, 0.09, 0.09)), m["Skin"]))
    objs.append(_cube("ear_L", Vector((-0.22 * sx, -0.02, 1.5)), Vector((0.05, 0.04, 0.11)), m["Skin"]))
    objs.append(_cube("ear_R", Vector((0.22 * sx, -0.02, 1.5)), Vector((0.05, 0.04, 0.11)), m["Skin"]))

    # Scar (raised cheek)
    objs.append(_cube("scar", Vector((-0.12, -0.14, 1.48)), Vector((0.08, 0.12, 0.03)), m["Scar"]))

    # Spiky hair
    base_z = 1.68
    spikes = [
        (0.0, -0.08, 0.22, 0.06, 0.12),
        (-0.12, -0.04, 0.2, 0.05, 0.14),
        (0.14, -0.05, 0.21, 0.05, 0.15),
        (-0.2, 0.02, 0.16, 0.04, 0.12),
        (0.18, 0.0, 0.17, 0.04, 0.13),
        (0.08, -0.12, 0.24, 0.05, 0.16),
        (-0.08, -0.1, 0.23, 0.05, 0.14),
    ]
    for i, (ox, oy, h, rw, rd) in enumerate(spikes):
        objs.append(
            _cube(
                f"spike_{i}",
                Vector((ox * sx, oy, base_z + h * 0.5)),
                Vector((rw, rd, h)),
                m["Hair"],
            )
        )

    objs.append(_cube("glass_L", Vector((-0.09, -0.2, 1.52)), Vector((0.09, 0.04, 0.055)), m["GlassPurp"]))
    objs.append(_cube("glass_R", Vector((0.09, -0.2, 1.52)), Vector((0.09, 0.04, 0.055)), m["GlassPurp"]))
    objs.append(_cube("glass_bridge", Vector((0, -0.2, 1.52)), Vector((0.05, 0.03, 0.04)), m["Handle"]))

    # Necklace at open collar
    objs.append(
        _cylinder(
            "chain",
            Vector((0, -0.08, 1.22)),
            0.09,
            0.03,
            m["Gold"],
            rot=Euler((math.radians(90), 0, 0), "XYZ"),
            segments=10,
        )
    )

    # Katana: left hand, blade along -Y
    objs.append(_cube("tsuba", Vector((-0.36 * sx, -0.02, 0.72)), Vector((0.04, 0.22, 0.22)), m["Blade"]))
    objs.append(_cube("blade", Vector((-0.36 * sx, -0.28, 0.78)), Vector((0.035, 0.55, 0.08)), m["Blade"]))
    objs.append(_cube("edge_glow", Vector((-0.36 * sx, -0.32, 0.78)), Vector((0.02, 0.5, 0.02)), m["NeonPurple"]))
    objs.append(_cube("handle", Vector((-0.36 * sx, 0.12, 0.68)), Vector((0.05, 0.2, 0.06)), m["Handle"]))
    objs.append(_cube("wrap", Vector((-0.36 * sx, 0.06, 0.68)), Vector((0.055, 0.12, 0.065)), m["Wrap"]))
    objs.append(_cube("wrap2", Vector((-0.36 * sx, -0.02, 0.68)), Vector((0.055, 0.12, 0.065)), m["Wrap"]))

    return objs


def export_glb(ob: bpy.types.Object, filepath: str) -> None:
    os.makedirs(os.path.dirname(filepath) or ".", exist_ok=True)
    bpy.ops.object.select_all(action="DESELECT")
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.export_scene.gltf(
        filepath=filepath,
        export_format="GLB",
        export_materials="EXPORT",
        export_texcoords=True,
        export_normals=True,
        export_apply=True,
        export_yup=True,
        use_selection=True,
        export_animations=False,
    )
    print(f"oyabaun: wrote {filepath}", file=sys.stderr)


def run_variant(variant: str, out_path: str) -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    col = bpy.context.scene.collection
    mats = _build_palette_mats(BOSS_PALETTE if variant == "boss" else RIVAL_PALETTE)
    parts = build_boss(mats) if variant == "boss" else build_rival(mats)
    for o in parts:
        col.objects.link(o)
    merged = _join(parts, "OyabaunCharacter")
    bpy.context.view_layer.objects.active = merged
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.faces_shade_smooth()
    bpy.ops.uv.smart_project(angle_limit=math.radians(66.0), island_margin=0.02)
    bpy.ops.object.mode_set(mode="OBJECT")
    export_glb(merged, out_path)


def main() -> None:
    variant = os.environ.get("OYABAUN_VARIANT", "boss").lower().strip()
    out_player = os.environ.get("OYABAUN_OUT", DEFAULT_PLAYER)
    out_rival = os.environ.get("OYABAUN_OUT_RIVAL", DEFAULT_RIVAL)

    if variant == "all":
        run_variant("boss", out_player)
        run_variant("rival", out_rival)
    elif variant == "rival":
        run_variant("rival", out_player)
    elif variant == "boss":
        run_variant("boss", out_player)
    else:
        print(f"oyabaun: unknown OYABAUN_VARIANT={variant!r} (use boss, rival, all)", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
