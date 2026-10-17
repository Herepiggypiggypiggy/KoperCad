use super::*;
use crate::actions::Action;
use crate::app::gate1_tests::Drive;
use crate::app::workbench::Drawer;

fn open_project() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

#[test]
fn size_classes_follow_the_width_in_points() {
    use SizeClass::*;
    assert_eq!(SizeClass::next(None, 1280.0), Compact); // 1920 px at 150%
    assert_eq!(SizeClass::next(None, 1366.0), Compact);
    assert_eq!(SizeClass::next(None, 1536.0), Regular); // 1920 px at 125%
    assert_eq!(SizeClass::next(None, 1920.0), Wide);
    assert_eq!(SizeClass::next(None, 1707.0), Wide); // 2560 px at 150%
}

#[test]
fn dragging_across_a_boundary_does_not_flip_the_class() {
    use SizeClass::*;
    assert_eq!(SizeClass::next(Some(Compact), 1410.0), Compact);
    assert_eq!(SizeClass::next(Some(Compact), 1430.0), Regular);
    assert_eq!(SizeClass::next(Some(Regular), 1390.0), Regular);
    assert_eq!(SizeClass::next(Some(Regular), 1370.0), Compact);
    assert_eq!(SizeClass::next(Some(Regular), 1710.0), Regular);
    assert_eq!(SizeClass::next(Some(Wide), 1690.0), Wide);
    assert_eq!(SizeClass::next(Some(Wide), 1600.0), Regular);
}

#[test]
fn slot_ids_and_commands_round_trip() {
    for slot in Slot::ALL {
        assert_eq!(Slot::from_id(slot.id()), Some(slot));
    }
    for slot in Slot::PRESETS {
        assert_eq!(Slot::from_command(slot.command()), Some(slot));
    }
    assert_eq!(Slot::from_id("nope"), None);
}

#[test]
fn every_preset_command_is_registered_with_a_menu_home() {
    for slot in Slot::PRESETS {
        let info = novo_workflow::commands::get(slot.command());
        assert!(info.is_some(), "{} is not a command", slot.command());
        assert!(
            crate::app::workbench::APP_COMMANDS.contains(&slot.command()),
            "{} is not run by the app",
            slot.command()
        );
    }
}

#[test]
fn defaults_differ_by_preset_and_size() {
    let draw = Slot::Draw.default_layout(SizeClass::Compact);
    let review = Slot::Review.default_layout(SizeClass::Compact);
    assert_eq!(review.outline_mode, "issues");
    assert_eq!(review.outline_pinned, Some(true));
    assert_ne!(draw, review);
    assert_eq!(
        Slot::Cabinet
            .default_layout(SizeClass::Compact)
            .inspector_pinned,
        Some(false)
    );
    assert_eq!(
        Slot::Cabinet
            .default_layout(SizeClass::Wide)
            .inspector_pinned,
        Some(true)
    );
    assert_eq!(
        Slot::ThreeD.default_layout(SizeClass::Wide).outline_pinned,
        Some(false)
    );
}

#[test]
fn a_layout_is_kept_per_window_size() {
    let (_dir, mut d) = open_project();
    d.app.follow_window_size(1366.0);
    d.app.wb.drawer = Some(Drawer::Cables);
    // The window grows to a wide screen: nothing stored there, so the
    // default shows, and the compact arrangement is kept aside.
    d.app.follow_window_size(1920.0);
    assert_eq!(d.app.wb.drawer, None);
    d.app.wb.drawer = Some(Drawer::Devices);
    d.app.follow_window_size(1300.0);
    assert_eq!(d.app.wb.drawer, Some(Drawer::Cables));
    d.app.follow_window_size(1920.0);
    assert_eq!(d.app.wb.drawer, Some(Drawer::Devices));
}

#[test]
fn presets_open_their_workspace_and_remember_their_panels() {
    let (_dir, mut d) = open_project();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "layout.review", None);
    assert_eq!(d.app.workspace(), Workspace::Schematic);
    assert_eq!(d.app.outline_mode, OutlineMode::Issues);
    d.app.run_command(&ctx, "layout.draw", None);
    assert_eq!(d.app.outline_mode, OutlineMode::Pages);
    d.app.wb.drawer = Some(Drawer::Terminals);
    d.app.run_command(&ctx, "layout.cabinet", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
    assert_eq!(d.app.wb.drawer, None);
    // Back to the drawing: Draw (the last schematic preset) with its drawer.
    d.app.run_command(&ctx, "layout.draw", None);
    assert_eq!(d.app.wb.drawer, Some(Drawer::Terminals));
    // The chip opens the preset last used in the schematic.
    d.app.run_command(&ctx, "layout.review", None);
    d.app.switch_workspace(Workspace::Cabinet);
    d.app.switch_workspace(Workspace::Schematic);
    assert_eq!(d.app.slot, Slot::Review);
    assert_eq!(d.app.outline_mode, OutlineMode::Issues);
}

