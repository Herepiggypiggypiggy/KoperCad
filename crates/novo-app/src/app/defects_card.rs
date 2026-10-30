//! The Defects card of the live view (lane F1, design `docs/design/fault-injection.md`).
//!
//! Where the user finds it: Simulate > Defects and walk-throughs (command `sim.defects`). The card breaks the drawn circuit on purpose:
//! a contact welds, a coil opens, a conductor breaks, a phase is lost, a pin touches earth. The
//! live view then solves the circuit with the defect in it, and the card lists how the plant now
//! differs from a healthy one under the same operator inputs. Walk-throughs are scripted
//! scenarios (operator inputs, defects, expected outcomes) that can be recorded from the live
//! view, saved in the project and replayed with a report. It rehearses wiring and logic; it is
//! not a safety function and not a compliance result, and the card says so.

use std::collections::BTreeSet;
use std::sync::Arc;

use eframe::egui;
use novo_core::{DeviceId, Netlist, Project, SymbolId, Value};
use novo_sim::inject::{
    Defect, DefectKind, DefectSet, Prepared, Target, devices_for, pins_of, potential_names,
    wire_numbers,
};
use novo_sim::walkthrough::{
    Action, Recorder, Report, Verdict, Walkthrough, differences_of, run, run_with_machinery,
};
use novo_sim::{Clock, PoleState, Settled, rest_state, settle_timed};

use super::{Level, NovoApp};

/// The project property that holds the saved walk-throughs.
pub(super) const PROPERTY: &str = "walkthroughs.project";

/// Everything the live view keeps about defects and walk-throughs.
#[derive(Default)]
pub(crate) struct DefectsLive {
    pub(super) open: bool,
    set: DefectSet,
    /// Counts every change of the defect set, so the live solution knows to solve again.
    generation: u64,
    prepared: Option<(u64, u64, Prepared)>,
    /// The healthy plant under the same inputs: where the faulted one is compared to.
    shadow_clock: Clock,
    shadow_held: BTreeSet<SymbolId>,
    differences: Vec<String>,
    /// Add form.
    kind: usize,
    target: String,
    second: String,
    /// Saved walk-throughs and what was last read from the project.
    walk: Vec<Walkthrough>,
    synced: Option<String>,
    dirty: bool,
    damaged: Option<String>,
    report: Option<Report>,
    rec: Option<Rec>,
    /// Frame time in seconds, for the recorder.
    now: f64,
    recordings: u32,
}

/// A recording in progress.
struct Rec {
    recorder: Recorder,
    started: Option<f64>,
    last_poles: Option<PoleState>,
}

impl DefectsLive {
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether any defect is injected.
    pub(super) fn active(&self) -> bool {
        !self.set.is_empty()
    }

    /// Time passes for the healthy comparison plant (timer contacts count in both).
    pub(super) fn advance(&mut self, dt: f64) {
        self.shadow_clock.advance(dt);
    }

    fn bump(&mut self) {
        self.generation += 1;
        self.prepared = None;
        self.shadow_clock = Clock::new();
        self.shadow_held.clear();
        self.differences.clear();
    }

    fn since_start(&self) -> f64 {
        self.rec
            .as_ref()
            .and_then(|r| r.started)
            .map_or(0.0, |t0| (self.now - t0).max(0.0))
    }

    /// Injects a defect; a recording notes it.
    pub(super) fn inject(&mut self, defect: Defect) {
        let t = self.since_start();
        if let Some(rec) = self.rec.as_mut() {
            rec.recorder.note(
                t,
                Action::Inject {
                    with: defect.clone(),
                },
            );
        }
        self.set.inject(defect);
        self.bump();
    }

    fn repair(&mut self, defect: &Defect) {
        let t = self.since_start();
        if self.set.clear(defect) {
            if let Some(rec) = self.rec.as_mut() {
                rec.recorder.note(
                    t,
                    Action::Repair {
                        with: defect.clone(),
                    },
                );
            }
            self.bump();
        }
    }

    /// Repairs everything (also what leaving the live view does, so defects never linger).
    pub(super) fn repair_all(&mut self) {
        if !self.set.is_empty() {
            let t = self.since_start();
            if let Some(rec) = self.rec.as_mut() {
                rec.recorder.note(t, Action::RepairAll);
            }
            self.set.clear_all();
            self.bump();
        }
    }

