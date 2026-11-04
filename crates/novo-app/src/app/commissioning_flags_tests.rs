//! Problem triangles of the Commissioning workspace and the Defects card (lane T13).

use novo_commissioning::{
    CheckKind, CheckState, CommissioningStore, PunchItem, PunchState, Severity, Target, TargetInfo,
    targets,
};
use novo_core::WireId;
use novo_flags::{Anchor, Source};

use super::commissioning_flags::{check_anchor, drafts, item_anchor, job};
use super::gate1_tests::Drive;
use crate::actions::Action;

fn item(id: u64, severity: Severity, state: PunchState, target: Option<Target>) -> PunchItem {
    PunchItem {
        id,
        title: format!("Item {id}"),
        note: String::new(),
        severity,
        state,
        target,
        check: None,
        raised_by: "tv".into(),
        raised_at: "2026-11-04T10:00".into(),
        closed_by: None,
        closed_at: None,
        photos: Vec::new(),
        signature: None,
    }
}

fn info(target: Target, label: &str) -> TargetInfo {
    TargetInfo {
        target,
        label: label.into(),
        pages: Vec::new(),
        cabinet: String::new(),
    }
}

#[test]
fn blocking_is_red_major_is_yellow_minor_has_none_and_closed_items_have_none() {
    let wire = Target::Wire(WireId(7));
    let mut store = CommissioningStore::new();
    store.add_punch(item(0, Severity::Blocking, PunchState::Open, Some(wire)));
    store.add_punch(item(0, Severity::Major, PunchState::Open, None));
    store.add_punch(item(0, Severity::Minor, PunchState::Open, None));
    store.add_punch(item(0, Severity::Blocking, PunchState::Verified, None));
    let made = drafts(&store, &[info(wire, "W12")]);
    assert_eq!(made.len(), 2, "{made:?}");
    assert_eq!(made[0].severity, novo_flags::Severity::Red);
    assert_eq!(made[1].severity, novo_flags::Severity::Yellow);
    assert!(made.iter().all(|d| d.source == Source::Commissioning));
    // The blocking item sits on its row and on its wire; the detail names the wire.
    assert!(made[0].anchors.contains(&item_anchor(1)));
    assert!(made[0].anchors.contains(&Anchor::Wire("7".into())));
    assert!(made[0].detail.0.iter().any(|l| l == "W12"));
    assert!(made[0].title.chars().count() <= 28);
}

#[test]
fn a_failed_check_with_an_open_item_is_one_triangle_and_without_one_it_stands_alone() {
    let wire = Target::Wire(WireId(3));
    let mut store = CommissioningStore::new();
    store
        .set_check(novo_commissioning::CheckRecord {
            target: wire,
            check: CheckKind::Continuity,
            state: CheckState::Failed,
            by: "tv".into(),
            at: "t".into(),
            value: "0.9 ohm".into(),
            note: "loose".into(),
        })
        .unwrap();
    // set_check opened a punch item for the failure.
    let made = drafts(&store, &[info(wire, "W3")]);
    assert_eq!(made.len(), 1, "{made:?}");
    assert!(
        made[0]
            .anchors
            .contains(&check_anchor(wire, CheckKind::Continuity)),
        "the check row shows the item's triangle"
    );
    // Once the item is closed the failed check still shows.
    let id = store.punch_items().next().unwrap().id;
    store
        .set_punch_state(id, PunchState::WontFix, "tv", "t")
        .unwrap();
    let made = drafts(&store, &[info(wire, "W3")]);
    assert_eq!(made.len(), 1);
    assert_eq!(made[0].severity, novo_flags::Severity::Red);
    assert!(
        made[0]
            .detail
            .0
            .iter()
            .any(|l| l.contains("Measured 0.9 ohm"))
    );
}

#[test]
fn the_flags_follow_the_data_and_a_failed_job_keeps_its_triangle_until_it_works() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenStarterExample);
    d.frame();
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    let page = d.app.current_page().unwrap();
    let wire = targets(d.doc().project())
        .into_iter()
        .find(|t| t.pages.contains(&page) && matches!(t.target, Target::Wire(_)))
        .unwrap()
        .target;
    d.app.wb.commissioning.as_mut().unwrap().decide(
        wire,
        CheckKind::Continuity,
        CheckState::Failed,
    );
    d.frame();
    d.frame();
    let flags = &d.app.wb.flags.store;
    let mine: Vec<_> = flags
        .list(false)
        .into_iter()
        .filter(|f| f.source == Source::Commissioning)
        .collect();
    assert_eq!(mine.len(), 1, "{mine:?}");
    assert_eq!(mine[0].severity, novo_flags::Severity::Red);
    // The schematic draws it as a mark that opens the popover.
    let marks = d
        .app
        .canvas
        .schematic_ref()
        .unwrap()
        .canvas
        .marks
        .list
        .clone();
    assert!(marks.iter().any(|m| m.key == mine[0].id.0), "{marks:?}");

    // A job fails: a red job flag with Retry, then it works and the flag goes.
    d.app
        .job_error(job::PUNCH_PDF, "Punch list not exported", "disk full", true);
    let jobs = |d: &Drive| {
        d.app
            .wb
            .flags
            .store
            .list(false)
            .into_iter()
            .filter(|f| f.source == Source::Job)
            .count()
    };
    assert_eq!(jobs(&d), 1);
    assert_eq!(
        d.app
            .wb
            .commissioning
            .as_ref()
            .unwrap()
            .panel_flags
            .flags
            .len(),
        2,
        "the panel draws the check and the job"
    );
    d.app.job_ok(job::PUNCH_PDF);
    assert_eq!(jobs(&d), 0);

    // Leaving the mode takes its flags away.
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    assert!(
        d.app
            .wb
            .flags
            .store
            .list(false)
            .iter()
            .all(|f| f.source != Source::Commissioning)
    );
}

/// Toast sites and coloured sentences left in this area. The number only goes down; the
/// design (docs/design/problem-triangles.md section 12) takes it to zero in lane T15.
#[test]
fn problem_census_commissioning() {
    const CEILING: usize = 0;
    let files = [
        include_str!("commissioning.rs"),
        include_str!("commissioning_photos.rs"),
        include_str!("commissioning_redlines.rs"),
        include_str!("commissioning_signature.rs"),
        include_str!("commissioning_viewer.rs"),
        include_str!("commissioning_test.rs"),
        include_str!("commissioning_cabinet.rs"),
        include_str!("defects_card.rs"),
    ];
    let mut found = Vec::new();
    for (i, text) in files.iter().enumerate() {
        for (n, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") {
                continue;
            }
            if line.contains("Level::Error")
                || line.contains("Level::Warning")
                || line.contains("colored_label(palette.error")
                || line.contains("colored_label(palette.warning")
                || line.contains(".color(palette.error)")
                || line.contains(".color(palette.warning)")
            {
                found.push(format!("file {i} line {}", n + 1));
            }
        }
    }
    assert!(found.len() == CEILING, "{found:?}");
}
