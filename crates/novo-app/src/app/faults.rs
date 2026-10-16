//! The Fault scenarios window (`sim.faults`, Simulate menu or the palette):
//! pick a scenario, run it, and read what the protection did. It shows the
//! timeline in words, one line per fault with the clearing device and whether
//! the selectivity held, and a timing diagram of fault, breakers and currents
//! that can be zoomed from the whole run down to milliseconds around the
//! fault, where the asymmetrical short-circuit current and the arc
//! interruption at a current zero are visible.
//!
//! The study (network, device curves, scenarios) is the file
//! `<project>.faults.json` next to the project file; "Create example study"
//! writes one to start from. Exports (SVG, CSV, VCD) go next to it. All
//! numbers are calculation values from the curves in the study.

use std::path::PathBuf;

use eframe::egui;
use novo_sim::protection::Band;
use novo_sim::scenario::{EventKind, ScenarioResult};
use novo_sim::study::Study;
use novo_sim::timing::{Level as Signal, TimingDiagram, format_time};
use novo_theme::tokens;

use super::{Level, NovoApp};

/// Which band of the curves the run uses.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BandChoice {
    AsStudied,
    Fastest,
    Typical,
    Slowest,
}

impl BandChoice {
    fn label(self) -> &'static str {
        match self {
            BandChoice::AsStudied => "As in the study",
            BandChoice::Fastest => "Fastest trip",
            BandChoice::Typical => "Typical trip",
            BandChoice::Slowest => "Slowest trip",
        }
    }
}

pub(crate) struct FaultsWindow {
    study: Study,
    /// Where the study is saved; `None` for a project without a file yet.
    path: Option<PathBuf>,
    selected: usize,
    band: BandChoice,
    result: Option<Result<ScenarioResult, String>>,
    /// Time window of the diagram in seconds, `None` for the whole run.
    view: Option<(f64, f64)>,
    /// A run is needed (scenario, band or study changed).
    stale: bool,
    /// The built-in example study is showing: no `<project>.faults.json`
    /// exists, and the numbers are not from the drawing.
    example: bool,
}

impl NovoApp {
    pub(super) fn toggle_faults(&mut self) {
        if self.wb.faults.take().is_some() {
            return;
        }
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let path = file.map(|f| f.with_extension("faults.json"));
        let example = !path.as_deref().is_some_and(|p| p.exists());
        let study = match path.as_deref().filter(|p| p.exists()) {
            Some(p) => match std::fs::read_to_string(p)
                .map_err(|e| e.to_string())
                .and_then(|t| Study::from_json(&t))
            {
                Ok(s) => s,
                Err(why) => {
                    self.message(Level::Error, format!("Could not read the study: {why}"));
                    return;
                }
            },
            None => Study::example(),
        };
        self.wb.faults = Some(FaultsWindow {
            study,
            path,
            selected: 0,
            band: BandChoice::AsStudied,
            result: None,
            view: None,
            stale: true,
            example,
        });
    }

    pub(super) fn faults_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.faults.take() else {
            return;
        };
        if win.stale {
            win.result = Some(run_selected(&win));
            win.view = None;
            win.stale = false;
        }
        let mut open = true;
        let mut note: Option<(Level, String)> = None;
        super::panel::dialog(ctx, "Fault scenarios", [760.0, 640.0])
            .open(&mut open)
            .show(ctx, |ui| {
                header(ui, &mut win, &mut note);
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| match &win.result {
                    Some(Ok(result)) => {
                        let result = result.clone();
                        body(ui, &mut win, &result);
                    }
                    Some(Err(why)) => {
                        ui.colored_label(crate::theme::ui_palette().error, why);
                    }
                    None => {}
                });
            });
        if let Some((level, text)) = note {
            self.message(level, text);
        }
        if open {
            self.wb.faults = Some(win);
        }
    }
}

fn run_selected(win: &FaultsWindow) -> Result<ScenarioResult, String> {
    let mut study = win.study.clone();
    let band = match win.band {
        BandChoice::AsStudied => None,
        BandChoice::Fastest => Some(Band::Fastest),
        BandChoice::Typical => Some(Band::Typical),
        BandChoice::Slowest => Some(Band::Slowest),
    };
    if let Some(band) = band {
        for d in &mut study.devices {
            d.band = band;
        }
    }
    study.run(win.selected)
}

