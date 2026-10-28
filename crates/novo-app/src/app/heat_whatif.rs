//! "Cooling what-if": a wizard that asks for the conditions (the hottest day
//! and how much of the loss is on), tries the fixes against each other in the
//! model and shows them ranked with a before and after table. The fix that is
//! picked is previewed in the 3D view and applied with one click.
//!
//! Where the user finds it: Cabinet > Heat strip > "Cooling what-if", the 3D
//! Heat panel, the plate's right-click menu, and the command palette
//! ("Cooling what-if wizard"). The run is on a worker thread in the 2D model
//! (`novo_thermal::whatif`); the window carries a "Model" badge and says what
//! the numbers are. Nothing is stored until "Apply".

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui::{self, Pos2, RichText};
use novo_core::{Project, StructureId};
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};
use novo_thermal::cooling::Product;
use novo_thermal::placement::Move;
use novo_thermal::placement_project::{plate_origin, rows};
use novo_thermal::sweep::{Candidate, Reading};
use novo_thermal::whatif::{
    self, BESIDE_NOTE, Choice, Conditions, Control, FAN_NOTE, Fix, MODEL_NOTE, Rails, WhatIf,
};

use super::heat_settings::HeatSettings;
use super::heat_sweep::{key_of, settings_for};

/// Width of the window's content, points: it fits beside the view cube at
/// 1366 px.
const WIDTH: f32 = 400.0;
/// Width of the window with its frame, points.
pub(super) const WINDOW_WIDTH: f32 = WIDTH + 24.0;
/// Tallest the results get before they scroll, points.
pub(super) const MAX_BODY: f32 = 470.0;
/// What the window takes besides the list: title, badge, conditions, the
/// headline above the list and the footnote below it.
pub(super) const CHROME: f32 = 250.0;
/// Lowest duty the slider offers, percent.
const MIN_DUTY_PCT: f64 = 10.0;

/// What a finished run found and what it needs to be applied.
#[derive(Clone)]
pub(super) struct Found {
    pub what_if: WhatIf,
    pub structure: StructureId,
    /// Device tags by id.
    tags: BTreeMap<u64, String>,
    /// Plate origin in the interior frame, mm.
    origin: (f64, f64),
    /// [`key_of`] when the run started; a different key is a changed cabinet.
    key: u64,
    /// The ambient the key was made with.
    ambient_c: f64,
    enclosure: novo_thermal::model::Enclosure,
}

struct Running {
    rx: mpsc::Receiver<Result<WhatIf, String>>,
    control: Arc<Control>,
    started: Instant,
    structure: StructureId,
    tags: BTreeMap<u64, String>,
    origin: (f64, f64),
    key: u64,
    ambient_c: f64,
    enclosure: novo_thermal::model::Enclosure,
}

/// What the app should do after the window.
pub(super) enum Ask {
    /// Move the devices as one undo step.
    Move(StructureId, Vec<Move>),
    /// Put the pair in the Vents and fans list.
    Openings(Candidate),
    /// Both, in that order (a combined fix).
    Both(StructureId, Vec<Move>, Candidate),
}

/// What the 3D view draws for the fix that is picked.
pub(super) struct Preview<'a> {
    /// Destination boxes of the moved devices, in the plate frame.
    pub moves: Vec<(u64, novo_layout::Rect)>,
    pub candidate: Option<&'a Candidate>,
}

/// State of the wizard and its window.
#[derive(Default)]
pub(super) struct Wizard {
    requested: bool,
    open: bool,
    /// The run starts on the next frame.
    go: bool,
    ambient_c: Option<f64>,
    duty_pct: Option<f64>,
    run: Option<Running>,
    pub(super) found: Option<Found>,
    notice: Option<String>,
    picked: Option<usize>,
    applied: Option<String>,
    /// A click on Apply, taken up by the app before the next frame.
    apply: Option<Ask>,
    /// The view the window opened in (1 Cabinet, 2 3D).
    view: Option<u8>,
    /// The option choice leaves devices out (set each frame by the view).
    pub(super) left_out: bool,
    #[cfg(test)]
    pub(super) settings: Option<whatif::Settings>,
}

impl Wizard {
    /// `heat.cooling_whatif`: open the window on the next frame.
    pub(super) fn request(&mut self) {
        self.requested = true;
        self.open = true;
    }

    /// Asked for and the 3D view has not taken it up.
    pub(super) fn pending(&self) -> bool {
        self.requested
    }

