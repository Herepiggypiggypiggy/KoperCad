//! Build mode: the guided assembly and wiring of a cabinet at the plate.
//! Reports > Build mode (command `view.build`), also on the Cabinet bar.
//!
//! One step at a time in large type with one big Done button, the progress
//! of the cabinet, a traffic light per step, and the steps that can be done
//! now. It runs fully local: the plan is computed from the project
//! (`novo_commissioning::BuildPlan`), the ticks are kept in the project file
//! beside the commissioning results and travel in the same `.kcfield` file,
//! so a tablet without a connection works through a cabinet and the desk
//! merges the result. Meant for a Windows tablet or a laptop at the
//! cabinet; no phone app, no server.
//!
//! The wires are cut and the holes drilled by hand: the steps say what to
//! cut and drill, and what is a model or a guess is said on the step.

use std::rc::Rc;

use eframe::egui;
use novo_commissioning::{
    BuildPhase, BuildPlan, BuildStep, CabinetPlan, Next, StepStatus, claim_step, mark_done,
    mark_problem, release_step, undo_build,
};
use novo_core::WireId;
use novo_theme::widgets::{self, Level as Tone};

use super::commissioning::{TABLET_MAX_WIDTH, now, touch_density};
use super::{Level, NovoApp};

/// Windows narrower than this (points) get the one-column layout.
const WIDE: f32 = 880.0;
/// Width of the step list beside the card.
const LIST_WIDTH: f32 = 290.0;
/// The Done button's height on a touch screen.
const BIG_BUTTON: f32 = 64.0;

/// What Build mode keeps between frames.
pub(crate) struct BuildState {
    pub(super) plan: Option<(u64, Rc<BuildPlan>)>,
    /// Index into the plan's cabinets.
    cabinet: usize,
    /// The step the card shows; `None` follows the next step to do.
    focus: Option<usize>,
    /// The note being typed for a problem; `Some` while the editor is open.
    problem: Option<String>,
    /// Pictures chosen for the problem being typed; kept with the punch item
    /// when it is raised.
    photos: Vec<std::path::PathBuf>,
    /// Large buttons and text, on for a window the size of a tablet.
    large: bool,
    /// Build mode is stepped aside to show something in another workspace;
    /// a small bar brings it back.
    pub(super) away: bool,
    /// Steps ticked in this session, newest last, for Back.
    history: Vec<novo_commissioning::StepKey>,
}

impl BuildState {
    fn new(large: bool) -> Self {
        BuildState {
            plan: None,
            cabinet: 0,
            focus: None,
            problem: None,
            photos: Vec::new(),
            large,
            away: false,
            history: Vec::new(),
        }
    }
}

/// Where a step's object is shown, in another workspace.
#[derive(Clone, Copy)]
enum Show {
    Wire3d(WireId),
    DevicePlate(u64),
    Device3d(u64),
}

/// What the screen asked for this frame.
#[derive(Default)]
struct Asked {
    close: bool,
    export: bool,
    import: bool,
    sheet: bool,
    show: Option<Show>,
    /// "Add photos…" was pressed in the problem editor.
    pick_photos: bool,
    /// Punch item raised with pictures to keep on it.
    attach: Option<(u64, Vec<std::path::PathBuf>)>,
    /// A step was ticked, taken back or parked: the store changed.
    changed: bool,
}

fn tone_of(status: StepStatus) -> Tone {
    match status {
        StepStatus::Open => Tone::Info,
        StepStatus::Done => Tone::Success,
        StepStatus::Problem => Tone::Error,
        StepStatus::Changed => Tone::Warning,
    }
}

/// The light in the list: a filled circle the colour of the status, and a
/// word on hover, so colour is never the only sign.
fn light(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Open => "○",
        StepStatus::Done => "✔",
        StepStatus::Problem => "✖",
        StepStatus::Changed => "▲",
    }
}

fn status_word(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Open => "Open",
        StepStatus::Done => "Done",
        StepStatus::Problem => "Problem raised",
        StepStatus::Changed => "Changed since it was ticked",
    }
}

