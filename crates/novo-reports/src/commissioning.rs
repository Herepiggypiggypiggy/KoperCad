//! The commissioning report: progress per cabinet and page, every check,
//! the punch list and the sign-off block, as ordinary report tables.
//!
//! The report reads a plain [`CommissioningInput`], not the model. The
//! commissioning data (checks, punch items, sign-offs) is owned by the
//! commissioning thread; it maps its records to this input in one place, so
//! the report, its CSV and Excel export and the PDF stay independent of how
//! the data is stored. Everything is sorted explicitly (natural order), so
//! the same input always gives the same output.
//!
//! ```
//! use novo_reports::commissioning::*;
//!
//! let input = CommissioningInput {
//!     project_name: "Press line".into(),
//!     checks: vec![CheckRow {
//!         page: "12".into(),
//!         cabinet: "+CAB1".into(),
//!         tag: "-K1".into(),
//!         what: "Coil pulls in".into(),
//!         state: CheckState::Passed,
//!         ..CheckRow::default()
//!     }],
//!     ..CommissioningInput::default()
//! };
//! let report = commissioning_report(&input);
//! assert_eq!(report.sections[0].key, "summary");
//! assert!(readiness(&input).ready());
//! ```

use std::collections::BTreeMap;

use novo_core::natural_cmp;

use crate::table::{Cell, Column, ReportOutput, Section};

/// Id of the commissioning report in exports.
pub const REPORT_ID: &str = "commissioning";

/// Where one check stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckState {
    #[default]
    Pending,
    Passed,
    Failed,
    NotApplicable,
}

impl CheckState {
    pub fn label(self) -> &'static str {
        match self {
            CheckState::Pending => "Pending",
            CheckState::Passed => "Passed",
            CheckState::Failed => "Failed",
            CheckState::NotApplicable => "N/A",
        }
    }
}

/// How much a punch item matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum PunchSeverity {
    #[default]
    Minor,
    Major,
    /// The machine cannot be handed over until it is closed.
    Blocking,
}

impl PunchSeverity {
    pub fn label(self) -> &'static str {
        match self {
            PunchSeverity::Minor => "Minor",
            PunchSeverity::Major => "Major",
            PunchSeverity::Blocking => "Blocking",
        }
    }
}

/// Where a punch item stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PunchState {
    #[default]
    Open,
    /// Fixed, not yet verified by someone else.
    Fixed,
    Verified,
    WontFix,
}

impl PunchState {
    pub fn label(self) -> &'static str {
        match self {
            PunchState::Open => "Open",
            PunchState::Fixed => "Fixed",
            PunchState::Verified => "Verified",
            PunchState::WontFix => "Won't fix",
        }
    }

    /// Whether the item still needs work or verification.
    pub fn is_open(self) -> bool {
        matches!(self, PunchState::Open | PunchState::Fixed)
    }
}

/// One thing to check on the machine (a coil, a terminal, a wire).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckRow {
    pub id: u64,
    /// Sheet name of the page the check belongs to (`12`).
    pub page: String,
    /// Location tag of the cabinet or field enclosure (`+CAB1`).
    pub cabinet: String,
    /// Device, terminal or wire tag (`-K1`).
    pub tag: String,
    pub what: String,
    pub state: CheckState,
    pub by: String,
    /// When it was ticked, as printed (ISO `2026-10-15 14:03`).
    pub at: String,
    pub note: String,
}

/// A defect or open job found while commissioning.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PunchRow {
    pub id: u64,
    pub page: String,
    pub cabinet: String,
    pub tag: String,
    pub text: String,
    pub severity: PunchSeverity,
    pub state: PunchState,
    pub raised_by: String,
    pub raised_at: String,
    pub closed_by: String,
    pub closed_at: String,
}

/// One line of the sign-off block. An empty `name` or `date` prints as a
/// blank line to sign by hand.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignOff {
    /// Who signs: `Commissioning engineer`, `Customer`.
    pub role: String,
    pub name: String,
    pub date: String,
    pub note: String,
}

