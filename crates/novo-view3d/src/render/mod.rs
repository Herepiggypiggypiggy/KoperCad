//! The wgpu renderer: draws a [`Scene`] into its own colour and depth textures
//! (offscreen), so it does not depend on the host's render pass, can render at
//! a reduced scale, and can produce an image without a window.
//!
//! Requirements are kept to the downlevel baseline: one uniform buffer, vertex
//! buffers (instance data is not read from storage buffers), no compute and no
//! optional features.

use crate::camera::{Camera, Projection};
use crate::cull;
use crate::look::{Highlight, Look, Rgb, mix};
use crate::math::Frustum;
use crate::mesh::{GeometryKey, MeshData, MeshLibrary, profile_mesh, unit_box};
use crate::overlay::Overlay;
use crate::scene::{Item, ItemId, ItemKind, Mark, Profile, Scene, Shape};
use crate::view_state::ViewState;
use crate::wire;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;
use wgpu::util::DeviceExt;

pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const SCENE_SHADER: &str = include_str!("scene.wgsl");
pub const BLIT_SHADER: &str = include_str!("blit.wgsl");

mod blit;
pub use blit::{BlitBind, Blitter};

/// How capable the adapter is, from its reported type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Discrete,
    Integrated,
    /// A software rasteriser (WARP, llvmpipe).
    Software,
}

impl Tier {
    pub fn of(info: &wgpu::AdapterInfo) -> Tier {
        match info.device_type {
            wgpu::DeviceType::DiscreteGpu | wgpu::DeviceType::VirtualGpu => Tier::Discrete,
            wgpu::DeviceType::Cpu => Tier::Software,
            _ => Tier::Integrated,
        }
    }
}

/// Interactive while the camera moves, Settled a moment after it stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Interactive,
    Settled,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quality {
    /// 1 or 4.
    pub msaa: u32,
    /// Share of the widget size the image is rendered at.
    pub scale: f32,
}

impl Quality {
    pub fn new(mode: Mode, tier: Tier) -> Self {
        match (mode, tier) {
            (Mode::Settled, Tier::Software) => Quality {
                msaa: 1,
                scale: 1.0,
            },
            (Mode::Settled, _) => Quality {
                msaa: 4,
                scale: 1.0,
            },
            (Mode::Interactive, Tier::Discrete) => Quality {
                msaa: 1,
                scale: 1.0,
            },
            (Mode::Interactive, Tier::Integrated) => Quality {
                msaa: 1,
                scale: 0.75,
            },
            (Mode::Interactive, Tier::Software) => Quality {
                msaa: 1,
                scale: 0.5,
            },
        }
    }
}

/// Everything one frame reads. Nothing is mutated.
pub struct Frame<'a> {
    pub scene: &'a Scene,
    pub state: &'a ViewState,
    pub camera: &'a Camera,
    pub library: &'a MeshLibrary,
    pub look: &'a Look,
    /// Size of the area the image is shown in, physical pixels.
    pub size_px: (u32, u32),
    pub quality: Quality,
    /// Geometry drawn over the cabinet (see `overlay`). Uploaded again only
    /// when `overlay_revision` differs from the one the target last saw.
    pub overlays: &'a [Overlay],
    pub overlay_revision: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameStats {
    pub visible_items: usize,
    pub draw_calls: u32,
    pub triangles: u64,
    /// CPU time to cull, build instances and record the pass, milliseconds.
    pub cpu_ms: f32,
    /// Size the image was rendered at.
    pub rendered_px: (u32, u32),
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    rows: [[f32; 4]; 3],
    scale: [f32; 3],
    color: [u8; 4],
    edge: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    light: [f32; 4],
    bg_top: [f32; 4],
    bg_bottom: [f32; 4],
    params: [f32; 4],
    nudge: [f32; 4],
    view_dir: [f32; 4],
    viewport: [f32; 4],
}

