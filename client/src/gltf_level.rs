//! Load Blender-exported `.glb` (glTF 2.0). Blender's exporter emits **Y-up** space matching this client.
//!
//! **Spawn**: Empty named `OyabaunSpawn` or `PlayerSpawn` (case-insensitive match on `spawn` + `player`/`oyabaun`).
//! If missing, spawn is placed near **min-Z** on the map XZ span (alley mouth), not the AABB center — long levels often have empty space at center-Z.
//! **Collision**: Mesh on a node whose name contains `Collider` or `OyabaunCollision` (case-insensitive).

use glam::{Mat4, Quat, Vec3, Vec4};

use crate::mesh::Aabb;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WorldVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
}

impl WorldVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<WorldVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// NPC / playable body mesh: positions are **pre-baked world** from the glTF scene; normals are
/// world-space after node transform (used for lighting in `SHADER_CHAR_TEX`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CharacterVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub nrm: [f32; 3],
}

impl CharacterVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CharacterVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 20,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub struct CharacterMeshCpu {
    pub vertices: Vec<CharacterVertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<GltfBatchCpu>,
    pub images_rgba8: Vec<(u32, u32, Vec<u8>)>,
}

fn vertex_normals_local(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut acc = vec![Vec3::ZERO; positions.len()];
    for tri in indices.chunks_exact(3) {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;
        if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
            continue;
        }
        let e1 = positions[i1] - positions[i0];
        let e2 = positions[i2] - positions[i0];
        let face_n = e1.cross(e2);
        if face_n.length_squared() < 1e-20 {
            continue;
        }
        let face_n = face_n.normalize();
        acc[i0] += face_n;
        acc[i1] += face_n;
        acc[i2] += face_n;
    }
    acc.into_iter()
        .map(|v| {
            if v.length_squared() > 1e-12 {
                v.normalize()
            } else {
                Vec3::Z
            }
        })
        .collect()
}

fn transform_normal(world: Mat4, n_local: Vec3) -> Vec3 {
    let v = world * Vec4::new(n_local.x, n_local.y, n_local.z, 0.0);
    let t = v.truncate();
    if t.length_squared() > 1e-12 {
        t.normalize()
    } else {
        Vec3::Z
    }
}

pub struct GltfBatchCpu {
    pub first_index: u32,
    pub index_count: u32,
    pub image_index: usize,
    pub tint: [f32; 4],
}

pub struct GltfLevelCpu {
    pub vertices: Vec<WorldVertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<GltfBatchCpu>,
    pub images_rgba8: Vec<(u32, u32, Vec<u8>)>,
    pub spawn: Vec3,
    pub spawn_yaw: f32,
    pub solids: Vec<Aabb>,
}

impl GltfLevelCpu {
    pub fn bounds(&self) -> Aabb {
        vertex_bounds_from_verts(&self.vertices)
    }
}

fn mat_from_transform(t: gltf::scene::Transform) -> Mat4 {
    let (tr, rot, sc) = t.decomposed();
    let translation = Vec3::from_array(tr);
    let scale = Vec3::from_array(sc);
    let q = Quat::from_xyzw(rot[0], rot[1], rot[2], rot[3]);
    Mat4::from_scale_rotation_translation(scale, q, translation)
}

/// glTF primitives that reference `baseColorTexture` but omit TEXCOORD_* (e.g. bmesh cubes
/// from Blender scripts) would otherwise get UV (0,0) for every vertex — one texel, often
/// near-white after sRGB decode. Derive repeating UVs from world position instead.
fn world_space_fallback_uv(world_pos: Vec3) -> [f32; 2] {
    const SX: f32 = 0.38;
    const SY: f32 = 0.22;
    [
        (world_pos.x * SX + world_pos.z * SX * 0.65).rem_euclid(1.0),
        (world_pos.y * SY + world_pos.z * SY * 0.55).rem_euclid(1.0),
    ]
}

fn is_spawn_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "oyabaunspawn"
        || n == "playerspawn"
        || n.contains("oyabaun_spawn")
        || n.contains("player_spawn")
}

