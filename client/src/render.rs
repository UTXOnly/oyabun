use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use crate::mesh::Aabb;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;
#[cfg(target_arch = "wasm32")]
use wgpu::{ExternalImageSource, ImageCopyExternalImage, Origin2d};

#[cfg(target_arch = "wasm32")]
fn warn_str(s: &str) {
    web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(s));
}

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
    let fog_amt = 1.0 - exp(-dist * g.fog_params.x);
    let fc = g.fog_color.rgb;
    return vec4<f32>(mix(q, fc, clamp(fog_amt, 0.0, 1.0)), 1.0);
}
"#;

/// Textured world: albedo × tint, **posterized** in fragment; sampler is nearest (non-filterable).
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
// Hash for procedural detail
fn oya_hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

@fragment
fn fs_tex(i: Vout) -> @location(0) vec4<f32> {
    let t = textureSample(albedo, albedo_samp, i.uv) * mu.tint;
    let wp = i.world_pos;
    let lum = t.r * 0.3 + t.g * 0.5 + t.b * 0.2;

    // Procedural brick/block pattern on dark surfaces (walls, ground)
    var detail = 1.0;
    if (lum < 0.45) {
        let wall_uv = vec2<f32>(wp.x + wp.z, wp.y);
        let bp = wall_uv * vec2<f32>(1.5, 3.0);
        let row = floor(bp.y);
        var bx = bp.x;
        if (fract(row * 0.5) > 0.25) { bx = bx + 0.5; }
        let cell = fract(vec2<f32>(bx, bp.y));
        let mortar = 0.06;
        let brick = step(mortar, cell.x) * step(mortar, 1.0 - cell.x)
                   * step(mortar, cell.y) * step(mortar, 1.0 - cell.y);
        let grime = oya_hash(floor(wall_uv * 4.0)) * 0.10;
        let streak = oya_hash(vec2<f32>(floor(wall_uv.x * 8.0), 0.5))
                   * step(fract(wall_uv.y * 2.0), 0.3) * 0.08;
        detail = mix(0.82, 1.0, brick) * (1.0 - grime) * (1.0 - streak);
    }

    // Cyberpunk ambient: warm overhead + cool neon bounce
    let h_norm = clamp((wp.y - 0.0) / 6.0, 0.0, 1.0);
    let ambient_warm = vec3<f32>(0.30, 0.22, 0.16) * (0.5 + 0.5 * h_norm);
    let ambient_cool = vec3<f32>(0.12, 0.20, 0.26) * (1.0 - h_norm * 0.5);
    let ambient = ambient_warm + ambient_cool;

    // Fake neon spill: sinusoidal color bands along the alley
    let neon_phase = wp.x * 0.15 + wp.z * 0.12;
    let neon_r = 0.10 * max(sin(neon_phase * 2.1 + 1.0), 0.0);
    let neon_g = 0.06 * max(sin(neon_phase * 1.7 + 3.5), 0.0);
    let neon_b = 0.12 * max(sin(neon_phase * 2.8 + 5.2), 0.0);
    let neon_spill = vec3<f32>(neon_r, neon_g, neon_b) * (1.0 - h_norm * 0.4);

    // Bright emissive surfaces glow extra (signs, neons)
    // Stronger boost so kanji signs really pop against dark walls
    let emit_boost = max(lum - 0.35, 0.0) * 1.2;

    let lit = t.rgb * detail * 2.0 + ambient + neon_spill + t.rgb * emit_boost;

    // Posterize to 24 levels for that arcade CRT look (less crushing than 15)
    let q = floor(clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)) * 24.0) / 24.0;
    let dist = length(wp - g.cam_pos.xyz);
    let fog_amt = 1.0 - exp(-dist * g.fog_params.x);
    let fc = g.fog_color.rgb;
    return vec4<f32>(mix(q, fc, clamp(fog_amt, 0.0, 1.0)), t.a);
}
"#;

/// Textured character mesh; vertex applies **per-entity model** (relay pose).
// 8-column atlas, N rows (row 0 = idle, rows 1..N = walk frames).
// char_params.x = mesh yaw, .y = world_x, .z = world_z, .w = anim_row (0=idle, 1-6=walk).
// Fragment picks column from (camera bearing − mesh_yaw) so the correct facing shows.
/// Billboard character shader — textured sprite quad always faces camera.
/// Fragment selects atlas column based on NPC facing vs camera angle.
/// 3D character shader — standard model transform with material tint colors.
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

