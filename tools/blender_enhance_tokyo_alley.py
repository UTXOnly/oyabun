"""
Improve **Tokyo alley** look in Blender: remove stray Oyabaun-generated props and add
**procedural texture** to existing façade materials (`OYA_*`, signs, neon) so reads closer to
a **90s arcade / low-res textured** lane — not flat colors, and **nothing new in the street**.

Does **not** add obstacles, vending, poles, or planters in the alley center.

Run:

  /path/to/Blender client/levels/tokyo_alley.blend --background \\
    --python tools/blender_enhance_tokyo_alley.py

Then:

  python3 tools/oyabaunctl.py export-world --blend client/levels/tokyo_alley.blend

Art-direction refs (not used by this script): repo-root **example_images/** (e.g. sokes1.png, soke*.mp4).

Re-running is idempotent (materials tagged with a NodeFrame marker are skipped).
"""
from __future__ import annotations

import sys

import bpy

MARK = "OyabaunTexEnhance"
COL_TRASH = "OyabaunTokyoDetail"


def _remove_generated_prop_collection() -> int:
    """Delete collection added by older script versions (center-street junk)."""
    if COL_TRASH not in bpy.data.collections:
        return 0
    old = bpy.data.collections[COL_TRASH]
    bpy.ops.object.select_all(action="DESELECT")
    n = 0
    for o in list(old.objects):
        o.select_set(True)
        n += 1
    if bpy.context.selected_objects:
        bpy.ops.object.delete(use_global=False)
    bpy.data.collections.remove(old)
    return n


def _principled(mat: bpy.types.Material) -> bpy.types.ShaderNodeBsdfPrincipled | None:
    if not mat.use_nodes:
        return None
    for n in mat.node_tree.nodes:
        if n.type == "BSDF_PRINCIPLED":
            return n
    return None


def _marked(nt: bpy.types.NodeTree) -> bool:
    return any(getattr(n, "name", "") == MARK for n in nt.nodes)


def _place(nodes: list, x0: float, y0: float, dx: float = 220) -> None:
    for i, n in enumerate(nodes):
        n.location = (x0 - i * dx, y0)


def _enhance_brick(nt: bpy.types.NodeTree, pr: bpy.types.ShaderNodeBsdfPrincipled) -> None:
    bc = pr.inputs["Base Color"].default_value
    if pr.inputs["Base Color"].is_linked:
        return
    r0, g0, b0, _ = bc
    tc = nt.nodes.new("ShaderNodeTexCoord")
    mp = nt.nodes.new("ShaderNodeMapping")
    mp.inputs["Scale"].default_value = (14.0, 6.0, 14.0)
    br = nt.nodes.new("ShaderNodeTexBrick")
    br.inputs["Color1"].default_value = (r0 * 0.82, g0 * 0.8, b0 * 0.88, 1.0)
    br.inputs["Color2"].default_value = (min(1.0, r0 * 1.08), min(1.0, g0 * 1.06), min(1.0, b0 * 1.1), 1.0)
    br.inputs["Mortar"].default_value = (r0 * 0.45, g0 * 0.42, b0 * 0.5, 1.0)
    br.inputs["Mortar Size"].default_value = 0.045
    br.inputs["Bias"].default_value = 0.42
    br.inputs["Brick Width"].default_value = 0.52
    br.inputs["Row Height"].default_value = 0.28
    try:
        uv_socket = tc.outputs["UV"]
    except KeyError:
        uv_socket = tc.outputs["Generated"]
    nt.links.new(uv_socket, mp.inputs["Vector"])
    nt.links.new(mp.outputs["Vector"], br.inputs["Vector"])
    nt.links.new(br.outputs["Color"], pr.inputs["Base Color"])
    pr.inputs["Roughness"].default_value = min(0.95, pr.inputs["Roughness"].default_value + 0.12)
    _place([tc, mp, br], pr.location.x - 520, pr.location.y)


