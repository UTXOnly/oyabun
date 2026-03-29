"""
Oyabaun 3D Character Generator — Faceted arcade body + detail meshes

Bodies use Blender's Skin modifier **without** Subdivision — flat-shaded,
edge-split prisms (Virtua / Model 2–style silhouettes), not smoothed blobs.

Detail props (glasses, weapons, lapels) are separate box meshes, also flat.

Usage:
  # Build both characters
  Blender --background --python tools/blender_build_oyabaun_characters_3d.py

  # Build one character
  OYABAUN_VARIANT=boss Blender --background --python tools/blender_build_oyabaun_characters_3d.py
  OYABAUN_VARIANT=rival Blender --background --python tools/blender_build_oyabaun_characters_3d.py

  # Or via Blender MCP execute_blender_code (paste the relevant section)

Output:
  client/characters/oyabaun_player.glb  (boss)
  client/characters/oyabaun_rival.glb   (rival)

After regenerating GLBs, rebuild WASM:
  cd client && wasm-pack build --target web --no-typescript

Optional:
  OYABAUN_CHAR_DECIMATE=0.85  (fraction of faces kept; default 1.0 = off for crisp facets)
  OYABAUN_CHAR_LEGACY_SMOOTH=1  (subsurf+smooth+heavy decimate — old “organic” look)

Body materials use embedded 32–48px "arcade" albedos (ordered dither, nearest sampling) so the
GLB carries pixel-era texture; client samples with nearest filtering.

Blender version: 5.1
glTF export: export_yup=True, export_materials='EXPORT', no animations

Coordinate conventions:
  - Blender: Z-up, character front faces -Y, feet at Z=0
  - glTF: Y-up (after export_yup=True), front becomes +Z
  - Game: character_model() adds PI to yaw for Blender→game facing
"""

from __future__ import annotations
import bpy
import bmesh
import math
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
OUT_DIR = os.path.join(ROOT, "client", "characters")


# ============================================================
# Utilities
# ============================================================

def make_material(name, color, metallic=0.0, roughness=0.8,
                  emission=(0, 0, 0), emission_strength=0.0):
    """Create or reuse a Principled BSDF material (no image textures)."""
    existing = bpy.data.materials.get(name)
    if existing:
        return existing
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()
    out = nodes.new('ShaderNodeOutputMaterial')
    bsdf = nodes.new('ShaderNodeBsdfPrincipled')
    bsdf.inputs['Base Color'].default_value = (*color, 1.0)
    bsdf.inputs['Metallic'].default_value = metallic
    bsdf.inputs['Roughness'].default_value = roughness
    if emission_strength > 0:
        bsdf.inputs['Emission Color'].default_value = (*emission, 1.0)
        bsdf.inputs['Emission Strength'].default_value = emission_strength
    links.new(bsdf.outputs['BSDF'], out.inputs['Surface'])
    return mat


# --- Low-res arcade albedos (packed into GLB; no external files) ---

_BAYER4 = (
    0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5,
)


def _bayer01(x: int, y: int) -> float:
    return _BAYER4[(y & 3) * 4 + (x & 3)] / 16.0


def _flatten_rgba(w: int, h: int, rgba_fn) -> list[float]:
    flat: list[float] = []
    for y in range(h):
        for x in range(w):
            r, g, b, a = rgba_fn(x, y)
            flat.extend((r, g, b, a))
    return flat


def make_arcade_image(name: str, w: int, h: int, rgba_fn) -> bpy.types.Image:
    """Create and pack a small RGBA image for glTF embed (Principled + Image Texture)."""
    existing = bpy.data.images.get(name)
    if existing:
        bpy.data.images.remove(existing, do_unlink=True)
    img = bpy.data.images.new(name, width=w, height=h, alpha=True)
    buf = _flatten_rgba(w, h, rgba_fn)
    img.pixels.foreach_set(buf)
    img.pack()
    img.update()
    return img


def make_textured_material(
    name: str,
    img: bpy.types.Image,
    metallic: float = 0.0,
    roughness: float = 0.78,
    emission: tuple[float, float, float] = (0, 0, 0),
    emission_strength: float = 0.0,
) -> bpy.types.Material:
    """Principled BSDF with nearest-neighbor base color texture (early-90s style)."""
    existing = bpy.data.materials.get(name)
    if existing:
        return existing
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()
    out = nodes.new('ShaderNodeOutputMaterial')
    bsdf = nodes.new('ShaderNodeBsdfPrincipled')
    tex = nodes.new('ShaderNodeTexImage')
    tex.image = img
    tex.interpolation = 'Closest'
    links.new(tex.outputs['Color'], bsdf.inputs['Base Color'])
    bsdf.inputs['Metallic'].default_value = metallic
    bsdf.inputs['Roughness'].default_value = roughness
    if emission_strength > 0:
        bsdf.inputs['Emission Color'].default_value = (*emission, 1.0)
        bsdf.inputs['Emission Strength'].default_value = emission_strength
    links.new(bsdf.outputs['BSDF'], out.inputs['Surface'])
    return mat


def _hash01(x: int, y: int) -> float:
    u = (x * 374761393 + y * 668265263) & 0xFFFFFFFF
    return u / 4294967296.0


def _clamp01(v: float) -> float:
    return min(1.0, max(0.0, v))


