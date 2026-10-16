//! The fast-keys walk-through (docs/ONBOARDING-DESIGN.md §2 and §5): a small
//! card over the canvas that lists seven steps and ticks each one when the
//! person does it, on a practice project or any other. It never takes a key
//! and never edits a project; the practice project is the only thing it
//! creates, and only when asked.

use eframe::egui;
use novo_core::{Document, Placement, Point, Tx};
use novo_editor::edit::{PlaceOptions, place};
use novo_editor::{Tool, naming::Numbering};
use novo_symbols::Library;
use novo_theme::widgets::section_label;

use super::{Level, NovoApp};

/// What the practice project is called.
pub(crate) const PRACTICE_NAME: &str = "Practice";

/// One step of the walk-through.
pub(crate) struct Step {
    /// Stable id kept in the settings.
    pub id: &'static str,
    pub title: &'static str,
    /// The command whose key the step shows, so a rebound key reads right.
    pub command: &'static str,
    /// What to do; `{key}` becomes the command's key.
    pub text: &'static str,
}

/// The seven steps, in the order a hand wants them.
pub(crate) const STEPS: [Step; 7] = [
    Step {
        id: "place",
        title: "Place",
        command: "tool.place",
        text: "Tap {key} and type a name, or hold it for the wheel and flick. R and X turn and mirror.",
    },
    Step {
        id: "proposal",
        title: "Proposals",
        command: "proposal.next",
        text: "{key} jumps to the next proposed wire and Enter accepts it. Put two devices facing each other to get one.",
    },
    Step {
        id: "wire",
        title: "Wire",
        command: "tool.connect",
        text: "{key}, then click pin to pin. A drag from a pin does the same, but only the key ticks this step.",
    },
    Step {
        id: "cable",
        title: "Cable",
        command: "tool.cable",
        text: "{key}, click a line over the wires that belong together, then Enter.",
    },
    Step {
        id: "duplicate",
        title: "Duplicate",
        command: "edit.duplicate",
        text: "Select a device and press {key}. Again repeats it with the next free tag.",
    },
    Step {
        id: "live",
        title: "Live view",
        command: "sim.live",
        text: "{key} closes the contacts and shows what each wire carries. Try it on the example.",
    },
    Step {
        id: "commands",
        title: "Every command",
        command: "view.command_palette",
        text: "{key} lists everything. Nothing lives only in a menu.",
    },
];

/// Draws the practice project into a project that has only its empty first
/// page: a motor and three terminals, so a wire and then a cable can be
/// drawn between them without placing anything first.
pub(crate) fn build_practice(doc: &mut Document) -> novo_core::Result<novo_core::ChangeSet> {
    let library = Library::builtin().map_err(|e| novo_core::ModelError::Refused {
        what: "Practice project".into(),
        reason: e.to_string(),
    })?;
    let ((), changes) = doc.transact("Practice project", |tx: &mut Tx<'_>| {
        let page = tx
            .project()
            .pages_in_order()
            .first()
            .map(|p| p.id)
            .ok_or_else(|| novo_core::ModelError::Refused {
                what: "Practice project".into(),
                reason: "it has no page".into(),
            })?;
        let options = PlaceOptions {
            numbering: Numbering::PerLetter,
            ..PlaceOptions::new()
        };
        let put = |tx: &mut Tx<'_>, key: &str, x: f64, y: f64| -> novo_core::Result<()> {
            let symbol = library
                .get(key)
                .ok_or_else(|| novo_core::ModelError::Refused {
                    what: "Practice project".into(),
                    reason: format!("the symbol library has no `{key}`"),
                })?;
            place(tx, page, symbol, Placement::at(Point::new(x, y)), &options)?;
            Ok(())
        };
        for x in [40.0, 48.0, 56.0] {
            put(tx, "iec/terminals/terminal", x, 40.0)?;
        }
        put(tx, "iec/motors/motor_3ph", 40.0, 96.0)
    })?;
    Ok(changes)
}

impl NovoApp {
    /// Whether the card is on show: the walk-through was started, not
    /// skipped, and a project is open.
    pub(super) fn walkthrough_on(&self) -> bool {
        let o = &self.settings.onboarding;
        o.started && !o.dismissed && self.is_open() && !self.learn_active()
    }

    /// First run: nothing opened before and the walk-through not yet
    /// started. The start panel points at it then, and nothing else.
    pub(super) fn first_run(&self) -> bool {
        !self.settings.onboarding.started
            && self.settings.last_open.is_none()
            && self.settings.recent_projects.is_empty()
    }

