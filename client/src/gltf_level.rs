//! Load Blender-exported `.glb` (glTF 2.0). Blender's exporter emits **Y-up** space matching this client.
//!
//! **Spawn**: Empty named `OyabaunSpawn` or `PlayerSpawn` (case-insensitive match on `spawn` + `player`/`oyabaun`).
//! If missing, spawn is placed near **min-Z** on the map XZ span (alley mouth), not the AABB center — long levels often have empty space at center-Z.
//! **Collision**: Mesh on a node whose name contains `Collider` or `OyabaunCollision` (case-insensitive).

use glam::{Mat4, Quat, Vec3, Vec4};
use std::collections::HashMap;

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

/// Max joints for skinned character shader storage (`array<mat4x4<f32>, N>` must match).
pub const CHARACTER_MAX_JOINTS: usize = 64;

/// Skinned character vertex (linear blend skinning in `render.rs`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinnedCharacterVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub nrm: [f32; 3],
    pub joints: [u32; 4],
    pub weights: [f32; 4],
}

impl SkinnedCharacterVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SkinnedCharacterVertex>() as wgpu::BufferAddress,
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
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32x4,
                },
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// One glTF animation channel (linear/step sampling).
#[derive(Clone, Debug)]
pub struct SkinnedAnimChannelCpu {
    pub node: usize,
    pub times: Vec<f32>,
    pub interpolation: gltf::animation::Interpolation,
    pub outputs: SkinnedAnimOutputsCpu,
}

#[derive(Clone, Debug)]
pub enum SkinnedAnimOutputsCpu {
    Translation(Vec<Vec3>),
    Rotation(Vec<Quat>),
    Scale(Vec<Vec3>),
}

#[derive(Clone, Debug)]
pub struct SkinnedClipCpu {
    pub name: String,
    pub duration: f32,
    pub looping: bool,
    pub channels: Vec<SkinnedAnimChannelCpu>,
}

/// Mixamo-style skinned body: joint palette = `global_joint * inverse_bind` per joint (glTF convention).
pub struct SkinnedCharacterMeshCpu {
    pub vertices: Vec<SkinnedCharacterVertex>,
    pub indices: Vec<u32>,
    pub batches: Vec<GltfBatchCpu>,
    pub images_rgba8: Vec<(u32, u32, Vec<u8>)>,
    /// `globalTransform(joint) * inverseBindMatrix` at default pose, length `CHARACTER_MAX_JOINTS` (padded).
    pub rest_joint_palette: Vec<Mat4>,
    /// For skinned Mixamo exports: **identity**. The armature rotation is already in joint globals; multiplying
    /// the mesh node's world matrix again tips the model onto the ground.
    pub mesh_node_world: Mat4,
    pub inverse_bind: Vec<Mat4>,
    pub joint_node_indices: Vec<usize>,
    /// Joint palette index for rigid weapon attach (`node_world * offset`, no inverse bind). E.g. Mixamo `RightHand`.
    pub weapon_attach_joint: Option<u32>,
    pub node_parent: Vec<Option<usize>>,
    pub node_local_rest: Vec<Mat4>,
    /// Scene graph DFS order (parent before children) for propagating animated locals.
    pub node_depth_order: Vec<usize>,
    pub clips: Vec<SkinnedClipCpu>,
    pub clip_index_by_name: HashMap<String, usize>,
}

/// Resolve clip indices after load (names from Blender/Mixamo export).
#[derive(Clone, Debug, Default)]
pub struct SkinnedAnimClipIds {
    pub idle: u32,
    pub run: u32,
    pub walk: u32,
    pub fire: u32,
    pub die: u32,
    pub die_duration: f32,
}

fn clip_loops_by_name(name: &str) -> bool {
    matches!(
        name,
        "rifle_aiming_idle"
            | "rifle_run"
            | "walking"
            | "walking_backwards"
            | "run_backwards"
            | "strafe_left"
            | "strafe_right"
    )
}

