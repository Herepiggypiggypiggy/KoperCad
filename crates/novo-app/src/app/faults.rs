//! The Fault scenarios window (`sim.faults`, Simulate menu or the palette):
//! pick a scenario, run it, and read what the protection did. It shows the
//! timeline in words, one line per fault with the clearing device and whether
//! the selectivity held, and a timing diagram of fault, breakers and currents
//! that can be zoomed from the whole run down to milliseconds around the
//! fault, where the asymmetrical short-circuit current and the arc
//! interruption at a current zero are visible.
//!
//! The study (network, device curves, scenarios) is the file
//! `<project>.faults.json` next to the project file. Faults are added from
//! the drawing: right-click a wire, device or terminal (or a selection) >
//! Add fault, or Simulate > Add fault to selection (`sim.add_fault`). They
//! are stored in that same file (a `faults` list; no change to the project
//! file format) and listed in this window, each run as a scenario of its
//! own. The built-in Example study shows only when the project has no
//! study file. Exports (SVG, CSV, VCD) go next to it. All numbers are
//! calculation values from the curves in the study.

use std::path::PathBuf;

use eframe::egui;
use novo_core::{AnyId, Netlist, Project, SymbolRole};
use novo_sim::fault::FaultKind;
use novo_sim::protection::Band;
use novo_sim::scenario::{EventKind, ScenarioResult};
use novo_sim::study::{FaultWhat, Prepared, Study};
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
    /// The study as stored in the file.
    study: Study,
    /// The study with the project's faults as scenarios; what runs.
    prepared: Prepared,
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

#[cfg(test)]
impl FaultsWindow {
    pub(super) fn is_example(&self) -> bool {
        self.example
    }

    pub(super) fn fault_count(&self) -> usize {
        self.study.faults.len()
    }

    pub(super) fn selected_name(&self) -> String {
        self.prepared
            .study
            .scenarios
            .get(self.selected)
            .map_or_else(String::new, |s| s.name.clone())
    }
}

