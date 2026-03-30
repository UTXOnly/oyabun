#!/usr/bin/env python3
"""Build 8×1 prop atlas (S,SE,E,NE,N,NW,W,SW) from existing R32 PNGs for billboard columns."""
from __future__ import annotations

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("Requires Pillow", file=sys.stderr)
    sys.exit(1)

ROOT = Path(__file__).resolve().parents[1]
SIDE = ROOT / "client/level_textures/tokyo_props/r32_side.png"
FRONT = ROOT / "client/level_textures/tokyo_props/r32_front.png"
REAR = ROOT / "client/level_textures/tokyo_props/r32_rear.png"
OUT = ROOT / "client/characters/prop_r32_atlas.png"

# Column order matches push_char_sprite_quad: S, SE, E, NE, N, NW, W, SW
ORDER = [
    ("rear", REAR),
    ("side", SIDE),
    ("side", SIDE),
    ("side", SIDE),
    ("front", FRONT),
    ("side_mir", SIDE),
    ("side_mir", SIDE),
    ("side_mir", SIDE),
]


def fit_cell(img: Image.Image, cw: int, ch: int, mirror: bool) -> Image.Image:
    im = img.transpose(Image.FLIP_LEFT_RIGHT) if mirror else img
    im = im.convert("RGBA")
    w, h = im.size
    scale = min(cw / w, ch / h)
    nw = max(1, int(w * scale))
    nh = max(1, int(h * scale))
    im = im.resize((nw, nh), Image.Resampling.NEAREST)
    cell = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    ox = (cw - nw) // 2
    oy = (ch - nh) // 2
    cell.paste(im, (ox, oy), im)
    return cell


def main() -> None:
    imgs = [Image.open(p) for _, p in ORDER]
    target_h = max(im.height for im in imgs)
    target_h = max(target_h, 96)
    cell_w = max(int(im.width * target_h / max(im.height, 1)) for im in imgs)
    cell_w = max(cell_w, 64)
    cw = cell_w
    ch = target_h
    row: list[Image.Image] = []
    for (kind, _), im_src in zip(ORDER, imgs):
        mirror = kind == "side_mir"
        row.append(fit_cell(im_src, cw, ch, mirror))
    atlas = Image.new("RGBA", (cw * 8, ch), (0, 0, 0, 0))
    for i, cell in enumerate(row):
        atlas.paste(cell, (i * cw, 0))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    atlas.save(OUT, "PNG")
    print(f"Wrote {atlas.size[0]}x{atlas.size[1]} -> {OUT}")


if __name__ == "__main__":
    main()
