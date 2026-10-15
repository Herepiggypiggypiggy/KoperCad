//! Cross-reference queries.

use novo_core::*;

fn symbol(tx: &mut Tx<'_>, page: PageId, x: f64, role: SymbolRole) -> Result<SymbolId> {
    tx.place_symbol(
        page,
        "iec/contacts/no",
        Placement::at(Point::new(x, 50.0)),
        vec![Pin::new("1", 0.0, 0.0)],
        role,
    )
}

#[test]
fn device_parts_in_reading_order() {
    let mut doc = Document::default();
    let ((coil, contacts), _) = doc
        .transact("K1 over three pages", |tx| {
            let [p1, p2, p10] = ["1", "2", "10"].map(|name| tx.add_page(name));
            let (p1, p2, p10) = (p1?, p2?, p10?);
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "Contactor")?;
            let aux = SymbolRole::Device {
                device: k1,
                main: false,
            };
            // A3 landscape is 420 mm wide: 42 mm per column.
            let late = symbol(tx, p10, 10.0, aux.clone())?;
            let right = symbol(tx, p2, 200.0, aux.clone())?;
            let first = symbol(tx, p1, 300.0, aux)?;
            let coil = symbol(
                tx,
                p2,
                50.0,
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            )?;
            Ok((coil, [first, right, late]))
        })
        .unwrap();
    let project = doc.project();
    let k1 = project.devices().next().unwrap().id;
    let refs = project.cross_references(k1);
    let labels: Vec<String> = refs.iter().map(|r| r.location.to_string()).collect();
    assert_eq!(labels, ["1.8", "2.2", "2.5", "10.1"]);
    let order: Vec<SymbolId> = refs.iter().map(|r| r.symbol).collect();
    assert_eq!(order, [contacts[0], coil, contacts[1], contacts[2]]);
    assert_eq!(
        refs.iter()
            .filter(|r| r.main)
            .map(|r| r.symbol)
            .collect::<Vec<_>>(),
        [coil]
    );
}

#[test]
fn interruption_points_find_each_other() {
    let mut doc = Document::default();
    let ((a1, a3, a3b, b), _) = doc
        .transact("Interruptions", |tx| {
            let p1 = tx.add_page("1")?;
            let p3 = tx.add_page("3")?;
            let named = |name: &str| SymbolRole::Interruption { name: name.into() };
            let a1 = symbol(tx, p1, 400.0, named("A"))?;
            let a3b = symbol(tx, p3, 100.0, named("A"))?;
            let a3 = symbol(tx, p3, 10.0, named("A"))?;
            let b = symbol(tx, p3, 50.0, named("B"))?;
            Ok((a1, a3, a3b, b))
        })
        .unwrap();
    let project = doc.project();
    let partners = |s| -> Vec<SymbolId> {
        project
            .interruption_partners(s)
            .iter()
            .map(|r| r.symbol)
            .collect()
    };
    assert_eq!(partners(a1), [a3, a3b]);
    assert_eq!(partners(a3), [a1, a3b]);
    assert!(partners(b).is_empty());
    let first = &project.interruption_partners(a3)[0];
    assert_eq!(first.location.to_string(), "1.10");
}