fn build_node_parent_and_order<'a>(
    roots: impl Iterator<Item = gltf::Node<'a>>,
    node_parent: &mut Vec<Option<usize>>,
    node_depth_order: &mut Vec<usize>,
) {
    fn visit(node: gltf::Node<'_>, parent: Option<usize>, node_parent: &mut Vec<Option<usize>>, order: &mut Vec<usize>) {
        let i = node.index();
        node_parent[i] = parent;
        order.push(i);
        for c in node.children() {
            visit(c, Some(i), node_parent, order);
        }
    }
    for root in roots {
        visit(root, None, node_parent, node_depth_order);
    }
}

fn parse_skinned_animations(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
) -> Result<Vec<SkinnedClipCpu>, String> {
    use gltf::animation::Property;

    let mut clips = Vec::new();
    for anim in document.animations() {
        let name = anim.name().unwrap_or("anim").to_string();
        let mut duration = 0.0_f32;
        let mut channels = Vec::new();

        for ch in anim.channels() {
            let reader = ch.reader(|b| Some(&buffers[b.index()]));
            let times: Vec<f32> = match reader.read_inputs() {
                Some(it) => it.collect(),
                None => continue,
            };
            if let Some(&last) = times.last() {
                duration = duration.max(last);
            }
            let target = ch.target();
            let node = target.node().index();
            let sampler = ch.sampler();
            let interpolation = sampler.interpolation();

            let outputs = match reader.read_outputs() {
                Some(o) => o,
                None => continue,
            };

            let prop = target.property();
            let chan = match (prop, outputs) {
                (Property::Translation, gltf::animation::util::ReadOutputs::Translations(it)) => {
                    let v: Vec<Vec3> = it.map(Vec3::from_array).collect();
                    SkinnedAnimChannelCpu {
                        node,
                        times,
                        interpolation,
                        outputs: SkinnedAnimOutputsCpu::Translation(v),
                    }
                }
                (Property::Rotation, gltf::animation::util::ReadOutputs::Rotations(r)) => {
                    let v: Vec<Quat> = r
                        .into_f32()
                        .map(|[x, y, z, w]| Quat::from_xyzw(x, y, z, w))
                        .collect();
                    SkinnedAnimChannelCpu {
                        node,
                        times,
                        interpolation,
                        outputs: SkinnedAnimOutputsCpu::Rotation(v),
                    }
                }
                (Property::Scale, gltf::animation::util::ReadOutputs::Scales(it)) => {
                    let v: Vec<Vec3> = it.map(Vec3::from_array).collect();
                    SkinnedAnimChannelCpu {
                        node,
                        times,
                        interpolation,
                        outputs: SkinnedAnimOutputsCpu::Scale(v),
                    }
                }
                _ => continue,
            };
            channels.push(chan);
        }

        let looping = clip_loops_by_name(name.as_str());
        clips.push(SkinnedClipCpu {
            name,
            duration,
            looping,
            channels,
        });
    }
    Ok(clips)
}

fn sample_linear(times: &[f32], t: f32, looping: bool, duration: f32) -> (usize, usize, f32) {
    if times.is_empty() {
        return (0, 0, 0.0);
    }
    let mut tt = t;
    if looping && duration > 1e-6 {
        tt = tt.rem_euclid(duration);
    } else {
        tt = tt.clamp(0.0, duration.max(*times.last().unwrap_or(&0.0)));
    }
    if tt <= times[0] {
        return (0, 0, 0.0);
    }
    let last = *times.last().unwrap();
    if tt >= last {
        let n = times.len() - 1;
        return (n, n, 0.0);
    }
    let i1 = times.partition_point(|&x| x < tt);
    let i0 = i1.saturating_sub(1).min(times.len() - 2);
    let i1 = (i0 + 1).min(times.len() - 1);
    let denom = (times[i1] - times[i0]).max(1e-8);
    let u = ((tt - times[i0]) / denom).clamp(0.0, 1.0);
    (i0, i1, u)
}