def _rgba_boss_suit(x: int, y: int) -> tuple[float, float, float, float]:
    # Near-flat navy silhouette (in-game ref): slow vertical read only, micro noise.
    nx = x / 95.0
    ny = y / 95.0
    fold = 0.78 + 0.22 * (0.5 + 0.5 * math.sin(nx * math.pi * 1.1)) * (0.55 + 0.45 * ny)
    base = (0.028, 0.032, 0.088)
    hi = (0.048, 0.052, 0.12)
    r = base[0] + (hi[0] - base[0]) * fold
    g = base[1] + (hi[1] - base[1]) * fold
    b = base[2] + (hi[2] - base[2]) * fold
    h = _hash01(x, y)
    r += (h - 0.5) * 0.008
    g += (h - 0.5) * 0.006
    b += (h - 0.5) * 0.010
    return (_clamp01(r), _clamp01(g), _clamp01(b), 1.0)


def _rgba_boss_skin(x: int, y: int) -> tuple[float, float, float, float]:
    nx = x / 47.0
    ny = y / 47.0
    smooth = 0.40 + 0.42 * ny + 0.10 * math.sin(nx * math.pi * 2.2)
    u = smooth * 0.88 + _bayer01(x, y) * 0.12
    c0 = (0.48, 0.34, 0.24)
    c1 = (0.76, 0.54, 0.38)
    c2 = (0.62, 0.42, 0.30)
    if u > 0.58:
        return (*c1, 1.0)
    if u > 0.38:
        return (*c2, 1.0)
    return (*c0, 1.0)


