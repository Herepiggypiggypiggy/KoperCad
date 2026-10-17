//! Devices on walls, roof, floor and extra plates in the 3D view: scene
//! items that can be picked and dragged on their own surface.

use glam::Vec3;
use novo_core::{AnyId, CabinetPlacement, Value};
use novo_layout::state::CabinetLayout;
use novo_layout::surfaces::SURFACE_PROPERTY;
use novo_layout::{Margins, Placement, Plate, Size};
use novo_view3d::build::Inputs;
use novo_view3d::scene::{Item, ItemId, ItemKind, PlateFrame, Scene};
use novo_view3d::widget::ItemDrag;
use novo_view3d::{Overlay, Ray, ViewState};

use super::cabinet_surfaces::{SurfaceRow, SurfaceSummary};
use super::view3d_surface_devices::{ghost, landing, push};
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

const FRAME: PlateFrame = PlateFrame {
    width: 600.0,
    height: 800.0,
};

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
    (summary, layout)
}

fn scene_of(summary: &SurfaceSummary, layout: &CabinetLayout) -> Scene {
    let mut scene = Scene::new(FRAME);
    push(&mut scene, summary, layout, &Inputs::default());
    scene
}

fn device(scene: &Scene, id: u64) -> Option<&Item> {
    scene.get(ItemId::new(ItemKind::Device, id))
}

fn drag(id: u64, delta: (f64, f64), done: bool) -> ItemDrag {
    ItemDrag {
        id: ItemId::new(ItemKind::Device, id),
        delta,
        done,
    }
}

#[test]
fn only_devices_on_walls_roof_floor_and_extra_plates_become_items_here() {
    let (summary, layout) = setup();
    let scene = scene_of(&summary, &layout);
    assert_eq!(scene.items.len(), 1);
    assert!(device(&scene, 3).is_some());
    assert!(device(&scene, 1).is_none() && device(&scene, 2).is_none());
}

#[test]
fn a_roof_device_hangs_from_the_roof_into_the_enclosure() {
    let (summary, layout) = setup();
    let scene = scene_of(&summary, &layout);
    let b = scene.world_bounds(device(&scene, 3).unwrap());
    // x 100..140 of the enclosure, 100 mm left of the plate corner.
    assert!(
        (b.min.x - 0.0).abs() < 0.01 && (b.max.x - 40.0).abs() < 0.01,
        "{b:?}"
    );
    // Along the roof's depth: 50..80 mm from the front edge, 20 mm behind
    // the plate face at z = 20.
    assert!(
        (b.min.z - 30.0).abs() < 0.01 && (b.max.z - 60.0).abs() < 0.01,
        "{b:?}"
    );
    // The default depth hangs down from the roof, 50 mm above the plate's
    // top edge: world y 850 down to 850 - 60.
    assert!(
        (b.max.y - 850.0).abs() < 0.01 && (b.min.y - 790.0).abs() < 0.01,
        "{b:?}"
    );
}

#[test]
fn the_item_carries_its_surface_as_the_drag_plane() {
    let (summary, layout) = setup();
    let scene = scene_of(&summary, &layout);
    let plane = device(&scene, 3).unwrap().drag_plane.expect("a drag plane");
    assert_eq!(plane.origin, Vec3::new(-100.0, 850.0, -20.0));
    assert_eq!(plane.u, Vec3::X);
    // The surface's y runs back into the cabinet (+z), the enclosure frame's
    // y-down flipped for the world.
    assert_eq!(plane.v, Vec3::Z);
    // A ray from above lands on the device's footprint corner in surface
    // coordinates.
    let ray = Ray {
        origin: Vec3::new(0.0, 1500.0, 30.0),
        dir: Vec3::NEG_Y,
    };
    let (x, y) = plane.point(&ray).unwrap();
    assert!(
        (x - 100.0).abs() < 0.01 && (y - 50.0).abs() < 0.01,
        "{x} {y}"
    );
}

#[test]
fn a_roof_device_can_be_picked_from_below() {
    let (summary, layout) = setup();
    let scene = scene_of(&summary, &layout);
    // Looking up from below the roof at the middle of the device.
    let ray = Ray {
        origin: Vec3::new(20.0, 100.0, 45.0),
        dir: Vec3::Y,
    };
    let hit = novo_view3d::pick::pick(&scene, &ViewState::default(), &ray, 0).expect("a hit");
    assert_eq!(hit.id, ItemId::new(ItemKind::Device, 3));
}

#[test]
fn a_drag_moves_the_corner_on_the_surface_and_is_kept_inside_it() {
    let (summary, layout) = setup();
    let inputs = Inputs::default();
    let land = landing(&summary, &layout, &inputs, &drag(3, (20.4, -5.0), false)).unwrap();
    assert_eq!(land.surface, "roof");
    assert_eq!(land.at, (120.0, 45.0), "rounded to whole millimetres");
    assert!(!land.done);
    // Far past the far edge: the device stays inside the usable area
    // (800 x 300 with a 10 mm margin, the device 40 x 30).
    let land = landing(&summary, &layout, &inputs, &drag(3, (5000.0, 5000.0), true)).unwrap();
    assert_eq!(land.at, (750.0, 260.0));
    assert!(land.done);
    let land = landing(
        &summary,
        &layout,
        &inputs,
        &drag(3, (-5000.0, -5000.0), false),
    )
    .unwrap();
    assert_eq!(land.at, (10.0, 10.0));
}

