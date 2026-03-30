#!/usr/bin/env python3
"""Write a minimal Y-up glTF binary: gray box (real mesh, no textures). Stdlib only."""
from __future__ import annotations

import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "client/props/arcade_parked_car_blockout.glb"

# Half extents (meters), Y-up; Z = length along alley when translated in arcade.
WX, H, DZ = 0.84, 1.18, 2.025


def box_24() -> tuple[list[list[float]], list[list[float]], list[int]]:
    """24 positions, 24 uvs, 36 u16 indices — CCW outward faces."""
    verts: list[list[float]] = []
    uvs: list[list[float]] = []
    # +X
    verts += [[WX, 0, -DZ], [WX, 0, DZ], [WX, H, DZ], [WX, H, -DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]
    # -X
    verts += [[-WX, 0, DZ], [-WX, 0, -DZ], [-WX, H, -DZ], [-WX, H, DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]
    # +Y top
    verts += [[-WX, H, -DZ], [WX, H, -DZ], [WX, H, DZ], [-WX, H, DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]
    # -Y bottom
    verts += [[-WX, 0, DZ], [WX, 0, DZ], [WX, 0, -DZ], [-WX, 0, -DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]
    # +Z rear
    verts += [[WX, 0, DZ], [-WX, 0, DZ], [-WX, H, DZ], [WX, H, DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]
    # -Z front
    verts += [[-WX, 0, -DZ], [WX, 0, -DZ], [WX, H, -DZ], [-WX, H, -DZ]]
    uvs += [[0, 0], [1, 0], [1, 1], [0, 1]]

    idx: list[int] = []
    for f in range(6):
        b = f * 4
        idx += [b, b + 1, b + 2, b, b + 2, b + 3]

    return verts, uvs, idx


def pad4(data: bytes) -> bytes:
    pad = (4 - len(data) % 4) % 4
    return data + b" " * pad


def main() -> None:
    positions, texcoords, indices = box_24()
    flat_p: list[float] = []
    for v in positions:
        flat_p += v
    flat_uv: list[float] = []
    for uv in texcoords:
        flat_uv += uv

    buf = struct.pack(f"<{len(flat_p)}f", *flat_p)
    buf += struct.pack(f"<{len(flat_uv)}f", *flat_uv)
    buf += struct.pack(f"<{len(indices)}H", *indices)

    pmin = [-WX, 0.0, -DZ]
    pmax = [WX, H, DZ]

    gltf = {
        "asset": {"version": "2.0", "generator": "oyabaun write_arcade_parked_car_blockout_glb.py"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": "ParkedCar_Blockout"}],
        "meshes": [
            {
                "primitives": [
                    {
                        "attributes": {"POSITION": 0, "TEXCOORD_0": 1},
                        "indices": 2,
                        "material": 0,
                    }
                ],
                "name": "CarBox",
            }
        ],
        "materials": [
            {
                "name": "BodyGray",
                "pbrMetallicRoughness": {
                    "baseColorFactor": [0.50, 0.50, 0.54, 1.0],
                    "metallicFactor": 0.15,
                    "roughnessFactor": 0.88,
                },
            }
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
            {
                "bufferView": 1,
                "componentType": 5126,
                "count": 24,
                "type": "VEC2",
            },
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
                "byteLength": len(indices) * 2,
            },
        ],
        "buffers": [{"byteLength": len(buf)}],
    }

    json_bytes = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_bytes = pad4(json_bytes)
    bin_chunk = pad4(buf)

    json_chunk_len = len(json_bytes)
    bin_chunk_len = len(bin_chunk)
    total = 12 + 8 + json_chunk_len + 8 + bin_chunk_len

    out = bytearray()
    out += struct.pack("<4sII", b"glTF", 2, total)
    out += struct.pack("<I", json_chunk_len)
    out += b"JSON"
    out += json_bytes
    out += struct.pack("<I", bin_chunk_len)
    out += b"BIN\x00"
    out += bin_chunk

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(out)
    print(f"Wrote {OUT} ({len(out)} bytes)")


if __name__ == "__main__":
    main()