def _rgba_boss_shirt(x: int, y: int) -> tuple[float, float, float, float]:
    # Clean white bib + sparse teal “icon” chips (HUD ref on chest).
    r = 0.93
    g = 0.91
    b = 0.86
    h = _hash01(x, y)
    r += (h - 0.5) * 0.006
    g += (h - 0.5) * 0.006
    b += (h - 0.5) * 0.005
    if (x // 3 + y // 3) % 9 == 0 and h > 0.55:
        return (0.15, 0.72, 0.78, 1.0)
    return (_clamp01(r), _clamp01(g), _clamp01(b), 1.0)


def _rgba_boss_hair(x: int, y: int) -> tuple[float, float, float, float]:
    u = _hash01(x ^ 31, y ^ 17) * 0.55 + _bayer01(x, y) * 0.45
    if u > 0.5:
        return (0.032, 0.032, 0.042, 1.0)
    return (0.006, 0.006, 0.010, 1.0)


def _rgba_boss_shoe(x: int, y: int) -> tuple[float, float, float, float]:
    u = _hash01(x, y * 3) * 0.5 + _bayer01(x, y) * 0.5
    c0 = (0.018, 0.018, 0.025)
    c1 = (0.09, 0.085, 0.10)
    return (*(c1 if u > 0.5 else c0), 1.0)


def _rgba_boss_tie(x: int, y: int) -> tuple[float, float, float, float]:
    stripe = ((x + y * 2) // 5) % 2
    t = _bayer01(x, y)
    if stripe:
        return (0.62, 0.04, 0.06, 1.0) if t > 0.45 else (0.48, 0.02, 0.05, 1.0)
    return (0.38, 0.02, 0.08, 1.0) if t > 0.48 else (0.52, 0.03, 0.05, 1.0)


def _rgba_rival_suit(x: int, y: int) -> tuple[float, float, float, float]:
    # Bronze / copper monotone (slender enemy in ref), vertical tone drift.
    nx = x / 95.0
    ny = y / 95.0
    fold = 0.45 + 0.55 * (0.5 + 0.5 * math.sin(nx * math.pi * 1.4)) * (0.4 + 0.6 * ny)
    c0 = (0.28, 0.14, 0.06)
    c1 = (0.52, 0.30, 0.12)
    r = c0[0] + (c1[0] - c0[0]) * fold
    g = c0[1] + (c1[1] - c0[1]) * fold
    b = c0[2] + (c1[2] - c0[2]) * fold
    h = _hash01(x + 11, y)
    r += (h - 0.5) * 0.018
    g += (h - 0.5) * 0.012
    b += (h - 0.5) * 0.008
    return (_clamp01(r), _clamp01(g), _clamp01(b), 1.0)


def _rgba_rival_skin(x: int, y: int) -> tuple[float, float, float, float]:
    nx = x / 47.0
    ny = y / 47.0
    u = 0.38 + 0.40 * ny + 0.12 * math.sin(nx * math.pi * 2.0)
    u = u * 0.85 + _bayer01(x, y) * 0.15
    c0 = (0.42, 0.30, 0.22)
    c1 = (0.68, 0.46, 0.32)
    return (*c1, 1.0) if u > 0.52 else (*c0, 1.0)


def _rgba_rival_hair(x: int, y: int) -> tuple[float, float, float, float]:
    # Dark crown read (bronze enemy silhouette in ref), not blonde.
    u = _hash01(x, y) * 0.55 + _bayer01(x, y) * 0.45
    c0 = (0.05, 0.035, 0.032)
    c1 = (0.12, 0.075, 0.055)
    return (*c1, 1.0) if u > 0.5 else (*c0, 1.0)


def _rgba_rival_shoe(x: int, y: int) -> tuple[float, float, float, float]:
    u = _hash01(x * 2, y) * 0.5 + _bayer01(x, y) * 0.5
    c0 = (0.62, 0.60, 0.56)
    c1 = (0.82, 0.78, 0.74)
    return (*(c1 if u > 0.5 else c0), 1.0)


def _rgba_rival_shirt(x: int, y: int) -> tuple[float, float, float, float]:
    ny = y / 31.0
    v = 0.5 + 0.5 * math.sin(ny * math.pi * 5.0)
    c0 = (0.05, 0.05, 0.07)
    c1 = (0.14, 0.12, 0.18)
    h = _hash01(x, y)
    if v + (h - 0.5) * 0.08 > 0.52:
        return (*c1, 1.0)
    return (*c0, 1.0)


def make_box(cx, cy, cz, sx, sy, sz):
    """Axis-aligned box centered at (cx,cy,cz) with half-extents (sx,sy,sz)."""
    v = [
        (cx - sx, cy - sy, cz - sz), (cx + sx, cy - sy, cz - sz),
        (cx + sx, cy + sy, cz - sz), (cx - sx, cy + sy, cz - sz),
        (cx - sx, cy - sy, cz + sz), (cx + sx, cy - sy, cz + sz),
        (cx + sx, cy + sy, cz + sz), (cx - sx, cy + sy, cz + sz),
    ]
    f = [(0, 1, 2, 3), (4, 7, 6, 5), (0, 4, 5, 1),
         (2, 6, 7, 3), (0, 3, 7, 4), (1, 5, 6, 2)]
    return v, f


def make_tapered_box(cx, cy, cz, sx_bot, sy_bot, sx_top, sy_top, hz):
    """Box that tapers from bottom to top."""
    v = [
        (cx - sx_bot, cy - sy_bot, cz), (cx + sx_bot, cy - sy_bot, cz),
        (cx + sx_bot, cy + sy_bot, cz), (cx - sx_bot, cy + sy_bot, cz),
        (cx - sx_top, cy - sy_top, cz + hz), (cx + sx_top, cy - sy_top, cz + hz),
        (cx + sx_top, cy + sy_top, cz + hz), (cx - sx_top, cy + sy_top, cz + hz),
    ]
    f = [(0, 1, 2, 3), (4, 7, 6, 5), (0, 4, 5, 1),
         (2, 6, 7, 3), (0, 3, 7, 4), (1, 5, 6, 2)]
    return v, f


def add_detail(name, verts, faces, material):
    """Add a separate detail mesh object with a single material."""
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    obj.data.materials.append(material)
    bpy.context.collection.objects.link(obj)
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.shade_flat()
    return obj


def build_skin_body(name, joints, edges_def, radii):
    """Build body from skin skeleton: faceted (default) or legacy smooth blob."""
    mesh = bpy.data.meshes.new(f"{name}Skeleton")
    obj = bpy.data.objects.new(f"{name}Body", mesh)
    bpy.context.collection.objects.link(obj)
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)

    bm = bmesh.new()
    vert_list = list(joints.keys())
    verts = {}
    for jname in vert_list:
        verts[jname] = bm.verts.new(joints[jname])
    for a, b in edges_def:
        bm.edges.new((verts[a], verts[b]))
    bm.to_mesh(mesh)
    bm.free()

    obj.modifiers.new("Skin", 'SKIN')
    for i, jname in enumerate(vert_list):
        sv = mesh.skin_vertices[""].data[i]
        sv.radius = radii.get(jname, (0.04, 0.04))
    mesh.skin_vertices[""].data[vert_list.index('pelvis')].use_root = True

    legacy = os.environ.get("OYABAUN_CHAR_LEGACY_SMOOTH", "").strip() in (
        "1", "true", "yes", "on",
    )

    if legacy:
        sub = obj.modifiers.new("Subsurf", 'SUBSURF')
        sub.levels = 1
        sub.render_levels = 1
        bpy.ops.object.modifier_apply(modifier="Skin")
        bpy.ops.object.modifier_apply(modifier="Subsurf")
        bpy.ops.object.shade_smooth()
        dec = obj.modifiers.new("Decimate", 'DECIMATE')
        try:
            dec.ratio = float(os.environ.get("OYABAUN_CHAR_DECIMATE", "0.46"))
        except ValueError:
            dec.ratio = 0.46
        dec.ratio = max(0.18, min(0.52, dec.ratio))
        bpy.ops.object.modifier_apply(modifier="Decimate")
        bpy.ops.object.shade_smooth()
    else:
        bpy.ops.object.modifier_apply(modifier="Skin")
        try:
            dr = float(os.environ.get("OYABAUN_CHAR_DECIMATE", "1.0"))
        except ValueError:
            dr = 1.0
        dr = max(0.35, min(1.0, dr))
        if dr < 0.995:
            dec = obj.modifiers.new("Decimate", 'DECIMATE')
            dec.ratio = dr
            bpy.ops.object.modifier_apply(modifier="Decimate")
        es = obj.modifiers.new("EdgeSplit", 'EDGE_SPLIT')
        es.split_angle = math.radians(32.0)
        bpy.ops.object.modifier_apply(modifier="EdgeSplit")
        bpy.ops.object.shade_flat()

    vcount = len(obj.data.vertices)
    fcount = len(obj.data.polygons)
    print(f"  {name} body: {vcount} verts, {fcount} faces")
    return obj


def assign_materials_by_position(obj, materials, thresholds):
    """Assign material indices to faces based on face center Z/X position.

    materials: list of materials in slot order
    thresholds: dict with zone boundaries and assignment logic
    """
    for mat in materials:
        obj.data.materials.append(mat)

    bm = bmesh.new()
    bm.from_mesh(obj.data)
    for face in bm.faces:
        c = face.calc_center_median()
        x, y, z = abs(c.x), c.y, c.z
        face.material_index = thresholds(x, y, z)
    bm.to_mesh(obj.data)
    bm.free()


def join_and_export(prefix, out_path):
    """Select all objects starting with prefix, join, and export as GLB."""
    bpy.ops.object.select_all(action='DESELECT')
    meshes = [o for o in bpy.data.objects
              if o.type == 'MESH' and o.name.startswith(prefix)]
    if not meshes:
        print(f"  ERROR: no meshes found with prefix '{prefix}'")
        return
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.join()

    joined = bpy.context.active_object
    joined.name = f"{prefix}Character"
    joined.data.name = f"{prefix}Character"

    vcount = len(joined.data.vertices)
    fcount = len(joined.data.polygons)
    mcount = len(joined.data.materials)
    print(f"  Joined: {vcount} verts, {fcount} faces, {mcount} materials")

    # Ensure feet at Z=0 (Y=0 in glTF after export_yup)
    min_z = min(v.co.z for v in joined.data.vertices)
    if abs(min_z) > 0.001:
        for v in joined.data.vertices:
            v.co.z -= min_z
        print(f"  Shifted Z by {-min_z:.4f} to ground feet")

    bpy.ops.object.select_all(action='DESELECT')
    joined.select_set(True)
    bpy.context.view_layer.objects.active = joined
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(66.0), island_margin=0.02)
    bpy.ops.object.mode_set(mode="OBJECT")

    bpy.ops.export_scene.gltf(
        filepath=out_path,
        export_format='GLB',
        export_materials='EXPORT',
        export_texcoords=True,
        export_normals=True,
        export_apply=True,
        export_yup=True,
        use_selection=True,
        export_animations=False,
    )

    fsize = os.path.getsize(out_path)
    print(f"  Exported: {out_path} ({fsize} bytes)")
    return joined


