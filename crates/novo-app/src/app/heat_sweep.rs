//! "Cooling advice": tries where a pair of vents or fans goes and how big it
//! is, ranks the tries by the predicted device temperatures and shows the best
//! few with before and after numbers.
//!
//! Where the user finds it: Cabinet and 3D view, the Heat strip's "Cooling
//! advice" button, and the command palette ("Cooling advice and vent sweep").
//! The sweep runs on a worker thread in the 2D model
//! (`novo_thermal::sweep`); the window says that these are planning values,
//! not a validated result. "Add these openings" puts the pair in the Vents
//! and fans list of the 3D Heat panel, one undo step there, where a 3D run
//! can check it.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui::{self, Pos2};
use novo_core::{Project, StructureId};
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};
use novo_thermal::model::{Enclosure, Placement};
use novo_thermal::sweep::{
    Candidate, Control, Delivered, Outcome, Pairing, Piece, SIGNIFICANT_K, Settings, Sweep,
    sweep_with,
};

use super::heat_settings::{HeatSettings, Kind, VentSpec, Wall};

/// Width of the window, points.
const WIDTH: f32 = 400.0;
/// Tallest the list of options gets before it scrolls, points: the window
/// stays small enough to leave the drawing visible.
const MAX_BODY: f32 = 360.0;

/// What a finished sweep found and what it needs to be applied.
pub(super) struct Found {
    pub sweep: Sweep,
    tags: BTreeMap<u64, String>,
    enclosure: Enclosure,
    /// Openings the cabinet already had when the sweep started.
    existing: usize,
    key: u64,
}

struct Running {
    rx: mpsc::Receiver<Result<Sweep, String>>,
    control: Arc<Control>,
    started: Instant,
    tags: BTreeMap<u64, String>,
    enclosure: Enclosure,
    existing: usize,
    key: u64,
}

/// State of the sweep and its window.
#[derive(Default)]
pub(super) struct CoolingAdvice {
    requested: bool,
    run: Option<Running>,
    pub(super) found: Option<Found>,
    notice: Option<String>,
    open: bool,
    /// What the last "Add" put in the list, shown in the window.
    added: Option<String>,
}

/// What the window asks the app to do.
pub(super) enum Ask {
    /// Put this pair in the list of vents and fans.
    Add(Candidate),
}

/// Identity of the cabinet the sweep is for: changes with the project and
/// with the openings already set.
pub(super) fn key_of(
    project: &Project,
    structure: StructureId,
    ambient: f64,
    settings: &HeatSettings,
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (structure, project.revision(), ambient.to_bits()).hash(&mut h);
    format!("{:?}", settings.vents).hash(&mut h);
    h.finish()
}

#[cfg(test)]
impl Found {
    pub(super) fn for_test(sweep: Sweep, key: u64) -> Found {
        Found {
            sweep,
            tags: BTreeMap::from([(1, "-T1".to_owned())]),
            enclosure: novo_thermal::model::Enclosure {
                width_mm: 800.0,
                height_mm: 1200.0,
                depth_mm: 300.0,
                wall_mm: 2.0,
                material: novo_layout::sizing::Material::Steel,
                installation: novo_layout::sizing::Installation::Wall,
                plate: novo_layout::Rect {
                    x: 50.0,
                    y: 50.0,
                    w: 690.0,
                    h: 1090.0,
                },
            },
            existing: 0,
            key,
        }
    }
}

impl CoolingAdvice {
    #[cfg(test)]
    pub(super) fn set_found_for_test(&mut self, f: Found) {
        self.found = Some(f);
    }

    /// `heat.cooling_advice`: sweep on the next frame.
    pub(super) fn request(&mut self) {
        self.requested = true;
        self.open = true;
    }

