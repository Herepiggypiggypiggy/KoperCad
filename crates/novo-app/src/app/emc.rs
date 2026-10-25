//! "EMC check…" (Check menu, Ctrl+K "EMC"): the EMC rules of novo-emc on the
//! open project. Turn the check on, read each finding with its reason, its
//! level (rule, index or estimate) and its suggestion, apply the suggestions
//! the program can do as one undo step, and see what the numbers assume and
//! what is not checked. A design aid, never a test result.
//!
//! The findings also arrive in the Issues list once the check is on; this
//! window adds the risk by wire, the reasons behind each score, the
//! assumptions and the Apply buttons.

use eframe::egui;
use novo_emc::project as emc;
use novo_emc::{Band, Fix, Level, Report, Severity, Status, Subject};

use super::NovoApp;
use super::emc_view::{WhatIfAction, WhatIfView};
use super::workspace::Workspace;

/// The report for one revision of the project, so the window does not route
/// every wire on every frame.
#[derive(Default)]
pub(super) struct EmcState {
    pub(super) open: bool,
    cache: Option<(u64, Cached)>,
    /// What the last Apply did, in a sentence.
    done: Option<String>,
    /// The fix being previewed and what it would change, for the revision
    /// of the project it was worked out on.
    preview: Option<(u64, Fix, Result<emc::Preview, String>)>,
    /// The previewed copy of the project for the 3D and plate views, set
    /// with the preview and dropped with it.
    whatif: Option<WhatIf>,
    /// The preview card has just opened: scroll it into view once.
    preview_fresh: bool,
}

impl EmcState {
    #[cfg(test)]
    pub(super) fn previewed(&self) -> Option<&Result<emc::Preview, String>> {
        self.preview.as_ref().map(|(_, _, p)| p)
    }

    #[cfg(test)]
    pub(super) fn done_text(&self) -> Option<&str> {
        self.done.as_deref()
    }
}

struct WhatIf {
    id: u64,
    revision: u64,
    project: std::sync::Arc<novo_core::Project>,
    summary: String,
    after: bool,
}

static WHATIF_IDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct Cached {
    report: Report,
    params: novo_emc::FieldParams,
    classes: Vec<(novo_emc::EmcClass, usize)>,
    names: std::collections::HashMap<u64, String>,
}

fn level_chip(level: Level) -> &'static str {
    match level {
        Level::Fact => "Rule",
        Level::Index => "Index",
        Level::Estimate => "Estimate",
        Level::Field => "Field",
    }
}

fn status_text(status: Status) -> &'static str {
    match status {
        Status::NoFinding => "No finding",
        Status::Finding => "Finding",
        Status::NotApplicable => "Not applicable",
        Status::NotEvaluated => "Not evaluated",
    }
}

/// A number the user drags or types. The edit is kept in the UI until the
/// drag ends or the field loses focus, so one gesture is one undo step.
fn number(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    value: f64,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
    tip: &str,
) -> Option<f64> {
    let id = ui.make_persistent_id(salt);
    let mut v = ui.data(|d| d.get_temp::<f64>(id)).unwrap_or(value);
    let r = ui
        .add(
            egui::DragValue::new(&mut v)
                .speed(speed)
                .range(range)
                .suffix(suffix),
        )
        .on_hover_text(tip);
    if r.dragged() || r.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, v));
    }
    if r.drag_stopped() || r.lost_focus() {
        ui.data_mut(|d| d.remove_temp::<f64>(id));
        return ((v - value).abs() > 1e-9).then_some(v);
    }
    None
}

