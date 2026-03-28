use glam::Vec3;

use crate::render::Vertex;

#[derive(serde::Deserialize)]
struct LevelJson {
    spawn: [f32; 3],
    vertices: Vec<f32>,
    indices: Vec<u32>,
    solids: Vec<JsonAabb>,
    #[serde(default)]
    boss_foot: Option<[f32; 3]>,
    #[serde(default)]
    rival_foot: Option<[f32; 3]>,
    #[serde(default)]
    spawn_yaw: Option<f32>,
}

#[derive(serde::Deserialize)]
struct JsonAabb {
    min: [f32; 3],
    max: [f32; 3],
}

pub struct LevelBoot {
    pub arena: Arena,
    pub spawn: Vec3,
    pub boss_foot: Vec3,
    pub rival_foot: Vec3,
    pub spawn_yaw: f32,
    pub level_bounds: Aabb,
    /// World Z of reference mural quad (far end of alley).
    pub mural_z: f32,
}

pub fn vertex_bounds(arena: &Arena) -> Aabb {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for v in &arena.vertices {
        let p = Vec3::from_array(v.pos);
        min = min.min(p);
        max = max.max(p);
    }
    if min.x > max.x {
        return Aabb {
            min: Vec3::new(-18.0, 0.0, -18.0),
            max: Vec3::new(18.0, 8.0, 18.0),
        };
    }
    Aabb { min, max }
}

fn default_spawn_yaw(bounds: &Aabb, spawn: Vec3) -> f32 {
    let cx = (bounds.min.x + bounds.max.x) * 0.5;
    let span_z = (bounds.max.z - bounds.min.z).max(0.5);
    let toward_min = spawn.z - bounds.min.z;
    let toward_max = bounds.max.z - spawn.z;
    let tz = if toward_min > toward_max {
        bounds.min.z + span_z * 0.18
    } else {
        bounds.max.z - span_z * 0.18
    };
    let dx = cx - spawn.x;
    let dz = tz - spawn.z;
    let len_sq = dx * dx + dz * dz;
    if len_sq < 1e-4 {
        return 0.0;
    }
    dx.atan2(-dz)
}

pub fn mural_z_plane(bounds: &Aabb, spawn: Vec3) -> f32 {
    let toward_min = spawn.z - bounds.min.z;
    let toward_max = bounds.max.z - spawn.z;
    if toward_min > toward_max {
        bounds.min.z - 1.15
    } else {
        bounds.max.z + 1.15
    }
}

pub fn npc_placements(spawn: Vec3, yaw: f32) -> (Vec3, Vec3) {
    let sx = yaw.sin();
    let sz = yaw.cos();
    let fwd = Vec3::new(sx, 0.0, -sz);
    let right = Vec3::new(-sz, 0.0, -sx);
    let boss = spawn + fwd * 11.0 + right * 1.8;
    let rival = spawn + fwd * 17.0 - right * 2.4;
    (boss, rival)
}

/// Blender export: `tools/blender_export_oyabaun.py` → `client/levels/tokyo_street.json` (Y-up game space).
pub fn arena_from_level_json(s: &str) -> Result<LevelBoot, String> {
    let j: LevelJson = serde_json::from_str(s).map_err(|e| e.to_string())?;
    let spawn = Vec3::from_array(j.spawn);
    if j.vertices.len() % 6 != 0 || j.indices.len() % 3 != 0 {
        return Err("invalid vertices/indices length".into());
    }
    let mut vertices = Vec::with_capacity(j.vertices.len() / 6);
    for chunk in j.vertices.chunks_exact(6) {
        vertices.push(Vertex::new(
            Vec3::new(chunk[0], chunk[1], chunk[2]),
            Vec3::new(chunk[3], chunk[4], chunk[5]),
        ));
    }
    let solids = j
        .solids
        .into_iter()
        .map(|a| Aabb {
            min: Vec3::from_array(a.min),
            max: Vec3::from_array(a.max),
        })
        .collect();
    let arena = Arena {
        vertices,
        indices: j.indices,
        solids,
    };
    let level_bounds = vertex_bounds(&arena);
    let spawn_yaw = j
        .spawn_yaw
        .unwrap_or_else(|| default_spawn_yaw(&level_bounds, spawn));
    let (auto_boss, auto_rival) = npc_placements(spawn, spawn_yaw);
    let boss_foot = j.boss_foot.map(Vec3::from_array).unwrap_or(auto_boss);
    let rival_foot = j.rival_foot.map(Vec3::from_array).unwrap_or(auto_rival);
    let mural_z = mural_z_plane(&level_bounds, spawn);
    Ok(LevelBoot {
        arena,
        spawn,
        boss_foot,
        rival_foot,
        spawn_yaw,
        level_bounds,
        mural_z,
    })
}


