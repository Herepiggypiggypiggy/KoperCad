//! Onboarding (docs/ONBOARDING-DESIGN.md): an example project and the
//! fast-keys walk-through, a card whose six steps tick when the person really
//! does the thing. Thin UI over existing commands: nothing here edits the
//! model except by building the example and the practice project.

use eframe::egui;
use novo_core::{DeviceTag, Document, ModelError, Placement, Point, Project, SymbolRole};
use novo_theme::{tokens, widgets};

use super::{Level, NovoApp};

/// Name of the example project.
const EXAMPLE_NAME: &str = "Motor starter (example)";

/// Name of the practice project the walk-through starts on.
const PRACTICE_NAME: &str = "Fast keys practice";

/// One thing to try. The key shown is the keymap's, so a remapped key is
/// shown as it is now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    Place,
    Wire,
    Rotate,
    Duplicate,
    Cable,
    Palette,
}

impl Step {
    pub(super) const ALL: [Step; 6] = [
        Step::Place,
        Step::Wire,
        Step::Rotate,
        Step::Duplicate,
        Step::Cable,
        Step::Palette,
    ];

    /// Stored in the settings.
    pub(super) fn id(self) -> &'static str {
        match self {
            Step::Place => "place",
            Step::Wire => "wire",
            Step::Rotate => "rotate",
            Step::Duplicate => "duplicate",
            Step::Cable => "cable",
            Step::Palette => "palette",
        }
    }

    /// The keymap command whose key the step teaches.
    pub(super) fn command(self) -> &'static str {
        match self {
            Step::Place => "tool.place",
            Step::Wire => "tool.connect",
            Step::Rotate => "edit.rotate",
            Step::Duplicate => "edit.duplicate",
            Step::Cable => "tool.cable",
            Step::Palette => "view.command_palette",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Step::Place => "Place a symbol",
            Step::Wire => "Draw a wire",
            Step::Rotate => "Rotate",
            Step::Duplicate => "Duplicate",
            Step::Cable => "Draw a cable",
            Step::Palette => "Search everything",
        }
    }

    /// One line: what to do. `key` is the step's key as the keymap has it.
    fn hint(self, key: &str) -> String {
        match self {
            Step::Place => format!("Hold {key} for the wheel, pick a symbol, click to drop it."),
            Step::Wire => format!("Press {key}, click a pin, then the pin it goes to."),
            Step::Rotate => format!("Select a symbol and press {key}."),
            Step::Duplicate => format!("Select a symbol: {key} copies it with the next free tag."),
            Step::Cable => format!("Press {key}, click along the wires of one cable, then Enter."),
            Step::Palette => format!("{key} finds any command, symbol or page."),
        }
    }

    /// The step an undo label stands for, or `None`. The labels are the ones
    /// the editor and the shell give the transactions of these gestures; a
    /// paste, a duplicate by menu or an undo has another label, so the
    /// step cannot tick without its key.
    pub(super) fn from_label(label: &str) -> Option<Step> {
        match label {
            "Draw wire" => Some(Step::Wire),
            "Rotate" => Some(Step::Rotate),
            "Duplicate" => Some(Step::Duplicate),
            "Draw cable" => Some(Step::Cable),
            l if l.starts_with("Place ") => Some(Step::Place),
            _ => None,
        }
    }
}

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("the practice tags are valid")
}

/// The practice project: one page with a motor `-M1` and a terminal strip
/// `-X1` of three terminals already placed, so every step including the
/// cable can be tried. One undo step.
pub(super) fn practice_document() -> Result<Document, ModelError> {
    let mut doc = Document::new(Project::new(PRACTICE_NAME));
    doc.transact("Practice project", |tx| {
        let page = tx.add_page("1")?;
        tx.update::<novo_core::Page>(page, |p| p.title = "Practice".into())?;
        let strip = tx.add_terminal_strip(tag("-X1"))?;
        for (name, x) in [("1", 40.0), ("2", 48.0), ("3", 56.0)] {
            let terminal = tx.add_terminal(strip, name)?;
            tx.place_symbol(
                page,
                "iec/terminals/terminal",
                Placement::at(Point::new(x, 60.0)),
                vec![
                    novo_core::Pin::new("1", 0.0, 0.0),
                    novo_core::Pin::new("2", 0.0, 8.0),
                ],
                SymbolRole::Terminal {
                    terminals: vec![terminal],
                },
            )?;
        }
        let motor = tx.add_device(tag("-M1"), "Practice motor")?;
        tx.place_symbol(
            page,
            "iec/motors/motor_3ph",
            Placement::at(Point::new(40.0, 100.0)),
            vec![
                novo_core::Pin::new("U1", 0.0, 0.0),
                novo_core::Pin::new("V1", 8.0, 0.0),
                novo_core::Pin::new("W1", 16.0, 0.0),
                novo_core::Pin::new("PE", 24.0, 0.0),
            ],
            SymbolRole::Device {
                device: motor,
                main: true,
            },
        )?;
        Ok(())
    })?;
    Ok(doc)
}

