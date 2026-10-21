//! The 3D workspace's toolbar and commands: layers, wires and the door.

use eframe::egui;
use novo_core::{
    CableCore, DeviceTag, MountingPlate, Point, PotentialKind, RailKind, Wire, WireId,
};
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::set_enclosure;
use novo_tests::motor_starter;
use novo_view3d::build::DOOR_HINGE;
use novo_view3d::{ItemKind, Scene};

use super::gate1_tests::Drive;
use super::workspace::Workspace;
use crate::actions::Action;

/// A project with the motor starter, two of its devices on a plate with a
/// duct, and a door that opens to 100 degrees.
pub(super) fn drive_with_cabinet(dir: &std::path::Path) -> Drive {
    let mut d = Drive::new(&dir.join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let doc = d.app.backend.document_mut().unwrap();
    let h = motor_starter(doc).expect("fixture builds");
    doc.transact("Cabinet", |tx| {
        let cab = tx.add_structure(DeviceTag::parse("+CAB")?, "Cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 800.0,
                height: 600.0,
                margin: 10.0,
            }),
        )?;
        tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, None)?;
        tx.place_device(h.k1, cab, Point::new(400.0, 100.0), false, None)?;
        let duct = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
        tx.set_rail_size(duct, 700.0, 50.0, 60.0)?;
        set_enclosure(
            tx,
            cab,
            &Enclosure {
                width: 800.0,
                height: 600.0,
                depth: 300.0,
                plate_setback: 40.0,
                door: Some(Door {
                    hinge: Hinge::Left,
                    max_angle: 100.0,
                    inner_depth: 0.0,
                }),
                front_clearance: None,
                hinge_side_clearance: None,
            },
        )?;
        Ok(())
    })
    .expect("cabinet");
    d.app.run_command(&ctx, "view.3d", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
    d
}

fn scene(d: &Drive) -> &Scene {
    &d.app.view3d.built.as_ref().expect("a scene").2
}

fn has(scene: &Scene, kind: ItemKind) -> bool {
    scene.items.iter().any(|i| i.id.kind == kind)
}

#[test]
fn the_routed_wires_are_in_the_3d_scene_and_the_command_hides_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    assert!(!scene(&d).wires.is_empty(), "the wires of the plate show");
    d.app.run_command(&ctx, "view.3d_wires", None);
    d.frame();
    assert!(scene(&d).wires.is_empty());
    assert!(has(scene(&d), ItemKind::Plate), "only the wires went");
    d.app.run_command(&ctx, "view.3d_wires", None);
    d.frame();
    assert!(!scene(&d).wires.is_empty(), "and they come back");
}

/// The wires drawn in the scene, as project wire ids.
fn wire_ids(d: &Drive) -> Vec<WireId> {
    scene(d)
        .wires
        .iter()
        .map(|w| WireId::from(w.id.raw))
        .collect()
}

#[test]
fn wires_are_drawn_in_the_colour_the_schematic_gives_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ids = wire_ids(&d);
    assert!(ids.len() >= 2, "the fixture has wires");
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Colours", |tx| {
        tx.update::<Wire>(ids[0], |w| w.color = Some("BU".into()))?;
        tx.update::<Wire>(ids[1], |w| w.color = Some("GNYE".into()))?;
        Ok(())
    })
    .unwrap();
    d.frame();
    let wire = |id: WireId| {
        let w = scene(&d)
            .wires
            .iter()
            .find(|w| w.id.raw == id.raw())
            .expect("the wire is still in the scene");
        (w.tint, w.stripe)
    };
    assert_eq!(wire(ids[0]), (Some([0x2F, 0x6F, 0xD0]), None));
    assert_eq!(
        wire(ids[1]),
        (Some([0x7F, 0xB0, 0x20]), Some([0xE3, 0xD3, 0x4A])),
        "green-yellow has a stripe"
    );
}

#[test]
fn a_wire_without_a_colour_takes_it_from_its_cable_core_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ids = wire_ids(&d);
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Cable", |tx| {
        let cable = tx.add_cable(
            DeviceTag::parse("-W1")?,
            vec![CableCore::new("BK"), CableCore::new("1")],
        )?;
        tx.assign_core(cable, 0, Some(ids[0]))?;
        tx.assign_core(cable, 1, Some(ids[1]))?;
        Ok(())
    })
    .unwrap();
    d.frame();
    let tint_of = |id: WireId| {
        scene(&d)
            .wires
            .iter()
            .find(|w| w.id.raw == id.raw())
            .expect("the wire is still in the scene")
            .tint
    };
    // BK is a colour code and colours the wire; "1" is a number and does not.
    assert_eq!(tint_of(ids[0]), Some([0x1B, 0x1B, 0x1E]));
    assert_eq!(tint_of(ids[1]), None);
}