    pub(super) fn take_request(&mut self) -> bool {
        std::mem::take(&mut self.requested)
    }

    pub(super) fn take_apply(&mut self) -> Option<Ask> {
        self.apply.take()
    }

    pub(super) fn take_go(&mut self) -> bool {
        std::mem::take(&mut self.go)
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn running(&self) -> bool {
        self.run.is_some()
    }

    pub(super) fn close(&mut self) {
        if let Some(r) = &self.run {
            r.control.stop();
        }
        self.run = None;
        self.open = false;
        self.go = false;
        self.found = None;
        self.notice = None;
        self.picked = None;
        self.applied = None;
        self.view = None;
    }

    /// Conditions as typed, with the defaults of the cabinet.
    fn conditions(&self, default_ambient: f64) -> Conditions {
        Conditions {
            ambient_c: self.ambient_c.unwrap_or(default_ambient),
            duty: self.duty_pct.unwrap_or(100.0).clamp(MIN_DUTY_PCT, 100.0) / 100.0,
        }
    }

    /// What the 3D view should draw: the picked fix. `None` without a result
    /// or when the result is out of date.
    pub(super) fn preview(&self) -> Option<Preview<'_>> {
        let found = self.found.as_ref()?;
        let choice = found.what_if.choices.get(self.picked?)?;
        let moves = choice
            .fix
            .moves()
            .iter()
            .map(|m| {
                (
                    m.id,
                    novo_layout::Rect {
                        x: m.to.x - found.origin.0,
                        y: m.to.y - found.origin.1,
                        w: m.to.w,
                        h: m.to.h,
                    },
                )
            })
            .collect();
        Some(Preview {
            moves,
            candidate: choice.fix.candidate(),
        })
    }

