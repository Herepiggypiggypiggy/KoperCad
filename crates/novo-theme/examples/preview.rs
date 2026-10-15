//! The KoperCAD theme on the Workbench window from docs/DESIGN.md §3,
//! so theme changes can be checked by eye: top bar, outline, canvas with
//! the HUD and the floating tool dock, the inspector overlay and the linked
//! drawer.
//!
//! `cargo run -p novo-theme --example preview`

use eframe::egui;
use egui::{Align2, Frame, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use novo_theme::{brand, icons, tokens, widgets};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("KoperCAD")
            .with_inner_size([1440.0, 860.0])
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
    lens: usize,
    nav_mode: usize,
    filter: String,
    tool: usize,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            lens: 0,
            nav_mode: 0,
            filter: String::new(),
            tool: 2,
        }
    }
}

const TERMINALS: [(&str, &str, &str, &str); 6] = [
    ("-X1:1", "L1", "-K1:2", "-W1 core 1"),
    ("-X1:2", "L2", "-K1:4", "-W1 core 2"),
    ("-X1:3", "L3", "-K1:6", "-W1 core 3"),
    ("-X1:PE", "PE", "PE rail", "-W1 gn/ye"),
    ("-X1:4", "24V", "-S1:13", "-W2 core 1"),
    ("-X1:5", "0V", "-H1:X2", "-W2 core 2"),
];

impl eframe::App for Preview {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.top_bar(ui);
        self.workbench(ui);
    }
}

fn canvas_frame() -> Frame {
    Frame::new().fill(novo_theme::canvas::to_color32(
        novo_theme::CANVAS_DARK.background,
    ))
}

impl Preview {
    // ------------------------------------------------------------------
    // Shared chrome

