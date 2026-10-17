//! Typing a rail or duct's values in the 3D view: what a value would run
//! into, and the edit going through the Cabinet's own path.

use novo_core::RailKind;
use novo_layout::state::CabinetLayout;
use novo_layout::{Duct, Margins, Placement, Plate, Rail, Size};

use super::cabinet_rails::RailEdit;
use super::view3d_rail::problems;

fn placement(id: u64, x: f64, y: f64, rail: Option<u64>) -> Placement {
    Placement {
        id,
        x,
        y,
        rotated: false,
        size: Size {
            width: 45.0,
            height: 90.0,
            guessed: false,
        },
        clearance: None,
        margins: Margins::default(),
        rail,
    }
}

fn layout() -> CabinetLayout {
    let mut l = CabinetLayout::new(Plate {
        width: 600.0,
        height: 800.0,
        margin: 10.0,
    });
    l.placements = vec![
        placement(1, 60.0, 380.0, Some(7)),
        placement(2, 300.0, 100.0, None),
    ];
    l.rails = vec![Rail {
        id: 7,
        x: 50.0,
        y: 400.0,
        length: 300.0,
        vertical: false,
    }];
    l.ducts = vec![Duct {
        id: 8,
        x: 50.0,
        y: 600.0,
        length: 400.0,
        width: 40.0,
        vertical: false,
    }];
    l
}

fn rail(x: f64, y: f64, length: f64) -> RailEdit {
    RailEdit {
        id: 7,
        x,
        y,
        length,
        width: 0.0,
        height: 0.0,
    }
}

fn duct(x: f64, y: f64, length: f64, width: f64) -> RailEdit {
    RailEdit {
        id: 8,
        x,
        y,
        length,
        width,
        height: 40.0,
    }
}

fn tag(id: u64) -> String {
    format!("-K{id}")
}

#[test]
fn values_that_change_nothing_are_clean() {
    let l = layout();
    assert!(
        problems(
            &l,
            &rail(50.0, 400.0, 300.0),
            RailKind::DinRail,
            false,
            &tag
        )
        .is_empty()
    );
    assert!(
        problems(
            &l,
            &duct(50.0, 600.0, 400.0, 40.0),
            RailKind::Duct,
            false,
            &tag
        )
        .is_empty()
    );
}

#[test]
fn a_rail_too_short_for_its_devices_says_how_much_they_need() {
    let l = layout();
    let out = problems(&l, &rail(50.0, 400.0, 30.0), RailKind::DinRail, false, &tag);
    assert!(
        out.iter().any(|s| s.starts_with("Its devices need 45 mm")),
        "{out:?}"
    );
}

#[test]
fn moving_a_rail_moves_its_devices_in_the_check() {
    let l = layout();
    // Down 200 mm the rider lands on nothing; its old place is not a blocker.
    assert!(
        problems(
            &l,
            &rail(50.0, 600.0, 300.0),
            RailKind::DinRail,
            false,
            &tag
        )
        .iter()
        .all(|s| !s.contains("run into"))
    );
    // Up under device 2 (at y 100..190, x 300..345): the rail runs into it.
    let out = problems(
        &l,
        &rail(250.0, 130.0, 300.0),
        RailKind::DinRail,
        false,
        &tag,
    );
    assert!(out.iter().any(|s| s == "It would run into -K2"), "{out:?}");
}

#[test]
fn leaving_the_plate_is_named() {
    let l = layout();
    let out = problems(
        &l,
        &duct(50.0, 600.0, 900.0, 40.0),
        RailKind::Duct,
        false,
        &tag,
    );
    assert!(out.iter().any(|s| s.contains("leave the plate")), "{out:?}");
}

#[test]
fn a_wider_duct_that_reaches_a_device_names_it() {
    let l = layout();
    // Centre line y 600, device 1 would not be near; widen over a far device.
    let mut l2 = l.clone();
    l2.placements.push(placement(3, 100.0, 630.0, None));
    let out = problems(
        &l2,
        &duct(50.0, 600.0, 400.0, 80.0),
        RailKind::Duct,
        false,
        &tag,
    );
    assert!(out.iter().any(|s| s == "It would run into -K3"), "{out:?}");
}

#[test]
fn a_rider_pushed_onto_another_device_is_reported() {
    let mut l = layout();
    l.placements.push(placement(4, 60.0, 580.0, None));
    let out = problems(
        &l,
        &rail(50.0, 600.0, 300.0),
        RailKind::DinRail,
        false,
        &tag,
    );
    assert!(
        out.iter().any(|s| s.contains("device on it would collide")),
        "{out:?}"
    );
}