/// The magnetic field estimate: what it assumes, the currents it uses (each
/// can be stated here), and what it finds at sensitive devices and wires.
fn field_section(
    ui: &mut egui::Ui,
    cached: &Cached,
    set_current: &mut Option<(u64, Option<f64>)>,
    set_setting: &mut Option<(&'static str, Option<novo_core::Value>)>,
) {
    let p = crate::theme::ui_palette();
    let Some(field) = &cached.report.field else {
        return;
    };
    let summary = if field.currents.is_empty() {
        "Magnetic field estimate: no wire has a current yet".to_owned()
    } else {
        let worst = field
            .devices
            .iter()
            .max_by(|a, b| a.b_ut.total_cmp(&b.b_ut))
            .map(|d| format!(", {} sits in about {:.1} \u{b5}T", d.tag, d.b_ut))
            .unwrap_or_default();
        format!(
            "Magnetic field estimate: {} wire{} with a current{worst}",
            field.currents.len(),
            if field.currents.len() == 1 { "" } else { "s" }
        )
    };
    egui::CollapsingHeader::new(egui::RichText::new(summary).strong())
        .id_salt("emc_field_section")
        .default_open(false)
        .show(ui, |ui| {
    ui.weak("A model: the currents below, on the routed wires, add up by Biot-Savart (go and return and the three phases cancel as drawn). Differential current only: no common-mode current, no door or cabinet screening. Order of magnitude, never a measurement.");
    ui.horizontal_wrapped(|ui| {
        ui.label("Frequency");
        if let Some(hz) = number(ui, "emc_field_hz", cached.params.hz, 1.0, 1.0..=30.0e6, " Hz", "Frequency of the currents. From 1 kHz up the plate counts as a conducting plane") {
            *set_setting = Some(("field_hz", Some(novo_core::Value::Float(hz))));
        }
        ui.label("Planning value at a PLC or analog module");
        if let Some(limit) = number(ui, "emc_field_limit", cached.params.limit_ut, 0.1, 0.1..=1000.0, " \u{b5}T", "The field at a PLC or analog module above which EMC-19 reports (house value, not a standard's limit)") {
            *set_setting = Some(("field_limit_ut", Some(novo_core::Value::Float(limit))));
        }
        let mut image = field.image;
        if ui
            .checkbox(&mut image, "Plate is a conducting plane")
            .on_hover_text("Treats the plate as a conducting plane (a mirror image of the current): right for steep edges, wrong for 50 Hz in a thin plate")
            .changed()
        {
            *set_setting = Some(("plate_image", Some(novo_core::Value::Bool(image))));
        }
    });
    if field.currents.is_empty() {
        ui.colored_label(p.warning, "No wire has a current yet, so the field is not evaluated. Give a wire its current in the list that appears here, or give the devices at its ends a rated current (Inspector, Rated current).");
    }
    for row in &field.currents {
        ui.horizontal_wrapped(|ui| {
            ui.label(&row.name);
            if let Some(amps) = number(ui, ("emc_amps", row.run), row.amps, 0.1, 0.0..=5000.0, " A", "RMS current in this wire. Changing it states the current (emc.current_a), one undo step") {
                *set_current = Some((row.run, Some(amps)));
            }
            ui.weak(format!("{:+.0}\u{b0}  {}{}", row.phase_deg, row.source, if row.routed { "" } else { "; no route on a plate, not in the field" }));
            if row.declared && ui.small_button("Use rating").on_hover_text("Forget the stated current").clicked() {
                *set_current = Some((row.run, None));
            }
        });
    }
    if !field.devices.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Sensitive devices").strong());
        for d in &field.devices {
            let colour = if d.b_ut >= field.limit_ut {
                p.warning
            } else {
                p.text_secondary
            };
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(colour, format!("{}: about {:.1} \u{b5}T", d.tag, d.b_ut));
                ui.weak(format!(
                    "planning value {:.1} \u{b5}T (largest over the device's footprint)",
                    field.limit_ut
                ));
            });
        }
    }
    if !field.victims.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Sensitive wires").strong());
        for v in &field.victims {
            let colour = if v.v_line_mv.max(v.v_edge_mv) >= v.limit_mv {
                p.warning
            } else {
                p.text_secondary
            };
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    colour,
                    format!(
                        "{}: {:.3} mV at {} Hz, {:.1} mV on a current step",
                        v.name, v.v_line_mv, field.hz, v.v_edge_mv
                    ),
                );
                ui.weak(format!(
                    "planning value {:.1} mV; {}",
                    v.limit_mv,
                    if v.paired {
                        "go and return drawn"
                    } else {
                        "return taken as the plate"
                    }
                ));
            });
        }
    }
        });
}

