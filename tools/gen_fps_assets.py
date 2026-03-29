#!/usr/bin/env python3
"""Generate repo-committed FP weapon HUD PNGs + VFX (pixel style). Run from repo root."""
from __future__ import annotations

import math
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:
    print("pip install Pillow", file=sys.stderr)
    sys.exit(1)

ROOT = Path(__file__).resolve().parent.parent
OUT_GUN = ROOT / "client" / "fpsweapons"
OUT_VFX = ROOT / "client" / "vfx"
W = 512


def line(draw, a, b, fill, wpx=2):
    draw.line([a, b], fill=fill, width=wpx)


def poly(draw, pts, fill, outline=None):
    draw.polygon(pts, fill=fill, outline=outline)


def pistol() -> Image.Image:
    im = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    cx, cy = 220, 340
    # Grip (viewer left / bottom)
    poly(
        d,
        [
            (cx - 55, cy + 120),
            (cx - 35, cy + 165),
            (cx + 25, cy + 155),
            (cx + 15, cy + 95),
        ],
        (28, 30, 38, 255),
        (12, 12, 16, 255),
    )
    # Frame / trigger guard
    poly(
        d,
        [
            (cx - 20, cy + 40),
            (cx - 55, cy + 115),
            (cx + 20, cy + 100),
            (cx + 45, cy + 35),
        ],
        (44, 48, 58, 255),
        (16, 18, 22, 255),
    )
    # Slide (along barrel axis → up-right)
    poly(
        d,
        [
            (cx + 40, cy + 25),
            (cx + 195, cy - 95),
            (cx + 210, cy - 75),
            (cx + 55, cy + 35),
        ],
        (180, 186, 198, 255),
        (60, 64, 72, 255),
    )
    # Barrel tip
    poly(
        d,
        [
            (cx + 185, cy - 88),
            (cx + 248, cy - 128),
            (cx + 255, cy - 118),
            (cx + 198, cy - 78),
        ],
        (120, 124, 132, 255),
        (40, 42, 48, 255),
    )
    # Trigger
    line(d, (cx - 8, cy + 72), (cx - 22, cy + 98), (90, 88, 95, 255), 3)
    return im


def shotgun() -> Image.Image:
    im = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    cx, cy = 200, 320
    # Stock / grip (wood tone)
    poly(
        d,
        [
            (cx - 120, cy + 140),
            (cx - 95, cy + 175),
            (cx - 20, cy + 155),
            (cx - 35, cy + 105),
        ],
        (92, 58, 38, 255),
        (40, 26, 18, 255),
    )
    # Receiver
    poly(
        d,
        [
            (cx - 35, cy + 95),
            (cx - 25, cy + 35),
            (cx + 85, cy + 15),
            (cx + 95, cy + 85),
        ],
        (48, 52, 58, 255),
        (18, 20, 24, 255),
    )
    # Twin barrels (horizontal, pointing up-right)
    for dy in (-18, 10):
        poly(
            d,
            [
                (cx + 88, cy + 18 + dy),
                (cx + 268, cy - 72 + dy),
                (cx + 275, cy - 58 + dy),
                (cx + 95, cy + 32 + dy),
            ],
            (160, 165, 175, 255),
            (50, 52, 58, 255),
        )
    # Pump forend
    poly(
        d,
        [
            (cx + 30, cy + 55),
            (cx + 115, cy + 20),
            (cx + 125, cy + 48),
            (cx + 40, cy + 82),
        ],
        (70, 74, 82, 255),
        (28, 30, 34, 255),
    )
    return im