    fn top_bar(&mut self, ui: &mut Ui) {
        egui::Panel::top("top_bar")
            .exact_size(42.0)
            .frame(
                Frame::new()
                    .fill(tokens::BG_1)
                    .inner_margin(Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let (rect, mark) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                    if mark.hovered() {
                        ui.painter().rect_filled(rect.expand(4.0), 4, tokens::BG_3);
                    }
                    brand::paint_mark(ui.painter(), rect, false);
                    mark.on_hover_text("Project: new, open, export, settings");
                    ui.add_space(14.0);

                    // The two real canvases.
                    for (i, lens) in ["Schematic", "Cabinet"].iter().enumerate() {
                        if widgets::segment(ui, lens, self.lens == i).clicked() {
                            self.lens = i;
                        }
                    }
                    ui.add_space(14.0);
                    ui.label(RichText::new("|").color(tokens::BORDER_STRONG));
                    ui.add_space(8.0);

                    // Where the page sits; a click on a segment filters the
                    // outline to it.
                    let crumbs = ["Line 4 Conveyor", "=A1 Power", "+CAB1", "3 Motor starter"];
                    for (i, crumb) in crumbs.iter().enumerate() {
                        let last = i + 1 == crumbs.len();
                        let color = if last {
                            tokens::TEXT
                        } else {
                            tokens::TEXT_MUTED
                        };
                        let crumb_button = ui.add(
                            egui::Button::new(RichText::new(*crumb).color(color)).frame(false),
                        );
                        if !last {
                            crumb_button.on_hover_text("Show these pages in the outline");
                        }
                        if !last {
                            ui.label(RichText::new("/").color(tokens::TEXT_FAINT));
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Saved").small().color(tokens::TEXT_FAINT));
                        ui.add_space(8.0);
                        widgets::issue_pill(ui, 0, 2);
                        ui.add_space(12.0);
                        search_field(ui, 320.0);
                    });
                });
            });
    }

    fn tool_dock(&mut self, ui: &mut Ui, canvas: Rect, hint: &str) {
        egui::Area::new(egui::Id::new("tool_dock"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(canvas.center().x - 250.0, canvas.bottom() - 54.0))
            .show(ui.ctx(), |ui| {
                Frame::new()
                    .fill(tokens::BG_2)
                    .stroke(Stroke::new(1.0, tokens::BORDER_STRONG))
                    .corner_radius(8)
                    .shadow(ui.visuals().popup_shadow)
                    .inner_margin(Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let tools = [
                                (icons::tool::SELECT, "Select  Esc"),
                                (icons::tool::PLACE, "Place  I"),
                                (icons::tool::CONNECT, "Connect  W"),
                                (icons::tool::TEXT, "Text"),
                            ];
                            for (i, (icon, tip)) in tools.iter().enumerate() {
                                if widgets::tool_button(ui, icon, self.tool == i, tip).clicked() {
                                    self.tool = i;
                                }
                            }
                            ui.separator();
                            ui.label(RichText::new(hint).color(tokens::TEXT));
                            ui.label(
                                RichText::new("R rotate · Esc stop")
                                    .small()
                                    .color(tokens::TEXT_FAINT),
                            );
                            ui.separator();
                            ui.label(
                                RichText::new(format!(
                                    "{} 3 proposals  Tab",
                                    icons::mark::PROPOSAL
                                ))
                                .small()
                                .color(tokens::INFO),
                            );
                            ui.add_space(4.0);
                        });
                    });
            });
    }

    fn hud(&self, ui: &Ui, canvas: Rect) {
        ui.painter().text(
            canvas.right_top() + Vec2::new(-14.0, 12.0),
            Align2::RIGHT_TOP,
            "x 125.0  y 80.0 mm    grid 2.5    snap    62%",
            egui::FontId::monospace(10.5),
            tokens::TEXT_FAINT,
        );
    }

    // ------------------------------------------------------------------
    // Workbench

    fn workbench(&mut self, ui: &mut Ui) {
        egui::Panel::left("outline")
            .default_size(248.0)
            .frame(
                Frame::new()
                    .fill(tokens::BG_1)
                    .inner_margin(Margin::symmetric(10, 10)),
            )
            .show(ui, |ui| self.outline(ui));

        egui::Panel::bottom("drawer")
            .default_size(206.0)
            .frame(
                Frame::new()
                    .fill(tokens::BG_1)
                    .inner_margin(Margin::symmetric(12, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, tab) in ["Terminals", "Cables", "Devices"].iter().enumerate() {
                        let _ = widgets::segment(ui, tab, i == 0);
                    }
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("-X1  ·  6 terminals  ·  linked to the drawing")
                            .small()
                            .color(tokens::TEXT_FAINT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let _ = ui.add(
                            egui::Button::new(RichText::new(icons::X).color(tokens::TEXT_FAINT))
                                .frame(false),
                        );
                        let _ = ui
                            .add(
                                egui::Button::new(
                                    RichText::new(icons::SQUARE_SPLIT_HORIZONTAL)
                                        .color(tokens::TEXT_FAINT),
                                )
                                .frame(false),
                            )
                            .on_hover_text("Dock to the right");
                    });
                });
                ui.add_space(4.0);
                terminal_table(ui, &[0, 1, 2]);
            });

        egui::CentralPanel::default()
            .frame(canvas_frame())
            .show(ui, |ui| {
                let canvas = ui.available_rect_before_wrap();
                ui.allocate_rect(canvas, Sense::click());
                let q1 = mock_sheet(ui.painter(), canvas.shrink2(Vec2::new(48.0, 30.0)));
                marks(ui.painter(), q1);
                self.hud(ui, canvas);
                self.tool_dock(ui, canvas, "Click the second pin");
                // Inspector floats over the canvas edge; the drawing never moves.
                let card = Rect::from_min_size(
                    Pos2::new(canvas.right() - 292.0, canvas.top() + 36.0),
                    Vec2::new(276.0, 330.0),
                );
                inspector_card(ui, card, true);
            });
    }

    fn outline(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (i, mode) in ["Pages", "Issues", "Changes"].iter().enumerate() {
                if widgets::segment(ui, mode, self.nav_mode == i).clicked() {
                    self.nav_mode = i;
                }
            }
        });
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text(format!("{}  =A1, +CAB1, motor…", icons::FUNNEL))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(10.0);
        widgets::section_label(ui, "Recent");
        for (n, t, cur) in [
            ("3", "Motor starter", true),
            ("12", "PLC inputs 2", false),
            ("6", "Terminals -X1", false),
        ] {
            page_row(ui, n, t, "", cur);
        }
        ui.add_space(10.0);
        widgets::section_label(ui, "All pages  ·  48");
        for (n, t, s) in [
            ("1", "Cover", ""),
            ("2", "Supply 400 V", "=A1 +CAB1"),
            ("3", "Motor starter", "=A1 +CAB1"),
            ("4", "Controls", "=A2 +CAB1"),
            ("5", "Safety relay", "=A2 +CAB1"),
            ("6", "Terminals -X1", "+CAB1"),
            ("7", "Field motors", "=A3 +M1"),
            ("8", "Cables", ""),
        ] {
            page_row(ui, n, t, s, n == "3");
        }
    }
}

// ----------------------------------------------------------------------
// Pieces

