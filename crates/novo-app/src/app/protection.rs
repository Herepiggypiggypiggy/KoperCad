//! The protection view: trip curves of the project's protective devices on
//! one log-log plot, selectivity between each device and the one behind it,
//! and the short-circuit current rating of the panel (novo-checks,
//! `protection`).
//!
//! Open it from the command palette (Ctrl+K, "Protection") or the View
//! menu. Everything is read from the project: devices with a rated current
//! and a protection symbol, their data from properties (`trip_curve`,
//! `breaking_capacity_ka`, `trip_class`, `sccr_ka`, ...), the prospective
//! current from the project property `supply.short_circuit_ka`. Maker
//! selectivity and back-up tables are the person's own file,
//! `protection-tables.json` in the KoperCAD folder under `%APPDATA%`, never
//! bundled. What the analysis had to assume is listed in the window.

use std::collections::BTreeSet;
use std::path::PathBuf;

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use novo_checks::Severity;
use novo_checks::protection::devices::MakerTables;
use novo_checks::protection::model::{ProjectDevice, Study, StudyReport};
use novo_checks::protection::sccr::{self, Source as SccrSource};
use novo_checks::protection::selectivity::{Backup, BackupBasis, Basis, Settings, Verdict};
use novo_core::DeviceId;
use novo_theme::tokens;

use super::NovoApp;

/// File name inside the KoperCAD folder under `%APPDATA%`.
pub const TABLES_FILE: &str = "protection-tables.json";

/// `%APPDATA%\KoperCAD\protection-tables.json` (the platform's data folder
/// elsewhere).
pub fn tables_path() -> Option<PathBuf> {
    Some(
        dirs::data_dir()?
            .join(crate::DATA_DIR_NAME)
            .join(TABLES_FILE),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    Curves,
    Selectivity,
    Rating,
}

/// What the window remembers.
pub(super) struct ProtectionState {
    pub(super) open: bool,
    pub(super) tab: Tab,
    /// Devices drawn on the plot.
    pub(super) shown: BTreeSet<DeviceId>,
    /// The study, read again when the project changes.
    pub(super) study: Option<(u64, Study)>,
    /// Supply current typed in the window, kA, instead of the project's.
    supply_override: Option<f64>,
    tables: MakerTables,
    tables_note: Option<String>,
    tables_loaded: bool,
    /// The analyses, for the study and supply current they were run on.
    pub(super) results: Option<Results>,
}

pub(super) struct Results {
    revision: u64,
    supply_ka: Option<f64>,
    pub(super) chain: StudyReport,
    pub(super) rating: sccr::Report,
}

impl ProtectionState {
    pub(super) fn new() -> Self {
        Self {
            open: false,
            tab: Tab::Curves,
            shown: BTreeSet::new(),
            study: None,
            supply_override: None,
            tables: MakerTables::default(),
            tables_note: None,
            tables_loaded: false,
            results: None,
        }
    }

    fn load_tables(&mut self) {
        if self.tables_loaded {
            return;
        }
        self.tables_loaded = true;
        let Some(path) = tables_path() else { return };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        match MakerTables::from_json(&text) {
            Ok(t) => {
                self.tables_note = Some(format!(
                    "{} maker table rows from {}",
                    t.rows.len(),
                    path.display()
                ));
                self.tables = t;
            }
            Err(e) => {
                self.tables_note = Some(format!("Could not read {}: {e}", path.display()));
            }
        }
    }
}

/// Series colours, muted and distinguishable on the dark plot.
const SERIES: [Color32; 8] = [
    Color32::from_rgb(0xC8, 0x8A, 0x5E),
    Color32::from_rgb(0x8F, 0xA8, 0xC8),
    Color32::from_rgb(0x7F, 0xB0, 0x86),
    Color32::from_rgb(0xD6, 0xA5, 0x5C),
    Color32::from_rgb(0xB6, 0x8F, 0xC8),
    Color32::from_rgb(0x6F, 0xB8, 0xB8),
    Color32::from_rgb(0xE0, 0x76, 0x6C),
    Color32::from_rgb(0xC8, 0xC2, 0x6E),
];

impl NovoApp {
    /// The "Protection" window.
    pub(super) fn protection_window(&mut self, ctx: &egui::Context) {
        if !self.protection.open {
            return;
        }
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            self.protection.open = false;
            return;
        };
        let state = &mut self.protection;
        state.load_tables();
        let revision = project.revision();
        if state.study.as_ref().is_none_or(|(r, _)| *r != revision) {
            let study = Study::read(project, &project.netlist());
            // New devices appear on the plot; devices that went are dropped.
            let ids: BTreeSet<DeviceId> = study.devices.iter().map(|d| d.id).collect();
            state.shown.retain(|id| ids.contains(id));
            let first_time = state.study.is_none();
            if first_time {
                state.shown = ids.iter().copied().take(SERIES.len()).collect();
            }
            state.study = Some((revision, study));
            state.results = None;
        }
        let Some((_, study)) = state.study.as_ref() else {
            return;
        };
        let mut study = study.clone();
        if let Some(k) = state.supply_override {
            study.supply_ka = Some(k);
        }
        let stale = state
            .results
            .as_ref()
            .is_none_or(|r| r.revision != revision || r.supply_ka != study.supply_ka);
        if stale {
            state.results = Some(Results {
                revision,
                supply_ka: study.supply_ka,
                chain: study.report(&state.tables, &Settings::default()),
                rating: study.sccr(&project.info().name),
            });
        }
        let mut open = true;
        egui::Window::new("Protection")
            .open(&mut open)
            .default_size([920.0, 600.0])
            .show(ctx, |ui| show(ui, state, &study));
        state.open = open;
    }
}

