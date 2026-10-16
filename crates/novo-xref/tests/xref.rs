//! WS-10 acceptance: docs/WORKFLOW.md journey J6 and the cross-reference
//! parts of J2 and J4.

use std::time::Instant;

use novo_core::{
    Command, DeviceId, DeviceTag, Document, Page, PageId, Pin, Placement, Point, SymbolId,
    SymbolRole, Tx,
};
use novo_xref::{
    ContactKind, FromKey, Issue, LabelAnchor, LinkError, LiveXref, Navigator, Place, XrefAction,
    XrefIndex, link_command, link_to_device_command,
};

fn coil(tx: &mut Tx<'_>, page: PageId, x: f64, device: DeviceId) -> SymbolId {
    tx.place_symbol(
        page,
        "iec/coils/relay",
        Placement::at(Point::new(x, 100.0)),
        vec![Pin::new("A1", 0.0, -8.0), Pin::new("A2", 0.0, 8.0)],
        SymbolRole::Device { device, main: true },
    )
    .unwrap()
}

fn contact(
    tx: &mut Tx<'_>,
    page: PageId,
    x: f64,
    key: &str,
    pins: [&str; 2],
    device: DeviceId,
) -> SymbolId {
    tx.place_symbol(
        page,
        key,
        Placement::at(Point::new(x, 100.0)),
        vec![Pin::new(pins[0], 0.0, -8.0), Pin::new(pins[1], 0.0, 8.0)],
        SymbolRole::Device {
            device,
            main: false,
        },
    )
    .unwrap()
}

struct Fixture {
    doc: Document,
    pages: [PageId; 3],
    k1: DeviceId,
    k1_coil: SymbolId,
    /// NO 13-14 on page 2 column 2, NC 21-22 on page 2 column 8, NO 33-34 on page 3 column 4.
    k1_contacts: [SymbolId; 3],
}

/// `-K1` coil on page 2 column 5; two contacts on the same page, one on page 3.
fn fixture() -> Fixture {
    let mut doc = Document::default();
    let (f, _) = doc
        .transact("Draw", |tx| {
            let p1 = tx.add_page("1")?;
            let p2 = tx.add_page("2")?;
            let p3 = tx.add_page("3")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "Contactor")?;
            let k1_coil = coil(tx, p2, 190.0, k1);
            let c1 = contact(tx, p2, 50.0, "iec/contacts/no", ["13", "14"], k1);
            let c2 = contact(tx, p2, 300.0, "iec/contacts/nc", ["21", "22"], k1);
            let c3 = contact(tx, p3, 140.0, "iec/contacts/no", ["33", "34"], k1);
            Ok(Fixture {
                doc: Document::default(),
                pages: [p1, p2, p3],
                k1,
                k1_coil,
                k1_contacts: [c1, c2, c3],
            })
        })
        .unwrap();
    Fixture { doc, ..f }
}

#[test]
fn coil_shows_contact_image_and_contacts_show_the_coil() {
    let f = fixture();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    assert_eq!(
        x.label(f.k1_coil).unwrap().lines,
        ["13-14 2.2", "21-22 2.8", "33-34 3.4"]
    );
    assert_eq!(x.label(f.k1_coil).unwrap().anchor, LabelAnchor::Below);
    for c in f.k1_contacts {
        let label = x.label(c).unwrap();
        assert_eq!(label.lines, ["2.5"]);
        assert_eq!(label.anchor, LabelAnchor::Right);
    }
    // Lists keep the kind; the dock hint is read away from the drawing.
    assert_eq!(x.all_references(f.k1_coil)[1].row(), "NO 13-14 2.2");
    assert_eq!(x.labels_on_page(f.pages[1]).len(), 3);
    assert!(x.labels_on_page(f.pages[0]).is_empty());
    assert!(x.issues().is_empty());
}

