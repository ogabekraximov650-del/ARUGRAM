//! wgpu chizuvchi: ko'rinishga (surface yoki oflayn tekstura) mustaqil.
//! Tartib: soya xaritasi → osmon + relyef + sahna → (chuqurlik tozalanib) qurol + HUD.

use crate::camera::Camera;
use crate::hud::{HudVertex, MAX_HUD_VERTICES};
use crate::mesh::{self, MeshVertex};
use crate::scene::{Instance, Lists, Mat, Mesh, MESH_COUNT};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const MAX_DYNAMIC: usize = 1024;
pub const MAX_VIEWMODEL: usize = 160;
pub const SHADOW_SIZE: u32 = 2048;
/// Ko'rinishni tanlash uchun dunyo katakchalari (relyef bo'laklariga mos: 10 katak x 3 m).
const CELL: f32 = 30.0;
const GRID: usize = 12;
const CULL_MARGIN: f32 = 14.0;
/// Shundan kattaroq jismlar (yo'llar) har doim chiziladi.
const ALWAYS_SIZE: f32 = 24.0;
const MAIN_DISTANCE: f32 = 240.0;
const SHADOW_RADIUS: f32 = 100.0;
/// Quyosh yo'nalishi (yorug'lik tarqaladigan tomon).
pub const SUN_DIR: Vec3 = Vec3::new(-0.45, -0.75, -0.40);

const ALL_MESHES: [Mesh; MESH_COUNT] = [Mesh::Cube, Mesh::Sphere, Mesh::Cylinder, Mesh::Cone, Mesh::Prism, Mesh::Frustum];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    light_vp: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    sun_dir: [f32; 4],
    fog: [f32; 4],
    misc: [f32; 4],
}

/// Har kadr uzatiladigan o'zgaruvchan sahna.
pub struct Frame<'a> {
    pub dynamic: &'a Lists,
    /// Har mesh turi uchun: dinamik ro'yxatning boshidagi nechtasi soya tashlaydi.
    pub dynamic_shadow: [usize; MESH_COUNT],
    /// Har mesh turi uchun: dinamik ro'yxat boshidagi nechtasiga anime konturi chiziladi (qiz).
    pub outline: [usize; MESH_COUNT],
    /// Birinchi shaxs qurol/qo'llar (kamera fazosida: x o'ng, y tepa, -z oldinga).
    pub viewmodel: &'a Lists,
    pub hud: &'a [HudVertex],
    pub time: f32,
}

struct MeshBuf {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    index_count: u32,
    static_buf: wgpu::Buffer,
    /// Har katakcha uchun (boshlanish, soni); oxirgisi — har doim chiziladigan katta jismlar.
    ranges: Vec<(u32, u32)>,
    dyn_buf: wgpu::Buffer,
    vm_buf: wgpu::Buffer,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    scene_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    outline_pipeline: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,
    shadow_view: wgpu::TextureView,
    globals_buf: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    shadow_bind: wgpu::BindGroup,
    vm_buf_globals: wgpu::Buffer,
    vm_bind: wgpu::BindGroup,
    screen_buf: wgpu::Buffer,
    screen_bind: wgpu::BindGroup,
    meshes: Vec<MeshBuf>,
    terrain_vb: wgpu::Buffer,
    terrain_ib: wgpu::Buffer,
    terrain_ranges: Vec<(u32, u32)>,
    terrain_inst: wgpu::Buffer,
    hud_buf: wgpu::Buffer,
    depth_view: wgpu::TextureView,
    width: u32,
    height: u32,
}

fn cell_of(x: f32, z: f32) -> usize {
    let ix = (((x + crate::terrain::EXTENT) / CELL).floor() as i32).clamp(0, GRID as i32 - 1) as usize;
    let iz = (((z + crate::terrain::EXTENT) / CELL).floor() as i32).clamp(0, GRID as i32 - 1) as usize;
    iz * GRID + ix
}

/// Katakchaning (x0,z0)-(x1,z1) chegarasi.
fn cell_rect(c: usize) -> (f32, f32, f32, f32) {
    let (ix, iz) = ((c % GRID) as f32, (c / GRID) as f32);
    let x0 = -crate::terrain::EXTENT + ix * CELL;
    let z0 = -crate::terrain::EXTENT + iz * CELL;
    (x0, z0, x0 + CELL, z0 + CELL)
}

