#!/usr/bin/env python3
"""
Compare a pixel-art reference PNG to a rendered character PNG and print
actionable gaps for Oyabaun (Blender generator + Rust character shader).

Dependencies: Python 3.10+ stdlib only (PNG reader built-in).

From repo root:
  python3 tools/character_style_audit.py --ref example_images/ref-image.png

Renders client/characters/oyabaun_player.glb via Blender unless --candidate is set.
See tools/blender_character_capture.py for the headless render.

  --loop N   repeat every 30 seconds N times (for iterative art passes)
"""
from __future__ import annotations

import argparse
import glob
import math
import os
import shutil
import struct
import subprocess
import sys
import time
import zlib
from collections import Counter

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
AUDIT_OUT = os.path.join(ROOT, "tools", "_audit_out")
CAPTURE_SCRIPT = os.path.join(ROOT, "tools", "blender_character_capture.py")


def read_png_rgb(path: str) -> tuple[int, int, list[tuple[int, int, int]]]:
    """Load 8-bit RGBA or RGB PNG → list of (R,G,B) per pixel, row-major."""
    with open(path, "rb") as f:
        if f.read(8) != b"\x89PNG\r\n\x1a\n":
            raise ValueError(f"Not a PNG: {path}")
        w = h = None
        idat: list[bytes] = []
        bit_depth = color_type = interlace = None
        while True:
            len_b = f.read(4)
            if not len_b:
                break
            (ln,) = struct.unpack(">I", len_b)
            ctype = f.read(4)
            body = f.read(ln)
            f.read(4)
            if ctype == b"IHDR":
                w, h, bit_depth, color_type, _, _, interlace = struct.unpack(
                    ">IIBBBBB", body
                )
            elif ctype == b"IDAT":
                idat.append(body)
            elif ctype == b"IEND":
                break
    if w is None or bit_depth != 8 or interlace != 0:
        raise ValueError(f"Unsupported PNG layout: {path}")
    raw = zlib.decompress(b"".join(idat))
    if color_type == 6:
        bpp = 4
    elif color_type == 2:
        bpp = 3
    else:
        raise ValueError(f"Need RGB or RGBA PNG, got color_type={color_type}")

    stride = w * bpp
    pixels: list[tuple[int, int, int]] = []
    prev = bytearray(stride)
    pos = 0

    def paeth(a: int, b: int, c: int) -> int:
        p = a + b - c
        pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
        if pa <= pb and pa <= pc:
            return a
        if pb <= pc:
            return b
        return c

    for _y in range(h):
        ftype = raw[pos]
        pos += 1
        scan = bytearray(raw[pos : pos + stride])
        pos += stride
        recon = bytearray(stride)
        if ftype == 0:
            recon[:] = scan
        elif ftype == 1:
            for i in range(stride):
                left = recon[i - bpp] if i >= bpp else 0
                recon[i] = (scan[i] + left) & 0xFF
        elif ftype == 2:
            for i in range(stride):
                recon[i] = (scan[i] + prev[i]) & 0xFF
        elif ftype == 3:
            for i in range(stride):
                left = recon[i - bpp] if i >= bpp else 0
                up = prev[i]
                recon[i] = (scan[i] + ((left + up) // 2)) & 0xFF
        elif ftype == 4:
            for i in range(stride):
                a = recon[i - bpp] if i >= bpp else 0
                b = prev[i]
                c = prev[i - bpp] if i >= bpp else 0
                recon[i] = (scan[i] + paeth(a, b, c)) & 0xFF
        else:
            raise ValueError(f"Unsupported PNG filter {ftype}")
        prev = recon
        if bpp == 4:
            for i in range(0, stride, 4):
                pixels.append((recon[i], recon[i + 1], recon[i + 2]))
        else:
            for i in range(0, stride, 3):
                pixels.append((recon[i], recon[i + 1], recon[i + 2]))
    return w, h, pixels


def resize_pixels(
    w: int, h: int, pixels: list[tuple[int, int, int]], tw: int, th: int
) -> list[tuple[int, int, int]]:
    out: list[tuple[int, int, int]] = []
    for y in range(th):
        sy = min(h - 1, int(y * h / th))
        row0 = sy * w
        for x in range(tw):
            sx = min(w - 1, int(x * w / tw))
            out.append(pixels[row0 + sx])
    return out


def default_ref_path() -> str | None:
    for pattern in (
        os.path.join(ROOT, "example_images", "ref-pixel-yakuza.png"),
        os.path.join(ROOT, "example_images", "ref-image.png"),
    ):
        if os.path.isfile(pattern):
            return pattern
    matches = sorted(glob.glob(os.path.join(ROOT, "example_images", "*.png")))
    return matches[0] if matches else None


def luminance(rgb: tuple[int, int, int]) -> float:
    r, g, b = rgb
    return (0.299 * r + 0.587 * g + 0.114 * b) / 255.0


def rgb_to_hsv(r: int, g: int, b: int) -> tuple[float, float, float]:
    r, g, b = r / 255.0, g / 255.0, b / 255.0
    mx = max(r, g, b)
    mn = min(r, g, b)
    df = mx - mn
    if mx == mn:
        h = 0.0
    elif mx == r:
        h = (60 * ((g - b) / df) + 360) % 360
    elif mx == g:
        h = (60 * ((b - r) / df) + 120) % 360
    else:
        h = (60 * ((r - g) / df) + 240) % 360
    s = 0.0 if mx == 0 else df / mx
    v = mx
    return h, s, v


def image_metrics(pixels: list[tuple[int, int, int]], tw: int, th: int) -> dict:
    lums = [luminance(p) for p in pixels]
    mean_lum = sum(lums) / len(lums)
    var_lum = sum((x - mean_lum) ** 2 for x in lums) / len(lums)

    sats = [rgb_to_hsv(*p)[1] for p in pixels]
    mean_sat = sum(sats) / len(sats)

    dif2: list[float] = []
    for y in range(th):
        for x in range(tw - 2):
            i = y * tw + x
            j = y * tw + x + 2
            dif2.append(abs(lums[i] - lums[j]))
    for y in range(th - 2):
        for x in range(tw):
            i = y * tw + x
            j = (y + 2) * tw + x
            dif2.append(abs(lums[i] - lums[j]))
    checker_proxy = sum(dif2) / len(dif2) if dif2 else 0.0

    mean_r = sum(p[0] for p in pixels) / len(pixels) / 255.0
    mean_g = sum(p[1] for p in pixels) / len(pixels) / 255.0
    red_bias = mean_r - mean_g

    bucket = 28
    keys = [
        (
            (r // bucket) * bucket,
            (g // bucket) * bucket,
            (b // bucket) * bucket,
        )
        for r, g, b in pixels
    ]
    top_colors = Counter(keys).most_common(14)

    return {
        "mean_lum": mean_lum,
        "var_lum": var_lum,
        "mean_sat": mean_sat,
        "checker_proxy": checker_proxy,
        "red_bias": red_bias,
        "top_colors": top_colors,
        "tw": tw,
        "th": th,
    }


def palette_distance(top_a, top_b) -> float:
    if not top_a or not top_b:
        return 1.0
    dists = []
    for (ra, ga, ba), _ in top_a[:10]:
        best = 999.0
        for (rb, gb, bb), __ in top_b[:14]:
            d = math.sqrt((ra - rb) ** 2 + (ga - gb) ** 2 + (ba - bb) ** 2) / 441.0
            best = min(best, d)
        dists.append(best)
    return sum(dists) / len(dists)


def find_blender(explicit: str | None) -> str | None:
    if explicit and os.path.isfile(explicit):
        return explicit
    env = os.environ.get("OYABAUN_BLENDER")
    if env and os.path.isfile(env):
        return env
    mac = "/Applications/Blender.app/Contents/MacOS/Blender"
    if os.path.isfile(mac):
        return mac
    return shutil.which("blender") or shutil.which("Blender")


def render_glb(blender_bin: str, glb: str, out_png: str) -> None:
    os.makedirs(os.path.dirname(out_png) or ".", exist_ok=True)
    cmd = [
        blender_bin,
        "--background",
        "--python",
        CAPTURE_SCRIPT,
        "--",
        os.path.abspath(glb),
        os.path.abspath(out_png),
    ]
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    if r.returncode != 0:
        print(r.stdout, file=sys.stderr)
        print(r.stderr, file=sys.stderr)
        raise RuntimeError(f"Blender capture failed ({r.returncode})")


def build_instructions(ref_m, cand_m, pal_dist: float) -> list[str]:
    lines: list[str] = []
    ratio_chk = cand_m["checker_proxy"] / max(ref_m["checker_proxy"], 0.012)
    if ratio_chk > 1.45:
        lines.append(
            "[CHECKERBOARD / GRID] Candidate has stronger 2px-ish contrast than the ref.\n"
            "  → tools/blender_build_oyabaun_characters_3d.py: NO 2×2 shirt tiles; suit = vertical folds "
            "+ hash noise at 96²; avoid Bayer as the main suit signal.\n"
            "  → client/src/render.rs SHADER_CHAR_TEX: reduce cel Bayer (d4 term) and stipple strength "
            "so screen dither does not stack on texels."
        )

    if cand_m["mean_sat"] > ref_m["mean_sat"] * 1.22:
        lines.append(
            "[SATURATION] Render hotter than ref.\n"
            "  → Darken texture bases; reduce rim_col / neon_spill in fs_char; pull palette midtones wine."
        )
    elif cand_m["mean_sat"] < ref_m["mean_sat"] * 0.78:
        lines.append(
            "[SATURATION] Render dull vs ref.\n"
            "  → tools/blender_character_capture.py: raise RimPink energy; in-game bump rim + neon slightly."
        )

    if cand_m["mean_lum"] > ref_m["mean_lum"] * 1.25:
        lines.append(
            "[LUMINANCE] Too bright.\n"
            "  → Darken suit/skin; reduce spec_step in fs_char."
        )
    elif cand_m["mean_lum"] < ref_m["mean_lum"] * 0.75:
        lines.append(
            "[LUMINANCE] Too dark.\n"
            "  → Lift textures; reduce ink mix in fs_char; increase fill in capture."
        )

    if cand_m["var_lum"] < ref_m["var_lum"] * 0.55:
        lines.append(
            "[CONTRAST] Too flat vs ref.\n"
            "  → Widen suit shadow/highlight separation in textures; check decimate/subsurf."
        )

    if pal_dist > 0.22:
        lines.append(
            f"[PALETTE] Histogram far from ref (dist={pal_dist:.3f}).\n"
            "  → Re-anchor textures: navy shadow, wine mid, cream highlight; fewer random accent pixels."
        )

    if cand_m["red_bias"] + 0.04 < ref_m["red_bias"]:
        lines.append(
            "[WARMTH] Ref is redder / more magenta.\n"
            "  → Shift shadows blue-red; align rim pink; audit lights in blender_character_capture.py."
        )

    if not lines:
        lines.append(
            "[METRICS OK] Numbers near ref — if it still looks wrong, the gap is silhouette/detail: "
            "widen joint radii, add hand-painted PNG albedos, or capture from in-game frame (--candidate)."
        )

    lines.append(
        "\nRe-run:\n  python3 tools/character_style_audit.py --ref <ref.png>\n"
        "Loop:  python3 tools/character_style_audit.py --ref ... --loop 12"
    )
    return lines


def prepare_image(path: str, target_w: int) -> tuple[list[tuple[int, int, int]], int, int]:
    w, h, pix = read_png_rgb(path)
    th = max(1, int(h * (target_w / w)))
    small = resize_pixels(w, h, pix, target_w, th)
    return small, target_w, th


def run_once(args) -> str:
    ref_path = args.ref or default_ref_path()
    if not ref_path or not os.path.isfile(ref_path):
        print("No --ref and no example_images PNG found.", file=sys.stderr)
        sys.exit(2)

    if args.candidate:
        cand_path = args.candidate
        if not os.path.isfile(cand_path):
            print(f"Missing candidate: {cand_path}", file=sys.stderr)
            sys.exit(2)
    else:
        os.makedirs(AUDIT_OUT, exist_ok=True)
        cand_path = os.path.join(AUDIT_OUT, "boss_render.png")
        bb = find_blender(args.blender)
        if not bb:
            print(
                "Blender not found. Set OYABAUN_BLENDER or pass --candidate screenshot.png",
                file=sys.stderr,
            )
            sys.exit(3)
        glb = args.glb if os.path.isabs(args.glb) else os.path.join(ROOT, args.glb)
        if not os.path.isfile(glb):
            print(f"GLB not found: {glb}", file=sys.stderr)
            sys.exit(2)
        print(f"Rendering {glb} with {bb} …")
        render_glb(bb, glb, cand_path)

    ref_pix, rw, rh = prepare_image(ref_path, 360)
    cand_pix, cw, ch = prepare_image(cand_path, rw)
    if ch != rh:
        cand_pix = resize_pixels(cw, ch, cand_pix, rw, rh)
        ch = rh

    ref_m = image_metrics(ref_pix, rw, rh)
    cand_m = image_metrics(cand_pix, rw, rh)
    pal_d = palette_distance(ref_m["top_colors"], cand_m["top_colors"])

    report = [
        "Oyabaun character style audit",
        f"Reference: {ref_path}",
        f"Candidate: {cand_path}",
        "",
        f"mean_lum   ref={ref_m['mean_lum']:.4f}  cand={cand_m['mean_lum']:.4f}",
        f"var_lum    ref={ref_m['var_lum']:.4f}  cand={cand_m['var_lum']:.4f}",
        f"mean_sat   ref={ref_m['mean_sat']:.4f}  cand={cand_m['mean_sat']:.4f}",
        "checker_px ref={:.4f}  cand={:.4f}  ratio={:.2f}".format(
            ref_m["checker_proxy"],
            cand_m["checker_proxy"],
            cand_m["checker_proxy"] / max(ref_m["checker_proxy"], 0.012),
        ),
        f"red_bias   ref={ref_m['red_bias']:.4f}  cand={cand_m['red_bias']:.4f}",
        f"palette_dist={pal_d:.4f}",
        "",
    ]
    report.extend(build_instructions(ref_m, cand_m, pal_d))
    report.append("")
    report.append("Ref top colors (RGB):")
    for rgb, n in ref_m["top_colors"][:8]:
        report.append(f"  {rgb}  n={n}")
    report.append("Candidate top colors:")
    for rgb, n in cand_m["top_colors"][:8]:
        report.append(f"  {rgb}  n={n}")

    text = "\n".join(report)
    os.makedirs(AUDIT_OUT, exist_ok=True)
    with open(os.path.join(AUDIT_OUT, "LAST_AUDIT_REPORT.txt"), "w", encoding="utf-8") as f:
        f.write(text)
    return text


def main():
    ap = argparse.ArgumentParser(description="Reference vs character render style audit")
    ap.add_argument("--ref", default=None, help="Reference PNG")
    ap.add_argument("--candidate", default=None, help="Skip Blender; use this PNG")
    ap.add_argument(
        "--glb",
        default="client/characters/oyabaun_player.glb",
        help="GLB for Blender capture",
    )
    ap.add_argument("--blender", default=None, help="Blender executable")
    ap.add_argument("--loop", type=int, default=0, help="Repeat every 30s, N times")
    args = ap.parse_args()

    if args.loop and args.loop > 0:
        for i in range(args.loop):
            print(f"\n=== Audit pass {i + 1}/{args.loop} ===\n")
            print(run_once(args))
            if i + 1 < args.loop:
                time.sleep(30)
    else:
        print(run_once(args))


if __name__ == "__main__":
    main()