#[derive(Clone)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

pub struct Arena {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub solids: Vec<Aabb>,
}

pub fn empty_arena() -> Arena {
    Arena {
        vertices: Vec::new(),
        indices: Vec::new(),
        solids: Vec::new(),
    }
}

fn push_quad(
    verts: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    a: Vec3,
    b: Vec3,
    c: Vec3,
    d: Vec3,
    color: Vec3,
) {
    let base = verts.len() as u32;
    verts.push(Vertex::new(a, color));
    verts.push(Vertex::new(b, color));
    verts.push(Vertex::new(c, color));
    verts.push(Vertex::new(d, color));
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn push_box(verts: &mut Vec<Vertex>, indices: &mut Vec<u32>, min: Vec3, max: Vec3, color: Vec3) {
    let x0 = min.x;
    let x1 = max.x;
    let y0 = min.y;
    let y1 = max.y;
    let z0 = min.z;
    let z1 = max.z;
    let c_top = color * 1.08;
    let c_side = color * 0.85;
    let c_bot = color * 0.55;
    push_quad(
        verts,
        indices,
        Vec3::new(x0, y1, z0),
        Vec3::new(x1, y1, z0),
        Vec3::new(x1, y1, z1),
        Vec3::new(x0, y1, z1),
        c_top,
    );
    push_quad(
        verts,
        indices,
        Vec3::new(x0, y0, z1),
        Vec3::new(x1, y0, z1),
        Vec3::new(x1, y0, z0),
        Vec3::new(x0, y0, z0),
        c_bot,
    );
    push_quad(
        verts,
        indices,
        Vec3::new(x0, y0, z1),
        Vec3::new(x0, y1, z1),
        Vec3::new(x0, y1, z0),
        Vec3::new(x0, y0, z0),
        c_side,
    );
    push_quad(
        verts,
        indices,
        Vec3::new(x1, y0, z0),
        Vec3::new(x1, y1, z0),
        Vec3::new(x1, y1, z1),
        Vec3::new(x1, y0, z1),
        c_side,
    );
    push_quad(
        verts,
        indices,
        Vec3::new(x0, y0, z0),
        Vec3::new(x0, y1, z0),
        Vec3::new(x1, y1, z0),
        Vec3::new(x1, y0, z0),
        c_side,
    );
    push_quad(
        verts,
        indices,
        Vec3::new(x1, y0, z1),
        Vec3::new(x1, y1, z1),
        Vec3::new(x0, y1, z1),
        Vec3::new(x0, y0, z1),
        c_side,
    );
}

pub fn build_arena() -> Arena {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut solids = Vec::new();

    let half = 14.0_f32;
    let wall_h = 5.0_f32;
    let thick = 0.55_f32;

    let floor_a = Vec3::new(0.1, 0.075, 0.15);
    let floor_b = Vec3::new(0.065, 0.055, 0.12);
    let cells: i32 = 10;
    let step = (half * 2.0) / cells as f32;
    for i in 0..cells {
        for j in 0..cells {
            let x0 = -half + i as f32 * step;
            let x1 = x0 + step;
            let z0 = -half + j as f32 * step;
            let z1 = z0 + step;
            let c = if (i + j) % 2 == 0 { floor_a } else { floor_b };
            push_quad(
                &mut vertices,
                &mut indices,
                Vec3::new(x0, 0.0, z0),
                Vec3::new(x1, 0.0, z0),
                Vec3::new(x1, 0.0, z1),
                Vec3::new(x0, 0.0, z1),
                c,
            );
        }
    }

    let wall_color = Vec3::new(0.13, 0.055, 0.095);
    let accent = Vec3::new(0.48, 0.11, 0.14);
    let torii_red = Vec3::new(0.68, 0.1, 0.12);
    let neon_pink = Vec3::new(0.58, 0.14, 0.32);
    let lantern = Vec3::new(0.92, 0.38, 0.22);

    push_box(
        &mut vertices,
        &mut indices,
        Vec3::new(-half - thick, 0.0, -half - thick),
        Vec3::new(half + thick, wall_h, -half),
        wall_color,
    );
    solids.push(Aabb {
        min: Vec3::new(-half - thick, 0.0, -half - thick),
        max: Vec3::new(half + thick, wall_h, -half),
    });

    push_box(
        &mut vertices,
        &mut indices,
        Vec3::new(-half - thick, 0.0, half),
        Vec3::new(half + thick, wall_h, half + thick),
        wall_color,
    );
    solids.push(Aabb {
        min: Vec3::new(-half - thick, 0.0, half),
        max: Vec3::new(half + thick, wall_h, half + thick),
    });

    push_box(
        &mut vertices,
        &mut indices,
        Vec3::new(-half - thick, 0.0, -half),
        Vec3::new(-half, wall_h, half),
        wall_color,
    );
    solids.push(Aabb {
        min: Vec3::new(-half - thick, 0.0, -half),
        max: Vec3::new(-half, wall_h, half),
    });

    push_box(
        &mut vertices,
        &mut indices,
        Vec3::new(half, 0.0, -half),
        Vec3::new(half + thick, wall_h, half),
        wall_color,
    );
    solids.push(Aabb {
        min: Vec3::new(half, 0.0, -half),
        max: Vec3::new(half + thick, wall_h, half),
    });

    let pillars: &[(f32, f32, f32, f32)] = &[
        (-6.0, -6.0, 1.1, 1.1),
        (6.0, -6.0, 1.1, 1.1),
        (-6.0, 6.0, 1.1, 1.1),
        (6.0, 6.0, 1.1, 1.1),
        (0.0, 0.0, 2.5, 2.5),
    ];
    for &(cx, cz, w, d) in pillars {
        let hx = w * 0.5;
        let hz = d * 0.5;
        let min = Vec3::new(cx - hx, 0.0, cz - hz);
        let max = Vec3::new(cx + hx, wall_h * 0.85, cz + hz);
        push_box(&mut vertices, &mut indices, min, max, accent);
        solids.push(Aabb { min, max });
    }

    let tz = -half + 1.2;
    push_box(
        &mut vertices,
        &mut indices,
        Vec3::new(-2.15, 0.0, tz - 0.12),
        Vec3::new(-1.75, 4.2, tz + 0.12),
        torii_red,
    );
    solids.push(Aabb {
        min: Vec3::new(-2.15, 0.0, tz - 0.12),
        max: Vec3::new(-1.75, 4.2, tz + 0.12),
    });
    push_box(
        &mut vertices,
        &mut indices,
        Vec3::new(1.75, 0.0, tz - 0.12),
        Vec3::new(2.15, 4.2, tz + 0.12),
        torii_red,
    );
    solids.push(Aabb {
        min: Vec3::new(1.75, 0.0, tz - 0.12),
        max: Vec3::new(2.15, 4.2, tz + 0.12),
    });
    push_box(
        &mut vertices,
        &mut indices,
        Vec3::new(-2.4, 3.85, tz - 0.18),
        Vec3::new(2.4, 4.15, tz + 0.18),
        torii_red,
    );
    solids.push(Aabb {
        min: Vec3::new(-2.4, 3.85, tz - 0.18),
        max: Vec3::new(2.4, 4.15, tz + 0.18),
    });
    push_box(
        &mut vertices,
        &mut indices,
        Vec3::new(-0.22, 4.05, tz - 0.22),
        Vec3::new(0.22, 4.45, tz + 0.22),
        torii_red,
    );
    solids.push(Aabb {
        min: Vec3::new(-0.22, 4.05, tz - 0.22),
        max: Vec3::new(0.22, 4.45, tz + 0.22),
    });

    for &(lx, lz) in &[
        (-10.0_f32, -10.0_f32),
        (10.0, -10.0),
        (-10.0, 10.0),
        (10.0, 10.0),
        (-4.0, -11.5),
        (4.0, -11.5),
    ] {
        push_box(
            &mut vertices,
            &mut indices,
            Vec3::new(lx - 0.08, 0.0, lz - 0.08),
            Vec3::new(lx + 0.08, 3.2, lz + 0.08),
            Vec3::new(0.06, 0.06, 0.07),
        );
        push_box(
            &mut vertices,
            &mut indices,
            Vec3::new(lx - 0.35, 2.9, lz - 0.35),
            Vec3::new(lx + 0.35, 3.5, lz + 0.35),
            lantern,
        );
        solids.push(Aabb {
            min: Vec3::new(lx - 0.08, 0.0, lz - 0.08),
            max: Vec3::new(lx + 0.08, 3.2, lz + 0.08),
        });
    }

    for i in -6..=6 {
        let gx = i as f32 * 2.0;
        push_box(
            &mut vertices,
            &mut indices,
            Vec3::new(gx - 0.04, 0.02, -half + 0.08),
            Vec3::new(gx + 0.04, 0.12, -half + 0.2),
            neon_pink,
        );
    }

    Arena {
        vertices,
        indices,
        solids,
    }
}
