//! Extra **shop-front** quads merged into the loaded glTF so the alley reads as a Tokyo side street
//! without hand-editing the `.glb` for every iteration. Uses one procedural RGBA atlas (16 stylized
//! sign cells). No extra collision — visuals only.

use crate::gltf_level::{GltfBatchCpu, GltfLevelCpu, WorldVertex};

const ATLAS_COLS: u32 = 4;
const ATLAS_ROWS: u32 = 4;
const ATLAS_W: u32 = 512;
const ATLAS_H: u32 = 512;

pub fn append_tokyo_facades(cpu: &mut GltfLevelCpu) {
    let b = cpu.bounds();
    let span_x = (b.max.x - b.min.x).max(4.0);
    let span_z = (b.max.z - b.min.z).max(4.0);
    if span_x > 800.0 || span_z > 800.0 {
        return;
    }

    let y_floor = b.min.y + 0.04;
    let y_top = (b.min.y + 4.35).min(b.max.y - 0.08).max(y_floor + 2.0);
    let y_awning = (y_floor + 2.55).min(y_top - 0.35);

    let img_idx = cpu.images_rgba8.len();
    cpu.images_rgba8.push(build_sign_atlas());

    let first_new_index = cpu.indices.len() as u32;
    let mut new_tri_count: u32 = 0;

    let xl = b.min.x + 0.06;
    let xr = b.max.x - 0.06;
    let z_back = b.min.z + 0.08;
    let z_front = b.max.z - 0.08;

    let mut sid: u32 = 0;
    let mut z = b.min.z + 0.55;
    while z + 0.75 < b.max.z - 0.45 {
        let z1 = (z + 1.55_f32).min(b.max.z - 0.42);
        push_quad_vertical_x(
            cpu,
            y_floor,
            y_top,
            &mut new_tri_count,
            xl,
            z,
            z1,
            false,
            sid.wrapping_mul(31),
        );
        push_quad_vertical_x(
            cpu,
            y_floor,
            y_top,
            &mut new_tri_count,
            xr,
            z,
            z1,
            true,
            sid.wrapping_mul(17).wrapping_add(3),
        );
        let strip_w = (z1 - z).min(2.1);
        let xm0 = xl + 0.12;
        let xm1 = (xm0 + strip_w).min(xr - 0.35);
        push_awning_strip(
            cpu,
            y_awning,
            &mut new_tri_count,
            xm0,
            xm1,
            z + (z1 - z) * 0.5,
            sid,
        );
        let xm0r = (xr - 0.12 - strip_w).max(xl + 0.35);
        let xm1r = xr - 0.12;
        push_awning_strip(
            cpu,
            y_awning,
            &mut new_tri_count,
            xm0r,
            xm1r,
            z + (z1 - z) * 0.5,
            sid.wrapping_add(11),
        );
        let zm = z + (z1 - z) * 0.5;
        let znarrow0 = (zm - 0.22).max(z + 0.08);
        let znarrow1 = (zm + 0.22).min(z1 - 0.08);
        if znarrow1 > znarrow0 + 0.08 {
            let y_narrow_top = y_floor + (y_top - y_floor) * 0.58;
            push_quad_vertical_x(
                cpu,
                y_floor,
                y_narrow_top,
                &mut new_tri_count,
                xl + 0.04,
                znarrow0,
                znarrow1,
                false,
                sid.wrapping_mul(41).wrapping_add(19),
            );
            push_quad_vertical_x(
                cpu,
                y_floor,
                y_narrow_top,
                &mut new_tri_count,
                xr - 0.04,
                znarrow0,
                znarrow1,
                true,
                sid.wrapping_mul(43).wrapping_add(7),
            );
        }
        z = z1 + 0.06;
        sid = sid.wrapping_add(1);
    }

    let x_step = (span_x * 0.42 / 8.0_f32).clamp(1.35, 2.6);
    let mut x = b.min.x + 1.0;
    let mut bid: u32 = 0;
    while x + x_step * 0.85 < b.max.x - 1.0 {
        let x1 = (x + x_step).min(b.max.x - 0.85);
        push_quad_vertical_z(
            cpu,
            y_floor,
            y_top,
            &mut new_tri_count,
            z_back,
            x,
            x1,
            false,
            bid.wrapping_mul(13).wrapping_add(5),
        );
        push_quad_vertical_z(
            cpu,
            y_floor,
            y_top,
            &mut new_tri_count,
            z_front,
            x,
            x1,
            true,
            bid.wrapping_mul(19).wrapping_add(2),
        );
        x = x1 + 0.15;
        bid = bid.wrapping_add(1);
    }

    let mid_x = (b.min.x + b.max.x) * 0.5;
    for (i, dz) in [-1.2_f32, 0.0, 1.2].iter().enumerate() {
        let zz = mid_x * 0.0 + (b.min.z + b.max.z) * 0.5 + dz + span_z * 0.12;
        if zz > b.min.z + 0.5 && zz < b.max.z - 0.5 {
            let w = 0.28_f32;
            let seed = 100_u32 + i as u32;
            let (u0, u1, v0, v1) = cell_uv(seed);
            let h = (y_top - y_floor) * 0.22;
            let yb = y_floor + 0.05;
            let yt = yb + h;
            let base = cpu.vertices.len() as u32;
            cpu.vertices.push(WorldVertex {
                pos: [mid_x - w, yb, zz],
                uv: [u0, v1],
            });
            cpu.vertices.push(WorldVertex {
                pos: [mid_x + w, yb, zz],
                uv: [u1, v1],
            });
            cpu.vertices.push(WorldVertex {
                pos: [mid_x + w, yt, zz],
                uv: [u1, v0],
            });
            cpu.vertices.push(WorldVertex {
                pos: [mid_x - w, yt, zz],
                uv: [u0, v0],
            });
            cpu.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            new_tri_count += 6;
        }
    }

    if new_tri_count > 0 {
        cpu.batches.push(GltfBatchCpu {
            first_index: first_new_index,
            index_count: new_tri_count,
            image_index: img_idx,
            tint: [1.0, 1.0, 1.0, 1.0],
        });
    }
}

