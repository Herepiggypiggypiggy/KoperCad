//! Part suggestions rank by the kind of device, not only its letter code,
//! and the match percentage says something about each row.

use novo_core::{Document, Potential};
use novo_parts::{Catalog, CatalogPart, Role, device_role, part_role, suggestions_for_device};
use novo_tests::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter, Catalog) {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    (doc, gate, Catalog::starter())
}

fn roles(doc: &Document, catalog: &Catalog, device: novo_core::DeviceId) -> Vec<Option<Role>> {
    suggestions_for_device(catalog, doc.project(), device, "", 50)
        .iter()
        .map(|s| part_role(s.part))
        .collect()
}

#[test]
fn the_device_kind_comes_from_its_symbol() {
    let (doc, gate, _) = starter();
    let p = doc.project();
    assert_eq!(device_role(p, gate.k1), Some(Role::Contactor));
    assert_eq!(device_role(p, gate.f2), Some(Role::Overload));
    assert_eq!(device_role(p, gate.m1), Some(Role::Motor));
    assert_eq!(device_role(p, gate.s0), Some(Role::PushNc));
    assert_eq!(device_role(p, gate.s1), Some(Role::PushNo));
}

#[test]
fn an_overload_is_offered_overload_relays_first() {
    let (doc, gate, catalog) = starter();
    let rows = suggestions_for_device(&catalog, doc.project(), gate.f2, "", 50);
    let kinds = roles(&doc, &catalog, gate.f2);
    let n = kinds
        .iter()
        .take_while(|r| **r == Some(Role::Overload))
        .count();
    assert!(n >= 3, "{kinds:?}");
    // What follows is of another kind and scores clearly lower.
    assert!(
        rows[n..]
            .iter()
            .all(|r| r.fit.percent < rows[n - 1].fit.percent)
    );
}

#[test]
fn a_breaker_lists_breakers_before_fuse_holders_and_nothing_is_hidden() {
    let (doc, gate, catalog) = starter();
    let kinds = roles(&doc, &catalog, gate.f3);
    // An MCB comes first, then the motor protection breaker; the fuse
    // holder and overloads are still there, after them.
    assert_eq!(kinds[0], Some(Role::Mcb));
    let first_other = kinds
        .iter()
        .position(|r| !matches!(r, Some(Role::Mcb | Role::Mpcb)))
        .unwrap();
    assert!(
        kinds[first_other..]
            .iter()
            .all(|r| !matches!(r, Some(Role::Mcb | Role::Mpcb)))
    );
    assert!(kinds.contains(&Some(Role::FuseHolder)));
    let all = catalog.suggest("F", "", usize::MAX).len();
    assert_eq!(kinds.len(), all, "every part for the letter is listed");
}

#[test]
fn a_contactor_gets_contactors_before_contactor_relays() {
    let (doc, gate, catalog) = starter();
    let kinds = roles(&doc, &catalog, gate.k1);
    assert_eq!(kinds[0], Some(Role::Contactor));
    // A contactor relay cannot carry a motor: it comes last.
    let relay = kinds.iter().position(|r| *r == Some(Role::Relay)).unwrap();
    assert_eq!(relay, kinds.len() - 1, "{kinds:?}");
}

#[test]
fn a_stop_button_lists_a_normally_closed_button_first() {
    let (doc, gate, catalog) = starter();
    let stop = suggestions_for_device(&catalog, doc.project(), gate.s0, "", 5);
    assert_eq!(part_role(stop[0].part), Some(Role::PushNc));
    assert_eq!(part_role(stop[1].part), Some(Role::PushNc));
    assert!(stop[1].fit.percent > stop[2].fit.percent);
    let start = suggestions_for_device(&catalog, doc.project(), gate.s1, "", 5);
    assert_eq!(part_role(start[0].part), Some(Role::PushNo));
}

#[test]
fn the_coil_voltage_of_the_circuit_ranks_the_contactors() {
    let (mut doc, gate, catalog) = starter();
    let l = doc
        .project()
        .potentials()
        .find(|p| p.name == "L")
        .unwrap()
        .id;
    doc.transact("Set L to 230 V", |tx| {
        tx.update::<Potential>(l, |p| p.voltage = Some(230.0))?;
        Ok(())
    })
    .unwrap();
    let rows = suggestions_for_device(&catalog, doc.project(), gate.k1, "", 10);
    let top = &rows[0];
    assert_eq!(
        top.part.data.get("coil_voltage"),
        Some(&novo_core::Value::Text("230 V AC".into()))
    );
    // Every 24 V DC contactor ranks below every 230 V AC one.
    let worst_match = rows
        .iter()
        .filter(|r| r.part.description.contains("230 V AC"))
        .map(|r| r.fit.percent)
        .min()
        .unwrap();
    let best_miss = rows
        .iter()
        .filter(|r| r.part.description.contains("24 V DC"))
        .map(|r| r.fit.percent)
        .max()
        .unwrap();
    assert!(worst_match > best_miss, "{worst_match} vs {best_miss}");
    assert!(top.fit.reasons.iter().any(|r| r.contains("230 V AC")));
}

