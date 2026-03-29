#!/usr/bin/env python3
"""
Build an Oyabaun game atlas (8 dirs × idle + walk + optional extra animations) from a PixelLab ZIP.

Column order matches client billboard shader: S, SW, W, NW, N, NE, E, SE.

Row layout (must match client `lib.rs` / `render.rs`): row 0 idle; rows 1–6 walk;
each `--extra` adds `--extra-frames` rows (default 6; use 9 for PixelLab 9-frame clips).
Example: walk + one 9-frame extra → 16 rows (shoot rows 7–15).

Missing walk directions (partial PixelLab exports) are filled with that direction's idle rotation.
`--extra` animations: if PixelLab only exported a few facings (common for custom anims), missing
columns are filled by cloning the first real strip (prefers `south`) so billboards shoot from all angles.
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


def load_n_frame_anim(
    z: zipfile.ZipFile,
    frames_meta: dict,
    anim_key: str,
    cell: int,
    idle_by_dir: dict[str, Image.Image],
    n: int,
    *,
    clone_missing_dirs_from_donor: bool = False,
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
        while len(loaded) < n:
            loaded.append(idle.copy())
        out[d] = loaded[:n]

    if clone_missing_dirs_from_donor:
        donor: list[Image.Image] | None = None
        for pref in (
            "south",
            "south-east",
            "south-west",
            "east",
            "west",
            "north",
            "north-east",
            "north-west",
        ):
            paths = anim_dirs.get(pref) if isinstance(anim_dirs.get(pref), list) else []
            real = [rel for rel in paths if rel in z.namelist()]
            if not real:
                continue
            seq: list[Image.Image] = []
            for rel in real[:n]:
                seq.append(resize_cell(load_png(z, rel), cell))
            idle = idle_by_dir[pref]
            while len(seq) < n:
                seq.append(idle.copy())
            donor = seq[:n]
            break
        if donor:
            for d in DIR_ORDER:
                paths = anim_dirs.get(d) if isinstance(anim_dirs.get(d), list) else []
                if not any(rel in z.namelist() for rel in paths):
                    out[d] = [im.copy() for im in donor]
    return out


def load_six_frame_anim(
    z: zipfile.ZipFile,
    frames_meta: dict,
    anim_key: str,
    cell: int,
    idle_by_dir: dict[str, Image.Image],
) -> dict[str, list[Image.Image]]:
    return load_n_frame_anim(
        z, frames_meta, anim_key, cell, idle_by_dir, 6, clone_missing_dirs_from_donor=False
    )


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
    p.add_argument(
        "--extra-frames",
        type=int,
        default=6,
        metavar="N",
        help="Row count per --extra block (default 6; use 9 for full PixelLab exports)",
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

        nf = max(1, min(args.extra_frames, 16))
        extra_block: list[dict[str, list[Image.Image]]] = []
        for ex in args.extra:
            extra_block.append(
                load_n_frame_anim(
                    z,
                    frames_meta,
                    ex,
                    cell,
                    idle_by_dir,
                    nf,
                    clone_missing_dirs_from_donor=True,
                )
            )

        n_extra_rows = nf * len(extra_block)
        nrows = 1 + 6 + n_extra_rows
        atlas = Image.new("RGBA", (cell * 8, cell * nrows), (0, 0, 0, 0))

        for col, d in enumerate(DIR_ORDER):
            idle = idle_by_dir[d]
            atlas.paste(idle, (col * cell, 0))
            for fi in range(6):
                atlas.paste(walk_frames[d][fi], (col * cell, (1 + fi) * cell))
            row_base = 7
            for blk in extra_block:
                for fi in range(nf):
                    atlas.paste(blk[d][fi], (col * cell, (row_base + fi) * cell))
                row_base += nf

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