    /// Solves the circuit with the defects in it; `None` when there are none (the caller
    /// solves the healthy plant). Also settles the healthy plant under the same inputs for the
    /// "what is different" list.
    pub(super) fn settle(
        &mut self,
        project: &Project,
        poles: &PoleState,
        forced: &BTreeSet<SymbolId>,
        held: &BTreeSet<SymbolId>,
        clock: &mut Clock,
    ) -> Option<(Arc<Netlist>, Settled)> {
        if self.set.is_empty() {
            return None;
        }
        let fresh = matches!(&self.prepared, Some((rev, generation, _))
            if *rev == project.revision() && *generation == self.generation);
        if !fresh {
            self.prepared = Some((
                project.revision(),
                self.generation,
                self.set.prepare(project),
            ));
        }
        let prepared = &self.prepared.as_ref()?.2;
        let faulted = prepared.settle(project, poles, forced, held, clock);
        let healthy_net = project.netlist();
        let healthy = settle_timed(
            project,
            &healthy_net,
            poles,
            forced,
            &self.shadow_held,
            &mut self.shadow_clock,
        );
        self.shadow_held = healthy.energized.clone();
        self.differences = differences_of(project, &healthy_net, &healthy, prepared, &faulted)
            .into_iter()
            .collect();
        Some((prepared.netlist().clone(), faulted))
    }

    fn problems(&self) -> &[String] {
        self.prepared
            .as_ref()
            .map_or(&[][..], |(_, _, p)| p.problems.as_slice())
    }

    /// Reads the saved walk-throughs from the project when its property changed.
    pub(super) fn sync(&mut self, project: &Project) {
        let text = stored(project);
        if self.synced.as_deref() == Some(text) {
            return;
        }
        self.synced = Some(text.to_owned());
        self.dirty = false;
        if text.trim().is_empty() {
            self.walk.clear();
            self.damaged = None;
            return;
        }
        match serde_json::from_str::<Vec<Walkthrough>>(text) {
            Ok(list) => {
                self.walk = list;
                self.damaged = None;
            }
            Err(e) => {
                self.walk.clear();
                self.damaged = Some(format!("The saved walk-throughs cannot be read: {e}"));
            }
        }
    }

    /// The property text to write, when the list changed and nothing damaged is kept.
    fn pending_write(&self) -> Option<String> {
        if !self.dirty || self.damaged.is_some() {
            return None;
        }
        Some(if self.walk.is_empty() {
            String::new()
        } else {
            serde_json::to_string_pretty(&self.walk).expect("walk-throughs serialize")
        })
    }

    fn start_recording(&mut self, poles: &PoleState) {
        self.rec = Some(Rec {
            recorder: Recorder::new(),
            started: None,
            last_poles: Some(poles.clone()),
        });
    }

    /// Ends the recording and keeps it as a walk-through when it caught anything.
    fn stop_recording(&mut self) -> bool {
        let Some(rec) = self.rec.take() else {
            return false;
        };
        if rec.recorder.is_empty() {
            return false;
        }
        let tail = self.since_start_of(&rec);
        self.recordings += 1;
        let name = format!("Recording {}", self.recordings);
        let last = rec.recorder.len();
        let mut walk = rec.recorder.finish(&name, 0.5);
        walk.tail = (tail - walk.steps.last().map_or(0.0, |s| s.t)).clamp(0.5, 10.0);
        walk.description = format!(
            "Recorded from the live view: {last} steps. Add Expect steps to make it a check."
        );
        self.walk.push(walk);
        self.dirty = true;
        true
    }

    fn since_start_of(&self, rec: &Rec) -> f64 {
        rec.started.map_or(0.0, |t0| (self.now - t0).max(0.0))
    }

    /// While recording: notes what the operator did since the last frame, from the change of
    /// the hand-set poles. The first thing noted starts the clock.
    fn observe(&mut self, project: &Project, poles: &PoleState) {
        let now = self.now;
        let Some(rec) = self.rec.as_mut() else {
            return;
        };
        let rest = rest_state(project);
        let before = rec.last_poles.clone().unwrap_or_else(|| poles.clone());
        let steps = operate_steps(project, &rest, &before, poles);
        if !steps.is_empty() {
            let t0 = *rec.started.get_or_insert(now);
            for step in steps {
                rec.recorder.note((now - t0).max(0.0), step);
            }
        }
        rec.last_poles = Some(poles.clone());
    }
}

fn stored(project: &Project) -> &str {
    match project.info().properties.get(PROPERTY) {
        Some(Value::Text(t)) => t.as_str(),
        _ => "",
    }
}