#[cfg(test)]
impl AddFault {
    pub(super) fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl NovoApp {
    pub(super) fn toggle_faults(&mut self) {
        if self.wb.faults.take().is_some() {
            return;
        }
        self.open_faults(None);
    }

    /// Opens the window (again), on the scenario of fault `select` if given.
    fn open_faults(&mut self, select: Option<&str>) {
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
        let prepared = study.prepared();
        let selected = select
            .and_then(|id| {
                prepared
                    .study
                    .scenarios
                    .iter()
                    .position(|s| s.name.starts_with(&format!("{id}:")))
            })
            .unwrap_or(0);
        self.wb.faults = Some(FaultsWindow {
            study,
            prepared,
            path,
            selected,
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
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                header(ui, &mut win, &mut note);
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| match &win.result {
                    Some(Ok(result)) => {
                        let result = result.clone();
                        body(ui, &mut win, &result);
                    }
                    Some(Err(why)) => {
                        novo_theme::widgets::banner(
                            ui,
                            novo_theme::widgets::Level::Error,
                            why,
                            |_| {},
                        );
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
    let mut study = win.prepared.study.clone();
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
            .prepared
            .study
            .scenarios
            .get(win.selected)
            .map_or("none", |s| s.name.as_str())
            .to_owned();
        egui::ComboBox::from_id_salt("fault_scenario")
            .selected_text(name)
            .width(240.0)
            .show_ui(ui, |ui| {
                for (i, s) in win.prepared.study.scenarios.iter().enumerate() {
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
    ui.weak(
        "How it works: a fault is a scenario. The program applies it to the network of the \
         study, finds the current through each protective device, and plays out which device \
         trips and when. Add faults by right-clicking a wire, device or terminal in the \
         drawing > Add fault.",
    );
    if win.example {
        novo_theme::widgets::banner(
            ui,
            novo_theme::widgets::Level::Warning,
            "Example study: a built-in pump feeder, not your drawing. It shows until you add a \
             fault of your own (right-click in the drawing > Add fault). Save study writes it \
             next to the project to edit by hand.",
            |_| {},
        );
    }
    for w in &win.prepared.warnings {
        ui.colored_label(crate::theme::ui_palette().warning, w);
    }
    project_faults(ui, win, note);
    if win.prepared.study.scenarios.is_empty() {
        ui.weak("This study has no scenarios yet.");
    }
    if let Some(s) = win.prepared.study.scenarios.get(win.selected)
        && !s.description.is_empty()
    {
        ui.weak(&s.description);
    }
    ui.horizontal(|ui| {
        let can_save = win.path.is_some();
        if !can_save {
            ui.weak("Save the project first: the study and exports go next to it.");
        }
        if ui
            .add_enabled_ui(can_save, |ui| {
                novo_theme::widgets::primary_button(ui, "Save study")
            })
            .inner
            .on_disabled_hover_text("Save the project first; the study is saved next to it")
            .on_hover_text("Writes the study file next to the project")
            .clicked()
            && let Some(path) = &win.path
        {
            *note = Some(
                match super::pdf::write_replacing(path, win.study.to_json().as_bytes()) {
                    Ok(()) => {
                        win.example = false;
                        (Level::Info, format!("Saved {}", path.display()))
                    }
                    Err(e) => (Level::Error, format!("Could not save the study: {e}")),
                },
            );
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

/// The faults added from the drawing, each with Run and Remove.
fn project_faults(ui: &mut egui::Ui, win: &mut FaultsWindow, note: &mut Option<(Level, String)>) {
    if win.study.faults.is_empty() {
        return;
    }
    ui.add_space(2.0);
    ui.strong(format!(
        "Faults of this project ({})",
        win.study.faults.len()
    ));
    let mut run: Option<String> = None;
    let mut remove: Option<String> = None;
    for f in &win.study.faults {
        ui.horizontal(|ui| {
            ui.monospace(&f.id);
            ui.label(format!("{} at {}", f.what.label(), f.target));
            if ui.small_button("Run").clicked() {
                run = Some(f.id.clone());
            }
            if ui
                .small_button("Remove")
                .on_hover_text("Takes the fault out of the study file")
                .clicked()
            {
                remove = Some(f.id.clone());
            }
        });
    }
    if let Some(id) = run
        && let Some(i) = win
            .prepared
            .study
            .scenarios
            .iter()
            .position(|s| s.name.starts_with(&format!("{id}:")))
    {
        win.selected = i;
        win.stale = true;
    }
    if let Some(id) = remove {
        win.study.remove_fault(&id);
        win.prepared = win.study.prepared();
        win.selected = 0;
        win.stale = true;
        if let Some(path) = &win.path {
            *note = Some(
                match super::pdf::write_replacing(path, win.study.to_json().as_bytes()) {
                    Ok(()) => (Level::Info, format!("Removed fault {id}")),
                    Err(e) => (Level::Error, format!("Could not save the study: {e}")),
                },
            );
        }
    }
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
        .prepared
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
    match super::pdf::write_replacing(&path, text.as_bytes()) {
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

// ---- Add fault: from the drawing --------------------------------------

/// The names a selection offers as the place of a fault: device tags,
/// terminals as `-X1:3`, wires by the potential or number of their net.
pub(super) fn fault_targets(
    project: &Project,
    netlist: &Netlist,
    selection: &[AnyId],
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |name: String| {
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    };
    for &id in selection {
        match id {
            AnyId::Symbol(s) => {
                let Some(symbol) = project.symbol(s) else {
                    continue;
                };
                match &symbol.role {
                    SymbolRole::Device { device, .. } => {
                        if let Some(d) = project.device(*device) {
                            add(d.tag.to_string());
                        }
                    }
                    SymbolRole::Terminal { terminals } => {
                        for t in terminals.iter().filter_map(|&t| project.terminal(t)) {
                            let strip = project
                                .device(t.strip)
                                .map_or_else(String::new, |d| d.tag.to_string());
                            add(format!("{strip}:{}", t.name));
                        }
                    }
                    _ => {}
                }
            }
            AnyId::Wire(w) => {
                let Some(wire) = project.wire(w) else {
                    continue;
                };
                let net = netlist.net_of_wire(wire);
                let potential = net.and_then(|n| {
                    netlist
                        .net(n)
                        .potentials
                        .iter()
                        .find_map(|&p| project.potential(p).map(|p| p.name.clone()))
                });
                add(potential
                    .or_else(|| wire.number.clone())
                    .unwrap_or_else(|| "Wire without number".to_owned()));
            }
            _ => {}
        }
    }
    out
}

/// What the Add fault window can set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum FaultChoice {
    Short(FaultKind),
    DeviceOpens,
    DeviceCloses,
}

impl FaultChoice {
    pub(super) const ALL: [FaultChoice; 7] = [
        FaultChoice::Short(FaultKind::ThreePhase),
        FaultChoice::Short(FaultKind::PhasePhase),
        FaultChoice::Short(FaultKind::PhaseNeutral),
        FaultChoice::Short(FaultKind::PhaseEarth),
        FaultChoice::Short(FaultKind::OpenPhase),
        FaultChoice::DeviceOpens,
        FaultChoice::DeviceCloses,
    ];

    pub(super) fn what(self) -> FaultWhat {
        match self {
            FaultChoice::Short(kind) => FaultWhat::Short { kind },
            FaultChoice::DeviceOpens => FaultWhat::DeviceOpens,
            FaultChoice::DeviceCloses => FaultWhat::DeviceCloses,
        }
    }

    fn label(self) -> String {
        match self {
            FaultChoice::Short(FaultKind::PhaseEarth) => "Short to earth (phase to PE)".into(),
            FaultChoice::Short(FaultKind::PhasePhase) => {
                "Short to another potential (phase to phase)".into()
            }
            FaultChoice::Short(FaultKind::PhaseNeutral) => "Short to neutral".into(),
            FaultChoice::Short(FaultKind::OpenPhase) => "Open circuit (one phase opens)".into(),
            FaultChoice::DeviceOpens => "Contact stuck open (device drops out)".into(),
            FaultChoice::DeviceCloses => "Contact stuck closed (device closes by itself)".into(),
            other => other.what().label(),
        }
    }
}

/// The Add fault window, while it is open.
pub(crate) struct AddFault {
    targets: Vec<String>,
    target: usize,
    choice: FaultChoice,
    fault_ohm: f64,
    at_s: f64,
}

/// Adds a fault to the study file at `path` (made if missing). Returns the
/// new fault's id.
pub(super) fn store_fault(
    path: &std::path::Path,
    name: &str,
    target: &str,
    what: FaultWhat,
    fault_ohm: f64,
    at_s: f64,
) -> Result<String, String> {
    let mut study = if path.exists() {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Study::from_json(&text)?
    } else {
        Study::empty(name)
    };
    let id = study.add_fault(target, what, fault_ohm, at_s)?;
    super::pdf::write_replacing(path, study.to_json().as_bytes()).map_err(|e| e.to_string())?;
    Ok(id)
}

impl NovoApp {
    /// `sim.add_fault`: opens the window that adds a fault at the selection.
    pub(super) fn begin_add_fault(&mut self) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let targets = fault_targets(project, &project.netlist(), &self.selection());
        if targets.is_empty() {
            self.message(
                Level::Info,
                "Select a wire, a device or a terminal first, then Add fault",
            );
            return;
        }
        self.wb.add_fault = Some(AddFault {
            targets,
            target: 0,
            choice: FaultChoice::Short(FaultKind::ThreePhase),
            fault_ohm: 0.0,
            at_s: 0.1,
        });
    }

    pub(super) fn add_fault_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.add_fault.take() else {
            return;
        };
        let mut open = true;
        let mut add = false;
        super::panel::dialog(ctx, "Add fault", [380.0, 0.0])
            .id(super::panel::keep_id(ctx, egui::Id::new("add_fault")))
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.weak(
                    "Adds a fault to this project's fault list; Simulate > Fault scenarios \
                     runs it.",
                );
                egui::ComboBox::from_label("Where")
                    .selected_text(win.targets[win.target].clone())
                    .show_ui(ui, |ui| {
                        for (i, t) in win.targets.iter().enumerate() {
                            ui.selectable_value(&mut win.target, i, t);
                        }
                    });
                egui::ComboBox::from_label("What")
                    .selected_text(win.choice.label())
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        for c in FaultChoice::ALL {
                            ui.selectable_value(&mut win.choice, c, c.label());
                        }
                    });
                if matches!(win.choice, FaultChoice::Short(k) if k != FaultKind::OpenPhase) {
                    ui.add(
                        egui::DragValue::new(&mut win.fault_ohm)
                            .range(0.0..=100.0)
                            .speed(0.001)
                            .suffix(" ohm in the fault itself"),
                    );
                }
                ui.add(
                    egui::DragValue::new(&mut win.at_s)
                        .range(0.0..=3600.0)
                        .speed(0.01)
                        .suffix(" s after the start"),
                );
                if ui
                    .add_enabled(
                        !win.targets[win.target].trim().is_empty(),
                        egui::Button::new("Add fault"),
                    )
                    .clicked()
                {
                    add = true;
                }
            });
        if add {
            let target = win.targets[win.target].clone();
            self.add_fault_stored(&target, win.choice.what(), win.fault_ohm, win.at_s);
            open = false;
        }
        if open {
            self.wb.add_fault = Some(win);
        }
    }

    /// Stores a fault next to the project and shows the window on it.
    pub(super) fn add_fault_stored(
        &mut self,
        target: &str,
        what: FaultWhat,
        fault_ohm: f64,
        at_s: f64,
    ) {
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let Some(path) = file.map(|f| f.with_extension("faults.json")) else {
            self.message(
                Level::Info,
                "Save the project first; faults are stored next to it",
            );
            return;
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".faults.json"))
            .unwrap_or("Project")
            .to_owned();
        match store_fault(&path, &name, target, what, fault_ohm, at_s) {
            Ok(id) => {
                self.message(
                    Level::Info,
                    format!("Added fault {id} at {target} (Simulate > Fault scenarios)"),
                );
                self.wb.faults = None;
                self.open_faults(Some(&id));
            }
            Err(e) => self.message(Level::Error, format!("Could not store the fault: {e}")),
        }
    }
}
