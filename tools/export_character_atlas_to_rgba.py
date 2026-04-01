#!/usr/bin/env python3
"""Embed a PNG character atlas as raw RGBA for include_bytes! (8-byte LE width/height header)."""
from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("Requires Pillow: pip install Pillow", file=sys.stderr)
    sys.exit(1)


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("png", type=Path, help="Atlas PNG (RGBA)")
    p.add_argument(
        "-o",
        "--out",
        type=Path,
        help="Output .rgba path (default: same basename next to png)",
    )
    args = p.parse_args()
    png = args.png.resolve()
    out = args.out.resolve() if args.out else png.with_suffix(".rgba")
    img = Image.open(png).convert("RGBA")
    w, h = img.size
    out.parent.mkdir(parents=True, exist_ok=True)
    with open(out, "wb") as f:
        f.write(struct.pack("<II", w, h))
        f.write(img.tobytes())
    print(f"Wrote {w}x{h} -> {out}")


if __name__ == "__main__":
    main()