/// What the operator did between two pole states, as walk-through steps: one step per device
/// whose contacts all moved the same way, else one per contact (named by a pin of its own).
pub(super) fn operate_steps(
    project: &Project,
    rest: &PoleState,
    before: &PoleState,
    now: &PoleState,
) -> Vec<Action> {
    let mut out = Vec::new();
    let devices: BTreeSet<DeviceId> = project
        .symbols()
        .filter(|s| !s.pass_through.is_empty())
        .filter_map(|s| s.device())
        .collect();
    for device in devices {
        let Some(tag) = project.device(device).map(|d| d.tag.to_string()) else {
            continue;
        };
        let symbols = project.symbols_of_device(device);
        let poles: Vec<(SymbolId, usize)> = symbols
            .iter()
            .flat_map(|s| (0..s.pass_through.len()).map(move |g| (s.id, g)))
            .collect();
        let operated = |state: &PoleState, (s, g): (SymbolId, usize)| {
            state.is_closed(s, g) != rest.is_closed(s, g)
        };
        let changed: Vec<(SymbolId, usize)> = poles
            .iter()
            .copied()
            .filter(|&p| operated(before, p) != operated(now, p))
            .collect();
        let Some(&first) = changed.first() else {
            continue;
        };
        let direction = operated(now, first);
        let same = changed.iter().all(|&p| operated(now, p) == direction);
        if same && changed.len() == poles.len() {
            out.push(Action::Operate {
                device: tag,
                pin: None,
                operated: direction,
            });
            continue;
        }
        for &(symbol, group) in &changed {
            let Some(sym) = project.symbol(symbol) else {
                continue;
            };
            // A pin that belongs to this contact alone names it.
            let own = sym.pass_through[group].iter().find_map(|&i| {
                let name = sym.pins.get(usize::from(i))?.name.clone();
                let shared = symbols.iter().any(|other| {
                    other.pass_through.iter().enumerate().any(|(g, pins)| {
                        (other.id, g) != (symbol, group)
                            && pins.iter().any(|&j| {
                                other
                                    .pins
                                    .get(usize::from(j))
                                    .is_some_and(|p| p.name == name)
                            })
                    })
                });
                (!shared && !name.is_empty()).then_some(name)
            });
            out.push(Action::Operate {
                device: tag.clone(),
                pin: own,
                operated: operated(now, (symbol, group)),
            });
        }
    }
    out
}

impl NovoApp {
    /// Simulate > Defects and walk-throughs: the live view on, the card open or closed.
    pub(super) fn toggle_defects_panel(&mut self) {
        if !self.wb.live.on {
            self.toggle_live();
        }
        self.wb.live.defects.open ^= true;
    }

    /// The Defects card and the banner, drawn over the live view.
    pub(super) fn defects_card(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.wb.live.on {
            return;
        }
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let live = &mut self.wb.live;
        live.defects.now = ctx.input(|i| i.time);
        live.defects.sync(project);
        if live.defects.rec.is_some() {
            let poles = live.poles.clone();
            live.defects.observe(project, &poles);
            ctx.request_repaint();
        }
        if live.defects.active() {
            banner(ctx, canvas, &live.defects, live.plc.is_running());
        }
        if !live.defects.open {
            self.defects_save();
            return;
        }
        let mut actions = Vec::new();
        let offset = if live.ctl.open { 360.0 } else { 10.0 };
        let poles = live.poles.clone();
        let running = live.plc.is_running();
        let defects = &mut live.defects;
        super::panel::Panel::new(
            "defects_card",
            "Defects and walk-throughs",
            canvas,
            super::panel::Side::Right,
            320.0,
        )
        .top(offset)
        .fixed_open()
        .fit()
        .scrolls()
        .show(ctx, |ui| {
            draw_card(ui, defects, project, &poles, running, &mut actions);
        });
        for action in actions {
            match action {
                CardAction::Close => self.wb.live.defects.open = false,
                CardAction::Run(index) => {
                    if let Some(doc) = self.backend.document() {
                        let defects = &mut self.wb.live.defects;
                        if let Some(w) = defects.walk.get(index) {
                            defects.report = Some(run_walkthrough(doc.project(), w));
                        }
                    }
                }
                CardAction::Delete(index) => {
                    let defects = &mut self.wb.live.defects;
                    if index < defects.walk.len() {
                        defects.walk.remove(index);
                        defects.dirty = true;
                        defects.report = None;
                    }
                }
                CardAction::Record => {
                    let poles = self.wb.live.poles.clone();
                    self.wb.live.defects.start_recording(&poles);
                }
                CardAction::StopRecord => {
                    if !self.wb.live.defects.stop_recording() {
                        self.message(Level::Info, "Nothing was recorded");
                    }
                }
            }
        }
        self.defects_save();
    }

