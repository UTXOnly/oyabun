#!/usr/bin/env python3
"""
Build an Oyabaun game atlas (8 dirs × idle + walk + optional extra animations) from a PixelLab ZIP.

Column order matches client billboard shader: S, SW, W, NW, N, NE, E, SE.

Row layout (must match client `lib.rs` / `render.rs`): row 0 idle; rows 1–6 walk;
each `--extra` animation adds 6 rows (e.g. `--extra running-6-frames --extra lead-jab`
→ rows 7–12 run, 13–18 shoot). Missing frames pad with that direction's idle.

Missing walk directions (partial PixelLab exports) are filled with that direction's idle rotation.
"""
from __future__ import annotations

import argparse
import json
import struct
import sys
import zipfile
from io import BytesIO
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("Requires Pillow: pip install Pillow", file=sys.stderr)
    sys.exit(1)

# Must match render.rs 8-dir quantization (col 0 = south, …)
DIR_ORDER = [
    "south",
    "south-west",
    "west",
    "north-west",
    "north",
    "north-east",
    "east",
    "south-east",
]


def load_png(z: zipfile.ZipFile, name: str) -> Image.Image:
    return Image.open(BytesIO(z.read(name))).convert("RGBA")


def resize_cell(im: Image.Image, cell: int) -> Image.Image:
    if im.size == (cell, cell):
        return im
    return im.resize((cell, cell), Image.Resampling.NEAREST)


def load_six_frame_anim(
    z: zipfile.ZipFile,
    frames_meta: dict,
    anim_key: str,
    cell: int,
    idle_by_dir: dict[str, Image.Image],
) -> dict[str, list[Image.Image]]:
    anim_dirs = (frames_meta.get(anim_key) or {}) if frames_meta else {}
    out: dict[str, list[Image.Image]] = {}
    for d in DIR_ORDER:
        idle = idle_by_dir[d]
        paths = anim_dirs.get(d) if isinstance(anim_dirs.get(d), list) else []
        loaded: list[Image.Image] = []
        for rel in paths:
            if rel in z.namelist():
                loaded.append(resize_cell(load_png(z, rel), cell))
        while len(loaded) < 6:
            loaded.append(idle.copy())
        out[d] = loaded[:6]
    return out


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("zip_path", type=Path, help="PixelLab character .zip")
    p.add_argument(
        "-o",
        "--out",
        type=Path,
        required=True,
        help="Output .png or .rgba",
    )
    p.add_argument(
        "--animation",
        default="walking",
        help="Primary walk animation key in metadata.json (default: walking)",
    )
    p.add_argument(
        "--extra",
        action="append",
        default=[],
        metavar="KEY",
        help="Additional animation key (repeatable), e.g. running-6-frames, lead-jab",
    )
    args = p.parse_args()
    zpath = args.zip_path.resolve()
    out = args.out.resolve()
    if not zpath.is_file():
        sys.exit(f"Missing zip: {zpath}")

    with zipfile.ZipFile(zpath) as z:
        meta = json.loads(z.read("metadata.json").decode("utf-8"))
        char = meta.get("character") or {}
        w0 = int(char.get("size", {}).get("width", 64))
        h0 = int(char.get("size", {}).get("height", 64))
        cell = max(w0, h0, 16)

        # Probe cell from first rotation
        idle_by_dir: dict[str, Image.Image] = {}
        walk_frames: dict[str, list[Image.Image]] = {d: [] for d in DIR_ORDER}

        frames_meta = (meta.get("frames") or {}).get("animations") or {}
        anim_key = args.animation
        if anim_key not in frames_meta:
            for k in frames_meta:
                anim_key = k
                break

        for ex in args.extra:
            if ex not in frames_meta:
                avail = ", ".join(sorted(frames_meta.keys())) or "(none)"
                sys.exit(f"Animation {ex!r} not in metadata; available: {avail}")

        for d in DIR_ORDER:
            rot_path = f"rotations/{d}.png"
            if rot_path not in z.namelist():
                sys.exit(f"ZIP missing {rot_path}")
            idle = resize_cell(load_png(z, rot_path), cell)
            idle_by_dir[d] = idle

        walk_frames = load_six_frame_anim(z, frames_meta, anim_key, cell, idle_by_dir)

        extra_block: list[dict[str, list[Image.Image]]] = []
        for ex in args.extra:
            extra_block.append(load_six_frame_anim(z, frames_meta, ex, cell, idle_by_dir))

        nrows = 1 + 6 + 6 * len(extra_block)
        atlas = Image.new("RGBA", (cell * 8, cell * nrows), (0, 0, 0, 0))

        for col, d in enumerate(DIR_ORDER):
            idle = idle_by_dir[d]
            atlas.paste(idle, (col * cell, 0))
            for fi in range(6):
                atlas.paste(walk_frames[d][fi], (col * cell, (1 + fi) * cell))
            row_base = 7
            for blk in extra_block:
                for fi in range(6):
                    atlas.paste(blk[d][fi], (col * cell, (row_base + fi) * cell))
                row_base += 6

    out.parent.mkdir(parents=True, exist_ok=True)
    if out.suffix.lower() == ".rgba":
        w, h = atlas.size
        with open(out, "wb") as f:
            f.write(struct.pack("<II", w, h))
            f.write(atlas.tobytes())
        print(f"Wrote {w}×{h} RGBA -> {out}")
    else:
        atlas.save(out)
        print(f"Wrote {atlas.size[0]}×{atlas.size[1]} PNG -> {out}")


if __name__ == "__main__":
    main()