    /// Starts the run from the cabinet as the project has it now, under the
    /// typed conditions. Says why when it cannot.
    pub(super) fn start(
        &mut self,
        project: &Project,
        structure: StructureId,
        default_ambient: f64,
        settings: &HeatSettings,
        fans: Vec<Product>,
        tag_of: &dyn Fn(u64) -> String,
    ) {
        if self.run.is_some() {
            return;
        }
        self.found = None;
        self.notice = None;
        self.applied = None;
        self.picked = None;
        let cond = self.conditions(default_ambient);
        let wires = super::heat_wires::wire_heat(project, structure);
        let options = BridgeOptions {
            ambient_c: cond.ambient_c,
            duct_loss_w: wires.by_duct,
            ..Default::default()
        };
        let mut scenario = match scenario_from_project(project, structure, &options) {
            Ok(b) => b.scenario,
            Err(e) => {
                self.notice = Some(format!(
                    "Cannot try: {e}. Set it in the Cabinet: Size enclosure or Plate setup"
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
        let key = key_of(project, structure, cond.ambient_c, settings, &fans);
        let tags = scenario
            .devices
            .iter()
            .map(|d| (d.id, tag_of(d.id)))
            .collect();
        let enclosure = scenario.enclosure.clone();
        let origin = plate_origin(project, structure).unwrap_or((0.0, 0.0));
        let layout_rows = rows(project, structure, &scenario)
            .filter(|r| !r.rows.is_empty() && r.row_of.iter().any(Option::is_some));
        let control = Arc::new(Control::default());
        let (tx, rx) = mpsc::channel();
        let c = control.clone();
        #[allow(unused_mut)]
        let mut run_settings = whatif::Settings {
            sweep: settings_for(&fans),
            ..whatif::Settings::default()
        };
        #[cfg(test)]
        if let Some(s) = self.settings.clone() {
            run_settings = s;
        }
        std::thread::spawn(move || {
            let rails = layout_rows.as_ref().map(|r| Rails {
                rows: &r.rows,
                row_of: &r.row_of,
            });
            let result =
                whatif::run(&scenario, &cond, rails, &run_settings, &c).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.run = Some(Running {
            rx,
            control,
            started: Instant::now(),
            structure,
            tags,
            origin,
            key,
            ambient_c: cond.ambient_c,
            enclosure,
        });
    }

    /// Takes in a finished run. Called every frame.
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        let Some(run) = &self.run else {
            return;
        };
        match run.rx.try_recv() {
            Ok(result) => {
                let run = self.run.take().expect("checked above");
                match result {
                    Ok(what_if) => {
                        // The best fix is previewed at once: the person sees
                        // an answer before they have to choose.
                        self.picked = (!what_if.choices.is_empty()).then_some(0);
                        self.found = Some(Found {
                            what_if,
                            structure: run.structure,
                            tags: run.tags,
                            origin: run.origin,
                            key: run.key,
                            ambient_c: run.ambient_c,
                            enclosure: run.enclosure,
                        });
                    }
                    Err(e) => self.notice = Some(format!("The what-if failed: {e}")),
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.run = None;
                self.notice = Some("The what-if stopped unexpectedly".into());
            }
        }
    }

    /// The ambient a stale check has to be made with.
    pub(super) fn ambient_used(&self) -> Option<f64> {
        self.found.as_ref().map(|f| f.ambient_c)
    }

    #[cfg(test)]
    pub(super) fn set_conditions_for_test(&mut self, ambient: Option<f64>, duty_pct: Option<f64>) {
        self.ambient_c = ambient;
        self.duty_pct = duty_pct;
    }

    #[cfg(test)]
    pub(super) fn set_found_for_test(&mut self, found: Found) {
        self.picked = (!found.what_if.choices.is_empty()).then_some(0);
        self.found = Some(found);
        self.open = true;
    }

    #[cfg(test)]
    pub(super) fn applied_text(&self) -> Option<String> {
        self.applied.clone()
    }

    #[cfg(test)]
    pub(super) fn picked(&self) -> Option<usize> {
        self.picked
    }

    #[cfg(test)]
    pub(super) fn pick(&mut self, i: usize) {
        self.picked = Some(i);
    }

    #[cfg(test)]
    pub(super) fn set_apply_for_test(&mut self, ask: Ask) {
        self.apply = Some(ask);
    }

    #[cfg(test)]
    pub(super) fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    #[cfg(test)]
    pub(super) fn applied(&self) -> Option<&str> {
        self.applied.as_deref()
    }

    /// Stops previewing: the fix is now in the project.
    pub(super) fn drop_picked(&mut self) {
        self.picked = None;
    }

    /// Says in the window what was applied.
    pub(super) fn set_applied(&mut self, text: String) {
        self.applied = Some(text);
    }

    pub(super) fn tag(&self, id: u64) -> String {
        self.found
            .as_ref()
            .and_then(|f| f.tags.get(&id))
            .filter(|t| !t.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("device {id}"))
    }

    /// The window. `now_key` is [`key_of`] for the cabinet as it is now, with
    /// the ambient of the result. `default_ambient` is the cabinet's own.
    pub(super) fn show(
        &mut self,
        ctx: &egui::Context,
        view: u8,
        now_key: Option<u64>,
        default_ambient: f64,
        at: Pos2,
        room: f32,
    ) {
        if !self.open {
            return;
        }
        match self.view {
            None => self.view = Some(view),
            Some(v) if v != view => {
                self.close();
                return;
            }
            Some(_) => {}
        }
        let mut ask = None;
        let mut open = true;
        let pal = crate::theme::ui_palette();
        egui::Window::new("Cooling what-if")
            .open(&mut open)
            // Folds to its title bar, so the preview behind it can be looked
            // at on a small screen.
            .collapsible(true)
            .resizable(false)
            // Hangs from its bottom left corner, so it grows upwards.
            .pivot(egui::Align2::LEFT_BOTTOM)
            .default_pos(at)
            .default_width(WIDTH)
            .max_width(WINDOW_WIDTH)
            .show(ctx, |ui| {
                ui.set_max_width(WIDTH);
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                super::heat_cooling::left_out_line(ui, self.left_out);
                model_badge(ui);
                if let Some(run) = &self.run {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label(format!(
                            "{}… {} runs so far, {:.0} s",
                            run.control.stage_name(),
                            run.control.evaluated(),
                            run.started.elapsed().as_secs_f64()
                        ));
                    });
                    if ui.button("Stop and keep what was found").clicked() {
                        run.control.stop();
                    }
                    return;
                }
                if let Some(n) = &self.notice {
                    ui.colored_label(pal.warning, n);
                }
                self.conditions_ui(ui, default_ambient);
                if let Some(a) = &self.applied {
                    ui.colored_label(pal.success, a);
                }
                let Some(found) = &self.found else {
                    return;
                };
                let stale = now_key.is_some_and(|k| k != found.key);
                if stale {
                    ui.colored_label(
                        pal.warning,
                        "The cabinet changed after this run, so the list is out of date. Try again before applying.",
                    );
                }
                let mut picked = self.picked;
                let mut copy = false;
                egui::ScrollArea::vertical()
                    .max_height(room)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        results(ui, self, found, &mut picked, stale, &mut ask, &mut copy);
                    });
                self.picked = picked;
                if copy {
                    ctx.copy_text(table_text(self, found));
                    self.applied = Some("The table is on the clipboard.".into());
                }
                ui.add_space(4.0);
                ui.label(RichText::new(MODEL_NOTE).small().color(pal.text_secondary));
            });
        if ask.is_some() {
            self.apply = ask;
        }
        if !open {
            self.close();
        }
    }

    /// The two questions and the button.
    fn conditions_ui(&mut self, ui: &mut egui::Ui, default_ambient: f64) {
        let pal = crate::theme::ui_palette();
        let mut ambient = self.ambient_c.unwrap_or(default_ambient);
        let mut duty = self.duty_pct.unwrap_or(100.0);
        egui::Grid::new("whatif.conditions")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("Hottest day outside");
                let a = ui.add(
                    egui::DragValue::new(&mut ambient)
                        .speed(0.5)
                        .range(-20.0..=70.0)
                        .suffix(" °C"),
                );
                a.on_hover_text(
                    "The air temperature outside the cabinet that it has to survive. Starts at the maximum ambient of the project.",
                );
                ui.end_row();
                ui.label("Duty (share of the loss that is on)");
                ui.add(
                    egui::Slider::new(&mut duty, MIN_DUTY_PCT..=100.0)
                        .suffix(" %")
                        .integer(),
                )
                .on_hover_text(
                    "100 % is every device at the loss you entered. A cabinet that only runs part of the time or at part load is cooler.",
                );
                ui.end_row();
            });
        self.ambient_c = Some(ambient);
        self.duty_pct = Some(duty);
        ui.horizontal(|ui| {
            let label = if self.found.is_some() {
                "Try again with these"
            } else {
                "Try the fixes"
            };
            let go = ui.button(label).on_hover_text(
                "Runs the cabinet under these conditions, then tries moving devices, vents and fans, both together, and a bigger enclosure",
            );
            super::cabinet_bar::note_ctx(ui.ctx(), "heat.whatif.go", "Try the fixes button", &go);
            if go.clicked() {
                self.go = true;
            }
            if self.found.is_none() {
                ui.label(
                    RichText::new("About a minute")
                        .small()
                        .color(pal.text_secondary),
                );
            }
        });
        ui.separator();
    }
}

