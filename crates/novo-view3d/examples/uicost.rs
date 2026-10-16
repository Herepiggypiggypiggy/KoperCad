//! What the widget itself costs per frame, without a GPU: input, culling,
//! label layout and painting into egui, with the demo cabinet turning.
//!
//!   cargo run --release -p novo-view3d --features egui --example uicost -- \
//!       [--items 1000] [--wires 300] [--frames 300] [--size 1366x768]
//!
//! Prints milliseconds per frame (median and 95th percentile) for the widget
//! as it is and with labels, edges and the selection switched off, so the part
//! that is worth cutting shows.

use egui::{Pos2, RawInput, Rect, vec2};
use novo_view3d::demo::{self, DemoOptions};
use novo_view3d::widget::{View3d, ViewOptions, install_headless, percentile};
use novo_view3d::{ItemKind, Look};
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let mut items = 1000usize;
    let mut wires = 300usize;
    let mut frames = 300usize;
    let mut size = (1366.0f32, 768.0f32);
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
            other => panic!("unknown argument {other}"),
        }
    }
    install_headless();
    let (scene, library) = demo::cabinet(&DemoOptions {
        devices: items,
        wires,
        ..DemoOptions::default()
    });
    let scene = Arc::new(scene);
    let library = Arc::new(library);
    println!(
        "scene: {} items, {} wires, {}x{} points",
        scene.items.len(),
        scene.wires.len(),
        size.0,
        size.1
    );
    let look = Look::dark();
    let selected = scene
        .items
        .iter()
        .filter(|i| i.id.kind == ItemKind::Device)
        .nth(11)
        .map(|d| d.id);

    for (name, labels, edges, select) in [
        ("as it is", true, true, true),
        ("no labels", false, true, true),
        ("no edges", true, false, true),
        ("no selection", true, true, false),
        ("nothing extra", false, false, false),
    ] {
        let ctx = egui::Context::default();
        let mut view = View3d::new(1);
        view.state.display.labels = labels;
        view.state.display.edges = edges;
        if select {
            view.state.selection.extend(selected);
        }
        let mut times = Vec::new();
        for f in 0..frames + 20 {
            let input = RawInput {
                time: Some(f as f64 / 60.0),
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(size.0, size.1))),
                ..Default::default()
            };
            let started = Instant::now();
            let mut out = ctx.run_ui(input, |ui| {
                view.spin(ui.ctx(), 0.5);
                view.show(ui, &scene, &library, &look, ViewOptions::default());
            });
            let ms = started.elapsed().as_secs_f32() * 1000.0;
            out.textures_delta.clear();
            if f >= 20 {
                times.push(ms);
            }
        }
        println!(
            "{name:>14}: {:.2} ms median, {:.2} ms 95th percentile",
            percentile(&times, 0.5),
            percentile(&times, 0.95)
        );
    }
}
