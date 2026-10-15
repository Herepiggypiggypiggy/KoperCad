//! Phase 0 gate demo: a stress page with 10,000 objects.
//!
//! ```text
//! cargo run --release -p novo-canvas --example stress            # 10,000 objects
//! cargo run --release -p novo-canvas --example stress -- 50000   # or any count
//! ```
//!
//! The top bar shows frames per second while "Animate" keeps the view
//! moving, so the number measures the canvas rather than an idle window.

use eframe::egui;
use novo_canvas::widget::CanvasView;
use novo_canvas::{GridStyle, Scene, Vec2};
use std::collections::VecDeque;

fn main() -> eframe::Result {
    let count = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(10_000);
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KoperCAD canvas stress test")
            .with_inner_size([1600.0, 900.0]),
        ..Default::default()
    };
    // Present without vsync so the counter shows real headroom above 60 fps.
    options.wgpu_options.surface.present_mode = wgpu::PresentMode::AutoNoVsync;
    eframe::run_native(
        "novo-canvas-stress",
        options,
        Box::new(move |cc| {
            let rs = cc
                .wgpu_render_state
                .as_ref()
                .expect("eframe must use the wgpu renderer");
            novo_canvas::widget::install(rs, 1);
            Ok(Box::new(Stress::new(count)))
        }),
    )
}

struct Stress {
    scene: Scene,
    view: CanvasView,
    count: usize,
    animate: bool,
    t: f64,
    frame_times: VecDeque<f64>,
}

impl Stress {
    fn new(count: usize) -> Self {
        Self {
            scene: novo_canvas::demo::stress_scene(count),
            view: CanvasView::new(),
            count,
            animate: true,
            t: 0.0,
            frame_times: VecDeque::new(),
        }
    }
}

impl eframe::App for Stress {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let (now, dt) = ui.ctx().input(|i| (i.time, i.unstable_dt as f64));
        self.frame_times.push_back(now);
        while self.frame_times.front().is_some_and(|t| now - t > 1.0) {
            self.frame_times.pop_front();
        }
        let fps = self.frame_times.len();

        egui::Panel::top("bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("{fps} fps"));
                ui.separator();
                let s = self.view.stats();
                ui.label(format!(
                    "{} items, {} triangles, tessellated in {:.1} ms",
                    s.items,
                    s.scene_triangles,
                    s.tessellate_ms.unwrap_or(0.0)
                ));
                ui.separator();
                ui.checkbox(&mut self.animate, "Animate");
                let mut dots = self.view.grid.style == GridStyle::Dots;
                if ui.checkbox(&mut dots, "Dot grid").changed() {
                    self.view.grid.style = if dots {
                        GridStyle::Dots
                    } else {
                        GridStyle::Lines
                    };
                }
                ui.checkbox(&mut self.view.grid.visible, "Grid");
                if ui.button("Fit").clicked() {
                    self.view.zoom_to_fit();
                }
                ui.separator();
                for layer in 1..self.scene.layers().len() {
                    let id = novo_canvas::LayerId(layer as u16);
                    let mut visible = self.scene.layer(id).is_some_and(|l| l.visible);
                    let name = self
                        .scene
                        .layer(id)
                        .map(|l| l.name.clone())
                        .unwrap_or_default();
                    if ui.checkbox(&mut visible, name).changed()
                        && let Some(l) = self.scene.layer_mut(id)
                    {
                        l.visible = visible;
                    }
                }
                ui.separator();
                ui.label(format!("selected {}", self.view.selection.len()));
                if ui.button(format!("Rebuild {}", self.count)).clicked() {
                    self.scene = novo_canvas::demo::stress_scene(self.count);
                }
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("zoom {:.2} pt/mm", self.view.viewport.zoom));
                if let Some(h) = self.view.hovered() {
                    ui.label(format!("hover {:?}", h));
                }
            });
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                if self.animate {
                    self.t += dt;
                    let v = &mut self.view.viewport;
                    let wobble = (self.t * 0.7).sin() as f32;
                    v.pan_by_screen(Vec2::new(wobble * 4.0, (self.t * 0.5).cos() as f32 * 2.0));
                    let anchor = v.size * 0.5;
                    v.zoom_at(anchor, 1.0 + 0.01 * (self.t * 0.9).sin() as f32);
                }
                let out = self.view.show(ui, &self.scene);
                if let Some(c) = out.cursor {
                    ui.ctx().debug_painter().text(
                        out.response.rect.left_bottom() + egui::vec2(8.0, -8.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("x {:.1}  y {:.1}  {:?}", c.point.x, c.point.y, c.kind),
                        egui::FontId::monospace(12.0),
                        egui::Color32::DARK_GRAY,
                    );
                }
            });
        if self.animate {
            ui.ctx().request_repaint();
        }
    }
}
