//! Filling in a default size and colour on the wires that have none.

use novo_core::{Document, Wire};
use novo_reports::{WireDefaults, apply_wire_defaults, unset_wires, wire_cut_jobs};
use novo_tests::motor_starter;

fn fresh() -> Document {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    // The fixture draws no sizes or colours; make sure of it.
    let ids: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    doc.transact("Clear", |tx| {
        for id in ids {
            tx.update::<Wire>(id, |w| {
                w.cross_section_mm2 = None;
                w.color = None;
            })?;
        }
        Ok(())
    })
    .unwrap();
    doc
}

#[test]
fn a_fresh_drawing_has_every_wire_unset() {
    let doc = fresh();
    let unset = unset_wires(doc.project());
    assert!(unset.total > 0);
    assert!(!unset.is_complete());
    // Motor wires on a cable take their size from the core; the rest print
    // "size not set".
    assert!(
        wire_cut_jobs(doc.project())
            .iter()
            .any(|j| j.cross_section_mm2.is_none() && j.colour.is_empty())
    );
}

#[test]
fn the_default_size_goes_on_unset_wires_only() {
    let mut doc = fresh();
    let own = doc.project().wires().next().unwrap().id;
    doc.transact("Own", |tx| {
        tx.update::<Wire>(own, |w| w.cross_section_mm2 = Some(4.0))
    })
    .unwrap();
    let before = unset_wires(doc.project());
    let (sized, coloured) = doc
        .transact("Defaults", |tx| {
            apply_wire_defaults(
                tx,
                &WireDefaults {
                    size_mm2: Some(1.5),
                    ..WireDefaults::default()
                },
            )
        })
        .unwrap()
        .0;
    assert_eq!((sized, coloured), (before.no_size.len(), 0));
    assert_eq!(
        doc.project().wire(own).unwrap().cross_section_mm2,
        Some(4.0)
    );
    let after = unset_wires(doc.project());
    assert!(after.no_size.is_empty());
    assert_eq!(after.no_colour.len(), before.no_colour.len());
    // A second run changes nothing.
    let again = doc
        .transact("Defaults", |tx| {
            apply_wire_defaults(
                tx,
                &WireDefaults {
                    size_mm2: Some(2.5),
                    ..WireDefaults::default()
                },
            )
        })
        .unwrap()
        .0;
    assert_eq!(again, (0, 0));
}

#[test]
fn earth_and_neutral_take_their_standard_colours_and_the_rest_the_chosen_one() {
    let mut doc = fresh();
    doc.transact("Defaults", |tx| {
        apply_wire_defaults(
            tx,
            &WireDefaults {
                size_mm2: None,
                colour: Some("BK".into()),
                fill_colours: true,
            },
        )
    })
    .unwrap();
    let colours: Vec<String> = doc
        .project()
        .wires()
        .map(|w| w.color.clone().unwrap_or_default())
        .collect();
    assert!(colours.iter().any(|c| c == "GNYE"), "{colours:?}");
    assert!(colours.iter().any(|c| c == "BK"), "{colours:?}");
    assert!(
        unset_wires(doc.project()).no_colour.is_empty(),
        "every wire has a colour"
    );
}

#[test]
fn without_filling_colours_none_are_set() {
    let mut doc = fresh();
    let (_, coloured) = doc
        .transact("Defaults", |tx| {
            apply_wire_defaults(
                tx,
                &WireDefaults {
                    colour: Some("BK".into()),
                    ..WireDefaults::default()
                },
            )
        })
        .unwrap()
        .0;
    assert_eq!(coloured, 0);
}

#[test]
fn a_colour_from_the_cable_core_is_not_filled_over_and_shows_on_the_cut_list() {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).expect("fixture builds");
    // The four motor wires lie on cores named BK, BN, GY and GNYE.
    let ids: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    doc.transact("Clear", |tx| {
        for id in ids {
            tx.update::<Wire>(id, |w| w.color = None)?;
        }
        Ok(())
    })
    .unwrap();
    let unset = unset_wires(doc.project());
    for wire in h.motor_wires {
        assert!(
            !unset.no_colour.contains(&wire),
            "the core gives its colour"
        );
    }
    doc.transact("Defaults", |tx| {
        apply_wire_defaults(
            tx,
            &WireDefaults {
                size_mm2: None,
                colour: Some("RD".into()),
                fill_colours: true,
            },
        )
    })
    .unwrap();
    for wire in h.motor_wires {
        assert_eq!(doc.project().wire(wire).unwrap().color, None);
    }
    // The cut list prints the core's colour for the motor earth wire.
    let jobs = wire_cut_jobs(doc.project());
    assert!(jobs.iter().any(|j| j.colour == "GNYE"));
    assert!(
        jobs.iter()
            .all(|j| j.colour != "RD" || !j.wires.iter().any(|w| h.motor_wires.contains(w)))
    );
}
