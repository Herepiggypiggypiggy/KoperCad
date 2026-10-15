//! A model page on the canvas: a direct-on-line motor starter built through
//! the novo-core API and drawn by `model::PageScene`.
//!
//! ```text
//! cargo run -p novo-canvas --example demo
//! cargo run --release -p novo-canvas --example demo -- --stress 10000 --seconds 10
//! cargo run -p novo-canvas --example demo -- --panel
//! ```
//!
//! `--panel` shows a mounting plate drawn with the drafting primitives
//! (hatched ducts, rails with a snap pitch, dimensions) instead.
//!
//! Click or box-select objects; the status bar shows the model ids behind
//! the selection.
//!
//! With `--stress N` the window instead shows a stress page of N objects,
//! pans and zooms it without vsync for `--seconds` (default 10), then prints
//! one result line and exits:
//!
//! ```text
//! fps avg=143.2 p1=97.5 min=61.0 frames=1432 items=10001 size=1600x900 gpu=NVIDIA GeForce RTX 3060 (Vulkan)
//! ```
//!
//! The same line is written to `stress-result.txt` next to the executable,
//! since a windowed Windows build has no console.

use eframe::egui;
use novo_canvas::model::{self, PageScene, PlaceholderArt};
use novo_canvas::widget::CanvasView;
use novo_canvas::{Scene, Vec2};
use std::time::{Duration, Instant};

/// Frames at the start of a stress run that are not counted (shader
/// compilation, first upload, window mapping).
const WARMUP: Duration = Duration::from_secs(1);
const WINDOW: [f32; 2] = [1600.0, 900.0];

struct Args {
    stress: Option<usize>,
    panel: bool,
    seconds: f64,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        stress: None,
        panel: false,
        seconds: 10.0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--stress" => {
                let v = value("--stress")?;
                args.stress = Some(v.parse().map_err(|_| format!("bad object count {v:?}"))?);
            }
            "--panel" => args.panel = true,
            "--seconds" => {
                let v = value("--seconds")?;
                args.seconds = v
                    .parse()
                    .ok()
                    .filter(|s: &f64| *s > 0.0)
                    .ok_or(format!("bad duration {v:?}"))?;
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(args)
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\nusage: demo [--panel] [--stress OBJECTS] [--seconds SECONDS]");
            std::process::exit(2);
        }
    };
    let stress = args.stress.is_some();
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NovoElectric canvas demo")
            .with_inner_size(WINDOW)
            .with_resizable(!stress),
        ..Default::default()
    };
    if stress {
        // Without vsync the result shows real headroom above 60 fps.
        options.wgpu_options.surface.present_mode = wgpu::PresentMode::AutoNoVsync;
    }
    let result = eframe::run_native(
        "novo-canvas-demo",
        options,
        Box::new(move |cc| {
            let rs = cc
                .wgpu_render_state
                .as_ref()
                .ok_or("eframe started without the wgpu renderer")?;
            novo_canvas::widget::install(rs, 1);
            let info = rs.adapter.get_info();
            let gpu = format!("{} ({:?})", info.name, info.backend);
            Ok(match args.stress {
                Some(n) => Box::new(Stress::new(n, args.seconds, gpu)) as Box<dyn eframe::App>,
                None if args.panel => Box::new(Panel {
                    scene: novo_canvas::demo::sample_panel(),
                    view: CanvasView::new(),
                    snapped: None,
                }),
                None => Box::new(Demo::new()),
            })
        }),
    );
    if let Err(e) = result {
        let line = format!("error={e}");
        if stress {
            report(&line);
        } else {
            eprintln!("{line}");
        }
        std::process::exit(1);
    }
}

/// Prints the result line and writes it to `stress-result.txt` next to the
/// executable (or the working directory if that is not writable).
fn report(line: &str) {
    println!("{line}");
    let next_to_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("stress-result.txt")));
    let written = next_to_exe
        .as_ref()
        .is_some_and(|p| std::fs::write(p, format!("{line}\n")).is_ok());
    if !written && let Err(e) = std::fs::write("stress-result.txt", format!("{line}\n")) {
        eprintln!("could not write stress-result.txt: {e}");
    }
}

struct Demo {
    _doc: novo_core::Document,
    page: PageScene,
    view: CanvasView,
}