fn show(ui: &mut egui::Ui, state: &mut ProtectionState, study: &Study) {
    ui.horizontal(|ui| {
        for (tab, label) in [
            (Tab::Curves, "Curves"),
            (Tab::Selectivity, "Selectivity"),
            (Tab::Rating, "Panel rating"),
        ] {
            if novo_theme::widgets::segment(ui, label, state.tab == tab).clicked() {
                state.tab = tab;
            }
        }
        ui.add_space(16.0);
        ui.label("Fault current at the supply");
        let mut k = state.supply_override.or(study.supply_ka).unwrap_or(0.0);
        let before = k;
        ui.add(
            egui::DragValue::new(&mut k)
                .range(0.0..=1000.0)
                .speed(0.1)
                .suffix(" kA"),
        );
        if (k - before).abs() > f64::EPSILON {
            state.supply_override = (k > 0.0).then_some(k);
            state.results = None;
        }
        if state.supply_override.is_some()
            && ui
                .small_button("Use the project's")
                .on_hover_text("Set supply.short_circuit_ka on the project to keep a value")
                .clicked()
        {
            state.supply_override = None;
            state.results = None;
        }
    });
    if let Some(note) = &state.tables_note {
        ui.weak(note);
    }
    ui.separator();
    if study.devices.is_empty() {
        ui.add_space(8.0);
        ui.label("No protective devices with a rated current yet.");
        ui.weak(
            "Place fuses, breakers or overload relays and give each a rated current (property rated_current_a, or from its part). Trip curve, breaking capacity and class are read from the same place.",
        );
        return;
    }
    match state.tab {
        Tab::Curves => curves_tab(ui, state, study),
        Tab::Selectivity => selectivity_tab(ui, state, study),
        Tab::Rating => rating_tab(ui, state, study),
    }
}

fn color_of(index: usize) -> Color32 {
    SERIES[index % SERIES.len()]
}