def _enhance_noise_tint(
    nt: bpy.types.NodeTree,
    pr: bpy.types.ShaderNodeBsdfPrincipled,
    scale: tuple[float, float, float],
    strength: float,
) -> None:
    if pr.inputs["Base Color"].is_linked:
        return
    r0, g0, b0, _ = pr.inputs["Base Color"].default_value
    tc = nt.nodes.new("ShaderNodeTexCoord")
    mp = nt.nodes.new("ShaderNodeMapping")
    mp.inputs["Scale"].default_value = scale
    nz = nt.nodes.new("ShaderNodeTexNoise")
    nz.inputs["Scale"].default_value = 18.0
    nz.inputs["Detail"].default_value = 6.0
    nz.inputs["Roughness"].default_value = 0.55
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].position = 0.35
    ramp.color_ramp.elements[0].color = (0.5, 0.5, 0.5, 1.0)
    ramp.color_ramp.elements[1].position = 0.85
    ramp.color_ramp.elements[1].color = (1.0, 1.0, 1.0, 1.0)
    mix = nt.nodes.new("ShaderNodeMixRGB")
    mix.blend_type = "MULTIPLY"
    mix.inputs["Fac"].default_value = strength
    mix.inputs["Color1"].default_value = (r0, g0, b0, 1.0)
    try:
        uv_socket = tc.outputs["UV"]
    except KeyError:
        uv_socket = tc.outputs["Generated"]
    nt.links.new(uv_socket, mp.inputs["Vector"])
    nt.links.new(mp.outputs["Vector"], nz.inputs["Vector"])
    nt.links.new(nz.outputs["Fac"], ramp.inputs["Fac"])
    nt.links.new(ramp.outputs["Color"], mix.inputs["Color2"])
    nt.links.new(mix.outputs["Color"], pr.inputs["Base Color"])
    _place([tc, mp, nz, ramp, mix], pr.location.x - 900, pr.location.y)


def _enhance_asphalt(nt: bpy.types.NodeTree, pr: bpy.types.ShaderNodeBsdfPrincipled) -> None:
    if pr.inputs["Base Color"].is_linked:
        return
    r0, g0, b0, _ = pr.inputs["Base Color"].default_value
    tc = nt.nodes.new("ShaderNodeTexCoord")
    mp = nt.nodes.new("ShaderNodeMapping")
    mp.inputs["Scale"].default_value = (22.0, 22.0, 22.0)
    vo = nt.nodes.new("ShaderNodeTexVoronoi")
    vo.voronoi_dimensions = "3D"
    vo.feature = "F1"
    vo.inputs["Scale"].default_value = 48.0
    ramp = nt.nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].position = 0.2
    ramp.color_ramp.elements[0].color = (0.25, 0.25, 0.28, 1.0)
    ramp.color_ramp.elements[1].position = 0.65
    ramp.color_ramp.elements[1].color = (1.0, 1.0, 1.0, 1.0)
    mix = nt.nodes.new("ShaderNodeMixRGB")
    mix.blend_type = "MULTIPLY"
    mix.inputs["Fac"].default_value = 0.72
    mix.inputs["Color1"].default_value = (r0, g0, b0, 1.0)
    try:
        uv_socket = tc.outputs["UV"]
    except KeyError:
        uv_socket = tc.outputs["Generated"]
    nt.links.new(uv_socket, mp.inputs["Vector"])
    nt.links.new(mp.outputs["Vector"], vo.inputs["Vector"])
    nt.links.new(vo.outputs["Distance"], ramp.inputs["Fac"])
    nt.links.new(ramp.outputs["Color"], mix.inputs["Color2"])
    nt.links.new(mix.outputs["Color"], pr.inputs["Base Color"])
    pr.inputs["Roughness"].default_value = 0.92
    _place([tc, mp, vo, ramp, mix], pr.location.x - 920, pr.location.y)


def _enhance_sign_flat(nt: bpy.types.NodeTree, pr: bpy.types.ShaderNodeBsdfPrincipled) -> None:
    """Retro: coarse UV grid banding on emissive-ish flat colors."""
    if pr.inputs["Base Color"].is_linked:
        return
    r0, g0, b0, _ = pr.inputs["Base Color"].default_value
    tc = nt.nodes.new("ShaderNodeTexCoord")
    mp = nt.nodes.new("ShaderNodeMapping")
    mp.inputs["Scale"].default_value = (96.0, 96.0, 1.0)
    nz = nt.nodes.new("ShaderNodeTexNoise")
    nz.inputs["Scale"].default_value = 320.0
    nz.inputs["Detail"].default_value = 1.0
    mix = nt.nodes.new("ShaderNodeMixRGB")
    mix.blend_type = "MULTIPLY"
    mix.inputs["Fac"].default_value = 0.18
    mix.inputs["Color1"].default_value = (r0, g0, b0, 1.0)
    nt.links.new(tc.outputs["UV"], mp.inputs["Vector"])
    nt.links.new(mp.outputs["Vector"], nz.inputs["Vector"])
    nt.links.new(nz.outputs["Color"], mix.inputs["Color2"])
    nt.links.new(mix.outputs["Color"], pr.inputs["Base Color"])
    _place([tc, mp, nz, mix], pr.location.x - 640, pr.location.y)


