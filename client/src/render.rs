use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3, Vec4};
use serde::Serialize;

use crate::mesh::Aabb;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;
#[cfg(target_arch = "wasm32")]
use wgpu::{ExternalImageSource, ImageCopyExternalImage, Origin2d};

#[cfg(target_arch = "wasm32")]
fn warn_str(s: &str) {
    web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(s));
}

/// Gentle distance darkening — the reference image is clear; depth is value/hue not murk.
const FOG_DENSITY: f32 = 0.00012;
/// Low cap — backdrop should read clearly even at the far wall.
const FOG_MAX_BLEND: f32 = 0.15;
/// Near-black tint (dark navy from palette, not purple haze).
const FOG_COLOR_RGBA: [f32; 4] = [0.047, 0.059, 0.133, 1.0];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub color: [f32; 3],
}

impl Vertex {
    pub fn new(pos: Vec3, color: Vec3) -> Self {
        Self {
            pos: pos.to_array(),
            color: color.to_array(),
        }
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct BillVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    /// RGB multiply + alpha (mural = white; NPC sprites = injury / corpse tint).
    pub tint: [f32; 4],
}

impl BillVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BillVertex>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    fog_color: [f32; 4],
    fog_params: [f32; 4],
    _pad: [f32; 8],
}

const SHADER_WORLD: &str = r#"
struct Globals {
  view_proj: mat4x4<f32>,
  cam_pos: vec4<f32>,
  fog_color: vec4<f32>,
  fog_params: vec4<f32>,
  _pad: vec4<f32>,
  _pad2: vec4<f32>,
}
@group(0) @binding(0) var<uniform> g: Globals;
struct Vin { @location(0) pos: vec3<f32>, @location(1) col: vec3<f32>, };
struct Vout {
  @builtin(position) clip: vec4<f32>,
  @location(0) col: vec3<f32>,
  @location(1) world_pos: vec3<f32>,
};
@vertex
fn vs_main(v: Vin) -> Vout {
    var o: Vout;
    let wp = v.pos;
    o.clip = g.view_proj * vec4<f32>(wp, 1.0);
    o.col = v.col;
    o.world_pos = wp;
    return o;
}
@fragment
fn fs_main(i: Vout) -> @location(0) vec4<f32> {
    let base = i.col * (0.90 + 0.10 * i.col.r) + vec3<f32>(0.08, 0.06, 0.10);
    let q = floor(clamp(base, vec3<f32>(0.0), vec3<f32>(1.0)) * 24.0) / 24.0;
    let dist = length(i.world_pos - g.cam_pos.xyz);
    let fog_amt = min(1.0 - exp(-dist * g.fog_params.x), g.fog_params.y);
    let fc = g.fog_color.rgb;
    return vec4<f32>(mix(q, fc, clamp(fog_amt, 0.0, 1.0)), 1.0);
}
"#;

/// Arcade Tokyo shader: texture × tint with warm ambient, emissive boost, posterize.
/// No procedural brick — art carries detail. Warm overhead + gentle fog.
const SHADER_WORLD_TEX: &str = r#"
struct Globals {
  view_proj: mat4x4<f32>,
  cam_pos: vec4<f32>,
  fog_color: vec4<f32>,
  fog_params: vec4<f32>,
  _pad: vec4<f32>,
  _pad2: vec4<f32>,
}
@group(0) @binding(0) var<uniform> g: Globals;
struct MatU { tint: vec4<f32>, }
@group(1) @binding(0) var albedo: texture_2d<f32>;
@group(1) @binding(1) var albedo_samp: sampler;
@group(1) @binding(2) var<uniform> mu: MatU;

struct Vin { @location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>, };
struct Vout {
  @builtin(position) clip: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) world_pos: vec3<f32>,
};
@vertex
fn vs_tex(v: Vin) -> Vout {
    var o: Vout;
    o.world_pos = v.pos;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    return o;
}

@fragment
fn fs_tex(i: Vout) -> @location(0) vec4<f32> {
    let t = textureSample(albedo, albedo_samp, i.uv) * mu.tint;
    // Map-object PNGs use transparency; without fill, fringe reads as black on box sides.
    let alley_fill = vec3<f32>(0.12, 0.11, 0.17);
    let a = clamp(t.a, 0.0, 1.0);
    let rgb_in = t.rgb + (1.0 - a) * alley_fill * 0.55;
    let wp = i.world_pos;
    let lum = rgb_in.r * 0.3 + rgb_in.g * 0.5 + rgb_in.b * 0.2;
    let mn = min(rgb_in.r, min(rgb_in.g, rgb_in.b));
    let mx = max(rgb_in.r, max(rgb_in.g, rgb_in.b));
    let sat = select(0.0, (mx - mn) / max(mx, 0.001), mx > 0.001);

    // Warm ambient: overhead orange glow fading toward ground (lanterns / neon from above)
    let h_norm = clamp((wp.y - 0.0) / 7.0, 0.0, 1.0);
    let ambient_warm = vec3<f32>(0.22, 0.14, 0.08) * (0.4 + 0.6 * (1.0 - h_norm));
    let ambient_cool = vec3<f32>(0.06, 0.10, 0.16) * h_norm;
    let amb_dim = 0.55 + 0.45 * lum;
    let ambient = (ambient_warm + ambient_cool) * amb_dim;

    // Bright / saturated surfaces glow (signs, lanterns). Achromatic mats (white paint,
    // factor-only glTF × white tex) have sat≈0 — skip boost or they blow out to white.
    let emit_sat = clamp((sat - 0.06) / 0.26, 0.0, 1.0);
    let emit_boost = max(lum - 0.30, 0.0) * (1.2 + sat * 1.5) * emit_sat;

    let lit = rgb_in * 1.6 + ambient + rgb_in * emit_boost;

    // Posterize to 24 levels — retro arcade banding
    var q = floor(clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)) * 24.0) / 24.0;

    // Mild contrast + vibrance pop
    q = clamp((q - 0.5) * 1.08 + 0.5, vec3<f32>(0.0), vec3<f32>(1.0));
    let luma_q = q.r * 0.299 + q.g * 0.587 + q.b * 0.114;
    q = mix(vec3<f32>(luma_q), q, 1.10);
    q = clamp(q, vec3<f32>(0.0), vec3<f32>(1.0));

    // Gentle distance darkening
    let dist = length(wp - g.cam_pos.xyz);
    let fog_amt = min(1.0 - exp(-dist * g.fog_params.x), g.fog_params.y);
    let fc = g.fog_color.rgb;
    return vec4<f32>(mix(q, fc, clamp(fog_amt, 0.0, 1.0)), t.a);
}
"#;

/// Textured character mesh; vertex applies **per-entity model** (relay pose).
/// `char_params`: mesh yaw, world XZ, `w` = hit flash (`0` or `100+h` for `fs_char`).
/// Skinned path uses `vs_char_skinned` + per-frame bone buffer.
const SHADER_CHAR_TEX: &str = r#"
struct CharU {
  view_proj: mat4x4<f32>,
  model: mat4x4<f32>,
  cam_pos: vec4<f32>,
  fog_color: vec4<f32>,
  fog_params: vec4<f32>,
  char_params: vec4<f32>,
  _pad1: vec4<f32>,
  _pad2: vec4<f32>,
  _pad3: vec4<f32>,
  _pad4: vec4<f32>,
}
@group(0) @binding(0) var<uniform> cu: CharU;
struct MatU { tint: vec4<f32>, }
@group(1) @binding(0) var albedo: texture_2d<f32>;
@group(1) @binding(1) var albedo_samp: sampler;
@group(1) @binding(2) var<uniform> mu: MatU;

struct Vin { @location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) norm: vec3<f32>, }
struct SkinnedVin {
  @location(0) pos: vec3<f32>,
  @location(1) uv: vec2<f32>,
  @location(2) norm: vec3<f32>,
  @location(3) joints: vec4<u32>,
  @location(4) weights: vec4<f32>,
}
struct Vout {
  @builtin(position) clip: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) world_pos: vec3<f32>,
  @location(2) world_n: vec3<f32>,
};
@group(2) @binding(0) var<storage, read> bone_matrices: array<mat4x4<f32>, 64u>;
@vertex
fn vs_char(v: Vin) -> Vout {
  let world_pos = (cu.model * vec4<f32>(v.pos, 1.0)).xyz;
  let raw_n = (cu.model * vec4<f32>(v.norm, 0.0)).xyz;
  let nl2 = dot(raw_n, raw_n);
  let world_n = select(normalize(raw_n), vec3<f32>(0.0, 1.0, 0.0), nl2 < 1e-8);
  var o: Vout;
  o.world_pos = world_pos;
  o.clip = cu.view_proj * vec4<f32>(world_pos, 1.0);
  o.uv = v.uv;
  o.world_n = world_n;
  return o;
}
@vertex
fn vs_char_skinned(v: SkinnedVin) -> Vout {
  let p = vec4<f32>(v.pos, 1.0);
  let skinned_p =
    bone_matrices[v.joints.x] * p * v.weights.x +
    bone_matrices[v.joints.y] * p * v.weights.y +
    bone_matrices[v.joints.z] * p * v.weights.z +
    bone_matrices[v.joints.w] * p * v.weights.w;
  let n = vec4<f32>(v.norm, 0.0);
  let skinned_n =
    bone_matrices[v.joints.x] * n * v.weights.x +
    bone_matrices[v.joints.y] * n * v.weights.y +
    bone_matrices[v.joints.z] * n * v.weights.z +
    bone_matrices[v.joints.w] * n * v.weights.w;
  let world_pos = (cu.model * vec4<f32>(skinned_p.xyz, 1.0)).xyz;
  let raw_n = (cu.model * vec4<f32>(skinned_n.xyz, 0.0)).xyz;
  let nl2 = dot(raw_n, raw_n);
  let world_n = select(normalize(raw_n), vec3<f32>(0.0, 1.0, 0.0), nl2 < 1e-8);
  var o: Vout;
  o.world_pos = world_pos;
  o.clip = cu.view_proj * vec4<f32>(world_pos, 1.0);
  o.uv = v.uv;
  o.world_n = world_n;
  return o;
}
fn char_oya_hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn char_bayer4(p: vec2<i32>) -> f32 {
    let xi = u32(p.x & 3);
    let yi = u32(p.y & 3);
    let idx = yi * 4u + xi;
    let m = array<f32, 16>(
        0.0, 8.0, 2.0, 10.0,
        12.0, 4.0, 14.0, 6.0,
        3.0, 11.0, 1.0, 9.0,
        15.0, 7.0, 13.0, 5.0
    );
    return m[idx];
}