#[test]
fn a_wire_without_a_colour_takes_the_colour_of_its_potential() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ids = wire_ids(&d);
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Earth", |tx| {
        let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
        let l1 = tx.add_potential("L1", PotentialKind::Line)?;
        tx.set_wire_potential(ids[0], Some(pe))?;
        tx.set_wire_potential(ids[1], Some(l1))?;
        Ok(())
    })
    .unwrap();
    d.frame();
    fn look(d: &Drive, id: WireId) -> (Option<[u8; 3]>, Option<[u8; 3]>) {
        let w = scene(d)
            .wires
            .iter()
            .find(|w| w.id.raw == id.raw())
            .expect("the wire is still in the scene");
        (w.tint, w.stripe)
    }
    assert_eq!(
        look(&d, ids[0]),
        (Some([0x7F, 0xB0, 0x20]), Some([0xE3, 0xD3, 0x4A])),
        "PE is green-yellow"
    );
    assert_eq!(
        look(&d, ids[1]),
        (None, None),
        "a line conductor stays plain"
    );
    // Neutral and 0 V are blue, positive DC red (3d-look.md section 3).
    for (name, kind, tint) in [
        ("N", PotentialKind::Neutral, [0x2F, 0x6F, 0xD0]),
        ("0V", PotentialKind::DcNegative, [0x2F, 0x6F, 0xD0]),
        ("24V", PotentialKind::DcPositive, [0xB3, 0x3A, 0x3A]),
    ] {
        let doc = d.app.backend.document_mut().unwrap();
        doc.transact("Potential", |tx| {
            let p = tx.add_potential(name, kind)?;
            tx.set_wire_potential(ids[1], Some(p))
        })
        .unwrap();
        d.frame();
        assert_eq!(look(&d, ids[1]), (Some(tint), None), "{name}");
    }
    // The schematic's own colour wins.
    let doc = d.app.backend.document_mut().unwrap();
    doc.transact("Colour", |tx| {
        tx.update::<Wire>(ids[0], |w| w.color = Some("BU".into()))
    })
    .unwrap();
    d.frame();
    assert_eq!(look(&d, ids[0]), (Some([0x2F, 0x6F, 0xD0]), None));
}

#[test]
fn the_enclosure_layer_hides_the_box_but_not_the_door() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    assert!(has(scene(&d), ItemKind::Enclosure) && has(scene(&d), ItemKind::Door));
    d.app.run_command(&ctx, "view.3d_shell", None);
    d.frame();
    assert!(!has(scene(&d), ItemKind::Enclosure));
    assert!(has(scene(&d), ItemKind::Door));
}

#[test]
fn the_door_command_opens_the_door_to_its_limit_and_closes_it_again() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    assert_eq!(d.app.view3d.controls.door_deg, 0.0);
    // The door is hinged on the left and the enclosure says 100 degrees.
    assert_eq!(scene(&d).group(DOOR_HINGE).unwrap().angle, 0.0);
    d.app.run_command(&ctx, "view.3d_door", None);
    assert_eq!(d.app.view3d.controls.door_deg, 100.0);
    d.frame();
    d.app.run_command(&ctx, "view.3d_door", None);
    assert_eq!(d.app.view3d.controls.door_deg, 0.0);
}

#[test]
fn hiding_the_door_layer_closes_it_and_a_door_command_brings_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.view3d.controls.layers.door = false;
    d.frame();
    assert!(!has(scene(&d), ItemKind::Door));
    assert!(scene(&d).group(DOOR_HINGE).is_none());
    d.app.run_command(&ctx, "view.3d_door", None);
    d.frame();
    assert!(has(scene(&d), ItemKind::Door));
    assert_eq!(d.app.view3d.controls.door_deg, 100.0);
}

#[test]
fn the_section_command_cuts_the_picture_and_picking_follows_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    assert!(d.app.view3d.view.state.section.is_none());
    d.app.run_command(&ctx, "view.3d_section", None);
    assert!(d.app.view3d.view.state.section.is_some());
    d.frame();
    // The scene itself is whole: only the picture is cut.
    assert!(has(scene(&d), ItemKind::Plate));
    d.app.run_command(&ctx, "view.3d_section", None);
    assert!(d.app.view3d.view.state.section.is_none());
}

#[test]
fn the_save_view_command_keeps_the_camera_in_the_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    // Headless there is no first-show fit: fit once, as a shown view would.
    let scene = scene(&d).clone();
    d.app.view3d.view.fit(&scene, 0.0, true);
    d.app.view3d.view.camera.yaw = 0.9;
    d.app.run_command(&ctx, "view.3d_save_view", None);
    d.frame();
    let views = &d.app.settings.camera_views;
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].name, "View 1");
    assert!((views[0].yaw - 0.9).abs() < 1e-6);
}

