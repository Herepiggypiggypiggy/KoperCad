//! Strip editing in the Terminals drawer (novo-terminals strip.rs, style.rs
//! and suggest.rs): a strip picker, the edit toolbar, the row context menu,
//! and the windows for rename and renumber (both with a preview), kind and
//! strip style. Every edit is one `terminal.*` command (menu Draw >
//! Terminals, command palette, drawer) and one undo step.

use eframe::egui;
use novo_core::{DeviceId, TerminalId, TerminalKind, WireId};
use novo_terminals::{RenameProblem, Style};

use super::{Level, NovoApp};

/// The commands the drawer toolbar and the row menu offer, in order.
pub(super) const EDIT_COMMANDS: [&str; 11] = [
    "terminal.add_spares",
    "terminal.rename",
    "terminal.renumber",
    "terminal.join_duplicates",
    "terminal.set_kind",
    "terminal.set_level",
    "terminal.set_jumper",
    "terminal.order_up",
    "terminal.order_down",
    "terminal.style",
    "terminal.restore_declined",
];

/// Every terminal kind, in the order the pickers list them.
pub(super) const KINDS: [TerminalKind; 5] = [
    TerminalKind::Feedthrough,
    TerminalKind::ProtectiveEarth,
    TerminalKind::Neutral,
    TerminalKind::Fuse,
    TerminalKind::Disconnect,
];

/// The window a terminal command opened.
pub(super) enum EditWindow {
    Rename {
        find: String,
        replace: String,
    },
    Renumber,
    Kind,
    /// The level to give the targets (from 1).
    Level(u8),
    /// The jumper group to give the targets.
    Jumper(u32),
    Style,
}

/// State of the strip editor in the Terminals drawer.
pub(super) struct TerminalEdit {
    /// The strip the drawer shows; follows the selection when that moves to
    /// another strip, and can be set with the picker.
    pub picked: Option<DeviceId>,
    followed: Option<DeviceId>,
    /// Terminals ticked in the table, the target of row edits.
    pub rows: Vec<TerminalId>,
    /// How many spares "Add spares" adds from the toolbar.
    pub spares: usize,
    pub window: Option<EditWindow>,
}

impl Default for TerminalEdit {
    fn default() -> Self {
        Self {
            picked: None,
            followed: None,
            rows: Vec::new(),
            spares: 1,
            window: None,
        }
    }
}

/// What a kind is called when typed (`terminal.set_kind pe`).
pub(super) fn parse_kind(text: &str) -> Option<TerminalKind> {
    let t = text.trim().to_lowercase();
    KINDS
        .into_iter()
        .find(|k| novo_terminals::kind_label(*k).to_lowercase() == t)
        .or(match t.as_str() {
            "feedthrough" | "feed through" => Some(TerminalKind::Feedthrough),
            "protective earth" | "earth" => Some(TerminalKind::ProtectiveEarth),
            "neutral" => Some(TerminalKind::Neutral),
            _ => None,
        })
}

/// What joining duplicates did, in words.
fn join_message(joined: usize) -> String {
    if joined == 0 {
        "No duplicate terminals on this strip".to_owned()
    } else {
        format!("Joined {joined} duplicate terminal records")
    }
}

impl NovoApp {
    /// The strip the selection points at, if any.
    pub(super) fn selection_strip(&self) -> Option<DeviceId> {
        use novo_core::{AnyId, SymbolRole};
        let project = self.backend.document()?.project();
        self.selection().into_iter().find_map(|id| match id {
            AnyId::Symbol(s) => match &project.symbol(s)?.role {
                SymbolRole::Terminal { terminals } => {
                    project.terminal(*terminals.first()?).map(|t| t.strip)
                }
                SymbolRole::Device { device, .. } => project
                    .device(*device)
                    .filter(|d| d.kind == novo_core::DeviceKind::TerminalStrip)
                    .map(|d| d.id),
                _ => None,
            },
            AnyId::Device(d) => Some(d),
            _ => None,
        })
    }

    /// Moving the selection to another strip moves the drawer along; the
    /// picker keeps its choice until then.
    pub(super) fn sync_strip_follow(&mut self) {
        let now = self.selection_strip();
        if now != self.wb.terminal_edit.followed {
            self.wb.terminal_edit.followed = now;
            if now.is_some() {
                self.wb.terminal_edit.picked = now;
            }
        }
    }

