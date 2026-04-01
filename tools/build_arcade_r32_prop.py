#!/usr/bin/env python3
"""Build arcade parked R32 prop: derive 3 PNGs from source art, embed in one Y-up `.glb`.

Reads:
  client/level_textures/tokyo_props/r32_side.png
  client/level_textures/tokyo_props/r32_front.png
  client/level_textures/tokyo_props/r32_rear.png

Writes (committed so `include_bytes!` works without re-running):
  client/props/generated/arcade_r32_1_side.png   — side view, NEAREST scale
  client/props/generated/arcade_r32_2_endcaps.png — front | rear in one atlas (left / right UV halves)
  client/props/generated/arcade_r32_3_body.png  — mean paint color from all three (roof / wall / under)

Then:
  client/props/arcade_r32_prop.glb — box mesh, 3 materials, nearest samplers (matches game)

Re-run after editing source R32 PNGs; then `wasm-pack build --target web` in `client/`.
CLI: `python3 tools/oyabaunctl.py build-arcade-r32-prop`
"""
from __future__ import annotations

import json
import struct
import zlib
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC_SIDE = ROOT / "client/level_textures/tokyo_props/r32_side.png"
SRC_FRONT = ROOT / "client/level_textures/tokyo_props/r32_front.png"
SRC_REAR = ROOT / "client/level_textures/tokyo_props/r32_rear.png"

OUT_DIR = ROOT / "client/props/generated"
OUT_SIDE = OUT_DIR / "arcade_r32_1_side.png"
OUT_ENDCAPS = OUT_DIR / "arcade_r32_2_endcaps.png"
OUT_BODY = OUT_DIR / "arcade_r32_3_body.png"
OUT_GLB = ROOT / "client/props/arcade_r32_prop.glb"

# World AABB half-extents (matches arcade_level collision); mesh centered at origin Y-up.
HX, HY, HZ = 0.91, 0.62, 2.12

SIDE_TARGET_H = 128
ENDCAP_TARGET_H = 112
BODY_TILE = 8


def pad4(data: bytes) -> bytes:
    pad = (4 - len(data) % 4) % 4
    return data + b" " * pad


def png_bytes(im) -> bytes:
    buf = BytesIO()
    im.save(buf, format="PNG", compress_level=9)
    return buf.getvalue()