fn push_quad_vertical_x(
    cpu: &mut GltfLevelCpu,
    y_floor: f32,
    y_top: f32,
    tri_count: &mut u32,
    x: f32,
    z0: f32,
    z1: f32,
    flip_u: bool,
    seed: u32,
) {
    let (u0, u1, v0, v1) = cell_uv(seed);
    let (u0, u1) = if flip_u { (u1, u0) } else { (u0, u1) };
    let base = cpu.vertices.len() as u32;
    cpu.vertices.push(WorldVertex {
        pos: [x, y_floor, z0],
        uv: [u0, v1],
    });
    cpu.vertices.push(WorldVertex {
        pos: [x, y_floor, z1],
        uv: [u1, v1],
    });
    cpu.vertices.push(WorldVertex {
        pos: [x, y_top, z1],
        uv: [u1, v0],
    });
    cpu.vertices.push(WorldVertex {
        pos: [x, y_top, z0],
        uv: [u0, v0],
    });
    cpu.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    *tri_count += 6;
}

fn push_quad_vertical_z(
    cpu: &mut GltfLevelCpu,
    y_floor: f32,
    y_top: f32,
    tri_count: &mut u32,
    z: f32,
    x0: f32,
    x1: f32,
    flip_u: bool,
    seed: u32,
) {
    let (u0, u1, v0, v1) = cell_uv(seed);
    let (u0, u1) = if flip_u { (u1, u0) } else { (u0, u1) };
    let base = cpu.vertices.len() as u32;
    cpu.vertices.push(WorldVertex {
        pos: [x0, y_floor, z],
        uv: [u0, v1],
    });
    cpu.vertices.push(WorldVertex {
        pos: [x1, y_floor, z],
        uv: [u1, v1],
    });
    cpu.vertices.push(WorldVertex {
        pos: [x1, y_top, z],
        uv: [u1, v0],
    });
    cpu.vertices.push(WorldVertex {
        pos: [x0, y_top, z],
        uv: [u0, v0],
    });
    cpu.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    *tri_count += 6;
}

fn push_awning_strip(
    cpu: &mut GltfLevelCpu,
    y_awning: f32,
    tri_count: &mut u32,
    x0: f32,
    x1: f32,
    z: f32,
    seed: u32,
) {
    let t0 = y_awning;
    let t1 = y_awning + 0.22;
    let (u0, u1, v0, v1) = cell_uv(seed | 7);
    let base = cpu.vertices.len() as u32;
    cpu.vertices.push(WorldVertex {
        pos: [x0, t0, z],
        uv: [u0, v1],
    });
    cpu.vertices.push(WorldVertex {
        pos: [x1, t0, z],
        uv: [u1, v1],
    });
    cpu.vertices.push(WorldVertex {
        pos: [x1, t1, z],
        uv: [u1, v0],
    });
    cpu.vertices.push(WorldVertex {
        pos: [x0, t1, z],
        uv: [u0, v0],
    });
    cpu.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    *tri_count += 6;
}