def _mark(nt: bpy.types.NodeTree) -> None:
    fr = nt.nodes.new("NodeFrame")
    fr.name = MARK
    fr.label = MARK
    fr.location = (-2400, -800)


def enhance_material(mat: bpy.types.Material, recipe: str) -> bool:
    if not mat.use_nodes or not mat.node_tree:
        return False
    nt = mat.node_tree
    if _marked(nt):
        return False
    pr = _principled(mat)
    if pr is None:
        return False
    try:
        if recipe == "brick":
            _enhance_brick(nt, pr)
        elif recipe == "concrete":
            _enhance_noise_tint(nt, pr, (9.0, 9.0, 9.0), 0.55)
        elif recipe == "asphalt":
            _enhance_asphalt(nt, pr)
        elif recipe == "trim":
            _enhance_noise_tint(nt, pr, (2.0, 28.0, 8.0), 0.42)
        elif recipe == "window":
            _enhance_noise_tint(nt, pr, (20.0, 20.0, 20.0), 0.22)
        elif recipe == "sign":
            _enhance_sign_flat(nt, pr)
        elif recipe == "neon":
            _enhance_noise_tint(nt, pr, (40.0, 40.0, 1.0), 0.15)
        else:
            return False
    except Exception as e:
        print(f"oyabaun: skip {mat.name}: {e}", file=sys.stderr)
        return False
    _mark(nt)
    return True


RECIPES: dict[str, str] = {
    "OYA_Building": "brick",
    "OYA_Concrete": "concrete",
    "OYA_Asphalt": "asphalt",
    "OYA_Trim": "trim",
    "OYA_Window": "window",
    "OYA_WindowWarm": "window",
    "OYA_Awning": "trim",
    "OYA_ACUnit": "concrete",
    "OYA_NeonCrimson": "neon",
    "OYA_NeonGold": "neon",
    "OYA_NeonTeal": "neon",
}

def main() -> None:
    n_del = _remove_generated_prop_collection()
    if n_del:
        print(f"oyabaun: removed {n_del} objects from collection {COL_TRASH}")

    done = 0
    for name, recipe in RECIPES.items():
        m = bpy.data.materials.get(name)
        if m and enhance_material(m, recipe):
            print(f"oyabaun: enhanced material {name} ({recipe})")
            done += 1

    prefixes_sign = ("ShopSign_", "Banner_", "Noren_")
    prefixes_neon = ("EmSign_", "Neon_")
    sign_exact = ("SignCream", "SignCrimson", "SignGold", "SignPink")

    for m in bpy.data.materials:
        if not m.use_nodes or not m.node_tree or _marked(m.node_tree):
            continue
        if m.name in RECIPES:
            continue
        n = m.name
        if n in sign_exact:
            if enhance_material(m, "sign"):
                print(f"oyabaun: enhanced material {n} (sign)")
                done += 1
            continue
        if n.startswith(prefixes_sign):
            if enhance_material(m, "sign"):
                print(f"oyabaun: enhanced material {n} (sign)")
                done += 1
            continue
        if n.startswith(prefixes_neon):
            if enhance_material(m, "neon"):
                print(f"oyabaun: enhanced material {n} (neon)")
                done += 1
            continue
        if (n.startswith("VM_") and n != "VM_Glass") or n in (
            "WoodCrate",
            "WoodFrame",
            "Trash",
            "Pot_Terra",
        ):
            if enhance_material(m, "concrete"):
                print(f"oyabaun: enhanced material {n} (concrete)")
                done += 1
            continue
        if n.startswith("Metal_"):
            if enhance_material(m, "trim"):
                print(f"oyabaun: enhanced material {n} (trim)")
                done += 1

    fp = bpy.data.filepath
    if fp:
        bpy.ops.wm.save_mainfile()
        print(f"oyabaun: saved {fp} ({done} materials touched)")
    else:
        print("oyabaun: unsaved blend", file=sys.stderr)


if __name__ == "__main__":
    main()