fn curves_tab(ui: &mut egui::Ui, state: &mut ProtectionState, study: &Study) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(210.0);
            novo_theme::widgets::section_label(ui, "DEVICES");
            egui::ScrollArea::vertical()
                .id_salt("protection_devices")
                .max_height(ui.available_height() - 4.0)
                .show(ui, |ui| {
                    for (i, d) in study.devices.iter().enumerate() {
                        let mut on = state.shown.contains(&d.id);
                        ui.horizontal(|ui| {
                            let swatch = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover()).0;
                            ui.painter().rect_filled(swatch, 2.0, color_of(i));
                            if ui.checkbox(&mut on, "").changed() {
                                if on {
                                    state.shown.insert(d.id);
                                } else {
                                    state.shown.remove(&d.id);
                                }
                            }
                            let r = ui.label(format!("{}  {}", d.tag, d.device.curve.label));
                            if !d.assumed.is_empty() {
                                r.on_hover_text(d.assumed.join("\n"));
                                ui.weak("*");
                            }
                        });
                    }
                    ui.add_space(6.0);
                    ui.weak(
                        "* stands for something the project did not state; hover for what was assumed.",
                    );
                });
        });
        ui.separator();
        let ik_a = study.supply_ka.map(|k| k * 1000.0);
        let series: Vec<(&ProjectDevice, Color32)> = study
            .devices
            .iter()
            .enumerate()
            .filter(|(_, d)| state.shown.contains(&d.id))
            .map(|(i, d)| (d, color_of(i)))
            .collect();
        plot(ui, &series, ik_a);
    });
}

fn fmt_current(a: f64) -> String {
    if a >= 1000.0 {
        let k = a / 1000.0;
        if k >= 100.0 {
            format!("{k:.0} kA")
        } else if k >= 10.0 {
            format!("{k:.1} kA")
        } else {
            format!("{k:.2} kA")
        }
    } else if a >= 100.0 {
        format!("{a:.0} A")
    } else {
        format!("{a:.1} A")
    }
}

fn fmt_time(s: f64) -> String {
    if !s.is_finite() {
        "no limit".to_string()
    } else if s < 1.0 {
        format!("{:.0} ms", s * 1000.0)
    } else if s < 120.0 {
        format!("{s:.1} s")
    } else if s < 7200.0 {
        format!("{:.0} min", s / 60.0)
    } else {
        format!("{:.1} h", s / 3600.0)
    }
}

/// The decade labels along an axis.
fn decade_label_current(exp: i32) -> String {
    fmt_current(10f64.powi(exp))
}

fn decade_label_time(exp: i32) -> String {
    let s = 10f64.powi(exp);
    if s < 1.0 {
        format!("{:.0} ms", s * 1000.0)
    } else if s < 60.0 {
        format!("{s:.0} s")
    } else if s < 3600.0 {
        format!("{:.0} min", s / 60.0)
    } else {
        format!("{:.0} h", s / 3600.0)
    }
}

