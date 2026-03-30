#!/usr/bin/env python3
"""Y-up glTF binary: box mesh, r32_side on street face (-X), dark gray PNG on other faces."""
from __future__ import annotations

import json
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "client/props/arcade_parked_car_blockout.glb"
R32_SIDE = ROOT / "client/level_textures/tokyo_props/r32_side.png"

WX, H, DZ = 0.84, 1.18, 2.025


def one_pixel_png_rgb(r: int, g: int, b: int) -> bytes:
    ihdr = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0)

    def chunk(tag: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    raw = bytes([0, r & 255, g & 255, b & 255])
    comp = zlib.compress(raw, 9)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", comp) + chunk(b"IEND", b"")


def box_24() -> tuple[list[float], list[float], list[int], list[int]]:
    verts: list[list[float]] = []
    uvs: list[list[float]] = []
    verts += [[WX, 0, -DZ], [WX, 0, DZ], [WX, H, DZ], [WX, H, -DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]
    verts += [[-WX, 0, DZ], [-WX, 0, -DZ], [-WX, H, -DZ], [-WX, H, DZ]]
    uvs += [[1, 1], [0, 1], [0, 0], [1, 0]]
    verts += [[-WX, H, -DZ], [WX, H, -DZ], [WX, H, DZ], [-WX, H, DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]
    verts += [[-WX, 0, DZ], [WX, 0, DZ], [WX, 0, -DZ], [-WX, 0, -DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]
    verts += [[WX, 0, DZ], [-WX, 0, DZ], [-WX, H, DZ], [WX, H, DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]
    verts += [[-WX, 0, -DZ], [WX, 0, -DZ], [WX, H, -DZ], [-WX, H, -DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]

    flat_p: list[float] = []
    for v in verts:
        flat_p += v
    flat_uv: list[float] = []
    for uv in uvs:
        flat_uv += uv

    idx_side = [4, 5, 6, 4, 6, 7]
    idx_rest = [
        0, 1, 2, 0, 2, 3,
        8, 9, 10, 8, 10, 11,
        12, 13, 14, 12, 14, 15,
        16, 17, 18, 16, 18, 19,
        20, 21, 22, 20, 22, 23,
    ]
    return flat_p, flat_uv, idx_side, idx_rest


def pad4(data: bytes) -> bytes:
    pad = (4 - len(data) % 4) % 4
    return data + b" " * pad


def main() -> None:
    flat_p, flat_uv, idx_side, idx_rest = box_24()
    gray_png = one_pixel_png_rgb(52, 54, 62)
    r32_bytes = R32_SIDE.read_bytes() if R32_SIDE.is_file() else b""
    use_r32 = len(r32_bytes) > 24

    blob = bytearray()
    blob += struct.pack(f"<{len(flat_p)}f", *flat_p)
    o_uv = len(blob)
    blob += struct.pack(f"<{len(flat_uv)}f", *flat_uv)
    o_is = len(blob)
    blob += struct.pack(f"<{len(idx_side)}H", *idx_side)
    o_ir = len(blob)
    blob += struct.pack(f"<{len(idx_rest)}H", *idx_rest)

    if use_r32:
        o_r32 = len(blob)
        blob += r32_bytes
        o_gray = len(blob)
        blob += gray_png
        total_buf = len(blob)
        pmin = [-WX, 0.0, -DZ]
        pmax = [WX, H, DZ]
        gltf = {
            "asset": {"version": "2.0", "generator": "oyabaun parked car glb"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0, "name": "ParkedCar_Blockout"}],
            "meshes": [
                {
                    "name": "CarBox",
                    "primitives": [
                        {
                            "attributes": {"POSITION": 0, "TEXCOORD_0": 1},
                            "indices": 2,
                            "material": 0,
                        },
                        {
                            "attributes": {"POSITION": 0, "TEXCOORD_0": 1},
                            "indices": 3,
                            "material": 1,
                        },
                    ],
                }
            ],
            "materials": [
                {
                    "name": "SideR32",
                    "pbrMetallicRoughness": {
                        "baseColorTexture": {"index": 0},
                        "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                        "metallicFactor": 0.12,
                        "roughnessFactor": 0.82,
                    },
                },
                {
                    "name": "BodyGray",
                    "pbrMetallicRoughness": {
                        "baseColorTexture": {"index": 1},
                        "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                        "metallicFactor": 0.18,
                        "roughnessFactor": 0.9,
                    },
                },
            ],
            "textures": [{"source": 0, "sampler": 0}, {"source": 1, "sampler": 0}],
            "samplers": [{"magFilter": 9728, "minFilter": 9728}],
            "images": [
                {"mimeType": "image/png", "bufferView": 4},
                {"mimeType": "image/png", "bufferView": 5},
            ],
            "accessors": [
                {
                    "bufferView": 0,
                    "componentType": 5126,
                    "count": 24,
                    "type": "VEC3",
                    "max": pmax,
                    "min": pmin,
                },
                {"bufferView": 1, "componentType": 5126, "count": 24, "type": "VEC2"},
                {"bufferView": 2, "componentType": 5123, "count": 6, "type": "SCALAR"},
                {"bufferView": 3, "componentType": 5123, "count": 30, "type": "SCALAR"},
            ],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": o_uv},
                {"buffer": 0, "byteOffset": o_uv, "byteLength": o_is - o_uv},
                {"buffer": 0, "byteOffset": o_is, "byteLength": o_ir - o_is},
                {"buffer": 0, "byteOffset": o_ir, "byteLength": o_r32 - o_ir},
                {"buffer": 0, "byteOffset": o_r32, "byteLength": o_gray - o_r32},
                {"buffer": 0, "byteOffset": o_gray, "byteLength": total_buf - o_gray},
            ],
            "buffers": [{"byteLength": total_buf}],
        }
    else:
        idx_all = idx_side + idx_rest
        blob = bytearray()
        blob += struct.pack(f"<{len(flat_p)}f", *flat_p)
        blob += struct.pack(f"<{len(flat_uv)}f", *flat_uv)
        blob += struct.pack(f"<{len(idx_all)}H", *idx_all)
        blob += gray_png
        total_buf = len(blob)
        pmin = [-WX, 0.0, -DZ]
        pmax = [WX, H, DZ]
        gltf = {
            "asset": {"version": "2.0", "generator": "oyabaun parked car glb (no r32 png)"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0, "name": "ParkedCar_Blockout"}],
            "meshes": [
                {
                    "name": "CarBox",
                    "primitives": [
                        {
                            "attributes": {"POSITION": 0, "TEXCOORD_0": 1},
                            "indices": 2,
                            "material": 0,
                        }
                    ],
                }
            ],
            "materials": [
                {
                    "name": "BodyGray",
                    "pbrMetallicRoughness": {
                        "baseColorTexture": {"index": 0},
                        "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                        "metallicFactor": 0.18,
                        "roughnessFactor": 0.9,
                    },
                }
            ],
            "textures": [{"source": 0, "sampler": 0}],
            "samplers": [{"magFilter": 9728, "minFilter": 9728}],
            "images": [{"mimeType": "image/png", "bufferView": 3}],
            "accessors": [
                {
                    "bufferView": 0,
                    "componentType": 5126,
                    "count": 24,
                    "type": "VEC3",
                    "max": pmax,
                    "min": pmin,
                },
                {"bufferView": 1, "componentType": 5126, "count": 24, "type": "VEC2"},
                {
                    "bufferView": 2,
                    "componentType": 5123,
                    "count": 36,
                    "type": "SCALAR",
                },
            ],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": len(flat_p) * 4},
                {
                    "buffer": 0,
                    "byteOffset": len(flat_p) * 4,
                    "byteLength": len(flat_uv) * 4,
                },
                {
                    "buffer": 0,
                    "byteOffset": len(flat_p) * 4 + len(flat_uv) * 4,
                    "byteLength": 36 * 2,
                },
                {
                    "buffer": 0,
                    "byteOffset": len(flat_p) * 4 + len(flat_uv) * 4 + 72,
                    "byteLength": len(gray_png),
                },
            ],
            "buffers": [{"byteLength": total_buf}],
        }

    bin_chunk = pad4(bytes(blob))
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
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(out)
    print(f"Wrote {OUT} ({len(out)} bytes, r32={'yes' if use_r32 else 'no'})")


if __name__ == "__main__":
    main()
