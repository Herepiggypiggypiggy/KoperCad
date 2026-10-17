//! Wire numbering: the Wire numbering window (scheme, preview, conflicts)
//! and the palette commands over `novo-numbering`.
//!
//! Where the user finds it: Ctrl+K "wire numbering" opens the window,
//! "number wires" fills what has no number, "renumber wires" starts over
//! with a preview, "check wire numbers" lists duplicates and gaps, and
//! "number wire 105" types a number on the selected wire's net and locks it.

use eframe::egui;
use novo_core::{AnyId, PageId, WireId};
use novo_numbering::{
    Conflict, ConflictKind, Mode, Plan, Reason, Scheme, SchemeKind, Scope, Severity,
};

use super::{Level, NovoApp};

/// Which wires the window works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScopeChoice {
    Project,
    ThisPage,
    Selection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Preview,
    Conflicts,
}

/// The Wire numbering window while it is open.
pub(crate) struct NumberingWindow {
    scheme: Scheme,
    mode: Mode,
    scope: ScopeChoice,
    tab: Tab,
    /// The plan for the inputs it was made from, at project revision `.0`.
    shown: Option<Shown>,
}

struct Shown {
    revision: u64,
    scheme: Scheme,
    mode: Mode,
    scope: Scope,
    plan: Plan,
}

