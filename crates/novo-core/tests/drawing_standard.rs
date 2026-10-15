//! The project-level drawing standard (the symbol pack id).

use novo_core::*;

fn set(doc: &mut Document, standard: &str) -> Result<Outcome> {
    let mut info = doc.project().info().clone();
    info.rules.standard = standard.into();
    doc.execute(Command::SetProjectInfo { info })
}

#[test]
fn the_standard_is_set_undone_and_checked() {
    let mut doc = Document::default();
    assert_eq!(doc.project().info().rules.standard, "");
    set(&mut doc, "nfpa").unwrap();
    assert_eq!(doc.project().info().rules.standard, "nfpa");
    doc.undo().unwrap();
    assert_eq!(doc.project().info().rules.standard, "");
    for bad in ["NFPA", "iec 60617", "iec/1", "é"] {
        assert!(set(&mut doc, bad).is_err(), "{bad}");
    }
    set(&mut doc, "iec").unwrap();
    set(&mut doc, "").unwrap();
}

#[test]
fn only_a_set_standard_reaches_the_file() {
    assert!(ProjectRules::recommended().standard.is_empty());
    let plain = Project::new("A").to_json();
    assert!(!plain.contains("standard"));

    let mut doc = Document::default();
    set(&mut doc, "nfpa").unwrap();
    let json = doc.project().to_json();
    assert!(json.contains("\"standard\": \"nfpa\""));
    let back = Project::from_json(&json).unwrap();
    assert_eq!(back.info().rules.standard, "nfpa");
    assert_eq!(back.to_json(), json);
}

fn symbol(doc: &mut Document, key: &str) -> SymbolId {
    doc.transact("Symbol", |tx| {
        let page = tx.add_page("1")?;
        tx.place_symbol(
            page,
            key,
            Placement::at(Point::new(10.0, 10.0)),
            vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
            SymbolRole::Unassigned,
        )
    })
    .unwrap()
    .0
}

#[test]
fn switching_the_standard_rekeys_symbols_in_one_step() {
    let mut doc = Document::default();
    let coil = symbol(&mut doc, "iec/relays/coil");
    let before = doc.project().symbol(coil).unwrap().clone();
    doc.execute(Command::SwitchStandard {
        standard: "nfpa".into(),
        symbols: vec![(coil, SymbolKey::from("nfpa/relays/coil"))],
    })
    .unwrap();
    let after = doc.project().symbol(coil).unwrap();
    assert_eq!(after.symbol.0, "nfpa/relays/coil");
    assert_eq!(
        (&after.pins, after.placement),
        (&before.pins, before.placement)
    );
    assert_eq!(doc.project().info().rules.standard, "nfpa");

    doc.undo().unwrap();
    assert_eq!(
        doc.project().symbol(coil).unwrap().symbol.0,
        "iec/relays/coil"
    );
    assert_eq!(doc.project().info().rules.standard, "");
}

#[test]
fn a_bad_switch_changes_nothing() {
    let mut doc = Document::default();
    let coil = symbol(&mut doc, "iec/relays/coil");
    let switch = |standard: &str, symbols| Command::SwitchStandard {
        standard: standard.into(),
        symbols,
    };
    let missing = (SymbolId(9999), SymbolKey::from("nfpa/x"));
    let ok = (coil, SymbolKey::from("nfpa/relays/coil"));
    assert!(
        doc.execute(switch("nfpa", vec![ok.clone(), missing]))
            .is_err()
    );
    assert!(doc.execute(switch("NFPA", vec![ok])).is_err());
    assert!(
        doc.execute(switch("nfpa", vec![(coil, SymbolKey::from(""))]))
            .is_err()
    );
    assert_eq!(
        doc.project().symbol(coil).unwrap().symbol.0,
        "iec/relays/coil"
    );
    assert_eq!(doc.project().info().rules.standard, "");

    // Switching with nothing to re-key still sets the standard.
    doc.execute(switch("nfpa", Vec::new())).unwrap();
    assert_eq!(doc.project().info().rules.standard, "nfpa");
}