@fragment
fn fs_char(i: Vout) -> @location(0) vec4<f32> {
    let anim_row = cu.char_params.w;

    var hit_mix = 0.0;
    if (anim_row > 99.0) {
        hit_mix = clamp(anim_row - 100.0, 0.0, 1.0);
    }

    let t = textureSample(albedo, albedo_samp, i.uv) * mu.tint;
    let a = t.rgb;
    let lum = a.r * 0.3 + a.g * 0.5 + a.b * 0.2;

    var wn = i.world_n;
    if (dot(wn, wn) < 1e-8) {
        wn = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        wn = normalize(wn);
    }
    let n = wn;
    let to_cam = cu.cam_pos.xyz - i.world_pos;
    let vd2 = dot(to_cam, to_cam);
    let view_dir = select(normalize(to_cam), vec3<f32>(0.0, 0.0, 1.0), vd2 < 1e-8);

    // Alley-aligned: soft cel + same posterize as world fs_tex; halftone grain (no dpdx/dpdy).
    let key_dir = normalize(vec3<f32>(-0.88, 0.35, 0.32));
    let fill_dir = normalize(vec3<f32>(0.45, 0.12, -0.42));
    let ndk = dot(n, key_dir);
    let ndf = dot(n, fill_dir);
    let shade_raw = clamp(ndk * 0.68 + ndf * 0.24 + 0.32, 0.0, 1.0);

    let px = vec2<i32>(i.clip.xy);
    let b4 = char_bayer4(px);
    let d4 = (b4 + 0.5) / 16.0 - 0.5;
    let n_cel = 4.0;
    let scaled = shade_raw * (n_cel - 1.0) + d4 * 0.28;
    let band = clamp(floor(scaled + 0.5), 0.0, n_cel - 1.0) / (n_cel - 1.0);

    let wp = i.world_pos;
    let h_norm = clamp((wp.y - 0.0) / 6.0, 0.0, 1.0);
    let ambient_warm = vec3<f32>(0.30, 0.22, 0.16) * (0.5 + 0.5 * h_norm);
    let ambient_cool = vec3<f32>(0.12, 0.20, 0.26) * (1.0 - h_norm * 0.5);
    let ambient = ambient_warm + ambient_cool;

    let neon_phase = wp.x * 0.15 + wp.z * 0.12;
    let neon_r = 0.10 * max(sin(neon_phase * 2.1 + 1.0), 0.0);
    let neon_g = 0.06 * max(sin(neon_phase * 1.7 + 3.5), 0.0);
    let neon_b = 0.12 * max(sin(neon_phase * 2.8 + 5.2), 0.0);
    let neon_spill = vec3<f32>(neon_r, neon_g, neon_b) * (1.0 - h_norm * 0.4);

    let emit_boost = max(lum - 0.35, 0.0) * 1.25;
    let al = max(a, vec3<f32>(0.03));
    let lit_char = al * (0.22 + 0.78 * band) + ambient * 0.52 * al + neon_spill * 0.46 + al * emit_boost * 1.15;

    let half_v = key_dir + view_dir;
    let h_len2 = dot(half_v, half_v);
    let half_dir = select(normalize(half_v), key_dir, h_len2 < 1e-8);
    let spec = pow(max(dot(n, half_dir), 0.0), 40.0);
    let spec_step = select(0.0, 0.26, spec > 0.42);
    var lit = lit_char + vec3<f32>(spec_step);

    let edge = 1.0 - max(dot(n, view_dir), 0.0);
    let fres = pow(edge, 2.35);
    let back_lit = max(-dot(n, key_dir), 0.0);
    let rim = smoothstep(0.14, 0.92, fres) * smoothstep(0.06, 0.70, back_lit);
    let rim_pink = vec3<f32>(0.95, 0.22, 0.52) * rim * 0.62;
    let rim_cyan = vec3<f32>(0.22, 0.88, 0.98) * fres * smoothstep(0.35, 0.98, edge) * 0.32;
    lit = lit + rim_pink + rim_cyan;

    let lit_boost = clamp(lit * vec3<f32>(1.75, 1.75, 1.75), vec3<f32>(0.0), vec3<f32>(1.0));
    var q = floor(clamp(lit_boost, vec3<f32>(0.0), vec3<f32>(1.0)) * 24.0) / 24.0;

    let fp = vec2<f32>(f32(px.x), f32(px.y));
    let hcell = floor(vec2<f32>(fp.x * 0.038, fp.y * 0.072));
    let h1 = char_oya_hash(hcell);
    let h2 = char_oya_hash(hcell + vec2<f32>(19.0, 7.0));
    let yl = dot(q, vec3<f32>(0.299, 0.587, 0.114));
    let vn = sin(fp.y * 0.095 + h1 * 6.2831853);
    let vertical_grain = 0.88 + 0.16 * smoothstep(-0.35, 0.35, vn) * (0.55 + yl * 0.35);
    let dot_mod = mix(0.92, 1.10, h2) * vertical_grain;
    q = clamp(q * dot_mod, vec3<f32>(0.02), vec3<f32>(1.0));

    let ink = smoothstep(0.68, 0.96, edge);
    q = mix(q, vec3<f32>(0.024, 0.012, 0.045), ink * 0.30);

    let floor_rgb = vec3<f32>(0.035, 0.033, 0.05);
    let poster_vis = max(q, floor_rgb);

    let flashed = mix(poster_vis, vec3<f32>(1.0, 0.28, 0.18), hit_mix * 0.62);

    let dist = length(wp - cu.cam_pos.xyz);
    let fog_amt = min(1.0 - exp(-dist * cu.fog_params.x), cu.fog_params.y);
    let fc = cu.fog_color.rgb;
    return vec4<f32>(mix(flashed, fc, clamp(fog_amt, 0.0, 1.0)), 1.0);
}
"#;

const SHADER_BILL: &str = r#"
struct Globals {
  view_proj: mat4x4<f32>,
  cam_pos: vec4<f32>,
  fog_color: vec4<f32>,
  fog_params: vec4<f32>,
  _pad: vec4<f32>,
  _pad2: vec4<f32>,
}
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct Bin { @location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) tint: vec4<f32>, };
struct Bout { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) tint: vec4<f32>, };

@vertex
fn vs_bill(v: Bin) -> Bout {
    var o: Bout;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    o.tint = v.tint;
    return o;
}

@fragment
fn fs_bill(i: Bout) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, i.uv);
    if (c.a < 0.35) { discard; }
    let d = length(i.uv - vec2<f32>(0.5, 0.5));
    let rim = smoothstep(0.72, 0.38, d) * 0.15;
    let rgb = c.rgb * i.tint.rgb * (1.05 + rim) + vec3<f32>(0.08, 0.02, 0.04) * rim * i.tint.rgb;
    return vec4<f32>(rgb, c.a * i.tint.a);
}

@fragment
fn fs_bill_blood(i: Bout) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, i.uv);
    if (c.a < 0.08) { discard; }
    let rgb = c.rgb * i.tint.rgb;
    return vec4<f32>(rgb, c.a * i.tint.a);
}
"#;

const MAX_BILL_QUADS: usize = 64;
const BILL_VERTS: usize = MAX_BILL_QUADS * 4;
const BILL_IDX: usize = MAX_BILL_QUADS * 6;
/// Boss + rival + remotes + offline demos (see draw_world batching).
const MAX_CHARACTER_INSTANCES: usize = 32;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct HudUniform {
    weapon: u32,
    flash: f32,
    bob: f32,
    recoil: f32,
    reload: f32,
    aspect: f32,
    anim_t: f32,
    /// 1.0 when drawing HUD arms over the 3D view-model (larger / lifted quad).
    fps_arms_overlay: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct HudVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

impl HudVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<HudVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

const SHADER_HUD: &str = r#"
struct Hu { weapon: u32, flash: f32, bob: f32, recoil: f32, reload: f32, aspect: f32, anim_t: f32, fps_arms: f32, }
@group(0) @binding(0) var<uniform> hu: Hu;
@group(1) @binding(0) var wtex: texture_2d<f32>;
@group(1) @binding(1) var wsamp: sampler;

struct HIn { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, }
struct HOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32>, }

fn hud_motion_offset() -> vec2<f32> {
  let inv_aspect = 1.0 / max(hu.aspect, 0.5);
  let bx = sin(hu.bob) * 0.025 * inv_aspect;
  let by = cos(hu.bob * 1.35) * 0.018;
  let recoil_y = hu.recoil * hu.recoil * 0.17;
  let recoil_x = -hu.recoil * 0.036 * inv_aspect;
  var reload_y = 0.0;
  if (hu.reload > 0.0) {
    if (hu.reload < 1.0) { reload_y = -hu.reload * 0.7; }
    else { reload_y = -(2.0 - hu.reload) * 0.7; }
  }
  return vec2<f32>(bx + recoil_x, by + recoil_y + reload_y);
}

@vertex
fn vs_hud(v: HIn) -> HOut {
  let inv_aspect = 1.0 / max(hu.aspect, 0.5);
  var p = v.pos;
  p.x = p.x * inv_aspect + 0.12 * inv_aspect;
  var o: HOut;
  o.clip = vec4<f32>(p + hud_motion_offset(), 0.0, 1.0);
  o.uv = v.uv;
  return o;
}

@vertex
fn vs_hud_arms(v: HIn) -> HOut {
  let inv_aspect = 1.0 / max(hu.aspect, 0.5);
  var p = v.pos;
  p.x = p.x * inv_aspect + 0.12 * inv_aspect;
  if (hu.fps_arms > 0.5) {
    p.x = p.x * 1.1;
    p.y = p.y * 1.12 + 0.16;
  }
  var o: HOut;
  o.clip = vec4<f32>(p + hud_motion_offset(), 0.0, 1.0);
  o.uv = v.uv;
  return o;
}

@fragment
fn fs_hud(i: HOut) -> @location(0) vec4<f32> {
  let uv_tex = vec2<f32>(i.uv.x, 1.0 - i.uv.y);
  let t = textureSample(wtex, wsamp, uv_tex);

  if (t.a < 0.10) { discard; }

  let flash_boost = 1.0 + 0.35 * hu.flash;
  return vec4<f32>(t.rgb * flash_boost, t.a);
}

@fragment
fn fs_hud_arms(i: HOut) -> @location(0) vec4<f32> {
  let uv_tex = vec2<f32>(i.uv.x, 1.0 - i.uv.y);
  let t = textureSample(wtex, wsamp, uv_tex);
  if (t.a < 0.06) { discard; }
  return vec4<f32>(t.rgb * (1.0 + 0.22 * hu.flash), t.a);
}
"#;

const SHADER_HUD_VFX: &str = r#"
struct Hu { weapon: u32, flash: f32, bob: f32, recoil: f32, reload: f32, aspect: f32, anim_t: f32, fps_arms: f32, }
@group(0) @binding(0) var<uniform> hu: Hu;
@group(1) @binding(0) var vtex: texture_2d<f32>;
@group(1) @binding(1) var vsamp: sampler;

struct HIn { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, }
struct HOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32>, }

fn hud_motion_vfx() -> vec2<f32> {
  let inv_aspect = 1.0 / max(hu.aspect, 0.5);
  let bx = sin(hu.bob) * 0.025 * inv_aspect;
  let by = cos(hu.bob * 1.35) * 0.018;
  let recoil_y = hu.recoil * hu.recoil * 0.17;
  let recoil_x = -hu.recoil * 0.036 * inv_aspect;
  var reload_y = 0.0;
  if (hu.reload > 0.0) {
    if (hu.reload < 1.0) { reload_y = -hu.reload * 0.7; }
    else { reload_y = -(2.0 - hu.reload) * 0.7; }
  }
  return vec2<f32>(bx + recoil_x, by + recoil_y + reload_y);
}

// Barrel tip in weapon HUD UV (matches weapon quad verts: u left→right, v bottom→top of sprite).
// FPS pack art has the muzzle high on the texture; older values sat mid-quad and read as "flash below gun".
// All HUD slots use the same m4a1.png sprite (tools/blender_m4a1_export_assets.py).
fn weapon_barrel_uv(_weapon: u32) -> vec2<f32> {
  return vec2<f32>(0.93, 0.87);
}

fn muzzle_quad_scale(_weapon: u32) -> f32 {
  return 0.112;
}

@vertex
fn vs_muzzle(v: HIn) -> HOut {
  let inv_aspect = 1.0 / max(hu.aspect, 0.5);
  let uv_tip = weapon_barrel_uv(hu.weapon);
  let ax = -0.5 + uv_tip.x;
  let ay = -1.05 + uv_tip.y * 1.1;
  let sc = muzzle_quad_scale(hu.weapon);
  var p = v.pos * vec2<f32>(sc, sc) + vec2<f32>(ax, ay);
  p.x = p.x * inv_aspect + 0.12 * inv_aspect;
  var o: HOut;
  o.clip = vec4<f32>(p + hud_motion_vfx(), 0.0, 1.0);
  o.uv = v.uv;
  return o;
}

@fragment
fn fs_muzzle(i: HOut) -> @location(0) vec4<f32> {
  let uv_tex = vec2<f32>(i.uv.x, 1.0 - i.uv.y);
  let t = textureSample(vtex, vsamp, uv_tex);
  let flicker = 1.0 + 0.5 * sin(hu.anim_t * 42.0);
  let pulse = 1.0 + 0.7 * hu.flash;
  let k = hu.flash * (2.85 + flicker) * pulse;
  let a = t.a * k;
  if (a < 0.02) { discard; }
  return vec4<f32>(t.rgb * k, a);
}
"#;

const SHADER_HUD_SHELL: &str = r#"
struct Hu { weapon: u32, flash: f32, bob: f32, recoil: f32, reload: f32, aspect: f32, anim_t: f32, fps_arms: f32, }
@group(0) @binding(0) var<uniform> hu: Hu;
@group(1) @binding(0) var stex: texture_2d<f32>;
@group(1) @binding(1) var ssamp: sampler;

struct HIn { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, }
struct HOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32>, }