/// The log-log plot: each device a band from its fastest to its slowest trip
/// time, the supply fault current a dashed line.
fn plot(ui: &mut egui::Ui, series: &[(&ProjectDevice, Color32)], ik_a: Option<f64>) {
    let size = Vec2::new(
        ui.available_width().max(320.0),
        ui.available_height().max(300.0),
    );
    let (outer, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(outer);
    painter.rect_filled(outer, 4.0, tokens::BG_0);
    let area = Rect::from_min_max(
        Pos2::new(outer.left() + 58.0, outer.top() + 12.0),
        Pos2::new(outer.right() - 14.0, outer.bottom() - 26.0),
    );
    // Axis ranges in decades.
    let lowest = series
        .iter()
        .map(|(d, _)| d.device.lowest_trip_a())
        .fold(f64::INFINITY, f64::min);
    let highest = series
        .iter()
        .map(|(d, _)| d.device.rated_a() * 60.0)
        .fold(0.0, f64::max)
        .max(ik_a.unwrap_or(0.0) * 1.5);
    let x0 = if lowest.is_finite() {
        (lowest / 2.0).log10().floor() as i32
    } else {
        0
    };
    let x1 = if highest > 0.0 {
        highest.log10().ceil() as i32
    } else {
        5
    }
    .max(x0 + 2);
    let (y0, y1) = (-3, 4);
    let to_x = |i: f64| {
        area.left() + ((i.log10() - f64::from(x0)) / f64::from(x1 - x0)) as f32 * area.width()
    };
    let to_y = |t: f64| {
        area.bottom() - ((t.log10() - f64::from(y0)) / f64::from(y1 - y0)) as f32 * area.height()
    };
    let font = egui::FontId::proportional(tokens::FONT_SMALL);
    // Grid: decades strong, 2..9 faint.
    for e in x0..=x1 {
        for m in 1..10 {
            let i = m as f64 * 10f64.powi(e);
            if i.log10() > x1 as f64 {
                break;
            }
            let x = to_x(i);
            let stroke = if m == 1 {
                Stroke::new(1.0, tokens::BORDER_STRONG)
            } else {
                Stroke::new(1.0, tokens::BORDER)
            };
            painter.line_segment(
                [Pos2::new(x, area.top()), Pos2::new(x, area.bottom())],
                stroke,
            );
        }
        painter.text(
            Pos2::new(to_x(10f64.powi(e)), area.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            decade_label_current(e),
            font.clone(),
            tokens::TEXT_FAINT,
        );
    }
    for e in y0..=y1 {
        for m in 1..10 {
            let t = m as f64 * 10f64.powi(e);
            if t.log10() > y1 as f64 {
                break;
            }
            let y = to_y(t);
            let stroke = if m == 1 {
                Stroke::new(1.0, tokens::BORDER_STRONG)
            } else {
                Stroke::new(1.0, tokens::BORDER)
            };
            painter.line_segment(
                [Pos2::new(area.left(), y), Pos2::new(area.right(), y)],
                stroke,
            );
        }
        painter.text(
            Pos2::new(area.left() - 6.0, to_y(10f64.powi(e))),
            egui::Align2::RIGHT_CENTER,
            decade_label_time(e),
            font.clone(),
            tokens::TEXT_FAINT,
        );
    }
    let plot_painter = painter.with_clip_rect(area);
    let (lo_a, hi_a) = (10f64.powi(x0), 10f64.powi(x1));
    let n = 360;
    for (d, color) in series {
        let curve = &d.device.curve;
        let samples = curve.sample(lo_a, hi_a, n);
        let top = 10f64.powi(y1);
        let clamp = |t: f64| t.clamp(10f64.powi(y0), top);
        // Band: triangles between neighbouring samples that both have a
        // fastest time.
        let fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 46);
        let mut mesh = egui::Mesh::default();
        let mut prev: Option<(Pos2, Pos2)> = None;
        let mut upper: Vec<Pos2> = Vec::new();
        let mut lower: Vec<Pos2> = Vec::new();
        for s in &samples {
            if !s.t_min.is_finite() {
                prev = None;
                continue;
            }
            let x = to_x(s.current_a);
            let p_top = Pos2::new(x, to_y(clamp(s.t_max)));
            let p_bottom = Pos2::new(x, to_y(clamp(s.t_min)));
            upper.push(p_top);
            lower.push(p_bottom);
            if let Some((pt, pb)) = prev {
                let base = mesh.vertices.len() as u32;
                for p in [pt, pb, p_top, p_bottom] {
                    mesh.colored_vertex(p, fill);
                }
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base + 1, base + 2, base + 3);
            }
            prev = Some((p_top, p_bottom));
        }
        plot_painter.add(egui::Shape::mesh(mesh));
        plot_painter.add(egui::Shape::line(upper, Stroke::new(1.6, *color)));
        let soft = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 150);
        plot_painter.add(egui::Shape::line(lower, Stroke::new(1.0, soft)));
        // Breaking capacity: a tick on the current axis.
        if d.device.icu_a.is_finite() && d.device.icu_a > lo_a && d.device.icu_a < hi_a {
            let x = to_x(d.device.icu_a);
            plot_painter.line_segment(
                [
                    Pos2::new(x, area.bottom() - 8.0),
                    Pos2::new(x, area.bottom()),
                ],
                Stroke::new(2.0, *color),
            );
        }
    }
    // The supply fault current.
    if let Some(ik) = ik_a.filter(|k| *k > lo_a && *k < hi_a) {
        let x = to_x(ik);
        let mut y = area.top();
        while y < area.bottom() {
            plot_painter.line_segment(
                [Pos2::new(x, y), Pos2::new(x, (y + 5.0).min(area.bottom()))],
                Stroke::new(1.2, tokens::ERROR),
            );
            y += 10.0;
        }
        plot_painter.text(
            Pos2::new(x - 4.0, area.top() + 4.0),
            egui::Align2::RIGHT_TOP,
            format!("Ik {}", fmt_current(ik)),
            font.clone(),
            tokens::ERROR,
        );
    }
    painter.rect_stroke(
        area,
        0.0,
        Stroke::new(1.0, tokens::BORDER_STRONG),
        egui::StrokeKind::Inside,
    );
    if series.is_empty() {
        painter.text(
            area.center(),
            egui::Align2::CENTER_CENTER,
            "Tick a device to see its curve",
            egui::FontId::proportional(tokens::FONT_BODY),
            tokens::TEXT_MUTED,
        );
    }
    // Crosshair and the times at the pointer's current.
    if let Some(pos) = response.hover_pos().filter(|p| area.contains(*p)) {
        painter.line_segment(
            [
                Pos2::new(pos.x, area.top()),
                Pos2::new(pos.x, area.bottom()),
            ],
            Stroke::new(1.0, tokens::TEXT_FAINT),
        );
        let exp = x0 as f64 + (pos.x - area.left()) as f64 / area.width() as f64 * (x1 - x0) as f64;
        let i = 10f64.powf(exp);
        response.on_hover_ui_at_pointer(|ui| {
            ui.label(egui::RichText::new(fmt_current(i)).strong());
            for (d, color) in series {
                let text = match d.device.curve.trip_time_range(i) {
                    None => "does not trip".to_string(),
                    Some((lo, hi)) => format!("{} to {}", fmt_time(lo), fmt_time(hi)),
                };
                ui.horizontal(|ui| {
                    ui.colored_label(*color, &d.tag);
                    ui.label(text);
                });
            }
        });
    }
}