impl NovoApp {
    /// Opens the window; `mode` and `tab` decide what it shows first.
    pub(super) fn open_numbering(&mut self, mode: Mode, tab: Tab) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let scheme = Scheme::load(doc.project());
        let scope = if self.selected_wires().is_empty() {
            ScopeChoice::Project
        } else {
            ScopeChoice::Selection
        };
        self.wb.numbering = Some(NumberingWindow {
            scheme,
            mode,
            scope,
            tab,
            shown: None,
        });
    }

    /// The wires in the canvas selection.
    pub(super) fn selected_wires(&self) -> Vec<WireId> {
        self.selection()
            .into_iter()
            .filter_map(|id| match id {
                AnyId::Wire(w) => Some(w),
                _ => None,
            })
            .collect()
    }

    /// "Number wires": numbers every net that has no number with the
    /// project's scheme, or only the selected wires' nets. Existing numbers
    /// are never touched. One undo step.
    pub(super) fn number_wires(&mut self) {
        let picked = self.selected_wires();
        let scope = if picked.is_empty() {
            Scope::Project
        } else {
            Scope::Wires(picked)
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let scheme = Scheme::load(doc.project());
        let plan = novo_numbering::plan(doc.project(), &scheme, Mode::Fill, &scope);
        if plan.is_empty() {
            self.message(Level::Info, "All wires are numbered");
            return;
        }
        let summary = plan.summary();
        match doc.transact("Number wires", |tx| plan.apply(tx)) {
            Ok((_, changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(Level::Info, format!("Wire numbers: {summary}"));
            }
            Err(e) => self.message(Level::Error, format!("Number wires: {e}")),
        }
    }

    /// "Number wire 105": types a number on the selected wire's net and
    /// locks it against numbering runs. An empty number clears it.
    pub(super) fn set_wire_number(&mut self, argument: Option<&str>) {
        let Some(&wire) = self.selected_wires().first() else {
            self.message(Level::Info, "Select a wire first");
            return;
        };
        let Some(argument) = argument else {
            // From the menu there is no number yet: ask for it.
            self.wb.number_prompt = Some(String::new());
            return;
        };
        let number = argument.trim().to_owned();
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        match doc.transact("Set wire number", |tx| {
            novo_numbering::set_number(tx, wire, &number)
        }) {
            Ok((_, changes)) => {
                self.changed_outside_canvas(&changes);
                let text = if number.is_empty() {
                    "Wire number cleared".to_owned()
                } else {
                    format!("Wire number {number} is fixed: numbering runs leave it alone")
                };
                self.message(Level::Info, text);
            }
            Err(e) => self.message(Level::Warning, format!("Wire number: {e}")),
        }
    }

    /// The one-field "Set wire number" prompt (Edit > Set wire number).
    pub(super) fn number_prompt_window(&mut self, ctx: &egui::Context) {
        let Some(mut text) = self.wb.number_prompt.take() else {
            return;
        };
        let mut open = true;
        let mut done = false;
        super::panel::dialog(ctx, "Set wire number", [320.0, 130.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Number for the selected wire's net (empty clears it):");
                let edit = ui.add(egui::TextEdit::singleline(&mut text).desired_width(200.0));
                if !edit.has_focus() && !done {
                    edit.request_focus();
                }
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    done = ui.button("Set").clicked() || enter;
                });
            });
        if done {
            self.set_wire_number(Some(&text));
        } else if open {
            self.wb.number_prompt = Some(text);
        }
    }

    /// "Unlock wire number": numbering runs may change the selected wires'
    /// nets again.
    pub(super) fn unlock_wire_number(&mut self) {
        let wires = self.selected_wires();
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let locked: Vec<WireId> = wires
            .into_iter()
            .filter(|&w| novo_numbering::is_locked(doc.project(), w))
            .collect();
        if locked.is_empty() {
            self.message(Level::Info, "No selected wire has a fixed number");
            return;
        }
        let result = doc.transact("Unlock wire number", |tx| {
            locked
                .iter()
                .try_for_each(|&w| novo_numbering::unlock(tx, w))
        });
        match result {
            Ok((_, changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(Level::Info, "Wire number unlocked");
            }
            Err(e) => self.message(Level::Error, format!("Unlock wire number: {e}")),
        }
    }

    pub(super) fn numbering_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.numbering.take() else {
            return;
        };
        let selected = self.selected_wires();
        let page = self.current_page;
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let scope = match win.scope {
            ScopeChoice::Project => Scope::Project,
            ScopeChoice::ThisPage => Scope::Pages(page.into_iter().collect::<Vec<PageId>>()),
            ScopeChoice::Selection => Scope::Wires(selected.clone()),
        };
        let stale = win.shown.as_ref().is_none_or(|s| {
            s.revision != project.revision()
                || s.scheme != win.scheme
                || s.mode != win.mode
                || s.scope != scope
        });
        if stale {
            let plan = novo_numbering::plan(project, &win.scheme, win.mode, &scope);
            win.shown = Some(Shown {
                revision: project.revision(),
                scheme: win.scheme.clone(),
                mode: win.mode,
                scope: scope.clone(),
                plan,
            });
        }
        let stored = Scheme::load(project) != win.scheme;
        let mut open = true;
        let mut apply = false;
        let mut save_only = false;
        let mut reveal: Option<WireId> = None;
        super::panel::dialog(ctx, "Wire numbering", [560.0, 620.0])
            .open(&mut open)
            .show(ctx, |ui| {
                novo_theme::widgets::section_label(ui, "Scheme");
                scheme_ui(ui, &mut win.scheme);
                ui.add_space(8.0);
                novo_theme::widgets::section_label(ui, "What to number");
                let modes = [Mode::Fill, Mode::FillStrict, Mode::Renumber];
                let mut mode = modes.iter().position(|m| *m == win.mode).unwrap_or(0);
                if novo_theme::widgets::segmented(
                    ui,
                    &mut mode,
                    &["Number new wires", "Also fix off-scheme", "Renumber all"],
                ) {
                    win.mode = modes[mode];
                }
                ui.weak(match win.mode {
                    Mode::Fill => "Only wires without a number; existing numbers stay.",
                    Mode::FillStrict => "Numbers the scheme would not give are replaced too.",
                    Mode::Renumber => "Start over in reading order; typed numbers stay.",
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Wires");
                    let scopes = [
                        ScopeChoice::Project,
                        ScopeChoice::ThisPage,
                        ScopeChoice::Selection,
                    ];
                    let mut scope = scopes.iter().position(|s| *s == win.scope).unwrap_or(0);
                    if novo_theme::widgets::segmented(
                        ui,
                        &mut scope,
                        &["Project", "This page", "Selection"],
                    ) {
                        let allowed = match scopes[scope] {
                            ScopeChoice::Project => true,
                            ScopeChoice::ThisPage => page.is_some(),
                            ScopeChoice::Selection => !selected.is_empty(),
                        };
                        if allowed {
                            win.scope = scopes[scope];
                        }
                    }
                    if page.is_none() {
                        ui.weak("This page: open a page first");
                    } else if selected.is_empty() {
                        ui.weak("Selection: select wires on the drawing first");
                    }
                });
                let Some(shown) = &win.shown else { return };
                ui.add_space(8.0);
                let conflicts = shown.plan.conflicts.len();
                let mut tab = usize::from(win.tab == Tab::Conflicts);
                if novo_theme::widgets::tabs(
                    ui,
                    &mut tab,
                    &["Preview", &format!("Conflicts ({conflicts})")],
                ) {
                    win.tab = [Tab::Preview, Tab::Conflicts][tab];
                }
                ui.weak(shown.plan.summary());
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| match win.tab {
                        Tab::Preview => preview_ui(ui, &shown.plan, &mut reveal),
                        Tab::Conflicts => conflicts_ui(ui, &shown.plan.conflicts, &mut reveal),
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let label = match win.mode {
                        Mode::Renumber => "Renumber",
                        _ => "Number",
                    };
                    let can = !shown.plan.is_empty();
                    if ui
                        .add_enabled_ui(can, |ui| novo_theme::widgets::primary_button(ui, label))
                        .inner
                        .clicked()
                    {
                        apply = true;
                    }
                    if ui
                        .add_enabled(stored, egui::Button::new("Save scheme only"))
                        .clicked()
                    {
                        save_only = true;
                    }
                });
            });
        if let Some(wire) = reveal {
            self.reveal_wire(wire);
        }
        if apply || save_only {
            self.apply_numbering(&mut win, apply);
        }
        if open && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.wb.numbering = Some(win);
        }
    }

    /// Opens a wire's page and selects it.
    fn reveal_wire(&mut self, wire: WireId) {
        let Some(page) = self
            .backend
            .document()
            .and_then(|d| d.project().wire(wire).map(|w| w.page))
        else {
            return;
        };
        self.show_page(page);
        self.wb.pending_item = Some((page, AnyId::Wire(wire)));
    }

    /// Saves the window's scheme and, with `numbers`, applies its plan: one
    /// undo step.
    pub(super) fn apply_numbering(&mut self, win: &mut NumberingWindow, numbers: bool) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let Some(shown) = win.shown.take() else {
            return;
        };
        let scheme = win.scheme.clone();
        let label = if numbers {
            "Number wires"
        } else {
            "Wire numbering scheme"
        };
        let result = doc.transact(label, |tx| {
            let n = if numbers { shown.plan.apply(tx)? } else { 0 };
            scheme.store(tx)?;
            Ok(n)
        });
        match result {
            Ok((n, changes)) => {
                self.changed_outside_canvas(&changes);
                let text = if numbers {
                    format!("Wire numbers: {}", shown.plan.summary())
                } else {
                    "Numbering scheme saved".to_owned()
                };
                let _ = n;
                self.message(Level::Info, text);
            }
            Err(e) => self.message(Level::Error, format!("Wire numbering: {e}")),
        }
    }
}