fn hud_motion_shell() -> vec2<f32> {
  let inv_aspect = 1.0 / max(hu.aspect, 0.5);
  let bx = sin(hu.bob) * 0.025 * inv_aspect;
  let by = cos(hu.bob * 1.35) * 0.018;
  let recoil_y = hu.recoil * hu.recoil * 0.17;
  let recoil_x = -hu.recoil * 0.036 * inv_aspect;
  var reload_y = 0.0;
  if (hu.reload > 0.0) {
    if (hu.reload < 1.0) { reload_y = -hu.reload * 0.7; }
    else { reload_y = -(2.0 - hu.reload) * 0.7; }
  }
  return vec2<f32>(bx + recoil_x, by + recoil_y + reload_y);
}

@vertex
fn vs_shell(v: HIn) -> HOut {
  let inv_aspect = 1.0 / max(hu.aspect, 0.5);
  var p = v.pos;
  p.x = p.x * inv_aspect + 0.12 * inv_aspect;
  var o: HOut;
  o.clip = vec4<f32>(p + hud_motion_shell(), 0.0, 1.0);
  o.uv = v.uv;
  return o;
}

@fragment
fn fs_shell(i: HOut) -> @location(0) vec4<f32> {
  let uv_tex = vec2<f32>(i.uv.x, 1.0 - i.uv.y);
  let t = textureSample(stex, ssamp, uv_tex);
  if (t.a < 0.08) { discard; }
  return vec4<f32>(t.rgb, t.a);
}
"#;

const WEAPON_BG_LABELS: [&str; 4] = ["weapon-0", "weapon-1", "weapon-2", "weapon-3"];

fn build_hud_shell_vertices(shell: &HudShell) -> [HudVertex; 4] {
    let g = shell.life.clamp(0.2, 1.0);
    let hw = 0.028 * g;
    let hh = 0.013 * g;
    let s = shell.rot.sin();
    let c = shell.rot.cos();
    let corners: [([f32; 2], [f32; 2]); 4] = [
        ([-hw, -hh], [0.0, 0.0]),
        ([hw, -hh], [1.0, 0.0]),
        ([hw, hh], [1.0, 1.0]),
        ([-hw, hh], [0.0, 1.0]),
    ];
    let mut out = [HudVertex {
        pos: [0.0, 0.0],
        uv: [0.0, 0.0],
    }; 4];
    for (i, (lj, uv)) in corners.into_iter().enumerate() {
        let (lx, ly) = (lj[0], lj[1]);
        let rx = lx * c - ly * s + shell.x;
        let ry = lx * s + ly * c + shell.y;
        out[i] = HudVertex { pos: [rx, ry], uv };
    }
    out
}

fn acquire_surface_texture(
    surface: &wgpu::Surface<'_>,
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
    match surface.get_current_texture() {
        Ok(t) => Ok(t),
        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
            surface.configure(device, config);
            surface.get_current_texture()
        }
        Err(e) => Err(e),
    }
}

/// Snapshot for [`crate::OyabaunApp::render_debug_json`]: swapchain, world draw, fog constants.
#[derive(Serialize)]
pub struct GpuRenderDiag {
    pub surface_width: u32,
    pub surface_height: u32,
    pub surface_format: String,
    pub present_mode: String,
    pub last_swapchain_acquire_ok: bool,
    pub last_swapchain_error: String,
    pub frames_submitted: u64,
    pub frames_skipped_no_swapchain: u64,
    pub world_raster: String,
    pub world_index_count: u64,
    pub world_batch_count: u32,
    pub fog_density: f32,
    pub fog_max_blend: f32,
    pub fog_color: [f32; 4],
    pub character_boss_loaded: bool,
    pub character_rival_loaded: bool,
    pub sprite_ready: bool,
}

struct WorldBatchGpu {
    first_index: u32,
    index_count: u32,
    bind_group: wgpu::BindGroup,
}

enum WorldRaster {
    Flat {
        pipeline: wgpu::RenderPipeline,
        vb: wgpu::Buffer,
        ib: wgpu::Buffer,
        index_count: u32,
    },
    Textured {
        pipeline: wgpu::RenderPipeline,
        #[allow(dead_code)]
        material_layout: wgpu::BindGroupLayout,
        #[allow(dead_code)]
        nearest_sampler: wgpu::Sampler,
        vb: wgpu::Buffer,
        ib: wgpu::Buffer,
        batches: Vec<WorldBatchGpu>,
        #[allow(dead_code)]
        textures: Vec<wgpu::Texture>,
        #[allow(dead_code)]
        tint_buffers: Vec<wgpu::Buffer>,
    },
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MatTintUniform {
    tint: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CharUniforms {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    fog_color: [f32; 4],
    fog_params: [f32; 4],
    char_params: [f32; 4], // [mesh_yaw, world_x, world_z, anim_row]
    _p1: [f32; 4],
    _p2: [f32; 4],
    _p3: [f32; 4],
    _p4: [f32; 4],
}

enum CharacterGeometry {
    Rigid {
        pipeline: wgpu::RenderPipeline,
        vb: wgpu::Buffer,
        ib: wgpu::Buffer,
    },
    Skinned {
        pipeline: wgpu::RenderPipeline,
        vb: wgpu::Buffer,
        ib: wgpu::Buffer,
        bone_buffer: wgpu::Buffer,
        bone_bind_group: wgpu::BindGroup,
        skinned_cpu: Arc<crate::gltf_level::SkinnedCharacterMeshCpu>,
    },
}

struct CharacterDraw {
    geometry: CharacterGeometry,
    /// For skinned glTF: per-file mesh node world matrix (composed with instance model each draw).
    mesh_node_world: Mat4,
    batches: Vec<WorldBatchGpu>,
    char_uniform: wgpu::Buffer,
    char_globals_bg: wgpu::BindGroup,
    /// Byte stride per instance; multiple of `min_uniform_buffer_offset_alignment`.
    char_uniform_stride: u32,
    /// Optional second rigid pipeline (depth write off) for first-person view-model draw.
    fps_view_pipeline: Option<wgpu::RenderPipeline>,
    /// Rigid pipeline with polygon depth bias so props parented to skinned bodies win depth over the torso.
    world_attach_pipeline: Option<wgpu::RenderPipeline>,
    #[allow(dead_code)]
    _textures: Vec<wgpu::Texture>,
    #[allow(dead_code)]
    _tint_buffers: Vec<wgpu::Buffer>,
}

impl CharacterDraw {
    fn is_skinned(&self) -> bool {
        matches!(self.geometry, CharacterGeometry::Skinned { .. })
    }

    fn skinned_cpu_ref(&self) -> Option<&Arc<crate::gltf_level::SkinnedCharacterMeshCpu>> {
        match &self.geometry {
            CharacterGeometry::Skinned { skinned_cpu, .. } => Some(skinned_cpu),
            _ => None,
        }
    }
}

/// World matrix for the FPS M4.  Prop vertex extent confirms barrel along **+Z** (thinnest cross-section
/// at max-Z).  Build camera-basis rotation: prop +Z → camera forward, +Y → up.
pub fn weapon_fps_world_model(game: &crate::game::GameState) -> Mat4 {
    let eye = game.eye_pos();
    let forward = game.view_forward();
    let mut right = forward.cross(Vec3::Y);
    if right.length_squared() < 1e-8 {
        right = Vec3::new(1.0, 0.0, 0.0);
    } else {
        right = right.normalize();
    }
    let up = right.cross(forward).normalize();

    let rot = Mat4::from_cols(
        (-right).extend(0.0),     // prop +X → camera left  (RH: up × forward)
        up.extend(0.0),           // prop +Y → camera up
        forward.extend(0.0),      // prop +Z (barrel) → camera forward
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    let scale = Mat4::from_scale(Vec3::splat(0.22));
    // In prop space: -X → camera right, -Y → camera down, +Z → camera forward.
    let local = Mat4::from_translation(Vec3::new(-0.12, -0.18, 0.32));
    let tilt = Mat4::from_quat(
        Quat::from_rotation_z(0.02) * Quat::from_rotation_y(0.03),
    );
    Mat4::from_translation(eye) * rot * local * tilt * scale
}

/// Barrel is along **+Z** in the prop mesh (confirmed by vertex extent).
/// Mixamo `RightHand` during `rifle_aiming_idle`: +Y ≈ aim forward, +X ≈ up.
/// Rotate so prop +Z→hand +Y (aim), prop +Y→hand +X (up), prop +X→hand +Z,
/// then pitch barrel down ~14° to compensate for the hand's upward tilt in the anim.
/// In hand space rotation_z(+angle) tilts +Y toward −X (barrel down).
fn weapon_hand_to_prop_transform() -> Mat4 {
    let scale = Mat4::from_scale(Vec3::splat(0.38));
    let barrel_align = Mat4::from_cols(
        Vec4::new(0.0, 0.0, 1.0, 0.0), // prop +X → hand +Z
        Vec4::new(1.0, 0.0, 0.0, 0.0), // prop +Y → hand +X (up)
        Vec4::new(0.0, 1.0, 0.0, 0.0), // prop +Z → hand +Y (aim)
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    );
    let pitch_down = Mat4::from_quat(Quat::from_rotation_z(0.24));
    Mat4::from_translation(Vec3::new(-0.05, 0.12, 0.0)) * pitch_down * barrel_align * scale
}

struct WeaponAttachPass<'a> {
    weapon_cd: &'a CharacterDraw,
    hand_to_prop: Mat4,
}

fn fill_char_uniform_bytes_for_world_models(
    world_models: &[Mat4],
    mesh_node_world: Mat4,
    cd: &CharacterDraw,
    view_proj: Mat4,
    cam_pos: Vec3,
) -> Vec<u8> {
    let n = world_models.len();
    let stride = cd.char_uniform_stride as usize;
    let mut raw = vec![0u8; stride * n];
    for (i, &wm) in world_models.iter().enumerate() {
        let inst_model = wm * mesh_node_world;
        let m = inst_model.to_cols_array_2d();
        let char_x = m[3][0];
        let char_z = m[3][2];
        let u = CharUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            model: m,
            cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 0.0],
            fog_color: FOG_COLOR_RGBA,
            fog_params: [FOG_DENSITY, FOG_MAX_BLEND, 0.0, 0.0],
            char_params: [0.0, char_x, char_z, 0.0],
            _p1: [0.0; 4],
            _p2: [0.0; 4],
            _p3: [0.0; 4],
            _p4: [0.0; 4],
        };
        let dst = &mut raw[i * stride..i * stride + std::mem::size_of::<CharUniforms>()];
        dst.copy_from_slice(bytemuck::bytes_of(&u));
    }
    raw
}

fn draw_rigid_character_batch_with_pipeline(
    pass: &mut wgpu::RenderPass<'_>,
    cd: &CharacterDraw,
    pipeline: &wgpu::RenderPipeline,
    count: usize,
) {
    let stride = cd.char_uniform_stride;
    let CharacterGeometry::Rigid { vb, ib, .. } = &cd.geometry else {
        return;
    };
    pass.set_pipeline(pipeline);
    pass.set_vertex_buffer(0, vb.slice(..));
    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
    for i in 0..count {
        pass.set_bind_group(0, &cd.char_globals_bg, &[stride * i as u32]);
        for b in &cd.batches {
            pass.set_bind_group(1, &b.bind_group, &[]);
            let end = b.first_index.saturating_add(b.index_count);
            pass.draw_indexed(b.first_index..end, 0, 0..1);
        }
    }
}

