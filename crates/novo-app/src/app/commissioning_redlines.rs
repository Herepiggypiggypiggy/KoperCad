//! As-built redlines in Commissioning mode: what the machine turned out to be
//! (a wire number, a device tag, a remark), noted at the machine and made in
//! the drawing afterwards, optionally under a new revision.
//!
//! The drawing is read-only while Commissioning is open, so a redline only
//! records the change. "Apply" makes the open ones in one undoable step; the
//! revision dialog then opens with a line per change as its description.

use eframe::egui;
use novo_commissioning::{
    Change, Redline, RedlineState, Target, TargetInfo, apply_commands, revision_description,
    targets,
};
use novo_core::{Command, PageId, Project};
use novo_theme::{Palette, icons, space, widgets};
use novo_xref::Place;

use super::cabinet_bar::note_ctx;
use super::commissioning::{CommissioningMode, now, place_of};
use super::{Level, NovoApp};

/// What the Redlines tab asks the app to do after the frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RedlineAction {
    Apply,
    ApplyAndIssue,
}

/// The kind of change being typed.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Kind {
    #[default]
    Note,
    WireNumber,
    DeviceTag,
    TerminalName,
}

/// The "new redline" form.
#[derive(Default)]
pub(super) struct RedlineDraft {
    kind: Kind,
    text: String,
}

fn kinds_for(target: Target) -> &'static [(Kind, &'static str)] {
    match target {
        Target::Wire(_) => &[(Kind::Note, "Note"), (Kind::WireNumber, "New number")],
        Target::Device(_) => &[(Kind::Note, "Note"), (Kind::DeviceTag, "New tag")],
        Target::Terminal(_) => &[
            (Kind::Note, "Note"),
            (Kind::TerminalName, "New designation"),
        ],
    }
}

pub(super) fn redlines_tab(
    ui: &mut egui::Ui,
    mode: &mut CommissioningMode,
    project: &Project,
    current: Option<PageId>,
    go: &mut Option<Place>,
) -> Option<RedlineAction> {
    let all = targets(project);
    let mut action = None;
    let info = mode
        .selected
        .and_then(|t| all.iter().find(|i| i.target == t));
    match info {
        None => {
            ui.weak("Pick a wire, terminal or device on the Checks tab to note a change");
        }
        Some(info) => {
            ui.label(egui::RichText::new(&info.label).strong());
            let kinds = kinds_for(info.target);
            if !kinds.iter().any(|(k, _)| *k == mode.redline.kind) {
                mode.redline.kind = Kind::Note;
            }
            ui.horizontal_wrapped(|ui| {
                for &(kind, label) in kinds {
                    ui.selectable_value(&mut mode.redline.kind, kind, label);
                }
            });
            let hint = match mode.redline.kind {
                Kind::Note => "What differs on the machine",
                Kind::WireNumber => "Number on the machine",
                Kind::DeviceTag => "Tag on the machine, e.g. -F33",
                Kind::TerminalName => "Designation on the machine, e.g. 3 or PE",
            };
            ui.add(
                egui::TextEdit::singleline(&mut mode.redline.text)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            );
            let can_add = !mode.redline.text.trim().is_empty();
            if ui
                .add_enabled(can_add, egui::Button::new("Add redline"))
                .clicked()
            {
                add_redline(mode, info);
            }
        }
    }
    ui.add_space(space::M);
    let open = mode.store.open_redline_count();
    if mode.store.redlines().next().is_none() {
        ui.weak("No redlines yet");
    } else {
        ui.horizontal_wrapped(|ui| {
            let apply = ui.add_enabled(open > 0, egui::Button::new("Apply to drawing"));
            note_ctx(
                ui.ctx(),
                "commissioning.redline_apply",
                "Apply button",
                &apply,
            );
            if apply
                .on_hover_text("Make the open changes in the drawing (one undo step)")
                .clicked()
            {
                action = Some(RedlineAction::Apply);
            }
            let issue = ui.add_enabled(open > 0, egui::Button::new("Apply and issue revision"));
            if issue
                .on_hover_text("Apply, then open the revision dialog with the changes listed")
                .clicked()
            {
                action = Some(RedlineAction::ApplyAndIssue);
            }
        });
    }
    let list: Vec<Redline> = mode.store.redlines().cloned().collect();
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let p = Palette::of(ui);
            for r in list.iter().rev() {
                let color = match r.state {
                    RedlineState::Open => p.warning,
                    RedlineState::Applied => p.text_secondary,
                    RedlineState::Rejected => p.text_tertiary,
                };
                let line = match &r.change {
                    Change::Note => format!("{}: {}", r.label, r.note.trim()),
                    Change::WireNumber(n) => format!("{} → number {}", r.label, n.trim()),
                    Change::DeviceTag(t) => format!("{} → tag {}", r.label, t.trim()),
                    Change::TerminalName(n) => format!("{} → designation {}", r.label, n.trim()),
                };
                let row = widgets::row(ui, false, |ui| {
                    ui.label(egui::RichText::new(icons::CIRCLE).color(color));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(r.state.label())
                                .size(12.0)
                                .color(p.text_secondary),
                        );
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Label::new(line).truncate())
                        });
                    });
                });
                if row.clicked()
                    && let Some(info) = all.iter().find(|i| i.target == r.target)
                {
                    mode.selected = Some(info.target);
                    *go = place_of(project, info, current);
                }
                ui.horizontal(|ui| {
                    // Applied ones reopen too: after an undo in the editor
                    // the change can be made again.
                    let (label, state) = if r.state == RedlineState::Open {
                        ("Reject", RedlineState::Rejected)
                    } else {
                        ("Reopen", RedlineState::Open)
                    };
                    if ui.button(label).clicked()
                        && mode.store.set_redline_state(r.id, state).is_ok()
                    {
                        mode.dirty = true;
                    }
                });
            }
        });
    action
}

