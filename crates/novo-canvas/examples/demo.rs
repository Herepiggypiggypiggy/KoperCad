//! A model page on the canvas: a direct-on-line motor starter built through
//! the novo-core API and drawn by `model::PageScene`.
//!
//! ```text
//! cargo run -p novo-canvas --example demo --features model
//! ```
//!
//! Click or box-select objects; the status bar shows the model ids behind
//! the selection.

use eframe::egui;
use novo_canvas::model::{self, PageScene, PlaceholderArt};
use novo_canvas::widget::CanvasView;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NovoElectric canvas demo")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "novo-canvas-demo",
        options,
        Box::new(|cc| {
            let rs = cc
                .wgpu_render_state
                .as_ref()
                .expect("eframe must use the wgpu renderer");
            novo_canvas::widget::install(rs, 1);
            Ok(Box::new(Demo::new()))
        }),
    )
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