/// What a previewed fix would change, with Apply and Cancel at the top so
/// they stay in reach in a short window. Returns true for Apply; `cancel`
/// is set for Cancel. `scroll` brings the card into view (just opened).
pub(super) fn preview_card(
    ui: &mut egui::Ui,
    preview: &Result<emc::Preview, String>,
    cancel: &mut bool,
    show: &mut Option<Workspace>,
    scroll: bool,
    apply_at: &mut Option<egui::Rect>,
) -> bool {
    let p = crate::theme::ui_palette();
    let mut apply = false;
    let card = egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Preview on a copy of the project").strong());
            if preview.is_ok() {
                let button = ui
                    .button("Apply (one undo step)")
                    .on_hover_text("Do this on the project; Edit > Undo takes it back");
                *apply_at = Some(button.rect);
                if button.clicked() {
                    apply = true;
                }
            }
            if ui.button("Cancel").clicked() {
                *cancel = true;
            }
        });
        if preview.is_ok() {
            ui.horizontal_wrapped(|ui| {
                ui.weak("Before and after, drawn in the EMC view:");
                if ui.small_button("In 3D").on_hover_text("Open the 3D view with the EMC layer and its before/after switch").clicked() {
                    *show = Some(Workspace::View3d);
                }
                if ui.small_button("On the plate").on_hover_text("Open the Cabinet plate with the EMC layer and its before/after switch").clicked() {
                    *show = Some(Workspace::Cabinet);
                }
            });
        }
        match preview {
            Err(e) => {
                ui.colored_label(p.error, format!("Cannot preview: {e}"));
            }
            Ok(v) => {
                ui.label(&v.said);
                ui.label(format!("Findings: {} before, {} after.", v.findings_before, v.findings_after));
                for (rule, msg) in &v.cleared {
                    ui.colored_label(p.success, format!("Clears {rule}: {msg}"));
                }
                for (rule, msg) in &v.added {
                    ui.colored_label(p.warning, format!("Brings {rule}: {msg}"));
                }
                for e in &v.eased {
                    ui.colored_label(
                        p.success,
                        format!(
                            "Eases {} from {} to {}: {}",
                            e.rule,
                            e.from.word(),
                            e.to.word(),
                            e.after
                        ),
                    );
                }
                if v.cleared.is_empty() && v.added.is_empty() && v.eased.is_empty() {
                    ui.weak("No finding is cleared or added by this fix.");
                }
                for (_, name, before, after) in &v.scores {
                    ui.weak(format!("Risk of {name}: {before} of 100 to {after} of 100"));
                }
                ui.weak("A model run on the rules and estimates above. It records what the build must do; the cabinet has to be built that way. Never compliance.");
            }
        }
    });
    if scroll {
        card.response.scroll_to_me(Some(egui::Align::Center));
    }
    apply
}

impl EmcState {
    /// The fix being previewed and what it would change, when it was worked
    /// out on the project as it is now.
    pub(super) fn shown_preview(
        &self,
        revision: u64,
    ) -> Option<(Fix, Result<emc::Preview, String>)> {
        self.preview
            .as_ref()
            .filter(|(r, _, _)| *r == revision)
            .map(|(_, f, p)| (f.clone(), p.clone()))
    }

    /// Takes the preview card's fresh flag (scroll it into view once).
    pub(super) fn take_fresh(&mut self) -> bool {
        std::mem::take(&mut self.preview_fresh)
    }
}

impl NovoApp {
    /// What a preview card asked for: preview a fix, cancel it, or apply it.
    pub(super) fn emc_preview_actions(
        &mut self,
        want: Option<Fix>,
        cancel: bool,
        show: Option<Workspace>,
        apply: Option<Fix>,
    ) {
        if cancel {
            self.emc.preview = None;
            self.emc.whatif = None;
        }
        if let Some(ws) = show {
            self.emc_show_whatif(ws);
        }
        if let Some(fix) = want {
            self.emc_preview(&fix);
        }
        if let Some(fix) = apply {
            self.emc.preview = None;
            self.emc.whatif = None;
            self.emc_apply(&fix);
        }
    }