/// A vertex of an overlay mesh.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct OverlayVertex {
    pos: [f32; 3],
    color: [u8; 4],
}

/// One segment of an overlay polyline.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RibbonSegment {
    a: [f32; 3],
    b: [f32; 3],
    color_a: [u8; 4],
    color_b: [u8; 4],
    width: f32,
}

const OVERLAY_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4];
const RIBBON_ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x3,
    1 => Float32x3,
    2 => Unorm8x4,
    3 => Unorm8x4,
    4 => Float32,
];

const VERTEX_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
const INSTANCE_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    2 => Float32x4,
    3 => Float32x4,
    4 => Float32x4,
    5 => Float32x3,
    6 => Unorm8x4,
    7 => Unorm8x4,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum MeshKey {
    Cube,
    /// Profile, width and height in half millimetres.
    Profile(Profile, u32, u32),
    Part(GeometryKey, u8),
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    edges: wgpu::Buffer,
    edge_count: u32,
    triangles: u32,
}

impl GpuMesh {
    fn upload(device: &wgpu::Device, label: &str, m: &MeshData) -> Self {
        let vertices: Vec<Vertex> = m
            .positions
            .iter()
            .zip(&m.normals)
            .map(|(&pos, &normal)| Vertex { pos, normal })
            .collect();
        let edges: Vec<u32> = m.edges.iter().flatten().copied().collect();
        let buf = |data: &[u8], usage, suffix: &str| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{label}_{suffix}")),
                contents: data,
                usage,
            })
        };
        Self {
            vertices: buf(
                bytemuck::cast_slice(&vertices),
                wgpu::BufferUsages::VERTEX,
                "vertices",
            ),
            indices: buf(
                bytemuck::cast_slice(&m.indices),
                wgpu::BufferUsages::INDEX,
                "indices",
            ),
            index_count: m.indices.len() as u32,
            edges: buf(
                bytemuck::cast_slice(&edges),
                wgpu::BufferUsages::INDEX,
                "edges",
            ),
            edge_count: edges.len() as u32,
            triangles: m.triangles() as u32,
        }
    }
}

struct Pipelines {
    background: wgpu::RenderPipeline,
    faces: wgpu::RenderPipeline,
    faces_blend: wgpu::RenderPipeline,
    edges: wgpu::RenderPipeline,
    overlay_faces: wgpu::RenderPipeline,
    overlay_faces_blend: wgpu::RenderPipeline,
    overlay_ribbons: wgpu::RenderPipeline,
}

/// An overlay on the GPU.
enum GpuOverlay {
    Mesh {
        vertices: wgpu::Buffer,
        indices: wgpu::Buffer,
        index_count: u32,
        translucent: bool,
        order: i32,
    },
    Lines {
        segments: wgpu::Buffer,
        count: u32,
        order: i32,
    },
}

impl GpuOverlay {
    fn order(&self) -> i32 {
        match self {
            GpuOverlay::Mesh { order, .. } | GpuOverlay::Lines { order, .. } => *order,
        }
    }