/// The badge at the top: what kind of number this is.
fn model_badge(ui: &mut egui::Ui) {
    let pal = crate::theme::ui_palette();
    ui.horizontal(|ui| {
        egui::Frame::new()
            .fill(pal.accent_wash)
            .corner_radius(3.0)
            .inner_margin(egui::Margin::symmetric(6, 1))
            .show(ui, |ui| {
                ui.label(RichText::new("MODEL, NOT MEASURED").small().strong());
            });
        ui.label(
            RichText::new("Ranked by the 2D heat model. Nothing changes until you apply.")
                .small()
                .color(pal.text_secondary),
        );
    });
}

fn results(
    ui: &mut egui::Ui,
    w: &Wizard,
    found: &Found,
    picked: &mut Option<usize>,
    stale: bool,
    ask: &mut Option<Ask>,
    copy: &mut bool,
) {
    let pal = crate::theme::ui_palette();
    let wi = &found.what_if;
    ui.label(RichText::new(headline(wi)).strong());
    ui.label(format!(
        "Now: {} · {} · hottest air {:.0} °C",
        margin_text(wi.baseline.worst_margin_k),
        over_text(wi.baseline.over),
        wi.baseline.max_air_c
    ));
    if let Some(i) = hottest(&wi.baseline) {
        ui.label(
            RichText::new(format!(
                "Hottest housing: {} at {:.0} °C{}",
                w.tag(wi.device_ids[i]),
                wi.baseline.surface_c[i],
                match wi.baseline.margins_k[i] {
                    Some(m) => format!(" ({m:+.1} K to its limit)"),
                    None => String::new(),
                }
            ))
            .small()
            .color(pal.text_secondary),
        );
    }
    for (i, c) in wi.choices.iter().enumerate() {
        ui.add_space(6.0);
        ui.separator();
        card(ui, w, found, i, c, picked, stale, ask, copy);
    }
    for n in wi.notes.iter().filter(|n| *n != MODEL_NOTE) {
        ui.add_space(3.0);
        ui.label(RichText::new(n).small().color(pal.text_secondary));
    }
    ui.label(
        RichText::new(format!("{} runs of the model in all.", wi.tried))
            .small()
            .color(pal.text_secondary),
    );
}