/// The example project: the Gate 1 motor starter, one undo step.
fn example_document() -> Result<Document, ModelError> {
    let mut doc = Document::new(Project::new(EXAMPLE_NAME));
    novo_tests::motor_starter(&mut doc)?;
    // Joins the fixture's step, so the whole example is one step to undo.
    doc.amend_last("Draw example", |tx| {
        tx.set_info(|i| i.name = EXAMPLE_NAME.into())
    })?;
    Ok(doc)
}

impl NovoApp {
    /// Open example: the motor starter as a new, untitled project.
    pub(super) fn open_example(&mut self) {
        match example_document() {
            Ok(doc) => self.start_from(doc, "the example"),
            Err(e) => self.message(Level::Error, format!("Could not build the example: {e}")),
        }
    }

    /// Try the fast keys: with a project open, the card starts again on it;
    /// with none, on a practice project.
    pub(super) fn fast_keys(&mut self) {
        if !self.is_open() {
            match practice_document() {
                Ok(doc) => self.start_from(doc, "the practice project"),
                Err(e) => {
                    self.message(Level::Error, format!("Could not build the practice: {e}"));
                    return;
                }
            }
        }
        self.settings.walkthrough_done.clear();
        self.settings.walkthrough_shown = true;
        self.settings.walkthrough_seen = true;
        self.wb.walkthrough_collapsed = false;
    }

    /// Replaces the open project with an unsaved one made from `doc`.
    fn start_from(&mut self, doc: Document, what: &str) {
        match self.backend.new_project_from(doc) {
            Ok(()) => {
                self.project_path = None;
                self.current_page = None;
                self.document_replaced();
                self.fix_current_page();
            }
            Err(e) => self.message(Level::Error, format!("Could not open {what}: {e}")),
        }
    }

    /// True on a first run: nothing opened before and the walk-through never
    /// started or skipped. Only the start panel's hint uses it; nothing opens
    /// by itself.
    pub(super) fn first_run(&self) -> bool {
        !self.settings.walkthrough_seen
            && self.settings.last_open.is_none()
            && self.settings.recent_projects.is_empty()
    }

    fn step_done(&self, step: Step) -> bool {
        self.settings
            .walkthrough_done
            .iter()
            .any(|id| id == step.id())
    }

    fn done_count(&self) -> usize {
        Step::ALL.iter().filter(|s| self.step_done(**s)).count()
    }

    /// A step was really done. Ticks it while the card is showing; when the
    /// sixth ticks the card goes away for good.
    pub(super) fn step_heard(&mut self, step: Step) {
        if !self.settings.walkthrough_shown || self.step_done(step) {
            return;
        }
        self.settings.walkthrough_done.push(step.id().to_owned());
        if self.done_count() == Step::ALL.len() {
            self.settings.walkthrough_shown = false;
            self.message(
                Level::Info,
                "All six fast keys done. Try the fast keys in the menu shows the card again",
            );
        }
    }

    /// The one funnel for steps taken by editing: called after the canvas
    /// or the shell committed an edit, it reads what the document calls the
    /// step just taken. Undo and redo do not come through here.
    pub(super) fn steps_after_edit(&mut self) {
        if !self.settings.walkthrough_shown {
            return;
        }
        let step = self
            .backend
            .document()
            .and_then(|doc| doc.undo_label())
            .and_then(Step::from_label);
        if let Some(step) = step {
            self.step_heard(step);
        }
    }

    /// Skip: the card never comes back by itself.
    fn skip_walkthrough(&mut self) {
        self.settings.walkthrough_shown = false;
        self.settings.walkthrough_seen = true;
    }