impl NovoApp {
    /// Reports > Build mode: opens the screen over the workspace.
    pub(super) fn open_build_mode(&mut self) {
        if !self.is_open() {
            return;
        }
        // The commissioning workspace keeps its own copy of the store.
        if self.workspace() == super::workspace::Workspace::Commissioning {
            self.switch_workspace(super::workspace::Workspace::Cabinet);
        }
        self.load_commissioning();
        let tablet = self.window_width > 0.0 && self.window_width < TABLET_MAX_WIDTH;
        self.wb.build = Some(BuildState::new(self.settings.large_buttons || tablet));
    }

    /// Draws Build mode over the central area while it is open.
    pub(super) fn build_mode(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        let Some(mut state) = self.wb.build.take() else {
            return;
        };
        if !self.is_open() || self.workspace() == super::workspace::Workspace::Commissioning {
            return;
        }
        self.load_commissioning();
        let mut asked = Asked::default();
        let who = self
            .wb
            .commissioning
            .as_ref()
            .map(|m| m.who.clone())
            .unwrap_or_default();
        let name = self.project_name().unwrap_or_default();
        if let (Some(doc), Some(mode)) = (self.backend.document(), self.wb.commissioning.as_mut()) {
            let project = doc.project();
            let stale = state
                .plan
                .as_ref()
                .is_none_or(|(rev, _)| *rev != project.revision());
            if stale {
                let plan = BuildPlan::of(project);
                // Ticks of steps the design no longer has are kept: they are
                // the record of work done, and an undo can bring the step back.
                state.cabinet = state.cabinet.min(plan.cabinets.len().saturating_sub(1));
                state.plan = Some((project.revision(), Rc::new(plan)));
            }
            let plan = state.plan.as_ref().map(|(_, p)| Rc::clone(p));
            if state.away {
                let step = plan
                    .as_ref()
                    .and_then(|p| p.cabinets.get(state.cabinet))
                    .and_then(|c| {
                        match state.focus.or_else(|| {
                            match c.next_for(&mode.store, or_unknown(&mode.who)) {
                                Next::Ready(n) | Next::Blocked { step: n, .. } => Some(n),
                                Next::Finished => None,
                            }
                        }) {
                            Some(n) => c.step(n).map(|s| format!("Step {n}: {}", s.title)),
                            None => None,
                        }
                    })
                    .unwrap_or_default();
                let back = away_bar(ctx, rect, &step);
                if back {
                    state.away = false;
                }
            } else if let Some(plan) = plan {
                let p = crate::theme::ui_palette();
                let id = egui::Id::new("build_mode");
                egui::Area::new(id)
                    .order(egui::Order::Middle)
                    .fixed_pos(rect.min)
                    .show(ctx, |ui| {
                        ui.set_clip_rect(rect);
                        ui.painter().rect_filled(rect, 0.0, p.panel);
                        // Swallows the pointer, so nothing behind is touched.
                        ui.interact(rect, id.with("shield"), egui::Sense::click_and_drag());
                        let inner = rect.shrink(14.0);
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            ui.set_min_size(inner.size());
                            touch_density(ui, state.large);
                            screen(ui, &mut state, &plan, mode, &who, &name, &mut asked);
                        });
                    });
                if ctx.input(|i| i.key_pressed(egui::Key::Escape))
                    && !ctx.egui_wants_keyboard_input()
                {
                    asked.close = true;
                }
            }
            if mode.dirty {
                mode.dirty = false;
                let json = mode.store.to_json();
                self.backend.set_commissioning_json(&json);
            }
        }
        if asked.pick_photos
            && let Some(paths) = Self::pick_photo_files()
        {
            for path in paths {
                if !state.photos.contains(&path) {
                    state.photos.push(path);
                }
            }
        }
        if let Some((id, paths)) = asked.attach.take() {
            self.keep_problem_photos(id, &paths);
        }
        if asked.export {
            self.export_field_results();
        }
        if asked.sheet {
            let plan = state.plan.as_ref().map(|(_, p)| Rc::clone(p));
            if let Some(plan) = plan {
                self.export_build_sheet(&plan);
            }
        }
        if asked.import {
            self.import_field_results();
            // Ticks came in: draw the card again from the merged store.
        }
        if let Some(show) = asked.show {
            // Build mode steps aside (a bar brings it back) so the other
            // workspace can be seen.
            state.away = true;
            let tag = state
                .plan
                .as_ref()
                .and_then(|(_, p)| p.cabinets.get(state.cabinet))
                .map(|c| c.tag.clone())
                .unwrap_or_default();
            match show {
                Show::Wire3d(wire) => self.show_wire_in_3d(wire),
                Show::DevicePlate(device) => self.show_device(device, &tag, false),
                Show::Device3d(device) => self.show_device(device, &tag, true),
            }
            self.wb.build = Some(state);
            return;
        }
        if asked.changed {
            self.message_if_finished(&state);
        }
        if !asked.close {
            self.wb.build = Some(state);
        }
    }

    /// A tap of Space while Build mode is open ticks the step on the card.
    /// Returns whether it was taken (otherwise Space opens the command
    /// field as everywhere).
    pub(super) fn build_space(&mut self) -> bool {
        let Some(mut state) = self.wb.build.take() else {
            return false;
        };
        // Typing a problem note: the space belongs to the text; stepped
        // aside, Build mode is not on screen.
        let mut taken = state.problem.is_some() || state.away;
        if !taken {
            let who = self
                .wb
                .commissioning
                .as_ref()
                .map(|m| m.who.clone())
                .unwrap_or_default();
            if let (Some((_, plan)), Some(mode)) =
                (state.plan.clone(), self.wb.commissioning.as_mut())
                && let Some(cab) = plan.cabinets.get(state.cabinet)
            {
                let number = state.focus.filter(|&n| cab.step(n).is_some()).or(
                    match cab.next_for(&mode.store, or_unknown(&who)) {
                        Next::Ready(n) | Next::Blocked { step: n, .. } => Some(n),
                        Next::Finished => None,
                    },
                );
                if let Some(step) = number.and_then(|n| cab.step(n)) {
                    mark_done(&mut mode.store, step, &who, &now());
                    state.history.push(step.key);
                    state.focus = None;
                    mode.dirty = true;
                    taken = true;
                }
            }
            if taken {
                self.flush_commissioning();
            }
        }
        self.wb.build = Some(state);
        taken
    }

    /// Keeps the pictures chosen in the problem editor on the punch item the
    /// problem raised, the same way the Commissioning workspace does.
    pub(super) fn keep_problem_photos(&mut self, id: u64, paths: &[std::path::PathBuf]) -> usize {
        // Flush first: the item must be in the store the photos go to.
        self.flush_commissioning();
        let added = self.attach_photo_files(id, paths);
        self.flush_commissioning();
        added
    }

    /// Writes the commissioning data to the project when it changed.
    fn flush_commissioning(&mut self) {
        if let Some(mode) = self.wb.commissioning.as_mut()
            && mode.dirty
        {
            mode.dirty = false;
            let json = mode.store.to_json();
            self.backend.set_commissioning_json(&json);
        }
    }

    /// Writes the build sheet PDF next to the project (`<project> build
    /// sheet.pdf`), replacing only a file of this program's own making.
    pub(super) fn export_build_sheet(&mut self, plan: &BuildPlan) -> Option<std::path::PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = super::pdf::replaceable_path(&dir.join(format!("{stem} build sheet.pdf")));
        self.export_build_sheet_to(plan, &path)
    }

    pub(super) fn export_build_sheet_to(
        &mut self,
        plan: &BuildPlan,
        path: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        self.load_commissioning();
        let name = self.machine_title().unwrap_or_default();
        let revision = self
            .backend
            .document()
            .map_or(0, |d| d.project().revision());
        let store = self.wb.commissioning.as_ref().map(|m| m.store.clone())?;
        let output = novo_commissioning::build_sheet(plan, &store, revision);
        if output.sections.is_empty() {
            self.message(
                Level::Warning,
                "No cabinet has a mounting plate yet, so there is no build sheet.",
            );
            return None;
        }
        let options = novo_pdf::ReportPdfOptions {
            title: format!("{name}  Build sheet"),
            row_mm: output
                .sections
                .iter()
                .map(|s| (s.key.clone(), novo_commissioning::build_sheet_row_mm(s)))
                .collect(),
            language: self.report_translator(),
            ..novo_pdf::ReportPdfOptions::default()
        };
        let bytes = novo_pdf::export_report_pdf(&[&output], &options);
        let replaced = path.exists();
        match super::pdf::write_replacing(path, &bytes) {
            Ok(()) => {
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message_file(format!("{verb} {}", path.display()), path);
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not export the build sheet: {e}"),
                );
                None
            }
        }
    }

    /// Selects the device on the Cabinet canvas and shows it there or, with
    /// `three_d`, in the 3D view (which follows the plate's selection). The
    /// Cabinet switches to the step's own cabinet (`tag`), its plate and not
    /// the table face, so a project with several cabinets shows the right one.
    pub(super) fn show_device(&mut self, device: u64, tag: &str, three_d: bool) {
        self.flush_commissioning();
        if let Ok(location) = novo_core::DeviceTag::parse(tag) {
            self.cabinet.location = Some(location);
        }
        self.cabinet.surface = None;
        self.cabinet.table = false;
        self.cabinet.view.selection.apply(
            [novo_canvas::cabinet::box_item(device)],
            novo_canvas::selection::SelectOp::Replace,
        );
        self.cabinet.rails.selected = None;
        self.switch_workspace(if three_d {
            super::workspace::Workspace::View3d
        } else {
            super::workspace::Workspace::Cabinet
        });
    }

    fn message_if_finished(&mut self, state: &BuildState) {
        let (Some((_, plan)), Some(mode)) = (state.plan.as_ref(), self.wb.commissioning.as_ref())
        else {
            return;
        };
        let Some(cab) = plan.cabinets.get(state.cabinet) else {
            return;
        };
        if cab.next(&mode.store) == Next::Finished && cab.progress(&mode.store).problems == 0 {
            let text = format!(
                "{} is built. Export the field results to carry the ticks back to the desk.",
                cab.tag
            );
            self.message(Level::Info, text);
        }
    }
}