#[test]
fn the_match_differs_between_rows_and_is_not_the_data_quality() {
    let (doc, gate, catalog) = starter();
    // Searching shows parts of every kind for the letter, each with its own
    // match: the right kind scores above the wrong one.
    let rows = suggestions_for_device(&catalog, doc.project(), gate.f3, "f", 50);
    let best = rows.iter().map(|r| r.fit.percent).max().unwrap();
    let worst = rows.iter().map(|r| r.fit.percent).min().unwrap();
    assert!(best > worst, "{best} {worst}");
    for row in &rows {
        // F3 is a one-pole breaker: the one-pole MCBs match fully.
        if part_role(row.part) == Some(Role::Mcb)
            && row
                .part
                .data
                .get("poles")
                .is_some_and(|v| *v == novo_core::Value::Int(1))
        {
            assert_eq!(row.fit.percent, best);
        } else if part_role(row.part) == Some(Role::FuseHolder) {
            assert!(row.fit.percent < 50);
        }
    }
}

#[test]
fn a_match_counts_only_the_checks_that_could_be_made_and_says_so() {
    let (doc, gate, catalog) = starter();
    // A stop button states no coil and no rating: kind and letter code are
    // the two checks, and the tooltip lists the skipped ones.
    let top = &suggestions_for_device(&catalog, doc.project(), gate.s0, "", 1)[0];
    assert!(
        top.fit
            .reasons
            .iter()
            .any(|r| r.starts_with("coil: not checked"))
    );
    assert!(
        top.fit
            .reasons
            .iter()
            .any(|r| r.starts_with("rating: not checked"))
    );
}

#[test]
fn a_part_that_does_not_say_what_it_is_ranks_below_a_confirmed_one() {
    let (doc, gate, _) = starter();
    let mut vague = CatalogPart::new("Acme", "V-1");
    vague.letter_codes = vec!["F".into()];
    let mut breaker = CatalogPart::new("Acme", "B-1");
    breaker.letter_codes = vec!["F".into()];
    breaker.keywords = vec!["mcb".into()];
    let mut catalog = Catalog::new();
    catalog.upsert(vague);
    catalog.upsert(breaker);
    let rows = suggestions_for_device(&catalog, doc.project(), gate.f3, "", 10);
    assert_eq!(rows[0].part.order_number, "B-1");
    // The breaker does not state its poles, so that check earns half.
    assert!(rows[0].fit.percent >= 85, "{}", rows[0].fit.percent);
    assert!(rows[1].fit.percent < 50);
}

#[test]
fn rating_a_large_catalog_is_fast() {
    let (doc, gate, _) = starter();
    let mut catalog = Catalog::new();
    for i in 0..5_000 {
        let mut part = CatalogPart::new("Acme", format!("K-{i:05}"));
        part.description = format!("Contactor relay {i}, coil 24 V DC");
        part.letter_codes = vec!["K".into()];
        catalog.upsert(part);
    }
    let t = std::time::Instant::now();
    let rows = suggestions_for_device(&catalog, doc.project(), gate.k1, "", 30);
    assert_eq!(rows.len(), 30);
    // Debug build, shared runner: a loose bound that still catches a
    // quadratic or per-part netlist rebuild.
    assert!(t.elapsed().as_millis() < 2000, "{:?}", t.elapsed());
}

#[test]
fn a_three_pole_breaker_ranks_three_pole_parts_first() {
    use novo_core::{DeviceTag, Pin, Placement, Point, SymbolRole};
    let (mut doc, gate, catalog) = starter();
    let (f9, _) = doc
        .transact("Three-pole breaker", |tx| {
            let f9 = tx.add_device(DeviceTag::parse("-F9").unwrap(), "Breaker")?;
            let pins = [
                ("1", 0.0, 0.0),
                ("2", 0.0, 16.0),
                ("3", 8.0, 0.0),
                ("4", 8.0, 16.0),
                ("5", 16.0, 0.0),
                ("6", 16.0, 16.0),
            ]
            .map(|(n, x, y)| Pin::new(n, x, y))
            .to_vec();
            tx.place_symbol(
                gate.main_page,
                "iec/protection/mcb_3p",
                Placement::at(Point::new(120.0, 40.0)),
                pins,
                SymbolRole::Device {
                    device: f9,
                    main: true,
                },
            )?;
            Ok(f9)
        })
        .unwrap();
    let rows = suggestions_for_device(&catalog, doc.project(), f9, "", 50);
    let poles = |s: &novo_parts::Suggestion| {
        s.part
            .data
            .get("poles")
            .and_then(|v| match v {
                novo_core::Value::Int(n) => Some(*n),
                novo_core::Value::Float(f) => Some(*f as i64),
                _ => None,
            })
            .unwrap_or(0)
    };
    let top = &rows[0];
    assert_eq!(part_role(top.part), Some(Role::Mcb));
    assert_eq!(poles(top), 3, "{}: {:?}", top.part.order_number, top.fit);
    assert_eq!(top.fit.percent, 100);
    let one_pole = rows
        .iter()
        .find(|r| part_role(r.part) == Some(Role::Mcb) && poles(r) == 1)
        .expect("a one-pole breaker is still listed");
    assert!(one_pole.fit.percent < top.fit.percent);
    assert!(one_pole.fit.reasons.iter().any(|r| r.contains("poles")));
}
