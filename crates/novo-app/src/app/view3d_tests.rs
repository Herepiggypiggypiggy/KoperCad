//! The 3D workspace's toolbar and commands: layers, wires and the door.

use eframe::egui;
use novo_core::{CableCore, DeviceTag, MountingPlate, Point, RailKind, Wire, WireId};
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
fn drive_with_cabinet(dir: &std::path::Path) -> Drive {
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
    for _ in 0..400 {
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