#[test]
fn coil_label_has_one_table_row_per_contact_group() {
    use novo_xref::ContactKind;
    let f = fixture();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    let label = x.label(f.k1_coil).unwrap();
    let rows: Vec<(ContactKind, &str, &str)> = label
        .rows
        .iter()
        .map(|r| (r.kind, r.pins.as_str(), r.location.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (ContactKind::NormallyOpen, "13-14", "2.2"),
            (ContactKind::NormallyClosed, "21-22", "2.8"),
            (ContactKind::NormallyOpen, "33-34", "3.4"),
        ]
    );
    assert!(label.extra_lines().is_empty());
    // A contact's label has no table.
    assert!(x.label(f.k1_contacts[0]).unwrap().rows.is_empty());
}

#[test]
fn moving_a_contact_updates_the_coil_at_once() {
    let mut f = fixture();
    let mut live = LiveXref::default();
    let before = live
        .get(f.doc.project(), &FromKey)
        .label(f.k1_coil)
        .unwrap();
    assert!(before.lines.contains(&"33-34 3.4".to_owned()));

    let moved = f.k1_contacts[2];
    f.doc
        .transact("Move", |tx| {
            tx.move_symbol(moved, Placement::at(Point::new(400.0, 10.0)))
        })
        .unwrap();
    let after = live
        .get(f.doc.project(), &FromKey)
        .label(f.k1_coil)
        .unwrap();
    assert!(after.lines.contains(&"33-34 3.10".to_owned()));

    f.doc.undo().unwrap();
    let undone = live
        .get(f.doc.project(), &FromKey)
        .label(f.k1_coil)
        .unwrap();
    assert_eq!(undone, before);
}

#[test]
fn g_and_alt_left_round_trip_between_pages() {
    let f = fixture();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    let mut nav = Navigator::default();
    let at_contact = Place {
        page: f.pages[2],
        symbol: Some(f.k1_contacts[2]),
    };

    // G on the contact on page 3: to the coil on page 2.
    let go = nav.go(&x, at_contact).unwrap();
    assert_eq!(
        go.to,
        Place {
            page: f.pages[1],
            symbol: Some(f.k1_coil)
        }
    );
    // Alt+Left: back to the contact.
    assert_eq!(nav.back(go.to), Some(at_contact));
    assert_eq!(nav.forward(at_contact), Some(go.to));
}

#[test]
fn g_on_a_coil_cycles_its_contacts() {
    let f = fixture();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    let mut nav = Navigator::default();
    let start = Place {
        page: f.pages[1],
        symbol: Some(f.k1_coil),
    };
    let [c1, c2, c3] = f.k1_contacts;
    let first = nav.go(&x, start).unwrap();
    // Reading order: 2.2, 2.8, 3.4.
    assert_eq!(first.to.symbol, Some(c1));
    assert_eq!(first.targets.len(), 3);
    let second = nav.go(&x, first.to).unwrap();
    assert_eq!(second.to.symbol, Some(c2));
    let third = nav.go(&x, second.to).unwrap();
    assert_eq!(third.to.symbol, Some(c3));
    assert_eq!(nav.go(&x, third.to).unwrap().to.symbol, Some(c1));
    // Cycling did not fill the history: one Alt+Left returns to the coil.
    assert_eq!(nav.back(third.to), Some(start));
    assert!(!nav.history.can_go_back());
}

#[test]
fn shift_g_lists_every_occurrence() {
    let f = fixture();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    let nav = Navigator::default();
    for s in [f.k1_coil, f.k1_contacts[1]] {
        let place = Place {
            page: f.pages[1],
            symbol: Some(s),
        };
        let all: Vec<String> = nav
            .all(&x, place)
            .iter()
            .map(|r| r.location.to_string())
            .collect();
        assert_eq!(all, ["2.2", "2.5", "2.8", "3.4"]);
    }
}

#[test]
fn selecting_the_coil_highlights_its_contacts_on_the_page() {
    let f = fixture();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    let h = x.highlight(f.k1_coil);
    let [c1, c2, c3] = f.k1_contacts;
    assert_eq!(h.on_page, [c1, c2]);
    assert_eq!(h.off_page.len(), 1);
    assert_eq!(h.off_page[0].symbol, c3);
    assert_eq!(h.dock_hint(), "-K1 elsewhere: NO 33-34 3.4");

    // Selecting the device from the project tree while page 3 is shown.
    let h = x.highlight_device(f.k1, f.pages[2]);
    assert_eq!(h.on_page, [c3]);
    assert_eq!(h.off_page.len(), 3);
}

