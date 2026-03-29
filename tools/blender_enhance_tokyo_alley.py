"""
Tokyo alley: remove legacy generated props and assign **packed pixel albedos** so glTF export
embeds real textures. Blender's glTF exporter does **not** bake procedural node trees — those
became baseColorFactor (1,1,1) with no images → white level in WASM.

Run:

  /path/to/Blender client/levels/tokyo_alley.blend --background \\
    --python tools/blender_enhance_tokyo_alley.py

Then:

  python3 tools/oyabaunctl.py export-world --blend client/levels/tokyo_alley.blend

Art-direction: repo-root **example_images/** (90s pixel Tokyo). Does not add lane obstacles.

Re-running skips materials already linked to **OyabaunPx_*** images. Set **OYABAUN_REPACK_ALBEDOS=1** to rebuild all.
"""
from __future__ import annotations

import math
import os
import sys
from typing import Callable

import bpy

MARK_SKIP = "OyabaunPx_"
COL_TRASH = "OyabaunTokyoDetail"
TEX_SIZE = 96

# When the blend stores (1,1,1) after failed procedural export, use neo-Tokyo defaults.
_FALLBACK_RGB: dict[str, tuple[float, float, float]] = {
    "OYA_Building": (0.36, 0.30, 0.42),
    "OYA_Concrete": (0.44, 0.43, 0.46),
    "OYA_Asphalt": (0.22, 0.22, 0.24),
    "OYA_Trim": (0.52, 0.50, 0.55),
    "OYA_Window": (0.12, 0.18, 0.35),
    "OYA_WindowWarm": (0.35, 0.22, 0.12),
    "OYA_Awning": (0.25, 0.22, 0.38),
    "OYA_ACUnit": (0.48, 0.48, 0.50),
    "OYA_NeonCrimson": (0.95, 0.15, 0.35),
    "OYA_NeonGold": (0.98, 0.82, 0.25),
    "OYA_NeonTeal": (0.15, 0.85, 0.78),
    "SignCream": (0.92, 0.88, 0.72),
    "SignCrimson": (0.75, 0.12, 0.22),
    "SignGold": (0.90, 0.72, 0.22),
    "SignPink": (0.92, 0.35, 0.62),
    "EmSign_Blue": (0.2, 0.45, 0.95),
    "EmSign_Cyan": (0.15, 0.88, 0.92),
    "EmSign_Green": (0.2, 0.85, 0.35),
    "EmSign_Pink": (0.95, 0.25, 0.65),
    "EmSign_Purple": (0.55, 0.2, 0.92),
    "EmSign_Red": (0.95, 0.18, 0.2),
    "EmSign_Yellow": (0.98, 0.88, 0.2),
    "Neon_Blue": (0.22, 0.4, 0.98),
    "Neon_Cyan": (0.2, 0.9, 0.95),
    "Neon_Green": (0.25, 0.92, 0.4),
    "Neon_Orange": (0.98, 0.55, 0.15),
    "Neon_Pink": (0.98, 0.35, 0.72),
    "Neon_Purple": (0.62, 0.22, 0.95),
    "Neon_Red": (0.95, 0.15, 0.22),
    "Neon_Yellow": (0.95, 0.9, 0.22),
    "VM_Glass": (0.75, 0.82, 0.88),
    "WoodCrate": (0.45, 0.32, 0.22),
    "WoodFrame": (0.38, 0.28, 0.2),
    "Trash": (0.18, 0.17, 0.19),
    "Pot_Terra": (0.52, 0.28, 0.2),
}


def _remove_generated_prop_collection() -> int:
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


def _hash12(x: int, y: int) -> float:
    n = (x * 374761393 + y * 668265263) & 0xFFFFFFFF
    n = (n ^ (n >> 13)) * 1274126177 & 0xFFFFFFFF
    return ((n ^ (n >> 16)) & 0xFFFFFFFF) / 4294967296.0


def _disp_to_linear(c: float) -> float:
    """Pixels in Blender images are scene-linear; our ramps are display-like 0..1."""
    c = max(0.0, min(1.0, c))
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def _rgba_lin(rr: float, gg: float, bb: float, aa: float = 1.0) -> tuple[float, float, float, float]:
    return (
        _disp_to_linear(rr),
        _disp_to_linear(gg),
        _disp_to_linear(bb),
        max(0.0, min(1.0, aa)),
    )