fn scheme_ui(ui: &mut egui::Ui, scheme: &mut Scheme) {
    ui.horizontal(|ui| {
        ui.label("Scheme");
        egui::ComboBox::from_id_salt("wirenum_scheme")
            .selected_text(scheme.kind.label())
            .show_ui(ui, |ui| {
                for kind in SchemeKind::ALL {
                    ui.selectable_value(&mut scheme.kind, kind, kind.label());
                }
            });
        ui.weak(scheme.kind.example());
    });
    ui.horizontal(|ui| {
        ui.label("Prefix");
        ui.add(egui::TextEdit::singleline(&mut scheme.prefix).desired_width(48.0));
        ui.label("Separator");
        ui.add(egui::TextEdit::singleline(&mut scheme.separator).desired_width(32.0));
        ui.label("Start");
        ui.add(egui::DragValue::new(&mut scheme.start).range(0..=99_999));
        ui.label("Step");
        ui.add(egui::DragValue::new(&mut scheme.step).range(1..=1000));
        ui.label("Digits");
        ui.add(egui::DragValue::new(&mut scheme.digits).range(1..=8));
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut scheme.bare_potentials, "PE and N keep their name")
            .on_hover_text("PE, N and PEN wires are numbered PE, N, PEN on every net");
        ui.checkbox(&mut scheme.reuse_gaps, "Reuse freed numbers")
            .on_hover_text("A deleted wire's number may go to a new wire");
    });
}

fn preview_ui(ui: &mut egui::Ui, plan: &Plan, reveal: &mut Option<WireId>) {
    let mut any = false;
    egui::Grid::new("wirenum_preview")
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            for g in plan.changed_groups() {
                any = true;
                let first = g.wires.first().copied();
                if ui.link(&g.at).clicked() {
                    *reveal = first;
                }
                ui.weak(g.potential.as_deref().unwrap_or(""));
                let old = if g.old.is_empty() {
                    "(none)".to_owned()
                } else {
                    g.old.join(" / ")
                };
                ui.label(format!("{old} → {}", g.new.as_deref().unwrap_or("(none)")));
                ui.weak(reason_text(g.reason));
                ui.end_row();
            }
        });
    if !any {
        ui.weak("Nothing to change");
    }
}

fn reason_text(reason: Reason) -> &'static str {
    match reason {
        Reason::Kept => "unchanged",
        Reason::Locked => "typed",
        Reason::New => "new",
        Reason::Healed => "made equal across the net",
        Reason::Replaced => "was used or off scheme",
        Reason::Renumbered => "renumbered",
        Reason::Blocked => "its number is taken",
    }
}

fn conflicts_ui(ui: &mut egui::Ui, conflicts: &[Conflict], reveal: &mut Option<WireId>) {
    if conflicts.is_empty() {
        ui.weak("No conflicts: every net has one number and no number is used twice");
        return;
    }
    for c in conflicts {
        let mark = match c.severity {
            Severity::Error => "●",
            Severity::Warning => "◐",
            Severity::Info => "○",
        };
        let kind = match c.kind {
            ConflictKind::Duplicate => "duplicate",
            ConflictKind::Split => "split",
            ConflictKind::Partial => "partial",
            ConflictKind::Unnumbered => "no number",
            ConflictKind::OffScheme => "off scheme",
        };
        if ui
            .selectable_label(false, format!("{mark} {kind}: {}", c.message))
            .clicked()
        {
            *reveal = c.wires.first().copied();
        }
    }
}