fn screen(
    ui: &mut egui::Ui,
    state: &mut BuildState,
    plan: &BuildPlan,
    mode: &mut super::commissioning::CommissioningMode,
    who: &str,
    project_name: &str,
    asked: &mut Asked,
) {
    let p = crate::theme::ui_palette();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Build mode")
                .heading()
                .color(p.text_primary),
        );
        if plan.cabinets.len() > 1 {
            let mut chosen = state.cabinet;
            egui::ComboBox::from_id_salt("build_cabinet")
                .selected_text(plan.cabinets[state.cabinet].tag.clone())
                .show_ui(ui, |ui| {
                    for (i, c) in plan.cabinets.iter().enumerate() {
                        ui.selectable_value(&mut chosen, i, c.tag.clone());
                    }
                });
            if chosen != state.cabinet {
                state.cabinet = chosen;
                state.focus = None;
                state.problem = None;
                state.photos.clear();
            }
        } else if let Some(c) = plan.cabinets.first() {
            ui.label(egui::RichText::new(&c.tag).strong());
        }
        ui.label(egui::RichText::new(project_name).color(p.text_secondary));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::ghost_button(ui, "Close").clicked() {
                asked.close = true;
            }
            if widgets::ghost_button(ui, "Print sheet…")
                .on_hover_text("A PDF of every step with its tick, for the workshop wall")
                .clicked()
            {
                asked.sheet = true;
            }
            if widgets::ghost_button(ui, "Import ticks…")
                .on_hover_text("Merge a .kcfield file from the tablet; the newer decision wins")
                .clicked()
            {
                asked.import = true;
            }
            if widgets::ghost_button(ui, "Export ticks…")
                .on_hover_text("Write a .kcfield file with the ticks, to carry to the desk or back")
                .clicked()
            {
                asked.export = true;
            }
            ui.checkbox(&mut state.large, "Large");
        });
    });
    ui.add_space(6.0);

    let Some(cab) = plan.cabinets.get(state.cabinet) else {
        empty(ui);
        return;
    };
    let progress = cab.progress(&mode.store);
    widgets::progress_bar(
        ui,
        &format!("Building {}", cab.tag),
        &format!(
            "{} of {} steps{}{}",
            progress.done,
            progress.total,
            if progress.problems > 0 {
                format!(
                    ", {} problem{}",
                    progress.problems,
                    if progress.problems == 1 { "" } else { "s" }
                )
            } else {
                String::new()
            },
            if progress.changed > 0 {
                format!(", {} changed", progress.changed)
            } else {
                String::new()
            }
        ),
        progress.fraction(),
        if progress.total == 0 {
            0.0
        } else {
            progress.problems as f32 / progress.total as f32
        },
    );
    ui.add_space(8.0);

    let next = cab.next_for(&mode.store, or_unknown(who));
    let shown = state
        .focus
        .filter(|&n| cab.step(n).is_some())
        .or(match next {
            Next::Ready(n) => Some(n),
            Next::Blocked { step, .. } => Some(step),
            Next::Finished => None,
        })
        .or_else(|| cab.steps.len().checked_sub(0).filter(|&n| n > 0));
    let wide = ui.available_width() >= WIDE;
    if wide {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(LIST_WIDTH, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| step_list(ui, state, cab, mode, shown),
            );
            ui.add_space(10.0);
            ui.vertical(|ui| card(ui, state, cab, mode, shown, next, who, asked));
        });
    } else {
        egui::ScrollArea::vertical().show(ui, |ui| {
            card(ui, state, cab, mode, shown, next, who, asked);
            ui.add_space(10.0);
            ui.collapsing("All steps", |ui| step_list(ui, state, cab, mode, shown));
        });
    }
}

