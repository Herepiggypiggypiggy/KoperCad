//! The printed twin of Build mode: one table per cabinet with every step and
//! its tick, for a workshop that wants paper. Ticked steps show who and when,
//! open ones a box to tick by pencil. Like the other reports it is a function
//! of the plan and the store, so the same state always prints the same sheet.

use novo_reports::{Cell, Column, ReportOutput, Section};

use crate::build::{BuildPhase, BuildPlan, StepStatus, TickState};
use crate::store::CommissioningStore;

/// The box on an open step.
pub const BOX: &str = "[ ]";

/// Width of the table the column weights add up to, in mm: a little under
/// the usable width of A4 landscape, so every weight is at least its mm.
const TABLE_MM: f64 = 250.0;
/// Width of one character at the report text size, in mm (as novo-reports
/// reckons it), and the padding of a cell.
const CHAR_MM: f64 = 1.5;
const PAD_MM: f64 = 3.0;
/// Pitch of a wrapped text line, in mm.
const LINE_MM: f64 = 2.9;

/// A header padded with spaces to `chars` characters. The page layout
/// reckons a column's width from the characters it holds, a little short for
/// capitals and bold type, and a page whose cells are all short cut the
/// header ("Do..." for Done) or its marks. The padding makes the estimate
/// the same, and generous, on every page.
fn header(name: &str, chars: usize) -> String {
    let pad = chars.saturating_sub(name.chars().count()) + 2;
    format!("{name}{}", " ".repeat(pad))
}

/// What a tick says in the Done column.
fn done_text(status: StepStatus, tick: Option<&crate::build::BuildTick>) -> String {
    match (status, tick) {
        (StepStatus::Done, Some(t)) => {
            let by = if t.by.is_empty() {
                "(no name set)"
            } else {
                &t.by
            };
            format!("[x] {by} {}", short_time(&t.at))
        }
        (StepStatus::Problem, Some(t)) if t.state == TickState::Problem => "PROBLEM".to_owned(),
        (StepStatus::Changed, _) => format!("{BOX} changed"),
        _ => BOX.to_owned(),
    }
}

/// The text of the Do column: the title and its detail lines.
fn do_text(step: &crate::build::BuildStep) -> String {
    let mut parts = vec![step.title.clone()];
    for line in &step.details {
        if step.phase == BuildPhase::Drill {
            // The positions of a drill step are many numbers: the 1:1
            // template and the Build mode screen carry them. Only this line
            // is replaced, never a part list or a wire's From and To.
            if line.ends_with(':') {
                continue;
            }
            if line.len() > 120 {
                parts.push("positions: see the drilling template".to_owned());
                continue;
            }
            parts.push(format!("positions in mm, across then down: {line}"));
            continue;
        }
        parts.push(line.clone());
    }
    parts.join("; ")
}

