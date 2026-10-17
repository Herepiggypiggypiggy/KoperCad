//! The DXF background panel: move and scale the reference drawing under
//! the shown page. It edits the page properties `underlay.dx`, `underlay.dy`
//! (millimetres from the page's lower-left corner) and `underlay.scale`
//! (shown as a percentage), one undo step per finished edit, never one per
//! frame of a drag. The canvas redraws when those properties change.

use eframe::egui;
use novo_core::{Command, Properties, Value};
use novo_interop::underlay as bg;

use super::NovoApp;

/// The default placement `import_underlay` writes.
const DEFAULT_OFFSET_MM: f64 = 10.0;

/// Placement of the background in the panel's units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    pub dx: f64,
    pub dy: f64,
    pub percent: f64,
}

impl Placement {
    pub(crate) fn of(props: &Properties) -> Placement {
        let number = |key: &str, default: f64| match props.get(key) {
            Some(Value::Float(v)) => *v,
            Some(Value::Int(v)) => *v as f64,
            _ => default,
        };
        Placement {
            dx: number(bg::KEY_DX, DEFAULT_OFFSET_MM),
            dy: number(bg::KEY_DY, DEFAULT_OFFSET_MM),
            percent: number(bg::KEY_SCALE, 1.0) * 100.0,
        }
    }

    /// The property edits that turn `self` into `to`, only for the values
    /// that differ. A scale of zero or less is not a drawing; it is ignored.
    pub(crate) fn edits(self, to: Placement) -> Vec<(&'static str, f64)> {
        let mut out = Vec::new();
        if to.dx != self.dx {
            out.push((bg::KEY_DX, to.dx));
        }
        if to.dy != self.dy {
            out.push((bg::KEY_DY, to.dy));
        }
        if to.percent != self.percent && to.percent > 0.0 {
            out.push((bg::KEY_SCALE, to.percent / 100.0));
        }
        out
    }
}

impl NovoApp {
    /// Opens the panel; it closes itself when the page has no background.
    pub(super) fn open_background_panel(&mut self) {
        if !self.has_background() {
            self.message(
                super::Level::Info,
                "This page has no DXF background. Import one first",
            );
            return;
        }
        self.wb.background_panel = Some(None);
    }

    pub(super) fn background_panel(&mut self, ctx: &egui::Context) {
        let Some(draft) = self.wb.background_panel else {
            return;
        };
        let Some(props) = self
            .page_properties()
            .filter(|p| bg::stored_text(p).is_some())
        else {
            self.wb.background_panel = None;
            return;
        };
        let stored = Placement::of(props);
        let name = bg::stored_name(props).unwrap_or("DXF").to_owned();
        let mut shown = draft.unwrap_or(stored);
        let mut commit = false;
        let mut reset = false;
        let mut open = true;
        super::panel::dialog(ctx, "DXF background", [420.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                super::dialog_ui::hint(ui, &name);
                egui::Grid::new("background_grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        let row = |ui: &mut egui::Ui,
                                   label: &str,
                                   value: &mut f64,
                                   unit: &str,
                                   speed: f64| {
                            ui.label(label);
                            let r = ui.add(
                                egui::DragValue::new(value)
                                    .speed(speed)
                                    .suffix(unit)
                                    .max_decimals(2),
                            );
                            ui.end_row();
                            r
                        };
                        let responses = [
                            row(ui, "Across", &mut shown.dx, " mm", 0.5),
                            row(ui, "Up", &mut shown.dy, " mm", 0.5),
                            row(ui, "Scale", &mut shown.percent, " %", 0.2),
                        ];
                        for r in &responses {
                            commit |= r.drag_stopped() || r.lost_focus();
                        }
                    });
                ui.add_space(6.0);
                let default = Placement {
                    dx: DEFAULT_OFFSET_MM,
                    dy: DEFAULT_OFFSET_MM,
                    percent: 100.0,
                };
                ui.horizontal(|ui| {
                    reset = super::dialog_ui::confirm_button(
                        ui,
                        "reset_background",
                        "Reset",
                        "Move the background back to 10 mm, 10 mm, 100 %?",
                        shown != default,
                    );
                });
            });
        if reset {
            shown = Placement {
                dx: DEFAULT_OFFSET_MM,
                dy: DEFAULT_OFFSET_MM,
                percent: 100.0,
            };
            commit = true;
        }
        if !open {
            self.wb.background_panel = None;
            return;
        }
        if commit {
            self.wb.background_panel = Some(None);
            self.apply_placement(stored, shown);
        } else {
            self.wb.background_panel = Some((shown != stored).then_some(shown));
        }
    }

    pub(super) fn apply_placement(&mut self, from: Placement, to: Placement) {
        let Some(page) = self.current_page else {
            return;
        };
        let commands: Vec<Command> = from
            .edits(to)
            .into_iter()
            .map(|(key, value)| Command::SetProperty {
                target: page.into(),
                key: key.into(),
                value: Some(Value::Float(value)),
            })
            .collect();
        if commands.is_empty() {
            return;
        }
        self.run_background(Command::Batch {
            label: "Move DXF background".into(),
            commands,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(dx: f64, dy: f64, percent: f64) -> Placement {
        Placement { dx, dy, percent }
    }

    #[test]
    fn only_changed_values_become_edits() {
        let edits = at(10.0, 10.0, 100.0).edits(at(25.0, 10.0, 50.0));
        assert_eq!(
            edits,
            vec![(bg::KEY_DX, 25.0), (bg::KEY_SCALE, 0.5)],
            "scale is stored as a ratio"
        );
        assert!(at(1.0, 2.0, 100.0).edits(at(1.0, 2.0, 100.0)).is_empty());
    }

    #[test]
    fn a_scale_of_zero_is_ignored() {
        assert!(at(0.0, 0.0, 100.0).edits(at(0.0, 0.0, 0.0)).is_empty());
        assert!(at(0.0, 0.0, 100.0).edits(at(0.0, 0.0, -5.0)).is_empty());
    }

    #[test]
    fn placement_reads_the_page_properties_with_the_import_defaults() {
        let p = Placement::of(&Properties::default());
        assert_eq!(p, at(10.0, 10.0, 100.0));
        let mut props = Properties::default();
        props.insert(bg::KEY_SCALE.into(), Value::Float(0.25));
        props.insert(bg::KEY_DX.into(), Value::Int(40));
        assert_eq!(Placement::of(&props), at(40.0, 10.0, 25.0));
    }
}