    /// Writes the walk-throughs to the project when the list changed: one undo step.
    pub(super) fn defects_save(&mut self) {
        let Some(text) = self.wb.live.defects.pending_write() else {
            return;
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let result = doc.transact("Walk-throughs", |tx| {
            tx.set_info(|i| {
                if text.is_empty() {
                    i.properties.remove(PROPERTY);
                } else {
                    i.properties
                        .insert(PROPERTY.to_owned(), Value::Text(text.clone()));
                }
            })
        });
        match result {
            Ok(((), changes)) => {
                let defects = &mut self.wb.live.defects;
                defects.synced = Some(text);
                defects.dirty = false;
                self.changed_outside_canvas(&changes);
            }
            Err(e) => {
                self.wb.live.defects.dirty = false;
                self.message(
                    Level::Error,
                    format!("Walk-throughs could not be saved: {e}"),
                );
            }
        }
    }
}

enum CardAction {
    Close,
    Run(usize),
    Delete(usize),
    Record,
    StopRecord,
}

/// The strip over the canvas while a defect is injected, so nobody mistakes a faulted plant for
/// the drawing.
fn banner(ctx: &egui::Context, canvas: egui::Rect, defects: &DefectsLive, plc_running: bool) {
    let palette = crate::theme::ui_palette();
    let count = defects.set.defects().len();
    let mut text = format!(
        "DEFECT INJECTED ({count}): {}",
        defects.set.defects()[0].label()
    );
    if count > 1 {
        text.push_str(&format!(" and {} more", count - 1));
    }
    if plc_running {
        text.push_str(" - not applied while a PLC program runs");
    }
    egui::Area::new(egui::Id::new("defect_banner"))
        .fixed_pos(egui::pos2(canvas.center().x, canvas.top() + 6.0))
        .pivot(egui::Align2::CENTER_TOP)
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(palette.error)
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(10, 4))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(text)
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                });
        });
}

fn draw_card(
    ui: &mut egui::Ui,
    d: &mut DefectsLive,
    project: &Project,
    poles: &PoleState,
    plc_running: bool,
    actions: &mut Vec<CardAction>,
) {
    let palette = crate::theme::ui_palette();
    ui.horizontal(|ui| {
        if ui.small_button("✕ Close").clicked() {
            actions.push(CardAction::Close);
        }
        ui.weak("Rehearsal of wiring and logic");
    });
    ui.add(
        egui::Label::new(
            egui::RichText::new(
                "Not a safety function and not a compliance result: it shows what the drawn \
                 circuit does when one thing breaks, not how likely that is.",
            )
            .weak()
            .small(),
        )
        .wrap(),
    );
    if plc_running && d.active() {
        ui.colored_label(
            palette.warning,
            "Defects are not applied while a PLC program runs.",
        );
    }
    ui.separator();
    add_form(ui, d, project);
    ui.separator();
    active_list(ui, d, palette.warning);
    ui.separator();
    ui.label(egui::RichText::new("Different from a healthy plant").strong());
    if !d.active() {
        ui.weak("Inject a defect, then operate the circuit.");
    } else if d.differences.is_empty() {
        ui.weak(
            "Nothing differs yet. Operate the circuit: a defect often shows only when it is used.",
        );
    } else {
        for line in &d.differences {
            ui.add(egui::Label::new(egui::RichText::new(line).color(palette.error)).wrap());
        }
    }
    ui.separator();
    walkthroughs(ui, d, poles, actions);
}

