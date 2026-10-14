//! Headless GPU tests: render to an offscreen texture and read pixels back.
//! They skip (pass with a note) on machines without any wgpu adapter.

use novo_canvas::render::{CanvasRenderer, FrameData, TargetConfig, background};
use novo_canvas::{Color, Mesh, Primitive, Scene, Shape, Stroke, Tessellator, Vec2, Viewport};
use std::sync::Arc;
use std::time::Instant;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: String,
}

fn gpu() -> Option<Gpu> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        ..Default::default()
    }))
    .ok()?;
    let info = adapter.get_info();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    Some(Gpu {
        device,
        queue,
        adapter: format!("{} ({:?})", info.name, info.backend),
    })
}

struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
    size: (u32, u32),
    padded_row: u32,
}

fn target(gpu: &Gpu, w: u32, h: u32) -> Target {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let padded_row = (w * 4).div_ceil(256) * 256;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (padded_row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    Target {
        texture,
        view,
        readback,
        size: (w, h),
        padded_row,
    }
}

fn draw(
    gpu: &Gpu,
    renderer: &CanvasRenderer,
    canvas: &mut novo_canvas::render::GpuCanvas,
    t: &Target,
    frame: &FrameData,
    copy: bool,
) {
    canvas.prepare(&gpu.device, &gpu.queue, frame);
    let mut enc = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("canvas"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &t.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        renderer.paint(&mut pass, canvas);
    }
    if copy {
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &t.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &t.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(t.padded_row),
                    rows_per_image: Some(t.size.1),
                },
            },
            wgpu::Extent3d {
                width: t.size.0,
                height: t.size.1,
                depth_or_array_layers: 1,
            },
        );
    }
    gpu.queue.submit([enc.finish()]);
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
}

fn read_pixels(gpu: &Gpu, t: &Target) -> Vec<[u8; 4]> {
    let slice = t.readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let data = slice.get_mapped_range().expect("mapped range");
    let mut out = Vec::with_capacity((t.size.0 * t.size.1) as usize);
    for row in 0..t.size.1 {
        let start = (row * t.padded_row) as usize;
        out.extend_from_slice(
            data[start..start + (t.size.0 * 4) as usize]
                .as_chunks::<4>()
                .0,
        );
    }
    drop(data);
    t.readback.unmap();
    out
}

fn frame(view: Viewport, scene: Option<Arc<novo_canvas::SceneMesh>>, s: &Scene) -> FrameData {
    let mut under = Mesh::default();
    background(&mut under, &view, Color::WHITE);
    let ranges = scene
        .as_ref()
        .map(|m| m.visible_ranges(s))
        .unwrap_or_default();
    FrameData {
        viewport: view,
        pixels_per_point: 1.0,
        scene,
        scene_ranges: ranges,
        highlight: None,
        under,
        over: Mesh::default(),
    }
}

#[test]
fn draws_lines_where_expected() {
    let Some(gpu) = gpu() else {
        eprintln!("no GPU adapter, skipping");
        return;
    };
    let renderer = CanvasRenderer::new(&gpu.device, TargetConfig::new(FORMAT));
    let mut canvas = renderer.create_canvas(&gpu.device);
    let t = target(&gpu, 200, 100);

    let mut scene = Scene::new();
    // A red horizontal line through the middle, 4 px wide at zoom 1.
    scene.insert(
        novo_canvas::ItemId(1),
        novo_canvas::Item::new(Default::default()).with(Primitive::stroked(
            Shape::line(Vec2::new(-80.0, 0.0), Vec2::new(80.0, 0.0)),
            Stroke::world(4.0, Color::RED),
        )),
    );
    // A blue filled square on the left, 1 pt screen outline.
    scene.insert(
        novo_canvas::ItemId(2),
        novo_canvas::Item::new(Default::default()).with(
            Primitive::stroked(
                Shape::rect(Vec2::new(-90.0, -40.0), Vec2::new(-70.0, -20.0)),
                Stroke::screen(1.0, Color::BLACK),
            )
            .with_fill(Color::BLUE),
        ),
    );
    let sm = Arc::new(Tessellator::default().scene(&scene));
    let view = Viewport {
        center: Vec2::ZERO,
        zoom: 1.0,
        size: Vec2::new(200.0, 100.0),
        ..Default::default()
    };
    draw(
        &gpu,
        &renderer,
        &mut canvas,
        &t,
        &frame(view, Some(sm.clone()), &scene),
        true,
    );
    let px = read_pixels(&gpu, &t);
    let at = |x: u32, y: u32| px[(y * 200 + x) as usize];

    // Centre of the line is solid red, well above and below it is white.
    assert_eq!(at(100, 50), [220, 40, 40, 255], "line centre");
    assert_eq!(at(100, 49), [220, 40, 40, 255], "line upper half");
    assert_eq!(at(100, 40), [255, 255, 255, 255], "background above");
    assert_eq!(at(100, 60), [255, 255, 255, 255], "background below");
    // Beyond the line's end is white.
    assert_eq!(at(195, 50), [255, 255, 255, 255], "past the end");
    // Fill of the square (world (-80, -30) is screen (20, 20)).
    assert_eq!(at(20, 20), [40, 90, 220, 255], "fill");

    // Panning moves the content without re-uploading the scene.
    let rev = canvas.scene_revision();
    let mut moved = view;
    moved.pan_by_screen(Vec2::new(0.0, 20.0));
    draw(
        &gpu,
        &renderer,
        &mut canvas,
        &t,
        &frame(moved, Some(sm), &scene),
        true,
    );
    assert_eq!(canvas.scene_revision(), rev);
    let px = read_pixels(&gpu, &t);
    let at = |x: u32, y: u32| px[(y * 200 + x) as usize];
    assert_eq!(at(100, 70), [220, 40, 40, 255], "line followed the pan");
    assert_eq!(at(100, 50), [255, 255, 255, 255]);
}

