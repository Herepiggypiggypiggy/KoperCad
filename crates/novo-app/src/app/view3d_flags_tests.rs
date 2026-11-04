//! The flags in the 3D workspace: drawn on the part, opened by a click,
//! refusals that end on the next input.

use eframe::egui;
use novo_flags::{Action, Anchor, Detail, FlagDraft, Life, Severity, Source};
use novo_view3d::ItemKind;

use super::view3d_tests::drive_with_cabinet;

fn device_flag(raw: u64) -> FlagDraft {
    FlagDraft {
        severity: Severity::Red,
        unchecked: false,
        source: Source::Layout,
        rule: "test.device".into(),
        anchors: vec![Anchor::Device(raw.to_string())],
        title: "Device too hot".into(),
        detail: Detail::new(["-F1 is 12 K over".to_owned()]),
        actions: vec![Action::Locate, Action::Copy],
        waiver_key: None,
        life: Life::Live,
    }
}

fn click(at: egui::Pos2) -> Vec<egui::Event> {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    vec![egui::Event::PointerMoved(at), button(true), button(false)]
}

#[test]
fn a_flag_on_a_device_is_drawn_on_it_and_a_click_opens_its_popover() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let raw = {
        let scene = &d.app.view3d.built.as_ref().unwrap().2;
        scene
            .items
            .iter()
            .find(|i| i.id.kind == ItemKind::Device)
            .expect("a device")
            .id
            .raw
    };
    let id = d.app.wb.flags.store.set(device_flag(raw));
    d.frame();
    let rect = d.app.learn_anchor("workspace.view3d").expect("3D canvas");
    let at = {
        let (_, _, scene) = d.app.view3d.built.as_ref().unwrap();
        let world = super::view3d_flags::world_of(scene, &[], &Anchor::Device(raw.to_string()))
            .expect("the device is in the scene");
        d.app
            .view3d
            .view
            .project(rect, scene, world)
            .expect("on screen")
    };
    assert!(rect.contains(at), "{at:?} in {rect:?}");
    assert!(d.app.wb.flags.open.is_none());
    d.frame_with(click(at));
    d.frame();
    assert_eq!(
        d.app.wb.flags.open.map(|(open, _)| open),
        Some(id),
        "the click on the triangle opens the popover of the flag"
    );
}

#[test]
fn a_refused_duplicate_is_a_yellow_flag_that_ends_on_the_next_press() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let flags_before = d.app.wb.flags.store.counts().total();
    d.app.view3d.flags.refuse(
        "No room for a copy",
        &["No free room on the plate for a copy of the selected device."],
        vec![Anchor::Device("1".into())],
    );
    d.frame();
    assert!(!d.app.view3d.flags.is_empty(), "drawn for a frame or more");
    assert_eq!(
        d.app.wb.flags.store.counts().total(),
        flags_before,
        "a refusal is never counted"
    );
    d.frame_with(vec![egui::Event::PointerButton {
        pos: egui::pos2(5.0, 5.0),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }]);
    assert!(d.app.view3d.flags.is_empty(), "the next press ended it");
}

/// Lines of the 3D view's files that still colour a sentence or a control as
/// a problem (error or warning colour, or a warning toast). Tests excluded.
/// The ceiling in `census/view3d.txt` only goes down; lane T15 asserts zero.
fn census() -> usize {
    const FILES: [&str; 8] = [
        include_str!("view3d.rs"),
        include_str!("view3d_move.rs"),
        include_str!("view3d_rail.rs"),
        include_str!("view3d_duplicate.rs"),
        include_str!("view3d_controls.rs"),
        include_str!("view3d_drag_gaps.rs"),
        include_str!("view3d_build_order.rs"),
        include_str!("hoist_3d.rs"),
    ];
    const MARKS: [&str; 9] = [
        "Level::Error",
        "Level::Warning",
        "warn_fg_color",
        "palette().error",
        "palette().warning",
        "p.error",
        "p.warning",
        "pal.error",
        "pal.warning",
    ];
    FILES
        .iter()
        .map(|text| {
            let code = text.split("\n#[cfg(test)]\nmod ").next().unwrap_or(text);
            code.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .filter(|l| MARKS.iter().any(|m| l.contains(m)))
                .count()
        })
        .sum()
}

#[test]
fn problem_census_view3d_only_goes_down() {
    let ceiling: usize = include_str!("../../census/view3d.txt")
        .trim()
        .parse()
        .expect("a number");
    let now = census();
    assert!(
        now <= ceiling,
        "{now} problem-coloured lines in the 3D view files, ceiling {ceiling}: \
         show the problem as a flag instead"
    );
    assert_eq!(
        now, ceiling,
        "lower census/view3d.txt to {now} (the ceiling only goes down)"
    );
}

/// Writes `qa/fixtures/cabinet-3d-no-room.novo` for scenarios 9430 and 9431: a plate
/// the first device fills, so a copy has nowhere to go; the second device stands off the plate, which is the check flag.
/// Run by hand: `cargo test -p novo-app --lib make_no_room_fixture -- --ignored`.
#[test]
#[ignore = "writes a fixture file"]
fn make_no_room_fixture() {
    use novo_core::{MountingPlate, Point};
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let doc = d.app.backend.document_mut().unwrap();
    let (f1, cab) = {
        let p = doc.project();
        (
            p.devices()
                .find(|dev| dev.tag.to_string().contains("F1"))
                .map(|dev| dev.id)
                .expect("F1"),
            p.structures().next().map(|s| s.id).unwrap(),
        )
    };
    doc.transact("Small plate", |tx| {
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 90.0,
                height: 120.0,
                margin: 5.0,
            }),
        )?;
        tx.place_device(f1, cab, Point::new(10.0, 10.0), false, None)?;
        Ok(())
    })
    .expect("small plate");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/cabinet-3d-no-room.novo");
    let _ = std::fs::remove_file(&path);
    novo_storage::ProjectFile::create(&path, doc.project()).expect("written");
}
