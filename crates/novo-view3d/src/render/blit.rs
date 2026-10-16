//! Copies a finished 3D image into the host's render pass (egui's), inside the
//! rectangle the host has set as the viewport.

use super::{BLIT_SHADER, ViewTarget};
use wgpu::util::DeviceExt;

pub struct Blitter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    srgb_target: wgpu::Buffer,
}

/// The binding of one view's image; rebuilt when its texture is recreated.
pub struct BlitBind {
    generation: u64,
    group: wgpu::BindGroup,
}

impl BlitBind {
    pub fn is_current(&self, target: &ViewTarget) -> bool {
        self.generation == target.generation
    }
}

impl Blitter {
    /// `target` is the format of the host's pass; `samples` its sample count
    /// (eframe's `multisampling`, 1 when off).
    pub fn new(device: &wgpu::Device, target: wgpu::TextureFormat, samples: u32) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("novo_view3d_blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("novo_view3d_blit"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("novo_view3d_blit"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("novo_view3d_blit"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: samples.max(1),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("novo_view3d_blit"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let flag: [f32; 4] = [f32::from(target.is_srgb()), 0.0, 0.0, 0.0];
        let srgb_target = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("novo_view3d_blit"),
            contents: bytemuck::bytes_of(&flag),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        Self {
            pipeline,
            layout,
            sampler,
            srgb_target,
        }
    }

    /// Binds a target's image. `None` before the target has been rendered.
    pub fn bind(&self, device: &wgpu::Device, target: &ViewTarget) -> Option<BlitBind> {
        let view = target.color_view()?;
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("novo_view3d_blit"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.srgb_target.as_entire_binding(),
                },
            ],
        });
        Some(BlitBind {
            generation: target.generation,
            group,
        })
    }

    /// Draws the image over the pass's current viewport.
    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>, bind: &BlitBind) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind.group, &[]);
        pass.draw(0..3, 0..1);
    }
}