fn draw_character_instances_3d(
    queue: &wgpu::Queue,
    pass: &mut wgpu::RenderPass<'_>,
    list: &[&CharacterInstance],
    cd: &CharacterDraw,
    skinned_locals_scratch: &mut Vec<Mat4>,
    skinned_bone_flat: &mut [f32],
    skinned_node_world_scratch: &mut Vec<Mat4>,
    weapon_attach: Option<WeaponAttachPass<'_>>,
    view_proj: Mat4,
    cam_pos: Vec3,
) {
    use crate::gltf_level::{compute_skinned_joint_palette, CHARACTER_MAX_JOINTS};
    let stride = cd.char_uniform_stride;
    match &cd.geometry {
        CharacterGeometry::Rigid { pipeline, vb, ib } => {
            pass.set_pipeline(pipeline);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            for i in 0..list.len() {
                pass.set_bind_group(0, &cd.char_globals_bg, &[stride * i as u32]);
                for b in &cd.batches {
                    pass.set_bind_group(1, &b.bind_group, &[]);
                    let end = b.first_index.saturating_add(b.index_count);
                    pass.draw_indexed(b.first_index..end, 0, 0..1);
                }
            }
        }
        CharacterGeometry::Skinned {
            pipeline,
            vb,
            ib,
            bone_buffer,
            bone_bind_group,
            skinned_cpu,
        } => {
            let node_n = skinned_cpu.node_parent.len();
            if skinned_locals_scratch.len() < node_n {
                skinned_locals_scratch.resize(node_n, Mat4::IDENTITY);
            }
            if skinned_node_world_scratch.len() < node_n {
                skinned_node_world_scratch.resize(node_n, Mat4::IDENTITY);
            }
            let joint_i = skinned_cpu.weapon_attach_joint.map(|j| j as usize);
            let do_attach = weapon_attach.is_some() && joint_i.is_some();
            let mut attached: Vec<Mat4> = Vec::new();
            #[cfg(target_arch = "wasm32")]
            {
                use std::sync::atomic::{AtomicU32, Ordering};
                static ATTACH_DIAG: AtomicU32 = AtomicU32::new(0);
                let c = ATTACH_DIAG.fetch_add(1, Ordering::Relaxed);
                if c < 5 {
                    let msg = format!(
                        "[weapon-attach-diag] weapon_attach={} joint_i={:?} do_attach={} node_n={} instances={}",
                        weapon_attach.is_some(), joint_i, do_attach, node_n, list.len(),
                    );
                    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&msg));
                }
            }

            pass.set_pipeline(pipeline);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_bind_group(2, bone_bind_group, &[]);
            let mut palette = [Mat4::IDENTITY; CHARACTER_MAX_JOINTS];
            for i in 0..list.len() {
                let inst = *list[i];
                compute_skinned_joint_palette(
                    skinned_cpu.as_ref(),
                    inst.skinned_clip as usize,
                    inst.skinned_anim_time,
                    &mut skinned_locals_scratch[..node_n],
                    &mut skinned_node_world_scratch[..node_n],
                    &mut palette,
                );
                for mi in 0..CHARACTER_MAX_JOINTS {
                    let cols = palette[mi].to_cols_array_2d();
                    for ci in 0..4 {
                        for ri in 0..4 {
                            skinned_bone_flat[mi * 16 + ci * 4 + ri] = cols[ci][ri];
                        }
                    }
                }
                queue.write_buffer(
                    bone_buffer,
                    0,
                    bytemuck::cast_slice(&skinned_bone_flat[..CHARACTER_MAX_JOINTS * 16]),
                );
                pass.set_bind_group(0, &cd.char_globals_bg, &[stride * i as u32]);
                for b in &cd.batches {
                    pass.set_bind_group(1, &b.bind_group, &[]);
                    let end = b.first_index.saturating_add(b.index_count);
                    pass.draw_indexed(b.first_index..end, 0, 0..1);
                }

                if do_attach {
                    if let (Some(wa), Some(ji)) = (&weapon_attach, joint_i) {
                        let jnode = skinned_cpu.joint_node_indices[ji];
                        if jnode < node_n {
                            let hand = skinned_node_world_scratch[jnode];
                            let wm = inst.model * hand * wa.hand_to_prop;
                            #[cfg(target_arch = "wasm32")]
                            {
                                use std::sync::atomic::{AtomicU32, Ordering};
                                static NPC_LOG: AtomicU32 = AtomicU32::new(0);
                                let c = NPC_LOG.fetch_add(1, Ordering::Relaxed);
                                if c < 3 {
                                    let hp = hand.to_cols_array();
                                    let wp = wm.to_cols_array();
                                    let msg = format!(
                                        "[weapon-attach] ji={} jnode={} hand_col0=[{:.3},{:.3},{:.3}] hand_col1=[{:.3},{:.3},{:.3}] hand_col2=[{:.3},{:.3},{:.3}] hand_t=[{:.3},{:.3},{:.3}] wm_t=[{:.3},{:.3},{:.3}]",
                                        ji, jnode,
                                        hp[0], hp[1], hp[2],
                                        hp[4], hp[5], hp[6],
                                        hp[8], hp[9], hp[10],
                                        hp[12], hp[13], hp[14],
                                        wp[12], wp[13], wp[14],
                                    );
                                    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&msg));
                                }
                            }
                            attached.push(wm);
                        }
                    }
                }
            }

            if do_attach {
                if let Some(wa) = weapon_attach {
                    #[cfg(target_arch = "wasm32")]
                    {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static DRAW_DIAG: AtomicU32 = AtomicU32::new(0);
                        let c = DRAW_DIAG.fetch_add(1, Ordering::Relaxed);
                        if c < 5 {
                            let has_pipe = wa.weapon_cd.world_attach_pipeline.is_some();
                            let is_rigid = matches!(&wa.weapon_cd.geometry, CharacterGeometry::Rigid { .. });
                            let msg = format!(
                                "[weapon-draw-diag] attached={} has_attach_pipe={} is_rigid={}",
                                attached.len(), has_pipe, is_rigid,
                            );
                            web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&msg));
                        }
                    }
                    if !attached.is_empty() {
                        let bytes = fill_char_uniform_bytes_for_world_models(
                            &attached,
                            wa.weapon_cd.mesh_node_world,
                            wa.weapon_cd,
                            view_proj,
                            cam_pos,
                        );
                        queue.write_buffer(&wa.weapon_cd.char_uniform, 0, bytes.as_slice());
                        if let CharacterGeometry::Rigid { pipeline, .. } = &wa.weapon_cd.geometry {
                            let pipe = wa
                                .weapon_cd
                                .world_attach_pipeline
                                .as_ref()
                                .unwrap_or(pipeline);
                            draw_rigid_character_batch_with_pipeline(
                                pass,
                                wa.weapon_cd,
                                pipe,
                                attached.len(),
                            );
                        }
                    }
                }
            }
        }
    }
}

pub struct WeaponHudParams {
    pub weapon_id: u32,
    pub bob: f32,
    pub flash: f32,
    pub recoil: f32,
    pub reload: f32,
    /// Game time (seconds) for muzzle flicker / cheap animated VFX.
    pub anim_t: f32,
    /// When set and `weapon_prop` is loaded, draws 3D view-model instead of the HUD weapon sprite.
    pub fps_weapon_model: Option<Mat4>,
}

/// World-space blood splat (camera-facing billboard; `yaw` rotates in XZ, `scale` sizes the quad).
#[derive(Clone, Copy)]
pub struct BloodSplat {
    pub pos: Vec3,
    pub life: f32,
    pub yaw: f32,
    pub scale: f32,
}

/// Ejected brass in first-person HUD space (same units as weapon quad `vs_hud` input `pos`).
#[derive(Clone, Copy, Debug)]
pub struct HudShell {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub rot: f32,
    pub spin: f32,
    pub life: f32,
}