#[allow(clippy::too_many_arguments)]
fn card(
    ui: &mut egui::Ui,
    w: &Wizard,
    found: &Found,
    index: usize,
    c: &Choice,
    picked: &mut Option<usize>,
    stale: bool,
    ask: &mut Option<Ask>,
    copy: &mut bool,
) {
    let pal = crate::theme::ui_palette();
    let wi = &found.what_if;
    let selected = *picked == Some(index);
    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(selected, format!("{}. {}", c.rank, c.fix.title()))
            .on_hover_text("Shows this fix in the 3D view as a preview. Nothing is stored")
            .clicked()
        {
            *picked = Some(index);
        }
        if c.solves {
            ui.colored_label(pal.success, "clears every limit");
        } else if c.clear {
            ui.colored_label(pal.success, format!("gains {:.1} K", c.gain_k));
        } else {
            ui.colored_label(
                pal.warning,
                format!("{:+.1} K: within the model's noise", c.gain_k),
            );
        }
    });
    ui.label(
        RichText::new(c.fix.effort_word())
            .small()
            .color(pal.text_secondary),
    );
    if !selected {
        ui.label(format!(
            "Tightest margin {} → {} · {}",
            margin_text(wi.baseline.worst_margin_k),
            margin_text(c.reading.worst_margin_k),
            over_text(c.reading.over)
        ));
        return;
    }
    ui.label(fix_description(w, found, &c.fix));
    before_after(ui, w, wi, c);
    if c.reading.unsettled {
        ui.label(
            RichText::new("This run did not settle fully; its numbers may move by about 1 K.")
                .small()
                .color(pal.warning),
        );
    }
    if has_fans(&c.fix) {
        ui.label(RichText::new(FAN_NOTE).small().color(pal.text_secondary));
        if let Some(t) = beside_text(w, wi, c) {
            ui.label(RichText::new(t).small().color(pal.warning));
        }
    }
    ui.label(
        RichText::new("Shown in the 3D view: green boxes are where devices would stand, new openings are drawn on their walls.")
            .small()
            .color(pal.text_secondary),
    );
    if ui
        .small_button("Copy the table")
        .on_hover_text("Puts the before and after table of this fix on the clipboard as text, for a report or a message")
        .clicked()
    {
        *copy = true;
    }
    if c.fix.applicable() {
        let text = match &c.fix {
            Fix::Layout { .. } => "Apply the moves",
            Fix::Openings { .. } => "Add the openings to Vents and fans",
            _ => "Apply the moves and add the openings",
        };
        let apply = ui.add_enabled(!stale, egui::Button::new(text)).on_hover_text(
            "One undo step for the moves; the openings go to the Vents and fans list of the 3D Heat panel, where Simulate checks them",
        );
        super::cabinet_bar::note_ctx(
            ui.ctx(),
            "heat.whatif.apply",
            "Apply button of the fix",
            &apply,
        );
        if apply.clicked() {
            *ask = Some(match &c.fix {
                Fix::Layout { moves } => Ask::Move(found.structure, moves.clone()),
                Fix::Openings { candidate } => Ask::Openings(candidate.clone()),
                Fix::LayoutAndOpenings { moves, candidate } => {
                    Ask::Both(found.structure, moves.clone(), candidate.clone())
                }
                Fix::BiggerEnclosure { .. } => return,
            });
        }
    } else {
        ui.label(
            RichText::new("Not applied for you: a different enclosure is a decision for Size the enclosure in the Cabinet.")
                .small()
                .color(pal.text_secondary),
        );
    }
}