fn sample_vec3_channel(
    times: &[f32],
    vals: &[Vec3],
    t: f32,
    interp: gltf::animation::Interpolation,
    looping: bool,
    duration: f32,
) -> Vec3 {
    if vals.is_empty() {
        return Vec3::ZERO;
    }
    if vals.len() == 1 || times.is_empty() {
        return vals[0];
    }
    let (i0, i1, u) = sample_linear(times, t, looping, duration);
    match interp {
        gltf::animation::Interpolation::Step => vals[i0],
        _ => vals[i0].lerp(vals[i1], u),
    }
}

fn sample_quat_channel(
    times: &[f32],
    vals: &[Quat],
    t: f32,
    interp: gltf::animation::Interpolation,
    looping: bool,
    duration: f32,
) -> Quat {
    if vals.is_empty() {
        return Quat::IDENTITY;
    }
    if vals.len() == 1 || times.is_empty() {
        return vals[0].normalize();
    }
    let (i0, i1, u) = sample_linear(times, t, looping, duration);
    match interp {
        gltf::animation::Interpolation::Step => vals[i0].normalize(),
        _ => vals[i0].normalize().slerp(vals[i1].normalize(), u),
    }
}

/// Fills `node_world_out` with animated scene node globals and `out_palette` for skinning.
/// `node_world_out` must have length ≥ node count; used for weapon attach (joint node transform, no inverse bind).
pub fn compute_skinned_joint_palette(
    skin: &SkinnedCharacterMeshCpu,
    clip_index: usize,
    time: f32,
    locals_scratch: &mut [Mat4],
    node_world_out: &mut [Mat4],
    out_palette: &mut [Mat4; CHARACTER_MAX_JOINTS],
) {
    let n = skin.node_parent.len();
    if locals_scratch.len() < n || skin.node_local_rest.len() < n || node_world_out.len() < n {
        return;
    }

    let mut t_ov = vec![None; n];
    let mut r_ov = vec![None; n];
    let mut s_ov = vec![None; n];

    if clip_index < skin.clips.len() {
        let clip = &skin.clips[clip_index];
        let dur = clip.duration.max(1e-4);
        for ch in &clip.channels {
            let ni = ch.node;
            if ni >= n {
                continue;
            }
            match &ch.outputs {
                SkinnedAnimOutputsCpu::Translation(v) => {
                    t_ov[ni] = Some(sample_vec3_channel(
                        &ch.times,
                        v,
                        time,
                        ch.interpolation,
                        clip.looping,
                        dur,
                    ));
                }
                SkinnedAnimOutputsCpu::Rotation(v) => {
                    r_ov[ni] = Some(sample_quat_channel(
                        &ch.times,
                        v,
                        time,
                        ch.interpolation,
                        clip.looping,
                        dur,
                    ));
                }
                SkinnedAnimOutputsCpu::Scale(v) => {
                    s_ov[ni] = Some(sample_vec3_channel(
                        &ch.times,
                        v,
                        time,
                        ch.interpolation,
                        clip.looping,
                        dur,
                    ));
                }
            }
        }
    }

    for ni in 0..n {
        let (rs, rr, rt) = skin.node_local_rest[ni].to_scale_rotation_translation();
        let sf = s_ov[ni].unwrap_or(rs);
        let rf = r_ov[ni].unwrap_or(rr);
        let tf = t_ov[ni].unwrap_or(rt);
        locals_scratch[ni] = Mat4::from_scale_rotation_translation(sf, rf, tf);
    }

    for &ni in &skin.node_depth_order {
        if ni >= n {
            continue;
        }
        let w = if let Some(pi) = skin.node_parent[ni] {
            node_world_out[pi] * locals_scratch[ni]
        } else {
            locals_scratch[ni]
        };
        node_world_out[ni] = w;
    }

    for i in 0..CHARACTER_MAX_JOINTS {
        out_palette[i] = Mat4::IDENTITY;
    }
    for (ji, &jnode) in skin.joint_node_indices.iter().enumerate() {
        if ji >= CHARACTER_MAX_JOINTS || jnode >= n {
            break;
        }
        out_palette[ji] = node_world_out[jnode] * skin.inverse_bind[ji];
    }
}