#[test]
fn renaming_the_device_carries_every_reference() {
    let mut f = fixture();
    let k1 = f.k1;
    f.doc
        .transact("Rename", |tx| tx.set_tag(k1, DeviceTag::parse("-K10")?))
        .unwrap();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    let found = x.find(f.doc.project(), "-K10");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1.len(), 4);
    assert_eq!(x.highlight(f.k1_coil).name, "-K10");
    assert_eq!(x.label(f.k1_contacts[0]).unwrap().lines, ["2.5"]);
}

#[test]
fn find_uses_the_model_search() {
    let mut doc = Document::default();
    doc.transact("Draw", |tx| {
        let p = tx.add_page("1")?;
        let a = tx.add_device(DeviceTag::parse("=A1-K1")?, "")?;
        let b = tx.add_device(DeviceTag::parse("-K1")?, "")?;
        coil(tx, p, 10.0, a);
        coil(tx, p, 100.0, b);
        tx.add_device(DeviceTag::parse("-K2")?, "")?;
        Ok(())
    })
    .unwrap();
    let p = doc.project();
    let x = XrefIndex::build(p, &FromKey);
    let hits = x.find(p, "k1");
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|(_, refs)| refs.len() == 1));
    assert_eq!(x.find(p, "=A1-K1").len(), 1);
    assert!(x.find(p, "-K2")[0].1.is_empty()); // not drawn yet
    assert!(x.find(p, "=B9").is_empty());
}

#[test]
fn order_matches_the_model() {
    let f = fixture();
    let p = f.doc.project();
    let x = XrefIndex::build(p, &FromKey);
    let model: Vec<SymbolId> = p.cross_references(f.k1).iter().map(|r| r.symbol).collect();
    let mut ours: Vec<&novo_xref::Reference> = x.device_references(f.k1);
    ours.sort_by_key(|r| r.order);
    assert_eq!(ours.iter().map(|r| r.symbol).collect::<Vec<_>>(), model);
    for r in ours {
        assert_eq!(Some(&r.location), p.location_of(r.symbol).as_ref());
    }
}

#[test]
fn too_many_contacts_show_in_the_contact_image() {
    let mut f = fixture();
    let k1 = f.k1;
    f.doc
        .transact("Limit", |tx| {
            tx.set_property(k1, "contacts.no", Some(1i64.into()))
        })
        .unwrap();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    let label = x.label(f.k1_coil).unwrap();
    assert!(label.warning);
    assert_eq!(label.lines.last().unwrap(), "! 2 NO drawn, 1 available");
    assert_eq!(
        x.issues(),
        [Issue::TooManyContacts {
            device: k1,
            kind: ContactKind::NormallyOpen,
            drawn: 2,
            available: 1
        }]
    );
}

#[test]
fn terminal_strips_list_their_terminals() {
    let mut doc = Document::default();
    let ((strip, t1, t2), _) = doc
        .transact("Draw", |tx| {
            let p1 = tx.add_page("1")?;
            let p2 = tx.add_page("2")?;
            let strip = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
            let mut syms = Vec::new();
            for (page, n) in [(p1, "1"), (p2, "2")] {
                let t = tx.add_terminal(strip, n)?;
                syms.push(tx.place_symbol(
                    page,
                    "iec/terminals/terminal",
                    Placement::at(Point::new(10.0, 10.0)),
                    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 4.0)],
                    SymbolRole::Terminal { terminals: vec![t] },
                )?);
            }
            Ok((strip, syms[0], syms[1]))
        })
        .unwrap();
    let x = XrefIndex::build(doc.project(), &FromKey);
    assert_eq!(x.device_references(strip).len(), 2);
    assert_eq!(
        x.partners(t1).iter().map(|r| r.symbol).collect::<Vec<_>>(),
        [t2]
    );
    // Terminals draw no cross-reference text and are not "contacts without a coil".
    assert!(x.label(t1).is_none());
    assert!(x.issues().is_empty());
}