fn selectivity_tab(ui: &mut egui::Ui, state: &mut ProtectionState, study: &Study) {
    let Some(results) = &state.results else {
        return;
    };
    let report = &results.chain;
    match results.supply_ka {
        Some(k) => {
            ui.weak(format!(
                "The fault current at every device is taken as the supply's {} kA, an upper bound: cable impedance along the way lowers it.",
                trim(k)
            ));
        }
        None => {
            ui.colored_label(
                tokens::WARNING,
                "No fault current at the supply. Type one above or set supply.short_circuit_ka on the project; until then only the curves can be compared.",
            );
        }
    }
    ui.add_space(6.0);
    let mut show_pair: Option<(usize, usize)> = None;
    egui::ScrollArea::vertical()
        .id_salt("protection_selectivity")
        .show(ui, |ui| {
            if report.pairs.is_empty() {
                ui.label("No device sits behind another protective device.");
                ui.weak(
                    "Selectivity needs two protective devices in series: wire a breaker or fuse behind another (a contactor between them is fine).",
                );
            } else {
                egui::Grid::new("protection_pairs")
                    .num_columns(5)
                    .striped(true)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        for h in ["Upstream", "Downstream", "Selectivity", "Back-up", ""] {
                            ui.label(egui::RichText::new(h).color(tokens::TEXT_MUTED).size(tokens::FONT_SMALL));
                        }
                        ui.end_row();
                        for p in &report.pairs {
                            let (up, down) = (&study.devices[p.upstream], &study.devices[p.downstream]);
                            ui.label(&up.tag);
                            ui.label(&down.tag);
                            let s = &p.selectivity;
                            let basis = match s.basis {
                                Basis::Curves => "by curves",
                                Basis::CurvesAndEnergy => "by curves and let-through energy",
                                Basis::MakerTable => "by the maker's table",
                            };
                            match s.verdict {
                                Verdict::Total => {
                                    ui.colored_label(tokens::SUCCESS, format!("Total, {basis}"))
                                }
                                Verdict::Partial => ui.colored_label(
                                    tokens::WARNING,
                                    format!("Up to {}, {basis}", fmt_current(s.limit_a)),
                                ),
                                Verdict::None => {
                                    ui.colored_label(tokens::ERROR, format!("None, {basis}"))
                                }
                            };
                            match &p.backup {
                                Backup::NotNeeded => ui.weak("not needed"),
                                Backup::Covered(b) => ui.colored_label(
                                    tokens::INFO,
                                    match b {
                                        BackupBasis::MakerTable => "covered (maker's table)",
                                        BackupBasis::LetThrough => "covered (let-through)",
                                    },
                                ),
                                Backup::NotCovered(why) => {
                                    ui.colored_label(tokens::ERROR, "not covered").on_hover_text(why)
                                }
                            };
                            if ui.small_button("Show curves").clicked() {
                                show_pair = Some((p.upstream, p.downstream));
                            }
                            ui.end_row();
                        }
                    });
            }
            ui.add_space(10.0);
            novo_theme::widgets::section_label(ui, "FINDINGS");
            if report.findings.is_empty() {
                ui.weak("Nothing to report.");
            }
            for f in &report.findings {
                let (glyph, color) = match f.severity {
                    Severity::Error => ("●", tokens::ERROR),
                    Severity::Warning => ("●", tokens::WARNING),
                    Severity::Info => ("●", tokens::INFO),
                };
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(color, glyph);
                    ui.label(&f.message);
                });
            }
            ui.add_space(10.0);
            assumptions(ui, study);
        });
    if let Some((a, b)) = show_pair {
        state.shown = [study.devices[a].id, study.devices[b].id]
            .into_iter()
            .collect();
        state.tab = Tab::Curves;
    }
}

