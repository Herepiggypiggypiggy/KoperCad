//! The routes overlay: duct tone and label, the project's wire rules and the
//! rule that a wire which is not routed shows no length.

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind};
use novo_routing::project_rules::{SLACK_MM, set_rule};
use novo_tests::motor_starter;

use super::cabinet_routes::{DuctFill, Overlay, Tone, duct_labels, overlay, tone};

fn duct(ratio: Option<f64>, wires: usize) -> DuctFill {
    DuctFill {
        rail: 1,
        rect: (0.0, 0.0, 40.0, 200.0),
        ratio,
        wires,
    }
}

#[test]
fn a_duct_has_a_tone_and_a_label_that_do_not_rely_on_colour() {
    let cases = [
        (duct(Some(0.0), 0), Tone::Idle, vec![]),
        (duct(None, 0), Tone::Idle, vec![]),
        (
            duct(Some(0.62), 4),
            Tone::Ok,
            vec!["62% · 4".to_owned(), "62%".to_owned()],
        ),
        (
            duct(Some(0.9), 7),
            Tone::Busy,
            vec!["90% · 7".to_owned(), "90%".to_owned()],
        ),
        (
            duct(Some(1.124), 9),
            Tone::Over,
            vec![
                "over 112% · 9".to_owned(),
                "over 112%".to_owned(),
                "112%".to_owned(),
            ],
        ),
        (
            duct(None, 3),
            Tone::NoSize,
            vec!["no size · 3".to_owned(), "no size".to_owned()],
        ),
    ];
    for (d, expected_tone, labels) in cases {
        assert_eq!(tone(&d), expected_tone, "{d:?}");
        assert_eq!(duct_labels(&d), labels, "{d:?}");
    }
    // The band edges: 80% is still fine, 100% is exactly full, not over.
    assert_eq!(tone(&duct(Some(0.8), 1)), Tone::Ok);
    assert_eq!(tone(&duct(Some(1.0), 1)), Tone::Busy);
    assert_eq!(tone(&duct(Some(1.0001), 1)), Tone::Over);
}

fn cabinet() -> (Document, novo_core::StructureId) {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).expect("fixture builds");
    let (cab, _) = doc
        .transact("Cabinet", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB")?, "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 600.0,
                    margin: 10.0,
                }),
            )?;
            // Only F1 and K1 are on the plate; the other devices are not.
            tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, None)?;
            tx.place_device(h.k1, cab, Point::new(400.0, 100.0), false, None)?;
            let d = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
            tx.set_rail_size(d, 700.0, 50.0, 60.0)?;
            Ok(cab)
        })
        .expect("layout");
    (doc, cab)
}

#[test]
fn only_wires_with_both_ends_on_the_plate_show_a_length() {
    let (doc, cab) = cabinet();
    let o = overlay(doc.project(), cab);
    assert!(!o.rows.is_empty());
    assert_eq!(o.rows.len(), o.routes.len());
    assert!(o.rows.iter().all(|r| r.length_mm >= 1.0));
    // The project has more wires than the plate routes: the rest has no row.
    assert!(doc.project().wires().count() > o.rows.len());
}

#[test]
fn the_overlay_is_routed_with_the_project_rules() {
    let (mut doc, cab) = cabinet();
    let before = overlay(doc.project(), cab);
    assert_eq!(before.rules, novo_routing::WireRules::default());
    doc.transact("Rule", |tx| set_rule(tx, SLACK_MM, Some(100.0)))
        .unwrap();
    let after = overlay(doc.project(), cab);
    assert_eq!(after.rules.routing.slack_mm, 100.0);
    assert_eq!(after.rows.len(), before.rows.len());
    // 50 mm more at each end.
    for (b, a) in before.rows.iter().zip(&after.rows) {
        assert_eq!(a.length_mm, b.length_mm + 100.0, "{b:?} {a:?}");
    }
    // A plate with no routes still carries the rules for the editor.
    let none = overlay(doc.project(), novo_core::StructureId::from(9999));
    assert_eq!(none.rules.routing.slack_mm, 100.0);
    assert!(none.rows.is_empty());
    assert_ne!(none, Overlay::default());
}

#[test]
fn the_overlay_carries_the_plate_and_its_entry_point_for_the_editor() {
    let (mut doc, cab) = cabinet();
    let o = overlay(doc.project(), cab);
    assert!(o.plate.is_some(), "the plate size is shown to the editor");
    assert_eq!(o.entry, None);
    let (w, h) = o.plate.unwrap();
    doc.transact("Entry", |tx| {
        novo_routing::entry::set_entry_point(tx, cab, Some(novo_routing::Point::new(w / 2.0, h)))
    })
    .unwrap();
    assert_eq!(overlay(doc.project(), cab).entry, Some((w / 2.0, h)));
    // A smaller plate hides the stored point and the panel says so.
    doc.transact("Shrink", |tx| {
        tx.update::<novo_core::Structure>(cab, |s| {
            s.plate = Some(novo_core::MountingPlate {
                width: 10.0,
                height: 10.0,
                margin: 0.0,
            });
        })
    })
    .unwrap();
    let hidden = overlay(doc.project(), cab);
    assert_eq!(hidden.entry, None);
    assert!(hidden.entry_hidden);
    doc.undo().unwrap();
    doc.undo().unwrap();
    let o = overlay(doc.project(), cab);
    assert_eq!((o.entry, o.entry_hidden), (None, false));
}
