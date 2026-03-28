# Blender Character Modeling Skill

## Current Characters in Scene
- **Boss_Armature** at Blender (0.5, -7.0, 0.0) facing +Y — 18 mesh children
- **Rival_Armature** at Blender (1.5, -12.0, 0.0) facing +Y — 19 mesh children
- Collection: "Characters"

## Style Target
90s game aesthetic — NOT blocky/Roblox. Think Virtua Fighter, Tekken, early PS1.
- Smooth limbs with actual shape (tapered cylinders, not cubes)
- Distinct facial features, hair detail
- Clothing folds/detail via geometry, not just color
- Proportions: semi-realistic, slight stylization

## Armature Convention
7 bones: Hips, Spine, Head, ArmL, ArmR, LegL, LegR
- All mesh parts must have vertex groups matching bone names
- Automatic weights via parent_set='ARMATURE_AUTO'

## Materials (Boss)
Boss_Skin(0.35,0.18,0.08), Boss_Suit(0.92,0.90,0.85), Boss_Hat(0.95,0.93,0.88),
Boss_Flower(0.85,0.08,0.08), Boss_Shoes(0.06,0.04,0.03), Boss_Shirt(0.75,0.72,0.68)

## Materials (Rival)
Rival_Jacket(0.08,0.10,0.18), Rival_Glasses(0.15,0.35,0.45), Rival_Hair(0.02,0.02,0.04),
Rival_Skin(0.72,0.55,0.42), Rival_Pants(0.12,0.12,0.15), Rival_Shoes(0.05,0.04,0.04)

## Animation
- Actions: Boss_Idle, Rival_Idle (60 frames)
- Keyframes at 1, 15, 30, 45, 60 with linear interpolation
- Subtle breathing/sway motion

## Mesh Parenting Fix
When creating meshes at offset positions and parenting to armature:
- Subtract the creation offset from mesh local position BEFORE parenting
- Or create meshes at origin, shape them, then parent
