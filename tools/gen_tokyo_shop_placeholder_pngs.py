#!/usr/bin/env python3
"""
Write chunky pixel-art style 320×384 PNGs for Tokyo alley shop recess panels.

These are **placeholders** — replace with PixelLab map-object exports per
`client/level_textures/tokyo_shops/EXPORT.txt` for final art.

Usage (from repo root):

  python3 tools/gen_tokyo_shop_placeholder_pngs.py

Requires: pip install pillow
"""
from __future__ import annotations

import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:
    print("gen_tokyo_shop_placeholder_pngs: need Pillow (pip install pillow)", file=sys.stderr)
    sys.exit(1)

ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "client" / "level_textures" / "tokyo_shops"
W, H = 320, 384
# Logical pixel grid then scale up (arcade chunky read)
GW, GH = 40, 48
SX = W // GW
SY = H // GH


def _upscale(im: Image.Image) -> Image.Image:
    return im.resize((W, H), Image.Resampling.NEAREST)


def gen_ramen() -> Image.Image:
    low = Image.new("RGBA", (GW, GH), (0, 0, 0, 0))
    ld = ImageDraw.Draw(low)
    # Wall
    for y in range(GH):
        for x in range(GW):
            ld.point((x, y), (42, 38, 52, 255))
    # Doorway dark
    ld.rectangle([8, 12, 31, 45], fill=(18, 14, 22, 255))
    # Warm window glow
    ld.rectangle([10, 18, 29, 32], fill=(255, 180, 90, 255))
    # Noren (split curtain)
    ld.rectangle([11, 10, 17, 22], fill=(180, 40, 45, 255))
    ld.rectangle([22, 10, 28, 22], fill=(180, 40, 45, 255))
    # Sign band
    ld.rectangle([6, 4, 33, 8], fill=(240, 220, 140, 255))
    return _upscale(low)


def gen_pachinko() -> Image.Image:
    low = Image.new("RGBA", (GW, GH), (0, 0, 0, 0))
    ld = ImageDraw.Draw(low)
    for y in range(GH):
        for x in range(GW):
            ld.point((x, y), (35, 30, 55, 255))
    ld.rectangle([6, 8, 33, 44], fill=(255, 255, 240, 255))
    ld.rectangle([8, 10, 31, 20], fill=(255, 60, 180, 255))
    ld.rectangle([8, 24, 31, 38], fill=(0, 255, 220, 255))
    ld.rectangle([10, 40, 29, 43], fill=(255, 220, 0, 255))
    return _upscale(low)


def gen_konbini() -> Image.Image:
    low = Image.new("RGBA", (GW, GH), (0, 0, 0, 0))
    ld = ImageDraw.Draw(low)
    for y in range(GH):
        for x in range(GW):
            ld.point((x, y), (48, 52, 62, 255))
    ld.rectangle([7, 10, 32, 42], fill=(200, 230, 255, 230))
    ld.rectangle([9, 14, 15, 38], fill=(255, 100, 80, 255))
    ld.rectangle([24, 14, 30, 38], fill=(80, 200, 120, 255))
    ld.rectangle([8, 4, 31, 9], fill=(255, 255, 255, 255))
    return _upscale(low)


def gen_shuttered() -> Image.Image:
    low = Image.new("RGBA", (GW, GH), (0, 0, 0, 0))
    ld = ImageDraw.Draw(low)
    for y in range(GH):
        for x in range(GW):
            ld.point((x, y), (30, 28, 34, 255))
    for i in range(12):
        y0 = 10 + i * 3
        ld.rectangle([8, y0, 31, y0 + 2], fill=(70, 72, 78, 255))
    ld.rectangle([6, 8, 33, 44], outline=(20, 20, 24, 255), width=1)
    ld.rectangle([10, 3, 28, 7], fill=(255, 80, 60, 200))
    return _upscale(low)


def gen_izakaya() -> Image.Image:
    low = Image.new("RGBA", (GW, GH), (0, 0, 0, 0))
    ld = ImageDraw.Draw(low)
    for y in range(GH):
        for x in range(GW):
            ld.point((x, y), (50, 35, 30, 255))
    ld.rectangle([9, 14, 30, 42], fill=(35, 22, 18, 255))
    ld.rectangle([11, 18, 28, 36], fill=(255, 140, 60, 180))
    ld.ellipse([7, 8, 13, 14], fill=(255, 90, 70, 255))
    ld.ellipse([26, 8, 32, 14], fill=(255, 90, 70, 255))
    ld.rectangle([12, 4, 27, 8], fill=(200, 50, 40, 255))
    return _upscale(low)


def gen_arcade() -> Image.Image:
    low = Image.new("RGBA", (GW, GH), (0, 0, 0, 0))
    ld = ImageDraw.Draw(low)
    for y in range(GH):
        for x in range(GW):
            ld.point((x, y), (25, 20, 45, 255))
    ld.rectangle([6, 8, 33, 44], fill=(40, 20, 80, 255))
    for i in range(4):
        x0 = 9 + i * 6
        ld.rectangle([x0, 16, x0 + 4, 28], fill=(0, 255, 200, 255))
        ld.rectangle([x0, 30, x0 + 4, 38], fill=(255, 0, 200, 255))
    ld.rectangle([8, 4, 31, 10], fill=(255, 255, 100, 255))
    return _upscale(low)


def gen_snackbar() -> Image.Image:
    low = Image.new("RGBA", (GW, GH), (0, 0, 0, 0))
    ld = ImageDraw.Draw(low)
    for y in range(GH):
        for x in range(GW):
            ld.point((x, y), (32, 30, 40, 255))
    ld.rectangle([8, 12, 31, 44], fill=(15, 12, 20, 255))
    ld.rectangle([10, 16, 29, 34], fill=(60, 50, 40, 255))
    ld.rectangle([11, 5, 28, 11], fill=(220, 190, 100, 255))
    ld.rectangle([12, 6, 26, 9], fill=(40, 30, 20, 255))
    return _upscale(low)


def gen_tattoo() -> Image.Image:
    low = Image.new("RGBA", (GW, GH), (0, 0, 0, 0))
    ld = ImageDraw.Draw(low)
    for y in range(GH):
        for x in range(GW):
            ld.point((x, y), (22, 22, 30, 255))
    ld.rectangle([7, 10, 32, 44], fill=(12, 12, 18, 255))
    ld.rectangle([10, 14, 29, 38], fill=(80, 40, 120, 200))
    ld.rectangle([12, 18, 27, 32], fill=(40, 200, 160, 180))
    ld.rectangle([9, 4, 30, 9], fill=(200, 40, 80, 255))
    return _upscale(low)


GENERATORS = (
    ("shop_ramen.png", gen_ramen),
    ("shop_pachinko.png", gen_pachinko),
    ("shop_konbini.png", gen_konbini),
    ("shop_shuttered.png", gen_shuttered),
    ("shop_izakaya.png", gen_izakaya),
    ("shop_arcade.png", gen_arcade),
    ("shop_snackbar.png", gen_snackbar),
    ("shop_tattoo.png", gen_tattoo),
)


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for fname, gen in GENERATORS:
        path = OUT_DIR / fname
        im = gen()
        im.save(path, "PNG")
        print(f"wrote {path}")
    print(
        "gen_tokyo_shop_placeholder_pngs: replace with PixelLab art when ready "
        "(see client/level_textures/tokyo_shops/EXPORT.txt)",
        flush=True,
    )


if __name__ == "__main__":
    main()
