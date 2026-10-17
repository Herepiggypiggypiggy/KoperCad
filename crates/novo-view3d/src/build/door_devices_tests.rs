//! Devices on the door in the scene: where they stand, that they swing with
//! the door, and the swing marks.

use super::*;
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::swing::DoorDevice;
use novo_layout::{Placement, Plate, Rect, Size};

fn enclosure() -> Enclosure {
    Enclosure {
        width: 420.0,
        height: 520.0,
        depth: 250.0,
        plate_setback: 20.0,
        door: Some(Door {
            hinge: Hinge::Left,
            ..Door::default()
        }),
        front_clearance: None,
        hinge_side_clearance: None,
    }
}

/// A 400 x 500 plate with plate device 5 and door device 8, the door device
/// at door x `door_x`.
fn layout(door_x: f64) -> CabinetLayout {
    let mut l = CabinetLayout::new(Plate {
        width: 400.0,
        height: 500.0,
        margin: 10.0,
    });
    l.enclosure = Some(enclosure());
    l.placements.push(Placement {
        id: 5,
        x: 30.0,
        y: 80.0,
        rotated: false,
        size: Size::from_part(Some(45.0), Some(90.0)),
        clearance: None,
        margins: Default::default(),
        rail: None,
    });
    l.placements.push(Placement {
        id: 8,
        x: door_x,
        y: 200.0,
        rotated: false,
        size: Size::from_part(Some(40.0), Some(40.0)),
        clearance: None,
        margins: Default::default(),
        rail: None,
    });
    l.off_plate.insert(8, "door".into());
    l.door_devices.push(DoorDevice {
        id: 8,
        rect: Rect {
            x: door_x,
            y: 200.0,
            w: 40.0,
            h: 40.0,
        },
        depth: 30.0,
        mesh: None,
    });
    l
}

fn scene(door_x: f64) -> Scene {
    let options = Options {
        shell: Shell {
            depth: 250.0,
            door: Some(DoorSpec {
                hinge: HingeSide::Left,
                open_deg: 0.0,
            }),
        },
        ..Options::default()
    };
    from_layout(&layout(door_x), &Inputs::default(), &options)
}

fn device(s: &Scene, raw: u64) -> Vec<&Item> {
    s.items
        .iter()
        .filter(|i| i.id == ItemId::new(ItemKind::Device, raw))
        .collect()
}

#[test]
fn a_door_device_stands_on_the_inside_of_the_door_and_not_on_the_plate() {
    let s = scene(100.0);
    let on_door = device(&s, 8);
    assert_eq!(on_door.len(), 1, "drawn once, on the door");
    let item = on_door[0];
    assert_eq!(item.group, Some(DOOR_HINGE), "it swings with the door");
    let b = s.world_bounds(item);
    // Door inside at 250 mm; the 30 mm body reaches back towards the plate.
    assert!(
        (b.max.z - 250.0).abs() < 1e-3 && (b.min.z - 220.0).abs() < 1e-3,
        "{b:?}"
    );
    assert!(
        (b.min.x - 100.0).abs() < 1e-3 && (b.max.x - 140.0).abs() < 1e-3,
        "{b:?}"
    );
    // Door y 200..240 from the top of a 500 mm plate: world 260..300.
    assert!(
        (b.min.y - 260.0).abs() < 1e-3 && (b.max.y - 300.0).abs() < 1e-3,
        "{b:?}"
    );
    assert_eq!(device(&s, 5).len(), 1, "the plate device is unchanged");
}

#[test]
fn a_door_device_does_not_count_as_reaching_the_door() {
    let s = scene(100.0);
    assert!(reaching_door(&s).iter().all(|(id, _)| *id != 8));
}

#[test]
fn a_swing_hit_marks_the_door_device_as_a_collision() {
    // Right edge 0.5 mm from the right wall of a 420 mm interior.
    let s = scene(379.5);
    assert_eq!(device(&s, 8)[0].mark, Mark::Collision);
    let clear = scene(100.0);
    assert_eq!(device(&clear, 8)[0].mark, Mark::None);
}