struct Vin { @location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>, }
struct Vout {
  @builtin(position) clip: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) world_pos: vec3<f32>,
  @location(2) normal_approx: vec3<f32>,
};
@vertex
fn vs_char(v: Vin) -> Vout {
  let world_pos = (cu.model * vec4<f32>(v.pos, 1.0)).xyz;
  // Approximate normal from model matrix (uniform scale assumed)
  let n = normalize((cu.model * vec4<f32>(0.0, 0.0, 1.0, 0.0)).xyz);
  var o: Vout;
  o.world_pos = world_pos;
  o.clip = cu.view_proj * vec4<f32>(world_pos, 1.0);
  o.uv = v.uv;
  o.normal_approx = n;
  return o;
}
@fragment
fn fs_char(i: Vout) -> @location(0) vec4<f32> {
    let anim_row = cu.char_params.w;

    // Hit flash: values > 100 encode flash intensity
    var hit_mix = 0.0;
    if (anim_row > 99.0) {
        hit_mix = clamp(anim_row - 100.0, 0.0, 1.0);
    }

    // Material color from tint (3D models use per-material colors, not atlas)
    let t = textureSample(albedo, albedo_samp, i.uv) * mu.tint;

    // Simple directional lighting
    let light_dir = normalize(vec3<f32>(0.3, 0.8, -0.5));
    let ndotl = max(dot(normalize(i.normal_approx), light_dir), 0.0);
    let ambient = vec3<f32>(0.25, 0.20, 0.28);
    let lit = t.rgb * (ambient + vec3<f32>(0.7) * ndotl);

    // Hit flash: red-white overlay
    let flashed = mix(lit, vec3<f32>(1.0, 0.3, 0.2), hit_mix * 0.6);

    // Distance fog
    let wp = i.world_pos;
    let dist = length(wp - cu.cam_pos.xyz);
    let fog_amt = 1.0 - exp(-dist * cu.fog_params.x);
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

struct Bin { @location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>, };
struct Bout { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32>, };

@vertex
fn vs_bill(v: Bin) -> Bout {
    var o: Bout;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    return o;
}

@fragment
fn fs_bill(i: Bout) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, i.uv);
    if (c.a < 0.35) { discard; }
    let d = length(i.uv - vec2<f32>(0.5, 0.5));
    let rim = smoothstep(0.72, 0.38, d) * 0.15;
    return vec4<f32>(c.rgb * (1.05 + rim) + vec3<f32>(0.08, 0.02, 0.04) * rim, c.a);
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
    _pad: [f32; 2],
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
struct Hu { weapon: u32, flash: f32, bob: f32, recoil: f32, reload: f32, aspect: f32, _p1: f32, _p2: f32, }
@group(0) @binding(0) var<uniform> hu: Hu;
@group(1) @binding(0) var wtex: texture_2d<f32>;
@group(1) @binding(1) var wsamp: sampler;

struct HIn { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, }
struct HOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32>, }

@vertex
fn vs_hud(v: HIn) -> HOut {
  // Aspect ratio correction — keep weapon square regardless of screen shape
  let inv_aspect = 1.0 / max(hu.aspect, 0.5);

  // Walk bob (scaled for aspect)
  let bx = sin(hu.bob) * 0.025 * inv_aspect;
  let by = cos(hu.bob * 1.35) * 0.018;

  // Recoil kick: weapon jumps up strongly on fire
  let recoil_y = hu.recoil * hu.recoil * 0.14;
  let recoil_x = -hu.recoil * 0.03 * inv_aspect;

  // Reload: weapon drops below screen then comes back
  var reload_y = 0.0;
  if (hu.reload > 0.0) {
    if (hu.reload < 1.0) {
      reload_y = -hu.reload * 0.7;
    } else {
      reload_y = -(2.0 - hu.reload) * 0.7;
    }
  }

  // Position: correct X for aspect ratio, offset weapon slightly right
  var p = v.pos;
  p.x = p.x * inv_aspect + 0.12 * inv_aspect;

  var o: HOut;
  o.clip = vec4<f32>(p + vec2<f32>(bx + recoil_x, by + recoil_y + reload_y), 0.0, 1.0);
  o.uv = v.uv;
  return o;
}

