use novo_layout::collide::collide;
use novo_layout::enclosure::{Door, Enclosure};
use novo_layout::surfaces::*;
use novo_layout::{Plate, Rect};

fn enclosure() -> Enclosure {
    Enclosure {
        width: 600.0,
        height: 800.0,
        depth: 250.0,
        plate_setback: 20.0,
        door: Some(Door::default()),
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

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
}

#[test]
fn standard_surfaces_follow_the_enclosure() {
    let (s, problems) = Surfaces::from_enclosure(&enclosure(), &plate(), &[]);
    assert!(problems.is_empty());
    let names: Vec<_> = s.list.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["plate", "door", "left", "right", "roof", "floor"]);
    let p = s.find(None).unwrap();
    assert!(near(p.origin, [20.0, 20.0, 20.0]));
    assert!(near(p.point(0.0, 0.0, 30.0), [20.0, 20.0, 50.0]));
    let d = s.find(Some("DOOR")).unwrap();
    assert!(near(d.point(0.0, 0.0, 15.0), [0.0, 0.0, 235.0]));
    assert!((d.width, d.height) == (600.0, 800.0));
    let l = s.find(Some("left")).unwrap();
    assert_eq!((l.width, l.height), (250.0, 800.0));
    assert!(near(l.point(250.0, 800.0, 0.0), [0.0, 800.0, 250.0]));
    let r = s.find(Some("right")).unwrap();
    assert!(near(r.point(0.0, 0.0, 10.0), [590.0, 0.0, 0.0]));
    let f = s.find(Some("floor")).unwrap();
    assert!(near(f.point(0.0, 0.0, 10.0), [0.0, 790.0, 0.0]));
}

#[test]
fn no_door_means_no_door_surface() {
    let mut e = enclosure();
    e.door = None;
    let (s, _) = Surfaces::from_enclosure(&e, &plate(), &[]);
    assert!(s.find(Some("door")).is_none());
    assert_eq!(s.of_kind(SurfaceKind::Door).count(), 0);
}

#[test]
fn names_and_sizes_parse_and_round_trip() {
    assert_eq!(SurfaceKind::from_name("Plate2"), Some(SurfaceKind::Plate));
    assert_eq!(
        SurfaceKind::from_name("profile3"),
        Some(SurfaceKind::Profile)
    );
    assert_eq!(SurfaceKind::from_name("door2"), None);
    assert_eq!(SurfaceKind::from_name("wall"), None);
    let s = SurfaceSize::parse("300 400 5").unwrap();
    assert_eq!(SurfaceSize::parse(&s.format()), Some(s));
    let o = SurfaceSize::parse("300,5 400 5 10 20 30").unwrap();
    assert_eq!(o.width, 300.5);
    assert_eq!(o.origin, Some([10.0, 20.0, 30.0]));
    assert_eq!(SurfaceSize::parse(&o.format()), Some(o));
    for bad in ["", "300 400", "0 400 5", "300 400 -1", "a b c", "1 2 3 4"] {
        assert!(SurfaceSize::parse(bad).is_none(), "{bad}");
    }
    assert_eq!(size_key(" Plate2 "), "surface.plate2");
}

#[test]
fn extra_plates_profiles_and_problems() {
    let extra = [
        ("plate2".to_string(), "200 700 10".to_string()),
        ("profile1".to_string(), "40 600 0 100 50 30".to_string()),
        ("profile2".to_string(), "40 600 0".to_string()),
        ("shelf".to_string(), "40 600 0".to_string()),
        ("roof".to_string(), "oops".to_string()),
        ("door".to_string(), "500 780 8".to_string()),
    ];
    let (s, problems) = Surfaces::from_enclosure(&enclosure(), &plate(), &extra);
    let p2 = s.find(Some("plate2")).unwrap();
    assert!(p2.assumed);
    assert!(near(p2.origin, [580.0, 20.0, 20.0]));
    assert_eq!(s.find(Some("profile1")).unwrap().kind, SurfaceKind::Profile);
    assert!(s.find(Some("profile2")).is_none());
    // A standard surface takes the new size.
    let door = s.find(Some("door")).unwrap();
    assert_eq!((door.width, door.height, door.margin), (500.0, 780.0, 8.0));
    assert_eq!(problems.len(), 3);
    assert!(problems.contains(&SurfaceProblem::ProfileNeedsOrigin("profile2".into())));
    assert!(problems.contains(&SurfaceProblem::UnknownName("shelf".into())));
    assert!(problems.contains(&SurfaceProblem::BadSize("roof".into())));
    assert!(problems.iter().all(|p| p.message().contains('\'')));
}

#[test]
fn devices_stand_out_along_the_normal() {
    let (s, _) = Surfaces::from_enclosure(&enclosure(), &plate(), &[]);
    let rect = Rect {
        x: 10.0,
        y: 10.0,
        w: 50.0,
        h: 90.0,
    };
    let on_plate = s.find(None).unwrap().device_body(&rect, 100.0, None);
    assert!(on_plate.is_fallback());
    let b = on_plate.bounds();
    assert!(near(b.min, [30.0, 30.0, 20.0]) && near(b.max, [80.0, 120.0, 120.0]));
    // Same footprint on the door stands out towards the plate.
    let on_door = s.find(Some("door")).unwrap().device_body(&rect, 40.0, None);
    let d = on_door.bounds();
    assert!(near(d.min, [10.0, 10.0, 210.0]) && near(d.max, [60.0, 100.0, 250.0]));
}

#[test]
fn a_deep_device_collides_with_the_door_mounted_one() {
    let (s, _) = Surfaces::from_enclosure(&enclosure(), &plate(), &[]);
    let at_plate = Rect {
        x: 100.0,
        y: 100.0,
        w: 50.0,
        h: 90.0,
    };
    // Same spot seen from the front: plate x offset 20, door x 0.
    let on_door_rect = Rect {
        x: 120.0,
        y: 120.0,
        w: 50.0,
        h: 90.0,
    };
    let door_dev = s
        .find(Some("door"))
        .unwrap()
        .device_body(&on_door_rect, 40.0, None);
    let shallow = s.find(None).unwrap().device_body(&at_plate, 150.0, None);
    let deep = s.find(None).unwrap().device_body(&at_plate, 215.0, None);
    assert!(collide(&shallow, &door_dev).is_none());
    assert!(collide(&deep, &door_dev).is_some());
}

#[test]
fn the_slab_sits_outside_the_interior() {
    let (s, _) = Surfaces::from_enclosure(&enclosure(), &plate(), &[]);
    let slab = s.find(Some("left")).unwrap().slab(2.0);
    let b = slab.bounds();
    assert!(b.max[0].abs() < 1e-9 && (b.min[0] + 2.0).abs() < 1e-9);
}