/// Everything the report prints.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommissioningInput {
    pub project_name: String,
    /// Printed under the title (`Run 2, 2026-10-15`); may be empty.
    pub subtitle: String,
    pub checks: Vec<CheckRow>,
    pub punch: Vec<PunchRow>,
    pub sign_offs: Vec<SignOff>,
}

/// The usual two lines of the sign-off block: the commissioning engineer
/// (named when `engineer` is not empty) and the customer, both to sign and
/// date by hand.
pub fn default_sign_offs(engineer: &str) -> Vec<SignOff> {
    vec![
        SignOff {
            role: "Commissioning engineer".into(),
            name: engineer.into(),
            ..SignOff::default()
        },
        SignOff {
            role: "Customer".into(),
            ..SignOff::default()
        },
    ]
}

/// Counts of checks by state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub not_applicable: usize,
    pub pending: usize,
}

impl Progress {
    fn add(&mut self, state: CheckState) {
        self.total += 1;
        match state {
            CheckState::Pending => self.pending += 1,
            CheckState::Passed => self.passed += 1,
            CheckState::Failed => self.failed += 1,
            CheckState::NotApplicable => self.not_applicable += 1,
        }
    }

    /// Checks that are settled: passed, or not applicable.
    pub fn done(&self) -> usize {
        self.passed + self.not_applicable
    }

    /// Whole percent done; `None` when there is nothing to check.
    pub fn percent(&self) -> Option<i64> {
        (self.total > 0).then(|| (self.done() * 100 / self.total) as i64)
    }
}

/// Progress of every check, grouped by `key(check)`, in natural order of
/// the group name. Checks with an empty group name fall under `—`.
fn grouped(input: &CommissioningInput, key: fn(&CheckRow) -> &str) -> Vec<(String, Progress)> {
    let mut groups: BTreeMap<String, Progress> = BTreeMap::new();
    for c in &input.checks {
        let name = key(c);
        let name = if name.is_empty() { "—" } else { name };
        groups.entry(name.to_owned()).or_default().add(c.state);
    }
    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort_by(|a, b| natural_cmp(&a.0, &b.0));
    groups
}

/// Progress per cabinet (location tag).
pub fn progress_by_cabinet(input: &CommissioningInput) -> Vec<(String, Progress)> {
    grouped(input, |c| &c.cabinet)
}

/// Progress per page (sheet name).
pub fn progress_by_page(input: &CommissioningInput) -> Vec<(String, Progress)> {
    grouped(input, |c| &c.page)
}

/// Progress over all checks.
pub fn progress(input: &CommissioningInput) -> Progress {
    let mut p = Progress::default();
    for c in &input.checks {
        p.add(c.state);
    }
    p
}

/// Whether the machine can be signed off, and what stands in the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Readiness {
    /// Checks recorded in all.
    pub checks_total: usize,
    pub checks_pending: usize,
    pub checks_failed: usize,
    /// Punch items still open or fixed but not verified.
    pub punch_open: usize,
    /// Of those, the blocking ones.
    pub punch_blocking: usize,
}

impl Readiness {
    /// At least one check, every check settled and no blocking punch item
    /// open (a project with nothing recorded is not ready). Open minor and
    /// major items are listed on the report but do not stop the sign-off.
    pub fn ready(&self) -> bool {
        self.checks_total > 0
            && self.checks_pending == 0
            && self.checks_failed == 0
            && self.punch_blocking == 0
    }
}

pub fn readiness(input: &CommissioningInput) -> Readiness {
    let p = progress(input);
    let open = || input.punch.iter().filter(|x| x.state.is_open());
    Readiness {
        checks_total: p.total,
        checks_pending: p.pending,
        checks_failed: p.failed,
        punch_open: open().count(),
        punch_blocking: open()
            .filter(|x| x.severity == PunchSeverity::Blocking)
            .count(),
    }
}

