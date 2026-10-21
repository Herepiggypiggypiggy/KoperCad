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

use novo_core::AnyId;

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
    /// DCavg as entered, in percent.
    pub dc_percent: f64,
    /// CCF score as entered.
    pub ccf: u32,
    /// Where the bound devices are drawn (`3.4`, reading order), all devices.
    pub drawn_at: Vec<String>,
    /// The bound devices, for jumps from the sheet.
    pub sources: Vec<AnyId>,
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
    /// Risk graph choice as entered: `S2 F1 P1`.
    pub risk: String,
    /// Duty as entered, one sentence (`220 days, 16 h, cycle 600 s: 21 120 operations per year`).
    pub duty: String,
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

/// Printed on every sheet about the clause numbers.
pub const STANDARDS_NOTE: &str = "Clause numbers of IEC 60204-1 (emergency stop, stop categories 0 and 1, safeguarding) are not quoted here: they are unverified because no licensed copy was checked. Take them from the standard.";

/// The first notes of every sheet: what is an estimate and what was assumed.
const ASSUMPTIONS: [&str; 3] = [
    "ESTIMATE, not a certification: the PFHd of categories 2 to 4 is a closed-form model, not ISO 13849-1 Table K.1.",
    "MTTFd, B10d, DCavg and CCF are the values entered or taken from the bound part or device; nothing was looked up from a datasheet.",
    "The required level comes from the risk graph choices above (ISO 13849-1 annex A); the machine's risk assessment is not part of this sheet.",
];

fn sheet_row(item: &str, detail: String, drawn: String, note: String) -> Vec<Cell> {
    vec![item.into(), detail.into(), drawn.into(), note.into()]
}

