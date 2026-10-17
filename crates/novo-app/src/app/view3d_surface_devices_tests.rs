//! Devices on walls, roof, floor and extra plates in the 3D view.

use glam::Vec3;
use novo_core::{AnyId, CabinetPlacement, Value};
use novo_layout::state::CabinetLayout;
use novo_layout::surfaces::SURFACE_PROPERTY;
use novo_layout::{Margins, Placement, Plate, Size};
use novo_view3d::{Overlay, PlateFrame};

use super::cabinet_surfaces::{SurfaceRow, SurfaceSummary};
use super::view3d_surface_devices::overlays;
use super::view3d_tests::drive_with_cabinet;

fn row(name: &str, origin: [f64; 3], v: [f64; 3], normal: [f64; 3]) -> SurfaceRow {
    SurfaceRow {
        name: name.into(),
        kind: "x",
        size: String::new(),
        devices: 0,
        assumed: false,
        plate: Plate {
            width: 800.0,
            height: 300.0,
            margin: 10.0,
        },
        origin,
        u: [1.0, 0.0, 0.0],
        v,
        normal,
    }
}

fn placement(id: u64, x: f64, y: f64) -> Placement {
    Placement {
        id,
        x,
        y,
        rotated: false,
        size: Size {
            width: 40.0,
            height: 30.0,
            guessed: false,
        },
        clearance: None,
        margins: Margins::default(),
        rail: None,
    }
}

/// Plate at (100, 50, 20); the roof along the top, reaching back in z, its
/// inside facing down (+y in the y-down enclosure frame).
fn setup() -> (SurfaceSummary, CabinetLayout) {
    let mut summary = SurfaceSummary {
        rows: vec![
            row("plate", [100.0, 50.0, 20.0], [0., 1., 0.], [0., 0., 1.]),
            row("door", [0., 0., 250.], [0., 1., 0.], [0., 0., -1.]),
            row("roof", [0., 0., 0.], [0., 0., 1.], [0., 1., 0.]),
        ],
        ..SurfaceSummary::default()
    };
    summary.assigned.insert(1, "plate".into());
    summary.assigned.insert(2, "door".into());
    summary.assigned.insert(3, "roof".into());
    let mut layout = CabinetLayout::new(Plate {
        width: 600.0,
        height: 800.0,
        margin: 10.0,
    });
    layout.placements = vec![
        placement(1, 10.0, 10.0),
        placement(2, 20.0, 20.0),
        placement(3, 100.0, 50.0),
    ];
    layout.depths.insert(3, 80.0);
    (summary, layout)
}

#[test]
fn only_devices_on_walls_roof_floor_and_extra_plates_are_drawn_here() {
    let (summary, layout) = setup();
    let frame = PlateFrame {
        width: 600.0,
        height: 800.0,
    };
    let all = overlays(&summary, &layout, None, frame);
    assert_eq!(all.len(), 2, "one body mesh and its outline");
    let Overlay::Mesh(m) = &all[0] else { panic!() };
    assert!(m.is_valid());
    assert_eq!(m.positions.len(), 8, "the roof device only, as one box");
    let Overlay::Lines(l) = &all[1] else { panic!() };
    assert!(l.is_valid());
    assert_eq!(l.starts.len(), 6);
}

#[test]
fn a_roof_device_hangs_from_the_roof_into_the_enclosure() {
    let (summary, layout) = setup();
    let frame = PlateFrame {
        width: 600.0,
        height: 800.0,
    };
    let Overlay::Mesh(m) = &overlays(&summary, &layout, None, frame)[0] else {
        panic!()
    };
    let lo = m
        .positions
        .iter()
        .copied()
        .fold(Vec3::splat(f32::MAX), Vec3::min);
    let hi = m
        .positions
        .iter()
        .copied()
        .fold(Vec3::splat(f32::MIN), Vec3::max);
    // x 100..140 relative to the enclosure, 100 mm left of the plate corner.
    assert_eq!((lo.x, hi.x), (0.0, 40.0));
    // Along the roof's depth the device sits 50..80 mm back from the front
    // edge, 20 mm behind the plate face at z = 20.
    assert_eq!((lo.z, hi.z), (30.0, 60.0));
    // 80 mm down from the roof, which is 50 mm above the plate's top edge.
    assert_eq!((lo.y, hi.y), (770.0, 850.0));
}

#[test]
fn nothing_is_drawn_when_every_device_is_on_the_plate_or_the_door() {
    let (summary, mut layout) = setup();
    layout.placements.truncate(2);
    let frame = PlateFrame {
        width: 600.0,
        height: 800.0,
    };
    assert!(overlays(&summary, &layout, None, frame).is_empty());
}

#[test]
fn the_picked_surface_draws_its_devices_stronger() {
    let (summary, layout) = setup();
    let frame = PlateFrame {
        width: 600.0,
        height: 800.0,
    };
    let colour = |picked| {
        let Overlay::Mesh(m) = &overlays(&summary, &layout, picked, frame)[0] else {
            panic!()
        };
        m.colors[0]
    };
    assert_ne!(colour(Some("roof")), colour(None));
    assert_eq!(colour(Some("door")), colour(None));
}

#[test]
fn a_roof_device_of_the_real_cabinet_reaches_the_3d_view_with_the_panes() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let (pid, cab) = {
        let project = d.app.backend.document().unwrap().project();
        let cab = project.structures().find(|s| s.enclosure).unwrap().id;
        let pid = project
            .all::<CabinetPlacement>()
            .find(|p| p.structure == cab)
            .unwrap()
            .id;
        (pid, cab)
    };
    d.app.cabinet.surface = Some("roof".to_owned());
    d.frame();
    d.frame();
    let before = d.app.view3d.surface_panes.len();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Surface", |tx| {
            tx.set_property(
                AnyId::CabinetPlacement(pid),
                SURFACE_PROPERTY,
                Some(Value::Text("roof".into())),
            )
        })
        .unwrap();
    d.frame();
    d.frame();
    assert_eq!(
        d.app.view3d.surface_panes.len(),
        before + 2,
        "the device body and its outline join the panes"
    );
    let _ = cab;
}