/// How many devices stand beside the stream between the fan and the vent of a
/// fan fix, and whether the ones the fix is meant for do; `None` when none do.
fn beside_text(w: &Wizard, wi: &WhatIf, c: &Choice) -> Option<String> {
    if c.beside_stream.is_empty() {
        return None;
    }
    let beside = |id: u64| c.beside_stream.contains(&id);
    let mut out = format!(
        "Beside the assumed fan stream: {} of {} devices.",
        c.beside_stream.len(),
        wi.device_ids.len()
    );
    let over: Vec<u64> = wi
        .device_ids
        .iter()
        .zip(&wi.baseline.margins_k)
        .filter(|(_, m)| m.is_some_and(|m| m < 0.0))
        .map(|(id, _)| *id)
        .collect();
    if over.is_empty() {
        if let Some(id) = hottest(&wi.baseline)
            .map(|i| wi.device_ids[i])
            .filter(|&id| beside(id))
        {
            out.push_str(&format!(
                " The hottest device, {}, is beside it.",
                w.tag(id)
            ));
        }
    } else {
        let names: Vec<String> = over
            .iter()
            .copied()
            .filter(|&id| beside(id))
            .take(6)
            .map(|id| w.tag(id))
            .collect();
        if names.is_empty() {
            out.push_str(" None of the devices over their limit now is beside it by this rule; devices on the line can also differ.");
        } else {
            out.push_str(&format!(
                " Over their limit now and beside it: {} (the gain shown for them may be too high).",
                names.join(", ")
            ));
        }
    }
    out.push(' ');
    out.push_str(BESIDE_NOTE);
    Some(out)
}

/// Whether the fix adds a fan.
fn has_fans(fix: &Fix) -> bool {
    fix.candidate().is_some_and(|c| c.pairing.fans() > 0)
}

/// The words for what the fix does, with tags.
fn fix_description(w: &Wizard, found: &Found, fix: &Fix) -> String {
    let moves: Vec<String> = fix
        .moves()
        .iter()
        .take(6)
        .map(|m| {
            let to_rail = if m.to_row == m.from_row {
                "along its rail"
            } else {
                "to another rail"
            };
            format!(
                "{} {} ({:.0} mm)",
                w.tag(m.id),
                to_rail,
                (m.to.x - m.from.x).hypot(m.to.y - m.from.y)
            )
        })
        .collect();
    let mut out = String::new();
    if !moves.is_empty() {
        out.push_str(&format!("Move: {}", moves.join(", ")));
        if fix.moves().len() > 6 {
            out.push_str(&format!(" and {} more", fix.moves().len() - 6));
        }
        out.push('.');
    }
    if let Some(c) = fix.candidate() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&c.describe(&found.enclosure));
        out.push('.');
    }
    if let Fix::BiggerEnclosure { from, to } = fix {
        out.push_str(&format!(
            "From {:.0} x {:.0} x {:.0} mm to {:.0} x {:.0} x {:.0} mm outside.",
            from.width_mm, from.height_mm, from.depth_mm, to.width_mm, to.height_mm, to.depth_mm
        ));
    }
    out
}

/// The before and after table of one fix.
fn before_after(ui: &mut egui::Ui, w: &Wizard, wi: &WhatIf, c: &Choice) {
    let pal = crate::theme::ui_palette();
    let rows = table_rows(w, wi, c);
    egui::Grid::new(("whatif.table", c.rank))
        .num_columns(4)
        .spacing([10.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for h in ["", "Now", "With fix", "Change"] {
                ui.label(RichText::new(h).small().strong());
            }
            ui.end_row();
            for r in &rows {
                ui.label(RichText::new(&r.label).small());
                ui.label(RichText::new(&r.before).small());
                ui.label(RichText::new(&r.after).small());
                let color = match r.better {
                    Some(true) => pal.success,
                    Some(false) => pal.error,
                    None => pal.text_secondary,
                };
                ui.label(RichText::new(&r.change).small().color(color));
                ui.end_row();
            }
        });
    // Devices that gain most.
    let mut gains: Vec<(usize, f64)> = (0..wi.device_ids.len())
        .map(|i| (i, wi.baseline.surface_c[i] - c.reading.surface_c[i]))
        .filter(|(_, g)| *g >= 0.5)
        .collect();
    gains.sort_by(|a, b| b.1.total_cmp(&a.1));
    if !gains.is_empty() {
        let text: Vec<String> = gains
            .iter()
            .take(3)
            .map(|(i, g)| {
                format!(
                    "{} {:.0} → {:.0} °C",
                    w.tag(wi.device_ids[*i]),
                    wi.baseline.surface_c[*i],
                    wi.baseline.surface_c[*i] - g
                )
            })
            .collect();
        ui.label(
            RichText::new(format!("Housings that cool most: {}", text.join(", ")))
                .small()
                .color(pal.text_secondary),
        );
    }
}

