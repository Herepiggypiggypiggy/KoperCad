//! Terminal markers in the 3D view: where they land, colour, turned devices.

use glam::{Quat, Vec3};
use novo_core::{DeviceTag, Project, Value};
use novo_view3d::scene::{Item, ItemId, ItemKind, PlateFrame, Pose, Shape};
use novo_view3d::{Overlay, Scene};

use super::view3d_pins::overlays;

/// A project with one device on part "ACME C1" with terminals A (5/10/60)
/// and B (40/80/60) of a 45 x 90 x 70 mm part, and a scene with its box.
fn fixture(estimated: bool, turned: bool) -> (Project, Scene) {
    let mut p = Project::new("t");
    let (device, _) = p
        .transact("setup", |tx| {
            let part = tx.add_part("ACME", "C1")?;
            for (name, at) in [("A", [5.0, 10.0, 60.0]), ("B", [40.0, 80.0, 60.0])] {
                for (axis, v) in ["x", "y", "z"].iter().zip(at) {
                    tx.set_property(part, format!("pin.{name}.{axis}_mm"), Some(Value::Float(v)))?;
                }
            }
            if estimated {
                tx.set_property(
                    part,
                    "pin_positions_source",
                    Some(Value::Text("estimated".into())),
                )?;
            }
            let d = tx.add_device(DeviceTag::parse("=A1+CAB1-K1").unwrap(), "")?;
            tx.assign_part(d, part)?;
            Ok(d)
        })
        .unwrap();
    let mut scene = Scene::new(PlateFrame {
        width: 400.0,
        height: 400.0,
    });
    scene.push(Item::new(
        ItemId::new(ItemKind::Device, device.raw()),
        Shape::Box,
        Pose {
            translation: Vec3::new(100.0, 200.0, 0.0),
            rotation: if turned {
                Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)
            } else {
                Quat::IDENTITY
            },
        },
        Vec3::new(45.0, 90.0, 70.0),
    ));
    (p, scene)
}

fn centre_of(overlay: &Overlay, marker: usize) -> Vec3 {
    let Overlay::Mesh(m) = overlay else {
        panic!("a mesh")
    };
    let v = &m.positions[marker * 6..marker * 6 + 6];
    v.iter().copied().sum::<Vec3>() / 6.0
}

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-3
}

#[test]
fn a_marker_sits_on_each_terminal_in_the_scene_frame() {
    let (p, scene) = fixture(false, false);
    let o = overlays(&p, &scene);
    assert_eq!(o.len(), 1);
    // Local = (x - 22.5, y - 45, z) from the box origin at (100, 200, 0).
    let pts = [centre_of(&o[0], 0), centre_of(&o[0], 1)];
    assert!(near(pts[0], Vec3::new(82.5, 165.0, 60.0)), "{:?}", pts[0]);
    assert!(near(pts[1], Vec3::new(117.5, 235.0, 60.0)), "{:?}", pts[1]);
    let Overlay::Mesh(m) = &o[0] else { panic!() };
    assert!(m.is_valid());
}

#[test]
fn a_turned_device_carries_its_markers_with_it() {
    let (p, scene) = fixture(false, true);
    let o = overlays(&p, &scene);
    // Clockwise quarter turn: local (dx, dy) goes to (dy, -dx).
    let a = centre_of(&o[0], 0);
    assert!(
        near(a, Vec3::new(100.0 - 35.0, 200.0 + 17.5, 60.0)),
        "{a:?}"
    );
}

#[test]
fn estimated_points_are_amber_and_a_part_without_points_draws_nothing() {
    let (p, scene) = fixture(true, false);
    let Overlay::Mesh(m) = &overlays(&p, &scene)[0] else {
        panic!()
    };
    assert_eq!(m.colors[0], [235, 170, 60, 255]);
    let (p, scene) = fixture(false, false);
    let Overlay::Mesh(m) = &overlays(&p, &scene)[0] else {
        panic!()
    };
    assert_eq!(m.colors[0], [90, 200, 120, 255]);
    let empty = Project::new("e");
    assert!(overlays(&empty, &scene).is_empty());
}