@fragment
fn fs_hud(i: HOut) -> @location(0) vec4<f32> {
  let uv_tex = vec2<f32>(i.uv.x, 1.0 - i.uv.y);
  let t = textureSample(wtex, wsamp, uv_tex);

  // Discard transparent pixels
  if (t.a < 0.10) { discard; }

  // Base color with flash brightening
  let flash_boost = 1.0 + 0.6 * hu.flash;
  var rgb = t.rgb * flash_boost;

  // White-hot flash on weapon surface when firing
  if (hu.flash > 0.3) {
    let hot = (hu.flash - 0.3) * 1.43; // 0..1
    rgb = mix(rgb, vec3<f32>(1.0, 0.95, 0.85), hot * 0.35);
  }

  // Muzzle flash glow — large dramatic burst at barrel tip
  if (hu.flash > 0.01) {
    // Flash center: top-right area where barrel is (adjusted for new sprite layout)
    let muzzle_uv = vec2<f32>(0.72, 0.78);
    let mf = length(i.uv - muzzle_uv);
    // Large primary flash
    let fl1 = smoothstep(0.30, 0.0, mf) * hu.flash;
    // Hot core
    let fl2 = smoothstep(0.10, 0.0, mf) * hu.flash;
    let flash_color = mix(vec3<f32>(1.0, 0.7, 0.2), vec3<f32>(1.0, 1.0, 0.9), fl2);
    rgb = mix(rgb, flash_color, fl1 * 0.8);
  }

  return vec4<f32>(rgb, t.a);
}

@fragment
fn fs_hud_arms(i: HOut) -> @location(0) vec4<f32> {
  let uv_tex = vec2<f32>(i.uv.x, 1.0 - i.uv.y);
  let t = textureSample(wtex, wsamp, uv_tex);
  if (t.a < 0.06) { discard; }
  return vec4<f32>(t.rgb * (1.0 + 0.3 * hu.flash), t.a);
}
"#;

const WEAPON_BG_LABELS: [&str; 4] = ["weapon-0", "weapon-1", "weapon-2", "weapon-3"];

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

struct CharacterDraw {
    pipeline: wgpu::RenderPipeline,
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    batches: Vec<WorldBatchGpu>,
    char_uniform: wgpu::Buffer,
    char_globals_bg: wgpu::BindGroup,
    /// Byte stride per instance; multiple of `min_uniform_buffer_offset_alignment`.
    char_uniform_stride: u32,
    #[allow(dead_code)]
    _textures: Vec<wgpu::Texture>,
    #[allow(dead_code)]
    _tint_buffers: Vec<wgpu::Buffer>,
}

