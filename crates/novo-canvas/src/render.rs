//! wgpu renderer for canvas meshes.
//!
//! One [`CanvasRenderer`] holds the pipeline and is shared by every canvas.
//! Each canvas on screen owns a [`GpuCanvas`] with its uniforms and buffers.
//! The scene mesh is uploaded only when the scene revision changes; panning
//! and zooming rewrite a 48-byte uniform, so a static page of any size costs
//! one draw call per visible layer run.

use crate::style::Color;
use crate::tessellate::{Mesh, SceneMesh, Vertex};
use crate::viewport::Viewport;
use bytemuck::{Pod, Zeroable};
use std::ops::Range;
use std::sync::Arc;

/// WGSL source of the canvas shader.
pub const SHADER: &str = include_str!("canvas.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Uniforms {
    center: [f32; 2],
    scale: [f32; 2],
    px_to_clip: [f32; 2],
    zoom_px: f32,
    pixels_per_point: f32,
    srgb_target: u32,
    _pad: [u32; 3],
}

impl Uniforms {
    fn new(view: &Viewport, pixels_per_point: f32, srgb_target: bool) -> Self {
        let (center, scale) = view.clip_transform();
        let ppp = pixels_per_point.max(f32::EPSILON);
        let size_px = view.size.max(glam::Vec2::ONE) * ppp;
        Self {
            center: center.to_array(),
            scale: scale.to_array(),
            px_to_clip: [2.0 / size_px.x, -2.0 / size_px.y],
            zoom_px: view.zoom * ppp,
            pixels_per_point: ppp,
            srgb_target: u32::from(srgb_target),
            _pad: [0; 3],
        }
    }
}

/// Target description the pipeline must match (egui's render pass).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetConfig {
    pub format: wgpu::TextureFormat,
    pub msaa_samples: u32,
    pub depth_format: Option<wgpu::TextureFormat>,
}

impl TargetConfig {
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            msaa_samples: 1,
            depth_format: None,
        }
    }
}

pub struct CanvasRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    srgb_target: bool,
}

impl CanvasRenderer {
    pub fn new(device: &wgpu::Device, target: TargetConfig) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("novo_canvas_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("novo_canvas_uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size_of::<Uniforms>() as u64),
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("novo_canvas_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("novo_canvas_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32,
                        3 => Float32,
                        4 => Float32,
                        5 => Unorm8x4,
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: target.depth_format.map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: target.msaa_samples.max(1),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            bind_group_layout,
            srgb_target: target.format.is_srgb(),
        }
    }

    pub fn create_canvas(&self, device: &wgpu::Device) -> GpuCanvas {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("novo_canvas_uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("novo_canvas_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        GpuCanvas {
            uniforms,
            bind_group,
            srgb_target: self.srgb_target,
            scene: GpuMesh::new("novo_canvas_scene"),
            scene_revision: None,
            scene_ranges: Vec::new(),
            highlight: GpuMesh::new("novo_canvas_highlight"),
            highlight_key: None,
            dynamic: GpuMesh::new("novo_canvas_dynamic"),
            under: 0..0,
            over: 0..0,
        }
    }

    /// Records the draw calls for one canvas. The render pass viewport must
    /// already cover the canvas rectangle.
    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>, canvas: &GpuCanvas) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &canvas.bind_group, &[]);
        if canvas.dynamic.bind(pass) && !canvas.under.is_empty() {
            pass.draw_indexed(canvas.under.clone(), 0, 0..1);
        }
        if canvas.scene.bind(pass) {
            for r in &canvas.scene_ranges {
                pass.draw_indexed(r.clone(), 0, 0..1);
            }
        }
        if canvas.highlight.bind(pass) {
            pass.draw_indexed(0..canvas.highlight.index_count, 0, 0..1);
        }
        if canvas.dynamic.bind(pass) && !canvas.over.is_empty() {
            pass.draw_indexed(canvas.over.clone(), 0, 0..1);
        }
    }
}

/// Everything one canvas needs to draw a frame.
#[derive(Clone, Debug)]
pub struct FrameData {
    pub viewport: Viewport,
    pub pixels_per_point: f32,
    /// The tessellated scene; re-uploaded only when its revision changes.
    pub scene: Option<Arc<SceneMesh>>,
    /// Index ranges of `scene` to draw (from [`SceneMesh::visible_ranges`]).
    pub scene_ranges: Vec<Range<u32>>,
    /// Drawn right above the scene and cached by key: the selection
    /// highlight, which can be large when much is selected.
    pub highlight: Option<Arc<KeyedMesh>>,
    /// Drawn below the scene: background and grid.
    pub under: Mesh,
    /// Drawn above the scene: highlights, rubber band, snap marker.
    pub over: Mesh,
}

/// A mesh with a cache key; the GPU copy is refreshed when the key changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyedMesh {
    pub key: u64,
    pub mesh: Mesh,
}

/// Per-canvas GPU state.
pub struct GpuCanvas {
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    srgb_target: bool,
    scene: GpuMesh,
    scene_revision: Option<u64>,
    scene_ranges: Vec<Range<u32>>,
    highlight: GpuMesh,
    highlight_key: Option<u64>,
    dynamic: GpuMesh,
    under: Range<u32>,
    over: Range<u32>,
}