/// One line of the table.
pub(super) struct Row {
    pub label: String,
    pub before: String,
    pub after: String,
    pub change: String,
    /// `Some(true)` when the change is for the better.
    pub better: Option<bool>,
}

pub(super) fn table_rows(w: &Wizard, wi: &WhatIf, c: &Choice) -> Vec<Row> {
    let (b, a) = (&wi.baseline, &c.reading);
    let mut rows = Vec::new();
    rows.push(Row {
        label: "Devices over limit".into(),
        before: b.over.to_string(),
        after: a.over.to_string(),
        change: signed_int(a.over as i64 - b.over as i64),
        better: dir(b.over as f64, a.over as f64, true),
    });
    if let (Some(x), Some(y)) = (b.worst_margin_k, a.worst_margin_k) {
        rows.push(Row {
            label: "Tightest margin".into(),
            before: format!("{x:+.1} K"),
            after: format!("{y:+.1} K"),
            change: format!("{:+.1} K", round1(y) - round1(x)),
            better: dir(x, y, false),
        });
    }
    if let Some(i) = hottest(b) {
        rows.push(Row {
            label: format!("{} housing", w.tag(wi.device_ids[i])),
            before: format!("{:.0} °C", b.surface_c[i]),
            after: format!("{:.0} °C", a.surface_c[i]),
            change: format!("{:+.0} K", a.surface_c[i] - b.surface_c[i]),
            better: dir(b.surface_c[i], a.surface_c[i], true),
        });
    }
    rows.push(Row {
        label: "Hottest air".into(),
        before: format!("{:.0} °C", b.max_air_c),
        after: format!("{:.0} °C", a.max_air_c),
        change: format!("{:+.0} K", a.max_air_c - b.max_air_c),
        better: dir(b.max_air_c, a.max_air_c, true),
    });
    rows.push(Row {
        label: "Mean air".into(),
        before: format!("{:.0} °C", b.mean_air_c),
        after: format!("{:.0} °C", a.mean_air_c),
        change: format!("{:+.0} K", a.mean_air_c - b.mean_air_c),
        better: dir(b.mean_air_c, a.mean_air_c, true),
    });
    rows
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn signed_int(v: i64) -> String {
    if v == 0 { "0".into() } else { format!("{v:+}") }
}

/// Whether the change from `before` to `after` is for the better; `lower`
/// says that a lower number is better. Changes under 0.5 are neither.
pub(super) fn dir(before: f64, after: f64, lower: bool) -> Option<bool> {
    let d = after - before;
    if d.abs() < 0.5 {
        None
    } else {
        Some((d < 0.0) == lower)
    }
}

/// The table of the picked fix as plain text.
pub(super) fn table_text(w: &Wizard, found: &Found) -> String {
    let wi = &found.what_if;
    let Some(c) = w.picked.and_then(|i| wi.choices.get(i)) else {
        return String::new();
    };
    let mut out = format!(
        "Cooling what-if (MODEL, NOT MEASURED)\nConditions: {:.0} °C outside, duty {:.0} %\nFix {}: {}\n{}\n\n",
        wi.conditions.ambient_c,
        100.0 * wi.conditions.duty,
        c.rank,
        c.fix.title(),
        fix_description(w, found, &c.fix)
    );
    out.push_str("Quantity\tNow\tWith fix\tChange\n");
    for r in table_rows(w, wi, c) {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            r.label, r.before, r.after, r.change
        ));
    }
    out.push('\n');
    out.push_str(MODEL_NOTE);
    if has_fans(&c.fix) {
        out.push('\n');
        out.push_str(FAN_NOTE);
        if let Some(t) = beside_text(w, wi, c) {
            out.push('\n');
            out.push_str(&t);
        }
    }
    out.push('\n');
    out
}

fn hottest(r: &Reading) -> Option<usize> {
    r.surface_c
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i)
}