#[test]
fn context_menu_and_command_bar_share_one_action_list() {
    let f = fixture();
    let p = f.doc.project();
    let x = XrefIndex::build(p, &FromKey);
    assert_eq!(
        XrefAction::for_symbol(p, &x, f.k1_coil),
        [
            XrefAction::GoToPartner,
            XrefAction::AllReferences,
            XrefAction::Link
        ]
    );
    assert_eq!(XrefAction::Link.label(), "Link cross reference");
    assert_eq!(XrefAction::GoToPartner.shortcut(), Some("G"));
}

#[test]
fn link_a_lone_contact_to_a_coil() {
    let mut f = fixture();
    let page = f.pages[0];
    // A contact placed without a tag got its own device -K2.
    let (lone, _) = f
        .doc
        .transact("Place", |tx| {
            let k2 = tx.add_device(DeviceTag::parse("-K2")?, "")?;
            Ok(contact(tx, page, 20.0, "iec/contacts/no", ["13", "14"], k2))
        })
        .unwrap();
    let devices_before = f.doc.project().devices().count();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    assert!(matches!(x.issues()[..], [Issue::NoMain { .. }]));

    // Right click on the contact, "Link cross reference", click the coil.
    let cmd = link_command(f.doc.project(), &FromKey, lone, f.k1_coil).unwrap();
    f.doc.execute(cmd).unwrap();
    assert_eq!(f.doc.undo_label(), Some("Link cross reference"));

    let p = f.doc.project();
    let sym = p.symbol(lone).unwrap();
    assert_eq!(sym.device(), Some(f.k1));
    // A contact joins as a secondary function, never as the main one.
    assert!(matches!(sym.role, SymbolRole::Device { main: false, .. }));
    // -K2 existed only for this contact, so it is gone.
    assert_eq!(p.devices().count(), devices_before - 1);
    // 13-14 was taken on -K1 and so was 33-34; 21-22 used ordinal 2.
    let names: Vec<&str> = sym.pins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["43", "44"]);
    let x = XrefIndex::build(p, &FromKey);
    assert_eq!(x.label(lone).unwrap().lines, ["2.5"]);
    assert!(
        x.label(f.k1_coil)
            .unwrap()
            .lines
            .contains(&"43-44 1.1".to_owned())
    );
    assert!(x.issues().is_empty());

    // One undo step brings everything back.
    f.doc.undo().unwrap();
    assert_eq!(f.doc.project().devices().count(), devices_before);
    assert_eq!(f.doc.project().symbol(lone).unwrap().pins[0].name, "13");
}

#[test]
fn link_works_from_the_coil_side_too_and_keeps_free_numbers() {
    let mut f = fixture();
    let page = f.pages[0];
    let (lone, _) = f
        .doc
        .transact("Place", |tx| {
            let k2 = tx.add_device(DeviceTag::parse("-K2")?, "Keep me")?;
            Ok(contact(tx, page, 20.0, "iec/contacts/no", ["53", "54"], k2))
        })
        .unwrap();
    let cmd = link_command(f.doc.project(), &FromKey, f.k1_coil, lone).unwrap();
    let Command::Batch { commands, .. } = &cmd else {
        panic!("link is a batch");
    };
    // -K2 has a description, so it is kept.
    assert_eq!(commands.len(), 1);
    f.doc.execute(cmd).unwrap();
    let sym = f.doc.project().symbol(lone).unwrap();
    assert_eq!(sym.device(), Some(f.k1));
    assert_eq!(sym.pins[0].name, "53");
}

/// An untagged coil placed on page 1, `Unassigned`.
fn unassigned_coil(f: &mut Fixture) -> SymbolId {
    let page = f.pages[0];
    f.doc
        .transact("Place", |tx| {
            tx.place_symbol(
                page,
                "iec/coils/relay",
                Placement::at(Point::new(20.0, 100.0)),
                vec![Pin::new("A1", 0.0, -8.0), Pin::new("A2", 0.0, 8.0)],
                SymbolRole::Unassigned,
            )
        })
        .unwrap()
        .0
}