    pub(super) fn take_request(&mut self) -> bool {
        std::mem::take(&mut self.requested)
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn running(&self) -> bool {
        self.run.is_some()
    }

    pub(super) fn close(&mut self) {
        if let Some(r) = &self.run {
            r.control.cancel.store(true, Ordering::Relaxed);
        }
        self.run = None;
        self.open = false;
        self.found = None;
        self.notice = None;
        self.added = None;
    }

    /// Says in the window what was added.
    pub(super) fn set_added(&mut self, text: String) {
        self.added = Some(text);
    }

    /// Starts the sweep from the cabinet as the project has it now, with the
    /// openings already in the Vents and fans list. Says why when it cannot.
    pub(super) fn start(
        &mut self,
        project: &Project,
        structure: StructureId,
        ambient: f64,
        settings: &HeatSettings,
        tag_of: &dyn Fn(u64) -> String,
    ) {
        if self.run.is_some() {
            return;
        }
        self.found = None;
        self.notice = None;
        self.added = None;
        let wires = super::heat_wires::wire_heat(project, structure);
        let options = BridgeOptions {
            ambient_c: ambient,
            duct_loss_w: wires.by_duct,
            ..Default::default()
        };
        let mut scenario = match scenario_from_project(project, structure, &options) {
            Ok(b) => b.scenario,
            Err(e) => {
                self.notice = Some(format!(
                    "Cannot search: {e}. Set it in the Cabinet: Size enclosure or Plate setup"
                ));
                return;
            }
        };
        settings.apply(&mut scenario);
        if scenario.devices.is_empty() {
            self.notice =
                Some("No device stands on the plate, so there is no heat to cool.".into());
            return;
        }
        let key = key_of(project, structure, ambient, settings);
        let tags = scenario
            .devices
            .iter()
            .map(|d| (d.id, tag_of(d.id)))
            .collect();
        let enclosure = scenario.enclosure.clone();
        let existing = scenario.vents.len() + scenario.fans.len();
        let control = Arc::new(Control::default());
        let (tx, rx) = mpsc::channel();
        let c = control.clone();
        std::thread::spawn(move || {
            let result = sweep_with(&scenario, &Settings::default(), &c).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.run = Some(Running {
            rx,
            control,
            started: Instant::now(),
            tags,
            enclosure,
            existing,
            key,
        });
    }

    /// Takes in a finished sweep. Called every frame.
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        let Some(run) = &self.run else {
            return;
        };
        match run.rx.try_recv() {
            Ok(result) => {
                let run = self.run.take().expect("checked above");
                match result {
                    Ok(sweep) => {
                        self.found = Some(Found {
                            sweep,
                            tags: run.tags,
                            enclosure: run.enclosure,
                            existing: run.existing,
                            key: run.key,
                        });
                    }
                    Err(e) => self.notice = Some(format!("The sweep failed: {e}")),
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.run = None;
                self.notice = Some("The sweep stopped unexpectedly".into());
            }
        }
    }

    /// The window. `now_key` is [`key_of`] for the cabinet as it is now.
    pub(super) fn show(&mut self, ctx: &egui::Context, now_key: u64, at: Pos2) -> Option<Ask> {
        if !self.open {
            return None;
        }
        let mut ask = None;
        let mut close = false;
        let mut open = true;
        let pal = crate::theme::ui_palette();
        egui::Window::new("Cooling advice")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_pos(at)
            .default_width(WIDTH)
            .show(ctx, |ui| {
                ui.set_max_width(WIDTH);
                if let Some(run) = &self.run {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(format!(
                            "Trying vent and fan places and sizes in the model… {} of {}, {:.0} s",
                            run.control.evaluated.load(Ordering::Relaxed),
                            run.control.planned.load(Ordering::Relaxed).max(1),
                            run.started.elapsed().as_secs_f64()
                        ));
                    });
                    if ui.button("Stop and keep the best so far").clicked() {
                        run.control.cancel.store(true, Ordering::Relaxed);
                    }
                    return;
                }
                if let Some(n) = &self.notice {
                    ui.colored_label(pal.warning, n);
                }
                if let Some(a) = &self.added {
                    ui.colored_label(pal.success, a);
                }
                let Some(found) = &self.found else {
                    return;
                };
                let stale = found.key != now_key;
                if stale {
                    ui.colored_label(
                        pal.warning,
                        "The cabinet changed after this sweep. Sweep again before adding.",
                    );
                }
                let room = (ctx.content_rect().bottom() - at.y - 170.0).clamp(160.0, MAX_BODY);
                egui::ScrollArea::vertical()
                    .max_height(room)
                    .auto_shrink([false, true])
                    .show(ui, |ui| body(ui, found, stale, &mut ask));
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "Model estimate: the 2D heat model with planning values and the losses you entered. Not validated against measurements. Check the winner with Simulate in 3D after adding it.",
                    )
                    .small()
                    .color(pal.text_secondary),
                );
                ui.add_space(4.0);
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        if !open || close {
            self.close();
        }
        ask
    }
}

fn body(ui: &mut egui::Ui, found: &Found, stale: bool, ask: &mut Option<Ask>) {
    let pal = crate::theme::ui_palette();
    let s = &found.sweep;
    ui.label(egui::RichText::new(headline(s)).strong());
    ui.add_space(2.0);
    ui.label(format!(
        "Now: {} · {} · hottest air {:.0} C",
        margin_text(s.before.worst_margin_k),
        over_text(s.before.over),
        s.before.max_air_c
    ));
    if found.existing > 0 {
        ui.label(
            egui::RichText::new(format!(
                "On top of the {} opening(s) already in the Vents and fans list.",
                found.existing
            ))
            .small()
            .color(pal.text_secondary),
        );
    }
    ui.label(
        egui::RichText::new(format!(
            "The sizing rule asks for about {:.0} m3/h of fan flow to carry all the loss.",
            s.sizing_flow_m3h
        ))
        .small()
        .color(pal.text_secondary),
    );
    for o in &s.best {
        ui.add_space(8.0);
        ui.separator();
        option(ui, found, o, stale, ask);
    }
    for n in &s.notes {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(n).small().color(pal.text_secondary));
    }
    if s.skipped > 0 {
        ui.label(
            egui::RichText::new(format!(
                "{} try(s) did not fit the cabinet and were left out.",
                s.skipped
            ))
            .small()
            .color(pal.text_secondary),
        );
    }
}

