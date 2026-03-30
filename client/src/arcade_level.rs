//! Dense Kabukicho-style arcade Tokyo alley — real 3D walkable geometry.
//!
//! Builds a [`GltfLevelCpu`] with:
//! - Narrow alley lined with Kabukicho shop facades (PixelLab pixel art)
//! - Vertical neon signs mounted on buildings
//! - Vending machines, awnings, lanterns, overhead wire tangles
//! - Dense, atmospheric, 90s arcade feel
//!
//! No Blender GLB required.

use glam::Vec3;

use crate::gltf_level::{GltfBatchCpu, GltfLevelCpu, WorldVertex};
use crate::mesh::Aabb;

// ---------------------------------------------------------------------------
// Embedded textures
// ---------------------------------------------------------------------------

// Shop facades (320×384 front-facing Kabukicho storefronts)
const SHOP_RAMEN: &[u8] = include_bytes!("../level_textures/tokyo_shops/shop_ramen.png");
const SHOP_PACHINKO: &[u8] = include_bytes!("../level_textures/tokyo_shops/shop_pachinko.png");
const SHOP_KONBINI: &[u8] = include_bytes!("../level_textures/tokyo_shops/shop_konbini.png");
const SHOP_SHUTTERED: &[u8] = include_bytes!("../level_textures/tokyo_shops/shop_shuttered.png");
const SHOP_IZAKAYA: &[u8] = include_bytes!("../level_textures/tokyo_shops/shop_izakaya.png");
const SHOP_ARCADE: &[u8] = include_bytes!("../level_textures/tokyo_shops/shop_arcade.png");
const SHOP_SNACKBAR: &[u8] = include_bytes!("../level_textures/tokyo_shops/shop_snackbar.png");
const SHOP_TATTOO: &[u8] = include_bytes!("../level_textures/tokyo_shops/shop_tattoo.png");

// Vertical neon signs
const SIGN_YAKINIKU: &[u8] = include_bytes!("../level_textures/tokyo_signs/sign_yakiniku.png");
const SIGN_KARAOKE: &[u8] = include_bytes!("../level_textures/tokyo_signs/sign_karaoke.png");
const SIGN_SAKE: &[u8] = include_bytes!("../level_textures/tokyo_signs/sign_sake.png");
const SIGN_MAHJONG: &[u8] = include_bytes!("../level_textures/tokyo_signs/sign_mahjong.png");

// Props
const VENDING_MACHINE: &[u8] = include_bytes!("../level_textures/tokyo_props/vending_machine.png");

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

const DARK_WALL: [u8; 4] = [0x12, 0x1A, 0x35, 0xFF];
const VERY_DARK: [u8; 4] = [0x08, 0x0A, 0x0F, 0xFF];
const STREET: [u8; 4] = [0x14, 0x16, 0x22, 0xFF];
const WARM_ACCENT: [u8; 4] = [0x8A, 0x3A, 0x12, 0xFF];
const LANTERN_GLOW: [u8; 4] = [0xFF, 0x7A, 0x2D, 0xFF];
const PIPE_GRAY: [u8; 4] = [0x2A, 0x2E, 0x38, 0xFF];
const WINDOW_WARM: [u8; 4] = [0x66, 0x4E, 0x28, 0xFF];
const NEON_PINK: [u8; 4] = [0xE0, 0x30, 0x80, 0xFF];
const NEON_BLUE: [u8; 4] = [0x30, 0x60, 0xE0, 0xFF];
const WET_STREET: [u8; 4] = [0x18, 0x1E, 0x30, 0xFF];

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

const STREET_HW: f32 = 2.4; // narrower = more Kabukicho claustrophobia
const BLDG_DEPTH: f32 = 3.5;
const SHOP_H: f32 = 4.0;
const UPPER_H: f32 = 2.8;
const SHOP_W: f32 = 4.8;
const SHOP_GAP: f32 = 0.35;
const SHOP_STEP: f32 = SHOP_W + SHOP_GAP;
const SHOPS_PER_SIDE: usize = 6; // 6 shops each side = longer alley
const Z_START: f32 = 4.0;

// ---------------------------------------------------------------------------
// Image indices
// ---------------------------------------------------------------------------