fn add_form(ui: &mut egui::Ui, d: &mut DefectsLive, project: &Project) {
    ui.label(egui::RichText::new("Inject a defect").strong());
    let kind = DefectKind::ALL[d.kind.min(DefectKind::ALL.len() - 1)];
    let mut chosen = d.kind;
    egui::ComboBox::from_id_salt("defect_kind")
        .selected_text(kind.label())
        .width(260.0)
        .show_ui(ui, |ui| {
            for (i, k) in DefectKind::ALL.iter().enumerate() {
                ui.selectable_value(&mut chosen, i, k.label());
            }
        });
    if chosen != d.kind {
        d.kind = chosen;
        d.target.clear();
        d.second.clear();
    }
    let kind = DefectKind::ALL[d.kind.min(DefectKind::ALL.len() - 1)];
    match kind.target() {
        Target::Contacts | Target::Coil => {
            let list = devices_for(project, kind.target());
            pick(ui, "defect_device", "Device", &list, &mut d.target);
        }
        Target::Pin => pin_pick(ui, project, d),
        Target::Wire => {
            let list = wire_numbers(project);
            pick(ui, "defect_wire", "Wire number", &list, &mut d.target);
        }
        Target::Potential => {
            let list = potential_names(project);
            pick(ui, "defect_potential", "Supply", &list, &mut d.second);
        }
        Target::PinAndPotential => {
            pin_pick(ui, project, d);
            let list = potential_names(project);
            pick(ui, "defect_potential", "Touches", &list, &mut d.second);
        }
    }
    let ready = match kind.target() {
        Target::Potential => !d.second.is_empty(),
        Target::PinAndPotential => d.target.contains(':') && !d.second.is_empty(),
        Target::Pin => d.target.contains(':'),
        _ => !d.target.is_empty(),
    };
    if ui
        .add_enabled(ready, egui::Button::new("Inject defect"))
        .on_hover_text(kind.build(&d.target, &d.second).meaning())
        .clicked()
    {
        let defect = kind.build(&d.target, &d.second);
        d.inject(defect);
    }
}

fn pick(ui: &mut egui::Ui, id: &str, label: &str, list: &[String], value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        let shown = if value.is_empty() {
            "choose...".to_owned()
        } else {
            value.clone()
        };
        egui::ComboBox::from_id_salt(id)
            .selected_text(shown)
            .width(200.0)
            .show_ui(ui, |ui| {
                for item in list {
                    ui.selectable_value(value, item.clone(), item);
                }
            });
    });
}

/// Device then pin: the target is stored as `tag:pin`.
fn pin_pick(ui: &mut egui::Ui, project: &Project, d: &mut DefectsLive) {
    let (mut device, mut pin) = match d.target.split_once(':') {
        Some((t, p)) => (t.to_owned(), p.to_owned()),
        None => (d.target.clone(), String::new()),
    };
    let devices = devices_for(project, Target::Pin);
    let before = device.clone();
    pick(ui, "defect_pin_device", "Device", &devices, &mut device);
    if device != before {
        pin.clear();
    }
    let pins = pins_of(project, &device);
    pick(ui, "defect_pin", "Pin", &pins, &mut pin);
    d.target = if pin.is_empty() {
        device
    } else {
        format!("{device}:{pin}")
    };
}

fn active_list(ui: &mut egui::Ui, d: &mut DefectsLive, warn: egui::Color32) {
    ui.label(egui::RichText::new("In the plant now").strong());
    if d.set.is_empty() {
        ui.weak("No defects: the plant is as drawn.");
        return;
    }
    let mut repair = None;
    for defect in d.set.defects() {
        ui.horizontal(|ui| {
            if ui
                .small_button("Repair")
                .on_hover_text("Take this defect out again")
                .clicked()
            {
                repair = Some(defect.clone());
            }
            ui.add(egui::Label::new(defect.label()).wrap())
                .on_hover_text(defect.meaning());
        });
    }
    for p in d.problems() {
        ui.add(egui::Label::new(egui::RichText::new(p).color(warn)).wrap());
    }
    let mut all = false;
    if d.set.defects().len() > 1 && ui.small_button("Repair everything").clicked() {
        all = true;
    }
    if let Some(defect) = repair {
        d.repair(&defect);
    }
    if all {
        d.repair_all();
    }
}

