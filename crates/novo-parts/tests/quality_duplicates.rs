//! Parts quality: duplicate groups and conflicting values.

use novo_core::Value;
use novo_parts::{
    Catalog, CatalogPart, Dimensions, DuplicateReason, Price, conflicts_between, duplicate_groups,
    incoming_conflicts, keys,
};

fn part(maker: &str, order: &str) -> CatalogPart {
    CatalogPart::new(maker, order)
}

fn sized(mut p: CatalogPart, w: f64) -> CatalogPart {
    p.dimensions = Some(Dimensions {
        width_mm: w,
        height_mm: 58.0,
        depth_mm: 73.0,
    });
    p
}

#[test]
fn one_maker_spelled_two_ways_is_a_likely_duplicate() {
    let mut c = Catalog::new();
    c.upsert(sized(part("Siemens", "3RT2016-1BB41"), 45.0));
    c.upsert(sized(part("Siemens AG", "3RT2016 1BB41"), 45.5));
    c.upsert(part("ABB", "S201-B16"));
    let groups = duplicate_groups(&c);
    assert_eq!(groups.len(), 1);
    let g = &groups[0];
    assert_eq!(g.reason, DuplicateReason::SameMakerSpelling);
    assert!(g.reason.likely());
    assert_eq!(g.members.len(), 2);
    assert_eq!(g.conflicts.len(), 1);
    assert_eq!(g.conflicts[0].field, keys::WIDTH_MM);
    assert_eq!(
        g.conflicts[0].values,
        vec![Some("45".into()), Some("45.5".into())]
    );
}

#[test]
fn unrelated_makers_with_one_order_number_are_only_candidates() {
    let mut c = Catalog::new();
    c.upsert(part("Phoenix Contact", "1234567"));
    c.upsert(part("Weidmuller", "1234567"));
    let groups = duplicate_groups(&c);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].reason, DuplicateReason::SameOrderNumber);
    assert!(!groups[0].reason.likely());
}

#[test]
fn short_order_numbers_are_not_flagged() {
    let mut c = Catalog::new();
    c.upsert(part("A Maker", "12"));
    c.upsert(part("B Maker", "12"));
    assert!(duplicate_groups(&c).is_empty());
}

#[test]
fn a_type_number_under_two_order_numbers_is_listed_with_its_differences() {
    let mut a = part("Siemens", "3RT2016-1BB41");
    a.type_number = "3RT2016".into();
    a.data.insert("coil_voltage_v".into(), Value::Float(24.0));
    let mut b = part("Siemens", "3RT2016-1AP01");
    b.type_number = "3RT2016".into();
    b.data.insert("coil_voltage_v".into(), Value::Float(230.0));
    b.data.insert("poles".into(), Value::Int(3));
    let mut c = Catalog::new();
    c.upsert(a);
    c.upsert(b);
    let groups = duplicate_groups(&c);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].reason, DuplicateReason::SameTypeNumber);
    // `poles` is recorded by one member only: unknown is not a conflict.
    let fields: Vec<&str> = groups[0]
        .conflicts
        .iter()
        .map(|c| c.field.as_str())
        .collect();
    assert_eq!(fields, ["coil_voltage_v"]);
}

#[test]
fn equal_values_in_other_units_of_writing_do_not_conflict() {
    let mut a = part("X", "ABCD1");
    a.data.insert("rated_current_a".into(), Value::Int(16));
    a.data
        .insert("trip_curve".into(), Value::Text(" B ".into()));
    let mut b = part("X", "ABCD2");
    b.data.insert("rated_current_a".into(), Value::Float(16.0));
    b.data.insert("trip_curve".into(), Value::Text("b".into()));
    assert!(conflicts_between(&[&a, &b]).is_empty());
}

#[test]
fn prices_compare_per_piece_and_currency() {
    let price = |amount, per, cur: &str| Price {
        amount,
        currency: cur.into(),
        per,
        date: String::new(),
    };
    let mut a = part("X", "ABCD1");
    a.price = Some(price(12.0, 100, "EUR"));
    let mut b = part("X", "ABCD1b");
    b.price = Some(price(0.12, 1, "EUR"));
    assert!(conflicts_between(&[&a, &b]).is_empty());
    b.price = Some(price(0.12, 1, "USD"));
    let c = conflicts_between(&[&a, &b]);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].field, "price");
}

#[test]
fn incoming_files_are_compared_with_the_catalog_by_key() {
    let mut c = Catalog::new();
    c.upsert(sized(part("WAGO", "2002-1201"), 5.2));
    let incoming = vec![
        sized(part("WAGO", "2002 1201"), 5.0),
        sized(part("WAGO", "2002-1202"), 9.9),
    ];
    let found = incoming_conflicts(&c, &incoming);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1[0].field, keys::WIDTH_MM);
    assert_eq!(
        found[0].1[0].values,
        vec![Some("5.2".into()), Some("5".into())]
    );
}

#[test]
fn the_result_does_not_depend_on_insertion_order() {
    let build = |flip: bool| {
        let mut parts = vec![
            part("Siemens", "3RT2016-1BB41"),
            part("Siemens AG", "3RT2016-1BB41"),
            part("Phoenix", "9988776"),
            part("Weidmuller", "9988776"),
        ];
        if flip {
            parts.reverse();
        }
        let mut c = Catalog::new();
        for p in parts {
            c.upsert(p);
        }
        duplicate_groups(&c)
    };
    assert_eq!(build(false), build(true));
    assert_eq!(build(false).len(), 2);
}
