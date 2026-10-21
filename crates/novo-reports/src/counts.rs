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
                .filter(|s| s.key.starts_with("group-"))
                .map(|s| s.rows.len())
                .sum(),
            "wires",
        ),
        "wirecut" => (rows_of(&["cuts"]), "wires"),
        "labels" => (rows_of(&["wires"]), "wire labels"),
        _ => (output.row_count(), "rows"),
    }
}

/// `1 wire`, `25 wires`: the noun of a headline count made singular for 1.
pub fn count_text(n: usize, noun: &str) -> String {
    let one = match noun {
        "connections" => "connection",
        "wires" => "wire",
        "cores" => "core",
        "wires to cut" => "wire to cut",
        "drawn wires" => "drawn wire",
        "wire labels" => "wire label",
        "rows" => "row",
        "supply and PE leads" => "supply or PE lead",
        "wire pieces" => "wire piece",
        other => other,
    };
    format!("{n} {}", if n == 1 { one } else { noun })
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
            wire_labels: headline_count("labels", &Labels.generate(project)).0,
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
            "{} between pins + {} = {} to cut; {} (one per wire end); \
             {} drawn (a connection can be drawn in several pieces, and supply feeds count too).",
            count_text(self.connections, "connections"),
            count_text(self.leads(), "supply and PE leads"),
            count_text(self.to_cut, "wires"),
            count_text(self.wire_labels, "wire labels"),
            count_text(self.drawn, "wire pieces"),
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
            2 * set.to_cut,
        ),
    ];
    checks
        .iter()
        .find(|(_, n, expect)| n != expect)
        .map(|(name, n, expect)| format!("{name} counts {n}, expected {expect}"))
}

#[cfg(test)]
mod tests {
    use super::count_text;

    #[test]
    fn one_is_singular() {
        assert_eq!(count_text(1, "wires to cut"), "1 wire to cut");
        assert_eq!(count_text(1, "drawn wires"), "1 drawn wire");
        assert_eq!(count_text(2, "drawn wires"), "2 drawn wires");
    }
}