fn progress_columns(first: (&str, &str)) -> Vec<Column> {
    vec![
        Column::new(first.0, first.1, 18.0),
        Column::new("total", "Checks", 8.0).right(),
        Column::new("passed", "Passed", 8.0).right(),
        Column::new("failed", "Failed", 8.0).right(),
        Column::new("na", "N/A", 8.0).right(),
        Column::new("pending", "Pending", 8.0).right(),
        Column::new("percent", "Done %", 8.0).right(),
    ]
}

fn progress_cells(name: &str, p: &Progress) -> Vec<Cell> {
    vec![
        Cell::text(name),
        Cell::Int(p.total as i64),
        Cell::Int(p.passed as i64),
        Cell::Int(p.failed as i64),
        Cell::Int(p.not_applicable as i64),
        Cell::Int(p.pending as i64),
        p.percent().map_or(Cell::Empty, Cell::Int),
    ]
}

fn progress_section(
    key: &str,
    title: &str,
    first: (&str, &str),
    groups: Vec<(String, Progress)>,
) -> Section {
    let mut section = Section::new(key, title, progress_columns(first));
    let mut all = Progress::default();
    for (name, p) in &groups {
        section.push(progress_cells(name, p), vec![]);
        all.total += p.total;
        all.passed += p.passed;
        all.failed += p.failed;
        all.not_applicable += p.not_applicable;
        all.pending += p.pending;
    }
    if groups.len() > 1 {
        section.push(progress_cells("Total", &all), vec![]);
    }
    section
}

fn summary_section(input: &CommissioningInput) -> Section {
    let mut s = Section::new(
        "summary",
        &input.project_name,
        vec![
            Column::new("item", "", 22.0),
            Column::new("value", "", 30.0),
        ],
    );
    let p = progress(input);
    let r = readiness(input);
    let mut row = |item: &str, value: Cell| s.push(vec![Cell::text(item), value], vec![]);
    if !input.subtitle.is_empty() {
        row("Run", Cell::text(&input.subtitle));
    }
    row("Checks", Cell::Int(p.total as i64));
    row("Passed", Cell::Int(p.passed as i64));
    row("Failed", Cell::Int(p.failed as i64));
    row("Not applicable", Cell::Int(p.not_applicable as i64));
    row("Pending", Cell::Int(p.pending as i64));
    row("Done %", p.percent().map_or(Cell::Empty, Cell::Int));
    row("Open punch items", Cell::Int(r.punch_open as i64));
    row("Blocking punch items", Cell::Int(r.punch_blocking as i64));
    row(
        "Ready for sign-off",
        Cell::text(if r.ready() { "Yes" } else { "No" }),
    );
    s
}

fn checks_section(input: &CommissioningInput) -> Section {
    let mut s = Section::new(
        "checks",
        "Checks",
        vec![
            Column::new("page", "Page", 7.0),
            Column::new("cabinet", "Cabinet", 10.0),
            Column::new("tag", "Tag", 10.0),
            Column::new("what", "Check", 26.0),
            Column::new("state", "Result", 9.0),
            Column::new("by", "By", 12.0),
            Column::new("at", "When", 14.0),
            Column::new("note", "Note", 24.0),
        ],
    );
    let mut rows: Vec<&CheckRow> = input.checks.iter().collect();
    rows.sort_by(|a, b| {
        natural_cmp(&a.page, &b.page)
            .then_with(|| natural_cmp(&a.cabinet, &b.cabinet))
            .then_with(|| natural_cmp(&a.tag, &b.tag))
            .then_with(|| a.id.cmp(&b.id))
    });
    for c in rows {
        s.push(
            vec![
                Cell::text(&c.page),
                Cell::text(&c.cabinet),
                Cell::text(&c.tag),
                Cell::text(&c.what),
                Cell::text(c.state.label()),
                Cell::text(&c.by),
                Cell::text(&c.at),
                Cell::text(&c.note),
            ],
            vec![],
        );
    }
    s
}