#[test]
fn a_view_saved_before_the_3d_view_was_ever_shown_is_the_fitted_camera() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    // A fresh 3D view: its camera is the unfitted default until it is shown.
    d.app.view3d.view = novo_view3d::widget::View3d::new(1);
    let unfitted = d.app.view3d.view.camera;
    d.app.run_command(&ctx, "view.3d_save_view", None);
    d.frame();
    // Not shown and fitted yet (no graphics adapter here): nothing is saved.
    assert!(d.app.settings.camera_views.is_empty());
    assert!(d.app.view3d.save_view);
    let scene = scene(&d).clone();
    d.app.view3d.view.fit(&scene, 0.0, true);
    d.frame();
    let views = &d.app.settings.camera_views;
    assert_eq!(views.len(), 1);
    assert_ne!(
        views[0].distance, unfitted.distance,
        "the saved camera is the fitted one"
    );
}

#[test]
fn the_hints_command_folds_and_unfolds_the_legend() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    assert!(!d.app.view3d.hints.open, "starts as the chip");
    d.app.run_command(&ctx, "view.3d_hints", None);
    d.frame();
    assert!(d.app.view3d.hints.open);
    d.app.run_command(&ctx, "view.3d_hints", None);
    assert!(!d.app.view3d.hints.open);
}

#[test]
fn select_by_type_asked_before_the_scene_exists_applies_after_the_first_build() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.view3d.built = None;
    d.app.view3d.view.state.selection.clear();
    d.app.run_command(&ctx, "view.3d_select_devices", None);
    assert!(d.app.view3d.view.state.selection.is_empty());
    d.frame();
    assert!(!d.app.view3d.view.state.selection.is_empty());
}

#[test]
fn every_rebuild_of_the_scene_has_a_new_revision_so_the_renderer_redraws() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    let before = scene(&d).revision;
    d.app.run_command(&ctx, "view.3d_select_devices", None);
    d.app.run_command(&ctx, "view.3d_hide", None);
    d.frame();
    assert_ne!(scene(&d).revision, before);
}

#[test]
fn hidden_ids_of_items_that_no_longer_exist_are_dropped_on_the_next_build() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ghost = novo_view3d::ItemId::new(ItemKind::Device, u64::MAX);
    d.app.view3d.controls.hidden.insert(ghost);
    d.app.view3d.built = None;
    d.frame();
    assert!(d.app.view3d.controls.hidden.is_empty());
}

#[test]
fn select_by_type_hide_isolate_and_show_all_work_on_the_picture_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    let count = |d: &Drive, kind| scene(d).items.iter().filter(|i| i.id.kind == kind).count();
    let devices = count(&d, ItemKind::Device);
    assert!(devices >= 2, "the example cabinet has devices");
    d.app.run_command(&ctx, "view.3d_select_devices", None);
    assert_eq!(d.app.view3d.view.state.selection.len(), devices);
    d.app.run_command(&ctx, "view.3d_hide", None);
    d.frame();
    assert_eq!(count(&d, ItemKind::Device), 0);
    assert!(d.app.view3d.view.state.selection.is_empty());
    d.app.run_command(&ctx, "view.3d_show_all", None);
    d.frame();
    assert_eq!(count(&d, ItemKind::Device), devices);
    // Isolate one device: the others go, the plate stays.
    let one = *d.app.view3d.view.state.selection.iter().next().unwrap_or(
        &scene(&d)
            .items
            .iter()
            .find(|i| i.id.kind == ItemKind::Device)
            .unwrap()
            .id,
    );
    d.app.view3d.view.state.selection = std::collections::BTreeSet::from([one]);
    d.app.run_command(&ctx, "view.3d_isolate", None);
    d.frame();
    assert_eq!(count(&d, ItemKind::Device), 1);
    assert!(has(scene(&d), ItemKind::Plate));
}

/// One frame with `key` pressed, as the app sees a keystroke.
fn press(d: &mut Drive, key: egui::Key, modifiers: egui::Modifiers) {
    d.frame_with(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }]);
}

#[test]
fn shift_d_swings_the_door_in_the_3d_workspace_and_only_there() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    press(&mut d, egui::Key::D, egui::Modifiers::SHIFT);
    assert_eq!(d.app.view3d.controls.door_deg, 100.0, "Shift+D opened it");
    press(&mut d, egui::Key::D, egui::Modifiers::SHIFT);
    assert_eq!(d.app.view3d.controls.door_deg, 0.0, "and closed it");

    // On the Schematic the key does nothing: no door, no workspace switch.
    d.app.switch_workspace(Workspace::Schematic);
    d.frame();
    press(&mut d, egui::Key::D, egui::Modifiers::SHIFT);
    assert_eq!(d.app.workspace(), Workspace::Schematic);
    assert_eq!(d.app.view3d.controls.door_deg, 0.0);
}