# ============================================================
# Shared skeleton edge definitions (same topology for both)
# ============================================================

SKELETON_EDGES = [
    # Spine
    ('pelvis', 'waist'), ('waist', 'chest'), ('chest', 'upper_chest'),
    ('upper_chest', 'neck'), ('neck', 'head'), ('head', 'head_top'),
    # Left arm
    ('upper_chest', 'l_shoulder'), ('l_shoulder', 'l_upper_arm'),
    ('l_upper_arm', 'l_elbow'), ('l_elbow', 'l_forearm'),
    ('l_forearm', 'l_wrist'), ('l_wrist', 'l_hand'),
    # Right arm
    ('upper_chest', 'r_shoulder'), ('r_shoulder', 'r_upper_arm'),
    ('r_upper_arm', 'r_elbow'), ('r_elbow', 'r_forearm'),
    ('r_forearm', 'r_wrist'), ('r_wrist', 'r_hand'),
    # Left leg
    ('pelvis', 'l_hip'), ('l_hip', 'l_knee'),
    ('l_knee', 'l_ankle'), ('l_ankle', 'l_foot'), ('l_foot', 'l_toe'),
    # Right leg
    ('pelvis', 'r_hip'), ('r_hip', 'r_knee'),
    ('r_knee', 'r_ankle'), ('r_ankle', 'r_foot'), ('r_foot', 'r_toe'),
]


# ============================================================
# BOSS CHARACTER
# ============================================================