/// Ko'rish piramidasi tekisliklari (ichkarisi: n·p + d >= 0), chuqurlik 0..1.
fn frustum_planes(vp: &Mat4) -> [glam::Vec4; 6] {
    let r = |i: usize| glam::Vec4::new(vp.x_axis[i], vp.y_axis[i], vp.z_axis[i], vp.w_axis[i]);
    let (r0, r1, r2, r3) = (r(0), r(1), r(2), r(3));
    let mut p = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
    for pl in p.iter_mut() {
        let l = pl.truncate().length().max(1e-6);
        *pl /= l;
    }
    p
}

fn box_in_frustum(planes: &[glam::Vec4; 6], min: Vec3, max: Vec3) -> bool {
    for pl in planes {
        let p = Vec3::new(
            if pl.x >= 0.0 { max.x } else { min.x },
            if pl.y >= 0.0 { max.y } else { min.y },
            if pl.z >= 0.0 { max.z } else { min.z },
        );
        if pl.truncate().dot(p) + pl.w < 0.0 {
            return false;
        }
    }
    true
}

/// Ko'rinadigan katakchalar (oxirgi element — har doim chiziladigan katta jismlar).
fn visible_cells(cam: Vec3, planes: Option<&[glam::Vec4; 6]>, radius: f32) -> Vec<bool> {
    let mut keep = vec![false; GRID * GRID + 1];
    keep[GRID * GRID] = true;
    for c in 0..GRID * GRID {
        let (x0, z0, x1, z1) = cell_rect(c);
        let (cx, cz) = ((x0 + x1) * 0.5, (z0 + z1) * 0.5);
        let reach = radius + CELL * 0.75;
        if (cx - cam.x).abs() > reach || (cz - cam.z).abs() > reach {
            continue;
        }
        if let Some(pl) = planes {
            let min = Vec3::new(x0 - CULL_MARGIN, -12.0, z0 - CULL_MARGIN);
            let max = Vec3::new(x1 + CULL_MARGIN, 45.0, z1 + CULL_MARGIN);
            if !box_in_frustum(pl, min, max) {
                continue;
            }
        }
        keep[c] = true;
    }
    keep
}

/// Ketma-ket diapazonlarni birlashtiradi (chizish chaqiruvlari kamayadi).
fn merged(ranges: &[(u32, u32)], keep: &[bool]) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for (c, &(s, n)) in ranges.iter().enumerate() {
        if n == 0 || !keep.get(c).copied().unwrap_or(false) {
            continue;
        }
        if let Some(last) = out.last_mut() {
            if last.0 + last.1 == s {
                last.1 += n;
                continue;
            }
        }
        out.push((s, n));
    }
    out
}

fn make_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

/// Quyosh nuri matritsasi: o'yinchi atrofidagi ±60 m, soya xaritasi pikseliga yaxlitlangan (titramaydi).
fn light_matrix(center: Vec3, sun: Vec3) -> Mat4 {
    let ext = 60.0;
    let texel = 2.0 * ext / SHADOW_SIZE as f32;
    let view0 = Mat4::look_to_rh(Vec3::ZERO, sun, Vec3::Y);
    let c = view0.transform_point3(center);
    let snapped = Vec3::new((c.x / texel).round() * texel, (c.y / texel).round() * texel, c.z);
    let proj = Mat4::orthographic_rh(-ext, ext, -ext, ext, -150.0, 150.0);
    proj * Mat4::from_translation(-snapped) * view0
}

fn lin(c: [f32; 3]) -> [f32; 3] {
    [c[0].powf(2.2), c[1].powf(2.2), c[2].powf(2.2)]
}

fn grow_write(queue: &wgpu::Queue, buf: &wgpu::Buffer, items: &[Instance], cap: usize) -> u32 {
    let n = items.len().min(cap);
    if n > 0 {
        queue.write_buffer(buf, 0, bytemuck::cast_slice(&items[..n]));
    }
    n as u32
}