pub fn resolve_skinned_clip_indices(clips: &[SkinnedClipCpu]) -> SkinnedAnimClipIds {
    fn find(name: &str, clips: &[SkinnedClipCpu]) -> u32 {
        clips
            .iter()
            .position(|c| c.name == name)
            .unwrap_or(0) as u32
    }
    let die_i = find("walking_to_dying", clips) as usize;
    let die_duration = clips
        .get(die_i)
        .map(|c| c.duration.max(0.05))
        .unwrap_or(3.0);
    SkinnedAnimClipIds {
        idle: find("rifle_aiming_idle", clips),
        run: find("rifle_run", clips),
        walk: find("walking", clips),
        fire: find("firing_rifle", clips),
        die: die_i as u32,
        die_duration,
    }
}

pub enum CharacterGltfCpu {
    Rigid(CharacterMeshCpu),
    Skinned(SkinnedCharacterMeshCpu),
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
    /// When true, `game_init_from_gltf` will not add a giant floor slab
    /// (the level already provides its own floor collision).
    pub skip_floor_slab: bool,
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
            0,
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
        skip_floor_slab: false,
    })
}

/// Merge a second `.glb` into an existing level (vertices, indices, batches, images).
/// Use for procedural `GltfLevelCpu` + optional Blender blockout props. Geometry is transformed by `root` (Y-up, same as Blender export).
/// `image_index` in appended batches is offset by the pre-append image count.
/// Returns extra collision AABBs from collider-named meshes in the appended file (often empty).
pub fn append_glb_transform(
    cpu: &mut GltfLevelCpu,
    bytes: &[u8],
    root: Mat4,
) -> Result<Vec<Aabb>, String> {
    let (document, buffers, images) =
        gltf::import_slice(bytes).map_err(|e| format!("append glb import: {e}"))?;

    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or_else(|| "append glb has no scenes".to_string())?;

    let image_base = cpu.images_rgba8.len();
    for img in &images {
        let rgba = image_data_to_rgba(img)?;
        cpu.images_rgba8.push((img.width, img.height, rgba));
    }

    let mut extra_colliders = Vec::new();
    let mut spawn_dummy: Option<(Vec3, Mat4)> = None;
    for root_node in scene.nodes() {
        visit_node(
            root_node,
            root,
            &buffers,
            &document,
            &mut cpu.vertices,
            &mut cpu.indices,
            &mut cpu.batches,
            &mut spawn_dummy,
            &mut extra_colliders,
            image_base,
        );
    }
    Ok(extra_colliders)
}

/// Minimal glTF (single mesh or small prop) for **playable / NPC 3D bodies**.
/// Rigid meshes bake node transforms into vertices; skinned meshes (Mixamo, etc.) return [`CharacterGltfCpu::Skinned`].
pub fn parse_character_glb(bytes: &[u8]) -> Result<CharacterGltfCpu, String> {
    let (document, buffers, images) =
        gltf::import_slice(bytes).map_err(|e| format!("character gltf import: {e}"))?;

    if glb_has_skinned_primitive(&document, &buffers) {
        parse_skinned_character_glb(&document, &buffers, images)
    } else {
        parse_rigid_character_glb(&document, &buffers, images).map(CharacterGltfCpu::Rigid)
    }
}

fn glb_has_skinned_primitive(document: &gltf::Document, buffers: &[gltf::buffer::Data]) -> bool {
    for mesh in document.meshes() {
        for prim in mesh.primitives() {
            let r = prim.reader(|b| Some(&buffers[b.index()]));
            if r.read_joints(0).is_some() {
                return true;
            }
        }
    }
    false
}