#[test]
fn ctrl_6_opens_the_3d_workspace_from_the_cabinet_and_from_3d_itself() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.app.switch_workspace(Workspace::Cabinet);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
    press(&mut d, egui::Key::Num6, egui::Modifiers::COMMAND);
    assert_eq!(d.app.workspace(), Workspace::View3d);
}

#[test]
fn the_keymap_has_the_3d_keys() {
    use novo_workflow::keymap::{Chord, Focus, Key, Keymap};
    let map = Keymap::workflow();
    assert_eq!(
        map.resolve(Chord::ctrl(Key::Digit(6)), Focus::Canvas),
        Some("view.3d")
    );
    assert_eq!(
        map.resolve(Chord::shift(Key::Letter('D')), Focus::Canvas),
        Some("view.3d_door")
    );
}

/// One frame at egui time `time` seconds.
fn frame_at(d: &mut Drive, time: f64) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 900.0),
        )),
        time: Some(time),
        ..Default::default()
    };
    let app = &mut d.app;
    let mut output = d.ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
}

#[test]
fn the_stats_command_shows_the_panel_and_spinning_turns_the_view_and_samples_frames() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    assert!(!d.app.view3d.stats.shown);
    d.app.run_command(&ctx, "view.3d_stats", None);
    assert!(d.app.view3d.stats.shown);

    let yaw = d.app.view3d.view.camera.yaw;
    d.app.view3d.stats.spin = true;
    for i in 0..6 {
        frame_at(&mut d, 10.0 + i as f64 * 0.016);
    }
    assert_ne!(d.app.view3d.view.camera.yaw, yaw, "the view turned");
    let samples = d.app.view3d.stats.samples();
    assert!(!samples.is_empty(), "frames were sampled");
    assert!(
        samples.iter().all(|ms| (15.0..17.0).contains(ms)),
        "16 ms frames: {samples:?}"
    );

    // Closing the panel stops the spin and the sampling.
    d.app.run_command(&ctx, "view.3d_stats", None);
    assert!(!d.app.view3d.stats.shown && !d.app.view3d.stats.spin);
    let yaw = d.app.view3d.view.camera.yaw;
    frame_at(&mut d, 11.0);
    frame_at(&mut d, 11.016);
    assert_eq!(d.app.view3d.view.camera.yaw, yaw);
}

#[test]
fn the_stats_command_is_in_the_view_menu_and_works_from_the_cabinet() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.cabinet", None);
    d.frame();
    assert_ne!(d.app.workspace(), Workspace::View3d);
    d.app.run_command(&ctx, "view.3d_stats", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
    assert!(d.app.view3d.stats.shown);
}

#[test]
fn the_3d_view_registers_the_anchor_lessons_point_at() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let rect = d
        .app
        .learn_anchor("workspace.view3d")
        .expect("`workspace.view3d` is not on screen");
    assert!(rect.width() > 0.0 && rect.height() > 0.0, "{rect:?}");
}

#[test]
fn the_shadows_command_toggles_the_views_shadows() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.frame();
    assert!(d.app.view3d.view.state.display.shadows, "on by default");
    d.app.run_command(&ctx, "view.3d_shadows", None);
    d.frame();
    assert!(!d.app.view3d.view.state.display.shadows);
    d.app.run_command(&ctx, "view.3d_shadows", None);
    d.frame();
    assert!(d.app.view3d.view.state.display.shadows);
}

#[test]
fn simulate_runs_from_the_3d_view_without_leaving_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    assert!(d.app.cabinet.heat.solution().is_none());
    d.app.run_command(&ctx, "heat.simulate", None);
    assert_eq!(d.app.workspace(), Workspace::View3d, "Simulate stays here");
    assert!(
        d.app.view3d.controls.heat_panel,
        "the panel with progress and stop opens with the run"
    );
    for _ in super::test_wait::patient() {
        d.frame();
        if d.app.cabinet.heat.solution().is_some() && !d.app.cabinet.heat.running() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(
        d.app.cabinet.heat.solution().is_some(),
        "the run published a result to the 3D view"
    );
}