    /// The terminals an edit acts on: the ticked rows and the selected
    /// drawn terminals, on the shown strip.
    fn edit_targets(&self, strip: DeviceId) -> Vec<TerminalId> {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return Vec::new();
        };
        let mut out: Vec<TerminalId> = Vec::new();
        for t in self
            .wb
            .terminal_edit
            .rows
            .iter()
            .copied()
            .chain(self.selected_terminals())
        {
            if project.terminal(t).is_some_and(|t| t.strip == strip) && !out.contains(&t) {
                out.push(t);
            }
        }
        out
    }

    /// Runs one `terminal.*` strip edit. Says what happened or why not.
    pub(super) fn terminal_edit_command(&mut self, id: &str, argument: Option<&str>) {
        self.sync_strip_follow();
        let Some(strip) = self.drawer_strip() else {
            self.message(
                Level::Warning,
                "No terminal strip yet. Place a terminal (I, terminal) or accept a proposal.",
            );
            return;
        };
        let targets = self.edit_targets(strip);
        // Make the strip visible: the edit acts on what the drawer shows.
        if self.wb.drawer.is_none() {
            self.wb.drawer = Some(super::workbench::Drawer::Terminals);
        }
        match id {
            "terminal.add_spares" => {
                let count = argument
                    .and_then(|a| a.trim().parse::<usize>().ok())
                    .unwrap_or(self.wb.terminal_edit.spares)
                    .clamp(1, 100);
                self.edit_strip("Add spare terminals", |tx| {
                    novo_terminals::add_spares(tx, strip, count).map(|_| ())
                });
            }
            "terminal.rename" => {
                self.wb.terminal_edit.window = Some(EditWindow::Rename {
                    find: String::new(),
                    replace: String::new(),
                });
            }
            "terminal.renumber" => self.wb.terminal_edit.window = Some(EditWindow::Renumber),
            "terminal.join_duplicates" => {
                let mut joined = 0;
                // A failed edit has already said why; only a real result is reported.
                if self.edit_strip("Join duplicate terminals", |tx| {
                    joined = novo_terminals::join_duplicates(tx, strip)?.len();
                    Ok(())
                }) {
                    self.message(Level::Info, join_message(joined));
                }
            }
            "terminal.set_kind" => match argument.and_then(parse_kind) {
                Some(kind) => self.set_kind(&targets, kind),
                None => self.wb.terminal_edit.window = Some(EditWindow::Kind),
            },
            "terminal.set_level" => {
                let typed = argument.and_then(|a| a.trim().parse::<u8>().ok());
                match typed {
                    Some(level) => self.set_level(&targets, level),
                    None => {
                        let first = self.first_level(&targets);
                        self.wb.terminal_edit.window = Some(EditWindow::Level(first));
                    }
                }
            }
            "terminal.set_jumper" => {
                let free = self.free_jumper(strip);
                match argument.map(|a| a.trim().to_lowercase()).as_deref() {
                    Some("none") => self.set_jumper(&targets, None),
                    Some("next") => self.set_jumper(&targets, Some(free)),
                    Some(n) if n.trim_start_matches('j').parse::<u32>().is_ok() => {
                        let n = n.trim_start_matches('j').parse().unwrap_or(free);
                        self.set_jumper(&targets, Some(n));
                    }
                    _ => self.wb.terminal_edit.window = Some(EditWindow::Jumper(free)),
                }
            }
            "terminal.order_up" | "terminal.order_down" => {
                self.move_in_strip(strip, &targets, id == "terminal.order_up");
            }
            "terminal.style" => self.wb.terminal_edit.window = Some(EditWindow::Style),
            "terminal.restore_declined" => {
                let wires = self.declined_wires();
                if wires.is_empty() {
                    self.message(Level::Info, "No declined terminal proposals");
                } else {
                    self.restore_declined(&wires);
                }
            }
            _ => {}
        }
    }

    /// One edit of the shown strip as one undo step; errors are said.
    fn edit_strip(
        &mut self,
        label: &str,
        edit: impl FnOnce(&mut novo_core::Tx<'_>) -> novo_core::Result<()>,
    ) -> bool {
        let Some(doc) = self.backend.document_mut() else {
            return false;
        };
        match doc.transact(label, edit) {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                true
            }
            Err(e) => {
                self.message(Level::Error, format!("{label}: {e}"));
                false
            }
        }
    }

    fn set_kind(&mut self, targets: &[TerminalId], kind: TerminalKind) {
        if targets.is_empty() {
            self.message(
                Level::Warning,
                "Tick terminals in the table (or select them) first",
            );
            return;
        }
        self.edit_strip("Set terminal kind", |tx| {
            for &t in targets {
                novo_terminals::set_kind(tx, t, kind)?;
            }
            Ok(())
        });
    }

    fn first_level(&self, targets: &[TerminalId]) -> u8 {
        let project = self.backend.document().map(|d| d.project());
        targets
            .first()
            .and_then(|&t| project?.terminal(t))
            .map_or(1, |t| t.level)
    }

    fn free_jumper(&self, strip: DeviceId) -> u32 {
        self.backend
            .document()
            .map_or(1, |d| novo_terminals::free_jumper(d.project(), strip))
    }

    fn set_level(&mut self, targets: &[TerminalId], level: u8) {
        if targets.is_empty() {
            self.message(
                Level::Warning,
                "Tick terminals in the table (or select them) first",
            );
            return;
        }
        self.edit_strip("Set terminal level", |tx| {
            for &t in targets {
                novo_terminals::set_level(tx, t, level)?;
            }
            Ok(())
        });
    }

    /// Bridges the targets under jumper number `jumper`, or takes them out
    /// of any jumper with `None`. A number already used on the strip joins
    /// that group.
    fn set_jumper(&mut self, targets: &[TerminalId], jumper: Option<u32>) {
        if targets.is_empty() {
            self.message(
                Level::Warning,
                "Tick terminals in the table (or select them) first",
            );
            return;
        }
        if jumper == Some(0) {
            self.message(Level::Warning, "Jumper numbers start at 1");
            return;
        }
        self.edit_strip("Set terminal jumper", |tx| {
            novo_terminals::set_jumper(tx, targets, jumper).map(|_| ())
        });
    }

    fn move_in_strip(&mut self, strip: DeviceId, targets: &[TerminalId], up: bool) {
        if targets.is_empty() {
            self.message(
                Level::Warning,
                "Tick terminals in the table (or select them) first",
            );
            return;
        }
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let mut order: Vec<TerminalId> = project
            .terminals_of_strip(strip)
            .into_iter()
            .map(|t| t.id)
            .collect();
        let before = order.clone();
        let n = order.len();
        let indices: Vec<usize> = if up {
            (0..n).collect()
        } else {
            (0..n).rev().collect()
        };
        for i in indices {
            if !targets.contains(&order[i]) {
                continue;
            }
            let j = if up { i.checked_sub(1) } else { Some(i + 1) };
            if let Some(j) = j.filter(|&j| j < n && !targets.contains(&order[j])) {
                order.swap(i, j);
            }
        }
        if order == before {
            return;
        }
        self.edit_strip("Move terminal in strip", |tx| {
            novo_terminals::reorder(tx, strip, &order)
        });
    }

    /// Wires whose terminal proposal was declined.
    pub(super) fn declined_wires(&self) -> Vec<WireId> {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return Vec::new();
        };
        project
            .dismissals()
            .filter(|d| d.proposal == novo_terminals::DISMISS_KEY)
            .filter_map(|d| match d.subjects.first() {
                Some(novo_core::AnyId::Wire(w)) => Some(*w),
                _ => None,
            })
            .collect()
    }

    /// Takes declined proposals back, as one undo step.
    pub(super) fn restore_declined(&mut self, wires: &[WireId]) {
        let done = self.edit_strip("Restore declined terminal proposals", |tx| {
            for &w in wires {
                novo_terminals::restore_suggestion(tx, w)?;
            }
            Ok(())
        });
        if done {
            self.message(Level::Info, "Declined terminal proposals are back");
        }
    }

    /// The windows of the strip editor, while one is open.
    pub(super) fn terminal_edit_windows(&mut self, ctx: &egui::Context) {
        let Some(mut window) = self.wb.terminal_edit.window.take() else {
            return;
        };
        let Some(strip) = self.drawer_strip() else {
            return;
        };
        let mut keep = true;
        let mut apply: Option<Apply> = None;
        {
            let Some(project) = self.backend.document().map(|d| d.project()) else {
                return;
            };
            let tag = project
                .device(strip)
                .map(|d| d.tag.to_string())
                .unwrap_or_default();
            let name_of = |t: TerminalId| {
                project
                    .terminal(t)
                    .map(|t| t.name.clone())
                    .unwrap_or_default()
            };
            let (title, id) = match &window {
                EditWindow::Rename { .. } => ("Rename terminals", "terminal_rename"),
                EditWindow::Renumber => ("Renumber terminals", "terminal_renumber"),
                EditWindow::Kind => ("Terminal kind", "terminal_kind"),
                EditWindow::Level(_) => ("Terminal level", "terminal_level"),
                EditWindow::Jumper(_) => ("Terminal jumper", "terminal_jumper"),
                EditWindow::Style => ("Strip style", "terminal_style"),
            };
            super::panel::dialog(ctx, format!("{title} {tag}"), [420.0, 0.0])
                .id(egui::Id::new(id))
                .collapsible(false)
                .open(&mut keep)
                .show(ctx, |ui| match &mut window {
                    EditWindow::Rename { find, replace } => {
                        ui.horizontal(|ui| {
                            ui.label("Find");
                            ui.text_edit_singleline(find);
                            ui.label("Replace with");
                            ui.text_edit_singleline(replace);
                        });
                        let changes = novo_terminals::rename_preview(project, strip, find, replace);
                        let blocked = changes.iter().any(|c| c.problem.is_some());
                        if changes.is_empty() {
                            ui.weak("Nothing matches yet");
                        }
                        for c in &changes {
                            let note = match c.problem {
                                Some(RenameProblem::Empty) => "  empty name".to_owned(),
                                Some(RenameProblem::Taken(o)) => {
                                    format!("  already used by {}", name_of(o))
                                }
                                None => String::new(),
                            };
                            ui.label(format!("{}  ->  {}{note}", c.old, c.new));
                        }
                        ui.add_space(6.0);
                        if ui
                            .add_enabled_ui(!changes.is_empty() && !blocked, |ui| {
                                novo_theme::widgets::primary_button(
                                    ui,
                                    &format!("Rename {}", changes.len()),
                                )
                            })
                            .inner
                            .clicked()
                        {
                            apply = Some(Apply::Rename(find.clone(), replace.clone()));
                        }
                    }
                    EditWindow::Renumber => {
                        let rows: Vec<_> = novo_terminals::renumber_preview(project, strip)
                            .into_iter()
                            .filter(|(_, old, new)| old != new)
                            .collect();
                        if rows.is_empty() {
                            ui.weak("The numbers already run 1, 2, 3 ...");
                        }
                        for (_, old, new) in &rows {
                            ui.label(format!("{old}  ->  {new}"));
                        }
                        ui.add_space(6.0);
                        if ui
                            .add_enabled_ui(!rows.is_empty(), |ui| {
                                novo_theme::widgets::primary_button(
                                    ui,
                                    &format!("Renumber {}", rows.len()),
                                )
                            })
                            .inner
                            .clicked()
                        {
                            apply = Some(Apply::Renumber);
                        }
                    }
                    EditWindow::Kind => {
                        ui.weak("Applies to the ticked rows and the selected terminals");
                        ui.horizontal(|ui| {
                            for kind in KINDS {
                                if ui.button(novo_terminals::kind_label(kind)).clicked() {
                                    apply = Some(Apply::Kind(kind));
                                }
                            }
                        });
                    }
                    EditWindow::Level(level) => {
                        ui.weak("Applies to the ticked rows and the selected terminals");
                        ui.weak("Level 1 is the lowest level of a multi-level terminal");
                        ui.horizontal(|ui| {
                            ui.label("Level");
                            ui.add(egui::DragValue::new(level).range(1..=8));
                            if novo_theme::widgets::primary_button(ui, "Set level").clicked() {
                                apply = Some(Apply::Level(*level));
                            }
                        });
                    }
                    EditWindow::Jumper(number) => {
                        ui.weak("Applies to the ticked rows and the selected terminals");
                        ui.weak("Terminals with the same jumper number are bridged");
                        let groups = jumper_groups(project, strip);
                        for (n, names) in &groups {
                            ui.horizontal(|ui| {
                                ui.label(format!("J{n}: {}", names.join(", ")));
                                if ui.small_button("Join").clicked() {
                                    apply = Some(Apply::Jumper(Some(*n)));
                                }
                            });
                        }
                        if groups.is_empty() {
                            ui.weak("No jumper on this strip yet");
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Number");
                            ui.add(egui::DragValue::new(number).range(1..=9999).prefix("J"));
                            let joins = groups.iter().any(|(n, _)| n == number);
                            let label = if joins {
                                format!("Join J{number}")
                            } else {
                                format!("Bridge as J{number}")
                            };
                            if novo_theme::widgets::primary_button(ui, &label).clicked() {
                                apply = Some(Apply::Jumper(Some(*number)));
                            }
                            if ui.button("Remove from jumper").clicked() {
                                apply = Some(Apply::Jumper(None));
                            }
                        });
                    }
                    EditWindow::Style => {
                        ui.weak("One symbol per kind for the whole strip");
                        for kind in KINDS {
                            let style = novo_terminals::strip_style(project, strip, kind);
                            ui.horizontal(|ui| {
                                ui.label(novo_terminals::kind_label(kind));
                                egui::ComboBox::from_id_salt(("strip_style", kind as u8))
                                    .selected_text(style.symbol.0.clone())
                                    .show_ui(ui, |ui| {
                                        for k in KINDS {
                                            let key = novo_terminals::default_symbol(k);
                                            if ui
                                                .selectable_label(style.symbol.0 == key, key)
                                                .clicked()
                                            {
                                                apply = Some(Apply::Style(
                                                    kind,
                                                    Style {
                                                        symbol: novo_core::SymbolKey(key.into()),
                                                        part: style.part,
                                                    },
                                                ));
                                            }
                                        }
                                    });
                            });
                        }
                        let overrides = novo_terminals::overrides(project, strip);
                        ui.separator();
                        if overrides.is_empty() {
                            ui.weak("No terminal differs from the strip style");
                        }
                        for t in overrides {
                            ui.horizontal(|ui| {
                                ui.label(format!("{} has its own symbol or part", name_of(t)));
                                if ui.small_button("Clear override").clicked() {
                                    apply = Some(Apply::ClearOverride(t));
                                }
                            });
                        }
                    }
                });
        }
        let mut close = !keep;
        if let Some(apply) = apply {
            let targets = self.edit_targets(strip);
            let ok = match apply {
                Apply::Rename(find, replace) => self.edit_strip("Rename terminals", |tx| {
                    novo_terminals::rename(tx, strip, &find, &replace).map(|_| ())
                }),
                Apply::Renumber => self.edit_strip("Renumber terminals", |tx| {
                    novo_terminals::renumber(tx, strip)
                }),
                Apply::Kind(kind) => {
                    self.set_kind(&targets, kind);
                    true
                }
                Apply::Level(level) => {
                    self.set_level(&targets, level);
                    true
                }
                Apply::Jumper(number) => {
                    self.set_jumper(&targets, number);
                    true
                }
                Apply::Style(kind, style) => self.edit_strip("Change strip style", |tx| {
                    novo_terminals::set_strip_style(tx, strip, kind, &style)
                }),
                Apply::ClearOverride(t) => self.edit_strip("Clear terminal override", |tx| {
                    novo_terminals::clear_override(tx, t)
                }),
            };
            // Style stays open for more changes; the others are done.
            close |= ok && !matches!(window, EditWindow::Style);
        }
        if !close {
            self.wb.terminal_edit.window = Some(window);
        }
    }
}

