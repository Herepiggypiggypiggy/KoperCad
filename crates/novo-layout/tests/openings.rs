use novo_layout::openings::*;
use novo_layout::{Margins, Placement, Size};

fn dev(id: u64, x: f64, y: f64, w: f64, h: f64) -> Placement {
    Placement {
        id,
        x,
        y,
        rotated: false,
        size: Size {
            width: w,
            height: h,
            guessed: false,
        },
        clearance: None,
        margins: Margins::default(),
        rail: None,
    }
}

#[test]
fn round_and_rect_openings_parse_with_keep_and_exit() {
    let o = Opening::parse("Grommet", "plate round 30 100 200").unwrap();
    assert_eq!(o.name, "grommet");
    assert_eq!(o.shape, OpeningShape::Round { dia: 30.0 });
    assert_eq!(o.keep, DEFAULT_KEEP_MM);
    assert!(o.exit.is_none());
    let o = Opening::parse("slot", "plate rect 60 20 300,5 50 keep 8 to plate2 10 50").unwrap();
    assert_eq!((o.x, o.keep), (300.5, 8.0));
    assert_eq!(o.exit.as_ref().unwrap().surface, "plate2");
}

#[test]
fn bad_text_is_a_sentence_not_a_silent_drop() {
    for text in [
        "",
        "plate round 0 1 1",
        "plate square 1 1 1",
        "plate round 30 1",
        "plate round 30 1 2 keep",
    ] {
        let e = Opening::parse("x", text).unwrap_err();
        assert!(e.contains("Opening 'x'"), "{text}");
    }
    let (list, problems) = parse_all(&[
        ("a".into(), "plate round 20 50 50".into()),
        ("b".into(), "nonsense".into()),
    ]);
    assert_eq!((list.len(), problems.len()), (1, 1));
}

#[test]
fn keep_out_is_the_opening_plus_its_ring() {
    let o = Opening::parse("g", "plate round 30 100 200 keep 10").unwrap();
    let z = o.keep_out();
    assert_eq!((z.x, z.y, z.w, z.h), (75.0, 175.0, 50.0, 50.0));
}

#[test]
fn devices_over_the_keep_out_conflict_and_touching_does_not() {
    let o = Opening::parse("g", "plate round 30 100 200 keep 10").unwrap();
    let devices = [
        dev(1, 90.0, 190.0, 20.0, 20.0),
        dev(2, 125.0, 175.0, 30.0, 30.0),
        dev(3, 300.0, 300.0, 20.0, 20.0),
    ];
    let c = conflicts(&[o], "plate", &devices);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].device, 1);
    assert_eq!(c[0].rule(), RULE_OPENING_KEEPOUT);
    assert!(c[0].message(&|id| format!("-K{id}")).contains("'g'"));
}

#[test]
fn conflicts_only_look_at_their_own_surface() {
    let o = Opening::parse("g", "plate2 round 30 100 200").unwrap();
    assert!(conflicts(&[o], "plate", &[dev(1, 100.0, 200.0, 10.0, 10.0)]).is_empty());
}

#[test]
fn the_best_passage_adds_the_least_wire_in_either_direction() {
    let near = Opening::parse("near", "plate rect 40 20 500 100 to plate2 20 100").unwrap();
    let far = Opening::parse("far", "plate rect 40 20 500 700 to plate2 20 700").unwrap();
    let other = Opening::parse("other", "plate round 20 10 10").unwrap();
    let all = [far.clone(), near.clone(), other];
    let p = best_passage(&all, "plate", "plate2", (400.0, 100.0), (120.0, 100.0)).unwrap();
    assert_eq!(p.opening.name, "near");
    assert!((p.detour - 200.0).abs() < 1e-9);
    // Reversed: wire from plate2 to plate uses the same opening.
    let q = best_passage(&all, "plate2", "plate", (120.0, 100.0), (400.0, 100.0)).unwrap();
    assert_eq!(q.opening.name, "near");
    assert_eq!(q.enter, (20.0, 100.0));
    assert!(best_passage(&all, "plate", "door", (0.0, 0.0), (0.0, 0.0)).is_none());
}

#[test]
fn plates_are_recognised_by_name() {
    assert!(is_plate("plate") && is_plate("plate2") && !is_plate("door"));
}

#[test]
fn the_layout_reports_devices_over_openings_and_loads_them_from_the_project() {
    use novo_core::{DeviceTag, MountingPlate, Project, Value};
    use novo_layout::model::{load, openings_of, place_device, set_plate};
    use novo_layout::openings::key;

    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                },
            )?;
            tx.set_property(
                cab,
                key("Grommet"),
                Some(Value::Text("plate round 30 100 200".into())),
            )?;
            tx.set_property(cab, key("bad"), Some(Value::Text("nonsense".into())))?;
            tx.set_property(
                cab,
                key("typo"),
                Some(Value::Text("plat round 10 5 5".into())),
            )?;
            tx.set_property(
                cab,
                key("ghost"),
                Some(Value::Text("plate round 10 5 5 to door 1 1".into())),
            )?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            let d = tx.add_device(DeviceTag::parse("=A1+CAB1-K1").unwrap(), "")?;
            tx.assign_part(d, part)?;
            place_device(tx, cab, d.raw(), 90.0, 170.0, false)?;
            Ok(cab)
        })
        .unwrap();
    let (list, problems) = openings_of(&p, cab);
    assert_eq!((list.len(), problems.len()), (3, 1));
    let layout = load(&p, cab).unwrap();
    // 'bad' cannot be read; 'typo' names 'plat'; 'ghost' leads to a door the
    // structure (no enclosure) does not have.
    assert_eq!(
        layout.opening_problems.len(),
        3,
        "{:?}",
        layout.opening_problems
    );
    assert!(layout.opening_problems.iter().any(|t| t.contains("'plat'")));
    assert!(layout.opening_problems.iter().any(|t| t.contains("'door'")));
    let r = layout.report();
    assert_eq!(r.opening_conflicts.len(), 1);
    assert!(!r.is_clean());
}

#[test]
fn unknown_surface_names_are_sentences() {
    let o = Opening::parse("a", "plate round 10 5 5 to plate2 1 1").unwrap();
    let known = vec!["plate".to_string()];
    let p = unknown_surfaces(std::slice::from_ref(&o), &known);
    assert_eq!(p.len(), 1);
    assert!(p[0].contains("'plate2'") && p[0].contains("known: plate"));
    assert!(unknown_surfaces(&[o], &["plate".into(), "plate2".into()]).is_empty());
}
