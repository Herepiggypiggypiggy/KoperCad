//! Refused edits as transient triangles (lane T7).

use super::flags_edit::{mark_targets, split_refusal};
use crate::actions::Action;
use crate::app::Level;
use crate::app::gate1_tests::Drive;
use eframe::egui;
use novo_canvas::{MarkKind, MarkTarget};
use novo_core::AnyId;
use novo_flags::{Anchor, Flag, FlagDraft, Life, Severity, Source};

fn flag_on(anchors: Vec<Anchor>, source: Source) -> Flag {
    Flag::from_draft(FlagDraft {
        severity: Severity::Yellow,
        unchecked: false,
        source,
        rule: "t".into(),
        anchors,
        title: "t".into(),
        detail: Default::default(),
        actions: vec![],
        waiver_key: None,
        life: Life::Live,
    })
}

fn open() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

#[test]
fn a_reason_after_a_short_head_becomes_title_and_detail() {
    let (t, d) = split_refusal(Level::Error, "Could not change it: the tag is taken");
    assert_eq!(
        (t.as_str(), d.as_str()),
        ("Could not change it", "the tag is taken")
    );
    let (t, d) = split_refusal(Level::Warning, "\"x\" is not a number");
    assert_eq!(
        (t.as_str(), d.as_str()),
        ("Not done", "\"x\" is not a number")
    );
    let long = format!("{}: reason", "w".repeat(40));
    assert_eq!(split_refusal(Level::Error, &long).0, "Could not do that");
}

#[test]
fn a_refused_edit_is_a_triangle_on_the_selection_and_not_a_toast() {
    let (_dir, mut d) = open();
    let symbol = d.place("contact no", 60.0, 60.0);
    d.select(symbol);
    d.app.refuse(Level::Warning, "Rename: that tag is taken");
    assert!(d.app.messages().iter().all(|m| m.dismissed), "no toast");
    assert!(
        d.app
            .messages()
            .iter()
            .any(|m| m.text.contains("tag is taken")),
        "the log keeps it"
    );
    d.frame();
    d.frame();
    let marks = &d.app.canvas.schematic_ref().unwrap().canvas.marks.list;
    let mark = marks
        .iter()
        .find(|m| m.text.contains("tag is taken"))
        .expect("a triangle on the selected symbol");
    assert_eq!(mark.kind, MarkKind::Warning);
    let id = novo_flags::FlagId(mark.key);
    let flag = d.app.wb.flags.store.get(id).unwrap();
    assert_eq!(flag.source, Source::Edit);
    assert_eq!(flag.title, "Rename");
    // It is not an Issues problem: the pill's count does not move.
    assert!(
        d.app
            .wb
            .problems
            .as_ref()
            .is_none_or(|(_, p)| p.iter().all(|p| !p.message.contains("tag is taken")))
    );
}

#[test]
fn a_refused_edit_goes_with_the_next_input_and_a_new_one_replaces_it() {
    let (_dir, mut d) = open();
    d.app.refuse(Level::Error, "Could not paste: nothing there");
    let first = d.app.wb.flags.store.list(false).len();
    d.app.refuse(Level::Error, "Could not paste: still nothing");
    assert_eq!(
        d.app.wb.flags.store.list(false).len(),
        first,
        "one at a time"
    );
    for _ in 0..4 {
        d.frame();
    }
    assert_eq!(
        d.app.wb.flags.store.list(false).len(),
        first,
        "stays until input"
    );
    d.key(egui::Key::A, egui::Modifiers::NONE);
    d.frame();
    assert!(
        d.app.wb.flags.store.list(false).is_empty(),
        "gone on the next key"
    );
}