fn punch_section(input: &CommissioningInput) -> Section {
    let mut s = Section::new(
        "punch",
        "Punch list",
        vec![
            Column::new("id", "No.", 5.0).right(),
            Column::new("severity", "Severity", 9.0),
            Column::new("state", "Status", 9.0),
            Column::new("page", "Page", 6.0),
            Column::new("cabinet", "Cabinet", 9.0),
            Column::new("tag", "Tag", 9.0),
            Column::new("text", "Description", 28.0),
            Column::new("raised", "Raised", 17.0),
            Column::new("closed", "Closed", 17.0),
        ],
    );
    let mut rows: Vec<&PunchRow> = input.punch.iter().collect();
    // Open items first, worst first, then by number (callers keep numbers
    // unique; equal numbers keep their input order).
    rows.sort_by(|a, b| {
        (!a.state.is_open())
            .cmp(&!b.state.is_open())
            .then_with(|| b.severity.cmp(&a.severity))
            .then_with(|| a.id.cmp(&b.id))
    });
    let who = |by: &str, at: &str| match (by.is_empty(), at.is_empty()) {
        (true, true) => String::new(),
        (false, true) => by.to_owned(),
        (true, false) => at.to_owned(),
        (false, false) => format!("{by}, {at}"),
    };
    for p in rows {
        s.push(
            vec![
                Cell::Int(p.id as i64),
                Cell::text(p.severity.label()),
                Cell::text(p.state.label()),
                Cell::text(&p.page),
                Cell::text(&p.cabinet),
                Cell::text(&p.tag),
                Cell::text(&p.text),
                Cell::text(who(&p.raised_by, &p.raised_at)),
                Cell::text(who(&p.closed_by, &p.closed_at)),
            ],
            vec![],
        );
    }
    s
}

fn signoff_section(input: &CommissioningInput) -> Section {
    let mut s = Section::new(
        "signoff",
        "Sign-off",
        vec![
            Column::new("role", "Role", 20.0),
            Column::new("name", "Name", 22.0),
            Column::new("date", "Date", 12.0),
            Column::new("signature", "Signature", 26.0),
        ],
    );
    for o in &input.sign_offs {
        s.push(
            vec![
                Cell::text(&o.role),
                Cell::text(&o.name),
                Cell::text(&o.date),
                Cell::Empty,
            ],
            vec![],
        );
    }
    s
}

