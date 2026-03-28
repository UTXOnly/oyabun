use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use crate::mesh::Aabb;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;
#[cfg(target_arch = "wasm32")]
use wgpu::{ExternalImageSource, ImageCopyExternalImage, Origin2d};

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
    let base = i.col * (0.82 + 0.18 * i.col.r);
    let dist = length(i.world_pos - g.cam_pos.xyz);
    let fog_amt = 1.0 - exp(-dist * g.fog_params.x);
    let fc = g.fog_color.rgb;
    return vec4<f32>(mix(base, fc, clamp(fog_amt, 0.0, 1.0)), 1.0);
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
@fragment
fn fs_tex(i: Vout) -> @location(0) vec4<f32> {
    let t = textureSample(albedo, albedo_samp, i.uv) * mu.tint;
    // Fake ambient: height-gradient fill simulates sky + bounced light
    // in an unlit shader where Blender materials are tuned for Eevee
    let h_norm = clamp((i.world_pos.y + 1.0) / 14.0, 0.0, 1.0);
    let ambient = vec3<f32>(0.06, 0.05, 0.08) + h_norm * vec3<f32>(0.04, 0.03, 0.02);
    let lit = t.rgb + ambient;
    let q = floor(clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)) * 15.0) / 15.0;
    let dist = length(i.world_pos - g.cam_pos.xyz);
    let fog_amt = 1.0 - exp(-dist * g.fog_params.x);
    let fc = g.fog_color.rgb;
    return vec4<f32>(mix(q, fc, clamp(fog_amt, 0.0, 1.0)), t.a);
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