#[test]
fn a_refusal_without_a_selection_points_at_the_tool() {
    let (_dir, mut d) = open();
    d.app.refuse(Level::Warning, "Could not add: no name");
    let flag = d.app.wb.flags.store.list(false)[0].clone();
    assert_eq!(flag.anchors, vec![Anchor::Control("tool".into())]);
    d.frame();
    d.frame();
    let ctx = d.ctx.clone();
    assert!(
        ctx.memory(|m| m.area_rect(egui::Id::new("flag_corner").with(0usize)))
            .is_some()
    );
}

#[test]
fn page_device_terminal_and_cable_anchors_are_drawn_on_the_sheet() {
    let (_dir, mut d) = open();
    let symbol = d.place("contact no", 60.0, 60.0);
    d.frame();
    let page = d.app.current_page().unwrap();
    let project = d.app.backend.document().unwrap().project();
    let sym = project.symbol(symbol).unwrap();
    // A page spot becomes a world area on that page only.
    let spot = flag_on(
        vec![Anchor::Page(page.0.to_string(), [10, 20])],
        Source::Issues("check.x".into()),
    );
    assert!(matches!(
        mark_targets(project, &spot, page).as_slice(),
        [MarkTarget::Area(_)]
    ));
    let other = flag_on(
        vec![Anchor::Page("99999".into(), [10, 20])],
        Source::Issues("check.x".into()),
    );
    assert!(mark_targets(project, &other, page).is_empty());
    // A device is drawn on its symbol, once even with the symbol also anchored.
    if let novo_core::SymbolRole::Device { device, .. } = &sym.role {
        let f = flag_on(
            vec![
                Anchor::Device(device.raw().to_string()),
                Anchor::Symbol(symbol.raw().to_string()),
            ],
            Source::Issues("check.x".into()),
        );
        assert_eq!(
            mark_targets(project, &f, page).len(),
            1,
            "one triangle per object"
        );
        // The cabinet kinds belong on the plate, not on the schematic symbol.
        let physical = flag_on(
            vec![Anchor::Device(device.raw().to_string())],
            Source::Issues(novo_checks::ProblemKind::DeviceCollision.key().into()),
        );
        assert!(mark_targets(project, &physical, page).is_empty());
    }
    let _ = AnyId::Symbol(symbol);
}

/// Warning and error toasts of the T7 files: the ceiling only goes down
/// (docs/design/problem-triangles.md section 12, one ceiling file per area).
#[test]
fn problem_census_schematic_edits() {
    const CEILING: usize = 0;
    const FILES: &[(&str, &str)] = &[
        ("inspector.rs", include_str!("inspector.rs")),
        ("clipboard.rs", include_str!("clipboard.rs")),
        ("duplicate.rs", include_str!("duplicate.rs")),
        ("pages.rs", include_str!("pages.rs")),
        ("outline.rs", include_str!("outline.rs")),
        ("macro_actions.rs", include_str!("macro_actions.rs")),
        ("macro_browser.rs", include_str!("macro_browser.rs")),
        ("macro_editor.rs", include_str!("macro_editor.rs")),
        ("tidy.rs", include_str!("tidy.rs")),
        ("pin_offer.rs", include_str!("pin_offer.rs")),
        ("potentials.rs", include_str!("potentials.rs")),
    ];
    let mut found = Vec::new();
    for (name, source) in FILES {
        let code = source.split("#[cfg(test)]").next().unwrap();
        for (n, line) in code.lines().enumerate() {
            let toast = line.contains("self.message(")
                && (line.contains("Level::Warning") || line.contains("Level::Error"));
            // A toast call whose level is on the next line.
            let split = line.trim_end().ends_with("self.message(")
                && code
                    .lines()
                    .nth(n + 1)
                    .is_some_and(|l| l.contains("Level::Warning") || l.contains("Level::Error"));
            if toast || split {
                found.push(format!("{name}:{}", n + 1));
            }
        }
    }
    assert!(
        found.len() == CEILING,
        "{} warning or error toasts, ceiling {CEILING} (lower it, never raise it): {found:?}",
        found.len()
    );
}