pub(super) fn headline(wi: &WhatIf) -> String {
    if wi.choices.is_empty() {
        return if wi.baseline.worst_margin_k.is_some_and(|m| m >= 5.0) {
            "Nothing to fix under these conditions: every device has room to spare.".into()
        } else {
            "None of the fixes wins anything worth listing.".into()
        };
    }
    let best = &wi.choices[0];
    if best.solves {
        let thin = best.reading.worst_margin_k.is_some_and(|m| m < 2.0);
        format!(
            "{} ranks first: it clears every limit in the model and is the easiest fix that does{}.",
            best.fix.title(),
            if thin { ", with a thin margin" } else { "" }
        )
    } else if wi.baseline.over == 0 {
        format!(
            "The cabinet holds its limits; {} widens the margin the most.",
            best.fix.title().to_lowercase()
        )
    } else {
        format!(
            "No single fix clears every limit. {} gains the most ({:.1} K).",
            best.fix.title(),
            best.gain_k
        )
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

/// The destination of a moved device as overlays for the 3D view: a green
/// box where it would stand and a line from where it stands now. `now` is
/// the device's placement on the plate.
pub(super) fn move_overlays(
    id: u64,
    to: novo_layout::Rect,
    now: Option<&novo_layout::Placement>,
    frame: &novo_view3d::scene::PlateFrame,
    top_mm: f64,
) -> Vec<novo_view3d::Overlay> {
    use novo_view3d::scene::{ItemId, ItemKind};
    use novo_view3d::{Overlay, OverlayLines};
    let land = super::view3d_move::Landing {
        item: ItemId::new(ItemKind::Device, id),
        at: (to.x, to.y),
        rect: to,
        clash: false,
        // Green, like a device lined up on a rail.
        rail: Some(0),
        follower: false,
        blockers: Vec::new(),
    };
    let mut out = super::view3d_move::ghost(&land, frame, top_mm);
    if let Some(p) = now {
        let (w, h) = if p.rotated {
            (p.size.height, p.size.width)
        } else {
            (p.size.width, p.size.height)
        };
        let from = (p.x + 0.5 * w, p.y + 0.5 * h);
        let dest = (to.x + 0.5 * to.w, to.y + 0.5 * to.h);
        let z = top_mm.max(1.0);
        let edge = [110, 225, 155, 255];
        out.push(Overlay::Lines(OverlayLines {
            positions: vec![
                frame.world(from.0, from.1, z),
                frame.world(dest.0, dest.1, z),
            ],
            colors: vec![edge; 2],
            starts: Vec::new(),
            width_px: 2.0,
            order: 52,
        }));
    }
    out
}

impl super::NovoApp {
    /// Carries out the Apply the person pressed in the window. Runs before the
    /// 3D view borrows the project, so the edit can be made.
    pub(super) fn whatif_apply(&mut self) {
        let Some(ask) = self.cabinet.heat.whatif.take_apply() else {
            return;
        };
        let mut said = Vec::new();
        let mut moved = false;
        let with_openings = matches!(ask, Ask::Both(..));
        let (moves, openings) = match ask {
            Ask::Move(s, m) => (Some((s, m)), None),
            Ask::Openings(c) => (None, Some(c)),
            Ask::Both(s, m, c) => (Some((s, m)), Some(c)),
        };
        if let Some((structure, moves)) = moves {
            let count = moves.len();
            let mut skipped = Vec::new();
            if self.cabinet_edit("Cooling what-if", |tx| {
                skipped = novo_thermal::placement_project::apply_moves(tx, structure, &moves)?;
                Ok(())
            }) {
                moved = skipped.is_empty();
                said.push(if skipped.is_empty() {
                    format!(
                        "Moved {count} {}. Undo puts {} back.",
                        if count == 1 { "device" } else { "devices" },
                        if count == 1 { "it" } else { "them" }
                    )
                } else if with_openings {
                    format!(
                        "No device moved, because {} target rail(s) are gone; only the openings were added. Run it again for the layout.",
                        skipped.len()
                    )
                } else {
                    format!(
                        "Nothing moved: {} target rail(s) are gone. Try again.",
                        skipped.len()
                    )
                });
            }
        }
        if let Some(c) = openings {
            // Read the project's list first, so adding to a list the 3D view
            // has not looked at yet keeps what is already there.
            self.sync_openings_now();
            super::heat_sweep::add_pair(&mut self.cabinet.heat.settings, &c);
            self.sync_openings_now();
            self.cabinet.heat.on = true;
            said.push(super::heat_sweep::added_text(&c));
            if moved {
                said.push(
                    "The moves undo with Edit > Undo; the openings undo in the Vents and fans list."
                        .into(),
                );
            }
        }
        if !said.is_empty() {
            // The result no longer matches the cabinet, and the preview would
            // draw what is now real twice: the window asks for a new run.
            let w = &mut self.cabinet.heat.whatif;
            w.drop_picked();
            w.set_applied(said.join(" "));
        }
    }
}
