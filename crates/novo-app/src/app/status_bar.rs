//! The status bar (docs/design/workspaces.md §7): 22 px at the bottom with
//! five slots. The last message, what is selected, the modes that are on
//! (each a button that turns it off), the cursor, and whether the project is
//! saved. It replaces the floating HUD and the info toasts; warnings and
//! errors still use toasts, since they must be dismissed.

use std::time::Duration;

use eframe::egui;
use novo_core::AnyId;

use super::workspace::Workspace;
use super::{Level, NovoApp, TOAST_INFO};

/// Height of the bar.
pub(super) const STATUS_BAR: f32 = 22.0;

/// A mode that is on, with the command that turns it off.
struct ModeChip {
    label: &'static str,
    command: &'static str,
    hover: &'static str,
}

impl NovoApp {
    /// Whether the bar is on screen this frame: not in Focus mode, and not
    /// switched off in the settings.
    pub(super) fn status_bar_shown(&self) -> bool {
        !self.focus_mode && !self.settings.hide_status_bar
    }

    /// The bar, as the lowest panel of the window.
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        if !self.status_bar_shown() {
            return;
        }
        let message = self.status_message(ui.ctx());
        let tool = if self.workspace() == Workspace::Schematic && self.is_open() {
            self.tool_hint()
        } else {
            super::dock::ToolHint::default()
        };
        let selection = self.selection_summary();
        let values = self.status_values();
        let chips = self.mode_chips();
        let saved = self.saved_state();
        let path = self.project_path.clone();
        let recent = self.recent_messages();
        let unsaved = matches!(saved, "Not saved yet" | "Not saved");
        let mut run = None;
        // Where the hint and the proposal count are, for the lessons.
        let mut spots: Vec<(&'static str, egui::Rect, &'static str)> = Vec::new();
        egui::Panel::bottom("status_bar")
            .exact_size(STATUS_BAR)
            .show(ui, |ui| {
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                ui.visuals_mut().override_text_color =
                    Some(crate::theme::ui_palette().text_secondary);
                ui.horizontal_centered(|ui| {
                    // The message gives way to the slots on its right: it is
                    // cut to half the bar, with the whole text on hover.
                    let room = ui.available_width() * 0.5;
                    ui.scope(|ui| {
                        ui.set_max_width(room);
                        // A refusal, a step in progress and the rest of a
                        // device outrank a message; only the empty-page tip
                        // gives way to one.
                        let hint = (tool.refused || !tool.tip || message.is_none())
                            .then_some(&tool.text)
                            .filter(|t| !t.is_empty());
                        match (hint, &message) {
                            (Some(text), _) if tool.refused => {
                                let text = format!("{} {text}", novo_theme::icons::status::WARNING);
                                let label = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&text)
                                            .color(novo_theme::Palette::of(ui).error),
                                    )
                                    .truncate(),
                                );
                                spots.push(("dock.hint", label.rect, "hint"));
                                label.on_hover_text(text);
                            }
                            (Some(text), _) => {
                                let label = ui.add(egui::Label::new(text).truncate());
                                spots.push(("dock.hint", label.rect, "hint"));
                                label.on_hover_text(text);
                            }
                            (None, Some((text, _))) => {
                                let label = ui.add(egui::Label::new(text).truncate());
                                label.on_hover_text(text);
                            }
                            (None, None) => {
                                let hint =
                                    ui.add(egui::Label::new(self.workspace_hint()).truncate());
                                if !recent.is_empty() {
                                    hint.on_hover_text(format!("Recent:\n{}", recent.join("\n")));
                                }
                            }
                        }
                    });
                    if tool.proposals > 0 {
                        ui.separator();
                        let label = ui.label(format!("{} proposed", tool.proposals));
                        spots.push(("dock.proposals", label.rect, "proposals"));
                        label.on_hover_text("↵ accept · Tab next · Delete decline");
                    }
                    if let Some(selection) = &selection {
                        ui.separator();
                        ui.label(selection);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !saved.is_empty() {
                            let label = ui.add(egui::Label::new(saved).sense(if unsaved {
                                egui::Sense::click()
                            } else {
                                egui::Sense::hover()
                            }));
                            let mut tip = path
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();
                            if unsaved {
                                if !tip.is_empty() {
                                    tip.push('\n');
                                }
                                tip.push_str("Click to save");
                                if label.clicked() {
                                    run = Some("file.save");
                                }
                            }
                            let label = if unsaved {
                                label.on_hover_cursor(egui::CursorIcon::PointingHand)
                            } else {
                                label
                            };
                            if !tip.is_empty() {
                                label.on_hover_text(tip);
                            }
                        }
                        if let Some(values) = &values {
                            ui.separator();
                            ui.label(egui::RichText::new(values).monospace());
                        }
                        for chip in &chips {
                            let button = egui::Button::new(
                                egui::RichText::new(chip.label)
                                    .color(crate::theme::ui_palette().text_primary),
                            )
                            .fill(crate::theme::ui_palette().control)
                            .small();
                            if ui.add(button).on_hover_text(chip.hover).clicked() {
                                run = Some(chip.command);
                            }
                        }
                    });
                });
            });
        for (id, rect, label) in spots {
            self.anchor(id, rect, label);
        }
        if let Some(command) = run {
            self.wb.queued.push(command);
        }
    }

    /// The newest info message while it is fresh, with the time it has
    /// left. A repaint is asked for when it expires.
    fn status_message(&self, ctx: &egui::Context) -> Option<(String, Duration)> {
        let now = std::time::Instant::now();
        let message = self
            .messages
            .iter()
            .rev()
            .find(|m| m.level == Level::Info && !m.dismissed)?;
        let left = TOAST_INFO.checked_sub(now - message.at)?;
        ctx.request_repaint_after(left);
        Some((message.text.clone(), left))
    }

    /// The last three info messages, oldest first, for the hover on the
    /// idle hint: a message that faded is still one hover away.
    fn recent_messages(&self) -> Vec<String> {
        let mut texts: Vec<String> = self
            .messages
            .iter()
            .rev()
            .filter(|m| m.level == Level::Info)
            .take(3)
            .map(|m| m.text.clone())
            .collect();
        texts.reverse();
        texts
    }

    /// What to say when nothing else is happening.
    fn workspace_hint(&self) -> &'static str {
        if !self.is_open() {
            return "";
        }
        match self.workspace() {
            Workspace::Schematic => "Q or Space to place · Ctrl+K to search",
            Workspace::Cabinet => "Drag a device onto the plate · Ctrl+K to search",
            _ => "",
        }
    }

    /// "-K1" for one device, "3 selected" for several, `None` for none.
    fn selection_summary(&self) -> Option<String> {
        if self.workspace() == Workspace::Reports {
            return self.reports_picked();
        }
        if self.workspace() != Workspace::Schematic {
            return None;
        }
        let selection = self.selection();
        match selection.as_slice() {
            [] => None,
            [only] => {
                let project = self.backend.document()?.project();
                let device = novo_editor::edit::device_of(project, *only)
                    .and_then(|id| project.device(id))
                    .map(|d| d.tag.to_string());
                Some(device.unwrap_or_else(|| match only {
                    AnyId::Wire(_) => "1 wire".to_owned(),
                    _ => "1 selected".to_owned(),
                }))
            }
            many => Some(format!("{} selected", many.len())),
        }
    }

    /// The cursor, in the workspaces that have one.
    fn status_values(&self) -> Option<String> {
        match self.workspace() {
            Workspace::Schematic => self
                .canvas
                .schematic_ref()
                .and_then(|view| view.editor.cursor())
                .map(|p| format!("X {:.1} mm  Y {:.1} mm", p.x, p.y)),
            Workspace::Cabinet => self.canvas.status_text(),
            _ => None,
        }
    }

    /// The modes that are on, each with the command that switches it off.
    fn mode_chips(&self) -> Vec<ModeChip> {
        let mut chips = Vec::new();
        if self.workspace() != Workspace::Schematic {
            return chips;
        }
        if !self.wb.marks {
            chips.push(ModeChip {
                label: "Marks hidden",
                command: "view.toggle_marks",
                hover: "Show the marks again",
            });
        }
        if self.wb.live.on {
            chips.push(ModeChip {
                label: "Live",
                command: "sim.live",
                hover: "Switch live potentials off",
            });
        }
        if self.wb.changes.is_some() {
            chips.push(ModeChip {
                label: "Changes",
                command: "view.changes",
                hover: "Leave Changes mode",
            });
        }
        chips
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Action;
    use crate::app::gate1_tests::Drive;

    #[test]
    fn the_bar_hides_in_focus_mode_and_by_setting() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        assert!(d.app.status_bar_shown());
        d.app.focus_mode = true;
        assert!(!d.app.status_bar_shown());
        d.app.focus_mode = false;
        d.app.settings.hide_status_bar = true;
        assert!(!d.app.status_bar_shown());
    }

    #[test]
    fn info_messages_land_in_the_bar_and_errors_stay_toasts() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        d.app.message(Level::Info, "Saved it");
        assert_eq!(
            d.app.status_message(&ctx).map(|(t, _)| t).as_deref(),
            Some("Saved it")
        );
        d.app.message(Level::Error, "Broke");
        // An error does not replace the info line; it is a toast.
        assert_eq!(
            d.app.status_message(&ctx).map(|(t, _)| t).as_deref(),
            Some("Saved it")
        );
    }

    #[test]
    fn modes_show_as_chips_that_name_their_command() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        assert!(d.app.mode_chips().is_empty());
        d.app.wb.marks = false;
        let chips = d.app.mode_chips();
        assert_eq!(chips.len(), 1);
        assert_eq!(chips[0].command, "view.toggle_marks");
        // Every chip's command is a registered command.
        assert!(novo_workflow::commands::is_known(chips[0].command));
    }
}
