//! What a report's count means. Several reports list the same wires under
//! different definitions; each says in plain words which one it counts, so
//! the numbers can differ and still be explained.

use novo_core::Project;

use crate::report::Report;
use crate::table::ReportOutput;
use crate::wirecut::wire_cut_jobs;
use crate::{CutList, Labels, WireCut, WireList};

/// The number to show for a report and what it counts: the wires, labels or
/// connections of the report, not the number of table rows (the cut list
/// also has summary rows, wire production a stock and a duct table).
pub fn headline_count(report_id: &str, output: &ReportOutput) -> (usize, &'static str) {
    let rows_of = |keys: &[&str]| -> usize {
        output
            .sections
            .iter()
            .filter(|s| keys.contains(&s.key.as_str()))
            .map(|s| s.rows.len())
            .sum()
    };
    match report_id {
        "wires" => (output.row_count(), "connections"),
        "cutlist" => (
            output
                .sections
                .iter()
                .filter(|s| s.key != "summary")
                .map(|s| s.rows.len())
                .sum(),
            "wires",
        ),
        "wirecut" => (rows_of(&["cuts"]), "wires"),
        "labels" => (output.row_count(), "labels"),
        _ => (output.row_count(), "rows"),
    }
}

/// The wire counts of a project and why they differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireSet {
    /// Connections between two device or terminal pins (the wire list).
    pub connections: usize,
    /// Wires to cut: the connections plus the supply and PE leads that end
    /// on a potential feed (cut list, wire production, bench cut list).
    pub to_cut: usize,
    /// Wire labels: one per end of each wire to cut.
    pub wire_labels: usize,
    /// Wires as drawn, including every segment and feed (page tree).
    pub drawn: usize,
}

impl WireSet {
    pub fn of(project: &Project) -> WireSet {
        let list = WireList::build(project, false);
        let to_cut = wire_cut_jobs(project).len();
        WireSet {
            connections: list.row_count(),
            to_cut,
            wire_labels: 2 * to_cut,
            drawn: project.wires().count(),
        }
    }

    /// Supply and PE leads that are cut but are not in the wire list.
    pub fn leads(&self) -> usize {
        self.to_cut.saturating_sub(self.connections)
    }

    /// One sentence naming each difference.
    pub fn explanation(&self) -> String {
        format!(
            "{} connections between pins + {} supply and PE leads = {} wires to cut; \
             {} wire labels (two per wire); {} wires drawn (segments and feeds counted).",
            self.connections,
            self.leads(),
            self.to_cut,
            self.wire_labels,
            self.drawn
        )
    }
}

/// Cross-check: every report that lists the wires to cut counts the same
/// wires, and the labels are two per wire. Returns what disagrees.
pub fn wire_set_mismatch(project: &Project) -> Option<String> {
    let set = WireSet::of(project);
    let checks = [
        (
            "cut list",
            headline_count("cutlist", &CutList::default().generate(project)).0,
            set.to_cut,
        ),
        (
            "wire production",
            headline_count("wirecut", &WireCut.generate(project)).0,
            set.to_cut,
        ),
        (
            "wire labels",
            Labels
                .generate(project)
                .sections
                .iter()
                .filter(|s| s.key == "wires")
                .map(|s| s.rows.len())
                .sum(),
            set.wire_labels,
        ),
    ];
    checks
        .iter()
        .find(|(_, n, expect)| n != expect)
        .map(|(name, n, expect)| format!("{name} counts {n}, expected {expect}"))
}
