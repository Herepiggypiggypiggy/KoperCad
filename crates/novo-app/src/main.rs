//! NovoElectric desktop application shell (owned by WS-01).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NovoElectric")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "NovoElectric",
        options,
        Box::new(|cc| {
            if let Some(rs) = cc.wgpu_render_state.as_ref() {
                novo_canvas::widget::install(rs, 1);
            }
            Ok(Box::<App>::default())
        }),
    )
}

struct App {
    /// Demo project until the schematic editor (WS-06) can draw real pages.
    _doc: novo_core::Document,
    page: novo_canvas::model::PageScene,
    canvas: novo_canvas::widget::CanvasView,
}

impl Default for App {
    fn default() -> Self {
        let (doc, page) = novo_canvas::model::demo_document().expect("demo project builds");
        let page = novo_canvas::model::PageScene::new(
            doc.project(),
            page,
            &novo_canvas::model::PlaceholderArt,
        );
        Self {
            _doc: doc,
            page,
            canvas: novo_canvas::widget::CanvasView::new(),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("menu").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("NovoElectric");
                ui.label(format!("model v{}", novo_core::MODEL_VERSION));
            });
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.canvas.show(ui, self.page.scene());
            });
    }
}