#[test]
fn grid_columns() {
    let a3 = PageFormat::default();
    assert_eq!(a3.columns, 10);
    assert_eq!(a3.column_at(0.0), 1);
    assert_eq!(a3.column_at(41.9), 1);
    assert_eq!(a3.column_at(42.0), 2);
    assert_eq!(a3.column_at(420.0), 10);
    assert_eq!(a3.column_at(-5.0), 1);
    assert_eq!(a3.column_at(1000.0), 10);
    let a4 = PageFormat {
        size: PaperSize::A4,
        landscape: false,
        columns: 4,
    };
    assert_eq!(a4.column_at(105.0), 3);
    let none = PageFormat { columns: 0, ..a3 };
    assert_eq!(none.column_at(300.0), 1);
    // Pages saved before columns existed get the default.
    let old: PageFormat = serde_json::from_str(r#"{"size": "A3", "landscape": true}"#).unwrap();
    assert_eq!(old.columns, 10);
}

#[test]
fn linking_a_contact_and_what_the_relay_switches() {
    let mut doc = Document::default();
    let ((page, k1, coil, contact, x1, block), _) = doc
        .transact("Relay", |tx| {
            let page = tx.add_page("1")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "Relay")?;
            let coil = tx.place_symbol(
                page,
                "iec/coils/relay",
                Placement::at(Point::new(20.0, 50.0)),
                vec![Pin::new("A1", 0.0, 0.0), Pin::new("A2", 0.0, 10.0)],
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            )?;
            let contact = tx.place_symbol(
                page,
                "iec/contacts/no",
                Placement::at(Point::new(200.0, 50.0)),
                vec![Pin::new("13", 0.0, 0.0), Pin::new("14", 0.0, 10.0)],
                SymbolRole::Unassigned,
            )?;
            tx.update::<SymbolInstance>(contact, |s| s.pass_through = vec![vec![0, 1]])?;
            let x1 = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
            let t1 = tx.add_terminal(x1, "1")?;
            let block = tx.place_symbol(
                page,
                "iec/terminals/terminal",
                Placement::at(Point::new(300.0, 50.0)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                SymbolRole::Terminal {
                    terminals: vec![t1],
                },
            )?;
            tx.connect(Endpoint::pin(contact, 1), Endpoint::pin(block, 0))?;
            Ok((page, k1, coil, contact, x1, block))
        })
        .unwrap();
    // Not linked yet: the contact is no reference of -K1.
    assert_eq!(doc.project().cross_references(k1).len(), 1);

    // Right-click, Cross reference: one command links the contact.
    doc.execute(Command::SetRole {
        symbol: contact,
        role: SymbolRole::Device {
            device: k1,
            main: false,
        },
    })
    .unwrap();
    let project = doc.project();
    let refs = project.cross_references(k1);
    assert_eq!(
        refs.iter().map(|r| (r.symbol, r.main)).collect::<Vec<_>>(),
        [(coil, true), (contact, false)]
    );
    assert!(refs.iter().all(|r| r.location.page == page));

    // Highlight what -K1 switches: the contact's two nets, not the coil's.
    let net = project.netlist();
    let switched = net.switched_by(project, k1);
    let contact_nets = [0, 1].map(|p| net.net_of(Endpoint::pin(contact, p)).unwrap());
    assert_eq!(switched, {
        let mut n = contact_nets.to_vec();
        n.sort();
        n
    });
    assert_eq!(net.nets_of_device(project, k1).len(), 4);

    // A terminal strip's references are its terminals' symbols.
    let strip_refs = project.cross_references(x1);
    assert_eq!(strip_refs.len(), 1);
    assert_eq!(strip_refs[0].symbol, block);
    assert!(strip_refs[0].terminal.is_some());
    assert!(net.nets_of_device(project, x1).contains(&contact_nets[1]));

    // Undo unlinks.
    doc.undo().unwrap();
    assert_eq!(doc.project().cross_references(k1).len(), 1);
}

#[test]
fn search_finds_devices_terminals_and_pages() {
    let mut doc = Document::default();
    doc.transact("Search", |tx| {
        let p = tx.add_page("1")?;
        tx.update::<Page>(p, |page| page.title = "Main circuit".into())?;
        tx.add_page("12")?;
        for tag in ["=A1+CAB1-K1", "-K12", "-K2", "-Q1"] {
            tx.add_device(DeviceTag::parse(tag)?, "")?;
        }
        let x2 = tx.add_terminal_strip(DeviceTag::parse("-X2")?)?;
        tx.add_terminal(x2, "1")?;
        tx.add_terminal(x2, "PE")?;
        tx.add_cable(DeviceTag::parse("-W1")?, Vec::new())?;
        Ok(())
    })
    .unwrap();
    let project = doc.project();
    let texts = |query: &str| -> Vec<(String, Match)> {
        project
            .search(query)
            .into_iter()
            .map(|h| (h.text, h.quality))
            .collect()
    };
    assert_eq!(
        texts("k1"),
        [
            ("=A1+CAB1-K1".to_string(), Match::Exact),
            ("-K12".to_string(), Match::Prefix),
        ]
    );
    assert_eq!(texts("-X2:PE"), [("-X2:PE".to_string(), Match::Exact)]);
    assert_eq!(texts("x2").len(), 3); // the strip and both terminals
    assert_eq!(texts("main")[0].0, "1 Main circuit");
    assert_eq!(texts("w1"), [("-W1".to_string(), Match::Exact)]);
    assert!(texts("  ").is_empty());
    assert!(texts("zz").is_empty());
}

fn structure(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

#[test]
fn pages_sort_by_structure_then_name() {
    let mut doc = Document::default();
    doc.transact("Pages", |tx| {
        for (name, s) in [
            ("2", "=A10"),
            ("1", "=A2+CAB1"),
            ("10", "=A2"),
            ("3", "=A2"),
            ("5", ""),
            ("1", "=A2"),
        ] {
            let page = tx.add_page(name)?;
            tx.update::<Page>(page, |p| p.structure = structure(s))?;
        }
        Ok(())
    })
    .unwrap();
    let order: Vec<String> = doc
        .project()
        .pages_in_order()
        .iter()
        .map(|p| format!("{}/{}", p.structure, p.name))
        .collect();
    assert_eq!(
        order,
        ["/5", "=A2/1", "=A2/3", "=A2/10", "=A2+CAB1/1", "=A10/2"]
    );
}

#[test]
fn page_structure_has_no_device_part() {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Page", |tx| tx.add_page("1")).unwrap();
    let before = doc.project().to_json();
    let err = doc
        .transact("x", |tx| {
            tx.update::<Page>(page, |p| p.structure = structure("=A2-K1"))
        })
        .unwrap_err();
    assert!(matches!(
        err,
        ModelError::InvalidValue {
            field: "structure",
            ..
        }
    ));
    let err = doc
        .transact("x", |tx| {
            tx.update::<Page>(page, |p| p.structure.function.push("A 2".into()))
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::Tag(_)), "{err:?}");
    assert_eq!(doc.project().to_json(), before);

    // A valid structure survives a save; an empty one is not written.
    assert!(!before.contains("structure"));
    doc.transact("Structure", |tx| {
        tx.update::<Page>(page, |p| p.structure = structure("=A2+CAB1"))
    })
    .unwrap();
    let loaded = Project::from_json(&doc.project().to_json()).unwrap();
    assert_eq!(loaded.page(page).unwrap().structure, structure("=A2+CAB1"));
}

#[test]
fn references_carry_the_structure_only_across_structures() {
    let mut doc = Document::default();
    let (coil, _) = doc
        .transact("K1 over two functions", |tx| {
            let supply = tx.add_page("3")?;
            tx.update::<Page>(supply, |p| p.structure = structure("=A1"))?;
            let loads = tx.add_page("3")?;
            tx.update::<Page>(loads, |p| p.structure = structure("=A2"))?;
            let k1 = tx.add_device(DeviceTag::parse("=A1-K1")?, "")?;
            let coil = symbol(
                tx,
                loads,
                130.0,
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            )?;
            symbol(
                tx,
                supply,
                10.0,
                SymbolRole::Device {
                    device: k1,
                    main: false,
                },
            )?;
            Ok(coil)
        })
        .unwrap();
    let project = doc.project();
    let k1 = project.devices().next().unwrap().id;
    let refs = project.cross_references(k1);
    // `=A1` pages come before `=A2` pages.
    assert_eq!(refs[1].symbol, coil);
    let coil_at = &refs[1].location;
    assert_eq!(coil_at.structure, structure("=A2"));
    assert_eq!(coil_at.to_string(), "3.4");
    assert_eq!(coil_at.qualified(&structure("=A2")), "3.4");
    assert_eq!(coil_at.qualified(&structure("=A1")), "=A2/3.4");
    assert_eq!(coil_at.qualified(&DeviceTag::default()), "=A2/3.4");
    let contact_at = &refs[0].location;
    assert_eq!(contact_at.qualified(&structure("=A2")), "=A1/3.1");
    assert_eq!(contact_at.qualified(&structure("=A1")), "3.1");
}

#[test]
fn pages_without_structure_read_plainly() {
    let mut doc = Document::default();
    let (contact, _) = doc
        .transact("x", |tx| {
            let page = tx.add_page("7")?;
            symbol(tx, page, 50.0, SymbolRole::Unassigned)
        })
        .unwrap();
    let at = doc.project().location_of(contact).unwrap();
    assert!(at.structure.is_empty());
    assert_eq!(at.qualified(&DeviceTag::default()), "7.2");
    assert_eq!(at.qualified(&structure("=A2")), "7.2");
}

#[test]
fn frozen_pages_keep_their_printed_references() {
    let mut doc = Document::default();
    let ((page, k1, coil), _) = doc
        .transact("K1", |tx| {
            let page = tx.add_page("3")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
            let coil = symbol(
                tx,
                page,
                50.0,
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            )?;
            Ok((page, k1, coil))
        })
        .unwrap();
    let labels = |doc: &Document| -> Vec<String> {
        doc.project()
            .cross_references(k1)
            .iter()
            .map(|r| r.location.to_string())
            .collect()
    };
    assert_eq!(labels(&doc), ["3.2"]);
    doc.execute(Command::FreezePage {
        page,
        revision: "A".into(),
    })
    .unwrap();
    // Renaming the page and moving the coil do not move its printed reference.
    doc.transact("Rework", |tx| {
        tx.update::<Page>(page, |p| p.name = "4".into())?;
        tx.move_symbol(coil, Placement::at(Point::new(300.0, 50.0)))
    })
    .unwrap();
    assert_eq!(labels(&doc), ["3.2"]);
    // A symbol added after the freeze is referenced where it is; the list
    // stays in drawing order (the contact is left of the moved coil).
    doc.transact("Contact", |tx| {
        symbol(
            tx,
            page,
            100.0,
            SymbolRole::Device {
                device: k1,
                main: false,
            },
        )
    })
    .unwrap();
    assert_eq!(labels(&doc), ["4.3", "3.2"]);
    let loaded = Project::from_json(&doc.project().to_json()).unwrap();
    assert_eq!(
        loaded.page(page).unwrap().frozen.as_ref().unwrap().revision,
        "A"
    );
    // The next revision re-issues the page as it is now.
    doc.execute(Command::FreezePage {
        page,
        revision: "B".into(),
    })
    .unwrap();
    assert_eq!(labels(&doc), ["4.3", "4.8"]);
    doc.execute(Command::UnfreezePage { page }).unwrap();
    assert_eq!(labels(&doc), ["4.3", "4.8"]);
}
