//! Headless orbit benchmark and screenshot.
//!
//!   cargo run --release -p novo-view3d --features gpu --example orbit -- \
//!       [--items 1000] [--wires 300] [--frames 300] [--size 1920x1080] [--msaa] [--scale 0.75] \
//!       [--png out.png]
//!
//! Renders a scripted 360 degree orbit of the demo cabinet into an offscreen
//! target and prints CPU time per frame (cull, instances, encode) and wall time
//! per frame (submit and wait for the GPU). With `--png` the last frame is
//! written as an image, rendered settled (4x MSAA, full scale).

use flate2::{Compression, write::ZlibEncoder};
use novo_view3d::demo::{self, DemoOptions};
use novo_view3d::render::{Frame, Mode, Quality, Renderer, Tier, ViewTarget, read_rgba};
use novo_view3d::{Camera, Look, ViewPreset, ViewState};
use std::io::Write;
use std::time::Instant;

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn write_png(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let mut raw = Vec::with_capacity((w * h * 4 + h) as usize);
    for row in rgba.chunks((w * 4) as usize) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(&raw).unwrap();
    let idat = z.finish().unwrap();
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &idat);
    chunk(b"IEND", &[]);
    std::fs::write(path, out).unwrap();
}

fn percentile(sorted: &[f32], p: f32) -> f32 {
    sorted[((sorted.len() - 1) as f32 * p).round() as usize]
}

fn main() {
    let mut items = 1000usize;
    let mut frames = 300usize;
    let mut size = (1920u32, 1080u32);
    let mut wires = 0usize;
    let mut msaa = false;
    let mut scale: Option<f32> = None;
    let mut png: Option<String> = None;
    let mut view: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--items" => items = args.next().unwrap().parse().unwrap(),
            "--wires" => wires = args.next().unwrap().parse().unwrap(),
            "--frames" => frames = args.next().unwrap().parse().unwrap(),
            "--size" => {
                let s = args.next().unwrap();
                let (w, h) = s.split_once('x').unwrap();
                size = (w.parse().unwrap(), h.parse().unwrap());
            }
            "--msaa" => msaa = true,
            "--scale" => scale = Some(args.next().unwrap().parse().unwrap()),
            "--png" => png = Some(args.next().unwrap()),
            "--view" => view = Some(args.next().unwrap()),
            other => panic!("unknown argument {other}"),
        }
    }

    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("no wgpu adapter");
    let info = adapter.get_info();
    let tier = Tier::of(&info);
    println!(
        "adapter: {} ({:?}, {:?}), tier {:?}",
        info.name, info.backend, info.device_type, tier
    );
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

    let (scene, library) = demo::cabinet(&DemoOptions {
        devices: items,
        wires,
        ..DemoOptions::default()
    });
    println!(
        "scene: {} items ({} devices requested), {} wires, {}x{}",
        scene.items.len(),
        items,
        scene.wires.len(),
        size.0,
        size.1
    );
    let look = Look::dark();
    let state = ViewState::default();
    let mut renderer = Renderer::new(&device);
    let mut target = ViewTarget::new(&renderer, &device);
    let mut quality = Quality::new(Mode::Interactive, tier);
    if msaa {
        quality.msaa = 4;
    }
    if let Some(s) = scale {
        quality.scale = s;
    }
    println!("quality: msaa {} scale {}", quality.msaa, quality.scale);

    let aspect = size.0 as f32 / size.1 as f32;
    let mut camera = Camera::default();
    camera.fit(&scene.bounds(), aspect);

    let mut cpu = Vec::new();
    let mut wall = Vec::new();
    let mut last = Default::default();
    for f in 0..frames {
        camera.yaw = f as f32 / frames as f32 * std::f32::consts::TAU;
        let started = Instant::now();
        let mut encoder = device.create_command_encoder(&Default::default());
        let stats = renderer.render(
            &device,
            &queue,
            &mut encoder,
            &mut target,
            &Frame {
                scene: &scene,
                state: &state,
                camera: &camera,
                library: &library,
                look: &look,
                size_px: size,
                quality,
                overlays: &[],
                overlay_revision: 0,
            },
        );
        queue.submit([encoder.finish()]);
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        wall.push(started.elapsed().as_secs_f32() * 1000.0);
        cpu.push(stats.cpu_ms);
        last = stats;
    }
    println!(
        "first frame wall ms: {:.1} (pipelines and buffers are made in it)",
        wall[0]
    );
    cpu.sort_by(|a, b| a.total_cmp(b));
    wall.sort_by(|a, b| a.total_cmp(b));
    println!(
        "last frame: {} visible items, {} draw calls, {} triangles, rendered at {}x{}",
        last.visible_items, last.draw_calls, last.triangles, last.rendered_px.0, last.rendered_px.1
    );
    println!(
        "cpu  ms/frame: p50 {:.3}  p95 {:.3}  max {:.3}",
        percentile(&cpu, 0.5),
        percentile(&cpu, 0.95),
        percentile(&cpu, 1.0)
    );
    println!(
        "wall ms/frame: p50 {:.3}  p95 {:.3}  max {:.3}  (= {:.0} fps at p50)",
        percentile(&wall, 0.5),
        percentile(&wall, 0.95),
        percentile(&wall, 1.0),
        1000.0 / percentile(&wall, 0.5)
    );

    if let Some(path) = png {
        match view.as_deref() {
            Some("front") => camera.set_preset(ViewPreset::Front),
            Some("top") => camera.set_preset(ViewPreset::Top),
            Some("left") => camera.set_preset(ViewPreset::Left),
            _ => camera.set_preset(ViewPreset::Iso),
        }
        camera.fit(&scene.bounds(), aspect);
        let mut state = ViewState::default();
        if let Some(d) = scene
            .items
            .iter()
            .filter(|i| i.id.kind == novo_view3d::ItemKind::Device)
            .nth(11)
        {
            state.selection.insert(d.id);
        }
        let mut encoder = device.create_command_encoder(&Default::default());
        renderer.render(
            &device,
            &queue,
            &mut encoder,
            &mut target,
            &Frame {
                scene: &scene,
                state: &state,
                camera: &camera,
                library: &library,
                look: &look,
                size_px: size,
                quality: Quality::new(Mode::Settled, Tier::Discrete),
                overlays: &[],
                overlay_revision: 0,
            },
        );
        queue.submit([encoder.finish()]);
        let (w, h) = target.size();
        write_png(&path, w, h, &read_rgba(&device, &queue, &target));
        println!("wrote {path}");
    }
}