fn walkthroughs(
    ui: &mut egui::Ui,
    d: &mut DefectsLive,
    _poles: &PoleState,
    actions: &mut Vec<CardAction>,
) {
    let palette = crate::theme::ui_palette();
    ui.label(egui::RichText::new("Walk-throughs").strong());
    if let Some(why) = d.damaged.clone() {
        ui.colored_label(palette.error, why);
        ui.weak("The stored text is kept in the project and not overwritten.");
    }
    if d.rec.is_some() {
        let steps = d.rec.as_ref().map_or(0, |r| r.recorder.len());
        ui.colored_label(palette.error, format!("Recording: {steps} steps"));
        ui.weak("Operate the circuit and inject defects as usual.");
        if ui.button("■ Stop and keep").clicked() {
            actions.push(CardAction::StopRecord);
        }
    } else if ui
        .button("● Record")
        .on_hover_text("Records your operator moves and defects as a walk-through")
        .clicked()
    {
        actions.push(CardAction::Record);
    }
    if d.walk.is_empty() {
        ui.weak("None saved. Record one, or write one in a file and import it.");
    }
    for index in 0..d.walk.len() {
        ui.horizontal(|ui| {
            if ui.small_button("▶ Run").clicked() {
                actions.push(CardAction::Run(index));
            }
            if ui.small_button("Delete").clicked() {
                actions.push(CardAction::Delete(index));
            }
            ui.add(egui::Label::new(&d.walk[index].name).wrap())
                .on_hover_text(&d.walk[index].description);
        });
    }
    if let Some(report) = d.report.clone() {
        ui.separator();
        report_view(ui, &report);
    }
}

fn report_view(ui: &mut egui::Ui, report: &Report) {
    let palette = crate::theme::ui_palette();
    let verdict = format!(
        "{}: {} passed, {} failed",
        report.name, report.passed, report.failed
    );
    let color = if !report.ok() {
        palette.error
    } else if report.passed == 0 {
        // Nothing was checked: neither a pass nor a fail.
        ui.visuals().weak_text_color()
    } else {
        palette.success
    };
    let verdict = if report.passed + report.failed == 0 {
        format!("{}: no checks yet", report.name)
    } else {
        verdict
    };
    ui.label(egui::RichText::new(verdict).strong().color(color));
    for p in &report.problems {
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("Not applied: {p}")).color(palette.warning),
            )
            .wrap(),
        );
    }
    for step in &report.steps {
        let line = format!("{:.1} s  {}", step.t, step.label);
        match step.verdict {
            Verdict::Fail => {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("✗ {line}: {}", step.detail))
                            .color(palette.error),
                    )
                    .wrap(),
                );
            }
            Verdict::Pass => {
                ui.add(egui::Label::new(format!("✓ {line}")).wrap());
            }
            Verdict::Done => {
                ui.add(egui::Label::new(egui::RichText::new(line).weak()).wrap());
            }
        }
        for a in &step.appeared {
            ui.add(egui::Label::new(egui::RichText::new(format!("    → {a}")).small()).wrap());
        }
        for g in &step.gone {
            ui.add(
                egui::Label::new(egui::RichText::new(format!("    ← back to normal: {g}")).small())
                    .wrap(),
            );
        }
    }
    if ui.small_button("Copy report").clicked() {
        ui.ctx().copy_text(report.to_markdown());
    }
}

/// Plays a walk-through with everything the project can run besides its circuit: hoists,
/// closed-loop winches and winch pairs. Steps that put a defect on a hoist or ask where a hook
/// is or what a winch's drive did then have something to act on.
pub(super) fn run_walkthrough(project: &novo_core::Project, w: &Walkthrough) -> Report {
    use novo_machine::cl_pair_loop::ClPairLoop;
    use novo_machine::cl_winch_loop::ClWinchLoop;
    use novo_machine::cosim::StepConfig;
    use novo_machine::hoist_loop::HoistLoop;
    use novo_sim::machine::Machinery;
    let (has_hoists, has_winches, has_pairs) = (
        !super::hoist_run::hoists_of(project).0.is_empty(),
        !super::drive_run::closed_winches_of(project).0.is_empty(),
        !super::drive_run::closed_pairs_of(project).0.is_empty(),
    );
    if !(has_hoists || has_winches || has_pairs) {
        return run(project, w);
    }
    let make_hoists = || {
        let mut lp = HoistLoop::new(StepConfig::default()).expect("the default step is valid");
        lp.hoists = super::hoist_run::hoists_of(project).0;
        lp
    };
    let make_winches = || {
        let mut cw = ClWinchLoop::new(StepConfig::default()).expect("the default step is valid");
        cw.modules = super::drive_run::closed_winches_of(project).0;
        cw
    };
    let make_pairs = || {
        let mut cp = ClPairLoop::new(StepConfig::default()).expect("the default step is valid");
        cp.modules = super::drive_run::closed_pairs_of(project).0;
        cp
    };
    run_with_machinery(
        project,
        w,
        &Machinery {
            hoists: has_hoists.then_some(&make_hoists),
            winches: has_winches.then_some(&make_winches),
            pairs: has_pairs.then_some(&make_pairs),
        },
    )
}