fn cell_uv(seed: u32) -> (f32, f32, f32, f32) {
    let cell = seed % 16;
    let col = cell % ATLAS_COLS;
    let row = cell / ATLAS_COLS;
    let du = 1.0 / ATLAS_COLS as f32;
    let dv = 1.0 / ATLAS_ROWS as f32;
    let u0 = col as f32 * du + 0.002;
    let u1 = (col + 1) as f32 * du - 0.002;
    let v0 = row as f32 * dv + 0.002;
    let v1 = (row + 1) as f32 * dv - 0.002;
    (u0, u1, v0, v1)
}

fn build_sign_atlas() -> (u32, u32, Vec<u8>) {
    let w = ATLAS_W as usize;
    let h = ATLAS_H as usize;
    let mut rgba = vec![0u8; w * h * 4];
    let cw = (ATLAS_W / ATLAS_COLS) as usize;
    let ch = (ATLAS_H / ATLAS_ROWS) as usize;
    for row in 0..ATLAS_ROWS {
        for col in 0..ATLAS_COLS {
            let seed = row * ATLAS_COLS + col;
            fill_cell(&mut rgba, w, cw, ch, col as usize * cw, row as usize * ch, seed);
        }
    }
    (ATLAS_W, ATLAS_H, rgba)
}

fn fill_cell(out: &mut [u8], stride: usize, cw: usize, ch: usize, ox: usize, oy: usize, seed: u32) {
    let img_h = out.len() / (stride * 4);
    let palettes: &[[u8; 3]; 6] = &[
        [12, 14, 28],
        [28, 8, 14],
        [8, 18, 22],
        [18, 10, 8],
        [22, 6, 6],
        [10, 12, 18],
    ];
    let bg = palettes[(seed as usize) % palettes.len()];
    let accent = palettes[((seed >> 2) as usize + 1) % palettes.len()];
    let neon = [255_u8, 235, 200];
    let paper = [248_u8, 244, 238];
    for yy in 0..ch {
        for xx in 0..cw {
            let px = ox + xx;
            let py = oy + yy;
            if px >= stride || py >= img_h {
                continue;
            }
            let i = (py * stride + px) * 4;
            let fx = xx as f32 / cw as f32;
            let fy = yy as f32 / ch as f32;
            let border = fx < 0.06 || fx > 0.94 || fy < 0.05 || fy > 0.95;
            let mut r = bg[0];
            let mut g = bg[1];
            let mut b = bg[2];
            if border {
                r = accent[0];
                g = accent[1];
                b = accent[2];
            }
            let band = (fy > 0.38 && fy < 0.62) && (fx > 0.12 && fx < 0.88);
            if band {
                r = paper[0];
                g = paper[1];
                b = paper[2];
            }
            let glyph = (fx > 0.22 && fx < 0.78) && ((fy > 0.18 && fy < 0.34) || (fy > 0.66 && fy < 0.82));
            if glyph {
                r = r.saturating_sub(40);
                g = g.saturating_sub(40);
                b = b.saturating_sub(30);
            }
            let vline = (fx > 0.48 && fx < 0.52) && fy > 0.2 && fy < 0.85;
            if vline && seed % 3 == 0 {
                r = neon[0];
                g = neon[1];
                b = neon[2];
            }
            let hbar = (fy > 0.48 && fy < 0.52) && fx > 0.15 && fx < 0.85;
            if hbar && seed % 2 == 0 {
                r = neon[0].saturating_sub(40);
                g = neon[1].saturating_sub(80);
                b = neon[2].saturating_sub(120);
            }
            if seed % 5 == 0 {
                let cx = 0.5_f32;
                let cy = 0.42_f32;
                let rr = 0.28_f32;
                let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
                if d < rr {
                    r = r.saturating_add(120);
                    g = g.saturating_sub(30);
                    b = b.saturating_sub(30);
                }
            }
            out[i] = r;
            out[i + 1] = g;
            out[i + 2] = b;
            out[i + 3] = 255;
        }
    }
}
