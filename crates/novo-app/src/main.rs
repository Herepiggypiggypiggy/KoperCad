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
        Box::new(|_cc| Ok(Box::<App>::default())),
    )
}

#[derive(Default)]
struct App;

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("menu").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("NovoElectric");
                ui.label(format!("model v{}", novo_core::MODEL_VERSION));
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label("Canvas arrives with WS-03");
            });
        });
    }
}