impl Renderer {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        statics: &Lists,
    ) -> Self {
        let uniform_entry = |vis| wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: vis,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let g_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[uniform_entry(wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT)],
        });
        let s_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("screen"),
            entries: &[uniform_entry(wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT)],
        });
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene"),
            entries: &[
                uniform_entry(wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow"),
            size: wgpu::Extent3d { width: SHADOW_SIZE, height: SHADOW_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_smp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let mk_globals = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<Globals>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let globals_buf = mk_globals("globals");
        let vm_buf_globals = mk_globals("vm_globals");
        let screen_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screen"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene_bind = |buf: &wgpu::Buffer, label| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &scene_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_smp) },
                ],
            })
        };
        let globals_bind = scene_bind(&globals_buf, "scene");
        let vm_bind = scene_bind(&vm_buf_globals, "viewmodel");
        let shadow_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow"),
            layout: &g_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }],
        });
        let screen_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("screen"),
            layout: &s_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: screen_buf.as_entire_binding() }],
        });

        let scene_sh = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let hud_sh = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hud"),
            source: wgpu::ShaderSource::Wgsl(include_str!("hud.wgsl").into()),
        });
        let scene_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[&scene_layout],
            push_constant_ranges: &[],
        });
        let shadow_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow"),
            bind_group_layouts: &[&g_layout],
            push_constant_ranges: &[],
        });
        let hud_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hud"),
            bind_group_layouts: &[&s_layout],
            push_constant_ranges: &[],
        });

        let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let inst_attrs = wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4];
        let vertex_layouts = [
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<MeshVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &mesh_attrs,
            },
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Instance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &inst_attrs,
            },
        ];
        let depth_state = |write: bool, cmp, bias| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: write,
            depth_compare: cmp,
            stencil: Default::default(),
            bias,
        };
        let scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState {
                module: &scene_sh,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &vertex_layouts,
            },
            fragment: Some(wgpu::FragmentState {
                module: &scene_sh,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(depth_state(true, wgpu::CompareFunction::Less, Default::default())),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_pl),
            vertex: wgpu::VertexState {
                module: &scene_sh,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &vertex_layouts,
            },
            fragment: None,
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(depth_state(
                true,
                wgpu::CompareFunction::Less,
                wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            )),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState {
                module: &scene_sh,
                entry_point: Some("vs_sky"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &scene_sh,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: Some(depth_state(false, wgpu::CompareFunction::LessEqual, Default::default())),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let outline_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("outline"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState {
                module: &scene_sh,
                entry_point: Some("vs_outline"),
                compilation_options: Default::default(),
                buffers: &vertex_layouts,
            },
            fragment: Some(wgpu::FragmentState {
                module: &scene_sh,
                entry_point: Some("fs_outline"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Front), ..Default::default() },
            depth_stencil: Some(depth_state(true, wgpu::CompareFunction::Less, Default::default())),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let hud_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
        let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud"),
            layout: Some(&hud_pl),
            vertex: wgpu::VertexState {
                module: &hud_sh,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<HudVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &hud_attrs,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &hud_sh,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(depth_state(false, wgpu::CompareFunction::Always, Default::default())),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let inst_size = std::mem::size_of::<Instance>();
        let mut meshes = Vec::new();
        for (k, m) in ALL_MESHES.iter().enumerate() {
            let (vs, is) = mesh::build(*m);
            // katakchalar bo'yicha saralash: ko'rinmaydigan katakchani chizmaslik uchun
            let mut keyed: Vec<(usize, Instance)> = statics.0[k]
                .iter()
                .map(|i| {
                    let big = i.scale[0].max(i.scale[1]).max(i.scale[2]) > ALWAYS_SIZE;
                    (if big { GRID * GRID } else { cell_of(i.pos[0], i.pos[2]) }, *i)
                })
                .collect();
            keyed.sort_by_key(|(c, _)| *c);
            let mut ranges = vec![(0u32, 0u32); GRID * GRID + 1];
            for (pos, (c, _)) in keyed.iter().enumerate() {
                if ranges[*c].1 == 0 {
                    ranges[*c].0 = pos as u32;
                }
                ranges[*c].1 += 1;
            }
            let items: Vec<Instance> = keyed.into_iter().map(|(_, i)| i).collect();
            let static_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("static"),
                size: (items.len().max(1) * inst_size) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if !items.is_empty() {
                queue.write_buffer(&static_buf, 0, bytemuck::cast_slice(&items));
            }
            meshes.push(MeshBuf {
                vb: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("mesh_v"),
                    contents: bytemuck::cast_slice(&vs),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                ib: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("mesh_i"),
                    contents: bytemuck::cast_slice(&is),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                index_count: is.len() as u32,
                static_buf,
                ranges,
                dyn_buf: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("dynamic"),
                    size: (MAX_DYNAMIC * inst_size) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                vm_buf: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("viewmodel"),
                    size: (MAX_VIEWMODEL * inst_size) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
            });
        }

        let (tv, ti, terrain_ranges) = mesh::terrain_chunks(crate::terrain::EXTENT, 3.0, 10);
        let terrain_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_v"),
            contents: bytemuck::cast_slice(&tv),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let terrain_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_i"),
            contents: bytemuck::cast_slice(&ti),
            usage: wgpu::BufferUsages::INDEX,
        });
        let terrain_inst = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_inst"),
            contents: bytemuck::bytes_of(&Instance::new(Vec3::ZERO, Vec3::ONE, [0.33, 0.52, 0.27], Mat::Grass, Quat::IDENTITY)),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let hud_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud"),
            size: (MAX_HUD_VERTICES * std::mem::size_of::<HudVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let depth_view = make_depth(&device, width, height);

        Self {
            device,
            queue,
            scene_pipeline,
            shadow_pipeline,
            sky_pipeline,
            outline_pipeline,
            hud_pipeline,
            shadow_view,
            globals_buf,
            globals_bind,
            shadow_bind,
            vm_buf_globals,
            vm_bind,
            screen_buf,
            screen_bind,
            meshes,
            terrain_vb,
            terrain_ib,
            terrain_ranges,
            terrain_inst,
            hud_buf,
            depth_view,
            width,
            height,
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w.max(1);
        self.height = h.max(1);
        self.depth_view = make_depth(&self.device, self.width, self.height);
    }

    pub fn render(&mut self, target: &wgpu::TextureView, cam: &Camera, frame: &Frame) {
        let aspect = self.width as f32 / self.height as f32;
        let sun = SUN_DIR.normalize();
        let vp = cam.view_proj(aspect);
        let fog = lin([0.78, 0.86, 0.95]);
        let globals = Globals {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            light_vp: light_matrix(cam.pos, sun).to_cols_array_2d(),
            cam_pos: [cam.pos.x, cam.pos.y, cam.pos.z, 0.0],
            sun_dir: [sun.x, sun.y, sun.z, 0.0],
            fog: [fog[0], fog[1], fog[2], 0.0045],
            misc: [frame.time, 0.0, 0.0, 0.0],
        };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        // qurol: kamera fazosi (ko'rish matritsasi birlik), alohida proyeksiya, quyosh yo'nalishi kamera fazosiga o'tkazilgan
        let view_rot = Mat4::look_to_rh(Vec3::ZERO, cam.forward(), Vec3::Y);
        let sun_view = view_rot.transform_vector3(sun);
        let vm_proj = Mat4::perspective_rh(58f32.to_radians(), aspect.max(0.01), 0.02, 10.0);
        let vm_globals = Globals {
            view_proj: vm_proj.to_cols_array_2d(),
            inv_view_proj: vm_proj.inverse().to_cols_array_2d(),
            light_vp: Mat4::IDENTITY.to_cols_array_2d(),
            cam_pos: [0.0, 0.0, 0.0, 1.0],
            sun_dir: [sun_view.x, sun_view.y, sun_view.z, 0.0],
            fog: [fog[0], fog[1], fog[2], 0.0],
            misc: [frame.time, 0.0, 0.0, 0.0],
        };
        self.queue.write_buffer(&self.vm_buf_globals, 0, bytemuck::bytes_of(&vm_globals));
        self.queue.write_buffer(
            &self.screen_buf,
            0,
            bytemuck::cast_slice(&[self.width as f32, self.height as f32, 0.0, 0.0]),
        );

        let mut dyn_n = [0u32; MESH_COUNT];
        let mut vm_n = [0u32; MESH_COUNT];
        for k in 0..MESH_COUNT {
            dyn_n[k] = grow_write(&self.queue, &self.meshes[k].dyn_buf, &frame.dynamic.0[k], MAX_DYNAMIC);
            vm_n[k] = grow_write(&self.queue, &self.meshes[k].vm_buf, &frame.viewmodel.0[k], MAX_VIEWMODEL);
        }
        let hud_n = frame.hud.len().min(MAX_HUD_VERTICES);
        if hud_n > 0 {
            self.queue.write_buffer(&self.hud_buf, 0, bytemuck::cast_slice(&frame.hud[..hud_n]));
        }

        // ko'rinishni tanlash (frustum + masofa); soya uchun o'yinchi atrofi
        let planes = frustum_planes(&vp);
        let keep_main = visible_cells(cam.pos, Some(&planes), MAIN_DISTANCE);
        let keep_shadow = visible_cells(cam.pos, None, SHADOW_RADIUS);
        let terrain_main = merged(&self.terrain_ranges, &keep_main);
        let terrain_shadow = merged(&self.terrain_ranges, &keep_shadow);

        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        // 1) soya xaritasi
        {
            let mut sp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            sp.set_pipeline(&self.shadow_pipeline);
            sp.set_bind_group(0, &self.shadow_bind, &[]);
            sp.set_vertex_buffer(0, self.terrain_vb.slice(..));
            sp.set_vertex_buffer(1, self.terrain_inst.slice(..));
            sp.set_index_buffer(self.terrain_ib.slice(..), wgpu::IndexFormat::Uint16);
            for (s, n) in &terrain_shadow {
                sp.draw_indexed(*s..*s + *n, 0, 0..1);
            }
            for (k, mb) in self.meshes.iter().enumerate() {
                sp.set_vertex_buffer(0, mb.vb.slice(..));
                sp.set_index_buffer(mb.ib.slice(..), wgpu::IndexFormat::Uint16);
                sp.set_vertex_buffer(1, mb.static_buf.slice(..));
                for (s, n) in merged(&mb.ranges, &keep_shadow) {
                    sp.draw_indexed(0..mb.index_count, 0, s..s + n);
                }
                let ds = (frame.dynamic_shadow[k] as u32).min(dyn_n[k]);
                if ds > 0 {
                    sp.set_vertex_buffer(1, mb.dyn_buf.slice(..));
                    sp.draw_indexed(0..mb.index_count, 0, 0..ds);
                }
            }
        }

        // 2) asosiy: osmon, relyef, sahna
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.globals_bind, &[]);
            pass.set_pipeline(&self.scene_pipeline);
            pass.set_vertex_buffer(0, self.terrain_vb.slice(..));
            pass.set_vertex_buffer(1, self.terrain_inst.slice(..));
            pass.set_index_buffer(self.terrain_ib.slice(..), wgpu::IndexFormat::Uint16);
            for (s, n) in &terrain_main {
                pass.draw_indexed(*s..*s + *n, 0, 0..1);
            }
            for (k, mb) in self.meshes.iter().enumerate() {
                pass.set_vertex_buffer(0, mb.vb.slice(..));
                pass.set_index_buffer(mb.ib.slice(..), wgpu::IndexFormat::Uint16);
                pass.set_vertex_buffer(1, mb.static_buf.slice(..));
                for (s, n) in merged(&mb.ranges, &keep_main) {
                    pass.draw_indexed(0..mb.index_count, 0, s..s + n);
                }
                if dyn_n[k] > 0 {
                    pass.set_vertex_buffer(1, mb.dyn_buf.slice(..));
                    pass.draw_indexed(0..mb.index_count, 0, 0..dyn_n[k]);
                }
            }
            // anime konturi (qiz)
            pass.set_pipeline(&self.outline_pipeline);
            for (k, mb) in self.meshes.iter().enumerate() {
                let n = (frame.outline[k] as u32).min(dyn_n[k]);
                if n > 0 {
                    pass.set_vertex_buffer(0, mb.vb.slice(..));
                    pass.set_index_buffer(mb.ib.slice(..), wgpu::IndexFormat::Uint16);
                    pass.set_vertex_buffer(1, mb.dyn_buf.slice(..));
                    pass.draw_indexed(0..mb.index_count, 0, 0..n);
                }
            }
            // osmon eng oxirida: faqat bo'sh piksellarda ishlaydi (erta chuqurlik tekshiruvi)
            pass.set_pipeline(&self.sky_pipeline);
            pass.draw(0..3, 0..1);
        }

        // 3) qurol (chuqurlik tozalanadi — devorga kirib ketmaydi) + HUD
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewmodel"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.scene_pipeline);
            pass.set_bind_group(0, &self.vm_bind, &[]);
            for (k, mb) in self.meshes.iter().enumerate() {
                if vm_n[k] > 0 {
                    pass.set_vertex_buffer(0, mb.vb.slice(..));
                    pass.set_vertex_buffer(1, mb.vm_buf.slice(..));
                    pass.set_index_buffer(mb.ib.slice(..), wgpu::IndexFormat::Uint16);
                    pass.draw_indexed(0..mb.index_count, 0, 0..vm_n[k]);
                }
            }
            if hud_n > 0 {
                pass.set_pipeline(&self.hud_pipeline);
                pass.set_bind_group(0, &self.screen_bind, &[]);
                pass.set_vertex_buffer(0, self.hud_buf.slice(..));
                pass.draw(0..hud_n as u32, 0..1);
            }
        }
        self.queue.submit(Some(enc.finish()));
    }
}