fn option(ui: &mut egui::Ui, found: &Found, o: &Outcome, stale: bool, ask: &mut Option<Ask>) {
    let pal = crate::theme::ui_palette();
    let before = &found.sweep.before;
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(format!("{}. {}", o.rank, o.candidate.pairing.label())).strong(),
        );
        if o.clear {
            ui.colored_label(pal.success, format!("wins {:.1} K", o.gain_k));
        } else {
            ui.colored_label(
                pal.warning,
                format!("{:+.1} K: within the model's noise", o.gain_k),
            );
        }
    });
    ui.label(o.candidate.describe(&found.enclosure));
    ui.label(format!(
        "Tightest margin {} → {}",
        margin_text(before.worst_margin_k),
        margin_text(o.reading.worst_margin_k)
    ));
    ui.label(format!(
        "{} → {} · hottest air {:.0} → {:.0} C",
        over_text(before.over),
        over_text(o.reading.over),
        before.max_air_c,
        o.reading.max_air_c
    ));
    if let (Some(i), Some(out)) = (&o.inlet, &o.outlet) {
        ui.label(
            egui::RichText::new(format!(
                "The model passes {} in and {} out.",
                flow_text(i),
                flow_text(out)
            ))
            .small()
            .color(pal.text_secondary),
        );
    }
    let changed: Vec<String> = o
        .devices
        .iter()
        .filter(|d| (d.surface_before_c - d.surface_after_c).abs() >= 0.5)
        .map(|d| {
            let tag = found
                .tags
                .get(&d.id)
                .filter(|t| !t.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("device {}", d.id));
            format!(
                "{tag} {:.0} → {:.0} C",
                d.surface_before_c, d.surface_after_c
            )
        })
        .collect();
    if !changed.is_empty() {
        ui.label(
            egui::RichText::new(format!("Housing: {}", changed.join(", ")))
                .small()
                .color(pal.text_secondary),
        );
    }
    if o.reading.unsettled {
        ui.label(
            egui::RichText::new(
                "This run did not settle fully; its numbers may move by about 1 K.",
            )
            .small()
            .color(pal.warning),
        );
    }
    if ui
        .add_enabled(!stale, egui::Button::new("Add these two to Vents and fans"))
        .on_hover_text(
            "Puts the pair in the list of the 3D Heat panel as one undo step there. Simulate in 3D then checks it",
        )
        .clicked()
    {
        *ask = Some(Ask::Add(o.candidate.clone()));
    }
}

