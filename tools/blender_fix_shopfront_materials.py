"""
Fix materials on existing phase1 shop modules without deleting geometry.

  - *_recess  -> OYA_Recess (dark stucco in enhance)
  - *_awning  -> OYA_Awning / OYA_AwningB / OYA_AwningC (fabric, not brick/trim)

Run after pulling texture fixes:

  python3 tools/oyabaunctl.py fix-tokyo-shopfront-materials

Then: export-world --force-all (or --enhance with OYABAUN_REPACK_ALBEDOS=1).
"""
from __future__ import annotations

import re
import sys

import bpy

AWNING_CYCLE = ("OYA_Awning", "OYA_AwningB", "OYA_AwningC")


def _link_mat(ob: bpy.types.Object, mat_name: str) -> None:
    mat = bpy.data.materials.get(mat_name)
    if not mat:
        mat = bpy.data.materials.new(mat_name)
        mat.use_nodes = True
    ob.data.materials.clear()
    ob.data.materials.append(mat)


def main() -> None:
    n = 0
    for ob in bpy.data.objects:
        if ob.type != "MESH":
            continue
        if ob.name.endswith("_recess"):
            _link_mat(ob, "OYA_Recess")
            n += 1
            continue
        m = re.match(r"^ShopFront_[LR]_(\d+)_awning$", ob.name)
        if m:
            idx = int(m.group(1))
            _link_mat(ob, AWNING_CYCLE[idx % len(AWNING_CYCLE)])
            n += 1

    fp = bpy.data.filepath
    if fp and n:
        bpy.ops.wm.save_mainfile()
        print(f"oyabaun: fix-shopfront-mats: updated {n} meshes, saved {fp}")
    elif not n:
        print("oyabaun: fix-shopfront-mats: no ShopFront_*_recess/_awning found", file=sys.stderr)
    else:
        print("oyabaun: fix-shopfront-mats: unsaved blend", file=sys.stderr)


if __name__ == "__main__":
    main()