/// The Heat panel is open before the first result; when the result adds the
/// Heat strip above the view, the panel moves down with the view cube instead
/// of staying where it was (egui keeps the position of a window that exists).
#[test]
fn an_open_heat_panel_follows_the_view_cube_down_when_the_heat_strip_appears() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.3d", None);
    d.app.view3d.controls.heat_panel = true;
    let size = egui::vec2(1366.0, 768.0);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    d.app.run_command(&ctx, "heat.simulate", None);
    let mut strip_seen = false;
    for frame in super::test_wait::patient() {
        d.frame_sized(size, Vec::new());
        let (Some(heat), Some(canvas)) = (
            ctx.memory(|m| m.area_rect(super::panel::id("view3d_heat_panel"))),
            d.app.learn_anchor("workspace.view3d"),
        ) else {
            continue;
        };
        // The canvas starts lower than the panel's first position once the strip shows.
        strip_seen |= canvas.top() > 100.0;
        let cube = egui::Rect::from_min_size(
            egui::pos2(canvas.right() - 12.0 - 88.0, canvas.top() + 12.0),
            egui::Vec2::splat(88.0),
        );
        assert!(
            !heat.intersects(cube),
            "frame {frame}: heat panel {heat:?} covers the view cube {cube:?} (canvas {canvas:?})"
        );
        if strip_seen && frame > 30 && !d.app.cabinet.heat.running() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(strip_seen, "needs the Heat strip above the view");
}

/// With a heat result the Heat strip sits above the view; the Heat panel must
/// still start under the view cube, which is drawn in the corner of the
/// canvas below that strip (creative audit i01, 1366 wide).
#[test]
fn the_heat_panel_starts_under_the_view_cube_when_a_result_shows_the_heat_strip() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "heat.simulate", None);
    let size = egui::vec2(1366.0, 768.0);
    let mut seen_result = false;
    for frame in super::test_wait::patient() {
        d.frame_sized(size, Vec::new());
        seen_result |= d.app.cabinet.heat.solution().is_some();
        // Every frame, while the run is still improving as well as after it.
        if let (Some(heat), Some(canvas)) = (
            ctx.memory(|m| m.area_rect(super::panel::id("view3d_heat_panel"))),
            d.app.learn_anchor("workspace.view3d"),
        ) {
            let cube = egui::Rect::from_min_size(
                egui::pos2(canvas.right() - 12.0 - 88.0, canvas.top() + 12.0),
                egui::Vec2::splat(88.0),
            );
            assert!(
                !heat.intersects(cube),
                "frame {frame}: heat panel {heat:?} covers the view cube {cube:?} (canvas {canvas:?})"
            );
        }
        if seen_result && !d.app.cabinet.heat.running() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(seen_result, "needs a result");
}

#[test]
fn the_measure_command_switches_the_tool_and_it_forgets_its_points_when_left() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    assert!(!d.app.view3d.measure.on);
    d.app.run_command(&ctx, "view.3d_measure", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
    assert!(d.app.view3d.measure.on);
    d.app
        .view3d
        .measure
        .click(glam::Vec3::new(10.0, 10.0, 0.0), None);
    assert!(d.app.view3d.measure.has_pins());
    d.app.run_command(&ctx, "view.3d_measure", None);
    d.frame();
    assert!(!d.app.view3d.measure.on && !d.app.view3d.measure.has_pins());
}

#[test]
fn the_wire_runs_command_opens_the_list_turns_the_wires_on_and_reads_their_lengths() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.view3d.controls.layers.wires = false;
    d.app.run_command(&ctx, "view.3d_wire_runs", None);
    d.frame();
    d.frame();
    assert!(d.app.view3d.wire_runs);
    assert!(d.app.view3d.controls.layers.wires, "the wires layer is on");
    let facts = d.app.view3d.wire_facts.as_ref().map(|f| &f.2);
    assert!(facts.is_some(), "the lengths were read with the routes");
    for f in facts.unwrap().values() {
        assert!(f.length_mm > 0.0, "a cut length is never zero: {f:?}");
        assert!(!f.from.is_empty() && !f.to.is_empty());
    }
    d.app.run_command(&ctx, "view.3d_wire_runs", None);
    assert!(!d.app.view3d.wire_runs);
}

#[test]
fn the_position_command_hides_and_shows_the_box() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    assert!(!d.app.view3d.position.hidden, "shown by default");
    d.app.run_command(&ctx, "view.3d_position", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
    assert!(d.app.view3d.position.hidden);
    d.app.run_command(&ctx, "view.3d_position", None);
    assert!(!d.app.view3d.position.hidden);
}

#[test]
fn the_place_command_opens_the_list_in_3d_and_toggling_lets_go_of_the_device() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    assert!(!d.app.view3d.place.open);
    d.app.run_command(&ctx, "view.3d_place", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
    assert!(d.app.view3d.place.open);
    d.app.view3d.place.armed = Some(1);
    d.frame();
    d.app.run_command(&ctx, "view.3d_place", None);
    d.frame();
    assert!(!d.app.view3d.place.open && d.app.view3d.place.armed.is_none());
}