    /// "Try the fast keys": a fresh practice project, with the card.
    pub(super) fn start_practice(&mut self) {
        if let Err(e) = self.backend.new_project(PRACTICE_NAME) {
            self.message(Level::Error, format!("Could not start the practice: {e}"));
            return;
        }
        self.project_path = None;
        self.current_page = None;
        let built = self.backend.document_mut().map(build_practice);
        match built {
            Some(Ok(changes)) => self.backend.record(&changes),
            Some(Err(e)) => self.message(Level::Error, format!("Could not draw the practice: {e}")),
            None => {}
        }
        self.document_replaced();
        self.fix_current_page();
        if let Some(doc) = self.backend.document_mut() {
            doc.mark_saved();
        }
        self.settings.onboarding.restart();
    }

    /// "Fast keys walk-through": the card again on whatever is open, from
    /// the first step.
    pub(super) fn show_walkthrough(&mut self) {
        self.settings.onboarding.restart();
    }

    /// A step the person just did. Only counts while the card is on.
    pub(super) fn tour_tick(&mut self, step: &str) {
        if self.walkthrough_on() {
            self.settings.onboarding.tick(step);
        }
    }

    /// Ticks from what the person is doing now: the tool in hand, and Tab or
    /// Enter while the drawing offers proposals. Reads state only.
    pub(super) fn tour_watch(&mut self, ctx: &egui::Context) {
        if !self.walkthrough_on() {
            return;
        }
        let Some(view) = self.canvas.schematic_ref() else {
            return;
        };
        let step = match view.editor.tool() {
            Tool::Place(_) => Some("place"),
            Tool::Wire(_) => Some("wire"),
            Tool::Cable(_) => Some("cable"),
            _ => None,
        };
        let proposals = !view.editor.proposals().is_empty() || self.terminal_proposals_have_keys();
        let pressed = proposals
            && ctx.input(|i| {
                !i.modifiers.any()
                    && (i.key_pressed(egui::Key::Tab) || i.key_pressed(egui::Key::Enter))
            });
        if let Some(step) = step {
            self.tour_tick(step);
        }
        if pressed {
            self.tour_tick("proposal");
        }
    }

    fn step_key(&self, step: &Step) -> String {
        self.wb
            .keymap
            .shortcut_text(step.command)
            .unwrap_or_else(|| "(no key)".to_owned())
    }

    /// The card, bottom right of the canvas. It steps aside to a pill while
    /// the live view is on.
    pub(super) fn walkthrough_card(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.walkthrough_on() {
            return;
        }
        let done = STEPS
            .iter()
            .filter(|s| self.settings.onboarding.is_ticked(s.id))
            .count();
        let next = STEPS
            .iter()
            .position(|s| !self.settings.onboarding.is_ticked(s.id));
        let rows: Vec<(bool, String, String, String)> = STEPS
            .iter()
            .map(|s| {
                let key = self.step_key(s);
                (
                    self.settings.onboarding.is_ticked(s.id),
                    s.title.to_owned(),
                    key.clone(),
                    s.text.replace("{key}", &key),
                )
            })
            .collect();
        let pill = self.wb.live.on;
        let on_example = self.project_name().as_deref() == Some(super::example::EXAMPLE_NAME);
        let mut example = false;
        let mut close = false;
        super::panel::Panel::new(
            "walkthrough",
            "Fast keys",
            canvas,
            super::panel::Side::BottomRight,
            300.0,
        )
        .fixed_open()
        .fit()
        .show(ctx, |ui| {
            if pill {
                ui.weak(format!("Fast keys {done} of {}", STEPS.len()));
                return;
            }
            ui.set_max_width(300.0);
            ui.horizontal(|ui| {
                section_label(ui, &format!("Fast keys · {done} of {}", STEPS.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if next.is_none() { "Close" } else { "Skip" };
                    if ui.small_button(label).clicked() {
                        close = true;
                    }
                });
            });
            for (i, (ticked, title, key, text)) in rows.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(if *ticked { "✓" } else { "○" });
                    let title = egui::RichText::new(format!("{key}  {title}"));
                    ui.label(if *ticked {
                        title.weak()
                    } else {
                        title.strong()
                    });
                });
                if next == Some(i) {
                    ui.indent("walkthrough_text", |ui| {
                        ui.label(text);
                        if STEPS[i].id == "live"
                            && !on_example
                            && ui.link("Open the example").clicked()
                        {
                            example = true;
                        }
                    });
                }
            }
            ui.add_space(2.0);
            if done < 3 {
                ui.weak("A tick means the tool opened, not that something was drawn.");
            }
            if next.is_some() {
                let key = self.step_key(&STEPS[STEPS.len() - 1]);
                ui.weak(format!(
                    "Skip hides this. {key}, then fast keys, brings it back."
                ));
            }
            if next.is_none() {
                ui.add_space(4.0);
                ui.label("That is all of them. The command field finds the rest.");
            }
        });
        if close {
            self.settings.onboarding.dismissed = true;
        }
        if example {
            self.open_example();
        }
    }
}