#[test]
fn reset_puts_the_default_back_at_this_size_only() {
    let (_dir, mut d) = open_project();
    let ctx = d.ctx.clone();
    d.app.follow_window_size(1366.0);
    d.app.wb.drawer = Some(Drawer::Cables);
    d.app.follow_window_size(1920.0);
    d.app.wb.drawer = Some(Drawer::Devices);
    d.app.run_command(&ctx, "layout.reset", None);
    assert_eq!(d.app.wb.drawer, None);
    d.app.follow_window_size(1300.0);
    assert_eq!(d.app.wb.drawer, Some(Drawer::Cables));
    d.app.run_command(&ctx, "layout.reset_all", None);
    assert_eq!(d.app.wb.drawer, None);
    assert!(d.app.settings.workspaces.is_empty());
}

#[test]
fn layouts_written_before_sizes_existed_still_apply() {
    let (_dir, mut d) = open_project();
    d.app.settings.workspaces.insert(
        "schematic".into(),
        WorkspaceLayout {
            outline_pinned: Some(true),
            outline_mode: "changes".into(),
            drawer: Some("cables".into()),
            inspector_pinned: None,
            panels: Default::default(),
        },
    );
    d.app.size_class = None;
    d.app.follow_window_size(1500.0);
    assert_eq!(d.app.outline_mode, OutlineMode::Changes);
    assert_eq!(d.app.wb.drawer, Some(Drawer::Cables));
}

#[test]
fn the_inspector_pin_is_part_of_the_layout() {
    let (_dir, mut d) = open_project();
    let ctx = d.ctx.clone();
    d.app.follow_window_size(1920.0);
    d.app.run_command(&ctx, "layout.cabinet", None);
    assert!(
        d.app.wb.inspector.pinned,
        "wide cabinet docks the inspector"
    );
    d.app.wb.inspector.pinned = false;
    d.app.run_command(&ctx, "layout.draw", None);
    d.app.run_command(&ctx, "layout.cabinet", None);
    assert!(!d.app.wb.inspector.pinned, "the person's choice is kept");
}

#[test]
fn short_windows_have_their_own_class_with_a_hysteresis() {
    assert!(!next_short(None, 0.0), "not measured yet is tall");
    assert!(next_short(None, 737.0), "1366 x 768 minus the title bar");
    assert!(next_short(None, 720.0), "1920 x 1080 at 150%");
    assert!(!next_short(None, 864.0), "1920 x 1080 at 125%");
    assert!(!next_short(None, 1080.0));
    // Near the edge the class stays as it was.
    assert!(next_short(Some(true), 810.0));
    assert!(!next_short(Some(false), 790.0));
    assert!(!next_short(Some(true), 830.0));
    assert!(next_short(Some(false), 770.0));
}

#[test]
fn a_short_window_keeps_its_layout_apart_and_starts_from_the_tall_one() {
    let (_dir, mut d) = open_project();
    let ctx = d.ctx.clone();
    d.frame();
    d.app.follow_window(1920.0, 1000.0);
    d.app.wb.drawer = Some(Drawer::Cables);
    // The window gets shorter: the layout of the same width is the start.
    d.app.follow_window(1920.0, 700.0);
    assert!(d.app.window_short);
    assert_eq!(d.app.wb.drawer, Some(Drawer::Cables));
    // Closing the drawer here is remembered for short windows only.
    d.app.wb.drawer = None;
    d.app.follow_window(1920.0, 1000.0);
    assert_eq!(d.app.wb.drawer, Some(Drawer::Cables), "the tall layout");
    d.app.follow_window(1920.0, 700.0);
    assert_eq!(d.app.wb.drawer, None, "the short layout");
    let keys: Vec<_> = d.app.settings.workspaces.keys().cloned().collect();
    assert!(keys.contains(&"draw/wide".to_owned()), "{keys:?}");
    assert!(keys.contains(&"draw/wide/short".to_owned()), "{keys:?}");
    // The reset goes back to the default for this size only.
    d.app.run_command(&ctx, "layout.reset", None);
    assert!(!d.app.settings.workspaces.contains_key("draw/wide/short"));
    assert!(d.app.settings.workspaces.contains_key("draw/wide"));
}

#[test]
fn a_moved_panel_is_part_of_the_layout() {
    let (_dir, mut d) = open_project();
    let ctx = d.ctx.clone();
    d.frame();
    d.app.follow_window_size(1920.0);
    // The person dragged a panel while Draw was on screen.
    d.app.panel_places.insert("probe".into(), [321, 123]);
    d.app.run_command(&ctx, "layout.review", None);
    let draw = &d.app.settings.workspaces["draw/wide"];
    assert_eq!(draw.panels.get("probe"), Some(&[321, 123]));
    // Review never moved it: it is handed back to its own side.
    assert_eq!(d.app.panel_pending, Some(Default::default()));
    d.app.run_command(&ctx, "layout.draw", None);
    assert_eq!(
        d.app.panel_pending.as_ref().and_then(|p| p.get("probe")),
        Some(&[321, 123]),
        "Draw puts it back where it was left"
    );
}