/// One sheet (section, so one page) per safety function, for the technical
/// file: the risk graph and duty, the sensor, logic and actuator blocks with
/// where their devices are drawn, the result, and what was assumed. Each
/// block row carries its devices as sources.
pub fn safety_sheets(input: &SafetyReportInput) -> ReportOutput {
    let columns = || {
        vec![
            Column::new("item", "Item", 16.0),
            Column::new("detail", "Detail", 44.0),
            Column::new("drawn", "Drawn at", 14.0),
            Column::new("note", "Note", 26.0),
        ]
    };
    let mut sections = Vec::new();
    for (i, f) in input.functions.iter().enumerate() {
        let title = format!("Safety function {}: {}", i + 1, f.name);
        let mut s = Section::new(&format!("function-{}", i + 1), &title, columns());
        s.push(
            sheet_row(
                "Project",
                input.project_name.clone(),
                String::new(),
                String::new(),
            ),
            Vec::new(),
        );
        s.push(
            sheet_row(
                "Required",
                f.required.clone(),
                String::new(),
                f.risk.clone(),
            ),
            Vec::new(),
        );
        if !f.duty.is_empty() {
            s.push(
                sheet_row("Duty", f.duty.clone(), String::new(), String::new()),
                Vec::new(),
            );
        }
        if f.blocks.is_empty() {
            s.push(
                sheet_row(
                    "Chain",
                    "No sensor, logic or actuator entered".into(),
                    String::new(),
                    "Not rated".into(),
                ),
                Vec::new(),
            );
        }
        for b in &f.blocks {
            let mut detail = format!("{}, category {}", b.name, b.category);
            if let Some(m) = b.mttfd_years {
                detail.push_str(&format!(", MTTFd {:.1} y", m));
            }
            if let Some(dc) = &b.dc {
                detail.push_str(&format!(", DC {dc} ({:.0} %)", b.dc_percent));
            }
            detail.push_str(&format!(", CCF {}", b.ccf));
            let result = match (&b.pl, b.pfhd) {
                (Some(pl), Some(p)) => format!("{pl}, {p:.2e} /h"),
                _ => "not rated".into(),
            };
            let devices = if b.devices.is_empty() {
                "no device bound".to_owned()
            } else {
                b.devices.join(", ")
            };
            s.push(
                sheet_row(
                    &b.role,
                    format!("{detail}. Devices: {devices}"),
                    b.drawn_at.join(", "),
                    result,
                ),
                b.sources.clone(),
            );
            for n in &b.notes {
                s.push(
                    sheet_row("", n.clone(), String::new(), "check".into()),
                    Vec::new(),
                );
            }
        }
        let result = match (&f.achieved, f.pfhd) {
            (Some(pl), Some(p)) => format!("{pl}, PFHd {p:.2e} /h (estimate)"),
            _ => "Not rated yet".into(),
        };
        s.push(
            sheet_row("Result", result, String::new(), result_text(f.met).into()),
            Vec::new(),
        );
        for a in ASSUMPTIONS {
            s.push(
                sheet_row("Assumed", a.into(), String::new(), String::new()),
                Vec::new(),
            );
        }
        s.push(
            sheet_row(
                "Standards",
                STANDARDS_NOTE.into(),
                String::new(),
                String::new(),
            ),
            Vec::new(),
        );
        s.push(
            sheet_row(
                "Disclaimer",
                DISCLAIMER.into(),
                String::new(),
                String::new(),
            ),
            Vec::new(),
        );
        // Blank lines to fill in by hand on the printed sheet.
        for who in ["Prepared by", "Checked by", "Revision and date"] {
            s.push(
                sheet_row(
                    who,
                    "______________________".into(),
                    String::new(),
                    String::new(),
                ),
                Vec::new(),
            );
        }
        sections.push(s);
    }
    if sections.is_empty() {
        let mut s = Section::new("function-0", "Safety function sheets", columns());
        s.push(
            sheet_row(
                "Functions",
                "No safety function in the project".into(),
                String::new(),
                String::new(),
            ),
            Vec::new(),
        );
        s.push(
            sheet_row(
                "Disclaimer",
                DISCLAIMER.into(),
                String::new(),
                String::new(),
            ),
            Vec::new(),
        );
        sections.push(s);
    }
    ReportOutput {
        report: "safety-sheets".into(),
        title: "Safety function sheets".into(),
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
                        drawn_at: vec!["3.4".into()],
                        ..BlockRow::default()
                    }],
                    risk: "S2 F2 P1".into(),
                    duty: String::new(),
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

    #[test]
    fn each_function_gets_its_own_sheet_with_the_estimate_labels() {
        let r = safety_sheets(&input());
        assert_eq!(r.sections.len(), 2);
        assert_eq!(r.sections[0].title, "Safety function 1: E-stop");
        let rows = r.sections[0].text_rows();
        let chain = rows.iter().find(|r| r[0] == "Sensor").unwrap();
        assert!(chain[1].contains("E-stop button") && chain[1].contains("-S1"));
        assert_eq!(chain[2], "3.4");
        let flat: String = rows.iter().flatten().cloned().collect();
        assert!(flat.contains("ESTIMATE, not a certification"));
        assert!(flat.contains("unverified"));
        assert!(flat.contains("NOT met"));
        let second = r.sections[1].text_rows();
        assert!(second.iter().any(|r| r[3] == "Not rated"));
        assert!(second.iter().any(|r| r[1].contains("no B10d")));
    }

    #[test]
    fn every_sheet_ends_with_blank_sign_off_lines() {
        for section in &safety_sheets(&input()).sections {
            let rows = section.text_rows();
            for who in ["Prepared by", "Checked by", "Revision and date"] {
                assert!(rows.iter().any(|r| r[0] == who), "{who}");
            }
        }
    }

    #[test]
    fn an_empty_project_still_gets_one_sheet_with_the_disclaimer() {
        let r = safety_sheets(&SafetyReportInput::default());
        assert_eq!(r.sections.len(), 1);
        assert!(r.sections[0].text_rows().iter().any(|r| r[1] == DISCLAIMER));
    }
}