/// The commissioning report: summary, progress per cabinet, progress per
/// page, every check, the punch list and the sign-off block, one section
/// each (each starts a new page when printed). Progress sections are left
/// out when there are no checks, the punch list when there are no punch
/// items.
pub fn commissioning_report(input: &CommissioningInput) -> ReportOutput {
    let mut sections = vec![summary_section(input)];
    if !input.checks.is_empty() {
        sections.push(progress_section(
            "cabinets",
            "Progress per cabinet",
            ("cabinet", "Cabinet"),
            progress_by_cabinet(input),
        ));
        sections.push(progress_section(
            "pages",
            "Progress per page",
            ("page", "Page"),
            progress_by_page(input),
        ));
        sections.push(checks_section(input));
    }
    if !input.punch.is_empty() {
        sections.push(punch_section(input));
    }
    sections.push(signoff_section(input));
    ReportOutput {
        report: REPORT_ID.into(),
        title: "Commissioning report".into(),
        revision: 0,
        sections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CsvOptions, to_csv};

    fn check(id: u64, page: &str, cab: &str, state: CheckState) -> CheckRow {
        CheckRow {
            id,
            page: page.into(),
            cabinet: cab.into(),
            tag: format!("-K{id}"),
            what: "Coil pulls in".into(),
            state,
            ..CheckRow::default()
        }
    }

    fn input() -> CommissioningInput {
        CommissioningInput {
            project_name: "Press line".into(),
            subtitle: "Run 2".into(),
            checks: vec![
                check(1, "10", "+CAB2", CheckState::Passed),
                check(2, "2", "+CAB1", CheckState::Passed),
                check(3, "2", "+CAB1", CheckState::Pending),
                check(4, "2", "+CAB10", CheckState::NotApplicable),
                check(5, "10", "+CAB2", CheckState::Failed),
            ],
            punch: vec![
                PunchRow {
                    id: 1,
                    text: "Label missing".into(),
                    state: PunchState::Verified,
                    severity: PunchSeverity::Blocking,
                    ..PunchRow::default()
                },
                PunchRow {
                    id: 2,
                    text: "Loose lug".into(),
                    severity: PunchSeverity::Major,
                    ..PunchRow::default()
                },
                PunchRow {
                    id: 3,
                    text: "Wrong colour".into(),
                    severity: PunchSeverity::Blocking,
                    raised_by: "TV".into(),
                    raised_at: "2026-10-15".into(),
                    ..PunchRow::default()
                },
            ],
            sign_offs: vec![SignOff {
                role: "Customer".into(),
                ..SignOff::default()
            }],
        }
    }

    #[test]
    fn progress_counts_and_natural_order() {
        let by_cab = progress_by_cabinet(&input());
        let names: Vec<_> = by_cab.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["+CAB1", "+CAB2", "+CAB10"]);
        let cab1 = by_cab[0].1;
        assert_eq!((cab1.total, cab1.passed, cab1.pending), (2, 1, 1));
        assert_eq!(cab1.percent(), Some(50));
        let by_page = progress_by_page(&input());
        assert_eq!(by_page[0].0, "2");
        assert_eq!(by_page[1].0, "10");
        let all = progress(&input());
        assert_eq!(all.done(), 3);
        assert_eq!(all.percent(), Some(60));
        assert_eq!(Progress::default().percent(), None);
    }

    #[test]
    fn readiness_ignores_verified_and_minor() {
        let r = readiness(&input());
        assert_eq!((r.checks_pending, r.checks_failed, r.punch_open), (1, 1, 2));
        assert_eq!(r.punch_blocking, 1);
        assert!(!r.ready());
        let clean = CommissioningInput {
            checks: vec![check(1, "1", "+A", CheckState::Passed)],
            punch: vec![PunchRow {
                severity: PunchSeverity::Major,
                ..PunchRow::default()
            }],
            ..CommissioningInput::default()
        };
        assert!(readiness(&clean).ready());
    }

    #[test]
    fn report_sections_and_totals() {
        let report = commissioning_report(&input());
        let keys: Vec<_> = report.sections.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(
            keys,
            ["summary", "cabinets", "pages", "checks", "punch", "signoff"]
        );
        let cabinets = report.section("cabinets").unwrap().text_rows();
        assert_eq!(
            cabinets.last().unwrap(),
            &["Total", "5", "2", "1", "1", "1", "60"]
        );
        let summary = report.section("summary").unwrap().text_rows();
        assert!(summary.contains(&vec!["Ready for sign-off".into(), "No".into()]));
        // Checks sorted by page, then cabinet, then tag.
        let checks = report.section("checks").unwrap().text_rows();
        assert_eq!(checks[0][0], "2");
        assert_eq!(checks.last().unwrap()[0], "10");
    }

    #[test]
    fn punch_list_puts_open_blocking_first() {
        let report = commissioning_report(&input());
        let punch = report.section("punch").unwrap().text_rows();
        let ids: Vec<_> = punch.iter().map(|r| r[0].as_str()).collect();
        assert_eq!(ids, ["3", "2", "1"]);
        assert_eq!(punch[0][7], "TV, 2026-10-15");
    }

    #[test]
    fn empty_input_still_gives_a_signable_report() {
        let report = commissioning_report(&CommissioningInput::default());
        let keys: Vec<_> = report.sections.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, ["summary", "signoff"]);
        assert!(!readiness(&CommissioningInput::default()).ready());
    }

    #[test]
    fn default_sign_offs_name_the_engineer_only() {
        let lines = default_sign_offs("TV");
        assert_eq!(lines[0].name, "TV");
        assert_eq!(lines[1].role, "Customer");
        assert!(lines[1].name.is_empty() && lines[1].date.is_empty());
    }

    #[test]
    fn csv_export_is_deterministic() {
        let report = commissioning_report(&input());
        let a = to_csv(&report, &CsvOptions::default());
        let b = to_csv(&commissioning_report(&input()), &CsvOptions::default());
        assert_eq!(a, b);
        assert!(a.contains("Progress per cabinet"));
    }
}
