//! The hand-off to the commissioning report (`novo_reports::commissioning`).

use std::collections::HashMap;

use novo_core::Project;
use novo_reports::commissioning as rep;

use crate::check::{CheckState, Target};
use crate::progress::{TargetInfo, targets};
use crate::punch::{PunchState, Severity};
use crate::store::CommissioningStore;

fn check_state(state: CheckState) -> rep::CheckState {
    match state {
        CheckState::Pending => rep::CheckState::Pending,
        CheckState::Passed => rep::CheckState::Passed,
        CheckState::Failed => rep::CheckState::Failed,
        CheckState::NotApplicable => rep::CheckState::NotApplicable,
    }
}

fn severity(severity: Severity) -> rep::PunchSeverity {
    match severity {
        Severity::Minor => rep::PunchSeverity::Minor,
        Severity::Major => rep::PunchSeverity::Major,
        Severity::Blocking => rep::PunchSeverity::Blocking,
    }
}

fn punch_state(state: PunchState) -> rep::PunchState {
    match state {
        PunchState::Open => rep::PunchState::Open,
        PunchState::Fixed => rep::PunchState::Fixed,
        PunchState::Verified => rep::PunchState::Verified,
        PunchState::WontFix => rep::PunchState::WontFix,
    }
}

fn page_names(project: &Project, info: &TargetInfo) -> String {
    info.pages
        .iter()
        .filter_map(|&p| project.page(p).map(|p| p.name.clone()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Everything the report needs: one row for every check that applies to the
/// project's targets (decided or still pending, so progress totals are
/// right), every punch item, and the given sign-offs.
pub fn to_report_input(
    project: &Project,
    store: &CommissioningStore,
    sign_offs: Vec<rep::SignOff>,
) -> rep::CommissioningInput {
    let all = targets(project);
    let by_target: HashMap<Target, &TargetInfo> = all.iter().map(|i| (i.target, i)).collect();

    let mut checks = Vec::new();
    for info in &all {
        let pages = page_names(project, info);
        for &check in info.target.checks() {
            let record = store.record(info.target, check);
            checks.push(rep::CheckRow {
                id: checks.len() as u64 + 1,
                page: pages.clone(),
                cabinet: info.cabinet.clone(),
                tag: info.label.clone(),
                what: check.label().to_owned(),
                state: record.map_or(rep::CheckState::Pending, |r| check_state(r.state)),
                by: record.map(|r| r.by.clone()).unwrap_or_default(),
                at: record.map(|r| r.at.clone()).unwrap_or_default(),
                note: record
                    .map(|r| join_value(&r.value, &r.note))
                    .unwrap_or_default(),
            });
        }
    }

    let punch = store
        .punch_items()
        .map(|item| {
            let info = item.target.and_then(|t| by_target.get(&t).copied());
            rep::PunchRow {
                id: item.id,
                page: info.map(|i| page_names(project, i)).unwrap_or_default(),
                cabinet: info.map(|i| i.cabinet.clone()).unwrap_or_default(),
                tag: info.map(|i| i.label.clone()).unwrap_or_default(),
                text: if item.note.is_empty() {
                    item.title.clone()
                } else {
                    format!("{}: {}", item.title, item.note)
                },
                severity: severity(item.severity),
                state: punch_state(item.state),
                raised_by: item.raised_by.clone(),
                raised_at: item.raised_at.clone(),
                closed_by: item.closed_by.clone().unwrap_or_default(),
                closed_at: item.closed_at.clone().unwrap_or_default(),
            }
        })
        .collect();

    rep::CommissioningInput {
        project_name: project.info().name.clone(),
        subtitle: String::new(),
        checks,
        punch,
        sign_offs,
    }
}

/// The measured value and the note in the report's one note column.
fn join_value(value: &str, note: &str) -> String {
    match (value.is_empty(), note.is_empty()) {
        (true, _) => note.to_owned(),
        (false, true) => value.to_owned(),
        (false, false) => format!("{value}; {note}"),
    }
}