fn header(ui: &mut egui::Ui, win: &mut FaultsWindow, note: &mut Option<(Level, String)>) {
    ui.horizontal(|ui| {
        ui.label("Scenario");
        let name = win
            .study
            .scenarios
            .get(win.selected)
            .map_or("none", |s| s.name.as_str())
            .to_owned();
        egui::ComboBox::from_id_salt("fault_scenario")
            .selected_text(name)
            .width(240.0)
            .show_ui(ui, |ui| {
                for (i, s) in win.study.scenarios.iter().enumerate() {
                    if ui.selectable_value(&mut win.selected, i, &s.name).changed() {
                        win.stale = true;
                    }
                }
            });
        ui.label("Trip");
        egui::ComboBox::from_id_salt("fault_band")
            .selected_text(win.band.label())
            .show_ui(ui, |ui| {
                for b in [
                    BandChoice::AsStudied,
                    BandChoice::Fastest,
                    BandChoice::Typical,
                    BandChoice::Slowest,
                ] {
                    if ui.selectable_value(&mut win.band, b, b.label()).changed() {
                        win.stale = true;
                    }
                }
            })
            .response
            .on_hover_text(
                "Every curve is a band. Fastest asks whether a start trips something it must \
                 not; slowest whether the cable is still protected.",
            );
    });
    if win.example {
        ui.colored_label(
            crate::theme::ui_palette().warning,
            "Example study: a built-in pump feeder, not your drawing. \
             Save study writes it next to the project to edit by hand.",
        );
    }
    if let Some(s) = win.study.scenarios.get(win.selected)
        && !s.description.is_empty()
    {
        ui.weak(&s.description);
    }
    ui.horizontal(|ui| {
        let can_save = win.path.is_some();
        if ui
            .add_enabled(can_save, egui::Button::new("Save study"))
            .on_disabled_hover_text("Save the project first; the study is saved next to it")
            .on_hover_text("Writes the study file next to the project")
            .clicked()
            && let Some(path) = &win.path
        {
            *note = Some(match std::fs::write(path, win.study.to_json()) {
                Ok(()) => {
                    win.example = false;
                    (Level::Info, format!("Saved {}", path.display()))
                }
                Err(e) => (Level::Error, format!("Could not save the study: {e}")),
            });
        }
        let exportable = matches!(win.result, Some(Ok(_))) && win.path.is_some();
        for (label, ext) in [
            ("Export SVG", "svg"),
            ("Export CSV", "csv"),
            ("Export VCD", "vcd"),
        ] {
            if ui
                .add_enabled(exportable, egui::Button::new(label))
                .on_disabled_hover_text("Save the project first; exports go next to it")
                .clicked()
                && let (Some(Ok(result)), Some(path)) = (&win.result, &win.path)
            {
                *note = Some(export(win, result, path, ext));
            }
        }
    });
}

fn export(
    win: &FaultsWindow,
    result: &ScenarioResult,
    study_path: &std::path::Path,
    ext: &str,
) -> (Level, String) {
    let stem = study_path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".faults.json"))
        .unwrap_or("study");
    let scenario = win
        .study
        .scenarios
        .get(win.selected)
        .map_or("scenario", |s| s.name.as_str())
        .replace(['/', '\\', ':'], "-");
    let path = study_path.with_file_name(format!("{stem} {scenario}.{ext}"));
    let text = match ext {
        "svg" => result.diagram().to_svg(1100.0, win.view),
        "csv" => result.traces.to_csv(),
        _ => result.timing.to_vcd(),
    };
    match std::fs::write(&path, text) {
        Ok(()) => (Level::Info, format!("Exported {}", path.display())),
        Err(e) => (Level::Error, format!("Could not export: {e}")),
    }
}