impl Demo {
    fn new() -> Self {
        let (doc, page) = model::demo_document().expect("demo project builds");
        let page = PageScene::new(doc.project(), page, &PlaceholderArt);
        Self {
            _doc: doc,
            page,
            view: CanvasView::new(),
        }
    }
}

impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("status").show(ui, |ui| {
            let ids: Vec<String> = self
                .view
                .selection
                .ids()
                .iter()
                .filter_map(|id| self.page.model_id(*id))
                .map(|id| format!("{id:?}"))
                .collect();
            ui.label(format!("selected: {}", ids.join(", ")));
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.view.show(ui, self.page.scene());
            });
    }
}

/// The `--panel` drawing; the status bar shows what the cursor snapped to.
struct Panel {
    scene: Scene,
    view: CanvasView,
    snapped: Option<novo_canvas::Snapped>,
}

impl eframe::App for Panel {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(match self.snapped {
                Some(s) => format!("{:?} at {:.1}, {:.1} mm", s.kind, s.point.x, s.point.y),
                None => String::new(),
            });
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.snapped = self.view.show(ui, &self.scene).cursor;
            });
    }
}

/// Timed stress run: animates the view and records every frame interval.
struct Stress {
    scene: Scene,
    view: CanvasView,
    gpu: String,
    duration: Duration,
    start: Option<Instant>,
    last: Option<Instant>,
    frames: Vec<f64>,
    done: bool,
}

impl Stress {
    fn new(objects: usize, seconds: f64, gpu: String) -> Self {
        Self {
            scene: novo_canvas::demo::stress_scene(objects),
            view: CanvasView::new(),
            gpu,
            duration: Duration::from_secs_f64(seconds),
            start: None,
            last: None,
            frames: Vec::new(),
            done: false,
        }
    }

    fn result(&self, size: egui::Vec2) -> String {
        let mut ms = self.frames.clone();
        ms.sort_by(f64::total_cmp);
        let fps = |ms: f64| if ms > 0.0 { 1000.0 / ms } else { 0.0 };
        let total: f64 = ms.iter().sum();
        let avg = if ms.is_empty() {
            0.0
        } else {
            ms.len() as f64 * 1000.0 / total
        };
        // 1 % low: the frame time 99 % of frames beat.
        let p99 = ms
            .get(((ms.len() as f64 * 0.99) as usize).min(ms.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0.0);
        let worst = ms.last().copied().unwrap_or(0.0);
        format!(
            "fps avg={avg:.1} p1={:.1} min={:.1} frames={} items={} size={}x{} gpu={}",
            fps(p99),
            fps(worst),
            ms.len(),
            self.scene.len(),
            size.x.round(),
            size.y.round(),
            self.gpu,
        )
    }
}

impl eframe::App for Stress {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let start = *self.start.get_or_insert(now);
        if let Some(last) = self.last
            && now - start > WARMUP
        {
            self.frames.push((now - last).as_secs_f64() * 1000.0);
        }
        self.last = Some(now);

        let t = (now - start).as_secs_f64();
        let mut size = egui::Vec2::ZERO;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                size = ui.available_size() * ui.ctx().pixels_per_point();
                // Steady pan and zoom over the whole page, as a user would.
                let v = &mut self.view.viewport;
                v.pan_by_screen(Vec2::new(
                    (t * 0.7).sin() as f32 * 4.0,
                    (t * 0.5).cos() as f32 * 2.0,
                ));
                let anchor = v.size * 0.5;
                v.zoom_at(anchor, 1.0 + 0.01 * (t * 0.9).sin() as f32);
                self.view.show(ui, &self.scene);
                ui.painter().text(
                    ui.max_rect().left_top() + egui::vec2(8.0, 8.0),
                    egui::Align2::LEFT_TOP,
                    format!(
                        "stress run: {:.0} of {:.0} s",
                        t.min(self.duration.as_secs_f64()),
                        self.duration.as_secs_f64()
                    ),
                    egui::FontId::monospace(14.0),
                    egui::Color32::DARK_GRAY,
                );
            });

        if !self.done && now - start >= WARMUP + self.duration {
            self.done = true;
            report(&self.result(size));
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ui.ctx().request_repaint();
    }
}