#[test]
fn dimensions_toggle_by_command_and_measure_the_selected_part_from_the_cabinet() {
    use novo_view3d::ItemKind;
    use novo_view3d::dimension::{Role, dimensions, format_mm};
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.frame();
    assert!(
        !d.app.view3d.view.state.display.dimensions,
        "off by default"
    );
    d.app.run_command(&ctx, "view.3d_dimensions", None);
    d.frame();
    assert!(d.app.view3d.view.state.display.dimensions);

    let scene = d.app.view3d.built.as_ref().expect("scene built").2.clone();
    let device = scene
        .items
        .iter()
        .find(|i| i.id.kind == ItemKind::Device)
        .expect("a device on the plate");
    let none = dimensions(&scene, &Default::default());
    assert!(
        none.iter().all(|m| m.role == Role::Overall) && none.len() >= 2,
        "overall size only: {none:?}"
    );
    let one = dimensions(&scene, &std::iter::once(device.id).collect());
    let w = format!("W {}", format_mm(device.size.x));
    let h = format!("H {}", format_mm(device.size.y));
    assert!(
        one.iter().any(|m| m.role == Role::Size && m.text == w),
        "{w}: {one:?}"
    );
    assert!(
        one.iter().any(|m| m.role == Role::Size && m.text == h),
        "{h}: {one:?}"
    );
    assert!(
        one.iter().any(|m| matches!(m.role, Role::Gap | Role::Edge)),
        "distances to the plate edge or a neighbour: {one:?}"
    );
    assert!(one.iter().all(|m| m.mm.is_finite() && m.mm >= 0.0));

    // Painting the lines for a selected part runs without trouble.
    d.app.view3d.view.state.selection = std::iter::once(device.id).collect();
    d.frame();
    d.frame();

    d.app.run_command(&ctx, "view.3d_dimensions", None);
    d.frame();
    assert!(!d.app.view3d.view.state.display.dimensions);
}

#[test]
fn restore_quality_returns_a_stepped_down_view_to_the_adapters_tier() {
    use novo_view3d::render::Tier;
    use novo_view3d::widget::register_adapter;
    let _guard = super::view3d_move_tests::STATS_GUARD
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    register_adapter("Test GPU", "Vulkan", Tier::Discrete);
    d.frame();
    assert_eq!(d.app.view3d.view.stepped_down(), None);

    // Moving frames at 40 ms for four seconds: the governor steps the
    // quality down. (The window-less test drives the governor itself; the
    // widget calls it every frame.)
    for i in 0..110 {
        let input = egui::RawInput {
            time: Some(20.0 + i as f64 * 0.04),
            ..Default::default()
        };
        let view = &mut d.app.view3d.view;
        let mut out = ctx.run_ui(input, |_| {
            view.govern(&ctx, true);
        });
        out.textures_delta.clear();
    }
    let down = d.app.view3d.view.stepped_down();
    assert_eq!(down, Some(Tier::Integrated), "the floor for hardware");
    frame_at(&mut d, 30.0);
    assert!(
        d.app.view3d.controls.reduced,
        "the toolbar offers to restore"
    );

    d.app.run_command(&ctx, "view.3d_quality", None);
    assert_eq!(
        d.app.view3d.view.stepped_down(),
        None,
        "back at the adapter's tier"
    );
    frame_at(&mut d, 31.0);
    assert!(
        !d.app.view3d.controls.reduced,
        "nothing to restore any more"
    );
}

#[test]
fn the_ambient_occlusion_command_toggles_the_views_occlusion() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.frame();
    assert!(d.app.view3d.view.state.display.ssao, "on by default");
    d.app.run_command(&ctx, "view.3d_ssao", None);
    d.frame();
    assert!(!d.app.view3d.view.state.display.ssao);
    d.app.run_command(&ctx, "view.3d_ssao", None);
    d.frame();
    assert!(d.app.view3d.view.state.display.ssao);
}

#[test]
fn duplicate_in_3d_adds_a_device_with_the_next_tag_on_the_plate_and_undo_takes_it_back() {
    use novo_view3d::scene::{ItemId, ItemKind};
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.frame();
    let (devices, placed, src) = {
        let project = d.app.backend.document().unwrap().project();
        let cab = project.structures().find(|s| s.enclosure).unwrap().id;
        let lay = novo_layout::model::load(project, cab).unwrap();
        (
            project.all::<novo_core::Device>().count(),
            lay.placements.len(),
            lay.placements[0].id,
        )
    };
    d.app.view3d.view.state.selection = [ItemId::new(ItemKind::Device, src)].into();
    d.app.run_command(&ctx, "view.3d_duplicate", None);
    d.frame();
    let project = d.app.backend.document().unwrap().project();
    let cab = project.structures().find(|s| s.enclosure).unwrap().id;
    let lay = novo_layout::model::load(project, cab).unwrap();
    assert_eq!(project.all::<novo_core::Device>().count(), devices + 1);
    assert_eq!(lay.placements.len(), placed + 1);
    assert!(lay.report().collisions.is_empty() && lay.report().outside.is_empty());
    assert_eq!(d.app.view3d.view.state.selection.len(), 1);
    assert!(
        !d.app
            .view3d
            .view
            .state
            .selection
            .iter()
            .any(|i| i.raw == src),
        "the copy is selected, not the original"
    );
    let tags: Vec<_> = project
        .all::<novo_core::Device>()
        .map(|x| x.tag.clone())
        .collect();
    let unique: std::collections::BTreeSet<_> = tags.iter().collect();
    assert_eq!(unique.len(), tags.len(), "every tag is still unique");

    d.app.run(&ctx, Action::Undo);
    d.frame();
    let project = d.app.backend.document().unwrap().project();
    assert_eq!(project.all::<novo_core::Device>().count(), devices);
}