fn mat4_from_gltf_accessor(cols: [[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array_2d(&cols)
}

fn dfs_node_world(
    node: gltf::Node<'_>,
    parent_world: Mat4,
    node_world: &mut [Mat4],
) {
    let local = mat_from_transform(node.transform());
    let w = parent_world * local;
    node_world[node.index()] = w;
    for child in node.children() {
        dfs_node_world(child, w, node_world);
    }
}

fn find_skinned_mesh_node<'a>(
    node: gltf::Node<'a>,
    out: &mut Option<(gltf::Node<'a>, gltf::Skin<'a>)>,
) {
    if out.is_some() {
        return;
    }
    if let Some(_mesh) = node.mesh() {
        if let Some(skin) = node.skin() {
            *out = Some((node, skin));
            return;
        }
    }
    for child in node.children() {
        find_skinned_mesh_node(child, out);
    }
}

fn parse_skinned_character_glb(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    images: Vec<gltf::image::Data>,
) -> Result<CharacterGltfCpu, String> {
    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or_else(|| "character glTF has no scenes".to_string())?;

    let n_nodes = document.nodes().len();
    let mut node_world = vec![Mat4::IDENTITY; n_nodes];
    for root in scene.nodes() {
        dfs_node_world(root, Mat4::IDENTITY, &mut node_world);
    }

    let mut found = None;
    for root in scene.nodes() {
        find_skinned_mesh_node(root, &mut found);
    }
    let (mesh_node, skin) = found.ok_or_else(|| "character glTF has no skinned mesh node".to_string())?;

    let mesh_node_idx = mesh_node.index();
    let mesh_node_mtx_for_uv = node_world[mesh_node_idx];

    let n_nodes = document.nodes().len();
    let mut node_parent = vec![None; n_nodes];
    let mut node_depth_order = Vec::new();
    build_node_parent_and_order(scene.nodes(), &mut node_parent, &mut node_depth_order);

    let mut node_local_rest = vec![Mat4::IDENTITY; n_nodes];
    for node in document.nodes() {
        let i = node.index();
        if i < n_nodes {
            node_local_rest[i] = mat_from_transform(node.transform());
        }
    }

    let skin_reader = skin.reader(|b| Some(&buffers[b.index()]));
    let ibm: Vec<Mat4> = skin_reader
        .read_inverse_bind_matrices()
        .ok_or_else(|| "skinned character: missing inverse bind matrices".to_string())?
        .map(mat4_from_gltf_accessor)
        .collect();
    let joint_node_indices: Vec<usize> = skin.joints().map(|j| j.index()).collect();
    if ibm.len() != joint_node_indices.len() {
        return Err(format!(
            "skinned character: IBM count {} != joint count {}",
            ibm.len(),
            joint_node_indices.len()
        ));
    }
    if joint_node_indices.len() > CHARACTER_MAX_JOINTS {
        return Err(format!(
            "skinned character: {} joints exceed max {}",
            joint_node_indices.len(),
            CHARACTER_MAX_JOINTS
        ));
    }

    // Mixamo: `RightHandIndex*` sorts before `RightHand` in many skins — must not use substring "righthand".
    let mut weapon_attach_joint: Option<u32> = None;
    for (ji, &jnode) in joint_node_indices.iter().enumerate() {
        let Some(node) = document.nodes().nth(jnode) else {
            continue;
        };
        let name = node.name().unwrap_or("");
        if name.ends_with("RightHand") && !name.contains("RightHandIndex") {
            weapon_attach_joint = Some(ji as u32);
            break;
        }
    }

    let mut rest_joint_palette = vec![Mat4::IDENTITY; CHARACTER_MAX_JOINTS];
    for (i, &jnode) in joint_node_indices.iter().enumerate() {
        let gw = node_world[jnode];
        rest_joint_palette[i] = gw * ibm[i];
    }

    let clips = parse_skinned_animations(document, buffers)?;
    let clip_index_by_name: HashMap<String, usize> = clips
        .iter()
        .enumerate()
        .map(|(i, c)| (c.name.clone(), i))
        .collect();

    let mesh = mesh_node
        .mesh()
        .ok_or_else(|| "skinned mesh node has no mesh".to_string())?;

    let mut vertices: Vec<SkinnedCharacterVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut batches: Vec<GltfBatchCpu> = Vec::new();

    let mut images_rgba8: Vec<(u32, u32, Vec<u8>)> = Vec::with_capacity(images.len());
    for img in &images {
        let rgba = image_data_to_rgba(img)?;
        images_rgba8.push((img.width, img.height, rgba));
    }

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

        let r_joints = prim.reader(|b| Some(&buffers[b.index()]));
        let joints_flat: Vec<[u32; 4]> = r_joints
            .read_joints(0)
            .ok_or_else(|| "skinned character primitive missing JOINTS_0".to_string())?
            .into_u16()
            .map(|[a, b, c, d]| [a as u32, b as u32, c as u32, d as u32])
            .collect();
        if joints_flat.len() != positions.len() {
            return Err("skinned character: JOINTS_0 length mismatch".into());
        }

        let r_w = prim.reader(|b| Some(&buffers[b.index()]));
        let weights_flat: Vec<[f32; 4]> = r_w
            .read_weights(0)
            .ok_or_else(|| "skinned character primitive missing WEIGHTS_0".to_string())?
            .into_f32()
            .collect();
        if weights_flat.len() != positions.len() {
            return Err("skinned character: WEIGHTS_0 length mismatch".into());
        }

        let r_uv = prim.reader(|b| Some(&buffers[b.index()]));
        let uv0: Vec<[f32; 2]> = match r_uv
            .read_tex_coords(uv_set)
            .map(|tc| tc.into_f32().collect::<Vec<[f32; 2]>>())
        {
            Some(collected) if collected.len() == positions.len() => collected,
            Some(collected) => positions
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    collected.get(i).copied().unwrap_or_else(|| {
                        world_space_fallback_uv(mesh_node_mtx_for_uv.transform_point3(*p))
                    })
                })
                .collect(),
            None if image_index != usize::MAX => positions
                .iter()
                .map(|p| world_space_fallback_uv(mesh_node_mtx_for_uv.transform_point3(*p)))
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
        for i in 0..positions.len() {
            let p = positions[i];
            let uv = uv0.get(i).copied().unwrap_or([0.0, 0.0]);
            let nl = normals_local.get(i).copied().unwrap_or(Vec3::Z);
            let j = joints_flat[i];
            let wv = weights_flat[i];
            vertices.push(SkinnedCharacterVertex {
                pos: p.to_array(),
                uv,
                nrm: nl.to_array(),
                joints: j,
                weights: wv,
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

    if vertices.is_empty() {
        return Err("skinned character glTF has no drawable geometry".into());
    }

    Ok(CharacterGltfCpu::Skinned(SkinnedCharacterMeshCpu {
        vertices,
        indices,
        batches,
        images_rgba8,
        rest_joint_palette,
        mesh_node_world: Mat4::IDENTITY,
        inverse_bind: ibm,
        joint_node_indices,
        weapon_attach_joint,
        node_parent,
        node_local_rest,
        node_depth_order,
        clips,
        clip_index_by_name,
    }))
}

fn parse_rigid_character_glb(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    images: Vec<gltf::image::Data>,
) -> Result<CharacterMeshCpu, String> {
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
            buffers,
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
    image_index_base: usize,
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

            let img_out = if image_index == usize::MAX {
                usize::MAX
            } else {
                image_index.saturating_add(image_index_base)
            };
            batches.push(GltfBatchCpu {
                first_index,
                index_count,
                image_index: img_out,
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
            image_index_base,
        );
    }
}