def build_boss():
    """Build the Boss (oyabaun_player) — dark suit, long limbs, visor read, pistol."""
    print("Building Boss character...")

    # ── Joint positions: ref silhouette — long thin limbs, shallow boxy head, stiff stance ──
    joints = {
        'pelvis':      (0, 0, 0.91),
        'waist':       (0, 0, 1.02),
        'chest':       (0, 0, 1.17),
        'upper_chest': (0, 0, 1.30),
        'neck':        (0, 0, 1.38),
        'head':        (0, 0, 1.46),
        'head_top':    (0, 0, 1.62),
        'l_shoulder':  (-0.27, 0, 1.32),
        'l_upper_arm': (-0.41, 0.01, 1.26),
        'l_elbow':     (-0.52, 0.02, 1.05),
        'l_forearm':   (-0.56, 0.01, 0.85),
        'l_wrist':     (-0.57, -0.03, 0.66),
        'l_hand':      (-0.57, -0.07, 0.56),
        'r_shoulder':  (0.27, 0, 1.32),
        'r_upper_arm': (0.41, 0.01, 1.26),
        'r_elbow':     (0.49, -0.05, 1.08),
        'r_forearm':   (0.51, -0.12, 0.94),
        'r_wrist':     (0.47, -0.20, 0.82),
        'r_hand':      (0.43, -0.27, 0.76),
        'l_hip':       (-0.095, 0, 0.89),
        'l_knee':      (-0.10, 0.02, 0.48),
        'l_ankle':     (-0.10, 0, 0.08),
        'l_foot':      (-0.10, -0.08, 0.02),
        'l_toe':       (-0.10, -0.16, 0.02),
        'r_hip':       (0.095, 0, 0.89),
        'r_knee':      (0.10, 0.02, 0.48),
        'r_ankle':     (0.10, 0, 0.08),
        'r_foot':      (0.10, -0.08, 0.02),
        'r_toe':       (0.10, -0.16, 0.02),
    }

    radii = {
        'pelvis':      (0.118, 0.086),
        'waist':       (0.108, 0.082),
        'chest':       (0.132, 0.095),
        'upper_chest': (0.152, 0.090),
        'neck':        (0.048, 0.048),
        'head':        (0.10, 0.078),
        'head_top':    (0.088, 0.058),
        'l_shoulder':  (0.042, 0.042),
        'l_upper_arm': (0.042, 0.042),
        'l_elbow':     (0.036, 0.036),
        'l_forearm':   (0.032, 0.032),
        'l_wrist':     (0.028, 0.026),
        'l_hand':      (0.028, 0.032),
        'r_shoulder':  (0.042, 0.042),
        'r_upper_arm': (0.042, 0.042),
        'r_elbow':     (0.036, 0.036),
        'r_forearm':   (0.032, 0.032),
        'r_wrist':     (0.028, 0.026),
        'r_hand':      (0.028, 0.032),
        'l_hip':       (0.076, 0.074),
        'l_knee':      (0.054, 0.056),
        'l_ankle':     (0.040, 0.040),
        'l_foot':      (0.040, 0.078),
        'l_toe':       (0.030, 0.040),
        'r_hip':       (0.076, 0.074),
        'r_knee':      (0.054, 0.056),
        'r_ankle':     (0.040, 0.040),
        'r_foot':      (0.040, 0.078),
        'r_toe':       (0.030, 0.040),
    }

    obj = build_skin_body("Boss", joints, SKELETON_EDGES, radii)

    # ── Materials (slot order: 0=suit, 1=skin, 2=hair, 3=shoe, 4=shirt) ──
    im_suit = make_arcade_image("ArcadeBoss_Suit", 96, 96, _rgba_boss_suit)
    im_skin = make_arcade_image("ArcadeBoss_Skin", 48, 48, _rgba_boss_skin)
    im_shirt = make_arcade_image("ArcadeBoss_Shirt", 32, 32, _rgba_boss_shirt)
    im_hair = make_arcade_image("ArcadeBoss_Hair", 32, 32, _rgba_boss_hair)
    im_shoe = make_arcade_image("ArcadeBoss_Shoe", 32, 32, _rgba_boss_shoe)
    mat_suit = make_textured_material("Boss_Suit", im_suit, roughness=0.65)
    mat_skin = make_textured_material("Boss_Skin", im_skin, roughness=0.85)
    mat_hair = make_textured_material("Boss_Hair", im_hair, roughness=0.35)
    mat_shoe = make_textured_material("Boss_Shoe", im_shoe, metallic=0.3, roughness=0.25)
    mat_shirt = make_textured_material("Boss_Shirt", im_shirt, roughness=0.55)

    def boss_zone(x, y, z):
        if z < 0.10:
            return 3
        elif z < 0.88:
            return 0
        elif z < 1.36:
            if x > 0.44 and z < 1.08:
                return 1
            elif x < 0.085 and z > 1.20 and y < -0.04:
                return 4
            else:
                return 0
        elif z < 1.42:
            return 1
        elif z < 1.58:
            return 1
        else:
            return 2

    assign_materials_by_position(
        obj, [mat_suit, mat_skin, mat_hair, mat_shoe, mat_shirt], boss_zone)

    # ── Detail materials ──
    mat_glasses = make_material("Boss_Glasses", (0.01, 0.01, 0.01),
                                metallic=0.8, roughness=0.1)
    mat_frames = make_material("Boss_Frames", (0.6, 0.5, 0.2),
                               metallic=0.9, roughness=0.15)
    mat_gun = make_material("Boss_Gun", (0.06, 0.06, 0.06),
                            metallic=0.7, roughness=0.25)
    mat_gun_grip = make_material("Boss_GunGrip", (0.35, 0.28, 0.15),
                                 metallic=0.3, roughness=0.6)
    mat_neon = make_material("Boss_Neon", (0.0, 0.8, 1.0),
                             emission=(0.0, 0.8, 1.0), emission_strength=3.0)
    im_tie = make_arcade_image("ArcadeBoss_Tie", 32, 32, _rgba_boss_tie)
    mat_tie = make_textured_material("Boss_Tie", im_tie, roughness=0.45)

    # ── Visor / block glasses (ref: dark head + glowing read) ──
    v, f = make_box(-0.05, -0.11, 1.52, 0.035, 0.008, 0.018)
    add_detail("Boss_LensL", v, f, mat_glasses)
    v, f = make_box(0.05, -0.11, 1.52, 0.035, 0.008, 0.018)
    add_detail("Boss_LensR", v, f, mat_glasses)
    v, f = make_box(0, -0.115, 1.52, 0.015, 0.005, 0.008)
    add_detail("Boss_Bridge", v, f, mat_frames)
    v, f = make_box(-0.085, -0.05, 1.52, 0.005, 0.065, 0.005)
    add_detail("Boss_TempleL", v, f, mat_frames)
    v, f = make_box(0.085, -0.05, 1.52, 0.005, 0.065, 0.005)
    add_detail("Boss_TempleR", v, f, mat_frames)

    tv = [
        (-0.02, -0.115, 1.32), (0.02, -0.115, 1.32),
        (0.025, -0.12, 1.17), (-0.025, -0.12, 1.17),
        (-0.018, -0.11, 1.02), (0.018, -0.11, 1.02),
        (0.015, -0.105, 0.90), (-0.015, -0.105, 0.90),
    ]
    tf = [(0, 1, 2, 3), (3, 2, 5, 4), (4, 5, 6, 7)]
    add_detail("Boss_Tie", tv, tf, mat_tie)

    rx, ry, rz = 0.43, -0.31, 0.76
    v, f = make_box(rx, ry, rz, 0.015, 0.08, 0.022)
    add_detail("Boss_GunSlide", v, f, mat_gun)
    v, f = make_box(rx, ry - 0.06, rz + 0.008, 0.008, 0.04, 0.008)
    add_detail("Boss_GunBarrel", v, f, mat_gun)
    v, f = make_box(rx, ry + 0.02, rz - 0.035, 0.013, 0.018, 0.028)
    add_detail("Boss_GunGripMesh", v, f, mat_gun_grip)
    v, f = make_box(rx, ry - 0.005, rz - 0.015, 0.004, 0.018, 0.004)
    add_detail("Boss_GunTrig", v, f, mat_gun)

    v, f = make_box(-0.06, -0.12, 1.22, 0.012, 0.004, 0.08)
    add_detail("Boss_NeonLapelL", v, f, mat_neon)
    v, f = make_box(0.06, -0.12, 1.22, 0.012, 0.004, 0.08)
    add_detail("Boss_NeonLapelR", v, f, mat_neon)
    v, f = make_box(-0.10, -0.12, 1.24, 0.018, 0.004, 0.012)
    add_detail("Boss_PocketNeon", v, f, mat_neon)
    v, f = make_box(0, -0.10, 0.88, 0.025, 0.005, 0.015)
    add_detail("Boss_BeltNeon", v, f, mat_neon)

    v, f = make_box(-0.04, -0.11, 1.38, 0.02, 0.006, 0.015)
    add_detail("Boss_CollarL", v, f, mat_suit)
    v, f = make_box(0.04, -0.11, 1.38, 0.02, 0.006, 0.015)
    add_detail("Boss_CollarR", v, f, mat_suit)

    v, f = make_tapered_box(-0.26, 0, 1.35, 0.038, 0.038, 0.028, 0.028, 0.02)
    add_detail("Boss_ShoulderL", v, f, mat_suit)
    v, f = make_tapered_box(0.26, 0, 1.35, 0.038, 0.038, 0.028, 0.028, 0.02)
    add_detail("Boss_ShoulderR", v, f, mat_suit)

    v, f = make_box(0, 0.07, 1.64, 0.10, 0.032, 0.026)
    add_detail("Boss_HairBack", v, f, mat_hair)
    v, f = make_tapered_box(-0.05, 0.05, 1.63, 0.05, 0.03, 0.03, 0.012, 0.042)
    add_detail("Boss_HairSideL", v, f, mat_hair)
    v, f = make_tapered_box(0.05, 0.05, 1.63, 0.05, 0.03, 0.03, 0.012, 0.042)
    add_detail("Boss_HairSideR", v, f, mat_hair)

    v, f = make_box(-0.095, -0.108, 1.12, 0.028, 0.004, 0.10)
    add_detail("Boss_LapelFlapL", v, f, mat_suit)
    v, f = make_box(0.095, -0.108, 1.12, 0.028, 0.004, 0.10)
    add_detail("Boss_LapelFlapR", v, f, mat_suit)

    v, f = make_box(-0.14, -0.10, 0.98, 0.04, 0.006, 0.05)
    add_detail("Boss_Pocket", v, f, mat_suit)
    v, f = make_box(-0.56, -0.05, 0.62, 0.022, 0.006, 0.035)
    add_detail("Boss_CuffL", v, f, mat_suit)

    v, f = make_box(-0.10, -0.11, 0.012, 0.052, 0.10, 0.008)
    add_detail("Boss_SoleL", v, f, mat_shoe)
    v, f = make_box(0.10, -0.11, 0.012, 0.052, 0.10, 0.008)
    add_detail("Boss_SoleR", v, f, mat_shoe)

    v, f = make_box(rx, ry + 0.045, rz - 0.018, 0.011, 0.028, 0.038)
    add_detail("Boss_GunMag", v, f, mat_gun)
    v, f = make_box(rx, ry - 0.03, rz + 0.006, 0.016, 0.022, 0.006)
    add_detail("Boss_GunSerration", v, f, mat_gun)

    v, f = make_box(0.40, -0.26, 0.74, 0.014, 0.01, 0.008)
    add_detail("Boss_Signet", v, f, mat_frames)

    # ── Join and export ──
    out_path = os.path.join(OUT_DIR, "oyabaun_player.glb")
    join_and_export("Boss", out_path)
    print("Boss complete.\n")


