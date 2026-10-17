//! Honest labels on numbers that are a model, an estimate or a placeholder.
//!
//! One list says what each such number is, the short label it carries
//! wherever it is shown (window, report, PDF, export) and the full sentence.
//! The windows call [`badge`] and [`note`] so they look the same everywhere,
//! and Help > "About the numbers" shows the whole list. The release plan
//! (`release/v0.1-plan.md`, section 8) is the source of the rows.
//!
//! Where the user finds it: Help > About the numbers, and the label next to
//! each number it covers.

use eframe::egui;

/// A kind of number that is not exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Model {
    /// Cable and wire ampacity, rated currents of wires.
    Ampacity,
    /// PFHd, performance level, SIL.
    Pfhd,
    /// Heat, airflow and device temperatures.
    Heat,
    /// Motor start speed and current curve.
    MotorStart,
    /// Wire lengths before routing.
    WireLength,
    /// Lengths of wires that end on the door or another surface.
    DoorWire,
    /// Verdicts of the standards checks.
    Standards,
}

/// One row of the "About the numbers" page.
pub(crate) struct Row {
    pub model: Model,
    /// What the number is.
    pub number: &'static str,
    /// Short label shown next to the number.
    pub label: &'static str,
    /// The full sentence: what is modelled, what is not.
    pub text: &'static str,
    /// Where the label appears.
    pub shown: &'static str,
}

/// Every number the product labels, in the order of the page.
pub(crate) const ROWS: [Row; 7] = [
    Row {
        model: Model::Ampacity,
        number: "Cable and wire ampacity, voltage drop tables",
        label: "planning value",
        text: "Ampacity and derating tables are planning values from published knowledge, not checked against the licensed standard text. Confirm the clause in your copy before relying on a number.",
        shown: "Issues (thin wire, cable rating), Calculations, Standards check and its report",
    },
    Row {
        model: Model::Pfhd,
        number: "PFHd, performance level",
        label: "model, not exact",
        text: "PFHd for categories 2 to 4 is a closed-form estimate; the ISO 13849-1 Table K.1 values are not used. Check functions that matter with SISTEMA or the standard.",
        shown: "Safety calculation, Safety functions, safety report PDF",
    },
    Row {
        model: Model::Heat,
        number: "Heat, airflow, device temperatures",
        label: "model, not validated",
        text: "A physical model with a stated accuracy (docs/thermal-validation.md). The enclosure result is not yet validated against IEC 60890 or a measurement. With a fan the heat balance is only good to about 3 % on one window.",
        shown: "Cabinet Heat view, 3D heat, heat legend and probe",
    },
    Row {
        model: Model::MotorStart,
        number: "Motor start speed and current",
        label: "estimated",
        text: "Estimated from the nameplate with a standard induction-motor model. The first-cycle inrush peak and the spike when a star-delta starter re-closes are not shown.",
        shown: "Simulate motor start window",
    },
    Row {
        model: Model::WireLength,
        number: "Wire lengths before routing",
        label: "~ estimate",
        text: "Until devices are placed and wires routed, a length is the distance on the schematic plus the service loops. Measure on the plate.",
        shown: "Wire list, cut list, bench output (marked ~)",
    },
    Row {
        model: Model::DoorWire,
        number: "Lengths of wires to the door",
        label: "~ door, no gap",
        text: "The length on the door leaves out the distance between plate and door and the hinge loop. Add them when you cut the wire.",
        shown: "Wire list, cut list, bench output (marked ~)",
    },
    Row {
        model: Model::Standards,
        number: "Standards checks (IEC 60204-1, NFPA 79, UL 508A)",
        label: "subset, not a certification",
        text: "Rule checks on the model that cover a subset of the standard. A pass is not a certification; the engineer certifies.",
        shown: "Standards check, its report and PDF footer",
    },
];

impl Model {
    fn row(self) -> &'static Row {
        ROWS.iter()
            .find(|r| r.model == self)
            .expect("every kind has a row")
    }

    /// The short label, for a badge or a column.
    pub(crate) fn label(self) -> &'static str {
        self.row().label
    }

    /// The full sentence.
    pub(crate) fn text(self) -> &'static str {
        self.row().text
    }
}

/// A small marker with the short label, the full sentence on hover. Put it
/// next to the number it covers.
pub(crate) fn badge(ui: &mut egui::Ui, model: Model) -> egui::Response {
    let p = crate::theme::ui_palette();
    ui.label(
        egui::RichText::new(format!("({})", model.label()))
            .small()
            .color(p.warning),
    )
    .on_hover_text(model.text())
}

