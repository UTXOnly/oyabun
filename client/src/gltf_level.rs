//! Load Blender-exported `.glb` (glTF 2.0). Blender's exporter emits **Y-up** space matching this client.
//!
//! **Spawn**: Empty named `OyabaunSpawn` or `PlayerSpawn` (case-insensitive match on `spawn` + `player`/`oyabaun`).
//! **Collision**: Mesh on a node whose name contains `Collider` or `OyabaunCollision` (case-insensitive).

use glam::{Mat4, Quat, Vec3};

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
    let mut spawn: Option<Vec3> = None;
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
    let spawn_pt = spawn.unwrap_or_else(|| {
        Vec3::new(
            (bounds.min.x + bounds.max.x) * 0.5,
            bounds.min.y + 0.06,
            (bounds.min.z + bounds.max.z) * 0.5,
        )
    });
    let spawn_yaw = default_spawn_yaw(&bounds, spawn_pt);

    let solids = if collision_boxes.is_empty() {
        vec![bounds]
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
    spawn: &mut Option<Vec3>,
    collision_boxes: &mut Vec<Aabb>,
) {
    let world = parent * mat_from_transform(node.transform());
    let name = node.name().unwrap_or("");

    if is_spawn_name(name) {
        let t = world.transform_point3(Vec3::ZERO);
        *spawn = Some(Vec3::new(t.x, t.y + 0.04, t.z));
    }

    if let Some(mesh) = node.mesh() {
        let collider = is_collision_name(name);
        for prim in mesh.primitives() {
            let mat = prim.material();
            let pbr = mat.pbr_metallic_roughness();
            let tint: [f32; 4] = pbr.base_color_factor();
            let (image_index, uv_set) = pbr
                .base_color_texture()
                .map(|info| (info.texture().source().index(), info.tex_coord()))
                .unwrap_or((usize::MAX, 0u32));

            let r_pos = prim.reader(|b| Some(&buffers[b.index()]));
            let Some(iter_pos) = r_pos.read_positions() else {
                continue;
            };
            let positions: Vec<Vec3> = iter_pos.map(Vec3::from_array).collect();
            if positions.is_empty() {
                continue;
            }

            let r_uv = prim.reader(|b| Some(&buffers[b.index()]));
            let uv0: Vec<[f32; 2]> = r_uv
                .read_tex_coords(uv_set)
                .map(|tc| tc.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

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