fn empty(ui: &mut egui::Ui) {
    widgets::empty_state(
        ui,
        widgets::EmptySize::Panel,
        novo_theme::icons::status::INFO,
        "Nothing to build yet",
        "Build mode needs a cabinet with a mounting plate. Create one on the Cabinet canvas, place the devices, and come back.",
        |_| {},
    );
}

/// The steps grouped by phase, with a count per phase and a light per step.
fn step_list(
    ui: &mut egui::Ui,
    state: &mut BuildState,
    cab: &CabinetPlan,
    mode: &super::commissioning::CommissioningMode,
    shown: Option<usize>,
) {
    let p = crate::theme::ui_palette();
    let focus_phase = shown.and_then(|n| cab.step(n)).map(|s| s.phase);
    egui::ScrollArea::vertical()
        .id_salt("build_steps")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for phase in BuildPhase::ALL {
                let counts = cab.phase_progress(&mode.store, phase);
                if counts.total == 0 {
                    continue;
                }
                let head = format!("{}  {}/{}", phase.name(), counts.done, counts.total);
                let open = Some(phase) == focus_phase;
                egui::CollapsingHeader::new(egui::RichText::new(head).strong().color(
                    if counts.done == counts.total {
                        p.success
                    } else {
                        p.text_primary
                    },
                ))
                .id_salt(("build_phase", cab.cabinet, phase.name()))
                .open(open.then_some(true))
                .show(ui, |ui| {
                    for step in cab.steps.iter().filter(|s| s.phase == phase) {
                        let status = cab.status(&mode.store, step);
                        let selected = Some(step.number) == shown;
                        let r = widgets::row(ui, selected, |ui| {
                            ui.label(
                                egui::RichText::new(light(status)).color(tone_of(status).color(p)),
                            );
                            ui.add(
                                egui::Label::new(egui::RichText::new(format!(
                                    "{}  {}{}",
                                    step.number,
                                    short(step),
                                    claimed_by(mode, step, status)
                                )))
                                .truncate(),
                            );
                        })
                        .on_hover_text(format!(
                            "{}: {}",
                            status_word(status),
                            step.title
                        ));
                        if r.clicked() {
                            state.focus = Some(step.number);
                            state.problem = None;
                            state.photos.clear();
                        }
                    }
                });
            }
        });
}