// 0..7 = shop textures
// 8..11 = sign textures
// 12 = vending machine
// 13+ = solid colors
const IMG_SIGN_YAKINIKU: usize = 8;
const IMG_SIGN_KARAOKE: usize = 9;
const IMG_SIGN_SAKE: usize = 10;
const IMG_SIGN_MAHJONG: usize = 11;
const IMG_VENDING: usize = 12;
const IMG_DARK_WALL: usize = 13;
const IMG_VERY_DARK: usize = 14;
const IMG_STREET: usize = 15;
const IMG_WARM_ACCENT: usize = 16;
const IMG_LANTERN: usize = 17;
const IMG_PIPE: usize = 18;
const IMG_WINDOW: usize = 19;
const IMG_NEON_PINK: usize = 20;
const IMG_NEON_BLUE: usize = 21;
const IMG_WET_STREET: usize = 22;

// ---------------------------------------------------------------------------
// Public entry
// ---------------------------------------------------------------------------

pub fn build_arcade_level() -> Result<GltfLevelCpu, String> {
    let shop_pngs: [&[u8]; 8] = [
        SHOP_RAMEN, SHOP_PACHINKO, SHOP_KONBINI, SHOP_SHUTTERED,
        SHOP_IZAKAYA, SHOP_ARCADE, SHOP_SNACKBAR, SHOP_TATTOO,
    ];

    let mut images: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    for png in &shop_pngs {
        images.push(decode_png(png)?);
    }
    // Signs
    images.push(decode_png(SIGN_YAKINIKU)?);  // 8
    images.push(decode_png(SIGN_KARAOKE)?);   // 9
    images.push(decode_png(SIGN_SAKE)?);       // 10
    images.push(decode_png(SIGN_MAHJONG)?);    // 11
    // Props
    images.push(decode_png(VENDING_MACHINE)?); // 12
    // Solid colors
    images.push((1, 1, DARK_WALL.to_vec()));   // 13
    images.push((1, 1, VERY_DARK.to_vec()));    // 14
    images.push((1, 1, STREET.to_vec()));        // 15
    images.push((1, 1, WARM_ACCENT.to_vec()));   // 16
    images.push((1, 1, LANTERN_GLOW.to_vec()));  // 17
    images.push((1, 1, PIPE_GRAY.to_vec()));      // 18
    images.push((1, 1, WINDOW_WARM.to_vec()));    // 19
    images.push((1, 1, NEON_PINK.to_vec()));      // 20
    images.push((1, 1, NEON_BLUE.to_vec()));      // 21
    images.push((1, 1, WET_STREET.to_vec()));     // 22

    let mut b = LevelBuilder::new();

    let alley_len = SHOPS_PER_SIDE as f32 * SHOP_STEP + SHOP_GAP;
    let z_far = Z_START - alley_len;

    // ══════════════════════════════════════════════════════════════════
    // STREET
    // ══════════════════════════════════════════════════════════════════
    let sx0 = -(STREET_HW + BLDG_DEPTH + 1.0);
    let sx1 = STREET_HW + BLDG_DEPTH + 1.0;

    // Main street surface
    b.quad(
        [Vec3::new(sx0, 0.0, Z_START + 2.0), Vec3::new(sx1, 0.0, Z_START + 2.0),
         Vec3::new(sx1, 0.0, z_far - 2.0), Vec3::new(sx0, 0.0, z_far - 2.0)],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        IMG_STREET, [1.0, 1.0, 1.0, 1.0],
    );

    // Wet reflection strips on street (slightly raised, lighter)
    for i in 0..3 {
        let z_c = Z_START - 3.0 - (i as f32) * (alley_len / 3.0);
        let strip_hw = STREET_HW * 0.7;
        b.quad(
            [Vec3::new(-strip_hw, 0.005, z_c + 2.5), Vec3::new(strip_hw, 0.005, z_c + 2.5),
             Vec3::new(strip_hw, 0.005, z_c - 2.5), Vec3::new(-strip_hw, 0.005, z_c - 2.5)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_WET_STREET, [1.2, 1.15, 1.3, 0.6],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // BUILDINGS — Left side (shops 0-5, reusing 8 textures cyclically)
    // ══════════════════════════════════════════════════════════════════
    let left_tex = [0usize, 1, 2, 3, 0, 7]; // ramen, pachinko, konbini, shuttered, ramen2, tattoo
    let left_stories = [2, 3, 1, 2, 3, 2]; // varying heights

    for (i, &tex) in left_tex.iter().enumerate() {
        let z0 = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP;
        let z1 = z0 - SHOP_W;
        let x_face = -STREET_HW;
        let x_back = x_face - BLDG_DEPTH;
        let stories = left_stories[i];
        let total_h = SHOP_H + stories as f32 * UPPER_H;

        build_shop_block(&mut b, x_back, x_face, z1, z0, SHOP_H, total_h, tex, true);

        // Upper floor windows (warm glow rectangles)
        for s in 0..stories {
            let wy0 = SHOP_H + s as f32 * UPPER_H + 0.5;
            let wy1 = wy0 + 1.4;
            let wz_mid = (z0 + z1) * 0.5;
            // Two windows per floor
            b.quad(
                [Vec3::new(x_face + 0.01, wy0, wz_mid + 1.2), Vec3::new(x_face + 0.01, wy0, wz_mid + 0.2),
                 Vec3::new(x_face + 0.01, wy1, wz_mid + 0.2), Vec3::new(x_face + 0.01, wy1, wz_mid + 1.2)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_WINDOW, [1.4, 1.1, 0.6, 1.0],
            );
            b.quad(
                [Vec3::new(x_face + 0.01, wy0, wz_mid - 0.2), Vec3::new(x_face + 0.01, wy0, wz_mid - 1.2),
                 Vec3::new(x_face + 0.01, wy1, wz_mid - 1.2), Vec3::new(x_face + 0.01, wy1, wz_mid - 0.2)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_WINDOW, [1.2, 1.0, 0.5, 1.0],
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // BUILDINGS — Right side (shops 6-11)
    // ══════════════════════════════════════════════════════════════════
    let right_tex = [4usize, 5, 6, 7, 4, 3]; // izakaya, arcade, snackbar, tattoo, izakaya2, shuttered
    let right_stories = [2, 2, 3, 1, 2, 3];

    for (i, &tex) in right_tex.iter().enumerate() {
        let z0 = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP;
        let z1 = z0 - SHOP_W;
        let x_face = STREET_HW;
        let x_back = x_face + BLDG_DEPTH;
        let stories = right_stories[i];
        let total_h = SHOP_H + stories as f32 * UPPER_H;

        build_shop_block(&mut b, x_face, x_back, z1, z0, SHOP_H, total_h, tex, false);

        // Upper floor windows
        for s in 0..stories {
            let wy0 = SHOP_H + s as f32 * UPPER_H + 0.5;
            let wy1 = wy0 + 1.4;
            let wz_mid = (z0 + z1) * 0.5;
            b.quad(
                [Vec3::new(x_face - 0.01, wy0, wz_mid - 1.2), Vec3::new(x_face - 0.01, wy0, wz_mid - 0.2),
                 Vec3::new(x_face - 0.01, wy1, wz_mid - 0.2), Vec3::new(x_face - 0.01, wy1, wz_mid - 1.2)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_WINDOW, [1.3, 1.0, 0.55, 1.0],
            );
            b.quad(
                [Vec3::new(x_face - 0.01, wy0, wz_mid + 0.2), Vec3::new(x_face - 0.01, wy0, wz_mid + 1.2),
                 Vec3::new(x_face - 0.01, wy1, wz_mid + 1.2), Vec3::new(x_face - 0.01, wy1, wz_mid + 0.2)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_WINDOW, [1.1, 0.9, 0.5, 1.0],
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // VERTICAL NEON SIGNS (mounted on building faces, jutting into alley)
    // ══════════════════════════════════════════════════════════════════
    let signs = [
        (IMG_SIGN_YAKINIKU, 0, true,  0.6, 2.5),  // left side, shop 0
        (IMG_SIGN_KARAOKE,  2, true,  0.4, 3.0),  // left side, shop 2
        (IMG_SIGN_SAKE,     1, false, 0.5, 2.0),  // right side, shop 1
        (IMG_SIGN_MAHJONG,  4, false, 0.6, 2.5),  // right side, shop 4
        (IMG_SIGN_YAKINIKU, 3, false, 0.5, 2.2),  // right side, shop 3
        (IMG_SIGN_KARAOKE,  5, true,  0.4, 2.8),  // left side, shop 5
    ];

    for &(tex, shop_idx, left_side, sign_w, sign_h) in &signs {
        let z_mid = Z_START - SHOP_GAP - (shop_idx as f32) * SHOP_STEP - SHOP_W * 0.5;
        let sign_y = SHOP_H + 0.3;

        if left_side {
            let x = -STREET_HW + 0.15;
            // Sign faces +X (into alley), hangs perpendicular to wall
            b.quad(
                [Vec3::new(x, sign_y, z_mid - sign_w * 0.5),
                 Vec3::new(x, sign_y, z_mid + sign_w * 0.5),
                 Vec3::new(x, sign_y + sign_h, z_mid + sign_w * 0.5),
                 Vec3::new(x, sign_y + sign_h, z_mid - sign_w * 0.5)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                tex, [2.0, 1.8, 1.6, 1.0], // emissive boost
            );
            // Back side
            b.quad(
                [Vec3::new(x, sign_y, z_mid + sign_w * 0.5),
                 Vec3::new(x, sign_y, z_mid - sign_w * 0.5),
                 Vec3::new(x, sign_y + sign_h, z_mid - sign_w * 0.5),
                 Vec3::new(x, sign_y + sign_h, z_mid + sign_w * 0.5)],
                [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
                tex, [2.0, 1.8, 1.6, 1.0],
            );
        } else {
            let x = STREET_HW - 0.15;
            b.quad(
                [Vec3::new(x, sign_y, z_mid + sign_w * 0.5),
                 Vec3::new(x, sign_y, z_mid - sign_w * 0.5),
                 Vec3::new(x, sign_y + sign_h, z_mid - sign_w * 0.5),
                 Vec3::new(x, sign_y + sign_h, z_mid + sign_w * 0.5)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                tex, [2.0, 1.8, 1.6, 1.0],
            );
            b.quad(
                [Vec3::new(x, sign_y, z_mid - sign_w * 0.5),
                 Vec3::new(x, sign_y, z_mid + sign_w * 0.5),
                 Vec3::new(x, sign_y + sign_h, z_mid + sign_w * 0.5),
                 Vec3::new(x, sign_y + sign_h, z_mid - sign_w * 0.5)],
                [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
                tex, [2.0, 1.8, 1.6, 1.0],
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // VENDING MACHINES (placed against building walls in gaps)
    // ══════════════════════════════════════════════════════════════════
    let vm_positions = [
        (-STREET_HW + 0.02, Z_START - SHOP_GAP - 0.5 * SHOP_STEP - 0.1, true),   // left gap 0-1
        (STREET_HW - 0.02, Z_START - SHOP_GAP - 2.5 * SHOP_STEP + 0.1, false),   // right gap 2-3
        (-STREET_HW + 0.02, Z_START - SHOP_GAP - 4.5 * SHOP_STEP - 0.1, true),   // left gap 4-5
    ];

    for &(x, z, left) in &vm_positions {
        let vm_w = 0.9;
        let vm_h = 1.8;
        if left {
            b.quad(
                [Vec3::new(x, 0.0, z + vm_w * 0.5), Vec3::new(x, 0.0, z - vm_w * 0.5),
                 Vec3::new(x, vm_h, z - vm_w * 0.5), Vec3::new(x, vm_h, z + vm_w * 0.5)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_VENDING, [1.6, 1.6, 1.6, 1.0], // bright, lit from inside
            );
        } else {
            b.quad(
                [Vec3::new(x, 0.0, z - vm_w * 0.5), Vec3::new(x, 0.0, z + vm_w * 0.5),
                 Vec3::new(x, vm_h, z + vm_w * 0.5), Vec3::new(x, vm_h, z - vm_w * 0.5)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_VENDING, [1.6, 1.6, 1.6, 1.0],
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // AWNINGS over every shop
    // ══════════════════════════════════════════════════════════════════
    for i in 0..SHOPS_PER_SIDE {
        let z0 = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP;
        let z1 = z0 - SHOP_W;
        let aw_drop = 0.35;
        let aw_out = 0.95;

        // Left awning
        b.quad(
            [Vec3::new(-STREET_HW, SHOP_H, z0), Vec3::new(-STREET_HW + aw_out, SHOP_H - aw_drop, z0),
             Vec3::new(-STREET_HW + aw_out, SHOP_H - aw_drop, z1), Vec3::new(-STREET_HW, SHOP_H, z1)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_WARM_ACCENT, [1.0, 1.0, 1.0, 1.0],
        );
        // Awning underside (darker)
        b.quad(
            [Vec3::new(-STREET_HW + aw_out, SHOP_H - aw_drop - 0.04, z1),
             Vec3::new(-STREET_HW + aw_out, SHOP_H - aw_drop - 0.04, z0),
             Vec3::new(-STREET_HW, SHOP_H - 0.04, z0),
             Vec3::new(-STREET_HW, SHOP_H - 0.04, z1)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_VERY_DARK, [1.0, 1.0, 1.0, 1.0],
        );

        // Right awning
        b.quad(
            [Vec3::new(STREET_HW - aw_out, SHOP_H - aw_drop, z0), Vec3::new(STREET_HW, SHOP_H, z0),
             Vec3::new(STREET_HW, SHOP_H, z1), Vec3::new(STREET_HW - aw_out, SHOP_H - aw_drop, z1)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_WARM_ACCENT, [1.0, 1.0, 1.0, 1.0],
        );
        b.quad(
            [Vec3::new(STREET_HW, SHOP_H - 0.04, z1),
             Vec3::new(STREET_HW, SHOP_H - 0.04, z0),
             Vec3::new(STREET_HW - aw_out, SHOP_H - aw_drop - 0.04, z0),
             Vec3::new(STREET_HW - aw_out, SHOP_H - aw_drop - 0.04, z1)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_VERY_DARK, [1.0, 1.0, 1.0, 1.0],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // LANTERNS (pairs at each shop, warm orange glow)
    // ══════════════════════════════════════════════════════════════════
    for i in 0..SHOPS_PER_SIDE {
        let z_mid = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP - SHOP_W * 0.5;
        let lh = 0.5;
        let lw = 0.3;
        let ly = SHOP_H - 0.7;

        // Left lanterns (two per shop)
        let lx = -STREET_HW + 0.06;
        for &z_off in &[-0.8, 0.8] {
            b.quad(
                [Vec3::new(lx, ly, z_mid + z_off - lw), Vec3::new(lx, ly, z_mid + z_off + lw),
                 Vec3::new(lx, ly + lh, z_mid + z_off + lw), Vec3::new(lx, ly + lh, z_mid + z_off - lw)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_LANTERN, [2.8, 2.0, 0.7, 1.0],
            );
        }

        // Right lanterns
        let rx = STREET_HW - 0.06;
        for &z_off in &[-0.8, 0.8] {
            b.quad(
                [Vec3::new(rx, ly, z_mid + z_off + lw), Vec3::new(rx, ly, z_mid + z_off - lw),
                 Vec3::new(rx, ly + lh, z_mid + z_off - lw), Vec3::new(rx, ly + lh, z_mid + z_off + lw)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_LANTERN, [2.8, 2.0, 0.7, 1.0],
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // OVERHEAD WIRES (dense tangle)
    // ══════════════════════════════════════════════════════════════════
    for j in 0..12 {
        let z_wire = Z_START - 1.5 - (j as f32) * (alley_len / 12.0);
        let base_y = SHOP_H + UPPER_H * 1.2;
        let wy = base_y + (j % 3) as f32 * 0.35 - (j % 2) as f32 * 0.2;
        let sag = 0.15 + (j % 4) as f32 * 0.12;
        let thick = 0.05 + (j % 3) as f32 * 0.02;
        let x_off = (j % 5) as f32 * 0.3 - 0.6;

        b.quad(
            [Vec3::new(-STREET_HW - 1.0 + x_off, wy, z_wire),
             Vec3::new(STREET_HW + 1.0 + x_off, wy - sag, z_wire),
             Vec3::new(STREET_HW + 1.0 + x_off, wy - sag + thick, z_wire),
             Vec3::new(-STREET_HW - 1.0 + x_off, wy + thick, z_wire)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_VERY_DARK, [1.0, 1.0, 1.0, 1.0],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // PIPES & AC UNITS on building sides (between-shop gaps)
    // ══════════════════════════════════════════════════════════════════
    for i in 0..(SHOPS_PER_SIDE - 1) {
        let z_gap = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP - SHOP_W;
        let pipe_w = 0.12;

        // Left side vertical pipe
        let px = -STREET_HW + 0.04;
        b.quad(
            [Vec3::new(px, 0.0, z_gap - 0.02), Vec3::new(px, 0.0, z_gap - 0.02 - pipe_w),
             Vec3::new(px, SHOP_H + UPPER_H * 2.0, z_gap - 0.02 - pipe_w),
             Vec3::new(px, SHOP_H + UPPER_H * 2.0, z_gap - 0.02)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            IMG_PIPE, [1.0, 1.0, 1.0, 1.0],
        );

        // Right side vertical pipe
        let rpx = STREET_HW - 0.04;
        b.quad(
            [Vec3::new(rpx, 0.0, z_gap - 0.02 - pipe_w), Vec3::new(rpx, 0.0, z_gap - 0.02),
             Vec3::new(rpx, SHOP_H + UPPER_H * 2.0, z_gap - 0.02),
             Vec3::new(rpx, SHOP_H + UPPER_H * 2.0, z_gap - 0.02 - pipe_w)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            IMG_PIPE, [1.0, 1.0, 1.0, 1.0],
        );

        // AC unit boxes (every other gap)
        if i % 2 == 0 {
            let ac_y = SHOP_H + 0.3;
            let ac_h = 0.6;
            let ac_w = 0.7;
            // Left AC
            b.quad(
                [Vec3::new(px + 0.01, ac_y, z_gap + 0.1), Vec3::new(px + 0.01, ac_y, z_gap + 0.1 - ac_w),
                 Vec3::new(px + 0.01, ac_y + ac_h, z_gap + 0.1 - ac_w), Vec3::new(px + 0.01, ac_y + ac_h, z_gap + 0.1)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_PIPE, [0.8, 0.8, 0.8, 1.0],
            );
            // Right AC
            b.quad(
                [Vec3::new(rpx - 0.01, ac_y, z_gap + 0.1 - ac_w), Vec3::new(rpx - 0.01, ac_y, z_gap + 0.1),
                 Vec3::new(rpx - 0.01, ac_y + ac_h, z_gap + 0.1), Vec3::new(rpx - 0.01, ac_y + ac_h, z_gap + 0.1 - ac_w)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_PIPE, [0.8, 0.8, 0.8, 1.0],
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // NEON ACCENT STRIPS (horizontal color bars on building faces)
    // ══════════════════════════════════════════════════════════════════
    for i in [0, 2, 4] {
        let z0 = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP;
        let z1 = z0 - SHOP_W;
        let ny = SHOP_H + 0.1;
        let nh = 0.08;
        // Left pink neon strip
        b.quad(
            [Vec3::new(-STREET_HW + 0.02, ny, z0 - 0.3), Vec3::new(-STREET_HW + 0.02, ny, z1 + 0.3),
             Vec3::new(-STREET_HW + 0.02, ny + nh, z1 + 0.3), Vec3::new(-STREET_HW + 0.02, ny + nh, z0 - 0.3)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_NEON_PINK, [3.0, 2.0, 2.0, 1.0],
        );
    }
    for i in [1, 3, 5] {
        let z0 = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP;
        let z1 = z0 - SHOP_W;
        let ny = SHOP_H + 0.1;
        let nh = 0.08;
        // Right blue neon strip
        b.quad(
            [Vec3::new(STREET_HW - 0.02, ny, z1 + 0.3), Vec3::new(STREET_HW - 0.02, ny, z0 - 0.3),
             Vec3::new(STREET_HW - 0.02, ny + nh, z0 - 0.3), Vec3::new(STREET_HW - 0.02, ny + nh, z1 + 0.3)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_NEON_BLUE, [2.0, 2.0, 3.0, 1.0],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // END WALLS (close the alley at both ends)
    // ══════════════════════════════════════════════════════════════════
    let wall_x0 = -(STREET_HW + BLDG_DEPTH + 0.5);
    let wall_x1 = STREET_HW + BLDG_DEPTH + 0.5;
    let max_h = SHOP_H + 3.0 * UPPER_H;

    // Far wall
    b.quad(
        [Vec3::new(wall_x0, 0.0, z_far - 1.0), Vec3::new(wall_x1, 0.0, z_far - 1.0),
         Vec3::new(wall_x1, max_h, z_far - 1.0), Vec3::new(wall_x0, max_h, z_far - 1.0)],
        [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        IMG_DARK_WALL, [1.0, 1.0, 1.0, 1.0],
    );
    // Near wall
    b.quad(
        [Vec3::new(wall_x1, 0.0, Z_START + 1.0), Vec3::new(wall_x0, 0.0, Z_START + 1.0),
         Vec3::new(wall_x0, max_h, Z_START + 1.0), Vec3::new(wall_x1, max_h, Z_START + 1.0)],
        [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        IMG_DARK_WALL, [1.0, 1.0, 1.0, 1.0],
    );

    // ══════════════════════════════════════════════════════════════════
    // COLLISION
    // ══════════════════════════════════════════════════════════════════
    let mut solids = Vec::new();

    // Floor
    solids.push(Aabb {
        min: Vec3::new(wall_x0, -0.5, z_far - 2.0),
        max: Vec3::new(wall_x1, 0.0, Z_START + 2.0),
    });

    // Building solids
    for i in 0..SHOPS_PER_SIDE {
        let z0 = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP;
        let z1 = z0 - SHOP_W;
        let lh = SHOP_H + left_stories[i] as f32 * UPPER_H;
        let rh = SHOP_H + right_stories[i] as f32 * UPPER_H;

        solids.push(Aabb {
            min: Vec3::new(-STREET_HW - BLDG_DEPTH, 0.0, z1),
            max: Vec3::new(-STREET_HW, lh, z0),
        });
        solids.push(Aabb {
            min: Vec3::new(STREET_HW, 0.0, z1),
            max: Vec3::new(STREET_HW + BLDG_DEPTH, rh, z0),
        });
    }

    // End walls
    solids.push(Aabb {
        min: Vec3::new(wall_x0, 0.0, z_far - 1.5),
        max: Vec3::new(wall_x1, max_h, z_far - 0.5),
    });
    solids.push(Aabb {
        min: Vec3::new(wall_x0, 0.0, Z_START + 0.5),
        max: Vec3::new(wall_x1, max_h, Z_START + 1.5),
    });

    // Vending machine collision
    for &(x, z, left) in &vm_positions {
        let d = 0.5;
        if left {
            solids.push(Aabb {
                min: Vec3::new(x - d, 0.0, z - 0.5),
                max: Vec3::new(x + 0.05, 1.8, z + 0.5),
            });
        } else {
            solids.push(Aabb {
                min: Vec3::new(x - 0.05, 0.0, z - 0.5),
                max: Vec3::new(x + d, 1.8, z + 0.5),
            });
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // SPAWN
    // ══════════════════════════════════════════════════════════════════
    let spawn = Vec3::new(0.0, 0.05, Z_START - 2.0);
    let spawn_yaw = 0.0; // facing -Z

    Ok(GltfLevelCpu {
        vertices: b.verts,
        indices: b.idxs,
        batches: b.batches,
        images_rgba8: images,
        spawn,
        spawn_yaw,
        solids,
        skip_floor_slab: true,
    })
}

// ---------------------------------------------------------------------------
// Building block
// ---------------------------------------------------------------------------

fn build_shop_block(
    b: &mut LevelBuilder,
    x0: f32, x1: f32,
    z0: f32, z1: f32,
    shop_h: f32, total_h: f32,
    shop_tex: usize,
    face_positive_x: bool,
) {
    let (face_x, back_x) = if face_positive_x { (x1, x0) } else { (x0, x1) };

    // Shop front (textured)
    if face_positive_x {
        b.quad(
            [Vec3::new(face_x, 0.0, z0), Vec3::new(face_x, 0.0, z1),
             Vec3::new(face_x, shop_h, z1), Vec3::new(face_x, shop_h, z0)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            shop_tex, [1.0, 1.0, 1.0, 1.0],
        );
    } else {
        b.quad(
            [Vec3::new(face_x, 0.0, z1), Vec3::new(face_x, 0.0, z0),
             Vec3::new(face_x, shop_h, z0), Vec3::new(face_x, shop_h, z1)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            shop_tex, [1.0, 1.0, 1.0, 1.0],
        );
    }

    // Upper dark wall above shop
    if total_h > shop_h + 0.1 {
        if face_positive_x {
            b.quad(
                [Vec3::new(face_x, shop_h, z0), Vec3::new(face_x, shop_h, z1),
                 Vec3::new(face_x, total_h, z1), Vec3::new(face_x, total_h, z0)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_DARK_WALL, [1.0, 1.0, 1.0, 1.0],
            );
        } else {
            b.quad(
                [Vec3::new(face_x, shop_h, z1), Vec3::new(face_x, shop_h, z0),
                 Vec3::new(face_x, total_h, z0), Vec3::new(face_x, total_h, z1)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_DARK_WALL, [1.0, 1.0, 1.0, 1.0],
            );
        }
    }

    // Roof
    b.quad(
        [Vec3::new(x0, total_h, z0), Vec3::new(x1, total_h, z0),
         Vec3::new(x1, total_h, z1), Vec3::new(x0, total_h, z1)],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        IMG_VERY_DARK, [1.0, 1.0, 1.0, 1.0],
    );

    // Side walls (z0 and z1 faces)
    b.quad(
        [Vec3::new(x0, 0.0, z1), Vec3::new(x1, 0.0, z1),
         Vec3::new(x1, total_h, z1), Vec3::new(x0, total_h, z1)],
        [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        IMG_VERY_DARK, [1.0, 1.0, 1.0, 1.0],
    );
    b.quad(
        [Vec3::new(x1, 0.0, z0), Vec3::new(x0, 0.0, z0),
         Vec3::new(x0, total_h, z0), Vec3::new(x1, total_h, z0)],
        [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        IMG_VERY_DARK, [1.0, 1.0, 1.0, 1.0],
    );

    // Back wall
    if face_positive_x {
        b.quad(
            [Vec3::new(back_x, 0.0, z1), Vec3::new(back_x, 0.0, z0),
             Vec3::new(back_x, total_h, z0), Vec3::new(back_x, total_h, z1)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            IMG_VERY_DARK, [1.0, 1.0, 1.0, 1.0],
        );
    } else {
        b.quad(
            [Vec3::new(back_x, 0.0, z0), Vec3::new(back_x, 0.0, z1),
             Vec3::new(back_x, total_h, z1), Vec3::new(back_x, total_h, z0)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            IMG_VERY_DARK, [1.0, 1.0, 1.0, 1.0],
        );
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

struct LevelBuilder {
    verts: Vec<WorldVertex>,
    idxs: Vec<u32>,
    batches: Vec<GltfBatchCpu>,
}

impl LevelBuilder {
    fn new() -> Self {
        Self { verts: Vec::new(), idxs: Vec::new(), batches: Vec::new() }
    }

    fn quad(&mut self, corners: [Vec3; 4], uvs: [[f32; 2]; 4], image_index: usize, tint: [f32; 4]) {
        let base = self.verts.len() as u32;
        let first_index = self.idxs.len() as u32;
        for (i, &pos) in corners.iter().enumerate() {
            self.verts.push(WorldVertex { pos: pos.to_array(), uv: uvs[i] });
        }
        self.idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        self.batches.push(GltfBatchCpu { first_index, index_count: 6, image_index, tint });
    }
}

// ---------------------------------------------------------------------------
// PNG decode
// ---------------------------------------------------------------------------

fn decode_png(data: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    use image::GenericImageView;
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Png)
        .map_err(|e| format!("PNG decode: {e}"))?;
    let (w, h) = img.dimensions();
    let rgba = img.into_rgba8().into_raw();
    Ok((w, h, rgba))
}
