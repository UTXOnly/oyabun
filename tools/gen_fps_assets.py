#!/usr/bin/env python3
"""Optional tiny VFX PNGs only. Do not use this for weapon HUD art — use Blender/real sprites in client/fpsweapons/."""
from __future__ import annotations

import math
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("pip install Pillow", file=sys.stderr)
    sys.exit(1)

ROOT = Path(__file__).resolve().parent.parent
OUT_VFX = ROOT / "client" / "vfx"


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
    from PIL import ImageDraw

    im = Image.new("RGBA", (48, 24), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((2, 5, 44, 19), radius=4, fill=(198, 162, 72, 255), outline=(90, 72, 38, 255))
    d.line((10, 8, 10, 16), fill=(240, 210, 120, 255), width=1)
    return im


def main() -> None:
    OUT_VFX.mkdir(parents=True, exist_ok=True)
    muzzle_flash().save(OUT_VFX / "muzzle_flash.png")
    shell_brass().save(OUT_VFX / "shell_brass.png")
    print("Wrote muzzle_flash.png, shell_brass.png →", OUT_VFX)


if __name__ == "__main__":
    main()