    /// The card, bottom right of the canvas above the tool dock's corner
    /// spot. It never takes input from the canvas and steps aside, folded to
    /// a pill with the count, while the live view is on.
    pub(super) fn walkthrough_card(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.settings.walkthrough_shown || !self.is_open() {
            return;
        }
        let count = self.done_count();
        let total = Step::ALL.len();
        let folded = self.wb.walkthrough_collapsed || self.wb.live.on;
        let rows: Vec<(Step, bool, String)> = Step::ALL
            .iter()
            .map(|&s| {
                let key = self
                    .wb
                    .keymap
                    .shortcut_text(s.command())
                    .unwrap_or_else(|| "?".to_owned());
                (s, self.step_done(s), key)
            })
            .collect();
        let mut skip = false;
        let mut fold = None;
        egui::Area::new(egui::Id::new("walkthrough_card"))
            .order(egui::Order::Middle)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(canvas.right_bottom() - egui::vec2(12.0, 72.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    if folded {
                        let pill = ui
                            .add(
                                egui::Button::new(format!("Fast keys {count} of {total}"))
                                    .frame(false),
                            )
                            .on_hover_text("Show the fast keys card");
                        if pill.clicked() && !self.wb.live.on {
                            fold = Some(false);
                        }
                        return;
                    }
                    ui.set_max_width(300.0);
                    ui.horizontal(|ui| {
                        widgets::section_label(ui, "Try the fast keys");
                        ui.weak(format!("{count} of {total}"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("Skip")
                                .on_hover_text("Close for good")
                                .clicked()
                            {
                                skip = true;
                            }
                            if ui.small_button("–").on_hover_text("Fold").clicked() {
                                fold = Some(true);
                            }
                        });
                    });
                    ui.add_space(4.0);
                    for (step, done, key) in &rows {
                        ui.horizontal_top(|ui| {
                            let mark = if *done { "✓" } else { "○" };
                            let color = if *done {
                                tokens::SUCCESS
                            } else {
                                tokens::TEXT_FAINT
                            };
                            ui.label(egui::RichText::new(mark).color(color));
                            ui.label(
                                egui::RichText::new(key.as_str())
                                    .monospace()
                                    .color(tokens::ACCENT_HOVER),
                            );
                            ui.vertical(|ui| {
                                let title = egui::RichText::new(step.title());
                                ui.label(if *done { title.weak() } else { title });
                                if !*done {
                                    ui.weak(step.hint(key));
                                }
                            });
                        });
                    }
                });
            });
        if let Some(fold) = fold {
            self.wb.walkthrough_collapsed = fold;
        }
        if skip {
            self.skip_walkthrough();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_gestures_own_labels_stand_for_a_step() {
        assert_eq!(Step::from_label("Place Contactor"), Some(Step::Place));
        assert_eq!(Step::from_label("Place K x3"), Some(Step::Place));
        assert_eq!(Step::from_label("Draw wire"), Some(Step::Wire));
        assert_eq!(Step::from_label("Rotate"), Some(Step::Rotate));
        assert_eq!(Step::from_label("Duplicate"), Some(Step::Duplicate));
        assert_eq!(Step::from_label("Draw cable"), Some(Step::Cable));
        // Paste, delete and a moved wire end must not tick anything.
        assert_eq!(Step::from_label("Paste"), None);
        assert_eq!(Step::from_label("Delete"), None);
        assert_eq!(Step::from_label("Move wire end"), None);
        assert_eq!(Step::from_label("Accept cables"), None);
    }

    #[test]
    fn every_step_names_a_distinct_keymap_command() {
        let keymap = novo_workflow::keymap::Keymap::workflow();
        let mut seen = Vec::new();
        for step in Step::ALL {
            assert!(
                keymap.shortcut_text(step.command()).is_some(),
                "{:?} has a key",
                step
            );
            assert!(!seen.contains(&step.command()));
            seen.push(step.command());
        }
    }

    #[test]
    fn the_practice_project_is_one_undo_step_with_a_motor_and_three_terminals() {
        let mut doc = practice_document().expect("builds");
        assert_eq!(doc.project().symbols().count(), 4);
        assert!(doc.undo().expect("undo").is_some());
        assert_eq!(doc.project().symbols().count(), 0);
        assert!(!doc.can_undo());
    }

    #[test]
    fn the_example_is_one_undo_step_named_as_an_example() {
        let mut doc = example_document().expect("builds");
        assert_eq!(doc.project().info().name, EXAMPLE_NAME);
        assert_eq!(doc.project().pages().count(), 2);
        assert!(doc.undo().expect("undo").is_some());
        assert!(!doc.can_undo());
    }
}
