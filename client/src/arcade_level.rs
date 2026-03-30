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
const TRASH_BAGS: &[u8] = include_bytes!("../level_textures/tokyo_props/trash_bags.png");
const BEER_CRATES: &[u8] = include_bytes!("../level_textures/tokyo_props/beer_crates.png");
const NEON_ARROW: &[u8] = include_bytes!("../level_textures/tokyo_props/neon_arrow.png");
const NOREN_CURTAIN: &[u8] = include_bytes!("../level_textures/tokyo_props/noren_curtain.png");
const BICYCLE: &[u8] = include_bytes!("../level_textures/tokyo_props/bicycle.png");

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
const IMG_TRASH: usize = 23;
const IMG_CRATES: usize = 24;
const IMG_ARROW: usize = 25;
const IMG_NOREN: usize = 26;
const IMG_BICYCLE: usize = 27;

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
    // New props
    images.push(decode_png(TRASH_BAGS)?);         // 23
    images.push(decode_png(BEER_CRATES)?);        // 24
    images.push(decode_png(NEON_ARROW)?);          // 25
    images.push(decode_png(NOREN_CURTAIN)?);       // 26
    images.push(decode_png(BICYCLE)?);             // 27

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
    // CURB / STEP (raised sidewalk edge along buildings)
    // ══════════════════════════════════════════════════════════════════
    let curb_h = 0.12;
    let curb_w = 0.3;
    // Left curb — top
    b.quad(
        [Vec3::new(-STREET_HW, curb_h, Z_START + 0.5), Vec3::new(-STREET_HW + curb_w, curb_h, Z_START + 0.5),
         Vec3::new(-STREET_HW + curb_w, curb_h, z_far - 0.5), Vec3::new(-STREET_HW, curb_h, z_far - 0.5)],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        IMG_PIPE, [0.9, 0.9, 0.9, 1.0],
    );
    // Left curb — face
    b.quad(
        [Vec3::new(-STREET_HW + curb_w, 0.0, Z_START + 0.5), Vec3::new(-STREET_HW + curb_w, curb_h, Z_START + 0.5),
         Vec3::new(-STREET_HW + curb_w, curb_h, z_far - 0.5), Vec3::new(-STREET_HW + curb_w, 0.0, z_far - 0.5)],
        [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        IMG_PIPE, [0.7, 0.7, 0.7, 1.0],
    );
    // Right curb — top
    b.quad(
        [Vec3::new(STREET_HW - curb_w, curb_h, Z_START + 0.5), Vec3::new(STREET_HW, curb_h, Z_START + 0.5),
         Vec3::new(STREET_HW, curb_h, z_far - 0.5), Vec3::new(STREET_HW - curb_w, curb_h, z_far - 0.5)],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        IMG_PIPE, [0.9, 0.9, 0.9, 1.0],
    );
    // Right curb — face
    b.quad(
        [Vec3::new(STREET_HW - curb_w, curb_h, z_far - 0.5), Vec3::new(STREET_HW - curb_w, curb_h, Z_START + 0.5),
         Vec3::new(STREET_HW - curb_w, 0.0, Z_START + 0.5), Vec3::new(STREET_HW - curb_w, 0.0, z_far - 0.5)],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        IMG_PIPE, [0.7, 0.7, 0.7, 1.0],
    );

    // ══════════════════════════════════════════════════════════════════
    // DRAIN GUTTER (dark strip down center of street)
    // ══════════════════════════════════════════════════════════════════
    let gutter_hw = 0.15;
    b.quad(
        [Vec3::new(-gutter_hw, 0.003, Z_START + 0.5), Vec3::new(gutter_hw, 0.003, Z_START + 0.5),
         Vec3::new(gutter_hw, 0.003, z_far - 0.5), Vec3::new(-gutter_hw, 0.003, z_far - 0.5)],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        IMG_VERY_DARK, [1.0, 1.0, 1.0, 1.0],
    );

    // ══════════════════════════════════════════════════════════════════
    // BLADE SIGNS (perpendicular signs sticking out into alley)
    // ══════════════════════════════════════════════════════════════════
    // These use existing sign textures but are mounted perpendicular to the wall
    let blade_signs = [
        (1, true, IMG_SIGN_SAKE, 0.5, 1.5),      // left shop 1
        (3, false, IMG_SIGN_KARAOKE, 0.5, 1.8),   // right shop 3
        (4, true, IMG_SIGN_MAHJONG, 0.5, 1.5),    // left shop 4
        (5, false, IMG_SIGN_YAKINIKU, 0.5, 1.6),  // right shop 5
    ];
    for &(shop_idx, left, tex, blade_w, blade_h) in &blade_signs {
        let z_edge = Z_START - SHOP_GAP - (shop_idx as f32) * SHOP_STEP - 0.3;
        let by = SHOP_H + 0.5;
        if left {
            let bx = -STREET_HW;
            // Blade sticks out in +X
            b.quad(
                [Vec3::new(bx, by, z_edge), Vec3::new(bx + blade_w, by, z_edge),
                 Vec3::new(bx + blade_w, by + blade_h, z_edge), Vec3::new(bx, by + blade_h, z_edge)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                tex, [2.2, 1.8, 1.5, 1.0],
            );
            // Back face
            b.quad(
                [Vec3::new(bx + blade_w, by, z_edge), Vec3::new(bx, by, z_edge),
                 Vec3::new(bx, by + blade_h, z_edge), Vec3::new(bx + blade_w, by + blade_h, z_edge)],
                [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
                tex, [2.2, 1.8, 1.5, 1.0],
            );
        } else {
            let bx = STREET_HW;
            b.quad(
                [Vec3::new(bx - blade_w, by, z_edge), Vec3::new(bx, by, z_edge),
                 Vec3::new(bx, by + blade_h, z_edge), Vec3::new(bx - blade_w, by + blade_h, z_edge)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                tex, [2.2, 1.8, 1.5, 1.0],
            );
            b.quad(
                [Vec3::new(bx, by, z_edge), Vec3::new(bx - blade_w, by, z_edge),
                 Vec3::new(bx - blade_w, by + blade_h, z_edge), Vec3::new(bx, by + blade_h, z_edge)],
                [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
                tex, [2.2, 1.8, 1.5, 1.0],
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // HORIZONTAL BANNERS (cloth banners across shop fronts, above door level)
    // ══════════════════════════════════════════════════════════════════
    for i in [0, 2, 5] {
        let z0 = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP;
        let z1 = z0 - SHOP_W;
        let banner_y = 2.8;
        let banner_h = 0.5;
        // Left banner
        b.quad(
            [Vec3::new(-STREET_HW + 0.03, banner_y, z0 - 0.4), Vec3::new(-STREET_HW + 0.03, banner_y, z1 + 0.4),
             Vec3::new(-STREET_HW + 0.03, banner_y + banner_h, z1 + 0.4), Vec3::new(-STREET_HW + 0.03, banner_y + banner_h, z0 - 0.4)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            IMG_WARM_ACCENT, [1.3, 0.9, 0.5, 1.0],
        );
    }
    for i in [1, 3, 4] {
        let z0 = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP;
        let z1 = z0 - SHOP_W;
        let banner_y = 2.6;
        let banner_h = 0.45;
        // Right banner
        b.quad(
            [Vec3::new(STREET_HW - 0.03, banner_y, z1 + 0.4), Vec3::new(STREET_HW - 0.03, banner_y, z0 - 0.4),
             Vec3::new(STREET_HW - 0.03, banner_y + banner_h, z0 - 0.4), Vec3::new(STREET_HW - 0.03, banner_y + banner_h, z1 + 0.4)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            IMG_WARM_ACCENT, [1.1, 0.7, 0.4, 1.0],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // FLOOR DETAIL — manhole covers, utility markings
    // ══════════════════════════════════════════════════════════════════
    // Manhole covers (small dark circles approximated as squares)
    for &z_pos in &[-5.0, -15.0, -25.0] {
        let mh = 0.5;
        b.quad(
            [Vec3::new(-mh, 0.004, z_pos - mh), Vec3::new(mh, 0.004, z_pos - mh),
             Vec3::new(mh, 0.004, z_pos + mh), Vec3::new(-mh, 0.004, z_pos + mh)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_PIPE, [0.6, 0.6, 0.6, 1.0],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // UPPER BUILDING DETAIL — ledges, rooftop lips
    // ══════════════════════════════════════════════════════════════════
    for i in 0..SHOPS_PER_SIDE {
        let z0 = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP;
        let z1 = z0 - SHOP_W;
        let lh = SHOP_H + left_stories[i] as f32 * UPPER_H;
        let rh = SHOP_H + right_stories[i] as f32 * UPPER_H;
        let lip = 0.15;

        // Left rooftop lip
        b.quad(
            [Vec3::new(-STREET_HW, lh, z0), Vec3::new(-STREET_HW + lip, lh, z0),
             Vec3::new(-STREET_HW + lip, lh, z1), Vec3::new(-STREET_HW, lh, z1)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_PIPE, [0.6, 0.6, 0.6, 1.0],
        );
        b.quad(
            [Vec3::new(-STREET_HW + lip, lh - 0.1, z0), Vec3::new(-STREET_HW + lip, lh, z0),
             Vec3::new(-STREET_HW + lip, lh, z1), Vec3::new(-STREET_HW + lip, lh - 0.1, z1)],
            [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
            IMG_PIPE, [0.5, 0.5, 0.5, 1.0],
        );

        // Right rooftop lip
        b.quad(
            [Vec3::new(STREET_HW - lip, rh, z0), Vec3::new(STREET_HW, rh, z0),
             Vec3::new(STREET_HW, rh, z1), Vec3::new(STREET_HW - lip, rh, z1)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_PIPE, [0.6, 0.6, 0.6, 1.0],
        );
        b.quad(
            [Vec3::new(STREET_HW - lip, lh, z1), Vec3::new(STREET_HW - lip, rh, z1),
             Vec3::new(STREET_HW - lip, rh, z0), Vec3::new(STREET_HW - lip, lh - 0.1, z0)],
            [[0.0, 0.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
            IMG_PIPE, [0.5, 0.5, 0.5, 1.0],
        );

        // Floor-level shop step (small bump at each doorway)
        let step_h = 0.06;
        let step_d = 0.2;
        let z_mid = (z0 + z1) * 0.5;
        // Left shop step
        b.quad(
            [Vec3::new(-STREET_HW, step_h, z_mid + 1.0), Vec3::new(-STREET_HW + step_d, step_h, z_mid + 1.0),
             Vec3::new(-STREET_HW + step_d, step_h, z_mid - 1.0), Vec3::new(-STREET_HW, step_h, z_mid - 1.0)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_STREET, [1.3, 1.3, 1.3, 1.0],
        );
        // Right shop step
        b.quad(
            [Vec3::new(STREET_HW - step_d, step_h, z_mid + 1.0), Vec3::new(STREET_HW, step_h, z_mid + 1.0),
             Vec3::new(STREET_HW, step_h, z_mid - 1.0), Vec3::new(STREET_HW - step_d, step_h, z_mid - 1.0)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_STREET, [1.3, 1.3, 1.3, 1.0],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // TRASH BAGS (3D box props against walls)
    // ══════════════════════════════════════════════════════════════════
    let trash_spots: [(usize, bool); 3] = [
        (0, true),   // left gap after shop 0
        (3, false),  // right gap after shop 3
        (5, true),   // left gap after shop 5
    ];
    for &(gap_idx, left) in &trash_spots {
        let z_gap = Z_START - SHOP_GAP - (gap_idx as f32) * SHOP_STEP - SHOP_W - SHOP_GAP * 0.5;
        let wall_x = if left { -STREET_HW } else { STREET_HW };
        b.wall_prop(
            wall_x, z_gap, left,
            0.5, 0.55, 0.8,
            IMG_TRASH, [1.6, 1.5, 1.5, 1.0],
            IMG_VERY_DARK, [0.8, 0.8, 0.8, 1.0],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // BEER CRATES (3D box props against walls)
    // ══════════════════════════════════════════════════════════════════
    let crate_spots: [(usize, bool); 3] = [
        (0, false),  // right side, near izakaya
        (2, true),   // left side, near konbini gap
        (4, false),  // right side, near izakaya2
    ];
    for &(gap_idx, left) in &crate_spots {
        let z_gap = Z_START - SHOP_GAP - (gap_idx as f32) * SHOP_STEP - SHOP_W + 0.3;
        let wall_x = if left { -STREET_HW } else { STREET_HW };
        b.wall_prop(
            wall_x, z_gap, left,
            0.45, 0.5, 1.0,
            IMG_CRATES, [1.3, 1.2, 1.1, 1.0],
            IMG_WARM_ACCENT, [0.4, 0.3, 0.2, 1.0],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // NEON ARROWS (pointing down above certain shop entrances)
    // ══════════════════════════════════════════════════════════════════
    let arrow_shops = [
        (1, true),   // left shop 1 (pachinko)
        (0, false),  // right shop 0 (izakaya)
        (5, false),  // right shop 5 (shuttered)
        (3, true),   // left shop 3 (shuttered)
    ];
    for &(shop_idx, left) in &arrow_shops {
        let z_mid = Z_START - SHOP_GAP - (shop_idx as f32) * SHOP_STEP - SHOP_W * 0.5;
        let aw = 0.4;
        let ah = 0.8;
        let ay = SHOP_H - 1.2;
        if left {
            let ax = -STREET_HW + 0.05;
            b.quad(
                [Vec3::new(ax, ay, z_mid + aw * 0.5), Vec3::new(ax, ay, z_mid - aw * 0.5),
                 Vec3::new(ax, ay + ah, z_mid - aw * 0.5), Vec3::new(ax, ay + ah, z_mid + aw * 0.5)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_ARROW, [2.5, 1.5, 2.0, 1.0],
            );
        } else {
            let ax = STREET_HW - 0.05;
            b.quad(
                [Vec3::new(ax, ay, z_mid - aw * 0.5), Vec3::new(ax, ay, z_mid + aw * 0.5),
                 Vec3::new(ax, ay + ah, z_mid + aw * 0.5), Vec3::new(ax, ay + ah, z_mid - aw * 0.5)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_ARROW, [2.5, 1.5, 2.0, 1.0],
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // NOREN CURTAINS (hanging in select shop doorways)
    // ══════════════════════════════════════════════════════════════════
    let noren_shops = [
        (0, true),   // left ramen shop
        (4, true),   // left ramen2
        (0, false),  // right izakaya
    ];
    for &(shop_idx, left) in &noren_shops {
        let z_mid = Z_START - SHOP_GAP - (shop_idx as f32) * SHOP_STEP - SHOP_W * 0.5;
        let nw = 2.0;
        let nh = 1.3;
        let ny = 1.8;
        if left {
            let nx = -STREET_HW + 0.04;
            b.quad(
                [Vec3::new(nx, ny, z_mid + nw * 0.5), Vec3::new(nx, ny, z_mid - nw * 0.5),
                 Vec3::new(nx, ny + nh, z_mid - nw * 0.5), Vec3::new(nx, ny + nh, z_mid + nw * 0.5)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_NOREN, [1.2, 1.0, 1.0, 1.0],
            );
        } else {
            let nx = STREET_HW - 0.04;
            b.quad(
                [Vec3::new(nx, ny, z_mid - nw * 0.5), Vec3::new(nx, ny, z_mid + nw * 0.5),
                 Vec3::new(nx, ny + nh, z_mid + nw * 0.5), Vec3::new(nx, ny + nh, z_mid - nw * 0.5)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                IMG_NOREN, [1.2, 1.0, 1.0, 1.0],
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // BICYCLES (3D box props leaning against walls)
    // ══════════════════════════════════════════════════════════════════
    let bike_spots: [(usize, bool); 2] = [
        (2, false),  // right side, gap after arcade shop
        (1, true),   // left side, gap after pachinko
    ];
    for &(gap_idx, left) in &bike_spots {
        let z_gap = Z_START - SHOP_GAP - (gap_idx as f32) * SHOP_STEP - SHOP_W - SHOP_GAP * 0.5;
        let wall_x = if left { -STREET_HW } else { STREET_HW };
        b.wall_prop(
            wall_x, z_gap, left,
            0.65, 0.5, 0.9,
            IMG_BICYCLE, [1.3, 1.2, 1.2, 1.0],
            IMG_PIPE, [0.5, 0.5, 0.5, 1.0],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // PUDDLE REFLECTIONS (additional wet spots near vending machines)
    // ══════════════════════════════════════════════════════════════════
    for &(x, z, _) in &vm_positions {
        let pw = 0.8;
        b.quad(
            [Vec3::new(x - pw, 0.006, z + pw * 0.6), Vec3::new(x + pw, 0.006, z + pw * 0.6),
             Vec3::new(x + pw, 0.006, z - pw * 0.6), Vec3::new(x - pw, 0.006, z - pw * 0.6)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_WET_STREET, [1.4, 1.3, 1.6, 0.5],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // ADDITIONAL NEON ACCENTS (vertical strips in gaps between shops)
    // ══════════════════════════════════════════════════════════════════
    for i in 0..(SHOPS_PER_SIDE - 1) {
        let z_gap = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP - SHOP_W;
        let ny = 0.3;
        let nh = SHOP_H - 0.5;
        let nw = 0.06;
        let color = if i % 2 == 0 { IMG_NEON_PINK } else { IMG_NEON_BLUE };
        let tint = if i % 2 == 0 { [2.5, 1.5, 2.0, 0.8] } else { [1.5, 1.5, 2.5, 0.8] };

        // Left side gap neon strip
        let lx = -STREET_HW + 0.02;
        b.quad(
            [Vec3::new(lx, ny, z_gap + nw), Vec3::new(lx, ny, z_gap - nw),
             Vec3::new(lx, ny + nh, z_gap - nw), Vec3::new(lx, ny + nh, z_gap + nw)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            color, tint,
        );
        // Right side gap neon strip
        let rx = STREET_HW - 0.02;
        b.quad(
            [Vec3::new(rx, ny, z_gap - nw), Vec3::new(rx, ny, z_gap + nw),
             Vec3::new(rx, ny + nh, z_gap + nw), Vec3::new(rx, ny + nh, z_gap - nw)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            color, tint,
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // OVERHEAD CROSS-ALLEY BANNERS (cloth banners strung across the alley)
    // ══════════════════════════════════════════════════════════════════
    for i in [1, 3, 5] {
        let z_c = Z_START - SHOP_GAP - (i as f32) * SHOP_STEP - SHOP_W * 0.5;
        let banner_y = SHOP_H + UPPER_H * 0.6;
        let bh = 0.6;
        let sag = 0.15;
        // Cloth banner stretched across alley
        b.quad(
            [Vec3::new(-STREET_HW + 0.2, banner_y, z_c),
             Vec3::new(STREET_HW - 0.2, banner_y - sag, z_c),
             Vec3::new(STREET_HW - 0.2, banner_y - sag + bh, z_c),
             Vec3::new(-STREET_HW + 0.2, banner_y + bh, z_c)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            IMG_WARM_ACCENT, [1.5, 0.8, 0.4, 0.9],
        );
        // Back face
        b.quad(
            [Vec3::new(STREET_HW - 0.2, banner_y - sag, z_c),
             Vec3::new(-STREET_HW + 0.2, banner_y, z_c),
             Vec3::new(-STREET_HW + 0.2, banner_y + bh, z_c),
             Vec3::new(STREET_HW - 0.2, banner_y - sag + bh, z_c)],
            [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
            IMG_WARM_ACCENT, [1.5, 0.8, 0.4, 0.9],
        );
    }

    // ══════════════════════════════════════════════════════════════════
    // GROUND STAINS / UTILITY MARKINGS (painted lines on street)
    // ══════════════════════════════════════════════════════════════════
    for i in 0..4 {
        let z_c = Z_START - 5.0 - (i as f32) * 8.0;
        // Yellow utility markings (thin lines)
        b.quad(
            [Vec3::new(-STREET_HW + 0.5, 0.004, z_c + 0.6), Vec3::new(-STREET_HW + 0.5 + 0.08, 0.004, z_c + 0.6),
             Vec3::new(-STREET_HW + 0.5 + 0.08, 0.004, z_c - 0.6), Vec3::new(-STREET_HW + 0.5, 0.004, z_c - 0.6)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            IMG_LANTERN, [0.8, 0.7, 0.3, 0.6],
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

    // Ground prop collision (trash bags, crates, bikes — box-shaped against walls)
    for &(gap_idx, left) in &trash_spots {
        let z_gap = Z_START - SHOP_GAP - (gap_idx as f32) * SHOP_STEP - SHOP_W - SHOP_GAP * 0.5;
        let (xmin, xmax) = if left { (-STREET_HW, -STREET_HW + 0.55) } else { (STREET_HW - 0.55, STREET_HW) };
        solids.push(Aabb { min: Vec3::new(xmin, 0.0, z_gap - 0.5), max: Vec3::new(xmax, 0.7, z_gap + 0.5) });
    }
    for &(gap_idx, left) in &crate_spots {
        let z_gap = Z_START - SHOP_GAP - (gap_idx as f32) * SHOP_STEP - SHOP_W + 0.3;
        let (xmin, xmax) = if left { (-STREET_HW, -STREET_HW + 0.5) } else { (STREET_HW - 0.5, STREET_HW) };
        solids.push(Aabb { min: Vec3::new(xmin, 0.0, z_gap - 0.45), max: Vec3::new(xmax, 0.9, z_gap + 0.45) });
    }
    for &(gap_idx, left) in &bike_spots {
        let z_gap = Z_START - SHOP_GAP - (gap_idx as f32) * SHOP_STEP - SHOP_W - SHOP_GAP * 0.5;
        let (xmin, xmax) = if left { (-STREET_HW, -STREET_HW + 0.5) } else { (STREET_HW - 0.5, STREET_HW) };
        solids.push(Aabb { min: Vec3::new(xmin, 0.0, z_gap - 0.65), max: Vec3::new(xmax, 0.8, z_gap + 0.65) });
    }

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

    /// Place a 3D box-shaped prop against a wall. Builds 5 visible faces of a
    /// box: the **wall face** (textured, against the building), **top** face,
    /// **front** face (facing the player in the street), and two **side** faces.
    /// `wall_x` is the wall plane. `left_side` = true means the wall is at -X
    /// (prop extends into +X). `z_mid` is the centre along Z. `hz` is half-width
    /// along Z, `depth` is how far it sticks out from the wall, `h` is height.
    fn wall_prop(
        &mut self,
        wall_x: f32, z_mid: f32, left_side: bool,
        hz: f32, depth: f32, h: f32,
        image_index: usize, tint: [f32; 4],
        side_img: usize, side_tint: [f32; 4],
    ) {
        let z0 = z_mid + hz;
        let z1 = z_mid - hz;

        let (back_x, front_x) = if left_side {
            (wall_x, wall_x + depth)
        } else {
            (wall_x, wall_x - depth)
        };

        // Wall face (textured image, flat against building)
        if left_side {
            self.quad(
                [Vec3::new(back_x, 0.0, z0), Vec3::new(back_x, 0.0, z1),
                 Vec3::new(back_x, h, z1), Vec3::new(back_x, h, z0)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                image_index, tint,
            );
        } else {
            self.quad(
                [Vec3::new(back_x, 0.0, z1), Vec3::new(back_x, 0.0, z0),
                 Vec3::new(back_x, h, z0), Vec3::new(back_x, h, z1)],
                [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                image_index, tint,
            );
        }

        // Front face (facing the player, same texture but darker)
        let front_tint = [tint[0] * 0.7, tint[1] * 0.7, tint[2] * 0.7, tint[3]];
        if left_side {
            self.quad(
                [Vec3::new(front_x, 0.0, z1), Vec3::new(front_x, 0.0, z0),
                 Vec3::new(front_x, h, z0), Vec3::new(front_x, h, z1)],
                [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
                image_index, front_tint,
            );
        } else {
            self.quad(
                [Vec3::new(front_x, 0.0, z0), Vec3::new(front_x, 0.0, z1),
                 Vec3::new(front_x, h, z1), Vec3::new(front_x, h, z0)],
                [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
                image_index, front_tint,
            );
        }

        // Top face
        let (x_min, x_max) = if left_side { (back_x, front_x) } else { (front_x, back_x) };
        self.quad(
            [Vec3::new(x_min, h, z0), Vec3::new(x_max, h, z0),
             Vec3::new(x_max, h, z1), Vec3::new(x_min, h, z1)],
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            side_img, side_tint,
        );

        // Side faces (z0 and z1)
        self.quad(
            [Vec3::new(x_min, 0.0, z0), Vec3::new(x_max, 0.0, z0),
             Vec3::new(x_max, h, z0), Vec3::new(x_min, h, z0)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            side_img, side_tint,
        );
        self.quad(
            [Vec3::new(x_max, 0.0, z1), Vec3::new(x_min, 0.0, z1),
             Vec3::new(x_min, h, z1), Vec3::new(x_max, h, z1)],
            [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            side_img, side_tint,
        );
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