#[test]
fn a_picked_surface_shows_as_panes_in_the_3d_view_and_clear_when_none_is_picked() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    assert!(
        d.app.view3d.surface_panes.is_empty(),
        "nothing picked, toggle off"
    );
    d.app.cabinet.surface = Some("door".to_owned());
    d.frame();
    let picked = d.app.view3d.surface_panes.len();
    assert!(picked > 0, "the picked door is drawn");
    d.app.cabinet.surface = None;
    d.frame();
    assert!(d.app.view3d.surface_panes.is_empty());
}

/// World-space bounding box of an item's local box.
fn world_bounds(item: &novo_view3d::scene::Item) -> (glam::Vec3, glam::Vec3) {
    let m = item.pose.matrix();
    let (hx, hy, sz) = (item.size.x / 2.0, item.size.y / 2.0, item.size.z);
    let mut lo = glam::Vec3::splat(f32::MAX);
    let mut hi = glam::Vec3::splat(f32::MIN);
    for x in [-hx, hx] {
        for y in [-hy, hy] {
            for z in [0.0, sz] {
                let p = m.transform_point3(glam::Vec3::new(x, y, z));
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
    }
    (lo, hi)
}

#[test]
fn the_surface_panes_land_on_the_real_shell_and_the_door_pane_is_the_door() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.app.cabinet.surface = Some("door".to_owned());
    d.frame();
    d.frame();
    let summary = {
        let project = d.app.backend.document().unwrap().project();
        let cab = project.structures().find(|s| s.enclosure).unwrap().id;
        super::cabinet_surfaces::summarize(project, cab).unwrap()
    };
    let names: Vec<&str> = summary
        .rows
        .iter()
        .filter(|r| r.name != "plate")
        .map(|r| r.name.as_str())
        .collect();
    let novo_view3d::Overlay::Mesh(mesh) = &d.app.view3d.surface_panes[0] else {
        panic!("the panes are the first overlay")
    };
    assert_eq!(mesh.positions.len(), names.len() * 4);
    let bounds = |i: usize| {
        let c = &mesh.positions[i * 4..i * 4 + 4];
        (
            c.iter()
                .copied()
                .fold(glam::Vec3::splat(f32::MAX), glam::Vec3::min),
            c.iter()
                .copied()
                .fold(glam::Vec3::splat(f32::MIN), glam::Vec3::max),
        )
    };
    let items = &scene(&d).items;
    // The enclosure shell: every pane lies inside it, and the door pane
    // fills the door ghost's face. The scene's box starts at the plate; the
    // cabinet's walls reach back to the back wall, `plate_setback` behind it.
    let setback = {
        let project = d.app.backend.document().unwrap().project();
        let cab = project.structures().find(|s| s.enclosure).unwrap().id;
        novo_layout::model::enclosure_of(project, cab)
            .unwrap()
            .plate_setback as f32
    };
    let shell = items
        .iter()
        .filter(|i| matches!(i.id.kind, ItemKind::Enclosure | ItemKind::Door))
        .map(world_bounds)
        .fold(
            (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN)),
            |(a, b), (lo, hi)| (a.min(lo), b.max(hi)),
        );
    for (i, name) in names.iter().enumerate() {
        let (lo, hi) = bounds(i);
        assert!(
            lo.cmpge(shell.0 - glam::Vec3::new(1.0, 1.0, setback + 1.0))
                .all()
                && hi.cmple(shell.1 + 1.0).all(),
            "{name} pane {lo:?}..{hi:?} outside the shell {:?}..{:?}",
            shell.0,
            shell.1
        );
    }
    let door = items
        .iter()
        .find(|i| i.id.kind == ItemKind::Door)
        .expect("a door in the scene");
    let (dlo, dhi) = world_bounds(door);
    let i = names
        .iter()
        .position(|n| *n == "door")
        .expect("a door surface");
    let (plo, phi) = bounds(i);
    let tol = 1.5;
    assert!(
        (plo.x - dlo.x).abs() < tol
            && (phi.x - dhi.x).abs() < tol
            && (plo.y - dlo.y).abs() < tol
            && (phi.y - dhi.y).abs() < tol,
        "door pane {plo:?}..{phi:?} vs door {dlo:?}..{dhi:?}"
    );
    assert!(plo.z >= dlo.z - tol && phi.z <= dhi.z + tol);
}