    fn upload(device: &wgpu::Device, overlay: &Overlay) -> Option<Self> {
        if !overlay.is_valid() {
            return None;
        }
        let buf = |data: &[u8], usage, label: &str| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: data,
                usage,
            })
        };
        match overlay {
            Overlay::Mesh(m) if !m.indices.is_empty() => {
                let vertices: Vec<OverlayVertex> = m
                    .positions
                    .iter()
                    .zip(&m.colors)
                    .map(|(p, &color)| OverlayVertex {
                        pos: p.to_array(),
                        color,
                    })
                    .collect();
                Some(GpuOverlay::Mesh {
                    vertices: buf(
                        bytemuck::cast_slice(&vertices),
                        wgpu::BufferUsages::VERTEX,
                        "novo_view3d_overlay_vertices",
                    ),
                    indices: buf(
                        bytemuck::cast_slice(&m.indices),
                        wgpu::BufferUsages::INDEX,
                        "novo_view3d_overlay_indices",
                    ),
                    index_count: m.indices.len() as u32,
                    translucent: m.translucent,
                    order: m.order,
                })
            }
            Overlay::Lines(l) => {
                let segments: Vec<RibbonSegment> = l
                    .segments()
                    .into_iter()
                    .map(|(a, b)| RibbonSegment {
                        a: l.positions[a as usize].to_array(),
                        b: l.positions[b as usize].to_array(),
                        color_a: l.colors[a as usize],
                        color_b: l.colors[b as usize],
                        width: l.width_px.max(0.5),
                    })
                    .collect();
                (!segments.is_empty()).then(|| GpuOverlay::Lines {
                    count: segments.len() as u32,
                    segments: buf(
                        bytemuck::cast_slice(&segments),
                        wgpu::BufferUsages::VERTEX,
                        "novo_view3d_overlay_segments",
                    ),
                    order: l.order,
                })
            }
            Overlay::Mesh(_) => None,
        }
    }
}

/// Shared by every view: pipelines and the mesh cache.
pub struct Renderer {
    module: wgpu::ShaderModule,
    globals_layout: wgpu::BindGroupLayout,
    pipelines: HashMap<u32, Pipelines>,
    meshes: HashMap<MeshKey, GpuMesh>,
    library_revision: u64,
    max_texture: u32,
}

/// Per-view GPU state: the textures, uniform and instance buffers.
pub struct ViewTarget {
    size: (u32, u32),
    samples: u32,
    color: Option<wgpu::Texture>,
    color_view: Option<wgpu::TextureView>,
    msaa_view: Option<wgpu::TextureView>,
    depth_view: Option<wgpu::TextureView>,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    instances: wgpu::Buffer,
    instance_capacity: u64,
    overlays: Vec<GpuOverlay>,
    overlay_revision: Option<u64>,
    /// Bumped each time the colour texture is recreated; the blitter rebinds.
    pub(crate) generation: u64,
}