/// " · bob" on an open step somebody has taken.
fn claimed_by(
    mode: &super::commissioning::CommissioningMode,
    step: &BuildStep,
    status: StepStatus,
) -> String {
    match mode.store.build_claim(step.key) {
        Some(c) if matches!(status, StepStatus::Open | StepStatus::Changed) => {
            format!("  · {}", c.by)
        }
        _ => String::new(),
    }
}

/// What a list row says: the mark, or the start of the instruction.
fn short(step: &BuildStep) -> String {
    if step.mark.is_empty() {
        step.title.clone()
    } else {
        step.mark.clone()
    }
}

#[allow(clippy::too_many_arguments)]
fn card(
    ui: &mut egui::Ui,
    state: &mut BuildState,
    cab: &CabinetPlan,
    mode: &mut super::commissioning::CommissioningMode,
    shown: Option<usize>,
    next: Next,
    who: &str,
    asked: &mut Asked,
) {
    let p = crate::theme::ui_palette();
    let Some(step) = shown.and_then(|n| cab.step(n)) else {
        empty(ui);
        return;
    };
    let status = cab.status(&mode.store, step);
    let big = if state.large { 1.0 } else { 0.8 };
    egui::ScrollArea::vertical()
        .id_salt("build_card")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Step {} of {} · {}",
                    step.number,
                    cab.steps.len(),
                    step.phase.name()
                ))
                .color(p.text_secondary),
            );
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&step.title)
                        .size(28.0 * big)
                        .strong()
                        .color(p.text_primary),
                )
                .wrap(),
            );
            if !step.mark.is_empty() {
                ui.add_space(2.0);
                widgets::chip(ui, &step.mark, true);
            }
            ui.add_space(8.0);
            for line in &step.details {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(line)
                            .size(18.0 * big)
                            .color(p.text_primary),
                    )
                    .wrap(),
                );
            }
            if let Some(caution) = &step.caution {
                ui.add_space(8.0);
                widgets::banner(ui, Tone::Warning, caution, |_| {});
            }
            // What this step waits for.
            let waiting: Vec<usize> = step
                .after
                .iter()
                .copied()
                .filter(|&a| {
                    cab.step(a)
                        .is_some_and(|s| cab.status(&mode.store, s) != StepStatus::Done)
                })
                .collect();
            if !waiting.is_empty() && status != StepStatus::Done {
                ui.add_space(8.0);
                widgets::banner(
                    ui,
                    Tone::Info,
                    &format!(
                        "Waits for {} step{} still open: {}",
                        waiting.len(),
                        if waiting.len() == 1 { "" } else { "s" },
                        ranges(&waiting)
                    ),
                    |ui| {
                        if let Some(&first) = waiting.first()
                            && widgets::ghost_button(ui, "Go there").clicked()
                        {
                            state.focus = Some(first);
                        }
                    },
                );
            }
            ui.add_space(10.0);
            status_and_buttons(ui, state, cab, mode, step, status, who, asked);
            // Real buttons, in the size of the others: a bare text link is
            // easy to miss and too small for a finger.
            let h = if state.large { 48.0 } else { 30.0 };
            let show = |ui: &mut egui::Ui, label: &str| {
                // The width follows the (possibly translated) label.
                ui.add(egui::Button::new(label).min_size(egui::vec2(0.0, h)))
                    .clicked()
            };
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if let Some(device) = step.device {
                    if show(ui, "Show on the plate") {
                        asked.show = Some(Show::DevicePlate(device));
                    }
                    if show(ui, "Show in 3D") {
                        asked.show = Some(Show::Device3d(device));
                    }
                }
                if let Some(&w) = step.wires.first()
                    && show(ui, "Show the wire in 3D")
                {
                    asked.show = Some(Show::Wire3d(w));
                }
            });
            ready_now(ui, state, cab, mode, step.number);
            if next == Next::Finished {
                ui.add_space(10.0);
                let problems = cab.progress(&mode.store).problems;
                widgets::banner(
                    ui,
                    if problems == 0 {
                        Tone::Success
                    } else {
                        Tone::Warning
                    },
                    &if problems == 0 {
                        format!(
                            "{} is built. Export the ticks to hand the result back.",
                            cab.tag
                        )
                    } else {
                        format!(
                            "Everything else is done; {problems} problem{} to settle.",
                            if problems == 1 { "" } else { "s" }
                        )
                    },
                    |_| {},
                );
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn status_and_buttons(
    ui: &mut egui::Ui,
    state: &mut BuildState,
    cab: &CabinetPlan,
    mode: &mut super::commissioning::CommissioningMode,
    step: &BuildStep,
    status: StepStatus,
    who: &str,
    asked: &mut Asked,
) {
    let p = crate::theme::ui_palette();
    let tick = mode.store.build_tick(step.key).cloned();
    match (status, &tick) {
        (StepStatus::Done, Some(t)) => {
            widgets::banner(
                ui,
                Tone::Success,
                &format!("Done by {} at {}", or_unknown(&t.by), t.at),
                |ui| {
                    if widgets::ghost_button(ui, "Undo").clicked() {
                        undo_build(&mut mode.store, step.key);
                        mode.dirty = true;
                        asked.changed = true;
                    }
                },
            );
            return;
        }
        (StepStatus::Problem, Some(t)) => {
            widgets::banner(
                ui,
                Tone::Error,
                &format!(
                    "Problem raised by {} at {}: {}",
                    or_unknown(&t.by),
                    t.at,
                    t.note
                ),
                |ui| {
                    if widgets::ghost_button(ui, "Undo").clicked() {
                        undo_build(&mut mode.store, step.key);
                        mode.dirty = true;
                        asked.changed = true;
                    }
                },
            );
            ui.label(
                egui::RichText::new("It is on the punch list in Commissioning.")
                    .color(p.text_secondary),
            );
            return;
        }
        (StepStatus::Changed, Some(t)) => {
            widgets::banner(
                ui,
                Tone::Warning,
                &format!(
                    "Ticked by {} at {}, but the drawing changed since. Look again and confirm.",
                    or_unknown(&t.by),
                    t.at
                ),
                |_| {},
            );
            ui.add_space(6.0);
        }
        _ => {}
    }

    // Who has this step: two people on one cabinet do not reach for the same
    // device. A claim is a signal, not a lock. Without a name (set under
    // Commissioning) the claim reads "someone", which cannot tell two people apart.
    if state.problem.is_none() && matches!(status, StepStatus::Open | StepStatus::Changed) {
        let me = or_unknown(who);
        let held = mode.store.build_claim(step.key).cloned();
        let h = if state.large { 48.0 } else { 30.0 };
        match held {
            Some(c) if c.by != me => {
                widgets::banner(
                    ui,
                    Tone::Warning,
                    &format!(
                        "{} took this step at {}. Do another step, or take it over.",
                        c.by,
                        c.at.get(11..16).unwrap_or(&c.at)
                    ),
                    |_| {},
                );
                ui.add_space(4.0);
                if ui
                    .add(egui::Button::new("Take it over").min_size(egui::vec2(0.0, h)))
                    .clicked()
                {
                    claim_step(&mut mode.store, step, me, &now());
                    mode.dirty = true;
                    asked.changed = true;
                }
                ui.add_space(6.0);
            }
            Some(_) => {
                ui.horizontal(|ui| {
                    ui.label("You have this step.");
                    if ui.small_button("Let go").clicked() {
                        release_step(&mut mode.store, step, &now());
                        mode.dirty = true;
                        asked.changed = true;
                    }
                });
                ui.add_space(4.0);
            }
            None => {
                let r = ui
                    .add(egui::Button::new("I take this step").min_size(egui::vec2(0.0, h)))
                    .on_hover_text(if who.trim().is_empty() {
                        "Shows the others that someone is on this step. Set your name in Commissioning so they see who."
                    } else {
                        "Shows the others that you are on this step"
                    });
                if r.clicked() {
                    claim_step(&mut mode.store, step, me, &now());
                    mode.dirty = true;
                    asked.changed = true;
                }
                ui.add_space(4.0);
            }
        }
    }

    if let Some(note) = state.problem.as_mut() {
        ui.label("What is wrong? Engineering reads this on the punch list.");
        let r = ui.add(
            egui::TextEdit::multiline(note)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        r.request_focus();
        let text = note.trim().to_owned();
        let (mut raise, mut cancel) = (false, false);
        ui.horizontal(|ui| {
            raise = ui
                .add_enabled(!text.is_empty(), egui::Button::new("Raise problem"))
                .clicked();
            cancel = ui.button("Cancel").clicked();
            if ui
                .button("📷  Add photos…")
                .on_hover_text("Pictures from a camera or the tablet's gallery")
                .clicked()
            {
                asked.pick_photos = true;
            }
        });
        if !state.photos.is_empty() {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} photo{} will be kept on the punch item",
                    state.photos.len(),
                    if state.photos.len() == 1 { "" } else { "s" }
                ));
                if ui.small_button("Remove").clicked() {
                    state.photos.clear();
                }
            });
        }
        if raise {
            let id = mark_problem(&mut mode.store, &cab.tag, step, &text, who, &now());
            if !state.photos.is_empty() {
                asked.attach = Some((id, std::mem::take(&mut state.photos)));
            }
            mode.dirty = true;
            asked.changed = true;
            state.problem = None;
            state.focus = None;
        } else if cancel {
            state.problem = None;
            state.photos.clear();
        }
        return;
    }

    let width = ui.available_width();
    let height = if state.large { BIG_BUTTON } else { 40.0 };
    let label = if status == StepStatus::Changed {
        "✔  Confirm again"
    } else {
        "✔  Done"
    };
    let done = ui.add_sized(
        [width, height],
        egui::Button::new(
            egui::RichText::new(label)
                .size(22.0)
                .strong()
                .color(p.on_accent),
        )
        .fill(p.accent),
    );
    let space_done = ui.input(|i| i.key_pressed(egui::Key::Enter))
        && !ui.ctx().egui_wants_keyboard_input()
        && ui.ctx().memory(|m| m.focused().is_none());
    if done.clicked() || space_done {
        mark_done(&mut mode.store, step, who, &now());
        state.history.push(step.key);
        mode.dirty = true;
        asked.changed = true;
        state.focus = None;
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let h = if state.large { 48.0 } else { 30.0 };
        if ui
            .add_sized([140.0, h], egui::Button::new("⚠  Problem…"))
            .clicked()
        {
            state.problem = Some(String::new());
        }
        let can_back = state.history.last().is_some();
        if ui
            .add_enabled_ui(can_back, |ui| {
                ui.add_sized([140.0, h], egui::Button::new("Back"))
                    .on_hover_text("Takes the last tick back")
            })
            .inner
            .clicked()
            && let Some(key) = state.history.pop()
        {
            undo_build(&mut mode.store, key);
            mode.dirty = true;
            asked.changed = true;
            state.focus = cab.find(key).map(|s| s.number);
        }
        if ui
            .add_sized([48.0, h], egui::Button::new("◀"))
            .on_hover_text("Previous step")
            .clicked()
        {
            state.focus = Some(step.number.saturating_sub(1).max(1));
        }
        if ui
            .add_sized([48.0, h], egui::Button::new("▶"))
            .on_hover_text("Next step")
            .clicked()
        {
            state.focus = Some((step.number + 1).min(cab.steps.len()));
        }
    });
}