/// The mark grammar on the drawing: a proposal (dashed steel blue, badge on
/// the nearest one) and a warning glyph (screen sized, top right of its
/// object). Positions are relative to `-Q1` in the mock sheet.
fn marks(p: &egui::Painter, q1: Pos2) {
    let blue = tokens::INFO.gamma_multiply(0.75);
    let dash = Stroke::new(1.2, blue);
    // Proposed terminals where the motor wires leave +CAB1.
    for dx in [-24.0, 24.0, 72.0] {
        let c = q1 + Vec2::new(dx, 126.0);
        let r = Rect::from_center_size(c, Vec2::new(14.0, 18.0));
        let pts = [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
            r.left_top(),
        ];
        p.extend(egui::Shape::dashed_line(&pts, dash, 3.0, 2.0));
    }
    let badge = Rect::from_min_size(q1 + Vec2::new(90.0, 115.0), Vec2::new(186.0, 22.0));
    p.rect(
        badge,
        4,
        tokens::BG_2,
        Stroke::new(1.0, blue),
        egui::StrokeKind::Inside,
    );
    p.text(
        badge.left_center() + Vec2::new(8.0, 0.0),
        Align2::LEFT_CENTER,
        "-X1:1..3  ↵ accept  ·  Tab next",
        egui::FontId::proportional(11.5),
        tokens::INFO,
    );
    // A warning on -M1: no part chosen.
    let w = q1 + Vec2::new(84.0, 178.0);
    let tri = vec![
        w + Vec2::new(0.0, -6.0),
        w + Vec2::new(6.0, 5.0),
        w + Vec2::new(-6.0, 5.0),
    ];
    p.add(egui::Shape::convex_polygon(
        tri,
        tokens::WARNING,
        Stroke::NONE,
    ));
}

fn search_field(ui: &mut Ui, width: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click());
    let fill = if response.hovered() {
        tokens::BG_3
    } else {
        tokens::BG_0
    };
    ui.painter().rect(
        rect,
        4,
        fill,
        Stroke::new(1.0, tokens::BORDER),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.left_center() + Vec2::new(10.0, 0.0),
        Align2::LEFT_CENTER,
        format!("{}   Search or run a command", icons::action::FIND),
        egui::FontId::proportional(tokens::FONT_BODY),
        tokens::TEXT_FAINT,
    );
    ui.painter().text(
        rect.right_center() - Vec2::new(10.0, 0.0),
        Align2::RIGHT_CENTER,
        "Ctrl K",
        egui::FontId::monospace(10.5),
        tokens::TEXT_FAINT,
    );
}

fn page_row(ui: &mut Ui, number: &str, title: &str, structure: &str, current: bool) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
    if current {
        ui.painter().rect_filled(rect, 3, tokens::ACCENT_SELECTION);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 3, tokens::BG_3);
    }
    let p = ui.painter();
    p.text(
        rect.left_center() + Vec2::new(8.0, 0.0),
        Align2::LEFT_CENTER,
        number,
        egui::FontId::monospace(11.5),
        tokens::TEXT_FAINT,
    );
    p.text(
        rect.left_center() + Vec2::new(34.0, 0.0),
        Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(tokens::FONT_BODY),
        if current {
            tokens::TEXT
        } else {
            tokens::TEXT_MUTED
        },
    );
    p.text(
        rect.right_center() - Vec2::new(8.0, 0.0),
        Align2::RIGHT_CENTER,
        structure,
        egui::FontId::monospace(10.5),
        tokens::TEXT_FAINT,
    );
}