/// The build sheet of every cabinet in `plan`, with the ticks of `store`.
/// Pair it with [`row_mm`] so no cell is cut on paper.
pub fn build_sheet(plan: &BuildPlan, store: &CommissioningStore, revision: u64) -> ReportOutput {
    let mut sections = Vec::new();
    for cab in &plan.cabinets {
        let progress = cab.progress(store);
        let rows: Vec<[String; 7]> = cab
            .steps
            .iter()
            .map(|step| {
                let tick = store.build_tick(step.key);
                let status = cab.status(store, step);
                let mut note = Vec::new();
                if let (StepStatus::Problem, Some(t)) = (status, tick)
                    && t.state == TickState::Problem
                {
                    note.push(format!("PROBLEM: {}", t.note));
                }
                if status == StepStatus::Changed {
                    note.push("The drawing changed since this was ticked: check again.".to_owned());
                }
                if let Some(caution) = &step.caution {
                    note.push(caution.clone());
                }
                let after = step.after.iter().map(|n| n.to_string()).collect::<Vec<_>>();
                [
                    step.number.to_string(),
                    step.phase.name().to_owned(),
                    step.mark.clone(),
                    do_text(step),
                    ranges(&after),
                    done_text(status, tick),
                    note.join(" "),
                ]
            })
            .collect();
        // Mark and Done never share their room: each gets what its longest
        // text needs, the Do column the rest.
        let longest = |i: usize| rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0);
        let (mark_chars, done_chars) = (longest(2).max(8), longest(5).max(14));
        let mark = mark_chars as f64 * CHAR_MM + PAD_MM + 2.0;
        let done = done_chars as f64 * CHAR_MM + PAD_MM + 2.0;
        let (step_w, phase, after, note) = (8.0, 14.0, 14.0, 70.0);
        let doing = (TABLE_MM - step_w - phase - mark - after - done - note).max(60.0);
        // One section per phase, flowing on under the one before: a phase of
        // short rows is not made as tall as the row with the longest note.
        let mut start = 0;
        while start < cab.steps.len() {
            let phase_of = cab.steps[start].phase;
            let end = (start..cab.steps.len())
                .find(|&i| cab.steps[i].phase != phase_of)
                .unwrap_or(cab.steps.len());
            let title = if start == 0 {
                format!(
                    "{}  Build sheet  {} of {} steps done",
                    cab.tag, progress.done, progress.total
                )
            } else {
                format!("{}  {}", cab.tag, phase_of.name())
            };
            let mut section = Section::new(
                &format!("{}:build:{}", cab.tag, phase_of.name()),
                &title,
                vec![
                    Column::new("step", &header("Step", 4), step_w).right(),
                    Column::new("phase", &header("Phase", 8), phase),
                    Column::new("mark", &header("Mark", mark_chars), mark),
                    Column::new("do", "Do", doing),
                    Column::new("after", &header("After", 8), after),
                    Column::new("done", &header("Done", done_chars), done),
                    Column::new("note", &header("Note", 8), note),
                ],
            );
            section.flow = start > 0;
            for (step, row) in cab.steps[start..end].iter().zip(&rows[start..end]) {
                let [_, phase, mark, doing, after, done, note] = row.clone();
                section.push(
                    vec![
                        Cell::Int(step.number as i64),
                        Cell::text(phase),
                        Cell::text(mark),
                        Cell::text(doing),
                        Cell::text(after),
                        Cell::text(done),
                        Cell::text(note),
                    ],
                    Vec::new(),
                );
            }
            sections.push(section);
            start = end;
        }
    }
    ReportOutput {
        report: "build-sheet".into(),
        title: "Build sheet".into(),
        revision,
        sections,
    }
}

/// Row height in mm for a section of [`build_sheet`]: tall enough for the
/// longest wrapped cell, so a caution or a problem note is printed whole.
pub fn row_mm(section: &Section) -> f64 {
    let total: f64 = section.columns.iter().map(|c| c.width).sum();
    let mut lines = 1usize;
    for row in section.text_rows() {
        for (cell, column) in row.iter().zip(&section.columns) {
            let room = (column.width / total * TABLE_MM - PAD_MM).max(CHAR_MM);
            // Words break at spaces: allow a tenth for the ragged edge.
            let need = cell.chars().count() as f64 * CHAR_MM * 1.1 / room;
            lines = lines.max(need.ceil() as usize);
        }
    }
    (lines as f64 * LINE_MM + 2.0).max(7.0)
}

/// `2026-10-22T08:12:00Z` as `10-22 08:12`.
fn short_time(at: &str) -> String {
    match (at.get(5..10), at.get(11..16)) {
        (Some(d), Some(t)) => format!("{d} {t}"),
        _ => at.to_owned(),
    }
}

/// Step numbers as a short list: runs of consecutive numbers become `5-12`.
fn ranges(numbers: &[String]) -> String {
    let n: Vec<usize> = numbers.iter().filter_map(|s| s.parse().ok()).collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < n.len() {
        let mut j = i;
        while j + 1 < n.len() && n[j + 1] == n[j] + 1 {
            j += 1;
        }
        out.push(if j > i {
            format!("{}-{}", n[i], n[j])
        } else {
            n[i].to_string()
        });
        i = j + 1;
    }
    out.join(", ")
}
