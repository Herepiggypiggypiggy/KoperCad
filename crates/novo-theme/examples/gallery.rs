//! Every widget of the design system in every state, in the theme and
//! density you pick, so changes to the look can be checked by eye and
//! screenshotted for pull requests.
//!
//! `cargo run -p novo-theme --example gallery`
//!
//! `GALLERY_THEME=light|dark|system` and `GALLERY_DENSITY=compact|standard|
//! touch` set the starting choice (for scripts/screenshot.sh).

use eframe::egui;
use egui::{Frame, Margin, RichText, ScrollArea, Stroke, Ui, Vec2};
use novo_theme::widgets::{self, EmptySize, Level};
use novo_theme::{Density, Palette, ThemeChoice, brand, icons, radius, space};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KoperCAD design system")
            .with_inner_size([1360.0, 880.0])
            .with_icon(eframe::icon_data::from_png_bytes(brand::ICON_PNG_256).unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "novo-theme-gallery",
        options,
        Box::new(|cc| {
            let theme = ThemeChoice::from_id(&std::env::var("GALLERY_THEME").unwrap_or_default());
            let density = Density::from_id(&std::env::var("GALLERY_DENSITY").unwrap_or_default());
            novo_theme::apply_with(&cc.egui_ctx, theme, density);
            Ok(Box::new(Gallery {
                theme,
                density,
                ..Gallery::default()
            }))
        }),
    )
}

#[derive(Default)]
struct Gallery {
    theme: ThemeChoice,
    density: Density,
    segment: usize,
    tab: usize,
    tool_on: bool,
    chip_on: bool,
    row: usize,
    text: String,
    number: f64,
    checked: bool,
    option: usize,
    slider: f32,
}

impl eframe::App for Gallery {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let p = *Palette::of(ui);
        egui::Panel::top("bar")
            .frame(
                Frame::new()
                    .fill(p.panel)
                    .stroke(Stroke::new(1.0, p.border_subtle))
                    .inner_margin(Margin::symmetric(12, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("KoperCAD design system").heading());
                    ui.add_space(space::XXL);
                    let mut theme = ThemeChoice::ALL
                        .iter()
                        .position(|t| *t == self.theme)
                        .unwrap_or(0);
                    let labels = ThemeChoice::ALL.map(ThemeChoice::label);
                    if widgets::segmented(ui, &mut theme, &labels) {
                        self.theme = ThemeChoice::ALL[theme];
                        novo_theme::set_theme(&ctx, self.theme);
                    }
                    let mut density = Density::ALL
                        .iter()
                        .position(|d| *d == self.density)
                        .unwrap_or(1);
                    let labels = Density::ALL.map(Density::label);
                    if widgets::segmented(ui, &mut density, &labels) {
                        self.density = Density::ALL[density];
                        novo_theme::set_density(&ctx, self.density);
                    }
                });
            });
        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.panel).inner_margin(Margin::same(16)))
            .show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.columns(3, |cols| {
                        self.column_one(&mut cols[0], &p);
                        self.column_two(&mut cols[1], &p);
                        self.column_three(&mut cols[2], &p);
                    });
                });
            });
    }
}

