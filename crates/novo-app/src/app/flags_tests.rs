//! Problem triangles on the schematic (lane T0b).

use super::flags::{draft, drafts, title};
use crate::actions::Action;
use crate::app::gate1_tests::Drive;
use novo_canvas::MarkKind;
use novo_checks::{Problem, ProblemKind, Severity};
use novo_core::{AnyId, SymbolId};
use novo_flags::{Flag, Severity as Sev};

fn problem(kind: ProblemKind, severity: Severity, at: Vec<AnyId>, message: &str) -> Problem {
    Problem {
        severity,
        ..Problem::new(kind, at, message.to_owned())
    }
}

#[test]
fn every_kind_has_a_title_that_fits_the_flag() {
    for kind in ProblemKind::ALL {
        let t = title(kind);
        assert!(!t.is_empty(), "{kind:?}");
        assert!(t.chars().count() <= 28, "{kind:?}: {t}");
    }
}

#[test]
fn errors_are_red_warnings_yellow_and_info_is_no_flag() {
    let at = vec![AnyId::Symbol(SymbolId(1))];
    let e = draft(&problem(
        ProblemKind::DuplicateTag,
        Severity::Error,
        at.clone(),
        "x",
    ))
    .unwrap();
    let w = draft(&problem(
        ProblemKind::UnconnectedPin,
        Severity::Warning,
        at.clone(),
        "x",
    ))
    .unwrap();
    assert_eq!(e.severity, Sev::Red);
    assert_eq!(w.severity, Sev::Yellow);
    assert!(draft(&problem(ProblemKind::SameShortTag, Severity::Info, at, "x")).is_none());
}

#[test]
fn missing_data_kinds_are_dashed() {
    let at = vec![AnyId::Symbol(SymbolId(1))];
    let d = draft(&problem(
        ProblemKind::DepthUnknown,
        Severity::Warning,
        at,
        "x",
    ))
    .unwrap();
    assert!(d.unchecked);
}

#[test]
fn a_flag_keeps_its_id_when_only_the_numbers_in_the_text_change() {
    let at = vec![AnyId::Symbol(SymbolId(7))];
    let a = problem(
        ProblemKind::VoltageDrop,
        Severity::Warning,
        at.clone(),
        "3.1 % drop",
    );
    let b = problem(
        ProblemKind::VoltageDrop,
        Severity::Warning,
        at,
        "3.4 % drop",
    );
    let (a, b) = (drafts(&[a]), drafts(&[b]));
    assert_eq!(
        Flag::from_draft(a[0].1.clone()).id,
        Flag::from_draft(b[0].1.clone()).id
    );
}

#[test]
fn a_problem_without_a_drawn_object_is_anchored_on_the_project() {
    let d = draft(&problem(
        ProblemKind::DuplicatePage,
        Severity::Warning,
        vec![],
        "x",
    ))
    .unwrap();
    assert_eq!(d.anchors, vec![novo_flags::Anchor::Project]);
}

#[test]
fn a_drawn_problem_gets_a_triangle_with_a_key_and_a_popover() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page = d.app.current_page().expect("a page is shown");
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("A contact with free points", |tx| {
            tx.place_symbol(
                page,
                "iec/contacts/no",
                novo_core::Placement::at(novo_core::Point::new(40.0, 40.0)),
                vec![
                    novo_core::Pin::new("1", 0.0, 0.0),
                    novo_core::Pin::new("2", 0.0, 10.0),
                ],
                novo_core::SymbolRole::Unassigned,
            )?;
            Ok(())
        })
        .unwrap();
    d.frame();
    d.frame();
    let marks = &d.app.canvas.schematic_ref().unwrap().canvas.marks.list;
    let triangles: Vec<_> = marks
        .iter()
        .filter(|m| {
            matches!(
                m.kind,
                MarkKind::Warning | MarkKind::Error | MarkKind::Unchecked
            )
        })
        .collect();
    assert!(!triangles.is_empty(), "the free points are flagged");
    assert!(triangles.iter().all(|m| m.key != 0 && !m.text.is_empty()));
    let id = novo_flags::FlagId(triangles[0].key);
    assert!(
        d.app.wb.flags.store.get(id).is_some(),
        "the key names a flag"
    );
    // The popover opens and closes on Esc without disturbing the frame.
    d.app.wb.flags.open = Some((id, eframe::egui::pos2(300.0, 300.0)));
    d.frame();
    assert!(d.app.wb.flags.open.is_some());
    d.key(eframe::egui::Key::Escape, eframe::egui::Modifiers::NONE);
    d.frame();
    assert!(d.app.wb.flags.open.is_none());
}