fn body(ui: &mut egui::Ui, win: &mut FaultsWindow, r: &ScenarioResult) {
    for w in &r.warnings {
        ui.colored_label(crate::theme::ui_palette().warning, w);
    }
    for f in &r.faults {
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!("Fault {} at {}", f.fault, f.node));
            ui.label(format!("Ik'' {:.0} A · peak {:.0} A", f.ik, f.peak));
            match (f.cleared_at, f.cleared_by.as_deref()) {
                (Some(t), Some(by)) => {
                    ui.label(format!(
                        "cleared at {} by {}",
                        format_time(t),
                        label(&win.study, by)
                    ));
                }
                (Some(t), None) => {
                    ui.label(format!("removed at {}", format_time(t)));
                }
                _ if f.ik > 0.0 => {
                    ui.colored_label(crate::theme::ui_palette().error, "never cleared");
                }
                _ => {}
            }
            if let Some(i2t) = f.let_through_i2t {
                ui.label(format!("let-through {:.2e} A²s", i2t));
            }
        });
        if f.ik > 0.0 {
            let (text, color) = match (&f.expected, f.selective, f.cleared_at) {
                (_, _, None) => (
                    "Not cleared: no device reached its trip curve",
                    crate::theme::ui_palette().error,
                ),
                (Some(_), true, _) => (
                    "Selective: only the device nearest the fault tripped",
                    crate::theme::ui_palette().success,
                ),
                (Some(e), false, _) => {
                    let by = f.cleared_by.as_deref().unwrap_or("?");
                    let _ = e;
                    let _ = by;
                    (
                        "Not selective: more than the nearest device tripped, or a different one",
                        crate::theme::ui_palette().warning,
                    )
                }
                (None, _, _) => (
                    "No protective device upstream of this fault",
                    crate::theme::ui_palette().error,
                ),
            };
            ui.colored_label(color, text);
        }
    }
    if !r.faults.is_empty() {
        ui.add_space(4.0);
    }
    ui.strong("What happened");
    for e in &r.events {
        let color = match e.kind {
            EventKind::TripShortCircuit | EventKind::TripOverload => {
                crate::theme::ui_palette().warning
            }
            EventKind::FaultStart => crate::theme::ui_palette().error,
            EventKind::FaultCleared | EventKind::Closed => crate::theme::ui_palette().success,
            _ => crate::theme::ui_palette().text_primary,
        };
        ui.horizontal(|ui| {
            ui.monospace(format!("{:>10}", format_time(e.t)));
            ui.colored_label(color, &e.text);
        });
    }
    if r.events.is_empty() {
        ui.weak("Nothing happens in this scenario: no device reached its curve.");
    }
    ui.add_space(6.0);
    ui.strong("Timing");
    ui.horizontal(|ui| {
        if ui.button("Whole run").clicked() {
            win.view = None;
        }
        if let Some(f) = r.faults.iter().find(|f| f.ik > 0.0)
            && ui
                .button("Around the first fault")
                .on_hover_text("Zooms to the first cycles: the DC offset and the interruption at a current zero")
                .clicked()
        {
            let end = f.cleared_at.unwrap_or(f.t_start + 0.1);
            win.view = Some((f.t_start - 0.005, end + 0.02));
        }
        let (mut a, mut b) = win.view.unwrap_or((0.0, r.duration));
        let mut changed = false;
        ui.label("from");
        changed |= ui
            .add(egui::DragValue::new(&mut a).speed(0.001).suffix(" s").range(0.0..=r.duration))
            .changed();
        ui.label("to");
        changed |= ui
            .add(egui::DragValue::new(&mut b).speed(0.001).suffix(" s").range(0.0..=r.duration))
            .changed();
        if changed && b > a {
            win.view = Some((a, b));
        }
    });
    draw_timing(ui, &r.diagram(), win.view);
}

fn label(study: &Study, key: &str) -> String {
    study
        .devices
        .iter()
        .find(|d| d.key == key)
        .map_or_else(|| key.to_owned(), |d| d.label.clone())
}