    pub(super) fn emc_window(&mut self, ctx: &egui::Context) {
        if !self.emc.open {
            return;
        }
        let Some(doc) = self.backend.document() else {
            self.emc.open = false;
            return;
        };
        let project = doc.project();
        let revision = project.revision();
        if self.emc.cache.as_ref().is_none_or(|(r, _)| *r != revision) {
            let netlist = project.netlist();
            let input = emc::input_of(project, &netlist);
            let field = emc::field_setup_of(project, &netlist, &input);
            let report = novo_emc::analyse_with_field(&input, Some(&field));
            let cached = Cached {
                params: field.params,
                classes: emc::class_counts(&input).into_iter().collect(),
                names: input.runs.iter().map(|r| (r.id, r.name.clone())).collect(),
                report,
            };
            self.emc.cache = Some((revision, cached));
        }
        let enabled = emc::is_enabled(project);
        let Some((_, cached)) = self.emc.cache.as_ref() else {
            return;
        };
        let report = &cached.report;
        let p = crate::theme::ui_palette();
        let mut turn: Option<bool> = None;
        let mut apply: Option<Fix> = None;
        let mut want_preview: Option<Fix> = None;
        let mut cancel_preview = false;
        let mut save_record: Option<super::emc_record::EmcRecordFormat> = None;
        let mut show_view: Option<Workspace> = None;
        let shown = self
            .emc
            .preview
            .as_ref()
            .filter(|(r, _, _)| *r == revision)
            .cloned();
        let fresh = std::mem::take(&mut self.emc.preview_fresh);
        let mut set_current: Option<(u64, Option<f64>)> = None;
        let mut set_setting: Option<(&'static str, Option<novo_core::Value>)> = None;
        let mut open = true;
        super::panel::dialog(ctx, "EMC check", [680.0, 540.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                let mut on = enabled;
                if ui
                    .checkbox(&mut on, "Run the EMC rules with the design checks")
                    .on_hover_text("Findings join the Issues list and the count in the top bar, and the standards report gets an EMC section")
                    .changed()
                {
                    turn = Some(on);
                }
                if !enabled {
                    ui.weak("Off: the findings below are a preview and do not appear in the Issues list.");
                }
                ui.weak(format!(
                    "{} Findings are rules on what is drawn, geometric indices and closed-form estimates with planning distances from common practice; clause numbers are unverified.",
                    novo_emc::DESIGN_AID
                ));
                if let Some(done) = &self.emc.done {
                    ui.colored_label(p.success, done);
                }
                ui.horizontal(|ui| {
                    if ui
                        .button("Save EMC record (PDF)")
                        .on_hover_text("The findings with what to do at the bench, the rules in the check, the assumptions and what is not checked, next to the project file. The PDF replaces an earlier record of yours; the Excel file never replaces one (a numbered name is used)")
                        .clicked()
                    {
                        save_record = Some(super::emc_record::EmcRecordFormat::Pdf);
                    }
                    if ui
                        .button("Save EMC record (Excel)")
                        .on_hover_text("One row per finding, for filtering and sorting. An earlier Excel record is never replaced: the file gets a number")
                        .clicked()
                    {
                        save_record = Some(super::emc_record::EmcRecordFormat::Excel);
                    }
                });
                ui.add_space(6.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    novo_theme::widgets::section_label(ui, "Wire classes");
                    if cached.classes.is_empty() {
                        ui.label("No wires drawn yet.");
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            for (class, n) in &cached.classes {
                                ui.label(format!("{} {n}", class.label()));
                            }
                        });
                        ui.weak("Each class is read from the drawing (drive outputs, safety and analog devices, potentials); set `emc.class` on a wire or cable to override it.");
                    }
                    ui.add_space(6.0);
                    field_section(ui, cached, &mut set_current, &mut set_setting);
                    ui.add_space(6.0);
                    novo_theme::widgets::section_label(ui, &format!("Findings ({})", report.findings.len()));
                    if report.findings.is_empty() {
                        ui.label("No finding. That is not a statement that the design is fine: see Not checked below.");
                    }
                    for f in &report.findings {
                        let colour = match f.severity {
                            Severity::Error => p.error,
                            Severity::Warning => p.warning,
                            Severity::Info => p.text_secondary,
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(colour, egui::RichText::new(f.rule).strong());
                            ui.weak(level_chip(f.level));
                            ui.label(&f.message);
                        });
                        for m in &f.metrics {
                            ui.weak(format!("    {} ({}): {}", m.label, level_chip(m.level).to_lowercase(), m.value));
                        }
                        ui.horizontal_wrapped(|ui| {
                            for fix in &f.fixes {
                                if fix.applicable() {
                                    let open = shown.as_ref().is_some_and(|(_, f, _)| f == fix);
                                    if ui
                                        .selectable_label(open, format!("Preview: {}", fix.hint()))
                                        .on_hover_text("See what it clears before anything changes")
                                        .clicked()
                                    {
                                        if open {
                                            cancel_preview = true;
                                        } else {
                                            want_preview = Some(fix.clone());
                                        }
                                    }
                                } else {
                                    ui.weak(format!("By hand: {}", fix.hint()));
                                }
                            }
                        });
                        if let Some((_, pf, pv)) = &shown
                            && f.fixes.contains(pf)
                            && preview_card(
                                ui,
                                pv,
                                &mut cancel_preview,
                                &mut show_view,
                                fresh,
                                &mut None,
                            )
                        {
                            apply = Some(pf.clone());
                        }
                        ui.add_space(4.0);
                    }
                    ui.add_space(6.0);
                    novo_theme::widgets::section_label(ui, "Risk by wire");
                    let mut runs: Vec<_> = report.runs.iter().filter(|s| s.points > 0).collect();
                    runs.sort_by_key(|r| std::cmp::Reverse(r.points));
                    if runs.is_empty() {
                        ui.label("No wire has a finding against it.");
                    }
                    for s in runs.iter().take(10) {
                        let name = match s.subject {
                            Subject::Run(id) => cached.names.get(&id).cloned().unwrap_or_default(),
                            _ => String::new(),
                        };
                        let band = match s.band {
                            Band::High => p.error,
                            Band::Medium => p.warning,
                            _ => p.text_secondary,
                        };
                        egui::CollapsingHeader::new(egui::RichText::new(format!("{name}: {} of 100, {}", s.points, s.band.label().to_lowercase())).color(band))
                            .id_salt(("emc_run", s.points, &name))
                            .show(ui, |ui| {
                                ui.weak("The score is the sum of these reasons, capped at 100. It orders wires for attention; it is not a measurement.");
                                for r in &s.reasons {
                                    ui.label(format!("+{}  {}  {}", r.points, r.rule, r.text));
                                }
                            });
                    }
                    ui.add_space(6.0);
                    egui::CollapsingHeader::new("Rules in the check").show(ui, |ui| {
                        for r in &report.rules {
                            let rule = novo_emc::rules::rule(r.rule);
                            ui.label(format!("{}  {}: {} ({}) {}", r.rule, rule.title, status_text(r.status), level_chip(rule.level).to_lowercase(), r.note));
                            ui.weak(format!("    {} Source: {}", rule.requirement, rule.source));
                        }
                    });
                    egui::CollapsingHeader::new("Assumptions").show(ui, |ui| {
                        for a in &report.assumptions {
                            ui.label(format!(
                                "{}: {}  ({})",
                                a.name,
                                a.value,
                                if a.property.is_empty() || a.user_set { a.origin() } else { "program default, edit with the property" }
                            ));
                        }
                    });
                    egui::CollapsingHeader::new("Not checked").default_open(true).show(ui, |ui| {
                        for t in novo_emc::NOT_CHECKED {
                            ui.label(*t);
                        }
                    });
                });
            });
        self.emc.open = open;
        if let Some(format) = save_record {
            self.export_emc_record(format);
        }
        if let Some(on) = turn {
            self.emc_turn(on);
        }
        self.emc_preview_actions(want_preview, cancel_preview, show_view, apply);
        if let Some((wire, amps)) = set_current {
            self.emc_set_current(wire, amps);
        }
        if let Some((key, value)) = set_setting {
            self.emc_set_setting(key, value);
        }
    }

    /// States (or clears) the current of a wire for the field estimate, one undo step.
    pub(super) fn emc_set_current(&mut self, wire: u64, amps: Option<f64>) {
        let id = novo_core::WireId::from(wire);
        self.edit_project("EMC: wire current", |tx| {
            tx.set_property(id, "emc.current_a", amps.map(novo_core::Value::Float))
        });
    }

    /// Sets or clears a project setting of the field estimate (`emc.<key>`), one undo step.
    pub(super) fn emc_set_setting(&mut self, key: &'static str, value: Option<novo_core::Value>) {
        self.edit_project("EMC: field setting", |tx| {
            tx.set_info(|info| match &value {
                Some(v) => {
                    info.properties.insert(format!("emc.{key}"), v.clone());
                }
                None => {
                    info.properties.remove(&format!("emc.{key}"));
                }
            })
        });
    }

    /// Turns the EMC rules on or off for the project (one undo step).
    pub(super) fn emc_turn(&mut self, on: bool) {
        self.edit_project("EMC check", |tx| emc::set_enabled(tx, on));
    }

    /// Works out what a suggestion would change, on a copy, for the window.
    pub(super) fn emc_preview(&mut self, fix: &Fix) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let result = emc::preview_on(project, fix).map_err(|e| {
            // A refusal is not corrupt data: drop the error type's prefix.
            let text = e.to_string();
            text.strip_prefix("invalid project data: ")
                .map_or(text.clone(), str::to_owned)
        });
        let revision = project.revision();
        self.emc.whatif = result.as_ref().ok().map(|(p, copy)| WhatIf {
            id: WHATIF_IDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            revision,
            project: std::sync::Arc::new(copy.clone()),
            summary: format!(
                "Findings {} before, {} after{}",
                p.findings_before,
                p.findings_after,
                match p.cleared.first() {
                    Some((rule, _)) => format!("; clears {rule}"),
                    None => match p.eased.first() {
                        Some(e) => format!("; eases {}", e.rule),
                        None => String::new(),
                    },
                }
            ),
            after: true,
        });
        let result = result.map(|(p, _)| p);
        self.emc.preview = Some((revision, fix.clone(), result));
        self.emc.done = None;
        self.emc.preview_fresh = true;
    }

    /// Opens a view that can draw the previewed fix, EMC layer on.
    fn emc_show_whatif(&mut self, ws: Workspace) {
        // The layer that shows what the fix changes goes on with the view.
        let fix = self.emc.preview.as_ref().map(|(_, f, _)| f.clone());
        let bonding = matches!(fix, Some(Fix::BarePlate { .. } | Fix::BondDevice { .. }));
        let shields = matches!(
            fix,
            Some(Fix::AddShield { .. } | Fix::Terminate360 { .. } | Fix::ShortenPigtail { .. })
        );
        let layers = match ws {
            Workspace::View3d => {
                self.view3d.controls.emc = true;
                &mut self.view3d.emc.layers
            }
            _ => {
                self.cabinet.emc.on = true;
                &mut self.cabinet.emc.layers
            }
        };
        layers.bonding |= bonding;
        layers.shields |= shields;
        // The window would cover the view it points to; Check > EMC check brings it back.
        self.emc.open = false;
        self.switch_workspace(ws);
    }

    /// Per frame: hands the previewed copy to the two views, and does what
    /// their before/after banner asked for. A preview whose project has
    /// since changed is dropped.
    pub(super) fn emc_whatif_sync(&mut self, ctx: &egui::Context) {
        let revision = self.backend.document().map(|d| d.project().revision());
        if self
            .emc
            .whatif
            .as_ref()
            .is_some_and(|w| Some(w.revision) != revision)
        {
            self.emc.whatif = None;
            self.emc.preview = None;
        }
        let action = self
            .view3d
            .emc
            .whatif_action
            .take()
            .or_else(|| self.cabinet.emc.whatif_action.take());
        match action {
            Some(WhatIfAction::Show { after }) => {
                if let Some(w) = &mut self.emc.whatif {
                    w.after = after;
                }
            }
            Some(WhatIfAction::Cancel) => {
                self.emc.whatif = None;
                self.emc.preview = None;
            }
            Some(WhatIfAction::Apply) => {
                if let Some((_, fix, _)) = self.emc.preview.take() {
                    self.emc.whatif = None;
                    self.emc_apply(&fix);
                }
            }
            None => {}
        }
        let hint = self.emc.preview.as_ref().map(|(_, f, _)| f.hint());
        let wanted = self.emc.whatif.as_ref().zip(hint.as_deref());
        // Rebuild the two views only when what they show changed.
        let same = |v: &Option<WhatIfView>| match (v, wanted) {
            (Some(v), Some((w, h))) => v.id == w.id && v.after == w.after && v.hint == h,
            (None, None) => true,
            _ => false,
        };
        let changed = !same(&self.view3d.emc.whatif) || !same(&self.cabinet.emc.whatif);
        if changed {
            let view = |(w, h): (&WhatIf, &str)| WhatIfView {
                id: w.id,
                project: w.project.clone(),
                hint: h.to_owned(),
                summary: w.summary.clone(),
                after: w.after,
            };
            self.view3d.emc.whatif = wanted.map(view);
            self.cabinet.emc.whatif = wanted.map(view);
        }
        if changed || action.is_some() {
            ctx.request_repaint();
        }
    }

    /// Applies one suggestion as one undo step and says what it did.
    pub(super) fn emc_apply(&mut self, fix: &Fix) {
        let mut said = String::new();
        let label = format!("EMC: {}", fix.hint());
        let ok = self.edit_project(&label, |tx| {
            said = emc::apply(tx, fix)?;
            Ok(())
        });
        self.emc.done = ok.then_some(said);
    }
}