fn assumptions(ui: &mut egui::Ui, study: &Study) {
    let any = study.devices.iter().any(|d| !d.assumed.is_empty());
    if !any {
        return;
    }
    egui::CollapsingHeader::new("What this assumes")
        .default_open(false)
        .show(ui, |ui| {
            for d in study.devices.iter().filter(|d| !d.assumed.is_empty()) {
                ui.label(egui::RichText::new(&d.tag).strong());
                for a in &d.assumed {
                    ui.weak(format!("  {a}"));
                }
            }
        });
}

fn rating_tab(ui: &mut egui::Ui, state: &mut ProtectionState, _study: &Study) {
    let Some(results) = &state.results else {
        return;
    };
    let r = &results.rating;
    ui.horizontal(|ui| {
        let text = if r.panel_sccr_ka.is_finite() {
            format!("{} kA", trim(r.panel_sccr_ka))
        } else {
            "not limited".to_string()
        };
        ui.label(egui::RichText::new(text).size(22.0).color(tokens::TEXT));
        ui.add_space(10.0);
        match (r.verdict, results.supply_ka) {
            (sccr::Verdict::Sufficient, Some(k)) => ui.colored_label(
                tokens::SUCCESS,
                format!("at least the {} kA available", trim(k)),
            ),
            (sccr::Verdict::Insufficient, Some(k)) => {
                ui.colored_label(tokens::ERROR, format!("below the {} kA available", trim(k)))
            }
            _ => ui.weak("no fault current at the supply to compare with"),
        };
    });
    ui.weak(r.marking());
    ui.add_space(6.0);
    for note in &r.notes {
        ui.label(note);
    }
    ui.add_space(8.0);
    novo_theme::widgets::section_label(ui, "COMPONENTS, WEAKEST FIRST");
    egui::ScrollArea::vertical()
        .id_salt("protection_rating")
        .show(ui, |ui| {
            egui::Grid::new("protection_rating_rows")
                .num_columns(4)
                .striped(true)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for h in ["Component", "Where", "Effective", "Basis"] {
                        ui.label(egui::RichText::new(h).color(tokens::TEXT_MUTED).size(tokens::FONT_SMALL));
                    }
                    ui.end_row();
                    for row in &r.rows {
                        ui.label(&row.name);
                        ui.weak(&row.path);
                        if row.effective_ka.is_finite() {
                            ui.label(format!("{} kA", trim(row.effective_ka)));
                        } else {
                            ui.weak("never limits");
                        }
                        ui.weak(match row.source {
                            SccrSource::Marked => "marked on the device".to_string(),
                            SccrSource::Default => format!(
                                "no rating marked: default {} kA for its kind",
                                trim(row.own_ka)
                            ),
                            SccrSource::Combination => "tested with the protector ahead".to_string(),
                            SccrSource::CurrentLimiting => {
                                "held up by the current-limiting protector ahead".to_string()
                            }
                        });
                        ui.end_row();
                    }
                });
            ui.add_space(8.0);
            ui.weak(
                "Marked ratings come from the property sccr_ka on the device or its part. Defaults are the UL 508A table for unmarked parts, planning values to check against the edition in force. Terminals, busbars and the enclosure are not read from the drawing yet.",
            );
        });
}

fn trim(x: f64) -> String {
    let t = format!("{x:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}
