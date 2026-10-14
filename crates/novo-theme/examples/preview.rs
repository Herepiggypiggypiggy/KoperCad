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
    tool: usize,
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
            tool: 0,
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
                use novo_theme::widgets::tool_button;
                tool_button(ui, icons::action::SAVE, false, "Save  Ctrl+S");
                tool_button(ui, icons::action::UNDO, false, "Undo  Ctrl+Z");
                tool_button(ui, icons::action::REDO, false, "Redo  Ctrl+Y");
                ui.separator();
                for (i, (icon, tip)) in [
                    (icons::action::SELECT, "Select  Esc"),
                    (icons::action::WIRE, "Wire  W"),
                    (icons::action::SYMBOL, "Symbol  S"),
                    (icons::action::TERMINAL, "Terminal  T"),
                    (icons::action::TEXT, "Text"),
                ]
                .into_iter()
                .enumerate()
                {
                    if tool_button(ui, icon, self.tool == i, tip).clicked() {
                        self.tool = i;
                    }
                }
                ui.separator();
                tool_button(ui, icons::action::SNAP, self.snap, "Snap to grid");
                tool_button(ui, icons::action::GRID, true, "Show grid");
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
                ui.add_space(6.0);
                novo_theme::widgets::section_label(ui, "Project");
                ui.add_space(2.0);
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
                ui.add_space(6.0);
                novo_theme::widgets::section_label(ui, "Properties");
                ui.add_space(2.0);
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
                    novo_theme::widgets::primary_button(ui, "Apply");
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
                let rect = ui.available_rect_before_wrap().shrink(28.0);
                mock_sheet(ui.painter(), rect);
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

/// A few schematic strokes in the canvas palette: sheet, grid, three phases
/// through a contactor to a motor.
fn mock_sheet(painter: &egui::Painter, rect: Rect) {
    use egui::{Pos2, Stroke, Vec2, pos2};
    let p = novo_theme::CANVAS_DARK;
    let c = novo_theme::canvas::to_color32;
    let sheet = Rect::from_center_size(
        rect.center(),
        Vec2::new(rect.width(), rect.width() / 1.414).min(rect.size()),
    );
    painter.rect_filled(sheet, 0, c(p.sheet));
    let step = 12.0;
    let mut y = sheet.top() + step;
    while y < sheet.bottom() {
        let mut x = sheet.left() + step;
        while x < sheet.right() {
            let major = ((x - sheet.left()) / step).round() as i32 % 5 == 0
                && ((y - sheet.top()) / step).round() as i32 % 5 == 0;
            let color = if major { p.grid_major } else { p.grid_minor };
            painter.circle_filled(pos2(x, y), if major { 1.2 } else { 0.8 }, c(color));
            x += step;
        }
        y += step;
    }
    painter.rect_stroke(
        sheet.shrink(10.0),
        0,
        Stroke::new(1.0, c(p.sheet_frame)),
        egui::StrokeKind::Inside,
    );

    let at = |x: f32, y: f32| Pos2::new(sheet.left() + x * step, sheet.top() + y * step);
    let wire = Stroke::new(1.4, c(p.wire));
    let sym = Stroke::new(1.4, c(p.symbol));
    let sel = Stroke::new(2.2, c(p.selection));
    let font = egui::FontId::monospace(11.0);
    for (i, label) in ["L1", "L2", "L3"].into_iter().enumerate() {
        let x = 20.0 + i as f32 * 4.0;
        painter.text(
            at(x, 4.0),
            egui::Align2::CENTER_BOTTOM,
            label,
            font.clone(),
            c(p.text),
        );
        painter.line_segment([at(x, 5.0), at(x, 14.0)], wire);
        painter.circle_filled(at(x, 14.0), 2.0, c(p.symbol));
        // Open contact: blade from the lower terminal, gap at the top.
        let stroke = if i == 1 { sel } else { sym };
        painter.line_segment([at(x, 18.0), at(x - 1.6, 15.0)], stroke);
        painter.line_segment([at(x, 18.0), at(x, 22.0)], wire);
        painter.line_segment([at(x, 22.0), at(26.0 - (26.0 - x) * 0.5, 27.0)], wire);
    }
    painter.text(
        at(30.0, 16.5),
        egui::Align2::LEFT_CENTER,
        "-Q1",
        font.clone(),
        c(p.text),
    );
    painter.circle_stroke(at(26.0, 30.0), 3.0 * step, sym);
    painter.text(
        at(26.0, 30.0),
        egui::Align2::CENTER_CENTER,
        "M\n3~",
        egui::FontId::proportional(13.0),
        c(p.symbol),
    );
    painter.text(
        at(30.5, 30.0),
        egui::Align2::LEFT_CENTER,
        "-M1",
        font.clone(),
        c(p.text),
    );
    painter.text(
        at(30.0, 18.0),
        egui::Align2::LEFT_TOP,
        "/4.2",
        font,
        c(p.cross_reference),
    );
}
