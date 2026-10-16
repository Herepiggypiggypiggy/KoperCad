//! Headless orbit benchmark and screenshot.
//!
//!   cargo run --release -p novo-view3d --features gpu --example orbit -- \
//!       [--items 1000] [--wires 300] [--frames 300] [--size 1920x1080] [--msaa] [--scale 0.75] \
//!       [--png out.png] [--tier discrete|integrated|software] [--shadows] [--no-edges] \
//!       [--heat N]
//!
//! Renders a scripted 360 degree orbit of the demo cabinet into an offscreen
//! target and prints CPU time per frame (cull, instances, encode) and wall time
//! per frame (submit and wait for the GPU). With `--png` the last frame is
//! written as an image, rendered settled (4x MSAA, full scale).

use flate2::{Compression, write::ZlibEncoder};
use novo_view3d::demo::{self, DemoOptions};
use novo_view3d::render::{Frame, Mode, Quality, Renderer, Tier, ViewTarget, read_rgba};
use novo_view3d::{Camera, Look, Overlay, OverlayLines, OverlayMesh, ViewPreset, ViewState};
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

/// A stand-in for the heat layer: a translucent slice plane through the
/// cabinet of `n` x `n` quads coloured by position, and `n` tracer lines of 40
/// points each.
fn heat_overlays(b: &novo_view3d::Aabb, n: usize) -> Vec<Overlay> {
    use glam::Vec3;
    let (min, max) = (b.min, b.max);
    let z = (min.z + max.z) * 0.5 + 20.0;
    let mut mesh = OverlayMesh {
        translucent: true,
        ..Default::default()
    };
    for j in 0..=n {
        for i in 0..=n {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            mesh.positions.push(Vec3::new(
                min.x + u * (max.x - min.x),
                min.y + v * (max.y - min.y),
                z,
            ));
            mesh.colors
                .push([(u * 255.0) as u8, 80, ((1.0 - v) * 255.0) as u8, 140]);
        }
    }
    for j in 0..n {
        for i in 0..n {
            let a = (j * (n + 1) + i) as u32;
            let (b2, c, d) = (a + 1, a + n as u32 + 1, a + n as u32 + 2);
            mesh.indices.extend([a, b2, d, a, d, c]);
        }
    }
    let mut lines = OverlayLines {
        width_px: 2.0,
        order: 1,
        ..Default::default()
    };
    for k in 0..n {
        lines.starts.push(lines.positions.len() as u32);
        let y = min.y + (k as f32 + 0.5) / n as f32 * (max.y - min.y);
        for p in 0..40 {
            let t = p as f32 / 39.0;
            lines.positions.push(Vec3::new(
                min.x + t * (max.x - min.x),
                y + (t * 12.0).sin() * 8.0,
                z + 2.0,
            ));
            lines.colors.push([255, (t * 255.0) as u8, 40, 255]);
        }
    }
    vec![Overlay::Mesh(mesh), Overlay::Lines(lines)]
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
    let mut forced_tier: Option<Tier> = None;
    let mut shadows = false;
    let mut edges = true;
    let mut heat = 0usize;
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
            "--tier" => {
                forced_tier = Some(match args.next().unwrap().as_str() {
                    "discrete" => Tier::Discrete,
                    "integrated" => Tier::Integrated,
                    "software" => Tier::Software,
                    other => panic!("unknown tier {other}"),
                })
            }
            "--shadows" => shadows = true,
            "--no-edges" => edges = false,
            "--heat" => heat = args.next().unwrap().parse().unwrap(),
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
    let tier = forced_tier.unwrap_or_else(|| Tier::of(&info));
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
    let mut state = ViewState::default();
    state.display.edges = edges;
    state.display.shadows = shadows;
    let overlays = if heat > 0 {
        heat_overlays(&scene.bounds(), heat)
    } else {
        Vec::new()
    };
    let mut renderer = Renderer::new(&device);
    let mut target = ViewTarget::new(&renderer, &device);
    let mut quality = Quality::new(Mode::Interactive, tier);
    if msaa {
        quality.msaa = 4;
    }
    if let Some(s) = scale {
        quality.scale = s;
    }
    quality.shadows = shadows;
    println!(
        "quality: msaa {} scale {} shadows {} edges {} heat overlay {} ({} overlays)",
        quality.msaa,
        quality.scale,
        quality.shadows,
        edges,
        heat,
        overlays.len()
    );

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
                overlays: &overlays,
                overlay_revision: 1,
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