impl GpuCanvas {
    /// Uploads uniforms, the scene mesh if it changed, and the per-frame meshes.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &FrameData) {
        let u = Uniforms::new(&frame.viewport, frame.pixels_per_point, self.srgb_target);
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));

        match &frame.scene {
            Some(sm) => {
                if self.scene_revision != Some(sm.revision) {
                    self.scene.upload(device, queue, &sm.mesh);
                    self.scene_revision = Some(sm.revision);
                }
                let n = sm.mesh.indices.len() as u32;
                self.scene_ranges = frame
                    .scene_ranges
                    .iter()
                    .map(|r| r.start.min(n)..r.end.min(n))
                    .filter(|r| !r.is_empty())
                    .collect();
            }
            None => {
                self.scene_ranges.clear();
            }
        }

        match &frame.highlight {
            Some(h) => {
                if self.highlight_key != Some(h.key) {
                    self.highlight.upload(device, queue, &h.mesh);
                    self.highlight_key = Some(h.key);
                }
            }
            None => {
                self.highlight.index_count = 0;
                self.highlight_key = None;
            }
        }

        // Pack under and over into one buffer.
        let mut dynamic = frame.under.clone();
        let under_end = dynamic.indices.len() as u32;
        let base = dynamic.vertices.len() as u32;
        dynamic.vertices.extend_from_slice(&frame.over.vertices);
        dynamic
            .indices
            .extend(frame.over.indices.iter().map(|i| i + base));
        self.under = 0..under_end;
        self.over = under_end..dynamic.indices.len() as u32;
        self.dynamic.upload(device, queue, &dynamic);
    }

    /// The scene revision currently on the GPU.
    pub fn scene_revision(&self) -> Option<u64> {
        self.scene_revision
    }
}

/// A growable vertex and index buffer pair.
struct GpuMesh {
    label: &'static str,
    vertices: Option<wgpu::Buffer>,
    indices: Option<wgpu::Buffer>,
    index_count: u32,
}

impl GpuMesh {
    fn new(label: &'static str) -> Self {
        Self {
            label,
            vertices: None,
            indices: None,
            index_count: 0,
        }
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, mesh: &Mesh) {
        self.index_count = mesh.indices.len() as u32;
        if mesh.is_empty() {
            return;
        }
        let vbytes: &[u8] = bytemuck::cast_slice(&mesh.vertices);
        let ibytes: &[u8] = bytemuck::cast_slice(&mesh.indices);
        ensure(
            device,
            &mut self.vertices,
            self.label,
            vbytes.len(),
            wgpu::BufferUsages::VERTEX,
        );
        ensure(
            device,
            &mut self.indices,
            self.label,
            ibytes.len(),
            wgpu::BufferUsages::INDEX,
        );
        if let (Some(v), Some(i)) = (&self.vertices, &self.indices) {
            queue.write_buffer(v, 0, vbytes);
            queue.write_buffer(i, 0, ibytes);
        }
    }

    fn bind(&self, pass: &mut wgpu::RenderPass<'_>) -> bool {
        match (&self.vertices, &self.indices) {
            (Some(v), Some(i)) if self.index_count > 0 => {
                pass.set_vertex_buffer(0, v.slice(..));
                pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
                true
            }
            _ => false,
        }
    }
}

fn ensure(
    device: &wgpu::Device,
    buf: &mut Option<wgpu::Buffer>,
    label: &'static str,
    len: usize,
    usage: wgpu::BufferUsages,
) {
    let needed = (len as u64).next_power_of_two().max(4096);
    if buf.as_ref().is_some_and(|b| b.size() >= len as u64) {
        return;
    }
    *buf = Some(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: needed,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }));
}

/// A full-canvas background quad in world coordinates.
pub fn background(mesh: &mut Mesh, view: &Viewport, color: Color) {
    let r = view.visible_world().expand(1.0);
    crate::tessellate::fill_polygon(
        mesh,
        &[
            r.min,
            glam::Vec2::new(r.max.x, r.min.y),
            r.max,
            glam::Vec2::new(r.min.x, r.max.y),
        ],
        color,
        true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_is_valid_wgsl() {
        let module = naga::front::wgsl::parse_str(SHADER).expect("WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("WGSL validates");
    }

    #[test]
    fn uniform_layout_matches_shader() {
        let module = naga::front::wgsl::parse_str(SHADER).unwrap();
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).unwrap();
        let handle = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Uniforms"))
            .unwrap()
            .0;
        assert_eq!(layouter[handle].size as usize, size_of::<Uniforms>());
    }

    #[test]
    fn uniforms_map_center_to_clip_origin() {
        let v = Viewport {
            center: glam::Vec2::new(10.0, 20.0),
            zoom: 4.0,
            size: glam::Vec2::new(400.0, 200.0),
            ..Default::default()
        };
        let u = Uniforms::new(&v, 2.0, false);
        // A point one world unit right of centre lands 4 points = 8 px right,
        // which is 8 / (400 * 2) * 2 clip units.
        let clip_x = (11.0 - u.center[0]) * u.scale[0];
        assert!((clip_x - 0.02).abs() < 1e-6);
        assert_eq!(u.zoom_px, 8.0);
        assert!((u.px_to_clip[0] - 2.0 / 800.0).abs() < 1e-9);
        assert!(u.scale[1] < 0.0, "y is flipped");
    }
}