fn is_collision_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("collider") || n.contains("oyabauncollision")
}

fn vertex_bounds_from_verts(vertices: &[WorldVertex]) -> Aabb {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for v in vertices {
        let p = Vec3::from_array(v.pos);
        min = min.min(p);
        max = max.max(p);
    }
    if min.x > max.x {
        return Aabb {
            min: Vec3::new(-8.0, 0.0, -8.0),
            max: Vec3::new(8.0, 6.0, 8.0),
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

pub fn parse_glb(bytes: &[u8]) -> Result<GltfLevelCpu, String> {
    let (document, buffers, images) =
        gltf::import_slice(bytes).map_err(|e| format!("gltf import: {e}"))?;

    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or_else(|| "glTF has no scenes".to_string())?;

    let mut vertices: Vec<WorldVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut batches: Vec<GltfBatchCpu> = Vec::new();
    let mut spawn: Option<(Vec3, Mat4)> = None;
    let mut collision_boxes: Vec<Aabb> = Vec::new();

    let mut images_rgba8: Vec<(u32, u32, Vec<u8>)> = Vec::with_capacity(images.len());
    for img in &images {
        let rgba = image_data_to_rgba(img)?;
        images_rgba8.push((img.width, img.height, rgba));
    }

    for root in scene.nodes() {
        visit_node(
            root,
            Mat4::IDENTITY,
            &buffers,
            &document,
            &mut vertices,
            &mut indices,
            &mut batches,
            &mut spawn,
            &mut collision_boxes,
        );
    }

    let bounds = vertex_bounds_from_verts(&vertices);
    let (spawn_pt, spawn_yaw) = if let Some((pos, world)) = spawn {
        // Extract yaw from spawn node's world transform: forward is -Z in glTF,
        // so the spawn's local -Z transformed gives the facing direction.
        let fwd = world.transform_vector3(Vec3::new(0.0, 0.0, -1.0));
        let yaw = fwd.x.atan2(-fwd.z);
        (pos, yaw)
    } else {
        let cx = (bounds.min.x + bounds.max.x) * 0.5;
        let span_z = (bounds.max.z - bounds.min.z).max(1.0);
        let z = bounds.min.z + span_z * 0.14;
        let pt = Vec3::new(cx, bounds.min.y + 0.08, z);
        (pt, default_spawn_yaw(&bounds, pt))
    };

    // When no explicit collision meshes exist, create a walkable floor
    // slab instead of the full bounds AABB.  Using the full bounds would
    // make the entire level interior a collision solid and the movement
    // code would push the player outside the map.
    let solids = if collision_boxes.is_empty() {
        vec![Aabb {
            min: Vec3::new(bounds.min.x - 2.0, bounds.min.y - 0.25, bounds.min.z - 2.0),
            max: Vec3::new(bounds.max.x + 2.0, bounds.min.y + 0.12, bounds.max.z + 2.0),
        }]
    } else {
        collision_boxes
    };

    Ok(GltfLevelCpu {
        vertices,
        indices,
        batches,
        images_rgba8,
        spawn: spawn_pt,
        spawn_yaw,
        solids,
    })
}

/// Minimal glTF (single mesh or small prop) for **playable / NPC 3D bodies**.
/// Vertices are baked in file space; the renderer applies per-entity `model` from relay pose.
pub fn parse_character_glb(bytes: &[u8]) -> Result<CharacterMeshCpu, String> {
    let (document, buffers, images) =
        gltf::import_slice(bytes).map_err(|e| format!("character gltf import: {e}"))?;

    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or_else(|| "character glTF has no scenes".to_string())?;

    let mut vertices: Vec<CharacterVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut batches: Vec<GltfBatchCpu> = Vec::new();

    let mut images_rgba8: Vec<(u32, u32, Vec<u8>)> = Vec::with_capacity(images.len());
    for img in &images {
        let rgba = image_data_to_rgba(img)?;
        images_rgba8.push((img.width, img.height, rgba));
    }

    for root in scene.nodes() {
        visit_character_node(
            root,
            Mat4::IDENTITY,
            &buffers,
            &mut vertices,
            &mut indices,
            &mut batches,
        );
    }

    if vertices.is_empty() {
        return Err("character glTF has no mesh geometry".into());
    }

    Ok(CharacterMeshCpu {
        vertices,
        indices,
        batches,
        images_rgba8,
    })
}

#[allow(clippy::too_many_arguments)]
fn visit_character_node(
    node: gltf::Node<'_>,
    parent: Mat4,
    buffers: &[gltf::buffer::Data],
    vertices: &mut Vec<CharacterVertex>,
    indices: &mut Vec<u32>,
    batches: &mut Vec<GltfBatchCpu>,
) {
    let world = parent * mat_from_transform(node.transform());

    if let Some(mesh) = node.mesh() {
        for prim in mesh.primitives() {
            let mat = prim.material();
            let pbr = mat.pbr_metallic_roughness();
            let raw_tint: [f32; 4] = pbr.base_color_factor();
            let emissive = mat.emissive_factor();
            let (image_index, uv_set) = pbr
                .base_color_texture()
                .map(|info| (info.texture().source().index(), info.tex_coord()))
                .unwrap_or((usize::MAX, 0u32));
            let tint = if image_index == usize::MAX {
                let base_lum = raw_tint[0] + raw_tint[1] + raw_tint[2];
                let emit_lum = emissive[0] + emissive[1] + emissive[2];
                if base_lum < 0.01 && emit_lum > 0.01 {
                    [emissive[0], emissive[1], emissive[2], raw_tint[3]]
                } else {
                    let r = (raw_tint[0] + emissive[0]).min(1.0);
                    let g = (raw_tint[1] + emissive[1]).min(1.0);
                    let b = (raw_tint[2] + emissive[2]).min(1.0);
                    [r, g, b, raw_tint[3]]
                }
            } else {
                let r = (raw_tint[0] + emissive[0]).min(1.0);
                let g = (raw_tint[1] + emissive[1]).min(1.0);
                let b = (raw_tint[2] + emissive[2]).min(1.0);
                [r, g, b, raw_tint[3]]
            };

            let r_pos = prim.reader(|b| Some(&buffers[b.index()]));
            let Some(iter_pos) = r_pos.read_positions() else {
                continue;
            };
            let positions: Vec<Vec3> = iter_pos.map(Vec3::from_array).collect();
            if positions.is_empty() {
                continue;
            }

            let r_uv = prim.reader(|b| Some(&buffers[b.index()]));
            let uv0: Vec<[f32; 2]> = match r_uv.read_tex_coords(uv_set).map(|tc| tc.into_f32().collect::<Vec<[f32; 2]>>()) {
                Some(collected) if collected.len() == positions.len() => collected,
                Some(collected) => positions
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        collected.get(i).copied().unwrap_or_else(|| {
                            world_space_fallback_uv(world.transform_point3(*p))
                        })
                    })
                    .collect(),
                None if image_index != usize::MAX => positions
                    .iter()
                    .map(|p| world_space_fallback_uv(world.transform_point3(*p)))
                    .collect(),
                None => vec![[0.0, 0.0]; positions.len()],
            };

            let r_idx = prim.reader(|b| Some(&buffers[b.index()]));
            let prim_indices: Vec<u32> = if let Some(idr) = r_idx.read_indices() {
                idr.into_u32().collect()
            } else {
                (0..positions.len() as u32).collect()
            };

            let r_nrm = prim.reader(|b| Some(&buffers[b.index()]));
            let normals_local: Vec<Vec3> = match r_nrm.read_normals() {
                Some(iter) => {
                    let mut v: Vec<Vec3> = iter.map(Vec3::from_array).collect();
                    if v.len() != positions.len() {
                        v = vertex_normals_local(&positions, &prim_indices);
                    }
                    v
                }
                None => vertex_normals_local(&positions, &prim_indices),
            };

            let base = vertices.len() as u32;
            for (i, p) in positions.iter().enumerate() {
                let wp = world.transform_point3(*p);
                let uv = uv0.get(i).copied().unwrap_or([0.0, 0.0]);
                let nl = normals_local.get(i).copied().unwrap_or(Vec3::Z);
                let nwm = transform_normal(world, nl);
                vertices.push(CharacterVertex {
                    pos: wp.to_array(),
                    uv,
                    nrm: nwm.to_array(),
                });
            }

            let first_index = indices.len() as u32;
            for idx in &prim_indices {
                indices.push(base + idx);
            }
            let index_count = prim_indices.len() as u32;

            batches.push(GltfBatchCpu {
                first_index,
                index_count,
                image_index,
                tint,
            });
        }
    }

    for child in node.children() {
        visit_character_node(
            child,
            world,
            buffers,
            vertices,
            indices,
            batches,
        );
    }
}

fn image_data_to_rgba(img: &gltf::image::Data) -> Result<Vec<u8>, String> {
    use gltf::image::Format;
    let w = img.width as usize;
    let h = img.height as usize;
    match img.format {
        Format::R8G8B8A8 => Ok(img.pixels.clone()),
        Format::R8G8B8 => {
            let mut out = Vec::with_capacity(w * h * 4);
            for chunk in img.pixels.chunks_exact(3) {
                out.push(chunk[0]);
                out.push(chunk[1]);
                out.push(chunk[2]);
                out.push(255);
            }
            Ok(out)
        }
        Format::R8G8 => {
            let mut out = Vec::with_capacity(w * h * 4);
            for chunk in img.pixels.chunks_exact(2) {
                out.push(chunk[0]);
                out.push(chunk[0]);
                out.push(chunk[0]);
                out.push(chunk[1]);
            }
            Ok(out)
        }
        Format::R8 => {
            let mut out = Vec::with_capacity(w * h * 4);
            for g in &img.pixels {
                out.push(*g);
                out.push(*g);
                out.push(*g);
                out.push(255);
            }
            Ok(out)
        }
        f => Err(format!("unsupported glTF image format {:?}", f)),
    }
}

#[allow(clippy::too_many_arguments)]
fn visit_node(
    node: gltf::Node<'_>,
    parent: Mat4,
    buffers: &[gltf::buffer::Data],
    _document: &gltf::Document,
    vertices: &mut Vec<WorldVertex>,
    indices: &mut Vec<u32>,
    batches: &mut Vec<GltfBatchCpu>,
    spawn: &mut Option<(Vec3, Mat4)>,
    collision_boxes: &mut Vec<Aabb>,
) {
    let world = parent * mat_from_transform(node.transform());
    let name = node.name().unwrap_or("");

    if is_spawn_name(name) {
        let t = world.transform_point3(Vec3::ZERO);
        *spawn = Some((Vec3::new(t.x, t.y + 0.04, t.z), world));
    }

    if let Some(mesh) = node.mesh() {
        let collider = is_collision_name(name);
        for prim in mesh.primitives() {
            let mat = prim.material();
            let pbr = mat.pbr_metallic_roughness();
            let raw_tint: [f32; 4] = pbr.base_color_factor();
            let emissive = mat.emissive_factor();
            let bct_info = pbr.base_color_texture();
            let (image_index, uv_set) = bct_info
                .as_ref()
                .map(|info| (info.texture().source().index(), info.tex_coord()))
                .unwrap_or((usize::MAX, 0u32));
            // Extract KHR_texture_transform (UV offset/scale/rotation) if present.
            let (uv_offset, uv_scale, uv_rotation) = bct_info
                .as_ref()
                .and_then(|info| info.texture_transform())
                .map(|tt| {
                    let off = tt.offset();
                    let sc = tt.scale();
                    let rot = tt.rotation();
                    ([off[0], off[1]], [sc[0], sc[1]], rot)
                })
                .unwrap_or(([0.0, 0.0], [1.0, 1.0], 0.0));

            // For factor-only materials (no texture): use emissive color
            // when baseColor is near-black (Blender emission-only materials).
            // Also add emissive contribution to base color.
            let tint = if image_index == usize::MAX {
                let base_lum = raw_tint[0] + raw_tint[1] + raw_tint[2];
                let emit_lum = emissive[0] + emissive[1] + emissive[2];
                if base_lum < 0.01 && emit_lum > 0.01 {
                    [emissive[0], emissive[1], emissive[2], raw_tint[3]]
                } else {
                    let r = (raw_tint[0] + emissive[0]).min(1.0);
                    let g = (raw_tint[1] + emissive[1]).min(1.0);
                    let b = (raw_tint[2] + emissive[2]).min(1.0);
                    [r, g, b, raw_tint[3]]
                }
            } else {
                let r = (raw_tint[0] + emissive[0]).min(1.0);
                let g = (raw_tint[1] + emissive[1]).min(1.0);
                let b = (raw_tint[2] + emissive[2]).min(1.0);
                [r, g, b, raw_tint[3]]
            };

            let r_pos = prim.reader(|b| Some(&buffers[b.index()]));
            let Some(iter_pos) = r_pos.read_positions() else {
                continue;
            };
            let positions: Vec<Vec3> = iter_pos.map(Vec3::from_array).collect();
            if positions.is_empty() {
                continue;
            }

            let r_uv = prim.reader(|b| Some(&buffers[b.index()]));
            let raw_uvs: Vec<[f32; 2]> = match r_uv.read_tex_coords(uv_set).map(|tc| tc.into_f32().collect::<Vec<[f32; 2]>>()) {
                Some(collected) if collected.len() == positions.len() => collected,
                Some(collected) => positions
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        collected.get(i).copied().unwrap_or_else(|| {
                            world_space_fallback_uv(world.transform_point3(*p))
                        })
                    })
                    .collect(),
                None if image_index != usize::MAX => positions
                    .iter()
                    .map(|p| world_space_fallback_uv(world.transform_point3(*p)))
                    .collect(),
                None => vec![[0.0, 0.0]; positions.len()],
            };
            // Apply KHR_texture_transform: uv' = rotation(uv) * scale + offset
            let uv0: Vec<[f32; 2]> = if uv_rotation.abs() > 1e-6 {
                let (sin_r, cos_r) = uv_rotation.sin_cos();
                raw_uvs.iter().map(|uv| {
                    let ru = uv[0] * cos_r - uv[1] * sin_r;
                    let rv = uv[0] * sin_r + uv[1] * cos_r;
                    [ru * uv_scale[0] + uv_offset[0], rv * uv_scale[1] + uv_offset[1]]
                }).collect()
            } else {
                raw_uvs.iter().map(|uv| {
                    [uv[0] * uv_scale[0] + uv_offset[0], uv[1] * uv_scale[1] + uv_offset[1]]
                }).collect()
            };

            let r_idx = prim.reader(|b| Some(&buffers[b.index()]));
            let prim_indices: Vec<u32> = if let Some(idr) = r_idx.read_indices() {
                idr.into_u32().collect()
            } else {
                (0..positions.len() as u32).collect()
            };

            let base = vertices.len() as u32;
            let mut cmin = Vec3::splat(f32::MAX);
            let mut cmax = Vec3::splat(f32::MIN);
            for (i, p) in positions.iter().enumerate() {
                let wp = world.transform_point3(*p);
                if collider {
                    cmin = cmin.min(wp);
                    cmax = cmax.max(wp);
                }
                let uv = uv0.get(i).copied().unwrap_or([0.0, 0.0]);
                vertices.push(WorldVertex {
                    pos: wp.to_array(),
                    uv,
                });
            }
            if collider && cmin.x <= cmax.x {
                collision_boxes.push(Aabb { min: cmin, max: cmax });
            }

            let first_index = indices.len() as u32;
            for idx in &prim_indices {
                indices.push(base + idx);
            }
            let index_count = prim_indices.len() as u32;

            batches.push(GltfBatchCpu {
                first_index,
                index_count,
                image_index,
                tint,
            });
        }
    }

    for child in node.children() {
        visit_node(
            child,
            world,
            buffers,
            _document,
            vertices,
            indices,
            batches,
            spawn,
            collision_boxes,
        );
    }
}