/// The jumper groups of a strip with the names of their terminals, by
/// number.
fn jumper_groups(project: &novo_core::Project, strip: DeviceId) -> Vec<(u32, Vec<String>)> {
    let mut groups: std::collections::BTreeMap<u32, Vec<String>> = Default::default();
    for t in project.terminals_of_strip(strip) {
        if let Some(j) = t.jumper {
            groups.entry(j).or_default().push(t.name.clone());
        }
    }
    groups.into_iter().collect()
}

enum Apply {
    Rename(String, String),
    Renumber,
    Kind(TerminalKind),
    Level(u8),
    Jumper(Option<u32>),
    Style(TerminalKind, Style),
    ClearOverride(TerminalId),
}

/// What the drawer's toolbar asks for.
pub(super) enum ToolbarAction {
    Command(&'static str),
    Pick(DeviceId),
    Restore(Vec<WireId>),
}

/// The Terminals tab toolbar: strip picker, the strip edits, declined
/// proposals.
pub(super) fn strip_toolbar(
    ui: &mut egui::Ui,
    project: &novo_core::Project,
    strip: Option<DeviceId>,
    edit: &mut TerminalEdit,
    declined: &[WireId],
    out: &mut Vec<ToolbarAction>,
) {
    ui.horizontal_wrapped(|ui| {
        let tag = |s: DeviceId| {
            project
                .device(s)
                .map(|d| d.tag.to_string())
                .unwrap_or_default()
        };
        let shown = strip.map(tag).unwrap_or_else(|| "no strip".into());
        egui::ComboBox::from_id_salt("terminal_strip_picker")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for s in novo_terminals::strips(project) {
                    if ui.selectable_label(Some(s) == strip, tag(s)).clicked() {
                        out.push(ToolbarAction::Pick(s));
                    }
                }
            })
            .response
            .on_hover_text("Choose the strip to show and edit");
        if strip.is_none() {
            return;
        }
        ui.add(
            egui::DragValue::new(&mut edit.spares)
                .range(1..=100)
                .prefix("x"),
        )
        .on_hover_text("How many spare terminals Add spares adds");
        for id in EDIT_COMMANDS {
            // Level and jumper number live in the row menu and the Draw
            // menu: the toolbar is already a full row.
            if matches!(
                id,
                "terminal.restore_declined" | "terminal.set_level" | "terminal.set_jumper"
            ) {
                continue;
            }
            let label = novo_workflow::commands::get(id).map_or(id, |c| c.label);
            if ui.small_button(label).clicked() {
                out.push(ToolbarAction::Command(id));
            }
        }
        if !declined.is_empty() {
            ui.menu_button(format!("Declined ({})", declined.len()), |ui| {
                if ui.button("Restore all").clicked() {
                    out.push(ToolbarAction::Restore(declined.to_vec()));
                    ui.close();
                }
                for &w in declined {
                    let name = project.wire(w).map_or_else(String::new, |wire| {
                        let page = project.page(wire.page).map_or("?", |p| p.name.as_str());
                        format!(
                            "wire {} on page {page}",
                            wire.number.as_deref().unwrap_or("")
                        )
                    });
                    if ui.button(format!("Restore {name}")).clicked() {
                        out.push(ToolbarAction::Restore(vec![w]));
                        ui.close();
                    }
                }
            });
        }
    });
}