impl Gallery {
    fn column_one(&mut self, ui: &mut Ui, p: &Palette) {
        widgets::section_label(ui, "Buttons");
        ui.add_space(space::S);
        ui.horizontal_wrapped(|ui| {
            let _ = ui.button("Button");
            widgets::primary_button(ui, "Primary");
            widgets::ghost_button(ui, "Ghost");
            widgets::danger_button(ui, "Delete page");
            ui.add_enabled(false, egui::Button::new("Disabled"));
        });
        ui.add_space(space::M);
        ui.horizontal(|ui| {
            widgets::icon_button(ui, icons::action::SAVE, "Save");
            widgets::icon_button(ui, icons::action::UNDO, "Undo");
            if widgets::icon_toggle(ui, icons::action::PIN, self.tool_on, "Pin").clicked() {
                self.tool_on = !self.tool_on;
            }
            if widgets::chip(ui, "=A1+CAB", self.chip_on).clicked() {
                self.chip_on = !self.chip_on;
            }
            widgets::chip(ui, "Unplaced", false);
            widgets::kbd(ui, "Ctrl+K");
        });

        ui.add_space(space::XL);
        widgets::section_label(ui, "Switching");
        ui.add_space(space::S);
        widgets::segmented(ui, &mut self.segment, &["Pages", "Issues", "Changes"]);
        ui.add_space(space::M);
        widgets::tabs(ui, &mut self.tab, &["Terminals", "Cables", "Devices"]);

        ui.add_space(space::XL);
        widgets::section_label(ui, "Inputs");
        ui.add_space(space::S);
        ui.add(
            egui::TextEdit::singleline(&mut self.text)
                .hint_text(format!("{}  Filter pages", icons::action::FIND))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(space::S);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Voltage").color(p.text_secondary));
            ui.add(egui::DragValue::new(&mut self.number).suffix(" V"));
        });
        ui.checkbox(&mut self.checked, "Show marks");
        ui.radio_value(&mut self.option, 0, "Draw them");
        ui.radio_value(&mut self.option, 1, "Ask me each time");
        ui.add(egui::Slider::new(&mut self.slider, 0.0..=1.0).text("Grid"));
        egui::ComboBox::from_id_salt("combo")
            .selected_text("Standard")
            .show_ui(ui, |ui| {
                let _ = ui.selectable_label(true, "Standard");
                let _ = ui.selectable_label(false, "Compact");
            });
    }

    fn column_two(&mut self, ui: &mut Ui, p: &Palette) {
        widgets::section_label(ui, "Rows");
        ui.add_space(space::S);
        for (i, (num, name, meta)) in [
            ("1", "Main circuit", "=A1 +CAB1"),
            ("2", "Control circuit", "=A1 +CAB1"),
            ("10", "Bill of materials", ""),
            ("40", "Terminal diagram", "=A1+CAB-X1"),
        ]
        .into_iter()
        .enumerate()
        {
            let response = widgets::row(ui, self.row == i, |ui| {
                ui.label(RichText::new(num).monospace().color(p.text_tertiary));
                ui.label(name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(meta)
                            .size(novo_theme::tokens::FONT_SMALL)
                            .color(p.text_tertiary),
                    );
                });
            });
            if response.clicked() {
                self.row = i;
            }
        }

        ui.add_space(space::XL);
        widgets::section_label(ui, "Menu");
        ui.add_space(space::S);
        Frame::new()
            .fill(p.raised)
            .stroke(Stroke::new(1.0, p.border_subtle))
            .corner_radius(radius::SURFACE)
            .inner_margin(Margin::same(6))
            .shadow(ui.visuals().popup_shadow)
            .show(ui, |ui| {
                widgets::menu_item(
                    ui,
                    Some(icons::action::NEW_PROJECT),
                    "New project",
                    Some("Ctrl+N"),
                    true,
                );
                widgets::menu_item(
                    ui,
                    Some(icons::action::OPEN_PROJECT),
                    "Open project",
                    Some("Ctrl+O"),
                    true,
                );
                widgets::menu_item(ui, Some(icons::action::SAVE), "Save", Some("Ctrl+S"), true);
                widgets::menu_item(
                    ui,
                    Some(icons::action::SETTINGS),
                    "Rules",
                    Some("Comes later"),
                    false,
                );
            });

        ui.add_space(space::XL);
        widgets::section_label(ui, "Messages");
        ui.add_space(space::S);
        for (level, text) in [
            (Level::Info, "Imported 48 parts from the catalogue."),
            (Level::Success, "Checks passed on all 12 pages."),
            (Level::Warning, "2 pages have no title."),
            (Level::Error, "-K1 and -K2 share a tag."),
        ] {
            widgets::banner(ui, level, text, |ui| {
                widgets::ghost_button(ui, "Show");
            });
            ui.add_space(space::S);
        }
        ui.horizontal(|ui| {
            widgets::issue_pill(ui, 0, 0);
            widgets::issue_pill(ui, 2, 5);
        });
    }

    fn column_three(&mut self, ui: &mut Ui, p: &Palette) {
        widgets::section_label(ui, "Empty state");
        ui.add_space(space::S);
        Frame::new()
            .fill(p.sunken)
            .stroke(Stroke::new(1.0, p.border_subtle))
            .corner_radius(radius::SURFACE)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::empty_state(
                    ui,
                    EmptySize::Panel,
                    icons::status::OK,
                    "All clear",
                    "No problems found on any page.",
                    |ui| {
                        let _ = ui.button("Run checks again");
                    },
                );
            });

        ui.add_space(space::XL);
        widgets::section_label(ui, "Colour roles");
        ui.add_space(space::S);
        let swatches: [(&str, egui::Color32); 18] = [
            ("sunken", p.sunken),
            ("panel", p.panel),
            ("raised", p.raised),
            ("control", p.control),
            ("hover", p.hover),
            ("border", p.border_subtle),
            ("border strong", p.border_strong),
            ("text", p.text_primary),
            ("secondary", p.text_secondary),
            ("tertiary", p.text_tertiary),
            ("accent", p.accent),
            ("accent hover", p.accent_hover),
            ("accent wash", p.accent_wash),
            ("info", p.info),
            ("success", p.success),
            ("warning", p.warning),
            ("error", p.error),
            ("on accent", p.on_accent),
        ];
        egui::Grid::new("swatches")
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                for (i, (name, color)) in swatches.into_iter().enumerate() {
                    ui.vertical(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(56.0, 28.0), egui::Sense::hover());
                        ui.painter().rect(
                            rect,
                            radius::CONTROL,
                            color,
                            Stroke::new(1.0, p.border_strong),
                            egui::StrokeKind::Inside,
                        );
                        ui.label(
                            RichText::new(name)
                                .size(novo_theme::tokens::FONT_SMALL - 1.0)
                                .color(p.text_secondary),
                        );
                    });
                    if i % 6 == 5 {
                        ui.end_row();
                    }
                }
            });

        ui.add_space(space::XL);
        widgets::section_label(ui, "Type");
        ui.add_space(space::S);
        ui.label(
            RichText::new("Display 20")
                .size(20.0)
                .family(novo_theme::fonts::strong_family()),
        );
        ui.label(RichText::new("Title 15").heading());
        ui.label("Body 13: the quick brown fox jumps over the lazy dog");
        ui.label(
            RichText::new("Caption 11 for hints and the HUD")
                .small()
                .color(p.text_secondary),
        );
        ui.label(RichText::new("-K1:13  3RV2011-1JA10  0123456789").monospace());
        ui.horizontal(|ui| {
            for (icon, name) in [
                (icons::tool::SELECT, "select"),
                (icons::tool::PLACE, "place"),
                (icons::tool::CONNECT, "connect"),
                (icons::panel::TERMINALS, "terminals"),
                (icons::panel::CABLES, "cables"),
                (icons::mark::PROPOSAL, "proposal"),
            ] {
                ui.label(RichText::new(icon).size(novo_theme::icon_size::HEADER))
                    .on_hover_text(name);
            }
        });
    }
}
