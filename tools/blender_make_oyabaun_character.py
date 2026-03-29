"""
Oyabaun character GLB tooling.

**Canonical 3D NPCs** (`client/characters/oyabaun_player.glb`, `oyabaun_rival.glb`) are
**skin-modifier humanoids**. The generator script is now committed as:

  tools/blender_build_oyabaun_characters_3d.py

Use that script instead. See **docs/CHARACTER_PIPELINE_HANDOFF.md** for full details.

This entry point **does nothing by default** so a bad run cannot overwrite the good GLBs.

Optional legacy mode (billboard atlas quad — not the current 3D tint pipeline):

  OYABAUN_LEGACY_SPRITE=1 /path/to/Blender --background --python tools/blender_make_oyabaun_character.py

Env: OYABAUN_OUT, OYABAUN_SPRITE (see legacy_sprite_export docstring).
"""
from __future__ import annotations

import os
import sys


def _fail_default() -> None:
    msg = """oyabaun: character GLBs are built by tools/blender_build_oyabaun_characters_3d.py

  Use:  Blender --background --python tools/blender_build_oyabaun_characters_3d.py
  Docs: docs/CHARACTER_PIPELINE_HANDOFF.md

  To rebuild the old atlas *billboard* quad only (not recommended for current 3D NPCs):
  OYABAUN_LEGACY_SPRITE=1 Blender --background --python tools/blender_make_oyabaun_character.py
"""
    print(msg, file=sys.stderr)
    sys.exit(1)


def legacy_sprite_export() -> None:
    """8-direction card quad + optional atlas texture (pre-3D pipeline)."""
    import bpy
    import bmesh

    ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    OUT_DIR = os.path.join(ROOT, "client", "characters")
    OUT = os.environ.get("OYABAUN_OUT", os.path.join(OUT_DIR, "oyabaun_player.glb"))
    SPRITE = os.environ.get(
        "OYABAUN_SPRITE", os.path.join(ROOT, "client", "sprite1.png")
    )

    ATLAS_ROWS = 7
    CELL_W = 64
    CELL_H = 50
    HEIGHT = 1.68
    HALF_W = HEIGHT * (float(CELL_W) / float(CELL_H)) / 2.0
    X_PLANE = 0.02

    os.makedirs(OUT_DIR, exist_ok=True)

    bpy.ops.wm.read_factory_settings(use_empty=True)

    bm = bmesh.new()
    uv_layer = bm.loops.layers.uv.new()

    v_bl = bm.verts.new((X_PLANE, -HALF_W, 0.0))
    v_br = bm.verts.new((X_PLANE, HALF_W, 0.0))
    v_tr = bm.verts.new((X_PLANE, HALF_W, HEIGHT))
    v_tl = bm.verts.new((X_PLANE, -HALF_W, HEIGHT))
    face = bm.faces.new((v_bl, v_br, v_tr, v_tl))
    uvs = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
    for loop, uv in zip(face.loops, uvs):
        loop[uv_layer].uv = uv

    eps = 0.04
    v2_bl = bm.verts.new((X_PLANE - eps, -HALF_W, 0.0))
    v2_br = bm.verts.new((X_PLANE - eps, HALF_W, 0.0))
    v2_tr = bm.verts.new((X_PLANE - eps, HALF_W, HEIGHT))
    v2_tl = bm.verts.new((X_PLANE - eps, -HALF_W, HEIGHT))
    face2 = bm.faces.new((v2_br, v2_bl, v2_tl, v2_tr))
    for loop, uv in zip(face2.loops, uvs):
        loop[uv_layer].uv = uv

    mesh = bpy.data.meshes.new("OyabaunCharacter")
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new("OyabaunCharacter", mesh)
    bpy.context.collection.objects.link(obj)
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)

    mat = bpy.data.materials.new(name="OyabaunCharMat")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()
    out = nodes.new("ShaderNodeOutputMaterial")
    prin = nodes.new("ShaderNodeBsdfPrincipled")
    tex = nodes.new("ShaderNodeTexImage")
    if os.path.isfile(SPRITE):
        tex.image = bpy.data.images.load(SPRITE, check_existing=True)
    else:
        tex.image = None
    links.new(tex.outputs["Color"], prin.inputs["Base Color"])
    if tex.image:
        links.new(tex.outputs["Alpha"], prin.inputs["Alpha"])
    prin.inputs["Roughness"].default_value = 0.85
    mat.blend_method = "BLEND" if tex.image else "OPAQUE"
    links.new(prin.outputs["BSDF"], out.inputs["Surface"])
    mesh.materials.append(mat)

    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj

    bpy.ops.export_scene.gltf(
        filepath=OUT,
        export_format="GLB",
        export_materials="EXPORT",
        export_texcoords=True,
        export_normals=True,
        export_apply=True,
        export_yup=True,
        use_selection=True,
        export_animations=False,
    )

    print(f"oyabaun: legacy sprite quad wrote {OUT}", file=sys.stderr)


def main() -> None:
    if os.environ.get("OYABAUN_LEGACY_SPRITE") == "1":
        legacy_sprite_export()
        return
    _fail_default()


main()