impl ViewTarget {
    pub fn new(renderer: &Renderer, device: &wgpu::Device) -> Self {
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("novo_view3d_globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("novo_view3d_globals"),
            layout: &renderer.globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let instance_capacity = 4096 * size_of::<Instance>() as u64;
        Self {
            size: (0, 0),
            samples: 0,
            color: None,
            color_view: None,
            msaa_view: None,
            depth_view: None,
            globals,
            globals_bind,
            instances: instance_buffer(device, instance_capacity),
            instance_capacity,
            overlays: Vec::new(),
            overlay_revision: None,
            generation: 0,
        }
    }

    /// The finished image (single sampled, gamma-encoded RGBA).
    pub fn color(&self) -> Option<&wgpu::Texture> {
        self.color.as_ref()
    }

    pub fn color_view(&self) -> Option<&wgpu::TextureView> {
        self.color_view.as_ref()
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    fn ensure(&mut self, device: &wgpu::Device, size: (u32, u32), samples: u32) {
        if self.size == size && self.samples == samples && self.color.is_some() {
            return;
        }
        let tex = |format, samples, usage, label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = tex(
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            "novo_view3d_color",
        );
        self.color_view = Some(color.create_view(&Default::default()));
        self.color = Some(color);
        self.msaa_view = (samples > 1).then(|| {
            tex(
                COLOR_FORMAT,
                samples,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                "novo_view3d_color_msaa",
            )
            .create_view(&Default::default())
        });
        self.depth_view = Some(
            tex(
                DEPTH_FORMAT,
                samples,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                "novo_view3d_depth",
            )
            .create_view(&Default::default()),
        );
        self.size = size;
        self.samples = samples;
        self.generation += 1;
    }
}

fn instance_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("novo_view3d_instances"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn pipelines(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    samples: u32,
) -> Pipelines {
    let vertex_layout = wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &VERTEX_ATTRS,
    };
    let instance_layout = wgpu::VertexBufferLayout {
        array_stride: size_of::<Instance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &INSTANCE_ATTRS,
    };
    let multisample = wgpu::MultisampleState {
        count: samples,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };
    let depth = |write: bool, compare: wgpu::CompareFunction| wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    };
    let target = |blend: Option<wgpu::BlendState>| {
        [Some(wgpu::ColorTargetState {
            format: COLOR_FORMAT,
            blend,
            write_mask: wgpu::ColorWrites::ALL,
        })]
    };
    let make = |label: &str,
                vs: &str,
                fs: &str,
                buffers: &[Option<wgpu::VertexBufferLayout>],
                topology: wgpu::PrimitiveTopology,
                cull: Option<wgpu::Face>,
                depth_state: wgpu::DepthStencilState,
                blend: Option<wgpu::BlendState>| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology,
                cull_mode: cull,
                ..Default::default()
            },
            depth_stencil: Some(depth_state),
            multisample,
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &target(blend),
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let buffers = [Some(vertex_layout), Some(instance_layout)];
    let overlay_layout = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<OverlayVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &OVERLAY_ATTRS,
    })];
    let ribbon_layout = [Some(wgpu::VertexBufferLayout {
        array_stride: size_of::<RibbonSegment>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &RIBBON_ATTRS,
    })];
    Pipelines {
        background: make(
            "novo_view3d_background",
            "vs_bg",
            "fs_bg",
            &[],
            wgpu::PrimitiveTopology::TriangleList,
            None,
            depth(false, wgpu::CompareFunction::Always),
            None,
        ),
        faces: make(
            "novo_view3d_faces",
            "vs_face",
            "fs_face",
            &buffers,
            wgpu::PrimitiveTopology::TriangleList,
            Some(wgpu::Face::Back),
            depth(true, wgpu::CompareFunction::Greater),
            None,
        ),
        faces_blend: make(
            "novo_view3d_faces_blend",
            "vs_face",
            "fs_face",
            &buffers,
            wgpu::PrimitiveTopology::TriangleList,
            None,
            depth(false, wgpu::CompareFunction::Greater),
            Some(wgpu::BlendState::ALPHA_BLENDING),
        ),
        edges: make(
            "novo_view3d_edges",
            "vs_edge",
            "fs_edge",
            &buffers,
            wgpu::PrimitiveTopology::LineList,
            None,
            depth(false, wgpu::CompareFunction::GreaterEqual),
            Some(wgpu::BlendState::ALPHA_BLENDING),
        ),
        overlay_faces: make(
            "novo_view3d_overlay_faces",
            "vs_overlay",
            "fs_overlay",
            &overlay_layout,
            wgpu::PrimitiveTopology::TriangleList,
            None,
            depth(true, wgpu::CompareFunction::Greater),
            None,
        ),
        overlay_faces_blend: make(
            "novo_view3d_overlay_faces_blend",
            "vs_overlay",
            "fs_overlay",
            &overlay_layout,
            wgpu::PrimitiveTopology::TriangleList,
            None,
            depth(false, wgpu::CompareFunction::Greater),
            Some(wgpu::BlendState::ALPHA_BLENDING),
        ),
        overlay_ribbons: make(
            "novo_view3d_overlay_ribbons",
            "vs_ribbon",
            "fs_ribbon",
            &ribbon_layout,
            wgpu::PrimitiveTopology::TriangleList,
            None,
            depth(false, wgpu::CompareFunction::Greater),
            Some(wgpu::BlendState::ALPHA_BLENDING),
        ),
    }
}