fn headline(s: &Sweep) -> String {
    if s.best.is_empty() {
        return "Nothing to fix: every device has room to spare.".into();
    }
    match s.winner() {
        Some(w) => format!(
            "Best of {} tries: {}. It wins {:.1} K of margin in the model.",
            s.tried,
            w.candidate.pairing.label().to_lowercase(),
            w.gain_k
        ),
        None => format!(
            "No position or size wins {SIGNIFICANT_K:.0} K or more in the model ({} tries).",
            s.tried
        ),
    }
}

fn margin_text(m: Option<f64>) -> String {
    match m {
        Some(m) => format!("{m:+.1} K"),
        None => "no limits set".into(),
    }
}

fn over_text(n: usize) -> String {
    match n {
        0 => "no device over its limit".into(),
        1 => "1 device over".into(),
        n => format!("{n} devices over"),
    }
}

fn flow_text(d: &Delivered) -> String {
    format!("{:.0} m3/h", d.flow_m3h.abs())
}

/// The Vents and fans entry for one piece of a candidate.
pub(super) fn spec_of(piece: &Piece, inlet: bool) -> VentSpec {
    let o = &piece.opening;
    let (wall, u, v) = match o.placement {
        Placement::Top { x_mm } => (Wall::Top, x_mm, 0.0),
        Placement::Bottom { x_mm } => (Wall::Bottom, x_mm, 0.0),
        Placement::Left { y_mm } => (Wall::Left, y_mm, 0.0),
        Placement::Right { y_mm } => (Wall::Right, y_mm, 0.0),
        Placement::Front { x_mm, y_mm } => (Wall::Door, x_mm, y_mm),
    };
    let mut spec = VentSpec::new(wall);
    spec.u_mm = u;
    spec.v_mm = v;
    spec.width_mm = o.width_mm;
    spec.height_mm = o.height_mm;
    match piece.fan_flow_m3h {
        Some(flow) => {
            spec.kind = if inlet { Kind::FilterFan } else { Kind::Fan };
            spec.exhaust = !inlet;
            spec.flow_m3h = flow;
        }
        None => spec.kind = Kind::Grille,
    }
    spec
}

/// The two entries of a candidate: inlet first.
pub(super) fn specs_of(c: &Candidate) -> [VentSpec; 2] {
    [spec_of(&c.inlet, true), spec_of(&c.outlet, false)]
}

/// Adds the pair to the settings as one undo step.
pub(super) fn add_pair(settings: &mut HeatSettings, c: &Candidate) {
    settings.checkpoint();
    settings.vents.extend(specs_of(c));
    settings.selected = Some(settings.vents.len() - 1);
}

/// The sentence for the status line after adding.
pub(super) fn added_text(c: &Candidate) -> String {
    let kind = match c.pairing {
        Pairing::VentPair => "two vents",
        Pairing::FanPair => "a filter fan and an exhaust fan",
        Pairing::VentInFanOut => "a vent and an exhaust fan",
        Pairing::FanInVentOut => "a filter fan and a vent",
    };
    format!(
        "Added {kind} to Vents and fans in the 3D Heat panel. Simulate in 3D to check them; undo there takes them out."
    )
}