/// K1 of the starter: its device id and one of its symbols.
fn k1(d: &Drive) -> (novo_core::DeviceId, novo_core::SymbolId) {
    let project = d.app.backend.document().unwrap().project();
    let device = project.devices_by_tag(&DeviceTag::parse("=A1+CAB-K1").unwrap())[0];
    let symbol = project
        .symbols()
        .find(|s| s.device() == Some(device))
        .expect("K1 is drawn")
        .id;
    (device, symbol)
}

/// Opens the drawing on the device's main symbol, selected.
fn draw_selected(d: &mut Drive, device: novo_core::DeviceId) {
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.schematic", None);
    let place = d.app.device_main(device).expect("K1 is drawn");
    d.app.jump(place, false);
    for _ in 0..4 {
        d.frame();
    }
}

#[test]
fn show_in_3d_from_the_schematic_selects_and_frames_the_device() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    let (device, _) = k1(&d);
    draw_selected(&mut d, device);
    assert_eq!(d.app.probe_devices(), vec![device]);
    d.app.run_command(&ctx, "nav.show_3d", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
    assert!(
        d.app
            .view3d
            .view
            .state
            .selection
            .contains(&novo_view3d::ItemId::new(ItemKind::Device, device.raw())),
        "the device is selected in 3D"
    );
}

#[test]
fn show_in_cabinet_selects_the_device_on_the_plate() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    let (device, _) = k1(&d);
    draw_selected(&mut d, device);
    d.app.run_command(&ctx, "nav.show_cabinet", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::Cabinet);
    assert_eq!(
        d.app.cabinet.view.selection.ids(),
        vec![novo_canvas::cabinet::box_item(device.raw())]
    );
}

#[test]
fn show_in_schematic_from_the_plate_opens_the_devices_symbol() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    let (device, symbol) = k1(&d);
    d.app.run_command(&ctx, "view.cabinet", None);
    d.frame();
    d.app.cabinet.view.selection.apply(
        [novo_canvas::cabinet::box_item(device.raw())],
        novo_canvas::selection::SelectOp::Replace,
    );
    d.app.run_command(&ctx, "nav.show_schematic", None);
    for _ in 0..4 {
        d.frame();
    }
    assert_eq!(d.app.workspace(), Workspace::Schematic);
    let shown = d.app.selected_symbol();
    let project = d.app.backend.document().unwrap().project();
    let owner = shown
        .and_then(|s| project.symbol(s))
        .and_then(|s| s.device());
    assert_eq!(
        owner,
        Some(device),
        "a symbol of K1 is selected ({symbol:?} is one)"
    );
}

#[test]
fn show_in_with_nothing_selected_says_so_and_stays() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.cabinet.view.selection.clear();
    d.app.run_command(&ctx, "nav.show_schematic", None);
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::View3d);
}

#[test]
fn the_trackpad_choice_in_the_settings_reaches_the_3d_view() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.3d", None);
    d.frame();
    assert!(
        !d.app.view3d.view.state.display.trackpad_pans,
        "off at first"
    );
    d.app.settings.trackpad_pans = true;
    d.frame();
    assert!(d.app.view3d.view.state.display.trackpad_pans);
    assert!(d.app.settings.trackpad_pans, "the frame keeps the choice");
}

#[test]
fn show_in_3d_for_an_unplaced_device_says_it_has_nothing_to_show() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    let (device, _) = k1(&d);
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Unplace K1", |tx| {
            novo_layout::model::unplace_device(tx, device.raw())
        })
        .expect("unplaced");
    draw_selected(&mut d, device);
    d.app.run_command(&ctx, "nav.show_3d", None);
    d.frame();
    // It still switches and selects, and the message says why nothing shows.
    assert_eq!(d.app.workspace(), Workspace::View3d);
    let last = d.app.messages().last().expect("a message").text.clone();
    assert!(last.contains("Not placed") && last.contains("K1"), "{last}");
}

#[test]
fn show_in_3d_for_a_placed_device_asks_for_a_fit() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    let (device, _) = k1(&d);
    draw_selected(&mut d, device);
    d.app.run_command(&ctx, "nav.show_3d", None);
    assert!(
        matches!(d.app.view3d.pending, Some(super::view3d::Pending::Fit)),
        "the 3D view is asked to frame the device before its first frame"
    );
    assert!(
        d.app
            .messages()
            .last()
            .is_none_or(|m| !m.text.contains("Not placed")),
        "a placed device gets no warning"
    );
}