fn inspector_card(ui: &Ui, rect: Rect, pinnable: bool) {
    let ctx = ui.ctx().clone();
    egui::Area::new(egui::Id::new(("inspector", pinnable)))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(&ctx, |ui| {
            Frame::new()
                .fill(tokens::BG_2)
                .stroke(Stroke::new(1.0, tokens::BORDER_STRONG))
                .corner_radius(6)
                .shadow(ui.visuals().window_shadow)
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(rect.width() - 28.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Circuit breaker").color(tokens::TEXT_MUTED));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if pinnable {
                                let _ = ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(icons::action::PIN)
                                                .color(tokens::TEXT_FAINT),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Pin the inspector");
                            }
                        });
                    });
                    ui.label(
                        RichText::new("-Q1")
                            .size(20.0)
                            .family(novo_theme::fonts::strong_family()),
                    );
                    ui.label(
                        RichText::new("=A1 Power  +CAB1  ·  page 3, column 4")
                            .monospace()
                            .small()
                            .color(tokens::TEXT_FAINT),
                    );
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Block").small().color(tokens::TEXT_FAINT));
                        let _ = ui.link(RichText::new("DOL feeder 4 kW").small());
                        ui.label(
                            RichText::new("2 overrides")
                                .small()
                                .color(tokens::TEXT_MUTED),
                        );
                    });
                    ui.add_space(10.0);
                    widgets::section_label(ui, "Relations");
                    for (k, v) in [
                        ("Feeds", "-K1  1 3 5"),
                        ("Supplied by", "/2.8  L1 L2 L3"),
                        ("Contacts", "/4.2  13-14"),
                    ] {
                        ui.horizontal(|ui| {
                            ui.allocate_ui_with_layout(
                                Vec2::new(86.0, 20.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_min_width(86.0);
                                    ui.label(RichText::new(k).color(tokens::TEXT_MUTED));
                                },
                            );
                            let _ = ui.link(RichText::new(v).monospace());
                        });
                    }
                    ui.add_space(10.0);
                    widgets::section_label(ui, "Part");
                    for (k, v, overridden) in [
                        ("Article", "3RV2011-1JA10", false),
                        ("Setting", "9 A", true),
                    ] {
                        ui.horizontal(|ui| {
                            ui.allocate_ui_with_layout(
                                Vec2::new(86.0, 20.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_min_width(86.0);
                                    ui.label(RichText::new(k).color(tokens::TEXT_MUTED));
                                },
                            );
                            ui.label(RichText::new(v).monospace());
                            if overridden {
                                ui.label(
                                    RichText::new(format!("{} override", icons::mark::OVERRIDE))
                                        .small()
                                        .color(tokens::TEXT_MUTED),
                                );
                                let _ = ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(icons::action::RESET)
                                                .color(tokens::TEXT_MUTED),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Reset to the block's 7 to 10 A");
                            }
                        });
                    }
                });
        });
}

fn terminal_table(ui: &mut Ui, highlighted: &[usize]) {
    egui::Grid::new("terminals")
        .num_columns(4)
        .striped(true)
        .spacing([28.0, 6.0])
        .show(ui, |ui| {
            for h in ["Terminal", "Potential", "Inside", "Outside"] {
                ui.label(
                    RichText::new(h.to_uppercase())
                        .small()
                        .extra_letter_spacing(1.0)
                        .color(tokens::TEXT_FAINT),
                );
            }
            ui.end_row();
            for (i, (t, pot, inside, outside)) in TERMINALS.iter().enumerate() {
                let on = highlighted.contains(&i);
                let c = if on { tokens::TEXT } else { tokens::TEXT_MUTED };
                let tag =
                    RichText::new(*t)
                        .monospace()
                        .color(if on { tokens::ACCENT_HOVER } else { c });
                ui.label(tag);
                ui.label(RichText::new(*pot).monospace().color(c));
                ui.label(RichText::new(*inside).monospace().color(c));
                ui.label(RichText::new(*outside).monospace().color(c));
                ui.end_row();
            }
        });
}

/// A few schematic strokes in the canvas palette: sheet, grid, three phases
/// through a contactor to a motor.
fn mock_sheet(painter: &egui::Painter, rect: Rect) -> egui::Pos2 {
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
        // -Q1 feeds -K1: a second open contact on the same phase.
        painter.line_segment([at(x, 18.0), at(x, 21.0)], wire);
        painter.circle_filled(at(x, 21.0), 2.0, c(p.symbol));
        painter.line_segment([at(x, 24.0), at(x - 1.6, 21.0)], sym);
        painter.line_segment([at(x, 24.0), at(x, 28.0)], wire);
        painter.line_segment([at(x, 28.0), at(26.0 - (26.0 - x) * 0.5, 30.2)], wire);
    }
    // -K1's three contacts are one device: a dashed mechanical link.
    painter.extend(egui::Shape::dashed_line(
        &[at(19.2, 22.5), at(27.2, 22.5)],
        Stroke::new(1.0, c(p.symbol)),
        3.0,
        3.0,
    ));
    painter.text(
        at(30.0, 22.5),
        egui::Align2::LEFT_CENTER,
        "-K1",
        font.clone(),
        c(p.text),
    );
    painter.text(
        at(30.0, 16.5),
        egui::Align2::LEFT_CENTER,
        "-Q1",
        font.clone(),
        c(p.text),
    );
    painter.circle_stroke(at(26.0, 33.0), 3.0 * step, sym);
    painter.text(
        at(26.0, 33.0),
        egui::Align2::CENTER_CENTER,
        "M\n3~",
        egui::FontId::proportional(13.0),
        c(p.symbol),
    );
    painter.text(
        at(30.5, 33.0),
        egui::Align2::LEFT_CENTER,
        "-M1",
        font.clone(),
        c(p.text),
    );
    painter.text(
        at(30.0, 23.5),
        egui::Align2::LEFT_TOP,
        "/4.2",
        font,
        c(p.cross_reference),
    );
    at(22.0, 16.0)
}