impl Renderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("novo_view3d_scene"),
            source: wgpu::ShaderSource::Wgsl(SCENE_SHADER.into()),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("novo_view3d_globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size_of::<Globals>() as u64),
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("novo_view3d_layout"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let pipelines = [1u32, 4]
            .into_iter()
            .map(|n| (n, pipelines(device, &module, &layout, n)))
            .collect();
        Self {
            module,
            globals_layout,
            pipelines,
            meshes: HashMap::new(),
            library_revision: u64::MAX,
            max_texture: device.limits().max_texture_dimension_2d,
        }
    }

    pub fn shader_module(&self) -> &wgpu::ShaderModule {
        &self.module
    }

    fn mesh(&mut self, device: &wgpu::Device, key: MeshKey, library: &MeshLibrary) -> &GpuMesh {
        self.meshes.entry(key).or_insert_with(|| match key {
            MeshKey::Cube => GpuMesh::upload(device, "cube", &unit_box()),
            MeshKey::Profile(p, w, h) => GpuMesh::upload(
                device,
                "profile",
                &profile_mesh(p, w as f32 * 0.5, h as f32 * 0.5),
            ),
            MeshKey::Part(k, lod) => {
                let m = library
                    .get(k)
                    .and_then(|g| g.lods.get(lod as usize))
                    .cloned()
                    .unwrap_or_else(unit_box);
                GpuMesh::upload(device, "part", &m)
            }
        })
    }

    /// Records one frame into `encoder`. The image is in `target.color()`.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &mut ViewTarget,
        frame: &Frame<'_>,
    ) -> FrameStats {
        let started = Instant::now();
        let mut stats = FrameStats::default();
        let look = frame.look;

        if self.library_revision != frame.library.revision() {
            // Parts may have changed: drop their GPU copies.
            self.meshes.retain(|k, _| !matches!(k, MeshKey::Part(..)));
            self.library_revision = frame.library.revision();
        }

        let scale = frame.quality.scale.clamp(0.25, 1.0);
        let max = self.max_texture.max(1);
        let size = (
            ((frame.size_px.0 as f32 * scale).round() as u32).clamp(1, max),
            ((frame.size_px.1 as f32 * scale).round() as u32).clamp(1, max),
        );
        let samples = if frame.quality.msaa > 1 { 4 } else { 1 };
        target.ensure(device, size, samples);
        stats.rendered_px = size;

        let scene = frame.scene;
        let aspect = size.0 as f32 / size.1 as f32;
        let bounds = scene.bounds();
        let view_proj = frame.camera.view_proj(aspect, &bounds);
        let eye = frame.camera.eye();
        let (_, _, fwd) = frame.camera.basis();
        let ortho = frame.camera.projection == Projection::Orthographic;

        let mut visible = Vec::new();
        cull::visible(scene, frame.state, &view_proj, &mut visible);
        stats.visible_items = visible.len();

        // Build instances grouped by mesh; translucent ones are sorted far to
        // near and drawn one by one.
        let mut opaque: BTreeMap<MeshKey, Vec<Instance>> = BTreeMap::new();
        let mut blended: Vec<(f32, MeshKey, Instance)> = Vec::new();
        for &i in &visible {
            let item = &scene.items[i];
            let world = scene.world_matrix(item);
            let centre = world.transform_point3(item.local_bounds().center());
            let dist = (centre - eye).length();
            let radius = item.size.length() * 0.5;
            let size_px = cull::screen_size_px(frame.camera, radius, dist, size.1 as f32);
            let Some((key, scale)) = self.resolve(item, frame.library, size_px) else {
                continue;
            };
            let (color, edge) = colors(
                item.id,
                item.tint,
                item.mark,
                item.translucent,
                look,
                frame.state,
            );
            let inst = Instance {
                rows: [
                    world.row(0).to_array(),
                    world.row(1).to_array(),
                    world.row(2).to_array(),
                ],
                scale: scale.to_array(),
                color,
                edge,
            };
            if item.translucent {
                blended.push((dist, key, inst));
            } else {
                opaque.entry(key).or_default().push(inst);
            }
        }
        if frame.state.is_visible(ItemKind::Wire) {
            let frustum = Frustum::from_view_proj(&view_proj);
            for wire in &scene.wires {
                if !frustum.intersects(&wire::world_bounds(wire)) {
                    continue;
                }
                let (color, edge) = colors(wire.id, wire.tint, wire.mark, false, look, frame.state);
                let striped = wire
                    .stripe
                    .map(|s| colors(wire.id, Some(s), wire.mark, false, look, frame.state));
                let r = wire.radius.max(0.25);
                for seg in wire::segments(wire) {
                    let dist = (seg.centre() - eye).length();
                    let size_px =
                        cull::screen_size_px(frame.camera, seg.length() * 0.5, dist, size.1 as f32);
                    if size_px < 3.0 {
                        continue;
                    }
                    let mut push = |piece: &wire::Segment, (color, edge): ([u8; 4], [u8; 4])| {
                        let m = piece.frame();
                        opaque.entry(MeshKey::Cube).or_default().push(Instance {
                            rows: [
                                m.row(0).to_array(),
                                m.row(1).to_array(),
                                m.row(2).to_array(),
                            ],
                            scale: [2.0 * r, 2.0 * r, piece.length()],
                            color,
                            edge,
                        });
                    };
                    match striped {
                        // Bands only where the wire is big enough on screen
                        // to show them; far away it is one body colour.
                        Some(stripe) if size_px >= 12.0 => {
                            for (piece, odd) in seg.bands(r * 6.0) {
                                push(&piece, if odd { stripe } else { (color, edge) });
                            }
                        }
                        _ => push(&seg, (color, edge)),
                    }
                }
            }
        }
        blended.sort_by(|a, b| b.0.total_cmp(&a.0));

        let mut data: Vec<Instance> = Vec::with_capacity(visible.len());
        let mut draws: Vec<(MeshKey, std::ops::Range<u32>)> = Vec::new();
        for (key, list) in &opaque {
            let start = data.len() as u32;
            data.extend_from_slice(list);
            draws.push((*key, start..data.len() as u32));
        }
        let opaque_draws = draws.len();
        for (_, key, inst) in &blended {
            let start = data.len() as u32;
            data.push(*inst);
            draws.push((*key, start..start + 1));
        }

        // Make sure every mesh exists before recording (borrow rules).
        for (key, _) in &draws {
            self.mesh(device, *key, frame.library);
        }

        if target.overlay_revision != Some(frame.overlay_revision) {
            target.overlays = frame
                .overlays
                .iter()
                .filter_map(|o| GpuOverlay::upload(device, o))
                .collect();
            target.overlays.sort_by_key(GpuOverlay::order);
            target.overlay_revision = Some(frame.overlay_revision);
        }

        let need = (data.len().max(1) * size_of::<Instance>()) as u64;
        if need > target.instance_capacity {
            target.instance_capacity = need.next_power_of_two();
            target.instances = instance_buffer(device, target.instance_capacity);
        }
        if !data.is_empty() {
            queue.write_buffer(&target.instances, 0, bytemuck::cast_slice(&data));
        }

        let rgb = |c: Rgb| {
            [
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
                1.0,
            ]
        };
        let l = Vec3::from(look.light).normalize_or_zero();
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            eye: eye.extend(1.0).to_array(),
            light: [l.x, l.y, l.z, look.ambient],
            bg_top: rgb(look.background[0]),
            bg_bottom: rgb(look.background[1]),
            params: [look.contact_shade, 25.0, 0.0, f32::from(ortho)],
            nudge: if ortho {
                // 0.6 mm in depth units: the projection maps the depth range
                // to 0..1, so one unit of depth is 1 / |m22| millimetres.
                let m22 = frame
                    .camera
                    .projection_matrix(aspect, &bounds)
                    .z_axis
                    .z
                    .abs();
                [0.0, 0.6 * m22, 0.0, 0.0]
            } else {
                [0.0015, 0.0, 0.0, 0.0]
            },
            view_dir: fwd.extend(0.0).to_array(),
            viewport: [size.0 as f32, size.1 as f32, 0.0, 0.0],
        };
        queue.write_buffer(&target.globals, 0, bytemuck::bytes_of(&globals));

        let pipes = &self.pipelines[&samples];
        let color_view = target.color_view.as_ref().expect("ensured");
        let depth_view = target.depth_view.as_ref().expect("ensured");
        let bg = rgb(look.background[1]);
        let clear = wgpu::Color {
            r: bg[0] as f64,
            g: bg[1] as f64,
            b: bg[2] as f64,
            a: 1.0,
        };
        let color_attachment = match &target.msaa_view {
            Some(msaa) => wgpu::RenderPassColorAttachment {
                view: msaa,
                depth_slice: None,
                resolve_target: Some(color_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Discard,
                },
            },
            None => wgpu::RenderPassColorAttachment {
                view: color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            },
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("novo_view3d"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &target.globals_bind, &[]);
        pass.set_pipeline(&pipes.background);
        pass.draw(0..3, 0..1);
        stats.draw_calls += 1;

        let edges_on = frame.state.display.edges;
        let mut draw_set = |pass: &mut wgpu::RenderPass<'_>,
                            set: &[(MeshKey, std::ops::Range<u32>)],
                            edges: bool| {
            for (key, range) in set {
                let mesh = &self.meshes[key];
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_vertex_buffer(1, target.instances.slice(..));
                if edges {
                    pass.set_index_buffer(mesh.edges.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.edge_count, 0, range.clone());
                } else {
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, range.clone());
                    stats.triangles += mesh.triangles as u64 * (range.end - range.start) as u64;
                }
                stats.draw_calls += 1;
            }
        };
        let (opaque_set, blended_set) = draws.split_at(opaque_draws);
        pass.set_pipeline(&pipes.faces);
        draw_set(&mut pass, opaque_set, false);
        if edges_on {
            pass.set_pipeline(&pipes.edges);
            draw_set(&mut pass, opaque_set, true);
        }
        // Opaque overlays write depth like the scene.
        let (mut overlay_calls, mut overlay_tris) = (0u32, 0u64);
        for overlay in &target.overlays {
            if let GpuOverlay::Mesh {
                vertices,
                indices,
                index_count,
                translucent: false,
                ..
            } = overlay
            {
                pass.set_pipeline(&pipes.overlay_faces);
                pass.set_vertex_buffer(0, vertices.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*index_count, 0, 0..1);
                overlay_calls += 1;
                overlay_tris += u64::from(*index_count / 3);
            }
        }
        pass.set_pipeline(&pipes.faces_blend);
        draw_set(&mut pass, blended_set, false);
        if edges_on {
            pass.set_pipeline(&pipes.edges);
            draw_set(&mut pass, blended_set, true);
        }
        // Translucent overlay meshes and ribbons, low order first.
        for overlay in &target.overlays {
            match overlay {
                GpuOverlay::Mesh {
                    vertices,
                    indices,
                    index_count,
                    translucent: true,
                    ..
                } => {
                    pass.set_pipeline(&pipes.overlay_faces_blend);
                    pass.set_vertex_buffer(0, vertices.slice(..));
                    pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*index_count, 0, 0..1);
                    overlay_calls += 1;
                    overlay_tris += u64::from(*index_count / 3);
                }
                GpuOverlay::Lines {
                    segments, count, ..
                } => {
                    pass.set_pipeline(&pipes.overlay_ribbons);
                    pass.set_vertex_buffer(0, segments.slice(..));
                    pass.draw(0..6, 0..*count);
                    overlay_calls += 1;
                    overlay_tris += u64::from(*count) * 2;
                }
                GpuOverlay::Mesh { .. } => {}
            }
        }
        drop(pass);
        stats.draw_calls += overlay_calls;
        stats.triangles += overlay_tris;

        stats.cpu_ms = started.elapsed().as_secs_f32() * 1000.0;
        stats
    }

    /// The mesh to draw an item with and the per-axis scale for it, or `None`
    /// when it is too small to be worth a draw.
    fn resolve(&self, item: &Item, library: &MeshLibrary, size_px: f32) -> Option<(MeshKey, Vec3)> {
        let q = |v: f32| (v * 2.0).round().max(0.0) as u32;
        match item.shape {
            Shape::Box => cull::lod_for(size_px, 1).map(|_| (MeshKey::Cube, item.size)),
            Shape::Profile(p) => cull::lod_for(size_px, 1).map(|_| {
                (
                    MeshKey::Profile(p, q(item.size.y), q(item.size.z)),
                    Vec3::new(item.size.x, 1.0, 1.0),
                )
            }),
            Shape::Part(k) => match library.get(k) {
                Some(g) => {
                    let lod = cull::lod_for(size_px, g.lods.len())?;
                    let extent = g.lods[lod].bounds.size();
                    let fit = Vec3::new(
                        if extent.x > 1e-6 {
                            item.size.x / extent.x
                        } else {
                            1.0
                        },
                        if extent.y > 1e-6 {
                            item.size.y / extent.y
                        } else {
                            1.0
                        },
                        if extent.z > 1e-6 {
                            item.size.z / extent.z
                        } else {
                            1.0
                        },
                    );
                    Some((MeshKey::Part(k, lod as u8), fit))
                }
                // Not there yet: the box, so the view never waits.
                None => cull::lod_for(size_px, 1).map(|_| (MeshKey::Cube, item.size)),
            },
        }
    }
}