/// The row context menu: the same commands as the toolbar and the menu.
pub(super) fn row_menu(ui: &mut egui::Ui, out: &mut Vec<ToolbarAction>) {
    for id in EDIT_COMMANDS {
        let label = novo_workflow::commands::get(id).map_or(id, |c| c.label);
        if ui.button(label).clicked() {
            out.push(ToolbarAction::Command(id));
            ui.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::gate1_tests::Drive;
    use novo_core::DeviceTag;

    /// A project with strips -X1 (terminals 1, 2, 3) and -X2 (one).
    fn drive_with_strip() -> (tempfile::TempDir, Drive, DeviceId, DeviceId) {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, crate::app::Action::NewProject);
        d.frame();
        let doc = d.app.backend.document_mut().unwrap();
        let ((x1, x2), _) = doc
            .transact("strips", |tx| {
                let x1 = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
                let x2 = tx.add_terminal_strip(DeviceTag::parse("-X2")?)?;
                for n in ["1", "2", "3"] {
                    tx.add_terminal(x1, n)?;
                }
                tx.add_terminal(x2, "1")?;
                Ok((x1, x2))
            })
            .unwrap();
        d.app.wb.terminal_edit.picked = Some(x1);
        (dir, d, x1, x2)
    }

    fn names(d: &Drive, strip: DeviceId) -> Vec<String> {
        d.app
            .backend
            .document()
            .unwrap()
            .project()
            .terminals_of_strip(strip)
            .into_iter()
            .map(|t| t.name.clone())
            .collect()
    }

    fn undo(d: &mut Drive) {
        d.app.backend.document_mut().unwrap().undo().unwrap();
    }

    #[test]
    fn the_commands_are_run_by_the_app_and_sit_under_draw_terminals() {
        let draw = novo_workflow::menus::entries("Draw");
        for id in EDIT_COMMANDS {
            assert!(crate::app::workbench::app_runs(id), "{id}");
            let (_, place) = draw
                .iter()
                .find(|(info, _)| info.id == id)
                .unwrap_or_else(|| panic!("{id} is not in the Draw menu"));
            assert_eq!(place.submenu, Some("Terminal strips"), "{id}");
        }
    }

    #[test]
    fn the_picked_strip_is_the_one_the_drawer_shows() {
        let (_dir, mut d, x1, x2) = drive_with_strip();
        assert_eq!(d.app.drawer_strip(), Some(x1));
        d.app.wb.terminal_edit.picked = Some(x2);
        assert_eq!(d.app.drawer_strip(), Some(x2));
    }

    #[test]
    fn add_spares_is_one_undo_step() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        d.app.wb.terminal_edit.spares = 2;
        d.app.wb.queued.push("terminal.add_spares");
        d.frame();
        assert_eq!(names(&d, x1), ["1", "2", "3", "4", "5"]);
        undo(&mut d);
        assert_eq!(names(&d, x1), ["1", "2", "3"]);
    }

    #[test]
    fn rename_previews_then_applies_as_one_step() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        d.app.wb.queued.push("terminal.rename");
        d.frame();
        assert!(matches!(
            d.app.wb.terminal_edit.window,
            Some(EditWindow::Rename { .. })
        ));
        let project = d.app.backend.document().unwrap().project();
        let preview = novo_terminals::rename_preview(project, x1, "2", "7");
        assert_eq!(preview.len(), 1);
        assert!(preview[0].problem.is_none());
        // A clash is shown in the preview and refuses the edit.
        let clash = novo_terminals::rename_preview(project, x1, "2", "3");
        assert!(clash[0].problem.is_some());
        assert!(d.app.edit_strip("Rename terminals", |tx| {
            novo_terminals::rename(tx, x1, "2", "7").map(|_| ())
        }));
        assert_eq!(names(&d, x1), ["1", "3", "7"]);
        undo(&mut d);
        assert_eq!(names(&d, x1), ["1", "2", "3"]);
    }

    #[test]
    fn renumber_closes_the_gaps_as_one_step() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        let doc = d.app.backend.document_mut().unwrap();
        let t = doc.project().terminals_of_strip(x1)[1].id;
        doc.transact("rename", |tx| {
            tx.update::<novo_core::Terminal>(t, |t| t.name = "9".into())
        })
        .unwrap();
        d.app.wb.queued.push("terminal.renumber");
        d.frame();
        assert!(matches!(
            d.app.wb.terminal_edit.window,
            Some(EditWindow::Renumber)
        ));
        assert!(d.app.edit_strip("Renumber terminals", |tx| {
            novo_terminals::renumber(tx, x1)
        }));
        let after = names(&d, x1);
        assert_eq!(after.len(), 3);
        assert!(after.contains(&"2".to_owned()));
        undo(&mut d);
        assert!(names(&d, x1).contains(&"9".to_owned()));
    }

    #[test]
    fn set_kind_changes_the_ticked_rows() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        let t = d
            .app
            .backend
            .document()
            .unwrap()
            .project()
            .terminals_of_strip(x1)[0]
            .id;
        d.app.wb.terminal_edit.rows = vec![t];
        d.app
            .terminal_edit_command("terminal.set_kind", Some("fuse"));
        let kind = |d: &Drive| {
            d.app
                .backend
                .document()
                .unwrap()
                .project()
                .terminal(t)
                .unwrap()
                .kind
        };
        assert_eq!(kind(&d), TerminalKind::Fuse);
        undo(&mut d);
        assert_eq!(kind(&d), TerminalKind::Feedthrough);
        assert_eq!(parse_kind("PE"), Some(TerminalKind::ProtectiveEarth));
    }

    #[test]
    fn moving_a_row_up_and_down_reorders_the_strip() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        let ids = |d: &Drive| -> Vec<TerminalId> {
            d.app
                .backend
                .document()
                .unwrap()
                .project()
                .terminals_of_strip(x1)
                .into_iter()
                .map(|t| t.id)
                .collect()
        };
        let first = ids(&d);
        d.app.wb.terminal_edit.rows = vec![first[2]];
        d.app.wb.queued.push("terminal.order_up");
        d.frame();
        assert_eq!(ids(&d), [first[0], first[2], first[1]]);
        d.app.wb.queued.push("terminal.order_down");
        d.frame();
        assert_eq!(ids(&d), first);
        undo(&mut d);
        assert_eq!(ids(&d), [first[0], first[2], first[1]]);
    }

    #[test]
    fn the_join_message_reports_the_count_and_a_failed_edit_says_nothing_else() {
        assert_eq!(join_message(0), "No duplicate terminals on this strip");
        assert_eq!(join_message(2), "Joined 2 duplicate terminal records");
        // A failing edit returns false (and reports its own error) so the
        // "no duplicates" line is not posted on top of it.
        let (_dir, mut d, _, _) = drive_with_strip();
        assert!(!d.app.edit_strip("Join duplicate terminals", |tx| {
            tx.remove(novo_core::WireId(u64::MAX))
        }));
    }

    #[test]
    fn join_duplicates_runs_from_the_queue() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        d.app.wb.queued.push("terminal.join_duplicates");
        d.frame();
        assert_eq!(names(&d, x1).len(), 3);
    }

    #[test]
    fn strip_style_goes_through_one_undo_step() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        d.app.wb.queued.push("terminal.style");
        d.frame();
        assert!(matches!(
            d.app.wb.terminal_edit.window,
            Some(EditWindow::Style)
        ));
        let style = Style {
            symbol: novo_core::SymbolKey(novo_terminals::default_symbol(TerminalKind::Fuse).into()),
            part: None,
        };
        assert!(d.app.edit_strip("Change strip style", |tx| {
            novo_terminals::set_strip_style(tx, x1, TerminalKind::Feedthrough, &style)
        }));
        let project = d.app.backend.document().unwrap().project();
        assert_eq!(
            novo_terminals::strip_style(project, x1, TerminalKind::Feedthrough).symbol,
            style.symbol
        );
        undo(&mut d);
        let project = d.app.backend.document().unwrap().project();
        assert_eq!(
            novo_terminals::strip_style(project, x1, TerminalKind::Feedthrough)
                .symbol
                .0,
            novo_terminals::default_symbol(TerminalKind::Feedthrough)
        );
    }

    #[test]
    fn restoring_a_declined_proposal_forgets_the_decline() {
        let (_dir, mut d, _, _) = drive_with_strip();
        assert!(d.app.declined_wires().is_empty());
        d.app.wb.queued.push("terminal.restore_declined");
        d.frame();
    }

    fn terminal_ids(d: &Drive, strip: DeviceId) -> Vec<TerminalId> {
        d.app
            .backend
            .document()
            .unwrap()
            .project()
            .terminals_of_strip(strip)
            .into_iter()
            .map(|t| t.id)
            .collect()
    }

    fn jumpers(d: &Drive, strip: DeviceId) -> Vec<Option<u32>> {
        d.app
            .backend
            .document()
            .unwrap()
            .project()
            .terminals_of_strip(strip)
            .into_iter()
            .map(|t| t.jumper)
            .collect()
    }

    #[test]
    fn the_level_is_set_from_the_command_and_one_undo_step_takes_it_back() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        let ids = terminal_ids(&d, x1);
        d.app.wb.terminal_edit.rows = vec![ids[0], ids[1]];
        d.app.terminal_edit_command("terminal.set_level", Some("2"));
        let levels = |d: &Drive| -> Vec<u8> {
            d.app
                .backend
                .document()
                .unwrap()
                .project()
                .terminals_of_strip(x1)
                .into_iter()
                .map(|t| t.level)
                .collect()
        };
        assert_eq!(levels(&d), [2, 2, 1]);
        undo(&mut d);
        assert_eq!(levels(&d), [1, 1, 1]);
        // Without a level the window opens, showing the current one.
        d.app.terminal_edit_command("terminal.set_level", None);
        assert!(matches!(
            d.app.wb.terminal_edit.window,
            Some(EditWindow::Level(1))
        ));
    }

    #[test]
    fn a_level_clash_is_refused_and_said() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        let ids = terminal_ids(&d, x1);
        // A second terminal named "1" on level 2 is fine; moving another
        // "1" there would clash.
        d.app.wb.terminal_edit.rows = vec![ids[0]];
        d.app.terminal_edit_command("terminal.set_level", Some("2"));
        let dup = d
            .app
            .backend
            .document_mut()
            .unwrap()
            .transact("dup", |tx| tx.add_terminal(x1, "1"))
            .unwrap()
            .0;
        d.app.wb.terminal_edit.rows = vec![dup];
        d.app.terminal_edit_command("terminal.set_level", Some("2"));
        let project = d.app.backend.document().unwrap().project();
        assert_eq!(project.terminal(dup).unwrap().level, 1, "unchanged");
    }

    #[test]
    fn explicit_jumper_numbers_bridge_join_and_remove() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        let ids = terminal_ids(&d, x1);
        d.app.wb.terminal_edit.rows = vec![ids[0], ids[1]];
        d.app
            .terminal_edit_command("terminal.set_jumper", Some("7"));
        assert_eq!(jumpers(&d, x1), [Some(7), Some(7), None]);
        // A number in use joins that group.
        d.app.wb.terminal_edit.rows = vec![ids[2]];
        d.app
            .terminal_edit_command("terminal.set_jumper", Some("J7"));
        assert_eq!(jumpers(&d, x1), [Some(7), Some(7), Some(7)]);
        // The next free number starts a new group.
        d.app.wb.terminal_edit.rows = vec![ids[2]];
        d.app
            .terminal_edit_command("terminal.set_jumper", Some("next"));
        assert_eq!(jumpers(&d, x1), [Some(7), Some(7), Some(8)]);
        d.app
            .terminal_edit_command("terminal.set_jumper", Some("none"));
        assert_eq!(jumpers(&d, x1), [Some(7), Some(7), None]);
        undo(&mut d);
        assert_eq!(jumpers(&d, x1), [Some(7), Some(7), Some(8)]);
        // Zero is refused and changes nothing; no number opens the window.
        d.app
            .terminal_edit_command("terminal.set_jumper", Some("0"));
        assert_eq!(jumpers(&d, x1), [Some(7), Some(7), Some(8)]);
        d.app.terminal_edit_command("terminal.set_jumper", None);
        assert!(matches!(
            d.app.wb.terminal_edit.window,
            Some(EditWindow::Jumper(9))
        ));
    }

    #[test]
    fn the_level_and_jumper_windows_open_and_apply_from_their_buttons() {
        let (_dir, mut d, x1, _) = drive_with_strip();
        let ids = terminal_ids(&d, x1);
        d.app.wb.terminal_edit.rows = vec![ids[0]];
        d.app.wb.queued.push("terminal.set_jumper");
        d.frame();
        assert!(matches!(
            d.app.wb.terminal_edit.window,
            Some(EditWindow::Jumper(1))
        ));
        assert_eq!(
            super::jumper_groups(d.app.backend.document().unwrap().project(), x1),
            Vec::<(u32, Vec<String>)>::new()
        );
    }
}