#[test]
fn unassigned_coil_becomes_the_main_function_of_a_contact_only_device() {
    let mut f = fixture();
    let page = f.pages[2];
    // -K3 has only a contact on page 3.
    let ((k3, k3_contact), _) = f
        .doc
        .transact("Place", |tx| {
            let k3 = tx.add_device(DeviceTag::parse("-K3")?, "Relay")?;
            Ok((
                k3,
                contact(tx, page, 300.0, "iec/contacts/no", ["13", "14"], k3),
            ))
        })
        .unwrap();
    let coil_sym = unassigned_coil(&mut f);
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    assert!(x.issues().iter().any(|i| matches!(i, Issue::NoMain { .. })));

    // Right click on the coil, "Link cross reference", click the contact.
    let cmd = link_command(f.doc.project(), &FromKey, coil_sym, k3_contact).unwrap();
    f.doc.execute(cmd).unwrap();

    let p = f.doc.project();
    assert_eq!(
        p.symbol(coil_sym).unwrap().role,
        SymbolRole::Device {
            device: k3,
            main: true
        }
    );
    let x = XrefIndex::build(p, &FromKey);
    assert_eq!(x.label(coil_sym).unwrap().lines, ["13-14 3.8"]);
    assert_eq!(x.label(k3_contact).unwrap().lines, ["1.1"]);
    assert!(x.issues().is_empty());

    // G on the coil finds the contact.
    let mut nav = Navigator::default();
    let at_coil = Place {
        page: f.pages[0],
        symbol: Some(coil_sym),
    };
    let go = nav.go(&x, at_coil).unwrap();
    assert_eq!(go.to.symbol, Some(k3_contact));
}

#[test]
fn coil_onto_a_device_that_has_a_main_function_is_refused() {
    let mut f = fixture();
    let coil_sym = unassigned_coil(&mut f);
    let p = f.doc.project();
    for (a, b) in [(coil_sym, f.k1_contacts[2]), (f.k1_contacts[2], coil_sym)] {
        assert_eq!(
            link_command(p, &FromKey, a, b),
            Err(LinkError::DeviceHasMain)
        );
    }
}