fn apply(base: Rgb, h: &Highlight) -> (Rgb, Rgb) {
    (mix(base, h.tint, h.tint_amount), h.edge)
}

/// Face and edge colour of an item: kind, maker colour, mark, then the
/// related, hover and selection highlights on top.
fn colors(
    id: ItemId,
    tint: Option<Rgb>,
    mark: Mark,
    translucent: bool,
    look: &Look,
    state: &ViewState,
) -> ([u8; 4], [u8; 4]) {
    let style = look.surface(id.kind);
    let mut face = tint.unwrap_or(style.color);
    let mut edge = style.edge;
    let mark = look.mark(mark);
    if let Some(t) = mark.tint {
        face = mix(face, t, mark.tint_amount);
    }
    if let Some(e) = mark.edge {
        edge = e;
    }
    if state.related.contains(&id) {
        (face, edge) = apply(face, &look.related);
    }
    if state.hover == Some(id) {
        (face, edge) = apply(face, &look.hover);
    }
    if state.selection.contains(&id) {
        (face, edge) = apply(face, &look.selection);
    }
    let (alpha, edge_alpha) = if translucent {
        ((style.alpha * 255.0) as u8, 150)
    } else {
        (255, 255)
    };
    (
        [face[0], face[1], face[2], alpha],
        [edge[0], edge[1], edge[2], edge_alpha],
    )
}

/// Copies the image of a target into CPU memory: tightly packed RGBA8 rows,
/// top row first. Blocks until the GPU is done; for tests, examples and
/// screenshots, not for frames.
pub fn read_rgba(device: &wgpu::Device, queue: &wgpu::Queue, target: &ViewTarget) -> Vec<u8> {
    let (w, h) = target.size();
    let padded = (w * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("novo_view3d_readback"),
        size: (padded * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: target.color().expect("rendered"),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let mapped = slice.get_mapped_range().expect("mapped range");
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded) as usize;
        out.extend_from_slice(&mapped[start..start + (w * 4) as usize]);
    }
    out
}

/// A matrix helper for tests that need the same view-projection as a frame.
pub fn frame_matrix(frame: &Frame<'_>) -> Mat4 {
    let aspect = frame.size_px.0 as f32 / frame.size_px.1.max(1) as f32;
    frame.camera.view_proj(aspect, &frame.scene.bounds())
}