#[test]
fn plate_and_door_devices_are_not_dragged_here() {
    let (summary, layout) = setup();
    let inputs = Inputs::default();
    assert!(landing(&summary, &layout, &inputs, &drag(1, (5.0, 5.0), false)).is_none());
    assert!(landing(&summary, &layout, &inputs, &drag(2, (5.0, 5.0), false)).is_none());
    assert!(landing(&summary, &layout, &inputs, &drag(99, (5.0, 5.0), false)).is_none());
}

#[test]
fn the_dragged_device_shows_a_ghost_box_at_the_landing() {
    let (summary, layout) = setup();
    let land = landing(
        &summary,
        &layout,
        &Inputs::default(),
        &drag(3, (0.0, 0.0), false),
    )
    .unwrap();
    let all = ghost(&summary, &land, FRAME);
    assert_eq!(all.len(), 2);
    let Overlay::Mesh(m) = &all[0] else { panic!() };
    assert!(m.is_valid() && m.translucent);
    let Overlay::Lines(l) = &all[1] else { panic!() };
    assert!(l.is_valid());
}

#[test]
fn a_roof_device_of_the_real_cabinet_is_a_scene_item_with_a_drag_plane() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let pid = {
        let project = d.app.backend.document().unwrap().project();
        let cab = project.structures().find(|s| s.enclosure).unwrap().id;
        let p = project
            .all::<CabinetPlacement>()
            .find(|p| p.structure == cab)
            .unwrap();
        (p.id, p.device.raw())
    };
    let in_scene = |d: &super::gate1_tests::Drive| {
        d.app
            .view3d
            .built
            .as_ref()
            .and_then(|(_, _, s)| s.get(ItemId::new(ItemKind::Device, pid.1)).cloned())
    };
    assert!(
        in_scene(&d).is_some_and(|i| i.drag_plane.is_none()),
        "on the plate"
    );
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Surface", |tx| {
            tx.set_property(
                AnyId::CabinetPlacement(pid.0),
                SURFACE_PROPERTY,
                Some(Value::Text("roof".into())),
            )
        })
        .unwrap();
    d.frame();
    d.frame();
    let item = in_scene(&d).expect("still in the scene");
    assert!(item.drag_plane.is_some(), "now on the roof's plane");
}

#[test]
fn a_device_on_every_surface_stands_into_the_enclosure_from_its_pane() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    d.frame();
    let (cab, pid) = {
        let project = d.app.backend.document().unwrap().project();
        let cab = project.structures().find(|s| s.enclosure).unwrap().id;
        let p = project
            .all::<CabinetPlacement>()
            .find(|p| p.structure == cab)
            .unwrap();
        (cab, (p.id, p.device.raw()))
    };
    for name in ["left", "right", "roof", "floor"] {
        d.app
            .backend
            .document_mut()
            .unwrap()
            .transact("Surface", |tx| {
                tx.set_property(
                    AnyId::CabinetPlacement(pid.0),
                    SURFACE_PROPERTY,
                    Some(Value::Text(name.into())),
                )
            })
            .unwrap();
        let project = d.app.backend.document().unwrap().project();
        let summary = super::cabinet_surfaces::summarize(project, cab).unwrap();
        let stored = novo_layout::model::load(project, cab).unwrap();
        let mut scene = Scene::new(FRAME);
        push(&mut scene, &summary, &stored, &Inputs::default());
        let item = device(&scene, pid.1).unwrap_or_else(|| panic!("{name}: no item"));
        let b = scene.world_bounds(item);
        let ext = b.max - b.min;
        let row = summary.row(name).unwrap();
        // The extent along the surface's normal is the depth; the footprint
        // fills the other two directions.
        let n = Vec3::new(
            row.normal[0] as f32,
            -row.normal[1] as f32,
            row.normal[2] as f32,
        );
        let along = (ext * n.abs()).length();
        assert!(
            (along - 60.0).abs() < 0.01,
            "{name}: depth {along} in {ext:?}"
        );
        // The pane is at the box's back face, the box on the inside of it.
        let plate = summary.row("plate").unwrap();
        let corner = |p: [f64; 3]| {
            FRAME.world(
                p[0] - plate.origin[0],
                p[1] - plate.origin[1],
                p[2] - plate.origin[2],
            )
        };
        let pane_point = corner(row.origin);
        let toward = (b.min + b.max) / 2.0 - pane_point;
        assert!(toward.dot(n) > 0.0, "{name}: the box is on the inside");
    }
}