const MAX_BILL_QUADS: usize = 48;
const BILL_VERTS: usize = MAX_BILL_QUADS * 4;
const BILL_IDX: usize = MAX_BILL_QUADS * 6;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct HudUniform {
    weapon: u32,
    flash: f32,
    bob: f32,
    _pad: f32,
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
struct Hu { weapon: u32, flash: f32, bob: f32, _p: f32, }
@group(0) @binding(0) var<uniform> hu: Hu;
@group(1) @binding(0) var wtex: texture_2d<f32>;
@group(1) @binding(1) var wsamp: sampler;

struct HIn { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, }
struct HOut { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32>, }

@vertex
fn vs_hud(v: HIn) -> HOut {
  let bx = sin(hu.bob) * 0.03;
  let by = cos(hu.bob * 1.35) * 0.02;
  var o: HOut;
  o.clip = vec4<f32>(v.pos + vec2<f32>(bx, by), 0.0, 1.0);
  o.uv = v.uv;
  return o;
}

@fragment
fn fs_hud(i: HOut) -> @location(0) vec4<f32> {
  let uv_tex = vec2<f32>(i.uv.x, 1.0 - i.uv.y);
  let t = textureSample(wtex, wsamp, uv_tex);
  var rgba = vec4<f32>(0.0);
  if (t.a > 0.18) {
    rgba = vec4<f32>(t.rgb * (1.02 + 0.08 * hu.flash), t.a);
  } else {
  let uv = i.uv;
  let skin = vec3<f32>(0.66, 0.46, 0.36);
  let lh = length(uv - vec2<f32>(0.13, 0.93));
  let rh = length(uv - vec2<f32>(0.87, 0.91));
  let ha = smoothstep(0.17, 0.03, lh);
  let hb = smoothstep(0.15, 0.03, rh);
  rgba = mix(rgba, vec4<f32>(skin, 0.9), max(ha, hb) * 0.88);

  let metal = vec3<f32>(0.2, 0.19, 0.22);
  let dark = vec3<f32>(0.12, 0.11, 0.13);

  if hu.weapon == 0u {
    let grip = step(0.44, uv.x) * step(uv.x, 0.57) * step(0.38, uv.y) * step(uv.y, 0.7);
    let bar = step(0.47, uv.x) * step(uv.x, 0.54) * step(0.58, uv.y) * step(uv.y, 0.96);
    rgba = mix(rgba, vec4<f32>(dark * 0.85, 0.95), grip);
    rgba = mix(rgba, vec4<f32>(metal, 0.98), bar);
  } else if hu.weapon == 1u {
    let stock = step(0.32, uv.x) * step(uv.x, 0.42) * step(0.4, uv.y) * step(uv.y, 0.68);
    let body = step(0.38, uv.x) * step(uv.x, 0.62) * step(0.32, uv.y) * step(uv.y, 0.62);
    let barrels = step(0.4, uv.x) * step(uv.x, 0.6) * step(0.68, uv.y) * step(uv.y, 0.94);
    rgba = mix(rgba, vec4<f32>(dark * 0.75, 0.92), stock);
    rgba = mix(rgba, vec4<f32>(metal * 0.9, 0.96), body);
    rgba = mix(rgba, vec4<f32>(metal, 0.99), barrels);
  } else if hu.weapon == 2u {
    let mag = step(0.46, uv.x) * step(uv.x, 0.54) * step(0.36, uv.y) * step(uv.y, 0.58);
    let body = step(0.35, uv.x) * step(uv.x, 0.65) * step(0.42, uv.y) * step(uv.y, 0.55);
    let barrel = step(0.42, uv.x) * step(uv.x, 0.58) * step(0.56, uv.y) * step(uv.y, 0.92);
    rgba = mix(rgba, vec4<f32>(dark, 0.94), mag);
    rgba = mix(rgba, vec4<f32>(metal * 0.8, 0.95), body);
    rgba = mix(rgba, vec4<f32>(metal * 0.95, 0.98), barrel);
  } else {
    let core = step(0.4, uv.x) * step(uv.x, 0.6) * step(0.4, uv.y) * step(uv.y, 0.65);
    let coils = step(0.38, uv.x) * step(uv.x, 0.62) * step(0.62, uv.y) * step(uv.y, 0.88);
    let glow = vec3<f32>(0.25, 0.75, 0.88);
    rgba = mix(rgba, vec4<f32>(dark * 0.9, 0.93), core);
    rgba = mix(rgba, vec4<f32>(mix(metal, glow, 0.55), 0.97), coils);
  }

  }
  if hu.flash > 0.01 {
    let mf = length(i.uv - vec2<f32>(0.505, 0.76));
    let fl = smoothstep(0.22, 0.0, mf) * hu.flash;
    rgba = mix(rgba, vec4<f32>(1.0, 0.88, 0.4, 1.0), fl);
  }
  return rgba;
}

@fragment
fn fs_hud_arms(i: HOut) -> @location(0) vec4<f32> {
  let uv_tex = vec2<f32>(i.uv.x, 1.0 - i.uv.y);
  let t = textureSample(wtex, wsamp, uv_tex);
  if (t.a < 0.06) { discard; }
  return vec4<f32>(t.rgb * (1.0 + 0.1 * hu.flash), t.a);
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

pub struct WeaponHudParams {
    pub weapon_id: u32,
    pub bob: f32,
    pub flash: f32,
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
    bill_pipeline: wgpu::RenderPipeline,
    bill_npc_pipeline: wgpu::RenderPipeline,
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
    boss_texture: wgpu::Texture,
    boss_bind_group: wgpu::BindGroup,
    pub boss_ready: bool,
    boss_vb: wgpu::Buffer,
    boss_ib: wgpu::Buffer,
    rival_texture: wgpu::Texture,
    rival_bind_group: wgpu::BindGroup,
    pub rival_ready: bool,
    rival_vb: wgpu::Buffer,
    rival_ib: wgpu::Buffer,
}

impl Gpu {
    pub async fn new(
        canvas: HtmlCanvasElement,
        flat_vertices: &[Vertex],
        flat_indices: &[u32],
        gltf_level: Option<crate::gltf_level::GltfLevelCpu>,
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

        let (boss_texture, boss_view) = make_transparent_tex(&device, &queue);
        let boss_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("boss-bg"),
            layout: &sprite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&boss_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&bill_sampler),
                },
            ],
        });
        let boss_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("boss-vb"),
            size: (std::mem::size_of::<BillVertex>() * 4) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let boss_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("boss-ib"),
            contents: bytemuck::cast_slice(&[0u32, 1, 2, 0, 2, 3]),
            usage: wgpu::BufferUsages::INDEX,
        });

        let (rival_texture, rival_view) = make_transparent_tex(&device, &queue);
        let rival_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rival-bg"),
            layout: &sprite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&rival_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&bill_sampler),
                },
            ],
        });
        let rival_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rival-vb"),
            size: (std::mem::size_of::<BillVertex>() * 4) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let rival_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rival-ib"),
            contents: bytemuck::cast_slice(&[0u32, 1, 2, 0, 2, 3]),
            usage: wgpu::BufferUsages::INDEX,
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

        let bill_npc_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bill-npc"),
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
                depth_compare: wgpu::CompareFunction::Always,
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
        let hud_verts: [HudVertex; 4] = [
            HudVertex {
                pos: [-0.42, -0.98],
                uv: [0.0, 0.0],
            },
            HudVertex {
                pos: [0.42, -0.98],
                uv: [1.0, 0.0],
            },
            HudVertex {
                pos: [0.42, -0.1],
                uv: [1.0, 1.0],
            },
            HudVertex {
                pos: [-0.42, -0.1],
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
            bill_npc_pipeline,
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
            boss_texture,
            boss_bind_group,
            boss_ready: false,
            boss_vb,
            boss_ib,
            rival_texture,
            rival_bind_group,
            rival_ready: false,
            rival_vb,
            rival_ib,
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
    pub fn upload_boss_sprite(&mut self, img: &web_sys::HtmlImageElement) -> Result<(), wasm_bindgen::JsValue> {
        let w = img.width().max(1);
        let h = img.height().max(1);
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("boss-sprite"),
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
            label: Some("boss-bg"),
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
        self.boss_texture = tex;
        self.boss_bind_group = bg;
        self.boss_ready = true;
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn upload_rival_sprite(&mut self, img: &web_sys::HtmlImageElement) -> Result<(), wasm_bindgen::JsValue> {
        let w = img.width().max(1);
        let h = img.height().max(1);
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rival-sprite"),
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
            label: Some("rival-bg"),
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
        self.rival_texture = tex;
        self.rival_bind_group = bg;
        self.rival_ready = true;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn upload_arms_sprite(&mut self, _img: &web_sys::HtmlImageElement) -> Result<(), wasm_bindgen::JsValue> {
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn upload_boss_sprite(&mut self, _img: &web_sys::HtmlImageElement) -> Result<(), wasm_bindgen::JsValue> {
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn upload_rival_sprite(&mut self, _img: &web_sys::HtmlImageElement) -> Result<(), wasm_bindgen::JsValue> {
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

    fn make_bill_quad(cam_pos: Vec3, c: Vec3, sc: f32) -> Option<[BillVertex; 4]> {
        let to_cam = (cam_pos - c).normalize_or_zero();
        if to_cam.length_squared() < 0.0001 {
            return None;
        }
        let mut right = to_cam.cross(Vec3::Y);
        if right.length_squared() < 1e-10 {
            right = to_cam.cross(Vec3::Z);
        }
        let right = right.normalize_or_zero();
        if right.length_squared() < 1e-10 {
            return None;
        }
        let up = right.cross(to_cam).normalize_or_zero();
        if up.length_squared() < 1e-10 {
            return None;
        }
        let hw = 0.55 * sc;
        let hh = 1.35 * sc;
        let mid = c + Vec3::Y * (hh * 0.55);
        let p = [
            mid - right * hw - up * hh,
            mid + right * hw - up * hh,
            mid + right * hw + up * hh,
            mid - right * hw + up * hh,
        ];
        let uvs = [[0.0_f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        if !p.iter().all(|v| v.x.is_finite() && v.y.is_finite() && v.z.is_finite()) {
            return None;
        }
        Some([
            BillVertex {
                pos: p[0].to_array(),
                uv: uvs[0],
            },
            BillVertex {
                pos: p[1].to_array(),
                uv: uvs[1],
            },
            BillVertex {
                pos: p[2].to_array(),
                uv: uvs[2],
            },
            BillVertex {
                pos: p[3].to_array(),
                uv: uvs[3],
            },
        ])
    }

    fn draw_npc_billboard(
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        cam_pos: Vec3,
        npc: Option<(Vec3, f32)>,
        ready: bool,
        npc_pipeline: &wgpu::RenderPipeline,
        globals_bg: &wgpu::BindGroup,
        bind_group: &wgpu::BindGroup,
        vb: &wgpu::Buffer,
        ib: &wgpu::Buffer,
    ) {
        if !ready {
            return;
        }
        if let Some((bc, sc)) = npc {
            if let Some(q) = Self::make_bill_quad(cam_pos, bc, sc) {
                queue.write_buffer(vb, 0, bytemuck::cast_slice(&q));
                let vb_bytes = 4 * std::mem::size_of::<BillVertex>() as u64;
                pass.set_pipeline(npc_pipeline);
                pass.set_bind_group(0, globals_bg, &[]);
                pass.set_bind_group(1, bind_group, &[]);
                pass.set_vertex_buffer(0, vb.slice(0..vb_bytes));
                pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..6, 0, 0..1);
            }
        }
    }

    pub fn draw_world(
        &mut self,
        view_proj: Mat4,
        clear_rgb: Vec3,
        cam_pos: Vec3,
        billboards: &[(Vec3, f32)],
        weapon_hud: WeaponHudParams,
        boss: Option<(Vec3, f32)>,
        rival: Option<(Vec3, f32)>,
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
            fog_color: [0.12, 0.09, 0.18, 1.0],
            fog_params: [0.022, 0.0, 0.0, 0.0],
            _pad: [0.0; 8],
        };
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&g));

        let mut bill_cpu: Vec<BillVertex> = Vec::new();
        let backdrop_quads: usize = if self.sprite_ready { 1 } else { 0 };
        let max_player_quads = MAX_BILL_QUADS.saturating_sub(backdrop_quads);
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
        }
        if !billboards.is_empty() {
            let n = billboards.len().min(max_player_quads);
            for &(c, sc) in billboards.iter().take(n) {
                if let Some(q) = Self::make_bill_quad(cam_pos, c, sc) {
                    bill_cpu.extend_from_slice(&q);
                }
            }
        }
        if !bill_cpu.is_empty() {
            self.queue
                .write_buffer(&self.bill_vb, 0, bytemuck::cast_slice(&bill_cpu));
        }

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

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

            if self.sprite_ready && !bill_cpu.is_empty() {
                let vcount = bill_cpu.len() as u32;
                let icount = (vcount / 4) * 6;
                let vb_bytes = bill_cpu.len() as u64 * std::mem::size_of::<BillVertex>() as u64;
                let ib_bytes = icount as u64 * 4;
                pass.set_pipeline(&self.bill_pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_bind_group(1, &self.bill_bind_group, &[]);
                pass.set_vertex_buffer(0, self.bill_vb.slice(0..vb_bytes));
                pass.set_index_buffer(self.bill_ib.slice(0..ib_bytes), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..icount, 0, 0..1);
            }
            Self::draw_npc_billboard(
                &self.queue,
                &mut pass,
                cam_pos,
                boss,
                self.boss_ready,
                &self.bill_npc_pipeline,
                &self.bind_group,
                &self.boss_bind_group,
                &self.boss_vb,
                &self.boss_ib,
            );
            Self::draw_npc_billboard(
                &self.queue,
                &mut pass,
                cam_pos,
                rival,
                self.rival_ready,
                &self.bill_npc_pipeline,
                &self.bind_group,
                &self.rival_bind_group,
                &self.rival_vb,
                &self.rival_ib,
            );
        }

        {
            let hu = HudUniform {
                weapon: weapon_hud.weapon_id,
                flash: weapon_hud.flash.clamp(0.0, 1.0),
                bob: weapon_hud.bob,
                _pad: 0.0,
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
