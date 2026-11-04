//! Headless picture of the generic crane (what Simulate > Hoist in 3D draws), for checking its
//! shapes without the app.
//!
//!   cargo run --release -p novo-view3d --features gpu --example crane_view -- \
//!       --png out.png [--view iso|front|side|top] [--focus all|trolley|truck|hook] \
//!       [--size 1920x1080] [--bridge 4.0] [--trolley 5.0] [--hook 0.0] [--swing 0.0]

use flate2::{Compression, write::ZlibEncoder};
use glam::Vec3;
use novo_view3d::crane::{CraneLayout, CraneState, build, hook_bounds};
use novo_view3d::crane_parts;
use novo_view3d::render::{Frame, Mode, Quality, Renderer, Tier, ViewTarget, read_rgba};
use novo_view3d::{Aabb, Camera, Look, MeshLibrary, ViewPreset, ViewState};
use std::io::Write;

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

fn main() {
    let mut size = (1920u32, 1080u32);
    let mut png = String::from("crane.png");
    let mut view = String::from("iso");
    let mut focus = String::from("all");
    let (mut bridge, mut trolley, mut hook, mut swing) = (4.0f32, 5.0f32, 0.0f32, 0.0f32);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--png" => png = args.next().unwrap(),
            "--view" => view = args.next().unwrap(),
            "--focus" => focus = args.next().unwrap(),
            "--bridge" => bridge = args.next().unwrap().parse().unwrap(),
            "--trolley" => trolley = args.next().unwrap().parse().unwrap(),
            "--hook" => hook = args.next().unwrap().parse().unwrap(),
            "--swing" => swing = args.next().unwrap().parse().unwrap(),
            "--size" => {
                let s = args.next().unwrap();
                let (w, h) = s.split_once('x').unwrap();
                size = (w.parse().unwrap(), h.parse().unwrap());
            }
            other => panic!("unknown argument {other}"),
        }
    }
    let layout = CraneLayout::default();
    let state = CraneState {
        bridge_m: bridge,
        trolley_m: trolley,
        hook_m: hook,
        swing_rad: [swing, 0.0],
        ..Default::default()
    };
    let scene = build(&layout, &state);
    let mut library = MeshLibrary::new();
    crane_parts::ensure(&mut library, &scene);

    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("no wgpu adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let mut renderer = Renderer::new(&device);
    let mut target = ViewTarget::new(&renderer, &device);
    let look = Look::dark();
    let mut vs = ViewState::default();
    vs.display.edges = true;
    vs.display.shadows = true;
    vs.display.finish = true;

    let bounds: Aabb = match focus.as_str() {
        "hook" => hook_bounds(&layout, &state),
        "trolley" => {
            let c = Vec3::new(
                bridge * 1000.0 + 300.0,
                layout.rail_m * 1000.0 + 1400.0,
                trolley * 1000.0,
            );
            Aabb::new(
                c - Vec3::new(1500.0, 1700.0, 1500.0),
                c + Vec3::new(1500.0, 900.0, 1500.0),
            )
        }
        "truck" => {
            let c = Vec3::new(bridge * 1000.0, layout.rail_m * 1000.0 + 300.0, 0.0);
            Aabb::new(
                c - Vec3::new(1700.0, 500.0, 900.0),
                c + Vec3::new(1700.0, 800.0, 900.0),
            )
        }
        _ => scene.bounds(),
    };
    let aspect = size.0 as f32 / size.1 as f32;
    let mut camera = Camera::default();
    camera.set_preset(match view.as_str() {
        "front" => ViewPreset::Front,
        "side" => ViewPreset::Right,
        "top" => ViewPreset::Top,
        _ => ViewPreset::Iso,
    });
    camera.fit(&bounds, aspect);
    let mut encoder = device.create_command_encoder(&Default::default());
    renderer.render(
        &device,
        &queue,
        &mut encoder,
        &mut target,
        &Frame {
            scene: &scene,
            state: &vs,
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
    write_png(&png, w, h, &read_rgba(&device, &queue, &target));
    println!("wrote {png}");
}
