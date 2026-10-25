//! The move-device fixes: the nearest free spot on the plate that meets the
//! rule, refused when there is none.

use novo_core::*;
use novo_emc::project::{analyse_project, apply, preview};
use novo_emc::{Fix, Subject};

fn tag(t: &str) -> DeviceTag {
    DeviceTag::parse(t).unwrap()
}

struct Plate {
    project: Project,
    a: DeviceId,
    b: DeviceId,
}

/// A plate of `w` x `h` with two 60 x 80 devices `a` (role `ra`) and `b`
/// (role `rb`) at the given corners.
fn plate(w: f64, h: f64, ra: (&str, f64, f64), rb: (&str, f64, f64)) -> Plate {
    let mut project = Project::new("Move");
    let (a, b) = project
        .transact("build", |tx| {
            let cab = tx.add_structure(tag("+CAB"), "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: w,
                    height: h,
                    margin: 10.0,
                }),
            )?;
            let mut put = |t: &str, role: &str, x: f64, y: f64| -> Result<DeviceId> {
                let d = tx.add_device(tag(t), "")?;
                tx.set_property(d, "width_mm", Some(Value::Float(60.0)))?;
                tx.set_property(d, "height_mm", Some(Value::Float(80.0)))?;
                tx.set_property(d, "emc.role", Some(Value::Text(role.to_owned())))?;
                tx.place_device(d, cab, Point::new(x, y), false, None)?;
                Ok(d)
            };
            let a = put("-A1", ra.0, ra.1, ra.2)?;
            let b = put("-T1", rb.0, rb.1, rb.2)?;
            Ok((a, b))
        })
        .unwrap()
        .0;
    Plate { project, a, b }
}

fn spot(p: &Project, d: DeviceId) -> (f64, f64) {
    let pl = p.all::<CabinetPlacement>().find(|x| x.device == d).unwrap();
    (pl.x, pl.y)
}

fn has(p: &Project, rule: &str, a: DeviceId, b: DeviceId) -> bool {
    analyse_project(p).findings.iter().any(|f| {
        f.rule == rule
            && f.subjects.contains(&Subject::Device(a.raw()))
            && f.subjects.contains(&Subject::Device(b.raw()))
    })
}

#[test]
fn a_sensitive_device_moves_to_the_nearest_free_spot_away() {
    let mut c = plate(
        800.0,
        400.0,
        ("sensitive", 80.0, 100.0),
        ("drive", 10.0, 100.0),
    );
    assert!(has(&c.project, "EMC-17", c.a, c.b));
    let fix = Fix::MoveAway {
        device: c.a.raw(),
        from: c.b.raw(),
        min_mm: 100.0,
    };
    assert!(fix.applicable());
    // The preview works on a copy.
    let pv = preview(&c.project, &fix).unwrap();
    assert!(pv.cleared.iter().any(|(r, _)| *r == "EMC-17"));
    assert!(pv.added.is_empty(), "{:?}", pv.added);
    assert_eq!(spot(&c.project, c.a), (80.0, 100.0));
    // Apply moves it, no further than needed.
    c.project
        .transact("move", |tx| apply(tx, &fix).map(|_| ()))
        .unwrap();
    let (x, y) = spot(&c.project, c.a);
    assert!(!has(&c.project, "EMC-17", c.a, c.b));
    assert!(x >= 10.0 + 60.0 + 100.0 - 1e-6 || y != 100.0, "{x} {y}");
    assert!(x <= 80.0 + 130.0, "moved far: {x}");
    let layout =
        novo_layout::model::load(&c.project, c.project.structures().next().unwrap().id).unwrap();
    assert!(layout.report().is_clean());
    // One undo step.
    let mut doc = Document::new(c.project);
    let depth = doc.undo_depth();
    doc.transact("EMC fix", |tx| {
        apply(
            tx,
            &Fix::MoveAway {
                device: c.a.raw(),
                from: c.b.raw(),
                min_mm: 300.0,
            },
        )
        .map(|_| ())
    })
    .unwrap();
    assert_eq!(doc.undo_depth(), depth + 1);
    doc.undo().unwrap();
    assert_eq!(spot(doc.project(), c.a), (x, y));
}

#[test]
fn a_filter_moves_next_to_its_drive() {
    let mut c = plate(
        800.0,
        400.0,
        ("filter", 700.0, 100.0),
        ("drive", 10.0, 100.0),
    );
    assert!(has(&c.project, "EMC-07", c.b, c.a) || has(&c.project, "EMC-07", c.a, c.b));
    let fix = Fix::MoveCloser {
        device: c.a.raw(),
        to: c.b.raw(),
        max_mm: 300.0,
    };
    c.project
        .transact("move", |tx| apply(tx, &fix).map(|_| ()))
        .unwrap();
    let (x, _) = spot(&c.project, c.a);
    assert!(x < 700.0 && x - 70.0 <= 300.0 + 1e-6, "{x}");
    assert!(!has(&c.project, "EMC-07", c.b, c.a) && !has(&c.project, "EMC-07", c.a, c.b));
}

#[test]
fn no_free_spot_refuses_and_writes_nothing() {
    let mut c = plate(
        200.0,
        120.0,
        ("sensitive", 80.0, 20.0),
        ("drive", 10.0, 20.0),
    );
    let fix = Fix::MoveAway {
        device: c.a.raw(),
        from: c.b.raw(),
        min_mm: 100.0,
    };
    let err = c
        .project
        .transact("move", |tx| apply(tx, &fix).map(|_| ()))
        .unwrap_err();
    assert!(err.to_string().contains("No free spot"), "{err}");
    assert_eq!(spot(&c.project, c.a), (80.0, 20.0));
    assert!(preview(&c.project, &fix).is_err());
}