BAYER4 = (
    0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5
)


def _dither_levels(r: float, g: float, b: float, x: int, y: int, levels: int = 10) -> tuple[float, float, float]:
    t = BAYER4[(y % 4) * 4 + (x % 4)] / 16.0 * (1.0 / levels)

    def q(ch: float) -> float:
        u = ch**0.45
        uq = math.floor((u + t) * levels) / levels
        return uq**2.2

    return q(r), q(g), q(b)


def _pix_brick(r0: float, g0: float, b0: float, w: int, h: int) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            row = y // 5
            ox = (row % 2) * 5
            bx = x + ox
            bw, bh = 14, 5
            bx_m = bx % bw
            by_m = y % bh
            mortar = bx_m < 1 or by_m < 1
            if mortar:
                rr, gg, bb = r0 * 0.38, g0 * 0.35, b0 * 0.42
            else:
                hv = _hash12(x // 3, y // 3)
                rr = r0 * (0.78 + hv * 0.22)
                gg = g0 * (0.78 + _hash12(x // 3 + 7, y // 3) * 0.22)
                bb = b0 * (0.78 + _hash12(x // 3 + 3, y // 3 + 2) * 0.22)
            rr, gg, bb = _dither_levels(rr, gg, bb, x, y)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb)
            out.extend([lr, lg, lb, la])
    return out


def _pix_noise(r0: float, g0: float, b0: float, w: int, h: int, scale: float = 14.0) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            n = _hash12(int(x * scale / w * 50), int(y * scale / h * 50))
            n2 = _hash12(x, y)
            rr = r0 * (0.82 + n * 0.18) * (0.92 + n2 * 0.08)
            gg = g0 * (0.82 + _hash12(x + 3, y) * 0.18)
            bb = b0 * (0.82 + _hash12(x, y + 9) * 0.18)
            rr, gg, bb = _dither_levels(rr, gg, bb, x, y, levels=12)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb)
            out.extend([lr, lg, lb, la])
    return out


def _pix_asphalt(r0: float, g0: float, b0: float, w: int, h: int) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            cx, cy = x / w, y / h
            cell_x = int(cx * 11)
            cell_y = int(cy * 11)
            d = abs(_hash12(cell_x, cell_y) - 0.35) * 0.4
            speck = _hash12(x, y) * 0.12
            rr = r0 * (0.55 + d + speck)
            gg = g0 * (0.55 + d * 0.9 + speck)
            bb = b0 * (0.55 + d * 0.95 + speck)
            rr, gg, bb = _dither_levels(rr, gg, bb, x, y, levels=9)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb)
            out.extend([lr, lg, lb, la])
    return out


def _pix_trim(r0: float, g0: float, b0: float, w: int, h: int) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            scratch = abs(math.sin((x + y) * 0.31)) * 0.08
            n = _hash12(x // 2, y // 4) * 0.14
            rr = r0 * (0.75 + scratch + n)
            gg = g0 * (0.75 + scratch + n)
            bb = b0 * (0.75 + scratch + n)
            rr, gg, bb = _dither_levels(rr, gg, bb, x, y)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb)
            out.extend([lr, lg, lb, la])
    return out


def _pix_window(r0: float, g0: float, b0: float, w: int, h: int, warm: bool) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            streak = (math.sin(x * 0.35) * 0.5 + 0.5) * 0.15
            vign = 1.0 - ((x / w - 0.5) ** 2 + (y / h - 0.5) ** 2) * 0.35
            if warm:
                rr = r0 * (0.65 + streak) * vign
                gg = g0 * (0.55 + streak * 0.8) * vign
                bb = b0 * (0.25 + streak * 0.3) * vign
            else:
                rr = r0 * (0.4 + streak * 0.5) * vign
                gg = g0 * (0.45 + streak * 0.6) * vign
                bb = b0 * (0.65 + streak) * vign
            rr, gg, bb = _dither_levels(rr, gg, bb, x, y, levels=8)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb)
            out.extend([lr, lg, lb, la])
    return out


def _pix_awning(r0: float, g0: float, b0: float, w: int, h: int) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            stripe = (x // 4) % 2
            m = 0.76 if stripe else 1.14
            n = _hash12(x, y) * 0.08
            rr = min(1.0, r0 * m + n)
            gg = min(1.0, g0 * m + n)
            bb = min(1.0, b0 * m + n)
            rr, gg, bb = _dither_levels(rr, gg, bb, x, y)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb)
            out.extend([lr, lg, lb, la])
    return out


def _pix_neon(r0: float, g0: float, b0: float, w: int, h: int) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            scan = 0.88 if (y % 5) < 1 else 1.0
            bleed = _hash12(x // 2, y // 2) * 0.07
            rr = min(1.0, r0 * scan + bleed)
            gg = min(1.0, g0 * scan + bleed)
            bb = min(1.0, b0 * scan + bleed)
            rr, gg, bb = _dither_levels(rr, gg, bb, x, y, levels=14)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb)
            out.extend([lr, lg, lb, la])
    return out


def _pix_sign(r0: float, g0: float, b0: float, w: int, h: int, seed: int) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            band = _hash12(x // 6 + seed, y // 8) * 0.12
            rr = min(1.0, r0 * (0.9 + band))
            gg = min(1.0, g0 * (0.9 + band))
            bb = min(1.0, b0 * (0.9 + band))
            rr, gg, bb = _dither_levels(rr, gg, bb, x, y, levels=11)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb)
            out.extend([lr, lg, lb, la])
    return out


def _pix_wood(r0: float, g0: float, b0: float, w: int, h: int) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            grain = (y % 7) / 7.0 * 0.15
            n = _hash12(x // 3, y) * 0.1
            rr = r0 * (0.7 + grain + n)
            gg = g0 * (0.65 + grain + n)
            bb = b0 * (0.5 + grain * 0.8 + n)
            rr, gg, bb = _dither_levels(rr, gg, bb, x, y)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb)
            out.extend([lr, lg, lb, la])
    return out


def _pix_glass(r0: float, g0: float, b0: float, w: int, h: int) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            n = _hash12(x, y) * 0.05
            rr = min(1.0, r0 * 0.92 + n)
            gg = min(1.0, g0 * 0.95 + n)
            bb = min(1.0, b0 * 1.0 + n)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb, 0.88)
            out.extend([lr, lg, lb, la])
    return out