# ============================================================
# RIVAL CHARACTER
# ============================================================

def build_rival():
    """Build the Rival (oyabaun_rival) — tall bronze arcade silhouette, crown, katana."""
    print("Building Rival character...")

    joints = {
        'pelvis':      (0, 0, 0.94),
        'waist':       (0, 0, 1.06),
        'chest':       (0, 0, 1.24),
        'upper_chest': (0, 0, 1.40),
        'neck':        (0, 0, 1.50),
        'head':        (0, 0, 1.60),
        'head_top':    (0, 0, 1.86),
        'l_shoulder':  (-0.17, 0, 1.42),
        'l_upper_arm': (-0.24, -0.06, 1.32),
        'l_elbow':     (-0.30, -0.16, 1.18),
        'l_forearm':   (-0.28, -0.26, 1.12),
        'l_wrist':     (-0.26, -0.32, 1.08),
        'l_hand':      (-0.24, -0.36, 1.05),
        'r_shoulder':  (0.17, 0, 1.42),
        'r_upper_arm': (0.24, 0, 1.32),
        'r_elbow':     (0.34, 0, 1.14),
        'r_forearm':   (0.38, 0, 0.98),
        'r_wrist':     (0.38, -0.02, 0.86),
        'r_hand':      (0.38, -0.04, 0.78),
        'l_hip':       (-0.075, 0, 0.92),
        'l_knee':      (-0.08, 0.02, 0.50),
        'l_ankle':     (-0.08, 0, 0.09),
        'l_foot':      (-0.08, -0.08, 0.02),
        'l_toe':       (-0.08, -0.15, 0.02),
        'r_hip':       (0.075, 0, 0.92),
        'r_knee':      (0.08, 0.02, 0.50),
        'r_ankle':     (0.08, 0, 0.09),
        'r_foot':      (0.08, -0.08, 0.02),
        'r_toe':       (0.08, -0.15, 0.02),
    }

    radii = {
        'pelvis':      (0.10, 0.078),
        'waist':       (0.092, 0.074),
        'chest':       (0.118, 0.084),
        'upper_chest': (0.132, 0.084),
        'neck':        (0.044, 0.044),
        'head':        (0.078, 0.072),
        'head_top':    (0.062, 0.056),
        'l_shoulder':  (0.048, 0.046),
        'l_upper_arm': (0.044, 0.044),
        'l_elbow':     (0.036, 0.036),
        'l_forearm':   (0.032, 0.032),
        'l_wrist':     (0.028, 0.026),
        'l_hand':      (0.028, 0.032),
        'r_shoulder':  (0.048, 0.046),
        'r_upper_arm': (0.044, 0.044),
        'r_elbow':     (0.036, 0.036),
        'r_forearm':   (0.032, 0.032),
        'r_wrist':     (0.028, 0.026),
        'r_hand':      (0.028, 0.032),
        'l_hip':       (0.064, 0.062),
        'l_knee':      (0.046, 0.048),
        'l_ankle':     (0.032, 0.032),
        'l_foot':      (0.032, 0.065),
        'l_toe':       (0.026, 0.032),
        'r_hip':       (0.064, 0.062),
        'r_knee':      (0.046, 0.048),
        'r_ankle':     (0.032, 0.032),
        'r_foot':      (0.032, 0.065),
        'r_toe':       (0.026, 0.032),
    }

    obj = build_skin_body("Rival", joints, SKELETON_EDGES, radii)

    # ── Materials (slot order: 0=suit, 1=skin, 2=hair, 3=shoe, 4=shirt) ──
    im_rs = make_arcade_image("ArcadeRival_Suit", 96, 96, _rgba_rival_suit)
    im_rk = make_arcade_image("ArcadeRival_Skin", 48, 48, _rgba_rival_skin)
    im_rh = make_arcade_image("ArcadeRival_Hair", 32, 32, _rgba_rival_hair)
    im_rf = make_arcade_image("ArcadeRival_Shoe", 32, 32, _rgba_rival_shoe)
    im_rt = make_arcade_image("ArcadeRival_Shirt", 32, 32, _rgba_rival_shirt)
    r_suit = make_textured_material("Rival_Suit", im_rs, roughness=0.55)
    r_skin = make_textured_material("Rival_Skin", im_rk, roughness=0.85)
    r_hair = make_textured_material("Rival_Hair", im_rh, roughness=0.35)
    r_shoe = make_textured_material("Rival_Shoe", im_rf, metallic=0.2, roughness=0.25)
    r_shirt = make_textured_material("Rival_Shirt", im_rt, roughness=0.5)

    def rival_zone(x, y, z):
        if z < 0.09:
            return 3
        elif z < 0.90:
            return 0
        elif z < 1.48:
            if x > 0.26 and z < 1.12:
                return 1
            elif x < 0.065 and z > 1.32 and y < -0.04:
                return 4
            else:
                return 0
        elif z < 1.54:
            return 1
        elif z < 1.78:
            return 1
        else:
            return 2

    assign_materials_by_position(
        obj, [r_suit, r_skin, r_hair, r_shoe, r_shirt], rival_zone)

    # ── Detail materials ──
    mat_glasses = make_material("Rival_Glasses", (0.25, 0.0, 0.35),
                                metallic=0.6, roughness=0.2)
    mat_frames = make_material("Rival_Frames", (0.7, 0.7, 0.7),
                               metallic=0.9, roughness=0.12)
    mat_blade = make_material("Rival_Blade", (0.85, 0.88, 0.92),
                              metallic=0.95, roughness=0.05)
    mat_wrap = make_material("Rival_KatanaWrap", (0.12, 0.0, 0.18), roughness=0.65)
    mat_neon = make_material("Rival_Neon", (0.8, 0.0, 1.0),
                             emission=(0.8, 0.0, 1.0), emission_strength=3.0)
    mat_scar = make_material("Rival_Scar", (0.85, 0.35, 0.3), roughness=0.9)
    mat_crown = make_material(
        "Rival_Crown", (0.48, 0.32, 0.10), metallic=0.9, roughness=0.22,
        emission=(0.42, 0.28, 0.08), emission_strength=0.65)

    v, f = make_box(-0.042, -0.11, 1.62, 0.030, 0.007, 0.015)
    add_detail("Rival_LensL", v, f, mat_glasses)
    v, f = make_box(0.042, -0.11, 1.62, 0.030, 0.007, 0.015)
    add_detail("Rival_LensR", v, f, mat_glasses)
    v, f = make_box(0, -0.115, 1.62, 0.011, 0.004, 0.006)
    add_detail("Rival_Bridge", v, f, mat_frames)
    v, f = make_box(-0.074, -0.05, 1.62, 0.004, 0.058, 0.004)
    add_detail("Rival_TempleL", v, f, mat_frames)
    v, f = make_box(0.074, -0.05, 1.62, 0.004, 0.058, 0.004)
    add_detail("Rival_TempleR", v, f, mat_frames)

    v, f = make_box(-0.082, -0.06, 1.60, 0.004, 0.024, 0.004)
    add_detail("Rival_Scar", v, f, mat_scar)

    lx, ly, lz = -0.24, -0.36, 1.05
    v, f = make_box(lx, ly, lz - 0.10, 0.010, 0.010, 0.12)
    add_detail("Rival_KatanaHandle", v, f, mat_wrap)
    v, f = make_box(lx, ly, lz + 0.02, 0.022, 0.022, 0.004)
    add_detail("Rival_KatanaGuard", v, f, mat_frames)
    v, f = make_tapered_box(lx, ly, lz + 0.024, 0.010, 0.004, 0.004, 0.002, 0.55)
    add_detail("Rival_KatanaBlade", v, f, mat_blade)
    v, f = make_box(lx, ly - 0.005, lz + 0.30, 0.002, 0.002, 0.25)
    add_detail("Rival_KatanaGlow", v, f, mat_neon)
    v, f = make_box(lx, ly - 0.012, lz - 0.02, 0.014, 0.014, 0.06)
    add_detail("Rival_KatanaWrap2", v, f, mat_wrap)
    v, f = make_box(lx + 0.018, ly, lz + 0.08, 0.006, 0.14, 0.003)
    add_detail("Rival_BladeRidge", v, f, mat_blade)

    v, f = make_box(0, -0.075, 1.34, 0.092, 0.018, 0.022)
    add_detail("Rival_NeckChain", v, f, mat_frames)

    v, f = make_box(-0.048, -0.10, 1.30, 0.010, 0.004, 0.07)
    add_detail("Rival_NeonLapelL", v, f, mat_neon)
    v, f = make_box(0.048, -0.10, 1.30, 0.010, 0.004, 0.07)
    add_detail("Rival_NeonLapelR", v, f, mat_neon)
    v, f = make_box(0, -0.10, 1.48, 0.048, 0.004, 0.006)
    add_detail("Rival_CollarNeon", v, f, mat_neon)
    v, f = make_box(0, -0.09, 0.91, 0.022, 0.004, 0.012)
    add_detail("Rival_BeltNeon", v, f, mat_neon)
    v, f = make_box(-0.08, -0.11, 0.014, 0.046, 0.088, 0.007)
    add_detail("Rival_SoleL", v, f, r_shoe)
    v, f = make_box(0.08, -0.11, 0.014, 0.046, 0.088, 0.007)
    add_detail("Rival_SoleR", v, f, r_shoe)

    # ── Facial features ──
    # Nose
    nv = [
        (-0.012, -0.11, 1.60), (0.012, -0.11, 1.60),
        (0.006, -0.14, 1.58), (-0.006, -0.14, 1.58),
        (-0.010, -0.11, 1.56), (0.010, -0.11, 1.56),
    ]
    nf = [(0, 1, 2, 3), (0, 4, 5, 1), (3, 2, 5, 4), (0, 3, 4), (1, 5, 2)]
    add_detail("Rival_Nose", nv, nf, r_skin)

    # Ears
    v, f = make_box(-0.088, -0.01, 1.61, 0.009, 0.007, 0.015)
    add_detail("Rival_EarL", v, f, r_skin)
    v, f = make_box(0.088, -0.01, 1.61, 0.009, 0.007, 0.015)
    add_detail("Rival_EarR", v, f, r_skin)

    v, f = make_box(-0.032, -0.11, 1.65, 0.020, 0.005, 0.005)
    add_detail("Rival_BrowL", v, f, r_skin)
    v, f = make_box(0.032, -0.11, 1.65, 0.020, 0.005, 0.005)
    add_detail("Rival_BrowR", v, f, r_skin)

    crown_spikes = [
        (0, -0.02, 1.86, 0.018, 0.018, 0.004, 0.004, 0.14),
        (-0.045, -0.015, 1.85, 0.014, 0.014, 0.003, 0.003, 0.11),
        (0.045, -0.015, 1.85, 0.014, 0.014, 0.003, 0.003, 0.11),
        (-0.024, -0.045, 1.84, 0.011, 0.011, 0.002, 0.002, 0.095),
        (0.024, -0.045, 1.84, 0.011, 0.011, 0.002, 0.002, 0.095),
    ]
    for i, (cx, cy, cz, sxb, syb, sxt, syt, h) in enumerate(crown_spikes):
        v, f = make_tapered_box(cx, cy, cz, sxb, syb, sxt, syt, h)
        add_detail(f"Rival_Crown{i}", v, f, mat_crown)

    hair_fill = [
        (0, 0.03, 1.84, 0.012, 0.018, 0.003, 0.007, 0.055),
        (-0.055, 0.008, 1.83, 0.010, 0.012, 0.003, 0.003, 0.05),
        (0.055, 0.008, 1.83, 0.010, 0.012, 0.003, 0.003, 0.05),
    ]
    for i, (cx, cy, cz, sxb, syb, sxt, syt, h) in enumerate(hair_fill):
        v, f = make_tapered_box(cx, cy, cz, sxb, syb, sxt, syt, h)
        add_detail(f"Rival_HairFill{i}", v, f, r_hair)

    v, f = make_box(-0.032, -0.10, 1.46, 0.016, 0.005, 0.012)
    add_detail("Rival_CollarL", v, f, r_suit)
    v, f = make_box(0.032, -0.10, 1.46, 0.016, 0.005, 0.012)
    add_detail("Rival_CollarR", v, f, r_suit)

    # ── Join and export ──
    out_path = os.path.join(OUT_DIR, "oyabaun_rival.glb")
    join_and_export("Rival", out_path)
    print("Rival complete.\n")


# ============================================================
# Main
# ============================================================

def main():
    variant = os.environ.get("OYABAUN_VARIANT", "all").lower()

    # Clear scene
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete()
    bpy.ops.outliner.orphans_purge(
        do_local_ids=True, do_linked_ids=True, do_recursive=True)

    if variant in ("boss", "all"):
        build_boss()

    if variant in ("rival", "all"):
        # Clear boss meshes before building rival (if building both)
        if variant == "all":
            bpy.ops.object.select_all(action='SELECT')
            bpy.ops.object.delete()
            bpy.ops.outliner.orphans_purge(
                do_local_ids=True, do_linked_ids=True, do_recursive=True)
        build_rival()

    print("Done.")


if __name__ == "__main__":
    main()