def mean_opaque_rgb(*images) -> tuple[int, int, int]:
    rs: list[int] = []
    gs: list[int] = []
    bs: list[int] = []
    for im in images:
        px = im.convert("RGBA").tobytes()
        for i in range(0, len(px), 4):
            r, g, b, a = px[i], px[i + 1], px[i + 2], px[i + 3]
            if a > 8:
                rs.append(r)
                gs.append(g)
                bs.append(b)
    if not rs:
        return (48, 52, 64)
    return (sum(rs) // len(rs), sum(gs) // len(gs), sum(bs) // len(bs))


def scale_to_height_nearest(im, target_h: int):
    from PIL import Image

    w, h = im.size
    if h == target_h:
        return im
    new_w = max(1, int(round(w * target_h / h)))
    return im.resize((new_w, target_h), Image.Resampling.NEAREST)


def build_derived_images() -> tuple[bytes, bytes, bytes]:
    from PIL import Image

    if not SRC_SIDE.is_file() or not SRC_FRONT.is_file() or not SRC_REAR.is_file():
        raise SystemExit(
            f"Missing R32 sources under client/level_textures/tokyo_props/ "
            f"(need r32_side.png, r32_front.png, r32_rear.png)"
        )

    side = Image.open(SRC_SIDE).convert("RGBA")
    front = Image.open(SRC_FRONT).convert("RGBA")
    rear = Image.open(SRC_REAR).convert("RGBA")

    side_b = scale_to_height_nearest(side, SIDE_TARGET_H)
    front_b = scale_to_height_nearest(front, ENDCAP_TARGET_H)
    rear_b = scale_to_height_nearest(rear, ENDCAP_TARGET_H)
    h = max(front_b.height, rear_b.height)
    w = front_b.width + rear_b.width
    endcaps = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    endcaps.paste(front_b, (0, 0))
    endcaps.paste(rear_b, (front_b.width, 0))

    mr, mg, mb = mean_opaque_rgb(side, front, rear)
    body = Image.new("RGBA", (BODY_TILE, BODY_TILE), (mr, mg, mb, 255))

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    side_b.save(OUT_SIDE, "PNG")
    endcaps.save(OUT_ENDCAPS, "PNG")
    body.save(OUT_BODY, "PNG")

    return png_bytes(side_b), png_bytes(endcaps), png_bytes(body)


def box_mesh_r32() -> tuple[bytes, list[dict], list[dict], list[dict]]:
    """24 verts (4 per face), 6 primitives × 6 indices, 3 materials."""
    hx, hy, hz = HX, HY, HZ
    xl, xr = -hx, hx
    yb, yt = -hy, hy
    zb, zf = -hz, hz

    # Per-face positions + UVs (glTF v = bottom origin).
    faces: list[tuple[list[list[float]], list[list[float]], int]] = [
        (
            [[xl, yb, zb], [xl, yb, zf], [xl, yt, zf], [xl, yt, zb]],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            0,
        ),
        (
            [[xr, yb, zf], [xr, yb, zb], [xr, yt, zb], [xr, yt, zf]],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            2,
        ),
        (
            [[xl, yb, zf], [xr, yb, zf], [xr, yt, zf], [xl, yt, zf]],
            [[0.0, 0.0], [0.5, 0.0], [0.5, 1.0], [0.0, 1.0]],
            1,
        ),
        (
            [[xr, yb, zb], [xl, yb, zb], [xl, yt, zb], [xr, yt, zb]],
            [[0.5, 0.0], [1.0, 0.0], [1.0, 1.0], [0.5, 1.0]],
            1,
        ),
        (
            [[xl, yt, zb], [xr, yt, zb], [xr, yt, zf], [xl, yt, zf]],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            2,
        ),
        (
            [[xl, yb, zf], [xr, yb, zf], [xr, yb, zb], [xl, yb, zb]],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            2,
        ),
    ]

    flat_p: list[float] = []
    flat_uv: list[float] = []
    idx_chunks: list[list[int]] = []
    base = 0
    for corners, uvs, _mat in faces:
        for c in corners:
            flat_p.extend(c)
        for uv in uvs:
            flat_uv.extend(uv)
        idx_chunks.append([base, base + 1, base + 2, base, base + 2, base + 3])
        base += 4

    blob = bytearray()
    blob.extend(struct.pack(f"<{len(flat_p)}f", *flat_p))
    o_uv = len(blob)
    blob.extend(struct.pack(f"<{len(flat_uv)}f", *flat_uv))
    o_idx = len(blob)
    all_idx: list[int] = []
    for ch in idx_chunks:
        all_idx.extend(ch)
    blob.extend(struct.pack(f"<{len(all_idx)}H", *all_idx))
    idx_byte_len = len(all_idx) * 2

    pmin = [-hx, -hy, -hz]
    pmax = [hx, hy, hz]

    accessors: list[dict] = [
        {
            "bufferView": 0,
            "componentType": 5126,
            "count": 24,
            "type": "VEC3",
            "max": pmax,
            "min": pmin,
        },
        {"bufferView": 1, "componentType": 5126, "count": 24, "type": "VEC2"},
    ]
    buffer_views: list[dict] = [
        {"buffer": 0, "byteOffset": 0, "byteLength": o_uv},
        {"buffer": 0, "byteOffset": o_uv, "byteLength": o_idx - o_uv},
        {"buffer": 0, "byteOffset": o_idx, "byteLength": idx_byte_len},
    ]

    primitives = []
    for i, (_, _, mat) in enumerate(faces):
        bo = i * 12
        accessors.append(
            {
                "bufferView": 2,
                "byteOffset": bo,
                "componentType": 5123,
                "count": 6,
                "type": "SCALAR",
            }
        )
        ai = 2 + i
        primitives.append(
            {
                "attributes": {"POSITION": 0, "TEXCOORD_0": 1},
                "indices": ai,
                "material": mat,
            }
        )

    return bytes(blob), accessors, buffer_views, primitives


def main() -> None:
    png_side, png_caps, png_body = build_derived_images()

    mesh_blob, accessors, buffer_views, primitives = box_mesh_r32()
    o_tex0 = len(mesh_blob)
    o_tex1 = o_tex0 + len(png_side)
    o_tex2 = o_tex1 + len(png_caps)
    total_mesh = o_tex2 + len(png_body)
    full = bytearray(mesh_blob)
    full += png_side
    full += png_caps
    full += png_body

    bv_tex0 = len(buffer_views)
    bv_tex1 = bv_tex0 + 1
    bv_tex2 = bv_tex0 + 2
    buffer_views.extend(
        [
            {"buffer": 0, "byteOffset": o_tex0, "byteLength": len(png_side)},
            {"buffer": 0, "byteOffset": o_tex1, "byteLength": len(png_caps)},
            {"buffer": 0, "byteOffset": o_tex2, "byteLength": len(png_body)},
        ]
    )

    gltf = {
        "asset": {"version": "2.0", "generator": "oyabaun build_arcade_r32_prop.py"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": "ArcadeR32_Prop"}],
        "meshes": [{"name": "R32Box", "primitives": primitives}],
        "materials": [
            {
                "name": "R32Side",
                "pbrMetallicRoughness": {
                    "baseColorTexture": {"index": 0},
                    "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                    "metallicFactor": 0.15,
                    "roughnessFactor": 0.85,
                },
            },
            {
                "name": "R32Endcaps",
                "pbrMetallicRoughness": {
                    "baseColorTexture": {"index": 1},
                    "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                    "metallicFactor": 0.12,
                    "roughnessFactor": 0.82,
                },
            },
            {
                "name": "R32BodyPaint",
                "pbrMetallicRoughness": {
                    "baseColorTexture": {"index": 2},
                    "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                    "metallicFactor": 0.22,
                    "roughnessFactor": 0.88,
                },
            },
        ],
        "textures": [
            {"source": 0, "sampler": 0},
            {"source": 1, "sampler": 0},
            {"source": 2, "sampler": 0},
        ],
        "samplers": [{"magFilter": 9728, "minFilter": 9728}],
        "images": [
            {"mimeType": "image/png", "bufferView": bv_tex0},
            {"mimeType": "image/png", "bufferView": bv_tex1},
            {"mimeType": "image/png", "bufferView": bv_tex2},
        ],
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{"byteLength": total_mesh}],
    }

    bin_chunk = pad4(bytes(full))
    json_bytes = pad4(json.dumps(gltf, separators=(",", ":")).encode("utf-8"))
    total = 12 + 8 + len(json_bytes) + 8 + len(bin_chunk)
    out = bytearray()
    out += struct.pack("<4sII", b"glTF", 2, total)
    out += struct.pack("<I", len(json_bytes))
    out += b"JSON"
    out += json_bytes
    out += struct.pack("<I", len(bin_chunk))
    out += b"BIN\x00"
    out += bin_chunk
    OUT_GLB.parent.mkdir(parents=True, exist_ok=True)
    OUT_GLB.write_bytes(out)
    print(
        f"Wrote {OUT_SIDE.name}, {OUT_ENDCAPS.name}, {OUT_BODY.name} → {OUT_GLB} ({len(out)} bytes)"
    )


if __name__ == "__main__":
    main()
