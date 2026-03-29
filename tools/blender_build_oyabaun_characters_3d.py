"""
Oyabaun 3D Character Generator — Skin Modifier Pipeline

Builds boss and rival characters as organic humanoid meshes using Blender's
skin modifier technique. Each character is a skeleton of joints + edges with
per-joint radii, subdivided for smooth organic shapes, decimated for game
performance, then dressed with detail meshes (glasses, weapons, neon accents).

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

Optional: OYABAUN_CHAR_DECIMATE=0.42 (higher = smoother body, more verts; default 0.38).

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
    bpy.ops.object.shade_smooth()
    return obj


def build_skin_body(name, joints, edges_def, radii):
    """Build an organic body mesh from skeleton joints using skin modifier.

    Returns the Blender object after skin+subsurf+decimate are applied.
    """
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

    # Skin modifier
    obj.modifiers.new("Skin", 'SKIN')
    for i, jname in enumerate(vert_list):
        sv = mesh.skin_vertices[""].data[i]
        sv.radius = radii.get(jname, (0.04, 0.04))
    mesh.skin_vertices[""].data[vert_list.index('pelvis')].use_root = True

    # Subdivision for smoothness
    sub = obj.modifiers.new("Subsurf", 'SUBSURF')
    sub.levels = 2
    sub.render_levels = 2

    # Apply modifiers
    bpy.ops.object.modifier_apply(modifier="Skin")
    bpy.ops.object.modifier_apply(modifier="Subsurf")
    bpy.ops.object.shade_smooth()

    # Decimate for game performance (ratio = fraction of faces *kept*)
    dec = obj.modifiers.new("Decimate", 'DECIMATE')
    try:
        dec.ratio = float(os.environ.get("OYABAUN_CHAR_DECIMATE", "0.38"))
    except ValueError:
        dec.ratio = 0.38
    dec.ratio = max(0.18, min(0.52, dec.ratio))
    bpy.ops.object.modifier_apply(modifier="Decimate")
    bpy.ops.object.shade_smooth()

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
    """Build the Boss (oyabaun_player) — dark suit, broad shoulders, pistol."""
    print("Building Boss character...")

    # ── Joint positions (Z-up, feet at Z=0, total height ~1.85m) ──
    joints = {
        'pelvis':      (0, 0, 0.90),
        'waist':       (0, 0, 1.00),
        'chest':       (0, 0, 1.20),
        'upper_chest': (0, 0, 1.38),
        'neck':        (0, 0, 1.50),
        'head':        (0, 0, 1.65),
        'head_top':    (0, 0, 1.82),
        # Left arm (relaxed at side)
        'l_shoulder':  (-0.22, 0, 1.42),
        'l_upper_arm': (-0.32, 0, 1.35),
        'l_elbow':     (-0.42, 0, 1.18),
        'l_forearm':   (-0.46, -0.02, 1.05),
        'l_wrist':     (-0.48, -0.04, 0.92),
        'l_hand':      (-0.48, -0.06, 0.82),
        # Right arm (gun arm — slightly forward)
        'r_shoulder':  (0.22, 0, 1.42),
        'r_upper_arm': (0.32, 0, 1.35),
        'r_elbow':     (0.38, -0.08, 1.22),
        'r_forearm':   (0.36, -0.18, 1.18),
        'r_wrist':     (0.34, -0.28, 1.16),
        'r_hand':      (0.33, -0.34, 1.16),
        # Left leg
        'l_hip':       (-0.10, 0, 0.88),
        'l_knee':      (-0.11, 0.02, 0.48),
        'l_ankle':     (-0.11, 0, 0.08),
        'l_foot':      (-0.11, -0.08, 0.02),
        'l_toe':       (-0.11, -0.16, 0.02),
        # Right leg
        'r_hip':       (0.10, 0, 0.88),
        'r_knee':      (0.11, 0.02, 0.48),
        'r_ankle':     (0.11, 0, 0.08),
        'r_foot':      (0.11, -0.08, 0.02),
        'r_toe':       (0.11, -0.16, 0.02),
    }

    # ── Per-joint radii (rx=left/right width, ry=front/back depth) ──
    radii = {
        'pelvis':      (0.14, 0.10),
        'waist':       (0.13, 0.10),
        'chest':       (0.16, 0.12),
        'upper_chest': (0.18, 0.12),  # broad shoulders
        'neck':        (0.06, 0.06),
        'head':        (0.10, 0.11),
        'head_top':    (0.08, 0.09),
        'l_shoulder':  (0.06, 0.06),
        'l_upper_arm': (0.055, 0.055),
        'l_elbow':     (0.045, 0.045),
        'l_forearm':   (0.04, 0.04),
        'l_wrist':     (0.035, 0.03),
        'l_hand':      (0.035, 0.04),
        'r_shoulder':  (0.06, 0.06),
        'r_upper_arm': (0.055, 0.055),
        'r_elbow':     (0.045, 0.045),
        'r_forearm':   (0.04, 0.04),
        'r_wrist':     (0.035, 0.03),
        'r_hand':      (0.035, 0.04),
        'l_hip':       (0.08, 0.08),
        'l_knee':      (0.055, 0.06),
        'l_ankle':     (0.04, 0.04),
        'l_foot':      (0.04, 0.08),
        'l_toe':       (0.03, 0.04),
        'r_hip':       (0.08, 0.08),
        'r_knee':      (0.055, 0.06),
        'r_ankle':     (0.04, 0.04),
        'r_foot':      (0.04, 0.08),
        'r_toe':       (0.03, 0.04),
    }

    obj = build_skin_body("Boss", joints, SKELETON_EDGES, radii)

    # ── Materials (slot order: 0=suit, 1=skin, 2=hair, 3=shoe, 4=shirt) ──
    mat_suit = make_material("Boss_Suit", (0.04, 0.04, 0.06), roughness=0.65)
    mat_skin = make_material("Boss_Skin", (0.72, 0.55, 0.42), roughness=0.85)
    mat_hair = make_material("Boss_Hair", (0.02, 0.02, 0.02), roughness=0.35)
    mat_shoe = make_material("Boss_Shoe", (0.02, 0.02, 0.02), metallic=0.3, roughness=0.25)
    mat_shirt = make_material("Boss_Shirt", (0.12, 0.01, 0.01), roughness=0.5)

    def boss_zone(x, y, z):
        if z < 0.10:
            return 3   # shoe
        elif z < 0.88:
            return 0   # suit pants
        elif z < 1.50:
            if x > 0.30 and z < 1.20:
                return 1   # skin (forearms/hands)
            elif x < 0.08 and z > 1.30 and y < -0.04:
                return 4   # shirt (visible at collar V)
            else:
                return 0   # suit jacket
        elif z < 1.55:
            return 1   # skin (neck)
        elif z < 1.72:
            return 1   # skin (head)
        else:
            return 2   # hair

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
    mat_tie = make_material("Boss_Tie", (0.5, 0.02, 0.02), roughness=0.45)
    mat_cig = make_material("Boss_Cigarette", (0.75, 0.72, 0.68), roughness=0.7)
    mat_cig_tip = make_material("Boss_CigaretteTip", (0.9, 0.4, 0.1),
                                emission=(0.9, 0.4, 0.1), emission_strength=2.0)

    # ── Sunglasses ──
    v, f = make_box(-0.05, -0.11, 1.64, 0.035, 0.008, 0.018)
    add_detail("Boss_LensL", v, f, mat_glasses)
    v, f = make_box(0.05, -0.11, 1.64, 0.035, 0.008, 0.018)
    add_detail("Boss_LensR", v, f, mat_glasses)
    v, f = make_box(0, -0.115, 1.64, 0.015, 0.005, 0.008)
    add_detail("Boss_Bridge", v, f, mat_frames)
    v, f = make_box(-0.085, -0.05, 1.64, 0.005, 0.065, 0.005)
    add_detail("Boss_TempleL", v, f, mat_frames)
    v, f = make_box(0.085, -0.05, 1.64, 0.005, 0.065, 0.005)
    add_detail("Boss_TempleR", v, f, mat_frames)

    # ── Tie ──
    tv = [
        (-0.02, -0.115, 1.40), (0.02, -0.115, 1.40),
        (0.025, -0.12, 1.25), (-0.025, -0.12, 1.25),
        (-0.018, -0.11, 1.08), (0.018, -0.11, 1.08),
        (0.015, -0.105, 0.95), (-0.015, -0.105, 0.95),
    ]
    tf = [(0, 1, 2, 3), (3, 2, 5, 4), (4, 5, 6, 7)]
    add_detail("Boss_Tie", tv, tf, mat_tie)

    # ── Pistol in right hand ──
    rx, ry, rz = 0.33, -0.38, 1.16
    v, f = make_box(rx, ry, rz, 0.015, 0.08, 0.022)
    add_detail("Boss_GunSlide", v, f, mat_gun)
    v, f = make_box(rx, ry - 0.06, rz + 0.008, 0.008, 0.04, 0.008)
    add_detail("Boss_GunBarrel", v, f, mat_gun)
    v, f = make_box(rx, ry + 0.02, rz - 0.035, 0.013, 0.018, 0.028)
    add_detail("Boss_GunGripMesh", v, f, mat_gun_grip)
    v, f = make_box(rx, ry - 0.005, rz - 0.015, 0.004, 0.018, 0.004)
    add_detail("Boss_GunTrig", v, f, mat_gun)

    # ── Neon accents (cyan) ──
    v, f = make_box(-0.06, -0.12, 1.30, 0.012, 0.004, 0.08)
    add_detail("Boss_NeonLapelL", v, f, mat_neon)
    v, f = make_box(0.06, -0.12, 1.30, 0.012, 0.004, 0.08)
    add_detail("Boss_NeonLapelR", v, f, mat_neon)
    v, f = make_box(-0.10, -0.12, 1.32, 0.018, 0.004, 0.012)
    add_detail("Boss_PocketNeon", v, f, mat_neon)
    v, f = make_box(0, -0.10, 0.88, 0.025, 0.005, 0.015)
    add_detail("Boss_BeltNeon", v, f, mat_neon)

    # ── Facial features ──
    # Nose (wedge protruding forward)
    nv = [
        (-0.015, -0.12, 1.62), (0.015, -0.12, 1.62),
        (0.008, -0.15, 1.60), (-0.008, -0.15, 1.60),
        (-0.012, -0.12, 1.58), (0.012, -0.12, 1.58),
    ]
    nf = [(0, 1, 2, 3), (0, 4, 5, 1), (3, 2, 5, 4), (0, 3, 4), (1, 5, 2)]
    add_detail("Boss_Nose", nv, nf, mat_skin)

    # Ears
    v, f = make_box(-0.11, -0.01, 1.63, 0.012, 0.008, 0.018)
    add_detail("Boss_EarL", v, f, mat_skin)
    v, f = make_box(0.11, -0.01, 1.63, 0.012, 0.008, 0.018)
    add_detail("Boss_EarR", v, f, mat_skin)

    # Eyebrow ridges
    v, f = make_box(-0.04, -0.115, 1.67, 0.025, 0.006, 0.006)
    add_detail("Boss_BrowL", v, f, mat_skin)
    v, f = make_box(0.04, -0.115, 1.67, 0.025, 0.006, 0.006)
    add_detail("Boss_BrowR", v, f, mat_skin)

    # Chin/jaw
    v, f = make_tapered_box(0, -0.10, 1.54, 0.03, 0.025, 0.025, 0.02, 0.03)
    add_detail("Boss_Chin", v, f, mat_skin)

    # Cigarette
    v, f = make_box(0.02, -0.15, 1.57, 0.004, 0.03, 0.004)
    add_detail("Boss_Cigarette", v, f, mat_cig)
    v, f = make_box(0.02, -0.18, 1.57, 0.005, 0.005, 0.005)
    add_detail("Boss_CigTip", v, f, mat_cig_tip)

    # ── Suit details ──
    # Collar points
    v, f = make_box(-0.04, -0.11, 1.46, 0.02, 0.006, 0.015)
    add_detail("Boss_CollarL", v, f, mat_suit)
    v, f = make_box(0.04, -0.11, 1.46, 0.02, 0.006, 0.015)
    add_detail("Boss_CollarR", v, f, mat_suit)

    # Shoulder pads
    v, f = make_tapered_box(-0.22, 0, 1.43, 0.04, 0.04, 0.03, 0.03, 0.02)
    add_detail("Boss_ShoulderL", v, f, mat_suit)
    v, f = make_tapered_box(0.22, 0, 1.43, 0.04, 0.04, 0.03, 0.03, 0.02)
    add_detail("Boss_ShoulderR", v, f, mat_suit)

    # Slicked hair volume (+Y = back of head; front is -Y)
    v, f = make_box(0, 0.07, 1.76, 0.10, 0.035, 0.028)
    add_detail("Boss_HairBack", v, f, mat_hair)
    v, f = make_tapered_box(-0.05, 0.05, 1.75, 0.05, 0.03, 0.03, 0.012, 0.045)
    add_detail("Boss_HairSideL", v, f, mat_hair)
    v, f = make_tapered_box(0.05, 0.05, 1.75, 0.05, 0.03, 0.03, 0.012, 0.045)
    add_detail("Boss_HairSideR", v, f, mat_hair)

    # Lapel flaps (read as V from front)
    v, f = make_box(-0.095, -0.108, 1.20, 0.028, 0.004, 0.11)
    add_detail("Boss_LapelFlapL", v, f, mat_suit)
    v, f = make_box(0.095, -0.108, 1.20, 0.028, 0.004, 0.11)
    add_detail("Boss_LapelFlapR", v, f, mat_suit)

    # Breast pocket + sleeve cuff hints
    v, f = make_box(-0.14, -0.10, 1.05, 0.04, 0.006, 0.05)
    add_detail("Boss_Pocket", v, f, mat_suit)
    v, f = make_box(-0.48, -0.05, 0.88, 0.022, 0.006, 0.035)
    add_detail("Boss_CuffL", v, f, mat_suit)

    # Shoe soles / toe cap
    v, f = make_box(-0.11, -0.11, 0.012, 0.055, 0.11, 0.008)
    add_detail("Boss_SoleL", v, f, mat_shoe)
    v, f = make_box(0.11, -0.11, 0.012, 0.055, 0.11, 0.008)
    add_detail("Boss_SoleR", v, f, mat_shoe)

    # Pistol magazine + slide serrations (extra metal)
    v, f = make_box(rx, ry + 0.045, rz - 0.018, 0.011, 0.028, 0.038)
    add_detail("Boss_GunMag", v, f, mat_gun)
    v, f = make_box(rx, ry - 0.03, rz + 0.006, 0.016, 0.022, 0.006)
    add_detail("Boss_GunSerration", v, f, mat_gun)

    # Signet ring (right hand)
    v, f = make_box(0.30, -0.33, 1.135, 0.014, 0.01, 0.008)
    add_detail("Boss_Signet", v, f, mat_frames)

    # ── Join and export ──
    out_path = os.path.join(OUT_DIR, "oyabaun_player.glb")
    join_and_export("Boss", out_path)
    print("Boss complete.\n")


# ============================================================
# RIVAL CHARACTER
# ============================================================

def build_rival():
    """Build the Rival (oyabaun_rival) — white suit, lean build, katana."""
    print("Building Rival character...")

    # ── Joint positions (slightly shorter, leaner) ──
    joints = {
        'pelvis':      (0, 0, 0.88),
        'waist':       (0, 0, 0.98),
        'chest':       (0, 0, 1.18),
        'upper_chest': (0, 0, 1.35),
        'neck':        (0, 0, 1.46),
        'head':        (0, 0, 1.60),
        'head_top':    (0, 0, 1.78),
        # Left arm — katana arm, extended forward
        'l_shoulder':  (-0.20, 0, 1.38),
        'l_upper_arm': (-0.28, -0.05, 1.30),
        'l_elbow':     (-0.34, -0.15, 1.20),
        'l_forearm':   (-0.32, -0.25, 1.16),
        'l_wrist':     (-0.30, -0.32, 1.14),
        'l_hand':      (-0.28, -0.36, 1.12),
        # Right arm — relaxed at side
        'r_shoulder':  (0.20, 0, 1.38),
        'r_upper_arm': (0.28, 0, 1.30),
        'r_elbow':     (0.38, 0, 1.15),
        'r_forearm':   (0.42, 0, 1.02),
        'r_wrist':     (0.42, -0.02, 0.90),
        'r_hand':      (0.42, -0.04, 0.82),
        # Legs
        'l_hip':       (-0.09, 0, 0.86),
        'l_knee':      (-0.10, 0.02, 0.46),
        'l_ankle':     (-0.10, 0, 0.07),
        'l_foot':      (-0.10, -0.08, 0.02),
        'l_toe':       (-0.10, -0.15, 0.02),
        'r_hip':       (0.09, 0, 0.86),
        'r_knee':      (0.10, 0.02, 0.46),
        'r_ankle':     (0.10, 0, 0.07),
        'r_foot':      (0.10, -0.08, 0.02),
        'r_toe':       (0.10, -0.15, 0.02),
    }

    # ── Slimmer proportions than boss ──
    radii = {
        'pelvis':      (0.12, 0.09),
        'waist':       (0.11, 0.09),
        'chest':       (0.14, 0.10),
        'upper_chest': (0.16, 0.10),
        'neck':        (0.05, 0.05),
        'head':        (0.09, 0.10),
        'head_top':    (0.07, 0.08),
        'l_shoulder':  (0.055, 0.055),
        'l_upper_arm': (0.048, 0.048),
        'l_elbow':     (0.04, 0.04),
        'l_forearm':   (0.035, 0.035),
        'l_wrist':     (0.03, 0.028),
        'l_hand':      (0.03, 0.035),
        'r_shoulder':  (0.055, 0.055),
        'r_upper_arm': (0.048, 0.048),
        'r_elbow':     (0.04, 0.04),
        'r_forearm':   (0.035, 0.035),
        'r_wrist':     (0.03, 0.028),
        'r_hand':      (0.03, 0.035),
        'l_hip':       (0.07, 0.07),
        'l_knee':      (0.05, 0.055),
        'l_ankle':     (0.035, 0.035),
        'l_foot':      (0.035, 0.07),
        'l_toe':       (0.028, 0.035),
        'r_hip':       (0.07, 0.07),
        'r_knee':      (0.05, 0.055),
        'r_ankle':     (0.035, 0.035),
        'r_foot':      (0.035, 0.07),
        'r_toe':       (0.028, 0.035),
    }

    obj = build_skin_body("Rival", joints, SKELETON_EDGES, radii)

    # ── Materials (slot order: 0=suit, 1=skin, 2=hair, 3=shoe, 4=shirt) ──
    r_suit = make_material("Rival_Suit", (0.85, 0.82, 0.78), roughness=0.55)
    r_skin = make_material("Rival_Skin", (0.65, 0.48, 0.35), roughness=0.85)
    r_hair = make_material("Rival_Hair", (0.82, 0.78, 0.65), roughness=0.35)
    r_shoe = make_material("Rival_Shoe", (0.80, 0.78, 0.75), metallic=0.2, roughness=0.25)
    r_shirt = make_material("Rival_Shirt", (0.08, 0.08, 0.10), roughness=0.5)

    def rival_zone(x, y, z):
        if z < 0.09:
            return 3   # shoe
        elif z < 0.86:
            return 0   # suit pants
        elif z < 1.46:
            if x > 0.28 and z < 1.15:
                return 1   # skin (forearms)
            elif x < 0.07 and z > 1.28 and y < -0.04:
                return 4   # shirt
            else:
                return 0   # suit
        elif z < 1.52:
            return 1   # skin (neck)
        elif z < 1.68:
            return 1   # skin (head)
        else:
            return 2   # hair

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

    # ── Sunglasses (purple lenses) ──
    v, f = make_box(-0.045, -0.11, 1.60, 0.032, 0.007, 0.016)
    add_detail("Rival_LensL", v, f, mat_glasses)
    v, f = make_box(0.045, -0.11, 1.60, 0.032, 0.007, 0.016)
    add_detail("Rival_LensR", v, f, mat_glasses)
    v, f = make_box(0, -0.115, 1.60, 0.012, 0.004, 0.006)
    add_detail("Rival_Bridge", v, f, mat_frames)
    v, f = make_box(-0.078, -0.05, 1.60, 0.004, 0.06, 0.004)
    add_detail("Rival_TempleL", v, f, mat_frames)
    v, f = make_box(0.078, -0.05, 1.60, 0.004, 0.06, 0.004)
    add_detail("Rival_TempleR", v, f, mat_frames)

    # ── Facial scar ──
    v, f = make_box(-0.085, -0.06, 1.58, 0.004, 0.025, 0.004)
    add_detail("Rival_Scar", v, f, mat_scar)

    # ── Katana in left hand ──
    lx, ly, lz = -0.28, -0.36, 1.12
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

    # Silver chain at open collar (uses frame metal)
    v, f = make_box(0, -0.075, 1.295, 0.095, 0.018, 0.022)
    add_detail("Rival_NeckChain", v, f, mat_frames)

    # ── Neon accents (purple) ──
    v, f = make_box(-0.05, -0.10, 1.26, 0.010, 0.004, 0.07)
    add_detail("Rival_NeonLapelL", v, f, mat_neon)
    v, f = make_box(0.05, -0.10, 1.26, 0.010, 0.004, 0.07)
    add_detail("Rival_NeonLapelR", v, f, mat_neon)
    v, f = make_box(0, -0.10, 1.44, 0.05, 0.004, 0.006)
    add_detail("Rival_CollarNeon", v, f, mat_neon)
    v, f = make_box(0, -0.09, 0.87, 0.022, 0.004, 0.012)
    add_detail("Rival_BeltNeon", v, f, mat_neon)
    v, f = make_box(-0.10, -0.11, 0.014, 0.048, 0.09, 0.007)
    add_detail("Rival_SoleL", v, f, r_shoe)
    v, f = make_box(0.10, -0.11, 0.014, 0.048, 0.09, 0.007)
    add_detail("Rival_SoleR", v, f, r_shoe)

    # ── Facial features ──
    # Nose
    nv = [
        (-0.012, -0.11, 1.58), (0.012, -0.11, 1.58),
        (0.006, -0.14, 1.56), (-0.006, -0.14, 1.56),
        (-0.010, -0.11, 1.54), (0.010, -0.11, 1.54),
    ]
    nf = [(0, 1, 2, 3), (0, 4, 5, 1), (3, 2, 5, 4), (0, 3, 4), (1, 5, 2)]
    add_detail("Rival_Nose", nv, nf, r_skin)

    # Ears
    v, f = make_box(-0.10, -0.01, 1.59, 0.010, 0.007, 0.016)
    add_detail("Rival_EarL", v, f, r_skin)
    v, f = make_box(0.10, -0.01, 1.59, 0.010, 0.007, 0.016)
    add_detail("Rival_EarR", v, f, r_skin)

    # Eyebrow ridges
    v, f = make_box(-0.035, -0.11, 1.63, 0.022, 0.005, 0.005)
    add_detail("Rival_BrowL", v, f, r_skin)
    v, f = make_box(0.035, -0.11, 1.63, 0.022, 0.005, 0.005)
    add_detail("Rival_BrowR", v, f, r_skin)

    # Spiky hair (5 main spikes + smaller ones)
    spike_mat = r_hair
    spikes = [
        (0, -0.02, 1.78, 0.02, 0.02, 0.005, 0.005, 0.10),
        (-0.04, -0.01, 1.77, 0.015, 0.015, 0.004, 0.004, 0.08),
        (0.04, -0.01, 1.77, 0.015, 0.015, 0.004, 0.004, 0.08),
        (-0.02, -0.04, 1.77, 0.012, 0.012, 0.003, 0.003, 0.07),
        (0.02, -0.04, 1.77, 0.012, 0.012, 0.003, 0.003, 0.07),
        (0, 0.03, 1.76, 0.014, 0.02, 0.004, 0.008, 0.06),
        (-0.06, 0.01, 1.76, 0.012, 0.014, 0.003, 0.004, 0.065),
        (0.06, 0.01, 1.76, 0.012, 0.014, 0.003, 0.004, 0.065),
    ]
    for i, (cx, cy, cz, sxb, syb, sxt, syt, h) in enumerate(spikes):
        v, f = make_tapered_box(cx, cy, cz, sxb, syb, sxt, syt, h)
        add_detail(f"Rival_Spike{i}", v, f, spike_mat)

    # Collar points
    v, f = make_box(-0.035, -0.10, 1.43, 0.018, 0.005, 0.013)
    add_detail("Rival_CollarL", v, f, r_suit)
    v, f = make_box(0.035, -0.10, 1.43, 0.018, 0.005, 0.013)
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