fn or_unknown(who: &str) -> &str {
    if who.is_empty() { "someone" } else { who }
}

/// The steps that can be done now besides the one shown, so two people can
/// work on different devices at once.
fn ready_now(
    ui: &mut egui::Ui,
    state: &mut BuildState,
    cab: &CabinetPlan,
    mode: &super::commissioning::CommissioningMode,
    current: usize,
) {
    let ready: Vec<usize> = cab
        .ready(&mode.store)
        .into_iter()
        .filter(|&n| n != current)
        .collect();
    if ready.is_empty() {
        return;
    }
    ui.add_space(14.0);
    widgets::section_label(ui, "Also ready now");
    ui.horizontal_wrapped(|ui| {
        for &n in ready.iter().take(12) {
            let Some(step) = cab.step(n) else { continue };
            if widgets::chip(
                ui,
                &format!(
                    "{n} {}{}",
                    short(step),
                    claimed_by(mode, step, cab.status(&mode.store, step))
                ),
                false,
            )
            .on_hover_text(&step.title)
            .clicked()
            {
                state.focus = Some(n);
            }
        }
        if ready.len() > 12 {
            ui.label(format!("and {} more", ready.len() - 12));
        }
    });
}

/// Step numbers as a short list: runs become `5-12`.
fn ranges(numbers: &[usize]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < numbers.len() {
        let mut j = i;
        while j + 1 < numbers.len() && numbers[j + 1] == numbers[j] + 1 {
            j += 1;
        }
        out.push(if j > i {
            format!("{}-{}", numbers[i], numbers[j])
        } else {
            numbers[i].to_string()
        });
        i = j + 1;
    }
    out.join(", ")
}

/// The bar shown while Build mode is stepped aside. Returns whether the
/// person asked to go back.
fn away_bar(ctx: &egui::Context, rect: egui::Rect, step: &str) -> bool {
    let mut back = false;
    egui::Area::new(egui::Id::new("build_mode_away"))
        .order(egui::Order::Middle)
        // Bottom left: the Cabinet's lowest device and the 3D view's
        // controls sit towards the middle and the right.
        .pivot(egui::Align2::LEFT_BOTTOM)
        .fixed_pos(egui::pos2(rect.left() + 14.0, rect.bottom() - 14.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, "Back to Build mode").clicked() {
                        back = true;
                    }
                    if !step.is_empty() {
                        ui.add(egui::Label::new(step).truncate());
                    }
                });
            });
        });
    back
}