#[test]
fn hidden_layers_are_not_drawn() {
    let Some(gpu) = gpu() else {
        eprintln!("no GPU adapter, skipping");
        return;
    };
    let renderer = CanvasRenderer::new(&gpu.device, TargetConfig::new(FORMAT));
    let mut canvas = renderer.create_canvas(&gpu.device);
    let t = target(&gpu, 64, 64);
    let mut scene = Scene::new();
    let l = scene.add_layer(novo_canvas::Layer::new("Wires", 1));
    scene.insert(
        novo_canvas::ItemId(1),
        novo_canvas::Item::new(l).with(Primitive::filled(
            Shape::rect(Vec2::splat(-10.0), Vec2::splat(10.0)),
            Color::GREEN,
        )),
    );
    let sm = Arc::new(Tessellator::default().scene(&scene));
    let view = Viewport {
        zoom: 1.0,
        size: Vec2::splat(64.0),
        ..Default::default()
    };
    draw(
        &gpu,
        &renderer,
        &mut canvas,
        &t,
        &frame(view, Some(sm.clone()), &scene),
        true,
    );
    assert_eq!(read_pixels(&gpu, &t)[32 * 64 + 32], [40, 170, 70, 255]);
    scene.layer_mut(l).unwrap().visible = false;
    draw(
        &gpu,
        &renderer,
        &mut canvas,
        &t,
        &frame(view, Some(sm), &scene),
        true,
    );
    assert_eq!(read_pixels(&gpu, &t)[32 * 64 + 32], [255, 255, 255, 255]);
}

/// Renders the 10,000-object stress page at 1920x1080 while panning and
/// zooming, and reports the GPU frame time. On a real GPU this is far below
/// the 16.7 ms budget; on a software rasteriser it is only informative.
#[test]
fn stress_page_frame_time() {
    let Some(gpu) = gpu() else {
        eprintln!("no GPU adapter, skipping");
        return;
    };
    let scene = novo_canvas::demo::stress_scene(10_000);
    let t0 = Instant::now();
    let sm = Arc::new(Tessellator::default().scene(&scene));
    let tess = t0.elapsed();
    let renderer = CanvasRenderer::new(&gpu.device, TargetConfig::new(FORMAT));
    let mut canvas = renderer.create_canvas(&gpu.device);
    let t = target(&gpu, 1920, 1080);
    let mut view = Viewport::new(Vec2::new(1920.0, 1080.0));
    view.fit(&scene.bounds(), 10.0);
    // Warm up and upload.
    draw(
        &gpu,
        &renderer,
        &mut canvas,
        &t,
        &frame(view, Some(sm.clone()), &scene),
        false,
    );
    let frames = 30;
    let start = Instant::now();
    for i in 0..frames {
        view.pan_by_screen(Vec2::new(3.0, 1.0));
        view.zoom_at(
            Vec2::new(960.0, 540.0),
            if i % 2 == 0 { 1.05 } else { 1.0 / 1.05 },
        );
        draw(
            &gpu,
            &renderer,
            &mut canvas,
            &t,
            &frame(view, Some(sm.clone()), &scene),
            false,
        );
    }
    let per = start.elapsed().as_secs_f64() * 1000.0 / frames as f64;
    eprintln!(
        "adapter {}: {} items, {} vertices, {} triangles; tessellate {:.1} ms; {:.2} ms per frame",
        gpu.adapter,
        scene.len(),
        sm.mesh.vertices.len(),
        sm.mesh.indices.len() / 3,
        tess.as_secs_f64() * 1000.0,
        per
    );
    // Pixels were actually drawn: the page is not blank.
    draw(
        &gpu,
        &renderer,
        &mut canvas,
        &t,
        &frame(view, Some(sm), &scene),
        true,
    );
    let px = read_pixels(&gpu, &t);
    let inked = px.iter().filter(|p| p[0] < 200).count();
    assert!(inked > 10_000, "only {inked} inked pixels");
}