fn add_redline(mode: &mut CommissioningMode, info: &TargetInfo) {
    let text = std::mem::take(&mut mode.redline.text).trim().to_owned();
    let (change, note) = match mode.redline.kind {
        Kind::Note => (Change::Note, text),
        Kind::WireNumber => (Change::WireNumber(text), String::new()),
        Kind::DeviceTag => (Change::DeviceTag(text), String::new()),
        Kind::TerminalName => (Change::TerminalName(text), String::new()),
    };
    let redline = Redline {
        id: 0,
        target: info.target,
        label: info.label.clone(),
        change,
        note,
        by: mode.who.clone(),
        at: now(),
        state: RedlineState::Open,
    };
    if mode.store.add_redline(redline).is_ok() {
        mode.dirty = true;
    }
}

impl NovoApp {
    /// Makes the open redlines in the drawing as one undo step. Those that
    /// cannot be made are named in a message and stay open. With `issue`, the
    /// revision dialog opens listing what was applied.
    pub(super) fn apply_redlines(&mut self, issue: bool) {
        self.load_commissioning();
        let Some(mode) = self.wb.commissioning.as_ref() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let mut ids = Vec::new();
        let mut commands = Vec::new();
        let mut failed = Vec::new();
        for (id, result) in apply_commands(mode.store.redlines(), project) {
            match result {
                Ok(command) => {
                    ids.push(id);
                    commands.push(command);
                }
                Err(why) => failed.push(why),
            }
        }
        // Remarks have nothing to make; applying them just files them.
        let notes: Vec<u64> = mode
            .store
            .redlines()
            .filter(|r| r.state == RedlineState::Open && !r.change.is_edit())
            .map(|r| r.id)
            .collect();
        if !commands.is_empty() {
            let batch = Command::Batch {
                label: "Apply as-built redlines".into(),
                commands,
            };
            match self.execute(batch) {
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    self.message(Level::Error, format!("Could not apply the redlines: {e}"));
                    return;
                }
                None => return,
            }
        }
        let Some(mode) = self.wb.commissioning.as_mut() else {
            return;
        };
        for id in ids.iter().chain(&notes) {
            if mode
                .store
                .set_redline_state(*id, RedlineState::Applied)
                .is_ok()
            {
                mode.dirty = true;
            }
        }
        let description = revision_description(
            mode.store
                .redlines()
                .filter(|r| ids.contains(&r.id) || notes.contains(&r.id)),
        );
        let applied = ids.len();
        if applied > 0 || !notes.is_empty() {
            self.message(
                Level::Info,
                format!("Applied {applied} redlines to the drawing"),
            );
        }
        for why in failed {
            self.message(Level::Warning, format!("Redline not applied: {why}"));
        }
        if issue && applied + notes.len() > 0 {
            self.open_issue_revision(Some(&description));
        }
    }
}