impl HudShell {
    pub fn new(x: f32, y: f32, vx: f32, vy: f32, spin: f32) -> Self {
        Self {
            x,
            y,
            vx,
            vy,
            rot: 0.0,
            spin,
            life: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterSkin {
    Boss,
    Rival,
    /// Other networked players (same `yakuza_shooter.glb` mesh as boss when skinned).
    Remote,
}

#[derive(Clone, Copy)]
pub struct CharacterInstance {
    pub model: Mat4,
    pub mesh_yaw: f32,
    pub skin: CharacterSkin,
    /// Rigid/3D fragment: `0` normal; `100+h` (h∈[0,1]) = hit flash mix (`fs_char`). Ignored for skinning pose.
    pub anim_frame: f32,
    /// Skinned glTF: clip index in `SkinnedCharacterMeshCpu::clips` (ignored for rigid mesh).
    pub skinned_clip: u32,
    /// Skinned glTF: local time within clip (seconds).
    pub skinned_anim_time: f32,
}

pub struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    world: WorldRaster,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Mural / reference backdrop (and later: world-placed signage textures).
    bill_pipeline: wgpu::RenderPipeline,
    bill_blood_pipeline: wgpu::RenderPipeline,
    bill_tex: wgpu::Texture,
    bill_view: wgpu::TextureView,
    bill_sampler: wgpu::Sampler,
    bill_bind_group: wgpu::BindGroup,
    sprite_layout: wgpu::BindGroupLayout,
    bill_vb: wgpu::Buffer,
    bill_ib: wgpu::Buffer,
    pub sprite_ready: bool,
    hud_pipeline: wgpu::RenderPipeline,
    hud_arms_pipeline: wgpu::RenderPipeline,
    hud_uniform: wgpu::Buffer,
    hud_bind_group: wgpu::BindGroup,
    hud_vb: wgpu::Buffer,
    hud_ib: wgpu::Buffer,
    weapon_textures: Vec<wgpu::Texture>,
    weapon_bind_groups: Vec<wgpu::BindGroup>,
    arms_texture: wgpu::Texture,
    arms_bind_group: wgpu::BindGroup,
    pub arms_ready: bool,
    vfx_sampler: wgpu::Sampler,
    hud_muzzle_pipeline: wgpu::RenderPipeline,
    vfx_muzzle_bg: Option<wgpu::BindGroup>,
    _vfx_muzzle_tex: Option<wgpu::Texture>,
    vfx_blood_bg: Option<wgpu::BindGroup>,
    _vfx_blood_tex: Option<wgpu::Texture>,
    vfx_muzzle_vb: wgpu::Buffer,
    pub vfx_muzzle_ready: bool,
    pub vfx_blood_ready: bool,
    hud_shell_pipeline: wgpu::RenderPipeline,
    shell_vb: wgpu::Buffer,
    vfx_shell_bg: Option<wgpu::BindGroup>,
    _vfx_shell_tex: Option<wgpu::Texture>,
    pub vfx_shell_ready: bool,
    character: Option<CharacterDraw>,
    character_rival: Option<CharacterDraw>,
    /// Rigid `m4a1_prop.glb` for FPS view + hand attach on skinned NPCs.
    weapon_prop: Option<CharacterDraw>,
    diag_last_surface_ok: bool,
    diag_last_surface_error: String,
    diag_frames_submitted: u64,
    diag_frames_skipped_swapchain: u64,
    skinned_locals_scratch: Vec<Mat4>,
    skinned_node_world_scratch: Vec<Mat4>,
    skinned_bone_flat: Vec<f32>,
    pub skinned_anim_ids: Option<crate::gltf_level::SkinnedAnimClipIds>,
}

impl Gpu {
    pub async fn new(
        canvas: HtmlCanvasElement,
        flat_vertices: &[Vertex],
        flat_indices: &[u32],
        gltf_level: Option<crate::gltf_level::GltfLevelCpu>,
        character_level: Option<crate::gltf_level::CharacterGltfCpu>,
        character_rival_level: Option<crate::gltf_level::CharacterGltfCpu>,
        weapon_prop_level: Option<crate::gltf_level::CharacterGltfCpu>,
    ) -> Result<Self, wasm_bindgen::JsValue> {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| wasm_bindgen::JsValue::from_str(&format!("surface: {e}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| wasm_bindgen::JsValue::from_str("no adapter"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("oyabaun"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .map_err(|e| wasm_bindgen::JsValue::from_str(&format!("device: {e}")))?;

        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| wasm_bindgen::JsValue::from_str("no surface config"))?;
        config.format = format;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
        surface.configure(&device, &config);

        device.on_uncaptured_error(Box::new(|e| {
            #[cfg(target_arch = "wasm32")]
            web_sys::console::error_1(
                &wasm_bindgen::JsValue::from_str(&format!("wgpu uncaptured error: {e:?}")),
            );
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("wgpu uncaptured error: {e:?}");
        }));

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals-bg"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        let world = if let Some(cpu) = gltf_level {
            if cpu.vertices.is_empty() {
                return Err(wasm_bindgen::JsValue::from_str("glTF level has no vertices"));
            }
            Self::raster_from_gltf(&device, &queue, format, &bind_layout, cpu)?
        } else {
            Self::raster_flat(&device, format, &bind_layout, flat_vertices, flat_indices)?
        };

        let (bill_tex, bill_view) = make_placeholder_sprite(&device, &queue, 4, 4);
        let bill_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let vfx_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("vfx-pixel"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let sprite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tex"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bill_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bill-bg"),
            layout: &sprite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&bill_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&bill_sampler),
                },
            ],
        });

        let mut weapon_textures: Vec<wgpu::Texture> = Vec::with_capacity(4);
        let mut weapon_bind_groups: Vec<wgpu::BindGroup> = Vec::with_capacity(4);
        for slot in 0..4 {
            let (tex, view) = make_transparent_tex(&device, &queue);
            weapon_textures.push(tex);
            weapon_bind_groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(WEAPON_BG_LABELS[slot]),
                layout: &sprite_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&bill_sampler),
                    },
                ],
            }));
        }

        let (arms_texture, arms_view) = make_transparent_tex(&device, &queue);
        let arms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arms-bg"),
            layout: &sprite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&arms_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&bill_sampler),
                },
            ],
        });

        let shader_bill = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bill"),
            source: wgpu::ShaderSource::Wgsl(SHADER_BILL.into()),
        });

        let bill_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl-bill"),
            bind_group_layouts: &[&bind_layout, &sprite_layout],
            push_constant_ranges: &[],
        });

        let bill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bill"),
            layout: Some(&bill_pl),
            vertex: wgpu::VertexState {
                module: &shader_bill,
                entry_point: Some("vs_bill"),
                buffers: &[BillVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_bill,
                entry_point: Some("fs_bill"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let bill_blood_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bill-blood"),
            layout: Some(&bill_pl),
            vertex: wgpu::VertexState {
                module: &shader_bill,
                entry_point: Some("vs_bill"),
                buffers: &[BillVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_bill,
                entry_point: Some("fs_bill_blood"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let bill_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bill-vb"),
            size: (std::mem::size_of::<BillVertex>() * BILL_VERTS) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut idx: Vec<u32> = Vec::with_capacity(BILL_IDX);
        for q in 0..MAX_BILL_QUADS as u32 {
            let b = q * 4;
            idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        let bill_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bill-ib"),
            contents: bytemuck::cast_slice(&idx),
            usage: wgpu::BufferUsages::INDEX,
        });

        let hud_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hud-u"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let hud_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud-uniform"),
            size: std::mem::size_of::<HudUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hud_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hud-bg"),
            layout: &hud_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: hud_uniform.as_entire_binding(),
            }],
        });
        let shader_hud = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hud"),
            source: wgpu::ShaderSource::Wgsl(SHADER_HUD.into()),
        });
        let hud_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl-hud"),
            bind_group_layouts: &[&hud_layout, &sprite_layout],
            push_constant_ranges: &[],
        });
        let hud_color_targets = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud"),
            layout: Some(&hud_pl),
            vertex: wgpu::VertexState {
                module: &shader_hud,
                entry_point: Some("vs_hud"),
                buffers: &[HudVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_hud,
                entry_point: Some("fs_hud"),
                targets: &hud_color_targets,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let hud_arms_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud-arms"),
            layout: Some(&hud_pl),
            vertex: wgpu::VertexState {
                module: &shader_hud,
                entry_point: Some("vs_hud_arms"),
                buffers: &[HudVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_hud,
                entry_point: Some("fs_hud_arms"),
                targets: &hud_color_targets,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        // Weapon HUD quad — larger coverage, aspect correction done in shader
        let hud_verts: [HudVertex; 4] = [
            HudVertex {
                pos: [-0.50, -1.05],
                uv: [0.0, 0.0],
            },
            HudVertex {
                pos: [0.50, -1.05],
                uv: [1.0, 0.0],
            },
            HudVertex {
                pos: [0.50, 0.05],
                uv: [1.0, 1.0],
            },
            HudVertex {
                pos: [-0.50, 0.05],
                uv: [0.0, 1.0],
            },
        ];
        let hud_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hud-vb"),
            contents: bytemuck::cast_slice(&hud_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let hud_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("hud-ib"),
            contents: bytemuck::cast_slice(&[0u32, 1, 2, 0, 2, 3]),
            usage: wgpu::BufferUsages::INDEX,
        });

        let shader_hud_vfx = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hud-vfx"),
            source: wgpu::ShaderSource::Wgsl(SHADER_HUD_VFX.into()),
        });
        let hud_muzzle_targets = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let hud_muzzle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud-muzzle"),
            layout: Some(&hud_pl),
            vertex: wgpu::VertexState {
                module: &shader_hud_vfx,
                entry_point: Some("vs_muzzle"),
                buffers: &[HudVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_hud_vfx,
                entry_point: Some("fs_muzzle"),
                targets: &hud_muzzle_targets,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let vfx_muzzle_verts: [HudVertex; 4] = [
            HudVertex {
                pos: [-0.5, -0.5],
                uv: [0.0, 0.0],
            },
            HudVertex {
                pos: [0.5, -0.5],
                uv: [1.0, 0.0],
            },
            HudVertex {
                pos: [0.5, 0.5],
                uv: [1.0, 1.0],
            },
            HudVertex {
                pos: [-0.5, 0.5],
                uv: [0.0, 1.0],
            },
        ];
        let vfx_muzzle_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vfx-muzzle-vb"),
            contents: bytemuck::cast_slice(&vfx_muzzle_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let shader_hud_shell = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hud-shell"),
            source: wgpu::ShaderSource::Wgsl(SHADER_HUD_SHELL.into()),
        });
        let hud_shell_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud-shell"),
            layout: Some(&hud_pl),
            vertex: wgpu::VertexState {
                module: &shader_hud_shell,
                entry_point: Some("vs_shell"),
                buffers: &[HudVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_hud_shell,
                entry_point: Some("fs_shell"),
                targets: &hud_color_targets,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let shell_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shell-vb"),
            size: (14 * 4 * std::mem::size_of::<HudVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let try_raster_char =
            |cpu: crate::gltf_level::CharacterGltfCpu, label: &str| -> Option<CharacterDraw> {
                match cpu {
                    crate::gltf_level::CharacterGltfCpu::Rigid(c) => {
                        if c.vertices.is_empty() || c.indices.is_empty() || c.batches.is_empty() {
                            #[cfg(target_arch = "wasm32")]
                            warn_str(&format!("oyabaun: {label} has no drawable geometry"));
                            return None;
                        }
                        match Self::raster_character_gltf(&device, &queue, format, c, false) {
                            Ok(cd) => Some(cd),
                            Err(e) => {
                                #[cfg(target_arch = "wasm32")]
                                warn_str(&format!(
                                    "oyabaun: {label} GPU init failed ({e:?}) — rebuild client/characters/*.glb"
                                ));
                                None
                            }
                        }
                    }
                    crate::gltf_level::CharacterGltfCpu::Skinned(c) => {
                        if c.vertices.is_empty() || c.indices.is_empty() || c.batches.is_empty() {
                            #[cfg(target_arch = "wasm32")]
                            warn_str(&format!("oyabaun: {label} (skinned) has no drawable geometry"));
                            return None;
                        }
                        let arc = Arc::new(c);
                        match Self::raster_skinned_character_gltf(&device, &queue, format, arc) {
                            Ok(cd) => Some(cd),
                            Err(e) => {
                                #[cfg(target_arch = "wasm32")]
                                warn_str(&format!(
                                    "oyabaun: {label} skinned GPU init failed ({e:?}) — check client/characters/*.glb"
                                ));
                                None
                            }
                        }
                    }
                }
            };

        #[cfg(target_arch = "wasm32")]
        if character_level.is_none() {
            warn_str("oyabaun: no yakuza_shooter.glb / character parsed — wasm-pack build after adding client/characters/*.glb");
        }
        let character = character_level.and_then(|cpu| try_raster_char(cpu, "yakuza_shooter.glb"));

        let character_rival =
            character_rival_level.and_then(|cpu| try_raster_char(cpu, "oyabaun_rival.glb"));

        let weapon_prop = weapon_prop_level.and_then(|cpu| match cpu {
            crate::gltf_level::CharacterGltfCpu::Rigid(c) => {
                if c.vertices.is_empty() || c.indices.is_empty() || c.batches.is_empty() {
                    #[cfg(target_arch = "wasm32")]
                    warn_str("oyabaun: m4a1_prop.glb has no drawable geometry");
                    None
                } else {
                    Self::raster_character_gltf(&device, &queue, format, c, true).ok()
                }
            }
            _ => None,
        });

        let mut skinned_locals_scratch = Vec::new();
        let skinned_anim_ids = character.as_ref().and_then(|cd| {
            cd.skinned_cpu_ref().map(|arc| {
                skinned_locals_scratch.resize(arc.node_parent.len(), Mat4::IDENTITY);
                crate::gltf_level::resolve_skinned_clip_indices(&arc.clips)
            })
        });
        let skinned_bone_flat = vec![
            0.0_f32;
            crate::gltf_level::CHARACTER_MAX_JOINTS * 16
        ];

        let (depth, depth_view) = create_depth(&device, width, height);

        Ok(Gpu {
            surface,
            device,
            queue,
            config,
            depth,
            depth_view,
            world,
            uniform,
            bind_group,
            bill_pipeline,
            bill_blood_pipeline,
            bill_tex,
            bill_view,
            bill_sampler,
            bill_bind_group,
            sprite_layout,
            bill_vb,
            bill_ib,
            sprite_ready: false,
            hud_pipeline,
            hud_arms_pipeline,
            hud_uniform,
            hud_bind_group,
            hud_vb,
            hud_ib,
            weapon_textures,
            weapon_bind_groups,
            arms_texture,
            arms_bind_group,
            arms_ready: false,
            vfx_sampler,
            hud_muzzle_pipeline,
            vfx_muzzle_bg: None,
            _vfx_muzzle_tex: None,
            vfx_blood_bg: None,
            _vfx_blood_tex: None,
            vfx_muzzle_vb,
            vfx_muzzle_ready: false,
            vfx_blood_ready: false,
            hud_shell_pipeline,
            shell_vb,
            vfx_shell_bg: None,
            _vfx_shell_tex: None,
            vfx_shell_ready: false,
            character,
            character_rival,
            weapon_prop,
            diag_last_surface_ok: true,
            diag_last_surface_error: String::new(),
            diag_frames_submitted: 0,
            diag_frames_skipped_swapchain: 0,
            skinned_locals_scratch,
            skinned_node_world_scratch: Vec::new(),
            skinned_bone_flat,
            skinned_anim_ids,
        })
    }

    pub fn weapon_prop_loaded(&self) -> bool {
        self.weapon_prop.is_some()
    }

    pub fn skinned_character_active(&self) -> bool {
        self.character.as_ref().is_some_and(CharacterDraw::is_skinned)
    }

    pub fn skinned_anim_ids(&self) -> Option<&crate::gltf_level::SkinnedAnimClipIds> {
        self.skinned_anim_ids.as_ref()
    }

    pub fn render_diag(&self) -> GpuRenderDiag {
        let (world_raster, world_index_count, world_batch_count) = match &self.world {
            WorldRaster::Flat { index_count, .. } => {
                ("flat".to_string(), *index_count as u64, 0u32)
            }
            WorldRaster::Textured { batches, .. } => {
                let ic: u64 = batches.iter().map(|b| b.index_count as u64).sum();
                ("textured".to_string(), ic, batches.len() as u32)
            }
        };
        GpuRenderDiag {
            surface_width: self.config.width,
            surface_height: self.config.height,
            surface_format: format!("{:?}", self.config.format),
            present_mode: format!("{:?}", self.config.present_mode),
            last_swapchain_acquire_ok: self.diag_last_surface_ok,
            last_swapchain_error: self.diag_last_surface_error.clone(),
            frames_submitted: self.diag_frames_submitted,
            frames_skipped_no_swapchain: self.diag_frames_skipped_swapchain,
            world_raster,
            world_index_count,
            world_batch_count,
            fog_density: FOG_DENSITY,
            fog_max_blend: FOG_MAX_BLEND,
            fog_color: FOG_COLOR_RGBA,
            character_boss_loaded: self.character.is_some(),
            character_rival_loaded: self.character_rival.is_some(),
            sprite_ready: self.sprite_ready,
        }
    }

    fn raster_character_gltf(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        cpu: crate::gltf_level::CharacterMeshCpu,
        with_fps_view_pipeline: bool,
    ) -> Result<CharacterDraw, wasm_bindgen::JsValue> {
        use crate::gltf_level::CharacterVertex;

        let char_struct_size = std::mem::size_of::<CharUniforms>();
        let align = device.limits().min_uniform_buffer_offset_alignment as usize;
        let char_uniform_stride =
            ((char_struct_size + align - 1) / align * align) as u32;
        debug_assert!(char_struct_size as u32 <= char_uniform_stride);

        let char_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("char-globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(char_struct_size as u64),
                },
                count: None,
            }],
        });

        let char_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("char-u"),
            size: u64::from(char_uniform_stride) * MAX_CHARACTER_INSTANCES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let char_globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("char-globals-bg"),
            layout: &char_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &char_uniform,
                    offset: 0,
                    size: std::num::NonZeroU64::new(char_struct_size as u64),
                }),
            }],
        });

        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("char-nearest"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("char-mat"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
            ],
        });

        let char_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl-char-tex"),
            bind_group_layouts: &[&char_layout, &material_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("char-tex"),
            source: wgpu::ShaderSource::Wgsl(SHADER_CHAR_TEX.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("char-tex-rigid"),
            layout: Some(&char_pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_char"),
                buffers: &[CharacterVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_char"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true, // solid 3D models need depth write
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let fps_view_pipeline = if with_fps_view_pipeline {
            Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("char-tex-rigid-fps"),
                layout: Some(&char_pl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_char"),
                    buffers: &[CharacterVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_char"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24Plus,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            }))
        } else {
            None
        };

        let world_attach_pipeline = if with_fps_view_pipeline {
            Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("char-tex-rigid-attach-bias"),
                layout: Some(&char_pl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_char"),
                    buffers: &[CharacterVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_char"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24Plus,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState {
                        constant: -200,
                        slope_scale: -8.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            }))
        } else {
            None
        };

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vb-char"),
            contents: bytemuck::cast_slice(&cpu.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ib-char"),
            contents: bytemuck::cast_slice(&cpu.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut textures: Vec<wgpu::Texture> = Vec::new();
        let mut views: Vec<wgpu::TextureView> = Vec::new();
        for (wi, (w, h, rgba)) in cpu.images_rgba8.iter().enumerate() {
            let width = (*w).max(1);
            let height = (*h).max(1);
            if width > 4096 || height > 4096 {
                return Err(wasm_bindgen::JsValue::from_str(&format!(
                    "character glTF image {} exceeds 4096 (got {}×{})",
                    wi, width, height
                )));
            }
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("char-img-{wi}")),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba.as_slice(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            views.push(tex.create_view(&wgpu::TextureViewDescriptor::default()));
            textures.push(tex);
        }

        let white_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("char-white"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &white_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white_view = white_tex.create_view(&wgpu::TextureViewDescriptor::default());
        textures.push(white_tex);

        let mut batches = Vec::with_capacity(cpu.batches.len());
        let mut tint_buffers = Vec::with_capacity(cpu.batches.len());
        for b in &cpu.batches {
            let view_ref: &wgpu::TextureView = if b.image_index < views.len() {
                &views[b.image_index]
            } else {
                &white_view
            };
            let tint = MatTintUniform { tint: b.tint };
            let tint_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("char-tint"),
                contents: bytemuck::bytes_of(&tint),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("char-mat-bg"),
                layout: &material_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view_ref),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&nearest_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: tint_buf.as_entire_binding(),
                    },
                ],
            });
            tint_buffers.push(tint_buf);
            batches.push(WorldBatchGpu {
                first_index: b.first_index,
                index_count: b.index_count,
                bind_group,
            });
        }

        Ok(CharacterDraw {
            geometry: CharacterGeometry::Rigid { pipeline, vb, ib },
            mesh_node_world: Mat4::IDENTITY,
            batches,
            char_uniform,
            char_globals_bg,
            char_uniform_stride,
            fps_view_pipeline,
            world_attach_pipeline,
            _textures: textures,
            _tint_buffers: tint_buffers,
        })
    }

    fn raster_skinned_character_gltf(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        arc: Arc<crate::gltf_level::SkinnedCharacterMeshCpu>,
    ) -> Result<CharacterDraw, wasm_bindgen::JsValue> {
        use crate::gltf_level::{compute_skinned_joint_palette, SkinnedCharacterVertex, CHARACTER_MAX_JOINTS};

        let cpu = arc.as_ref();

        let char_struct_size = std::mem::size_of::<CharUniforms>();
        let align = device.limits().min_uniform_buffer_offset_alignment as usize;
        let char_uniform_stride =
            ((char_struct_size + align - 1) / align * align) as u32;
        debug_assert!(char_struct_size as u32 <= char_uniform_stride);

        let char_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("char-globals-skinned"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(char_struct_size as u64),
                },
                count: None,
            }],
        });

        let char_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("char-u-skinned"),
            size: u64::from(char_uniform_stride) * MAX_CHARACTER_INSTANCES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let char_globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("char-globals-bg-skinned"),
            layout: &char_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &char_uniform,
                    offset: 0,
                    size: std::num::NonZeroU64::new(char_struct_size as u64),
                }),
            }],
        });

        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("char-nearest-skinned"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("char-mat-skinned"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
            ],
        });

        let bone_sz = (CHARACTER_MAX_JOINTS * std::mem::size_of::<Mat4>()) as u64;
        let bone_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("char-bones"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(bone_sz),
                },
                count: None,
            }],
        });

        let char_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl-char-skinned"),
            bind_group_layouts: &[&char_layout, &material_layout, &bone_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("char-skinned"),
            source: wgpu::ShaderSource::Wgsl(SHADER_CHAR_TEX.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("char-tex-skinned"),
            layout: Some(&char_pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_char_skinned"),
                buffers: &[SkinnedCharacterVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_char"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let bone_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("char-bones-buf"),
            size: bone_sz,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let clip_ids = crate::gltf_level::resolve_skinned_clip_indices(&cpu.clips);
        let mut loc = vec![Mat4::IDENTITY; cpu.node_parent.len()];
        let mut nw = vec![Mat4::IDENTITY; cpu.node_parent.len()];
        let mut pal = [Mat4::IDENTITY; CHARACTER_MAX_JOINTS];
        compute_skinned_joint_palette(
            cpu,
            clip_ids.idle as usize,
            0.0,
            &mut loc,
            &mut nw,
            &mut pal,
        );
        let mut bone_f32 = vec![0.0_f32; CHARACTER_MAX_JOINTS * 16];
        for (mi, mat) in pal.iter().enumerate() {
            let cols = mat.to_cols_array_2d();
            for ci in 0..4 {
                for ri in 0..4 {
                    bone_f32[mi * 16 + ci * 4 + ri] = cols[ci][ri];
                }
            }
        }
        queue.write_buffer(&bone_buffer, 0, bytemuck::cast_slice(&bone_f32));

        let bone_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("char-bones-bg"),
            layout: &bone_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bone_buffer.as_entire_binding(),
            }],
        });

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vb-char-skinned"),
            contents: bytemuck::cast_slice(&cpu.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ib-char-skinned"),
            contents: bytemuck::cast_slice(&cpu.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut textures: Vec<wgpu::Texture> = Vec::new();
        let mut views: Vec<wgpu::TextureView> = Vec::new();
        for (wi, (w, h, rgba)) in cpu.images_rgba8.iter().enumerate() {
            let width = (*w).max(1);
            let height = (*h).max(1);
            if width > 4096 || height > 4096 {
                return Err(wasm_bindgen::JsValue::from_str(&format!(
                    "character glTF image {} exceeds 4096 (got {}×{})",
                    wi, width, height
                )));
            }
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("char-skinned-img-{wi}")),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba.as_slice(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            views.push(tex.create_view(&wgpu::TextureViewDescriptor::default()));
            textures.push(tex);
        }

        let white_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("char-skinned-white"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &white_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white_view = white_tex.create_view(&wgpu::TextureViewDescriptor::default());
        textures.push(white_tex);

        let mut batches = Vec::with_capacity(cpu.batches.len());
        let mut tint_buffers = Vec::with_capacity(cpu.batches.len());
        for b in &cpu.batches {
            let view_ref: &wgpu::TextureView = if b.image_index < views.len() {
                &views[b.image_index]
            } else {
                &white_view
            };
            let tint = MatTintUniform { tint: b.tint };
            let tint_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("char-skinned-tint"),
                contents: bytemuck::bytes_of(&tint),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("char-skinned-mat-bg"),
                layout: &material_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view_ref),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&nearest_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: tint_buf.as_entire_binding(),
                    },
                ],
            });
            tint_buffers.push(tint_buf);
            batches.push(WorldBatchGpu {
                first_index: b.first_index,
                index_count: b.index_count,
                bind_group,
            });
        }

        let mesh_node_world = cpu.mesh_node_world;

        Ok(CharacterDraw {
            geometry: CharacterGeometry::Skinned {
                pipeline,
                vb,
                ib,
                bone_buffer,
                bone_bind_group,
                skinned_cpu: arc,
            },
            mesh_node_world,
            batches,
            char_uniform,
            char_globals_bg,
            char_uniform_stride,
            fps_view_pipeline: None,
            world_attach_pipeline: None,
            _textures: textures,
            _tint_buffers: tint_buffers,
        })
    }

    fn raster_flat(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals_layout: &wgpu::BindGroupLayout,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> Result<WorldRaster, wasm_bindgen::JsValue> {
        let shader_world = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world-flat"),
            source: wgpu::ShaderSource::Wgsl(SHADER_WORLD.into()),
        });
        let world_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl-world-flat"),
            bind_group_layouts: &[globals_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("world-flat"),
            layout: Some(&world_layout),
            vertex: wgpu::VertexState {
                module: &shader_world,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_world,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vb-flat"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ib-flat"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Ok(WorldRaster::Flat {
            pipeline,
            vb,
            ib,
            index_count: indices.len() as u32,
        })
    }

    fn raster_from_gltf(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        globals_layout: &wgpu::BindGroupLayout,
        cpu: crate::gltf_level::GltfLevelCpu,
    ) -> Result<WorldRaster, wasm_bindgen::JsValue> {
        use crate::gltf_level::WorldVertex;

        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("world-nearest"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world-mat"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(16),
                    },
                    count: None,
                },
            ],
        });

        let world_tex_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl-world-tex"),
            bind_group_layouts: &[globals_layout, &material_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world-tex"),
            source: wgpu::ShaderSource::Wgsl(SHADER_WORLD_TEX.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("world-tex"),
            layout: Some(&world_tex_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_tex"),
                buffers: &[WorldVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_tex"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vb-gltf"),
            contents: bytemuck::cast_slice(&cpu.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ib-gltf"),
            contents: bytemuck::cast_slice(&cpu.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut textures: Vec<wgpu::Texture> = Vec::new();
        let mut views: Vec<wgpu::TextureView> = Vec::new();
        for (wi, (w, h, rgba)) in cpu.images_rgba8.iter().enumerate() {
            let width = (*w).max(1);
            let height = (*h).max(1);
            if width > 4096 || height > 4096 {
                return Err(wasm_bindgen::JsValue::from_str(&format!(
                    "glTF image {} exceeds 4096 (got {}×{})",
                    wi, width, height
                )));
            }
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("gltf-img-{wi}")),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba.as_slice(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            views.push(tex.create_view(&wgpu::TextureViewDescriptor::default()));
            textures.push(tex);
        }

        let white_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gltf-white"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &white_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white_view = white_tex.create_view(&wgpu::TextureViewDescriptor::default());
        textures.push(white_tex);

        let mut batches = Vec::with_capacity(cpu.batches.len());
        let mut tint_buffers = Vec::with_capacity(cpu.batches.len());
        for b in &cpu.batches {
            let view_ref: &wgpu::TextureView = if b.image_index < views.len() {
                &views[b.image_index]
            } else {
                &white_view
            };
            let tint = MatTintUniform { tint: b.tint };
            let tint_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("gltf-tint"),
                contents: bytemuck::bytes_of(&tint),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gltf-mat-bg"),
                layout: &material_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view_ref),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&nearest_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: tint_buf.as_entire_binding(),
                    },
                ],
            });
            tint_buffers.push(tint_buf);
            batches.push(WorldBatchGpu {
                first_index: b.first_index,
                index_count: b.index_count,
                bind_group,
            });
        }

        Ok(WorldRaster::Textured {
            pipeline,
            material_layout,
            nearest_sampler,
            vb,
            ib,
            batches,
            textures,
            tint_buffers,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            let (d, dv) = create_depth(&self.device, width, height);
            self.depth = d;
            self.depth_view = dv;
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn upload_reference_sprite(&mut self, img: &web_sys::HtmlImageElement) -> Result<(), wasm_bindgen::JsValue> {
        let w = img.width().max(1);
        let h = img.height().max(1);
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ref-sprite"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let src = ImageCopyExternalImage {
            source: ExternalImageSource::HTMLImageElement(img.clone()),
            origin: Origin2d::ZERO,
            flip_y: false,
        };
        let dst = wgpu::ImageCopyTextureTagged {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
            color_space: wgpu::PredefinedColorSpace::Srgb,
            premultiplied_alpha: false,
        };
        self.queue.copy_external_image_to_texture(
            &src,
            dst,
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.bill_tex = tex;
        self.bill_view = view;
        self.bill_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bill-bg2"),
            layout: &self.sprite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.bill_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.bill_sampler),
                },
            ],
        });
        self.sprite_ready = true;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn upload_reference_sprite(&mut self, _img: &web_sys::HtmlImageElement) -> Result<(), wasm_bindgen::JsValue> {
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn upload_weapon_sprite(
        &mut self,
        slot: u32,
        img: &web_sys::HtmlImageElement,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let slot = slot as usize;
        if slot >= self.weapon_bind_groups.len() {
            return Err(wasm_bindgen::JsValue::from_str("weapon slot must be 0..4"));
        }
        let w = img.width().max(1);
        let h = img.height().max(1);
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("weapon-sprite"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let src = ImageCopyExternalImage {
            source: ExternalImageSource::HTMLImageElement(img.clone()),
            origin: Origin2d::ZERO,
            flip_y: false,
        };
        let dst = wgpu::ImageCopyTextureTagged {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
            color_space: wgpu::PredefinedColorSpace::Srgb,
            premultiplied_alpha: false,
        };
        self.queue.copy_external_image_to_texture(
            &src,
            dst,
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(WEAPON_BG_LABELS[slot]),
            layout: &self.sprite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.bill_sampler),
                },
            ],
        });
        self.weapon_textures[slot] = tex;
        self.weapon_bind_groups[slot] = bg;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn upload_arms_sprite(&mut self, img: &web_sys::HtmlImageElement) -> Result<(), wasm_bindgen::JsValue> {
        let w = img.width().max(1);
        let h = img.height().max(1);
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("arms-sprite"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let src = ImageCopyExternalImage {
            source: ExternalImageSource::HTMLImageElement(img.clone()),
            origin: Origin2d::ZERO,
            flip_y: false,
        };
        let dst = wgpu::ImageCopyTextureTagged {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
            color_space: wgpu::PredefinedColorSpace::Srgb,
            premultiplied_alpha: false,
        };
        self.queue.copy_external_image_to_texture(
            &src,
            dst,
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arms-bg"),
            layout: &self.sprite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.bill_sampler),
                },
            ],
        });
        self.arms_texture = tex;
        self.arms_bind_group = bg;
        self.arms_ready = true;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn upload_vfx_muzzle_sprite(
        &mut self,
        img: &web_sys::HtmlImageElement,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let w = img.width().max(1);
        let h = img.height().max(1);
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vfx-muzzle"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let src = ImageCopyExternalImage {
            source: ExternalImageSource::HTMLImageElement(img.clone()),
            origin: Origin2d::ZERO,
            flip_y: false,
        };
        let dst = wgpu::ImageCopyTextureTagged {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
            color_space: wgpu::PredefinedColorSpace::Srgb,
            premultiplied_alpha: false,
        };
        self.queue.copy_external_image_to_texture(
            &src,
            dst,
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vfx-muzzle-bg"),
            layout: &self.sprite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.vfx_sampler),
                },
            ],
        });
        self._vfx_muzzle_tex = Some(tex);
        self.vfx_muzzle_bg = Some(bg);
        self.vfx_muzzle_ready = true;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn upload_vfx_blood_sprite(
        &mut self,
        img: &web_sys::HtmlImageElement,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let w = img.width().max(1);
        let h = img.height().max(1);
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vfx-blood"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let src = ImageCopyExternalImage {
            source: ExternalImageSource::HTMLImageElement(img.clone()),
            origin: Origin2d::ZERO,
            flip_y: false,
        };
        let dst = wgpu::ImageCopyTextureTagged {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
            color_space: wgpu::PredefinedColorSpace::Srgb,
            premultiplied_alpha: false,
        };
        self.queue.copy_external_image_to_texture(
            &src,
            dst,
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vfx-blood-bg"),
            layout: &self.sprite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.vfx_sampler),
                },
            ],
        });
        self._vfx_blood_tex = Some(tex);
        self.vfx_blood_bg = Some(bg);
        self.vfx_blood_ready = true;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn upload_vfx_shell_sprite(
        &mut self,
        img: &web_sys::HtmlImageElement,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let w = img.width().max(1);
        let h = img.height().max(1);
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vfx-shell"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let src = ImageCopyExternalImage {
            source: ExternalImageSource::HTMLImageElement(img.clone()),
            origin: Origin2d::ZERO,
            flip_y: false,
        };
        let dst = wgpu::ImageCopyTextureTagged {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
            color_space: wgpu::PredefinedColorSpace::Srgb,
            premultiplied_alpha: false,
        };
        self.queue.copy_external_image_to_texture(
            &src,
            dst,
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vfx-shell-bg"),
            layout: &self.sprite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.vfx_sampler),
                },
            ],
        });
        self._vfx_shell_tex = Some(tex);
        self.vfx_shell_bg = Some(bg);
        self.vfx_shell_ready = true;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn upload_arms_sprite(&mut self, _img: &web_sys::HtmlImageElement) -> Result<(), wasm_bindgen::JsValue> {
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn upload_vfx_muzzle_sprite(
        &mut self,
        _img: &web_sys::HtmlImageElement,
    ) -> Result<(), wasm_bindgen::JsValue> {
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn upload_vfx_blood_sprite(
        &mut self,
        _img: &web_sys::HtmlImageElement,
    ) -> Result<(), wasm_bindgen::JsValue> {
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn upload_vfx_shell_sprite(
        &mut self,
        _img: &web_sys::HtmlImageElement,
    ) -> Result<(), wasm_bindgen::JsValue> {
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn upload_weapon_sprite(
        &mut self,
        _slot: u32,
        _img: &web_sys::HtmlImageElement,
    ) -> Result<(), wasm_bindgen::JsValue> {
        Ok(())
    }

    pub fn characters_loaded(&self) -> bool {
        self.character.is_some()
    }

    pub fn character_rival_loaded(&self) -> bool {
        self.character_rival.is_some()
    }

    pub fn draw_world(
        &mut self,
        view_proj: Mat4,
        clear_rgb: Vec3,
        cam_pos: Vec3,
        characters: &[CharacterInstance],
        weapon_hud: WeaponHudParams,
        blood_splats: &[BloodSplat],
        hud_shells: &[HudShell],
        level_bounds: &Aabb,
        mural_z: f32,
    ) {
        let frame = match acquire_surface_texture(&self.surface, &self.device, &self.config) {
            Ok(f) => f,
            Err(e) => {
                self.diag_last_surface_ok = false;
                self.diag_last_surface_error = e.to_string();
                self.diag_frames_skipped_swapchain += 1;
                return;
            }
        };
        self.diag_last_surface_ok = true;
        self.diag_last_surface_error.clear();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let g = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 0.0],
            fog_color: FOG_COLOR_RGBA,
            fog_params: [FOG_DENSITY, FOG_MAX_BLEND, 0.0, 0.0],
            _pad: [0.0; 8],
        };
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&g));

        let capped: Vec<&CharacterInstance> = characters.iter().take(MAX_CHARACTER_INSTANCES).collect();

        let mut bill_cpu: Vec<BillVertex> = Vec::new();
        let mut mural_vert_count: usize = 0;
        if self.sprite_ready {
            let b = level_bounds;
            let span_x = (b.max.x - b.min.x).max(8.0);
            let pad_x = span_x * 0.14 + 3.0;
            let h = ((b.max.y - b.min.y).max(4.0) + 4.0).max(9.0);
            let z_plane = mural_z;
            let cx = (b.min.x + b.max.x) * 0.5;
            let x0 = cx - span_x * 0.5 - pad_x;
            let x1 = cx + span_x * 0.5 + pad_x;
            let y0 = b.min.y;
            let y1 = y0 + h;
            let bl = Vec3::new(x0, y0, z_plane);
            let br = Vec3::new(x1, y0, z_plane);
            let tr = Vec3::new(x1, y1, z_plane);
            let tl = Vec3::new(x0, y1, z_plane);
            let uvs = [
                [0.0_f32, 1.0],
                [1.0, 1.0],
                [1.0, 0.0],
                [0.0, 0.0],
            ];
            let corners = [bl, br, tr, tl];
            let white = [1.0_f32, 1.0, 1.0, 1.0];
            for i in 0..4 {
                bill_cpu.push(BillVertex {
                    pos: corners[i].to_array(),
                    uv: uvs[i],
                    tint: white,
                });
            }
            mural_vert_count = 4;
        }
        // Gun billboards removed — they created a distracting floating gun
        // near NPC faces; the character atlas already includes weapon sprites.
        let mural_idx_count: u32 = (mural_vert_count / 4 * 6) as u32;

        let splat_vert_start = bill_cpu.len();
        for s in blood_splats {
            if s.life <= 0.008 {
                continue;
            }
            let center = s.pos;
            let to_cam = cam_pos - center;
            let to_cam_xz = Vec3::new(to_cam.x, 0.0, to_cam.z);
            let len_xz = to_cam_xz.length();
            let mut right = if len_xz > 0.001 {
                Vec3::new(-to_cam_xz.z, 0.0, to_cam_xz.x) / len_xz
            } else {
                Vec3::new(1.0, 0.0, 0.0)
            };
            let cy = s.yaw.cos();
            let sy = s.yaw.sin();
            let rx = right.x * cy - right.z * sy;
            let rz = right.x * sy + right.z * cy;
            right = Vec3::new(rx, 0.0, rz);
            let rlen = (right.x * right.x + right.z * right.z).sqrt();
            if rlen > 0.001 {
                right.x /= rlen;
                right.z /= rlen;
            }
            let up = Vec3::new(0.0, 1.0, 0.0);
            let sz = 0.14_f32
                * s.scale
                * (0.35 + 0.65 * s.life.clamp(0.0, 1.0));
            let half = sz * 0.5;
            let bl = center - right * half - up * half;
            let br = center + right * half - up * half;
            let tr = center + right * half + up * half;
            let tl = center - right * half + up * half;
            let lf = s.life.clamp(0.0, 1.0);
            let tt = [1.15_f32 * lf, 0.22 * lf, 0.18 * lf, (0.92 * lf).min(1.0)];
            let uvs = [
                [0.0_f32, 1.0],
                [1.0, 1.0],
                [1.0, 0.0],
                [0.0, 0.0],
            ];
            let corners = [bl, br, tr, tl];
            for i in 0..4 {
                bill_cpu.push(BillVertex {
                    pos: corners[i].to_array(),
                    uv: uvs[i],
                    tint: tt,
                });
            }
        }
        let splat_idx_start = (splat_vert_start / 4 * 6) as u32;
        let splat_idx_count: u32 = ((bill_cpu.len() - splat_vert_start) / 4 * 6) as u32;

        let bill_idx_count: u32 = (bill_cpu.len() / 4 * 6) as u32;

        if !bill_cpu.is_empty() {
            self.queue
                .write_buffer(&self.bill_vb, 0, bytemuck::cast_slice(&bill_cpu));
        }

        let (rivals, boss_like): (Vec<_>, Vec<_>) = capped
            .iter()
            .copied()
            .partition(|c| c.skin == CharacterSkin::Rival);

        let fill_char_uniforms = |list: &[&CharacterInstance], cd: &CharacterDraw| -> Vec<u8> {
            let n = list.len();
            let stride = cd.char_uniform_stride as usize;
            let mut raw = vec![0u8; stride * n];
            for (i, inst) in list.iter().enumerate() {
                let inst_model = inst.model * cd.mesh_node_world;
                let m = inst_model.to_cols_array_2d();
                let char_x = m[3][0];
                let char_z = m[3][2];
                let u = CharUniforms {
                    view_proj: view_proj.to_cols_array_2d(),
                    model: m,
                    cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 0.0],
                    fog_color: FOG_COLOR_RGBA,
                    fog_params: [FOG_DENSITY, FOG_MAX_BLEND, 0.0, 0.0],
                    char_params: [inst.mesh_yaw, char_x, char_z, inst.anim_frame],
                    _p1: [0.0; 4],
                    _p2: [0.0; 4],
                    _p3: [0.0; 4],
                    _p4: [0.0; 4],
                };
                let dst = &mut raw[i * stride..i * stride + std::mem::size_of::<CharUniforms>()];
                dst.copy_from_slice(bytemuck::bytes_of(&u));
            }
            raw
        };

        let char_share_buffer = self.character_rival.is_none() && self.character.is_some();
        let split_char_passes =
            char_share_buffer && !boss_like.is_empty() && !rivals.is_empty();

        let write_boss_uniforms = |gpu: &Gpu| {
            if let Some(cd) = gpu.character.as_ref() {
                if !boss_like.is_empty() {
                    let bytes = fill_char_uniforms(&boss_like, cd);
                    gpu.queue.write_buffer(&cd.char_uniform, 0, bytes.as_slice());
                }
            }
        };
        let write_rival_uniforms = |gpu: &Gpu| {
            if !rivals.is_empty() {
                if let Some(cd) = gpu.character_rival.as_ref().or(gpu.character.as_ref()) {
                    let bytes = fill_char_uniforms(&rivals, cd);
                    gpu.queue.write_buffer(&cd.char_uniform, 0, bytes.as_slice());
                }
            }
        };

        if !split_char_passes {
            write_boss_uniforms(&*self);
            write_rival_uniforms(&*self);
        }

        let draw_world = |pass: &mut wgpu::RenderPass<'_>, gpu: &Gpu| {
            match &gpu.world {
                WorldRaster::Flat {
                    pipeline,
                    vb,
                    ib,
                    index_count,
                } => {
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &gpu.bind_group, &[]);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*index_count, 0, 0..1);
                }
                WorldRaster::Textured {
                    pipeline,
                    vb,
                    ib,
                    batches,
                    ..
                } => {
                    pass.set_pipeline(pipeline);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    for b in batches {
                        pass.set_bind_group(0, &gpu.bind_group, &[]);
                        pass.set_bind_group(1, &b.bind_group, &[]);
                        let end = b.first_index.saturating_add(b.index_count);
                        pass.draw_indexed(b.first_index..end, 0, 0..1);
                    }
                }
            }
        };

        let draw_boss_batch = |pass: &mut wgpu::RenderPass<'_>, gpu: &mut Gpu| {
            if let Some(cd) = gpu.character.as_ref() {
                if !boss_like.is_empty() {
                    let attach = gpu.weapon_prop.as_ref().and_then(|wpn| {
                        if cd.is_skinned() {
                            Some(WeaponAttachPass {
                                weapon_cd: wpn,
                                hand_to_prop: weapon_hand_to_prop_transform(),
                            })
                        } else {
                            None
                        }
                    });
                    draw_character_instances_3d(
                        &gpu.queue,
                        pass,
                        &boss_like,
                        cd,
                        &mut gpu.skinned_locals_scratch,
                        &mut gpu.skinned_bone_flat,
                        &mut gpu.skinned_node_world_scratch,
                        attach,
                        view_proj,
                        cam_pos,
                    );
                }
            }
        };

        let draw_rival_batch = |pass: &mut wgpu::RenderPass<'_>, gpu: &mut Gpu| {
            if !rivals.is_empty() {
                if let Some(cd) = gpu
                    .character_rival
                    .as_ref()
                    .or(gpu.character.as_ref())
                {
                    let attach = gpu.weapon_prop.as_ref().and_then(|wpn| {
                        if cd.is_skinned() {
                            Some(WeaponAttachPass {
                                weapon_cd: wpn,
                                hand_to_prop: weapon_hand_to_prop_transform(),
                            })
                        } else {
                            None
                        }
                    });
                    draw_character_instances_3d(
                        &gpu.queue,
                        pass,
                        &rivals,
                        cd,
                        &mut gpu.skinned_locals_scratch,
                        &mut gpu.skinned_bone_flat,
                        &mut gpu.skinned_node_world_scratch,
                        attach,
                        view_proj,
                        cam_pos,
                    );
                }
            }
        };

        // FPS weapon writes to a RESERVED high slot in the weapon_prop uniform buffer
        // so it does not overwrite the NPC weapon-attach data at offset 0.
        // wgpu stages all write_buffer calls before submit, so the last write wins
        // for any given byte range — using separate slots avoids the conflict.
        const FPS_WEAPON_SLOT: usize = MAX_CHARACTER_INSTANCES - 1;

        let draw_fps_weapon_3d = |pass: &mut wgpu::RenderPass<'_>, gpu: &Gpu| {
            if let Some(model) = weapon_hud.fps_weapon_model {
                if let Some(wpn) = gpu.weapon_prop.as_ref() {
                    if let Some(pipe) = wpn.fps_view_pipeline.as_ref() {
                        let bytes = fill_char_uniform_bytes_for_world_models(
                            &[model],
                            wpn.mesh_node_world,
                            wpn,
                            view_proj,
                            cam_pos,
                        );
                        let fps_byte_offset =
                            wpn.char_uniform_stride as u64 * FPS_WEAPON_SLOT as u64;
                        gpu.queue
                            .write_buffer(&wpn.char_uniform, fps_byte_offset, bytes.as_slice());

                        // Draw one instance at the reserved slot's dynamic offset.
                        let CharacterGeometry::Rigid { vb, ib, .. } = &wpn.geometry else {
                            return;
                        };
                        pass.set_pipeline(pipe);
                        pass.set_vertex_buffer(0, vb.slice(..));
                        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                        pass.set_bind_group(
                            0,
                            &wpn.char_globals_bg,
                            &[wpn.char_uniform_stride * FPS_WEAPON_SLOT as u32],
                        );
                        for b in &wpn.batches {
                            pass.set_bind_group(1, &b.bind_group, &[]);
                            let end = b.first_index.saturating_add(b.index_count);
                            pass.draw_indexed(b.first_index..end, 0, 0..1);
                        }
                    }
                }
            }
        };

        let draw_billboard = |gpu: &Gpu, pass: &mut wgpu::RenderPass<'_>| {
            if bill_cpu.is_empty() || bill_idx_count == 0 {
                return;
            }
            let vb_bytes = bill_cpu.len() as u64 * std::mem::size_of::<BillVertex>() as u64;
            let ib_bytes = bill_idx_count as u64 * 4;
            pass.set_pipeline(&gpu.bill_pipeline);
            pass.set_bind_group(0, &gpu.bind_group, &[]);
            pass.set_vertex_buffer(0, gpu.bill_vb.slice(0..vb_bytes));
            pass.set_index_buffer(gpu.bill_ib.slice(0..ib_bytes), wgpu::IndexFormat::Uint32);
            if mural_idx_count > 0 && gpu.sprite_ready {
                pass.set_bind_group(1, &gpu.bill_bind_group, &[]);
                pass.draw_indexed(0..mural_idx_count, 0, 0..1);
            }
            if splat_idx_count > 0 {
                if let Some(ref bg) = gpu.vfx_blood_bg {
                    pass.set_pipeline(&gpu.bill_blood_pipeline);
                    pass.set_bind_group(1, bg, &[]);
                    pass.draw_indexed(splat_idx_start..splat_idx_start + splat_idx_count, 0, 0..1);
                    pass.set_pipeline(&gpu.bill_pipeline);
                }
            }
        };

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        if split_char_passes {
            write_boss_uniforms(&*self);
            {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("world"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: clear_rgb.x as f64,
                                g: clear_rgb.y as f64,
                                b: clear_rgb.z as f64,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                draw_world(&mut pass, &*self);
                draw_boss_batch(&mut pass, self);
            }
            write_rival_uniforms(&*self);
            {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("world_chars2"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                draw_rival_batch(&mut pass, self);
                draw_fps_weapon_3d(&mut pass, &*self);
                draw_billboard(&*self, &mut pass);
            }
        } else {
            {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("world"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: clear_rgb.x as f64,
                                g: clear_rgb.y as f64,
                                b: clear_rgb.z as f64,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                draw_world(&mut pass, &*self);
                draw_boss_batch(&mut pass, self);
                draw_rival_batch(&mut pass, self);
                draw_fps_weapon_3d(&mut pass, &*self);
                draw_billboard(&*self, &mut pass);
            }
        }

        {
            let screen_aspect = self.config.width as f32 / self.config.height.max(1) as f32;
            let hu = HudUniform {
                weapon: weapon_hud.weapon_id,
                flash: weapon_hud.flash.clamp(0.0, 1.0),
                bob: weapon_hud.bob,
                recoil: weapon_hud.recoil.clamp(0.0, 1.0),
                reload: weapon_hud.reload.clamp(0.0, 2.0),
                aspect: screen_aspect,
                anim_t: weapon_hud.anim_t,
                fps_arms_overlay: if weapon_hud.fps_weapon_model.is_some() {
                    1.0
                } else {
                    0.0
                },
            };
            self.queue
                .write_buffer(&self.hud_uniform, 0, bytemuck::bytes_of(&hu));
            let mut shell_draw_n: i32 = 0;
            if self.vfx_shell_ready && self.vfx_shell_bg.is_some() {
                for shell in hud_shells {
                    if shell.life <= 0.03 {
                        continue;
                    }
                    let verts = build_hud_shell_vertices(shell);
                    let off = (shell_draw_n as u64) * (4 * std::mem::size_of::<HudVertex>() as u64);
                    self.queue
                        .write_buffer(&self.shell_vb, off, bytemuck::cast_slice(&verts));
                    shell_draw_n += 1;
                }
            }
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.hud_pipeline);
            pass.set_bind_group(0, &self.hud_bind_group, &[]);
            let wi = (weapon_hud.weapon_id as usize).min(3);
            if weapon_hud.fps_weapon_model.is_none() {
                if let Some(bg) = self.weapon_bind_groups.get(wi) {
                    pass.set_bind_group(1, bg, &[]);
                }
                pass.set_vertex_buffer(0, self.hud_vb.slice(..));
                pass.set_index_buffer(self.hud_ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..6, 0, 0..1);
            }
            if self.arms_ready {
                pass.set_pipeline(&self.hud_arms_pipeline);
                pass.set_bind_group(0, &self.hud_bind_group, &[]);
                pass.set_bind_group(1, &self.arms_bind_group, &[]);
                pass.set_vertex_buffer(0, self.hud_vb.slice(..));
                pass.set_index_buffer(self.hud_ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..6, 0, 0..1);
            }
            if self.vfx_muzzle_ready && weapon_hud.flash > 0.02 {
                if let Some(ref bg) = self.vfx_muzzle_bg {
                    pass.set_pipeline(&self.hud_muzzle_pipeline);
                    pass.set_bind_group(0, &self.hud_bind_group, &[]);
                    pass.set_bind_group(1, bg, &[]);
                    pass.set_vertex_buffer(0, self.vfx_muzzle_vb.slice(..));
                    pass.set_index_buffer(self.hud_ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..6, 0, 0..1);
                }
            }
            if shell_draw_n > 0 {
                if let Some(ref bg) = self.vfx_shell_bg {
                    pass.set_pipeline(&self.hud_shell_pipeline);
                    pass.set_bind_group(0, &self.hud_bind_group, &[]);
                    pass.set_bind_group(1, bg, &[]);
                    pass.set_vertex_buffer(0, self.shell_vb.slice(..));
                    pass.set_index_buffer(self.hud_ib.slice(..), wgpu::IndexFormat::Uint32);
                    for k in 0..shell_draw_n {
                        pass.draw_indexed(0..6, k * 4, 0..1);
                    }
                }
            }
        }

        self.queue.submit([enc.finish()]);
        frame.present();
        #[cfg(target_arch = "wasm32")]
        self.device.poll(wgpu::Maintain::Poll);
        self.diag_frames_submitted += 1;
    }
}

fn make_transparent_tex(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("weapon-empty"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[0, 0, 0, 0],
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn make_placeholder_sprite(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    w: u32,
    h: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ph"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let px = vec![200u8, 40u8, 60u8, 255u8, 30u8, 20u8, 35u8, 255u8, 30u8, 20u8, 35u8, 255u8, 200u8, 40u8, 60u8, 255u8];
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let s = ((x + y) % 2 * 4) as usize;
            data[i..i + 4].copy_from_slice(&px[s..s + 4]);
        }
    }
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * w),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn create_depth(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    (depth, view)
}
