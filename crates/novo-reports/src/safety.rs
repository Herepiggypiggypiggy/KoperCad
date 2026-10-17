//! The safety function report: for each function the required and achieved
//! performance level, then each block with its category, MTTFd, DCavg, PFHd
//! and PL, and a notes section with what is unbound or unrated and the
//! calculation-aid disclaimer.
//!
//! The report reads a plain [`SafetyReportInput`], not the model: the app
//! maps the rated safety functions (`novo_checks::safety`) to it in one
//! place, so the report, its CSV and Excel export and the PDF stay
//! independent of how the functions are stored.
//!
//! **Calculation aid, not a certification.** The disclaimer is part of every
//! report and cannot be switched off.

use crate::table::{Cell, Column, ReportOutput, Section};

/// Id of the safety report in exports.
pub const REPORT_ID: &str = "safety";

/// Printed in the notes section of every report.
pub const DISCLAIMER: &str = "Calculation aid only, not a certification. PFHd for categories 2 to 4 is a closed-form estimate, not ISO 13849-1 Table K.1. Check functions that matter with SISTEMA or the standard and have them assessed by a competent person.";

/// One block (subsystem) of a function.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BlockRow {
    /// `Sensor`, `Logic` or `Actuator`.
    pub role: String,
    pub name: String,
    pub category: String,
    /// Devices the block is bound to, as tag texts.
    pub devices: Vec<String>,
    pub mttfd_years: Option<f64>,
    pub dc: Option<String>,
    pub pfhd: Option<f64>,
    /// `PL d`; `None` when the block is not rated.
    pub pl: Option<String>,
    /// What stops the rating or needs a look.
    pub notes: Vec<String>,
}

/// One safety function and its result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FunctionRow {
    pub name: String,
    /// `PL d`.
    pub required: String,
    /// `None` while the function cannot be rated.
    pub achieved: Option<String>,
    pub pfhd: Option<f64>,
    /// `Some(true)` met, `Some(false)` not met, `None` not rated.
    pub met: Option<bool>,
    pub blocks: Vec<BlockRow>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SafetyReportInput {
    pub project_name: String,
    pub functions: Vec<FunctionRow>,
}

fn result_text(met: Option<bool>) -> &'static str {
    match met {
        Some(true) => "Met",
        Some(false) => "NOT met",
        None => "Not rated",
    }
}

fn pfhd_cell(v: Option<f64>) -> Cell {
    v.map_or(Cell::Empty, |v| Cell::Text(format!("{v:.2e}")))
}

/// The report. Functions keep the order they were entered in.
pub fn safety_report(input: &SafetyReportInput) -> ReportOutput {
    let mut summary = Section::new(
        "summary",
        "Safety functions",
        vec![
            Column::new("function", "Function", 30.0),
            Column::new("required", "Required", 10.0),
            Column::new("achieved", "Achieved", 10.0),
            Column::new("pfhd", "PFHd (1/h)", 14.0).right(),
            Column::new("result", "Result", 12.0),
        ],
    );
    for f in &input.functions {
        summary.push(
            vec![
                f.name.clone().into(),
                f.required.clone().into(),
                f.achieved.clone().into(),
                pfhd_cell(f.pfhd),
                result_text(f.met).into(),
            ],
            Vec::new(),
        );
    }
    let mut sections = vec![summary];
    let mut blocks = Section::new(
        "blocks",
        "Blocks",
        vec![
            Column::new("function", "Function", 20.0),
            Column::new("role", "Role", 9.0),
            Column::new("block", "Block", 18.0),
            Column::new("devices", "Devices", 16.0),
            Column::new("category", "Cat.", 5.0),
            Column::new("mttfd", "MTTFd (y)", 9.0).right(),
            Column::new("dc", "DC", 8.0),
            Column::new("pfhd", "PFHd (1/h)", 12.0).right(),
            Column::new("pl", "PL", 8.0),
        ],
    );
    let mut notes = Section::new(
        "notes",
        "Notes",
        vec![
            Column::new("about", "About", 25.0),
            Column::new("note", "Note", 75.0),
        ],
    );
    for f in &input.functions {
        for b in &f.blocks {
            blocks.push(
                vec![
                    f.name.clone().into(),
                    b.role.clone().into(),
                    b.name.clone().into(),
                    b.devices.join(", ").into(),
                    b.category.clone().into(),
                    b.mttfd_years.map(|v| (v * 10.0).round() / 10.0).into(),
                    b.dc.clone().into(),
                    pfhd_cell(b.pfhd),
                    b.pl.clone().unwrap_or_else(|| "not rated".into()).into(),
                ],
                Vec::new(),
            );
            for n in &b.notes {
                notes.push(vec![f.name.clone().into(), n.clone().into()], Vec::new());
            }
        }
    }
    if !blocks.rows.is_empty() {
        sections.push(blocks);
    }
    notes.push(vec!["Disclaimer".into(), DISCLAIMER.into()], Vec::new());
    sections.push(notes);
    ReportOutput {
        report: REPORT_ID.into(),
        title: "Safety functions".into(),
        revision: 0,
        sections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> SafetyReportInput {
        SafetyReportInput {
            project_name: "Press".into(),
            functions: vec![
                FunctionRow {
                    name: "E-stop".into(),
                    required: "PL d".into(),
                    achieved: Some("PL c".into()),
                    pfhd: Some(2.0e-6),
                    met: Some(false),
                    blocks: vec![BlockRow {
                        role: "Sensor".into(),
                        name: "E-stop button".into(),
                        category: "3".into(),
                        devices: vec!["-S1".into()],
                        mttfd_years: Some(42.34),
                        dc: Some("Medium".into()),
                        pfhd: Some(2.0e-6),
                        pl: Some("PL c".into()),
                        notes: vec![],
                    }],
                },
                FunctionRow {
                    name: "Guard door".into(),
                    required: "PL c".into(),
                    blocks: vec![BlockRow {
                        role: "Logic".into(),
                        name: "Relay".into(),
                        category: "3".into(),
                        notes: vec!["Logic part \"K9\" has no B10d or MTTFd".into()],
                        ..BlockRow::default()
                    }],
                    ..FunctionRow::default()
                },
            ],
        }
    }

    #[test]
    fn the_summary_says_met_not_met_and_not_rated() {
        let r = safety_report(&input());
        let rows = r.section("summary").unwrap().text_rows();
        assert_eq!(rows[0][4], "NOT met");
        assert_eq!(rows[0][3], "2.00e-6");
        assert_eq!(rows[1][4], "Not rated");
        assert_eq!(rows[1][2], "");
    }

    #[test]
    fn blocks_list_devices_and_unrated_blocks_say_so() {
        let r = safety_report(&input());
        let rows = r.section("blocks").unwrap().text_rows();
        assert_eq!(rows[0][3], "-S1");
        assert_eq!(rows[0][5], "42.3");
        assert_eq!(rows[1][8], "not rated");
    }

    #[test]
    fn every_report_carries_the_disclaimer_and_the_notes() {
        for r in [safety_report(&input()), safety_report(&Default::default())] {
            let notes = r.section("notes").unwrap().text_rows();
            assert!(
                notes
                    .iter()
                    .any(|row| row[1].contains("not a certification"))
            );
        }
        let notes = safety_report(&input())
            .section("notes")
            .unwrap()
            .text_rows();
        assert!(notes.iter().any(|row| row[1].contains("no B10d or MTTFd")));
    }
}