def _pix_vending(r0: float, g0: float, b0: float, w: int, h: int) -> list[float]:
    out: list[float] = []
    for y in range(h):
        for x in range(w):
            panel = (x // 12) % 2
            slot = (y % 6) < 1
            base = 0.85 if panel else 1.0
            if slot:
                base *= 0.75
            n = _hash12(x, y) * 0.08
            rr = min(1.0, r0 * base + n)
            gg = min(1.0, g0 * base + n)
            bb = min(1.0, b0 * base + n)
            rr, gg, bb = _dither_levels(rr, gg, bb, x, y)
            lr, lg, lb, la = _rgba_lin(rr, gg, bb)
            out.extend([lr, lg, lb, la])
    return out


def _already_packed(mat: bpy.types.Material) -> bool:
    if not mat.use_nodes or not mat.node_tree:
        return False
    for n in mat.node_tree.nodes:
        if n.type != "BSDF_PRINCIPLED":
            continue
        sock = n.inputs.get("Base Color")
        if not sock or not sock.is_linked:
            continue
        ln = sock.links[0].from_node
        if ln.type == "TEX_IMAGE" and ln.image and ln.image.name.startswith(MARK_SKIP):
            return True
    return False


def _drop_orphan_packed_images() -> None:
    for prefix in ("OyabaunAlb_", "OyabaunPx_"):
        for img in list(bpy.data.images):
            if not img.name.startswith(prefix):
                continue
            try:
                if img.users == 0:
                    bpy.data.images.remove(img)
            except Exception:
                pass


def _read_base_rgb(mat: bpy.types.Material) -> tuple[float, float, float]:
    if not mat.use_nodes or not mat.node_tree:
        return _FALLBACK_RGB.get(mat.name) or _fallback_for_prefix(mat.name) or (0.48, 0.47, 0.5)
    for n in mat.node_tree.nodes:
        if n.type == "BSDF_PRINCIPLED":
            v = n.inputs["Base Color"].default_value
            r, g, b = float(v[0]), float(v[1]), float(v[2])
            if r > 0.92 and g > 0.92 and b > 0.92:
                return _FALLBACK_RGB.get(mat.name) or _fallback_for_prefix(mat.name) or (0.48, 0.47, 0.5)
            return (r, g, b)
    return _FALLBACK_RGB.get(mat.name) or _fallback_for_prefix(mat.name) or (0.48, 0.47, 0.5)


def _fallback_for_prefix(name: str) -> tuple[float, float, float] | None:
    if name.startswith("ShopSign_"):
        return (0.82, 0.28, 0.32)
    if name.startswith("Banner_"):
        return (0.28, 0.52, 0.82)
    if name.startswith("Noren_"):
        return (0.25, 0.42, 0.38)
    if name.startswith("VM_") and name != "VM_Glass":
        return (0.42, 0.4, 0.44)
    return None


def _clear_nodes(nt: bpy.types.NodeTree) -> None:
    for n in list(nt.nodes):
        nt.nodes.remove(n)


def _build_material_image(
    mat: bpy.types.Material,
    image_name: str,
    pixels: list[float],
    width: int,
    height: int,
    *,
    roughness: float = 0.88,
    metallic: float = 0.0,
    emission: tuple[float, float, float] | None = None,
    emission_strength: float = 0.0,
) -> None:
    if image_name in bpy.data.images:
        img = bpy.data.images[image_name]
        bpy.data.images.remove(img)
    img = bpy.data.images.new(image_name, width, height, alpha=True)
    img.alpha_mode = "STRAIGHT"
    if len(pixels) != width * height * 4:
        raise ValueError(f"pixel len {len(pixels)} != {width * height * 4}")
    img.pixels.foreach_set(pixels)
    img.pack()
    try:
        img.colorspace_settings.name = "sRGB"
    except Exception:
        for cs in ("Linear Rec.709", "Non-Color"):
            try:
                img.colorspace_settings.name = cs
                break
            except Exception:
                continue

    nt = mat.node_tree
    if not nt:
        mat.use_nodes = True
        nt = mat.node_tree
    _clear_nodes(nt)

    out = nt.nodes.new("ShaderNodeOutputMaterial")
    pr = nt.nodes.new("ShaderNodeBsdfPrincipled")
    tex = nt.nodes.new("ShaderNodeTexImage")
    tc = nt.nodes.new("ShaderNodeTexCoord")
    mp = nt.nodes.new("ShaderNodeMapping")
    mp.inputs["Scale"].default_value = (3.0, 3.0, 3.0)

    tex.image = img
    tex.interpolation = "Closest"
    tex.extension = "REPEAT"

    nt.links.new(tc.outputs["UV"], mp.inputs["Vector"])
    nt.links.new(mp.outputs["Vector"], tex.inputs["Vector"])
    nt.links.new(tex.outputs["Color"], pr.inputs["Base Color"])

    pr.inputs["Roughness"].default_value = roughness
    pr.inputs["Metallic"].default_value = metallic
    pr.inputs["Base Color"].default_value = (1.0, 1.0, 1.0, 1.0)

    if emission and emission_strength > 0.0:
        pr.inputs["Emission Color"].default_value = (*emission, 1.0)
        pr.inputs["Emission Strength"].default_value = emission_strength

    out.location = (340, 0)
    pr.location = (80, 0)
    tex.location = (-280, 0)
    mp.location = (-520, 0)
    tc.location = (-740, 0)

    nt.links.new(pr.outputs["BSDF"], out.inputs["Surface"])


RecipeFn = Callable[[float, float, float, int, int], list[float]]


def _recipe_for_material(name: str) -> tuple[str, RecipeFn, float, float, bool]:
    """Returns (image_suffix, pix_fn, roughness, metallic, use_emission_hint)."""
    if name == "OYA_Building":
        return ("brick", _pix_brick, 0.92, 0.0, False)
    if name in ("OYA_Concrete", "OYA_ACUnit", "Trash", "Pot_Terra"):
        return ("concrete", _pix_noise, 0.9, 0.0, False)
    if name == "OYA_Asphalt":
        return ("asphalt", _pix_asphalt, 0.95, 0.0, False)
    if name == "OYA_Trim":
        return ("trim", _pix_trim, 0.78, 0.15, False)
    if name == "OYA_Window":
        return ("window", lambda r, g, b, w, h: _pix_window(r, g, b, w, h, False), 0.25, 0.0, False)
    if name == "OYA_WindowWarm":
        return ("window_warm", lambda r, g, b, w, h: _pix_window(r, g, b, w, h, True), 0.28, 0.0, False)
    if name == "OYA_Awning":
        return ("awning", _pix_awning, 0.85, 0.0, False)
    if name.startswith("OYA_Neon"):
        return ("neon", _pix_neon, 0.45, 0.0, True)
    if name.startswith("Metal_"):
        return ("metal", _pix_trim, 0.55, 0.65, False)
    if name == "VM_Glass":
        return ("glass", _pix_glass, 0.15, 0.0, False)
    if name.startswith("VM_"):
        return ("vend", _pix_vending, 0.82, 0.05, False)
    if name in ("WoodCrate", "WoodFrame"):
        return ("wood", _pix_wood, 0.88, 0.0, False)
    if name.startswith(("ShopSign_", "Banner_", "Noren_")) or name in (
        "SignCream",
        "SignCrimson",
        "SignGold",
        "SignPink",
    ):
        return ("sign", _pix_sign, 0.75, 0.0, False)
    if name.startswith(("EmSign_", "Neon_")):
        return ("emit", _pix_neon, 0.35, 0.0, True)
    return ("generic", _pix_noise, 0.88, 0.0, False)


def _apply_material(mat: bpy.types.Material) -> bool:
    if mat.name.startswith(("Gun_", "FPS_", "OYA_ShopFace_")):
        return False
    if not os.environ.get("OYABAUN_REPACK_ALBEDOS") and _already_packed(mat):
        return False
    r0, g0, b0 = _read_base_rgb(mat)
    suf, fn, rough, metal, neon_hint = _recipe_for_material(mat.name)
    seed = hash(mat.name) % 10000
    w = h = TEX_SIZE
    if suf == "sign":
        pix = fn(r0, g0, b0, w, h, seed)
    else:
        pix = fn(r0, g0, b0, w, h)
    safe = "".join(c if c.isalnum() or c in "._-" else "_" for c in mat.name)
    iname = f"{MARK_SKIP}{safe}_{suf}"
    em = None
    em_str = 0.0
    if neon_hint:
        em = (
            min(1.0, r0 * 1.12),
            min(1.0, g0 * 1.12),
            min(1.0, b0 * 1.12),
        )
        if mat.name.startswith(("EmSign_", "Neon_")):
            em_str = 2.2
        elif mat.name.startswith("OYA_Neon"):
            em_str = 1.6
        else:
            em_str = 1.0
    try:
        _build_material_image(
            mat,
            iname,
            pix,
            w,
            h,
            roughness=rough,
            metallic=metal,
            emission=em,
            emission_strength=em_str,
        )
    except Exception as e:
        print(f"oyabaun: skip {mat.name}: {e}", file=sys.stderr)
        return False
    return True


def _used_mesh_materials() -> list[bpy.types.Material]:
    seen: set[bpy.types.Material] = set()
    for o in bpy.data.objects:
        if o.type != "MESH":
            continue
        for slot in o.material_slots:
            if slot.material:
                seen.add(slot.material)
    return list(seen)


def main() -> None:
    n_del = _remove_generated_prop_collection()
    if n_del:
        print(f"oyabaun: removed {n_del} objects from collection {COL_TRASH}")

    done = 0
    for mat in _used_mesh_materials():
        if not mat.use_nodes:
            mat.use_nodes = True
        if _apply_material(mat):
            print(f"oyabaun: packed albedo {mat.name}")
            done += 1

    _drop_orphan_packed_images()

    fp = bpy.data.filepath
    if fp:
        bpy.ops.wm.save_mainfile()
        print(f"oyabaun: saved {fp} ({done} materials packed)")
    else:
        print("oyabaun: unsaved blend", file=sys.stderr)


if __name__ == "__main__":
    main()