#[test]
fn link_interruption_points() {
    let mut doc = Document::default();
    let ((a, b, c), _) = doc
        .transact("Draw", |tx| {
            let p1 = tx.add_page("1")?;
            let p2 = tx.add_page("2")?;
            let arrow = |tx: &mut Tx<'_>, page, x, name: &str| {
                tx.place_symbol(
                    page,
                    "iec/connectivity/interruption",
                    Placement::at(Point::new(x, 10.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Interruption { name: name.into() },
                )
            };
            Ok((
                arrow(tx, p1, 400.0, "L1")?,
                arrow(tx, p2, 10.0, "")?,
                arrow(tx, p2, 50.0, "24V")?,
            ))
        })
        .unwrap();
    let x = XrefIndex::build(doc.project(), &FromKey);
    assert_eq!(x.issues().len(), 3); // each arrow is alone, the unnamed one too
    assert!(x.label(a).is_none());

    // The unnamed arrow takes its partner's name, whichever is clicked first.
    doc.execute(link_command(doc.project(), &FromKey, a, b).unwrap())
        .unwrap();
    let x = XrefIndex::build(doc.project(), &FromKey);
    assert_eq!(x.label(a).unwrap().lines, ["2.1"]);
    assert_eq!(x.label(b).unwrap().lines, ["1.10"]);
    assert_eq!(
        x.partners(b).iter().map(|r| r.symbol).collect::<Vec<_>>(),
        [a]
    );

    // A named arrow right-clicked first takes the partner's name.
    doc.execute(link_command(doc.project(), &FromKey, c, a).unwrap())
        .unwrap();
    let x = XrefIndex::build(doc.project(), &FromKey);
    assert_eq!(x.all_references(c).len(), 3);
    assert!(x.issues().is_empty());
    assert_eq!(
        link_command(doc.project(), &FromKey, a, b),
        Err(LinkError::AlreadyLinked)
    );
}

#[test]
fn link_refuses_what_makes_no_sense() {
    let mut f = fixture();
    let page = f.pages[0];
    let ((k2_coil, arrow, wire_end), _) = f
        .doc
        .transact("Draw", |tx| {
            let k2 = tx.add_device(DeviceTag::parse("-K2")?, "")?;
            let k2_coil = coil(tx, page, 10.0, k2);
            let arrow = tx.place_symbol(
                page,
                "iec/connectivity/interruption",
                Placement::at(Point::new(50.0, 10.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Interruption { name: "X".into() },
            )?;
            let strip = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
            let t = tx.add_terminal(strip, "1")?;
            let term = tx.place_symbol(
                page,
                "iec/terminals/terminal",
                Placement::at(Point::new(90.0, 10.0)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 4.0)],
                SymbolRole::Terminal { terminals: vec![t] },
            )?;
            Ok((k2_coil, arrow, term))
        })
        .unwrap();
    let p = f.doc.project();
    let link = |a, b| link_command(p, &FromKey, a, b);
    assert_eq!(link(f.k1_coil, k2_coil), Err(LinkError::BothMain));
    assert_eq!(link(f.k1_coil, arrow), Err(LinkError::Mixed));
    assert_eq!(link(f.k1_coil, f.k1_coil), Err(LinkError::SameSymbol));
    assert_eq!(
        link(f.k1_coil, wire_end),
        Err(LinkError::NotLinkable(wire_end))
    );
    assert_eq!(
        link(f.k1_coil, f.k1_contacts[0]),
        Err(LinkError::AlreadyLinked)
    );
    assert!(!novo_xref::can_link(p, wire_end));
    assert!(novo_xref::can_link(p, arrow));
}

#[test]
fn builtin_library_drives_the_contact_image() {
    let lib = novo_symbols::Library::builtin().unwrap();
    let mut doc = Document::default();
    let (k1_coil, _) = doc
        .transact("Draw", |tx| {
            let p = tx.add_page("1")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1")?, "")?;
            let c = coil(tx, p, 10.0, k1);
            tx.place_symbol(
                p,
                "iec/contactors/main_3p",
                Placement::at(Point::new(100.0, 10.0)),
                ["1", "3", "5", "2", "4", "6"]
                    .iter()
                    .enumerate()
                    .map(|(i, n)| Pin::new(*n, i as f64 * 4.0, 0.0))
                    .collect(),
                SymbolRole::Device {
                    device: k1,
                    main: false,
                },
            )?;
            contact(tx, p, 200.0, "iec/contacts/nc", ["21", "22"], k1);
            Ok(c)
        })
        .unwrap();
    let x = XrefIndex::build(doc.project(), &lib);
    assert_eq!(
        x.label(k1_coil).unwrap().lines,
        ["1-2 3-4 5-6 1.3", "21-22 1.5"]
    );
    let image = x.contact_image(x.reference(k1_coil).unwrap().device().unwrap());
    assert_eq!(image[0].kind, ContactKind::Power);
}

/// 2,000 devices, each a coil and four contacts over 200 pages: 10,000
/// cross-referenced symbols, rebuilt well within a frame in release builds.
#[test]
fn ten_thousand_symbols() {
    let mut doc = Document::default();
    doc.transact("Big", |tx| {
        let pages: Vec<PageId> = (1..=200)
            .map(|n| tx.add_page(n.to_string()))
            .collect::<Result<_, _>>()?;
        for d in 0..2000usize {
            let k = tx.add_device(DeviceTag::product(format!("K{}", d + 1)), "")?;
            coil(tx, pages[d % 200], 20.0 * (d % 20) as f64, k);
            for c in 0..4usize {
                let page = pages[(d * 7 + c * 13) % 200];
                let o = c + 1;
                contact(
                    tx,
                    page,
                    20.0 * c as f64,
                    "iec/contacts/no",
                    [&format!("{o}3"), &format!("{o}4")],
                    k,
                );
            }
        }
        Ok(())
    })
    .unwrap();
    let start = Instant::now();
    let x = XrefIndex::build(doc.project(), &FromKey);
    let took = start.elapsed();
    let some = doc.project().devices().next().unwrap().id;
    assert_eq!(x.device_references(some).len(), 5);
    assert_eq!(x.contact_image(some).len(), 4);
    assert!(x.issues().is_empty());
    // Generous for debug builds on slow CI runners.
    assert!(took.as_secs_f64() < 2.0, "index build took {took:?}");
    eprintln!("xref index over 10k symbols: {took:?}");
}

#[test]
fn structure_prefix_only_when_it_differs() {
    let mut doc = Document::default();
    let ((coil_sym, near, far), _) = doc
        .transact("Draw", |tx| {
            let p1 = tx.add_page("1")?;
            let p2 = tx.add_page("2")?;
            let p3 = tx.add_page("3")?;
            for (p, f) in [(p1, "A1"), (p2, "A1"), (p3, "A2")] {
                let structure = DeviceTag::parse(&format!("={f}"))?;
                tx.update::<Page>(p, |page| page.structure = structure)?;
            }
            let k1 = tx.add_device(DeviceTag::parse("=A1-K1")?, "")?;
            let c = coil(tx, p1, 10.0, k1);
            let near = contact(tx, p2, 10.0, "iec/contacts/no", ["13", "14"], k1);
            let far = contact(tx, p3, 10.0, "iec/contacts/nc", ["21", "22"], k1);
            Ok((c, near, far))
        })
        .unwrap();
    let x = XrefIndex::build(doc.project(), &FromKey);
    assert_eq!(
        x.label(coil_sym).unwrap().lines,
        ["13-14 2.1", "21-22 =A2/3.1"]
    );
    assert_eq!(x.label(near).unwrap().lines, ["1.1"]);
    assert_eq!(x.label(far).unwrap().lines, ["=A1/1.1"]);
    assert_eq!(
        x.highlight(coil_sym).dock_hint(),
        "=A1-K1 elsewhere: NO 13-14 2.1, NC 21-22 =A2/3.1"
    );
}

#[test]
fn exact_tag_jumps_to_the_main_location() {
    let f = fixture();
    let p = f.doc.project();
    let x = XrefIndex::build(p, &FromKey);
    let jump = x.go_to_tag(p, "-K1").unwrap();
    assert_eq!(jump.device, f.k1);
    assert_eq!(jump.main.symbol, f.k1_coil);
    assert_eq!(jump.others.len(), 3);
    assert!(x.go_to_tag(p, "-K").is_none()); // only a prefix
    assert!(x.go_to_tag(p, "-Q9").is_none());
}

#[test]
fn link_renumbers_when_the_ordinal_is_taken() {
    // -K1 has 13-14, 21-22 and 33-34; a 23-24 shares no pin name with them,
    // but ordinal 2 is taken by the NC contact (EN 50005).
    let mut f = fixture();
    let page = f.pages[0];
    let (lone, _) = f
        .doc
        .transact("Place", |tx| {
            let k2 = tx.add_device(DeviceTag::parse("-K2")?, "")?;
            Ok(contact(tx, page, 20.0, "iec/contacts/no", ["23", "24"], k2))
        })
        .unwrap();
    let cmd = link_command(f.doc.project(), &FromKey, lone, f.k1_coil).unwrap();
    f.doc.execute(cmd).unwrap();
    let names: Vec<String> = f
        .doc
        .project()
        .symbol(lone)
        .unwrap()
        .pins
        .iter()
        .map(|p| p.name.clone())
        .collect();
    assert_eq!(names, ["43", "44"]);
}

#[test]
fn references_into_a_frozen_page_do_not_move() {
    // DESIGN.md §8: an issued page keeps the references printed on it and
    // pointing at it, until it is unfrozen.
    let mut f = fixture();
    let page3 = f.pages[2];
    let moved = f.k1_contacts[2];
    f.doc
        .execute(Command::FreezePage {
            page: page3,
            revision: "A".into(),
        })
        .unwrap();
    f.doc
        .transact("Move", |tx| {
            tx.move_symbol(moved, Placement::at(Point::new(400.0, 10.0)))
        })
        .unwrap();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    assert!(
        x.label(f.k1_coil)
            .unwrap()
            .lines
            .contains(&"33-34 3.4".to_owned())
    );

    f.doc
        .execute(Command::UnfreezePage { page: page3 })
        .unwrap();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    assert!(
        x.label(f.k1_coil)
            .unwrap()
            .lines
            .contains(&"33-34 3.10".to_owned())
    );
}

#[test]
fn link_to_a_named_device_is_one_undo_step() {
    let mut f = fixture();
    let page = f.pages[0];
    let (lone, _) = f
        .doc
        .transact("Place", |tx| {
            let k9 = tx.add_device(DeviceTag::parse("-K9")?, "")?;
            Ok(contact(tx, page, 20.0, "iec/contacts/no", ["23", "24"], k9))
        })
        .unwrap();
    let depth = f.doc.undo_depth();
    let cmd = link_to_device_command(f.doc.project(), &FromKey, lone, f.k1).unwrap();
    f.doc.execute(cmd).unwrap();
    assert_eq!(f.doc.undo_depth(), depth + 1);
    assert_eq!(f.doc.project().symbol(lone).unwrap().device(), Some(f.k1));
    assert!(
        f.doc
            .project()
            .devices_by_tag(&DeviceTag::parse("-K9").unwrap())
            .is_empty()
    );

    // A coil may not join a device that has one.
    let coil_sym = unassigned_coil(&mut f);
    assert_eq!(
        link_to_device_command(f.doc.project(), &FromKey, coil_sym, f.k1),
        Err(LinkError::DeviceHasMain)
    );
    assert_eq!(
        link_to_device_command(f.doc.project(), &FromKey, lone, f.k1),
        Err(LinkError::AlreadyLinked)
    );
    f.doc.undo().unwrap(); // the coil placed above
    f.doc.undo().unwrap(); // the link
    assert_ne!(f.doc.project().symbol(lone).unwrap().device(), Some(f.k1));
}

#[test]
fn interruption_points_are_numbered_automatically() {
    use novo_xref::{auto_name_interruptions, next_interruption_name};
    let mut doc = Document::default();
    let ((a, b, c), _) = doc
        .transact("Draw", |tx| {
            let p = tx.add_page("1")?;
            let arrow = |tx: &mut Tx<'_>, x, name: &str| {
                tx.place_symbol(
                    p,
                    "iec/connectivity/interruption",
                    Placement::at(Point::new(x, 10.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Interruption { name: name.into() },
                )
            };
            Ok((
                arrow(tx, 10.0, "2")?,
                arrow(tx, 50.0, "")?,
                arrow(tx, 90.0, "")?,
            ))
        })
        .unwrap();
    assert_eq!(next_interruption_name(doc.project()), "1");
    let (named, _) = doc.transact("Number", auto_name_interruptions).unwrap();
    assert_eq!(named, vec![b, c]);
    let name = |id| match &doc.project().symbol(id).unwrap().role {
        SymbolRole::Interruption { name } => name.clone(),
        _ => unreachable!(),
    };
    assert_eq!(
        (name(a).as_str(), name(b).as_str(), name(c).as_str()),
        ("2", "1", "3")
    );
    assert_eq!(next_interruption_name(doc.project()), "4");
}

#[test]
fn shift_g_list_leads_with_the_page_and_sorts_by_page() {
    let f = fixture();
    let x = XrefIndex::build(f.doc.project(), &FromKey);
    let nav = Navigator::default();
    // From the page 3 contact: the coil (page 2) is not first in `all_references`.
    let here = Place {
        page: f.pages[2],
        symbol: Some(f.k1_contacts[2]),
    };
    let reader = DeviceTag::default();
    let rows: Vec<String> = nav
        .all(&x, here)
        .iter()
        .map(|r| r.list_row_from(&reader, ""))
        .collect();
    assert_eq!(
        rows,
        [
            "2.2  \u{b7}  NO 13-14",
            "2.5  \u{b7}  A1-A2",
            "2.8  \u{b7}  NC 21-22",
            "3.4  \u{b7}  NO 33-34"
        ]
    );
    let r = nav.all(&x, here)[0];
    assert_eq!(
        r.list_row_from(&reader, "Power"),
        "2.2 Power  \u{b7}  NO 13-14"
    );
}