/// The full sentence as a quiet wrapping line under a block of numbers.
pub(crate) fn note(ui: &mut egui::Ui, model: Model) {
    super::dialog_ui::hint(ui, model.text());
}

/// The page as plain text, for "Copy" and for tests.
pub(crate) fn page_text() -> String {
    let mut out = String::from("About the numbers\n");
    for r in &ROWS {
        out.push_str(&format!(
            "\n{} [{}]\n{}\nShown in: {}\n",
            r.number, r.label, r.text, r.shown
        ));
    }
    out
}

impl super::NovoApp {
    /// Help > About the numbers.
    pub(super) fn numbers_window(&mut self, ctx: &egui::Context) {
        if !self.show_numbers {
            return;
        }
        let p = crate::theme::ui_palette();
        let modal = egui::Modal::new(egui::Id::new("about_numbers")).show(ctx, |ui| {
            ui.set_width(novo_theme::dialog_width::MEDIUM);
            super::dialog_ui::title(
                ui,
                "About the numbers",
                "Some numbers are a model, an estimate or a placeholder. Each carries a label where it is shown; this is the list.",
            );
            egui::ScrollArea::vertical()
                .max_height(ctx.content_rect().height() * 0.6)
                .show(ui, |ui| {
                    for r in &ROWS {
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(r.number);
                            ui.label(egui::RichText::new(format!("({})", r.label)).color(p.warning));
                        });
                        ui.label(r.text);
                        ui.weak(format!("Shown in: {}", r.shown));
                    }
                    ui.add_space(8.0);
                    ui.weak(format!(
                        "{} planning values and models behind the standards rules are listed in the compliance report under \"Basis of the numbers\".",
                        novo_checks::standards::provenance::BASIS.len()
                    ));
                });
            ui.add_space(8.0);
            if ui.button("Copy as text").clicked() {
                ctx.copy_text(page_text());
            }
            super::dialog_ui::done(ui)
        });
        if modal.inner || modal.should_close() {
            self.show_numbers = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_one_row_with_a_label_and_a_place() {
        for m in [
            Model::Ampacity,
            Model::Pfhd,
            Model::Heat,
            Model::MotorStart,
            Model::WireLength,
            Model::DoorWire,
            Model::Standards,
        ] {
            assert_eq!(ROWS.iter().filter(|r| r.model == m).count(), 1, "{m:?}");
            assert!(!m.label().is_empty() && !m.text().is_empty());
        }
        assert!(ROWS.iter().all(|r| !r.shown.is_empty()));
    }

    #[test]
    fn the_command_is_in_help_and_runs() {
        assert!(super::super::workbench::app_runs("help.numbers"));
        assert!(
            novo_workflow::commands::COMMANDS
                .iter()
                .any(|c| c.id == "help.numbers")
        );
    }

    #[test]
    fn the_labels_match_the_engines_wording() {
        assert!(Model::Ampacity.text().contains("planning value"));
        assert!(Model::Pfhd.label().contains("model, not exact"));
        assert!(Model::Pfhd.text().contains("Table K.1"));
        assert!(Model::Heat.label().contains("not validated"));
    }

    /// A window that prints one of these numbers must use the shared label
    /// (or the engine's own disclaimer) in the same file.
    #[test]
    fn a_file_that_shows_a_model_number_carries_its_label() {
        let files: [(&str, &str, &[&str], &[&str]); 5] = [
            (
                "safety.rs",
                include_str!("safety.rs"),
                &["PFHd {"],
                &["DISCLAIMER", "model_labels::"],
            ),
            (
                "safety_functions.rs",
                include_str!("safety_functions.rs"),
                &["PFHd {"],
                &["DISCLAIMER", "model_labels::"],
            ),
            (
                "simulate.rs",
                include_str!("simulate.rs"),
                &["peak_line_current"],
                &["model_labels::"],
            ),
            (
                "calculations.rs",
                include_str!("calculations.rs"),
                &["rating"],
                &["PLANNING_NOTE", "model_labels::"],
            ),
            (
                "standards.rs",
                include_str!("standards.rs"),
                &["Verdicts"],
                &["subset"],
            ),
        ];
        for (name, source, shows, needs) in files {
            if shows.iter().any(|s| source.contains(s)) {
                assert!(
                    needs.iter().any(|n| source.contains(n)),
                    "{name} shows a model number without its label ({needs:?})"
                );
            }
        }
    }
}
