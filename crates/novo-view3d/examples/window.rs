//! The 3D view in an egui window: orbit with the right mouse button (or Alt +
//! left), pan with the middle button (or Space + left), zoom with the wheel,
//! click to select, drag for a marquee, F or Home to fit.
//!
//!   cargo run --release -p novo-view3d --features egui --example window -- [--items 300]
//!   cargo run --release -p novo-view3d --features egui --example window -- --bench 10
//!
//! `--bench SECONDS` orbits the cabinet without vsync, then prints one line
//! (frames per second, frame time percentiles, adapter) and exits.

use eframe::egui;
use novo_view3d::demo::{self, DemoOptions};
use novo_view3d::widget::{self, View3d, ViewOptions};
use novo_view3d::{Look, MeshLibrary, Projection, Scene, ViewPreset};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct App {
    view: View3d,
    scene: Arc<Scene>,
    library: Arc<MeshLibrary>,
    look: Look,
    items: usize,
    door_angle: f32,
    auto_orbit: bool,
    last_click: String,
    gpu: String,
    bench: Option<Bench>,
}

struct Bench {
    seconds: f64,
    start: Option<Instant>,
    last: Option<Instant>,
    frames: Vec<f64>,
}

impl App {
    fn rebuild(&mut self) {
        let (scene, library) = demo::cabinet(&DemoOptions {
            devices: self.items,
            ..DemoOptions::default()
        });
        self.scene = Arc::new(scene);
        self.library = library;
        self.view.request_fit();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = ui.input(|i| i.time);
        egui::Panel::top("controls").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let r = ui.add(egui::Slider::new(&mut self.items, 10..=3000).text("devices"));
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    self.rebuild();
                }
                for (name, preset) in [
                    ("Front", ViewPreset::Front),
                    ("Left", ViewPreset::Left),
                    ("Right", ViewPreset::Right),
                    ("Top", ViewPreset::Top),
                    ("Iso", ViewPreset::Iso),
                ] {
                    if ui.button(name).clicked() {
                        self.view.go_to(preset, &self.scene, now, false);
                    }
                }
                if ui.button("Fit").clicked() {
                    self.view.fit(&self.scene, now, false);
                }
                let mut ortho = self.view.camera.projection == Projection::Orthographic;
                if ui.checkbox(&mut ortho, "Orthographic").changed() {
                    self.view.camera.projection = if ortho {
                        Projection::Orthographic
                    } else {
                        Projection::Perspective
                    };
                }
                ui.checkbox(&mut self.auto_orbit, "Auto orbit");
                if ui
                    .add(egui::Slider::new(&mut self.door_angle, 0.0..=1.9).text("door"))
                    .changed()
                {
                    let mut s = (*self.scene).clone();
                    s.set_group_angle(demo::HINGE, -self.door_angle);
                    self.scene = Arc::new(s);
                }
                ui.checkbox(&mut self.view.state.display.edges, "Edges");
                ui.checkbox(&mut self.view.state.display.labels, "Labels");
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let dt = ui.input(|i| i.stable_dt) * 1000.0;
                ui.label(format!(
                    "{} items, {} selected | {:.1} ms/frame | {} | {}",
                    self.scene.items.len(),
                    self.view.state.selection.len(),
                    dt,
                    self.gpu,
                    self.last_click
                ));
            });
        });

        if self.auto_orbit || self.bench.is_some() {
            self.view.camera.orbit(0.012, 0.0);
            ui.ctx().request_repaint();
        }
        let out = egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.view.show(
                    ui,
                    &self.scene,
                    &self.library,
                    &self.look,
                    ViewOptions::default(),
                )
            })
            .inner;
        if let Some(click) = out.clicked {
            self.last_click = format!("clicked {:?}", click.id.map(|i| (i.kind, i.raw)));
            if !click.modifiers.shift {
                self.view.state.selection.clear();
            }
            if let Some(id) = click.id {
                self.view.state.selection.insert(id);
            }
        }
        if let Some(ids) = out.marquee {
            self.view.state.selection = ids.into_iter().collect();
        }

        if let Some(b) = &mut self.bench {
            let t = Instant::now();
            let start = *b.start.get_or_insert(t);
            if let Some(last) = b.last
                && t - start > Duration::from_secs(1)
            {
                b.frames.push((t - last).as_secs_f64() * 1000.0);
            }
            b.last = Some(t);
            if (t - start).as_secs_f64() > b.seconds + 1.0 {
                let mut f = b.frames.clone();
                f.sort_by(|a, b| a.total_cmp(b));
                let avg = f.iter().sum::<f64>() / f.len().max(1) as f64;
                let p = |q: f64| f[((f.len() - 1) as f64 * q) as usize];
                let line = format!(
                    "fps avg={:.1} frame ms p50={:.2} p95={:.2} p99={:.2} frames={} items={} gpu={}",
                    1000.0 / avg,
                    p(0.5),
                    p(0.95),
                    p(0.99),
                    f.len(),
                    self.scene.items.len(),
                    self.gpu
                );
                println!("{line}");
                std::process::exit(0);
            }
        }
    }
}

fn main() {
    let mut items = 300usize;
    let mut bench: Option<f64> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--items" => items = args.next().and_then(|v| v.parse().ok()).expect("--items N"),
            "--bench" => bench = Some(args.next().and_then(|v| v.parse().ok()).expect("--bench S")),
            other => panic!("unknown argument {other}"),
        }
    }
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KoperCAD 3D view spike")
            .with_inner_size([1400.0, 860.0]),
        ..Default::default()
    };
    if bench.is_some() {
        options.wgpu_options.surface.present_mode = wgpu::PresentMode::AutoNoVsync;
    }
    eframe::run_native(
        "novo-view3d-window",
        options,
        Box::new(move |cc| {
            let rs = cc
                .wgpu_render_state
                .as_ref()
                .ok_or("eframe started without the wgpu renderer")?;
            widget::install(rs, 1);
            let info = rs.adapter.get_info();
            let (scene, library) = demo::cabinet(&DemoOptions {
                devices: items,
                ..DemoOptions::default()
            });
            Ok(Box::new(App {
                view: View3d::new(1),
                scene: Arc::new(scene),
                library,
                look: Look::dark(),
                items,
                door_angle: 0.0,
                auto_orbit: false,
                last_click: String::new(),
                gpu: format!("{} ({:?})", info.name, info.backend),
                bench: bench.map(|seconds| Bench {
                    seconds,
                    start: None,
                    last: None,
                    frames: Vec::new(),
                }),
            }))
        }),
    )
    .expect("window");
}