/// The diagram with the egui painter: the same layout as the SVG export.
fn draw_timing(ui: &mut egui::Ui, d: &TimingDiagram, window: Option<(f64, f64)>) {
    let (t0, t1) = window.unwrap_or((0.0, d.duration));
    let t1 = if t1 > t0 { t1 } else { t0 + 1.0 };
    let (label_w, row_h, gap, head_h, analog_h) =
        (170.0_f32, 20.0_f32, 6.0_f32, 16.0_f32, 52.0_f32);
    let mut groups: Vec<(&str, Vec<&novo_sim::timing::DigitalChannel>)> = Vec::new();
    for c in &d.channels {
        match groups.iter_mut().find(|g| g.0 == c.group) {
            Some(g) => g.1.push(c),
            None => groups.push((&c.group, vec![c])),
        }
    }
    let rows: usize = groups.iter().map(|g| g.1.len()).sum();
    let height = 22.0
        + groups.len() as f32 * head_h
        + rows as f32 * (row_h + gap)
        + d.analog.len() as f32 * (analog_h + gap);
    let width = ui.available_width().max(400.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, crate::theme::ui_palette().panel);
    let plot_left = rect.left() + label_w;
    let plot_w = (rect.width() - label_w - 10.0).max(50.0);
    let x = |t: f64| plot_left + ((t - t0) / (t1 - t0)) as f32 * plot_w;
    let font = egui::FontId::proportional(tokens::FONT_SMALL);
    // Time axis.
    let step = nice_step((t1 - t0) / 8.0);
    let mut tick = (t0 / step).ceil() * step;
    while tick <= t1 + step * 1e-9 {
        let px = x(tick);
        painter.line_segment(
            [
                egui::pos2(px, rect.top() + 18.0),
                egui::pos2(px, rect.bottom()),
            ],
            egui::Stroke::new(1.0, crate::theme::ui_palette().border_subtle),
        );
        painter.text(
            egui::pos2(px, rect.top() + 3.0),
            egui::Align2::CENTER_TOP,
            format_time(tick),
            font.clone(),
            crate::theme::ui_palette().text_secondary,
        );
        tick += step;
    }
    let mut y = rect.top() + 22.0;
    for (group, channels) in &groups {
        painter.text(
            egui::pos2(rect.left() + 6.0, y),
            egui::Align2::LEFT_TOP,
            *group,
            font.clone(),
            crate::theme::ui_palette().text_tertiary,
        );
        y += head_h;
        for c in channels {
            painter.text(
                egui::pos2(plot_left - 8.0, y + row_h * 0.5),
                egui::Align2::RIGHT_CENTER,
                &c.name,
                font.clone(),
                crate::theme::ui_palette().text_primary,
            );
            let (hi, lo) = (y + 3.0, y + row_h - 3.0);
            let mut prev: Option<f32> = None;
            let mut line: Vec<egui::Pos2> = Vec::new();
            let alarmed = c.edges.iter().any(|e| e.1 == Signal::Alarm);
            for (a, b, level) in c.segments(d.duration) {
                if b <= t0 || a >= t1 {
                    continue;
                }
                let (a, b) = (a.max(t0), b.min(t1));
                let ly = if level == Signal::Low { lo } else { hi };
                if let Some(py) = prev
                    && py != ly
                {
                    line.push(egui::pos2(x(a), py));
                }
                line.push(egui::pos2(x(a), ly));
                line.push(egui::pos2(x(b), ly));
                prev = Some(ly);
                if level == Signal::Alarm {
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x(a), hi),
                            egui::pos2(x(b).max(x(a) + 1.0), lo),
                        ),
                        0.0,
                        crate::theme::ui_palette().error.gamma_multiply(0.25),
                    );
                }
            }
            painter.add(egui::Shape::line(
                line,
                egui::Stroke::new(
                    1.5,
                    if alarmed {
                        crate::theme::ui_palette().error
                    } else {
                        crate::theme::ui_palette().success
                    },
                ),
            ));
            y += row_h + gap;
        }
    }
    for tr in &d.analog {
        let mut w = novo_sim::trace::Trace::new(tr.name.clone(), tr.unit.clone());
        if let Some(v) = tr.value_at(t0) {
            w.push(t0, v);
        }
        for (&t, &v) in tr.t.iter().zip(&tr.v) {
            if t > t0 && t < t1 {
                w.push(t, v);
            }
        }
        if let Some(v) = tr.value_at(t1) {
            w.push(t1, v);
        }
        let w = w.decimate((plot_w as usize).max(16) * 2);
        let (mut lo_v, mut hi_v) = (w.min().unwrap_or(0.0), w.max().unwrap_or(1.0));
        if hi_v - lo_v < 1e-12 {
            lo_v -= 1.0;
            hi_v += 1.0;
        }
        painter.text(
            egui::pos2(plot_left - 8.0, y + 2.0),
            egui::Align2::RIGHT_TOP,
            &tr.name,
            font.clone(),
            crate::theme::ui_palette().text_primary,
        );
        painter.text(
            egui::pos2(plot_left - 8.0, y + 16.0),
            egui::Align2::RIGHT_TOP,
            format!("{hi_v:.4} {}", tr.unit),
            font.clone(),
            crate::theme::ui_palette().text_tertiary,
        );
        let plot =
            egui::Rect::from_min_size(egui::pos2(plot_left, y), egui::vec2(plot_w, analog_h));
        painter.rect_stroke(
            plot,
            0.0,
            egui::Stroke::new(1.0, crate::theme::ui_palette().border_subtle),
            egui::StrokeKind::Inside,
        );
        let pts: Vec<egui::Pos2> =
            w.t.iter()
                .zip(&w.v)
                .map(|(&t, &v)| {
                    egui::pos2(
                        x(t),
                        y + analog_h - ((v - lo_v) / (hi_v - lo_v)) as f32 * analog_h,
                    )
                })
                .collect();
        painter.add(egui::Shape::line(
            pts,
            egui::Stroke::new(1.2, crate::theme::ui_palette().info),
        ));
        y += analog_h + gap;
    }
}

fn nice_step(raw: f64) -> f64 {
    let raw = raw.max(1e-9);
    let mag = 10f64.powf(raw.log10().floor());
    let f = raw / mag;
    mag * if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    }
}
