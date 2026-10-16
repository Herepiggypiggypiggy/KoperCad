use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::surfaces::Surfaces;
use novo_layout::swing::*;
use novo_layout::{Plate, Rect};

fn enclosure(hinge: Hinge) -> Enclosure {
    Enclosure {
        width: 600.0,
        height: 800.0,
        depth: 250.0,
        plate_setback: 20.0,
        door: Some(Door {
            hinge,
            ..Door::default()
        }),
        front_clearance: None,
        hinge_side_clearance: None,
    }
}

fn plate() -> Plate {
    Plate {
        width: 560.0,
        height: 760.0,
        margin: 10.0,
    }
}

fn surfaces(e: &Enclosure) -> Surfaces {
    Surfaces::from_enclosure(e, &plate(), &[]).0
}

fn door_dev(id: u64, x: f64, y: f64, w: f64, h: f64, depth: f64) -> DoorDevice {
    DoorDevice {
        id,
        rect: Rect { x, y, w, h },
        depth,
        mesh: None,
    }
}

fn tag(id: u64) -> String {
    format!("-X{id}")
}

#[test]
fn a_door_device_in_the_open_never_touches() {
    let e = enclosure(Hinge::Left);
    let r = sweep(
        &e,
        &surfaces(&e),
        &[door_dev(1, 250.0, 300.0, 40.0, 40.0, 40.0)],
        &[],
        &SweepSettings::default(),
    );
    assert!(r.is_clean());
    assert_eq!(r.max_angle_deg, 90.0);
}

#[test]
fn a_deep_plate_device_collides_with_the_closed_door() {
    let e = enclosure(Hinge::Left);
    let s = surfaces(&e);
    // Plate surface is at z = 20; 200 deep reaches z = 220, the door device starts at 210.
    let plate_dev = PlateDevice {
        id: 7,
        surface: "plate".into(),
        rect: Rect {
            x: 250.0,
            y: 300.0,
            w: 50.0,
            h: 50.0,
        },
        depth: 200.0,
        mesh: None,
    };
    // The plate surface starts at x = 20, y = 20, so plate x 250 is cabinet x 270.
    let dd = door_dev(1, 270.0, 320.0, 30.0, 30.0, 40.0);
    let r = sweep(&e, &s, &[dd], &[plate_dev], &SweepSettings::default());
    assert_eq!(r.hits.len(), 1);
    let h = &r.hits[0];
    assert_eq!(h.obstacle, Obstacle::Device(7));
    assert_eq!(h.angle_deg, 0.0);
    assert_eq!(h.rule(), RULE_DOOR_CLOSED);
    assert!(h.approximate);
    assert!(h.message(&tag).contains("with the door closed"));
}

#[test]
fn a_device_on_the_strike_side_catches_the_wall_at_the_analytic_angle() {
    // Hinge left, pivot at x = 0. The inner bottom corner (x, depth - d)
    // moves to x cos a + d sin a, which passes the right wall at x = 600.
    let e = enclosure(Hinge::Left);
    let (x, d) = (599.5_f64, 40.0_f64);
    let dd = door_dev(1, 560.0, 300.0, x - 560.0, 40.0, d);
    let r = sweep(&e, &surfaces(&e), &[dd], &[], &SweepSettings::default());
    assert_eq!(r.hits.len(), 1);
    assert_eq!(r.hits[0].obstacle, Obstacle::Wall("right".into()));
    let rr = x.hypot(d);
    let expect = (d.atan2(x) - (600.0 / rr).acos()).to_degrees();
    assert!(
        (r.hits[0].angle_deg - expect).abs() < 0.05,
        "got {} expected {expect}",
        r.hits[0].angle_deg
    );
    assert_eq!(r.hits[0].rule(), RULE_DOOR_SWING);
}

#[test]
fn a_right_hinge_mirrors_the_result() {
    let left = enclosure(Hinge::Left);
    let right = enclosure(Hinge::Right);
    let a = sweep(
        &left,
        &surfaces(&left),
        &[door_dev(1, 560.0, 300.0, 39.5, 40.0, 40.0)],
        &[],
        &SweepSettings::default(),
    );
    let b = sweep(
        &right,
        &surfaces(&right),
        &[door_dev(1, 0.5, 300.0, 39.5, 40.0, 40.0)],
        &[],
        &SweepSettings::default(),
    );
    assert_eq!(b.hits.len(), 1);
    assert_eq!(b.hits[0].obstacle, Obstacle::Wall("left".into()));
    assert!((a.hits[0].angle_deg - b.hits[0].angle_deg).abs() < 0.02);
}

#[test]
fn no_door_or_no_depth_sweeps_nothing() {
    let mut e = enclosure(Hinge::Left);
    let s = surfaces(&e);
    let r = sweep(
        &e,
        &s,
        &[door_dev(1, 10.0, 10.0, 10.0, 10.0, 0.0)],
        &[],
        &SweepSettings::default(),
    );
    assert_eq!(r.unknown_depth, vec![1]);
    assert!(r.is_clean());
    e.door = None;
    let s = surfaces(&e);
    let r = sweep(
        &e,
        &s,
        &[door_dev(1, 599.0, 10.0, 1.0, 10.0, 40.0)],
        &[],
        &SweepSettings::default(),
    );
    assert!(r.is_clean() && r.max_angle_deg == 0.0);
}

#[test]
fn hits_are_sorted_by_angle() {
    let e = enclosure(Hinge::Left);
    let devs = [
        door_dev(2, 560.0, 300.0, 39.9, 40.0, 40.0),
        door_dev(1, 560.0, 400.0, 39.5, 40.0, 40.0),
    ];
    let r = sweep(&e, &surfaces(&e), &devs, &[], &SweepSettings::default());
    assert_eq!(r.hits.len(), 2);
    assert!(r.hits[0].angle_deg <= r.hits[1].angle_deg);
}
