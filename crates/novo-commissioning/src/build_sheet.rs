//! The printed twin of Build mode: one table per cabinet with every step and
//! its tick, for a workshop that wants paper. Ticked steps show who and when,
//! open ones a box to tick by pencil. Like the other reports it is a function
//! of the plan and the store, so the same state always prints the same sheet.

use novo_reports::{Cell, Column, ReportOutput, Section};

use crate::build::{BuildPlan, StepStatus, TickState};
use crate::store::CommissioningStore;

/// The box on an open step.
pub const BOX: &str = "[ ]";

/// The build sheet of every cabinet in `plan`, with the ticks of `store`.
pub fn build_sheet(plan: &BuildPlan, store: &CommissioningStore, revision: u64) -> ReportOutput {
    let mut sections = Vec::new();
    for cab in &plan.cabinets {
        let progress = cab.progress(store);
        let mut section = Section::new(
            &format!("{}:build", cab.tag),
            &format!(
                "{}  Build sheet  {} of {} steps done",
                cab.tag, progress.done, progress.total
            ),
            vec![
                Column::new("step", "Step", 8.0).right(),
                Column::new("phase", "Phase", 12.0),
                Column::new("mark", "Mark", 18.0),
                Column::new("do", "Do", 74.0),
                Column::new("after", "After", 14.0),
                Column::new("done", "Done", 34.0),
            ],
        );
        for step in &cab.steps {
            let mut text = step.title.clone();
            for line in &step.details {
                text.push_str("; ");
                // A long position list does not fit a row: the positions are
                // on the 1:1 template and on the Build mode screen.
                if line.len() > 120 {
                    text.push_str("positions: see the drilling template");
                } else {
                    text.push_str(line);
                }
            }
            if let Some(caution) = &step.caution {
                text.push_str(" [");
                text.push_str(caution);
                text.push(']');
            }
            let tick = store.build_tick(step.key);
            let done = match (cab.status(store, step), tick) {
                (StepStatus::Done, Some(t)) => format!("[x] {} {}", t.by, short_time(&t.at)),
                (StepStatus::Problem, Some(t)) if t.state == TickState::Problem => {
                    format!("PROBLEM: {}", t.note)
                }
                (StepStatus::Changed, _) => format!("{BOX} changed since ticked, check again"),
                _ => BOX.to_owned(),
            };
            let after = step.after.iter().map(|n| n.to_string()).collect::<Vec<_>>();
            section.push(
                vec![
                    Cell::Int(step.number as i64),
                    Cell::text(step.phase.name()),
                    Cell::text(step.mark.clone()),
                    Cell::text(text),
                    Cell::text(ranges(&after)),
                    Cell::text(done),
                ],
                Vec::new(),
            );
        }
        sections.push(section);
    }
    ReportOutput {
        report: "build-sheet".into(),
        title: "Build sheet".into(),
        revision,
        sections,
    }
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