pub struct WeaponHudParams {
    pub weapon_id: u32,
    pub bob: f32,
    pub flash: f32,
    pub recoil: f32,
    pub reload: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterSkin {
    Boss,
    Rival,
    /// Other players / offline demos (boss atlas until a player strip ships).
    Remote,
}

pub struct CharacterInstance {
    pub model: Mat4,
    pub mesh_yaw: f32,
    pub skin: CharacterSkin,
    /// 0.0 = idle row, 1.0–6.0 = walk frame rows in the atlas.
    pub anim_frame: f32,
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
    character: Option<CharacterDraw>,
    character_rival: Option<CharacterDraw>,
}

impl Gpu {
    pub async fn new(
        canvas: HtmlCanvasElement,
        flat_vertices: &[Vertex],
        flat_indices: &[u32],
        gltf_level: Option<crate::gltf_level::GltfLevelCpu>,
        character_level: Option<crate::gltf_level::GltfLevelCpu>,
        character_rival_level: Option<crate::gltf_level::GltfLevelCpu>,
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
        surface.configure(&device, &config);

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
                entry_point: Some("vs_hud"),
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

        let try_raster_char =
            |cpu: crate::gltf_level::GltfLevelCpu, label: &str| -> Option<CharacterDraw> {
                if cpu.vertices.is_empty() || cpu.indices.is_empty() || cpu.batches.is_empty() {
                    #[cfg(target_arch = "wasm32")]
                    warn_str(&format!("oyabaun: {label} has no drawable geometry"));
                    return None;
                }
                match Self::raster_character_gltf(&device, &queue, format, cpu) {
                    Ok(cd) => Some(cd),
                    Err(e) => {
                        #[cfg(target_arch = "wasm32")]
                        warn_str(&format!(
                            "oyabaun: {label} GPU init failed ({e:?}) — rebuild client/characters/*.glb"
                        ));
                        None
                    }
                }
            };

        #[cfg(target_arch = "wasm32")]
        if character_level.is_none() {
            warn_str("oyabaun: no oyabaun_player.glb parsed — Blender: tools/blender_make_oyabaun_character.py (OYABAUN_VARIANT=all), then wasm-pack build");
        }
        let character = character_level.and_then(|cpu| try_raster_char(cpu, "oyabaun_player.glb"));

        let character_rival =
            character_rival_level.and_then(|cpu| try_raster_char(cpu, "oyabaun_rival.glb"));

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
            character,
            character_rival,
        })
    }

    fn raster_character_gltf(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        cpu: crate::gltf_level::GltfLevelCpu,
    ) -> Result<CharacterDraw, wasm_bindgen::JsValue> {
        use crate::gltf_level::WorldVertex;

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
            label: Some("char-tex"),
            layout: Some(&char_pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_char"),
                buffers: &[WorldVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_char"),
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
                depth_write_enabled: true, // solid 3D models need depth write
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

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
            pipeline,
            vb,
            ib,
            batches,
            char_uniform,
            char_globals_bg,
            char_uniform_stride,
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

    #[cfg(not(target_arch = "wasm32"))]
    pub fn upload_arms_sprite(&mut self, _img: &web_sys::HtmlImageElement) -> Result<(), wasm_bindgen::JsValue> {
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
        level_bounds: &Aabb,
        mural_z: f32,
    ) {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let g = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 0.0],
            fog_color: [0.06, 0.04, 0.10, 1.0],
            fog_params: [0.003, 0.0, 0.0, 0.0],
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
            for i in 0..4 {
                bill_cpu.push(BillVertex {
                    pos: corners[i].to_array(),
                    uv: uvs[i],
                });
            }
            mural_vert_count = 4;
        }
        // Gun billboards removed — they created a distracting floating gun
        // near NPC faces; the character atlas already includes weapon sprites.
        let mural_idx_count: u32 = (mural_vert_count / 4 * 6) as u32;
        let bill_idx_count: u32 = (bill_cpu.len() / 4 * 6) as u32;
        let gun_idx_count: u32 = bill_idx_count.saturating_sub(mural_idx_count);

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
                let m = inst.model.to_cols_array_2d();
                let char_x = m[3][0];
                let char_z = m[3][2];
                let u = CharUniforms {
                    view_proj: view_proj.to_cols_array_2d(),
                    model: m,
                    cam_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 0.0],
                    fog_color: [0.06, 0.04, 0.10, 1.0],
                    fog_params: [0.003, 0.0, 0.0, 0.0],
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

        let write_boss_uniforms = || {
            if let Some(cd) = self.character.as_ref() {
                if !boss_like.is_empty() {
                    let bytes = fill_char_uniforms(&boss_like, cd);
                    self.queue.write_buffer(&cd.char_uniform, 0, bytes.as_slice());
                }
            }
        };
        let write_rival_uniforms = || {
            if !rivals.is_empty() {
                if let Some(cd) = self.character_rival.as_ref().or(self.character.as_ref()) {
                    let bytes = fill_char_uniforms(&rivals, cd);
                    self.queue.write_buffer(&cd.char_uniform, 0, bytes.as_slice());
                }
            }
        };

        if !split_char_passes {
            write_boss_uniforms();
            write_rival_uniforms();
        }

        let draw_world = |pass: &mut wgpu::RenderPass<'_>| {
            match &self.world {
                WorldRaster::Flat {
                    pipeline,
                    vb,
                    ib,
                    index_count,
                } => {
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &self.bind_group, &[]);
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
                        pass.set_bind_group(0, &self.bind_group, &[]);
                        pass.set_bind_group(1, &b.bind_group, &[]);
                        let end = b.first_index.saturating_add(b.index_count);
                        pass.draw_indexed(b.first_index..end, 0, 0..1);
                    }
                }
            }
        };

        let draw_boss_batch = |pass: &mut wgpu::RenderPass<'_>| {
            if let Some(ref cd) = self.character {
                if !boss_like.is_empty() {
                    pass.set_pipeline(&cd.pipeline);
                    pass.set_vertex_buffer(0, cd.vb.slice(..));
                    pass.set_index_buffer(cd.ib.slice(..), wgpu::IndexFormat::Uint32);
                    let stride = cd.char_uniform_stride;
                    for i in 0..boss_like.len() {
                        pass.set_bind_group(0, &cd.char_globals_bg, &[stride * i as u32]);
                        for b in &cd.batches {
                            pass.set_bind_group(1, &b.bind_group, &[]);
                            let end = b.first_index.saturating_add(b.index_count);
                            pass.draw_indexed(b.first_index..end, 0, 0..1);
                        }
                    }
                }
            }
        };

        let draw_rival_batch = |pass: &mut wgpu::RenderPass<'_>| {
            if !rivals.is_empty() {
                if let Some(cd) = self.character_rival.as_ref().or(self.character.as_ref()) {
                    pass.set_pipeline(&cd.pipeline);
                    pass.set_vertex_buffer(0, cd.vb.slice(..));
                    pass.set_index_buffer(cd.ib.slice(..), wgpu::IndexFormat::Uint32);
                    let stride = cd.char_uniform_stride;
                    for i in 0..rivals.len() {
                        pass.set_bind_group(0, &cd.char_globals_bg, &[stride * i as u32]);
                        for b in &cd.batches {
                            pass.set_bind_group(1, &b.bind_group, &[]);
                            let end = b.first_index.saturating_add(b.index_count);
                            pass.draw_indexed(b.first_index..end, 0, 0..1);
                        }
                    }
                }
            }
        };

        let draw_billboard = |pass: &mut wgpu::RenderPass<'_>| {
            if bill_cpu.is_empty() || bill_idx_count == 0 {
                return;
            }
            let vb_bytes = bill_cpu.len() as u64 * std::mem::size_of::<BillVertex>() as u64;
            let ib_bytes = bill_idx_count as u64 * 4;
            pass.set_pipeline(&self.bill_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.bill_vb.slice(0..vb_bytes));
            pass.set_index_buffer(self.bill_ib.slice(0..ib_bytes), wgpu::IndexFormat::Uint32);
            if mural_idx_count > 0 && self.sprite_ready {
                pass.set_bind_group(1, &self.bill_bind_group, &[]);
                pass.draw_indexed(0..mural_idx_count, 0, 0..1);
            }
            if gun_idx_count > 0 && self.character.is_some() {
                if let Some(bg) = self.weapon_bind_groups.get(0) {
                    pass.set_bind_group(1, bg, &[]);
                    pass.draw_indexed(mural_idx_count..bill_idx_count, 0, 0..1);
                }
            }
        };

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        if split_char_passes {
            write_boss_uniforms();
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
                draw_world(&mut pass);
                draw_boss_batch(&mut pass);
            }
            write_rival_uniforms();
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
                draw_rival_batch(&mut pass);
                draw_billboard(&mut pass);
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
                draw_world(&mut pass);
                draw_boss_batch(&mut pass);
                draw_rival_batch(&mut pass);
                draw_billboard(&mut pass);
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
                _pad: [0.0; 2],
            };
            self.queue
                .write_buffer(&self.hud_uniform, 0, bytemuck::bytes_of(&hu));
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
            if let Some(bg) = self.weapon_bind_groups.get(wi) {
                pass.set_bind_group(1, bg, &[]);
            }
            pass.set_vertex_buffer(0, self.hud_vb.slice(..));
            pass.set_index_buffer(self.hud_ib.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..6, 0, 0..1);
            if self.arms_ready {
                pass.set_pipeline(&self.hud_arms_pipeline);
                pass.set_bind_group(0, &self.hud_bind_group, &[]);
                pass.set_bind_group(1, &self.arms_bind_group, &[]);
                pass.set_vertex_buffer(0, self.hud_vb.slice(..));
                pass.set_index_buffer(self.hud_ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..6, 0, 0..1);
            }
        }

        self.queue.submit([enc.finish()]);
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
