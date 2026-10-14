//! Shows every common widget in the NovoElectric theme, laid out like the
//! main window, so theme changes can be checked by eye.
//!
//! `cargo run -p novo-theme --example preview`

use eframe::egui;
use egui::Rect;
use novo_theme::{brand, icons, tokens};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NovoElectric theme preview")
            .with_inner_size([1280.0, 800.0])
            .with_icon(eframe::icon_data::from_png_bytes(brand::ICON_PNG_256).unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "novo-theme-preview",
        options,
        Box::new(|cc| {
            novo_theme::apply(&cc.egui_ctx);
            Ok(Box::<Preview>::default())
        }),
    )
}

struct Preview {
    name: String,
    voltage: f32,
    snap: bool,
    tab: usize,
    selected: usize,
    show_about: bool,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            name: "-K1".into(),
            voltage: 24.0,
            snap: true,
            tab: 0,
            selected: 1,
            show_about: false,
        }
    }
}

impl eframe::App for Preview {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    let _ = ui.button(format!("{}  New project", icons::action::NEW_PROJECT));
                    let _ = ui.button(format!("{}  Open...", icons::action::OPEN_PROJECT));
                    ui.separator();
                    let _ = ui.button(format!("{}  Save", icons::action::SAVE));
                    let _ = ui.button(format!("{}  Export PDF...", icons::action::EXPORT_PDF));
                });
                for menu in ["Edit", "View", "Insert", "Project", "Tools"] {
                    ui.menu_button(menu, |ui| {
                        let _ = ui.button("Item");
                    });
                }
                ui.menu_button("Help", |ui| {
                    if ui.button("About NovoElectric").clicked() {
                        self.show_about = true;
                    }
                });
            });
        });
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                for icon in [
                    icons::action::SAVE,
                    icons::action::UNDO,
                    icons::action::REDO,
                ] {
                    let _ = ui.button(icon);
                }
                ui.separator();
                for icon in [
                    icons::action::SELECT,
                    icons::action::WIRE,
                    icons::action::SYMBOL,
                    icons::action::TERMINAL,
                    icons::action::TEXT,
                ] {
                    let _ = ui.selectable_label(icon == icons::action::SELECT, icon);
                }
                ui.separator();
                ui.toggle_value(&mut self.snap, format!("{} Snap", icons::action::SNAP));
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.small("X 125.0  Y 80.0 mm");
                ui.separator();
                ui.small("Grid 2.5 mm");
                ui.separator();
                ui.colored_label(
                    tokens::SUCCESS,
                    format!("{} No design errors", icons::status::OK),
                );
            });
        });
        egui::Panel::left("project")
            .default_size(240.0)
            .show(ui, |ui| {
                ui.heading("Project");
                ui.add_space(4.0);
                egui::CollapsingHeader::new(format!("{} Pages", icons::action::PAGE))
                    .default_open(true)
                    .show(ui, |ui| {
                        for (i, page) in
                            ["1 Cover", "2 Power supply", "3 Motor starter", "4 Controls"]
                                .into_iter()
                                .enumerate()
                        {
                            if ui.selectable_label(self.selected == i, page).clicked() {
                                self.selected = i;
                            }
                        }
                    });
                egui::CollapsingHeader::new(format!("{} Devices", icons::action::PARTS)).show(
                    ui,
                    |ui| {
                        ui.monospace("=A1+CAB1-K1");
                        ui.monospace("=A1+CAB1-Q1");
                    },
                );
            });
        egui::Panel::right("properties")
            .default_size(280.0)
            .show(ui, |ui| {
                ui.heading("Properties");
                egui::Grid::new("props")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Tag");
                        ui.text_edit_singleline(&mut self.name);
                        ui.end_row();
                        ui.label("Coil voltage");
                        ui.add(egui::DragValue::new(&mut self.voltage).suffix(" V"));
                        ui.end_row();
                        ui.label("Part");
                        egui::ComboBox::from_id_salt("part")
                            .selected_text("3RT2016-1BB41")
                            .show_ui(ui, |ui| {
                                let _ = ui.selectable_label(true, "3RT2016-1BB41");
                                let _ = ui.selectable_label(false, "3RT2017-1BB42");
                            });
                        ui.end_row();
                        ui.label("Snap");
                        ui.checkbox(&mut self.snap, "");
                        ui.end_row();
                    });
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Unsaved changes").color(tokens::WARNING));
                ui.label(egui::RichText::new("Terminal X1:3 is unconnected").color(tokens::ERROR));
                ui.weak("Hint text looks like this");
                ui.hyperlink_to("Open part datasheet", "https://example.com");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let _ = ui.button("Cancel");
                    let _ = ui.add(
                        egui::Button::new(
                            egui::RichText::new("Apply").color(tokens::TEXT_ON_ACCENT),
                        )
                        .fill(tokens::ACCENT),
                    );
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(novo_theme::canvas::to_color32(
                novo_theme::CANVAS_DARK.background,
            )))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, tab) in ["2 Power supply", "3 Motor starter"]
                        .into_iter()
                        .enumerate()
                    {
                        if ui.selectable_label(self.tab == i, tab).clicked() {
                            self.tab = i;
                        }
                    }
                });
                let rect = ui.available_rect_before_wrap().shrink(24.0);
                brand::paint_mark(
                    ui.painter(),
                    Rect::from_center_size(rect.center(), egui::Vec2::splat(160.0)),
                    true,
                );
            });

        if self.show_about {
            egui::Window::new("About")
                .collapsible(false)
                .resizable(false)
                .open(&mut self.show_about)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::Vec2::splat(48.0), egui::Sense::hover());
                        brand::paint_mark(ui.painter(), rect, true);
                        ui.vertical(|ui| {
                            ui.heading(brand::NAME);
                            ui.weak(brand::TAGLINE);
                        });
                    });
                });
        }
    }
}