def smg() -> Image.Image:
    im = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    cx, cy = 210, 300
    # Compact body
    poly(
        d,
        [
            (cx - 30, cy + 130),
            (cx - 15, cy + 40),
            (cx + 140, cy - 30),
            (cx + 155, cy + 55),
        ],
        (42, 46, 52, 255),
        (16, 18, 22, 255),
    )
    # Top rail
    line(d, (cx + 5, cy + 35), (cx + 150, cy - 35), (58, 60, 66, 255), 4)
    # Mag (vertical, below grip)
    poly(
        d,
        [
            (cx - 5, cy + 95),
            (cx + 18, cy + 95),
            (cx + 28, cy + 175),
            (cx - 15, cy + 175),
        ],
        (36, 38, 44, 255),
        (14, 15, 18, 255),
    )
    # Barrel shroud
    poly(
        d,
        [
            (cx + 130, cy - 22),
            (cx + 245, cy - 78),
            (cx + 252, cy - 62),
            (cx + 138, cy - 8),
        ],
        (88, 92, 100, 255),
        (32, 34, 38, 255),
    )
    # Stock stub
    poly(
        d,
        [
            (cx - 85, cy + 100),
            (cx - 35, cy + 125),
            (cx - 28, cy + 95),
            (cx - 72, cy + 72),
        ],
        (52, 54, 60, 255),
        (22, 24, 28, 255),
    )
    return im


def plasma() -> Image.Image:
    im = Image.new("RGBA", (W, W), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    cx, cy = 200, 300
    poly(
        d,
        [
            (cx - 40, cy + 120),
            (cx - 20, cy + 30),
            (cx + 160, cy - 40),
            (cx + 175, cy + 70),
        ],
        (32, 42, 52, 255),
        (12, 80, 95, 255),
    )
    # Glowing core strips
    for i in range(4):
        ox = i * 22
        poly(
            d,
            [
                (cx + 20 + ox, cy + 40),
                (cx + 35 + ox, cy + 25),
                (cx + 50 + ox, cy + 55),
                (cx + 35 + ox, cy + 70),
            ],
            (40, 220, 200, 200),
            (20, 140, 130, 255),
        )
    poly(
        d,
        [
            (cx + 140, cy - 25),
            (cx + 230, cy - 70),
            (cx + 242, cy - 52),
            (cx + 152, cy - 8),
        ],
        (60, 255, 230, 255),
        (30, 160, 150, 255),
    )
    return im


def muzzle_flash() -> Image.Image:
    n = 160
    im = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    cx = cy = n // 2
    for y in range(n):
        for x in range(n):
            dx, dy = x - cx, y - cy
            r = math.hypot(dx, dy)
            if r > 72:
                continue
            # star burst
            ang = math.atan2(dy, dx)
            spokes = abs(math.cos(ang * 5.0)) * 0.55 + 0.45
            t = max(0.0, 1.0 - r / 72.0) * spokes
            if t < 0.04:
                continue
            core = min(1.0, (48.0 / max(r, 1.0)) ** 1.2)
            rr = int(255 * (0.35 + 0.65 * core) * t)
            gg = int(240 * (0.2 + 0.8 * core) * t)
            bb = int(120 * (0.15 + 0.5 * core) * t)
            aa = int(255 * min(1.0, t * 1.35))
            im.putpixel((x, y), (rr, gg, bb, aa))
    return im


def shell_brass() -> Image.Image:
    im = Image.new("RGBA", (48, 24), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((2, 5, 44, 19), radius=4, fill=(198, 162, 72, 255), outline=(90, 72, 38, 255))
    d.line((10, 8, 10, 16), fill=(240, 210, 120, 255), width=1)
    return im


def main() -> None:
    OUT_GUN.mkdir(parents=True, exist_ok=True)
    OUT_VFX.mkdir(parents=True, exist_ok=True)
    pistol().save(OUT_GUN / "pistol.png")
    shotgun().save(OUT_GUN / "shotgun.png")
    smg().save(OUT_GUN / "smg.png")
    plasma().save(OUT_GUN / "plasma.png")
    muzzle_flash().save(OUT_VFX / "muzzle_flash.png")
    shell_brass().save(OUT_VFX / "shell_brass.png")
    print("Wrote pistol/shotgun/smg/plasma →", OUT_GUN)
    print("Wrote muzzle_flash/shell_brass →", OUT_VFX)


if __name__ == "__main__":
    main()
