use novo_cables::{
    CableType, Conductor, Finding, ShieldConnection, accept_proposals, add_shield, assign_core,
    cable_screen, check, check_with, connect_shield, dismiss_proposal, overview, propose_cables,
    restore_proposal, set_spare, summarize_proposals, swap_cores, unassign_core,
};
use novo_core::{
    CableId, Command, DeviceKind, DeviceTag, Document, Endpoint, Pin, Placement, Point,
    PotentialKind, SymbolRole, Tx, WireId,
};
use novo_parts::Catalog;
use novo_tests::motor_starter;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

/// The Gate 1 project without its hand-made cable `-W1`.
fn gate1_without_cable() -> (Document, novo_tests::MotorStarter) {
    let mut doc = Document::default();
    let m = motor_starter(&mut doc).unwrap();
    doc.execute(Command::Remove { id: m.w1.into() }).unwrap();
    (doc, m)
}

fn core_names(doc: &Document, cable: CableId) -> Vec<(String, Option<WireId>)> {
    doc.project()
        .cable(cable)
        .unwrap()
        .cores
        .iter()
        .map(|c| (c.name.clone(), c.wire))
        .collect()
}

#[test]
fn gate1_motor_cable_is_proposed_as_4g_and_accepted() {
    let (mut doc, m) = gate1_without_cable();
    let proposals = propose_cables(doc.project());
    assert_eq!(proposals.len(), 1, "{proposals:?}");
    let p = &proposals[0];
    assert_eq!(p.tag, tag("=A1-W1"));
    assert_eq!(p.cable_type.name(), "4G1.5");
    assert_eq!(p.ends[0].device, m.x1, "the cabinet side is the strip");
    assert_eq!(p.ends[0].location, ["CAB"]);
    assert_eq!(p.ends[1].device, m.m1);
    assert_eq!(p.ends[1].location, ["FLD"]);
    assert_eq!(p.wires, m.motor_wires);
    assert_eq!(
        p.conductors,
        [
            Conductor::Line("L1".into()),
            Conductor::Line("L2".into()),
            Conductor::Line("L3".into()),
            Conductor::Protective,
        ]
    );
    assert_eq!(summarize_proposals(&proposals), "1 cable: =A1-W1 4G1.5");

    let depth = doc.undo_depth();
    let catalog = Catalog::starter();
    let (cables, _) = doc
        .transact("Accept cables", |tx| {
            accept_proposals(tx, &proposals, Some(&catalog))
        })
        .unwrap();
    assert_eq!(doc.undo_depth(), depth + 1, "one undo step");
    let cable = cables[0];
    let w = m.motor_wires;
    assert_eq!(
        core_names(&doc, cable),
        [
            ("BN".to_owned(), Some(w[0])),
            ("BK".to_owned(), Some(w[1])),
            ("GY".to_owned(), Some(w[2])),
            ("GNYE".to_owned(), Some(w[3])),
        ]
    );
    let c = doc.project().cable(cable).unwrap();
    let part = doc.project().part(c.part.unwrap()).unwrap();
    assert_eq!(part.order_number, "1119304", "the fixture's part is reused");
    assert_eq!(
        doc.project().parts().count(),
        10,
        "no second copy of the part"
    );
    assert!(propose_cables(doc.project()).is_empty());
    assert!(
        check(doc.project()).is_empty(),
        "{:?}",
        check(doc.project())
    );

    doc.undo().unwrap();
    assert_eq!(doc.project().cables().count(), 0);
    assert_eq!(propose_cables(doc.project()).len(), 1);
}

#[test]
fn declined_proposals_stay_declined() {
    let (mut doc, m) = gate1_without_cable();
    let proposals = propose_cables(doc.project());
    doc.transact("Decline cable", |tx| dismiss_proposal(tx, &proposals[0]))
        .unwrap();
    assert!(propose_cables(doc.project()).is_empty());
    doc.transact("Restore", |tx| restore_proposal(tx, &m.motor_wires))
        .unwrap();
    assert_eq!(propose_cables(doc.project()), proposals);
    doc.undo().unwrap();
    assert!(propose_cables(doc.project()).is_empty());
}

#[test]
fn gate1_cable_screen_shows_both_ends_of_every_core() {
    let mut doc = Document::default();
    let m = motor_starter(&mut doc).unwrap();
    let screen = cable_screen(doc.project(), m.w1).unwrap();
    assert_eq!(screen.tag, "=A1-W1");
    assert_eq!(screen.type_name, "4G1.5");
    assert_eq!(screen.part.as_deref(), Some("Lapp 1119304"));
    assert_eq!((screen.from.as_str(), screen.to.as_str()), ("+CAB", "+FLD"));
    let expected = [
        ("BK", "=A1+CAB-X1:1", "=A1+FLD-M1:U1", "L1"),
        ("BN", "=A1+CAB-X1:2", "=A1+FLD-M1:V1", "L2"),
        ("GY", "=A1+CAB-X1:3", "=A1+FLD-M1:W1", "L3"),
        ("GNYE", "=A1+CAB-X1:PE", "=A1+FLD-M1:PE", "PE"),
    ];
    assert_eq!(screen.rows.len(), expected.len());
    for (row, (core, a, b, potential)) in screen.rows.iter().zip(expected) {
        assert_eq!(row.core, core);
        let [ea, eb] = &row.ends;
        let (ea, eb) = (ea.as_ref().unwrap(), eb.as_ref().unwrap());
        assert_eq!((ea.target.as_str(), ea.location.as_str()), (a, "+CAB"));
        assert_eq!((eb.target.as_str(), eb.location.as_str()), (b, "+FLD"));
        assert_eq!(row.potential.as_deref(), Some(potential));
    }
    let text = screen.to_text();
    assert!(text.starts_with("=A1-W1 4G1.5 +CAB -> +FLD 12 m\n"));
    assert!(text.contains("=A1+CAB-X1:PE"), "{text}");

    let rows = overview(doc.project());
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!((r.used, r.cores, r.spare), (4, 4, 0));
    assert_eq!((r.from.as_str(), r.to.as_str()), ("+CAB", "+FLD"));
    assert_eq!(r.length_m, Some(12.0));
}

/// A strip `-X1` in `+CAB` with terminals `names` and a field device in
/// `+FLD` with pins `pins`, wired terminal i to pin i in the given order.
struct Small {
    doc: Document,
    wires: Vec<WireId>,
}

fn small(names: &[&str], field: &str, pins: &[&str]) -> Small {
    let mut doc = Document::default();
    let wires = doc
        .transact("Draw", |tx: &mut Tx<'_>| {
            let page = tx.add_page("1")?;
            let strip = tx.add_terminal_strip(tag("=A1+CAB-X1"))?;
            tx.add_device(tag("=A1+CAB-K1"), "")?;
            let dev = tx.add_device_of_kind(tag(field), "", DeviceKind::General)?;
            let field_pins: Vec<Pin> = pins
                .iter()
                .enumerate()
                .map(|(i, n)| Pin::new(*n, i as f64 * 8.0, 0.0))
                .collect();
            let fs = tx.place_symbol(
                page,
                "iec/general",
                Placement::at(Point::new(40.0, 200.0)),
                field_pins,
                SymbolRole::Device {
                    device: dev,
                    main: true,
                },
            )?;
            let mut wires = Vec::new();
            for (i, name) in names.iter().enumerate() {
                let t = tx.add_terminal(strip, *name)?;
                let s = tx.place_symbol(
                    page,
                    "iec/terminals/terminal",
                    Placement::at(Point::new(40.0 + i as f64 * 8.0, 100.0)),
                    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                    SymbolRole::Terminal { terminals: vec![t] },
                )?;
                let pin = u16::try_from(i).unwrap();
                wires.push(tx.connect(Endpoint::pin(s, 1), Endpoint::pin(fs, pin))?);
            }
            Ok(wires)
        })
        .unwrap()
        .0;
    Small { doc, wires }
}

#[test]
fn core_order_is_stable_whatever_the_drawing_order() {
    // Drawn PE, L3, N, L2, L1: the cable still reads L1 L2 L3 N PE.
    let mut s = small(
        &["PE", "3", "N", "2", "1"],
        "=A1+FLD-M2",
        &["PE", "W1", "N", "V1", "U1"],
    );
    let names = ["PE", "L3", "N", "L2", "L1"];
    let kinds = [
        PotentialKind::ProtectiveEarth,
        PotentialKind::Line,
        PotentialKind::Neutral,
        PotentialKind::Line,
        PotentialKind::Line,
    ];
    let wires = s.wires.clone();
    s.doc
        .transact("Potentials", |tx| {
            for ((w, name), kind) in wires.iter().zip(names).zip(kinds) {
                let p = tx.add_potential(name, kind)?;
                tx.set_wire_potential(*w, Some(p))?;
            }
            Ok(())
        })
        .unwrap();
    let proposals = propose_cables(s.doc.project());
    assert_eq!(proposals.len(), 1);
    let p = &proposals[0];
    assert_eq!(p.cable_type.name(), "5G1.5");
    let order = [4, 3, 1, 2, 0].map(|i| wires[i]);
    assert_eq!(p.wires, order);
    let (cables, _) = s
        .doc
        .transact("Accept", |tx| accept_proposals(tx, &proposals, None))
        .unwrap();
    let cores = core_names(&s.doc, cables[0]);
    let by_core = |name: &str| cores.iter().find(|(n, _)| n == name).unwrap().1;
    assert_eq!(by_core("BN"), Some(wires[4]), "L1");
    assert_eq!(by_core("BK"), Some(wires[3]), "L2");
    assert_eq!(by_core("GY"), Some(wires[1]), "L3");
    assert_eq!(by_core("BU"), Some(wires[2]), "N");
    assert_eq!(by_core("GNYE"), Some(wires[0]), "PE");
    assert!(s.doc.project().cable(cables[0]).unwrap().part.is_none());
}

#[test]
fn signals_get_a_numbered_cable_with_spares() {
    let mut s = small(
        &["5", "6", "7", "8", "9", "10"],
        "=A1+FLD-B1",
        &["1", "2", "3", "4", "5", "6"],
    );
    let proposals = propose_cables(s.doc.project());
    let p = &proposals[0];
    assert_eq!(p.cable_type.name(), "7x1");
    assert_eq!(p.cores, [0, 1, 2, 3, 4, 5]);
    let (cables, _) = s
        .doc
        .transact("Accept", |tx| accept_proposals(tx, &proposals, None))
        .unwrap();
    let rows = overview(s.doc.project());
    assert_eq!((rows[0].used, rows[0].cores), (6, 7));
    assert_eq!(rows[0].type_name, "7x1");
    let screen = cable_screen(s.doc.project(), cables[0]).unwrap();
    assert!(screen.rows[6].wire.is_none());
    assert!(screen.to_text().contains("free"), "{}", screen.to_text());
}

#[test]
fn a_shielded_variant_can_be_chosen_before_accepting() {
    let s = small(&["1", "2", "PE"], "=A1+FLD-M3", &["U1", "V1", "PE"]);
    let mut proposals = propose_cables(s.doc.project());
    let p = &mut proposals[0];
    assert_eq!(p.cable_type.name(), "3G1.5");
    assert!(
        !p.retype(CableType::numbered(3, false, 1.5)),
        "PE needs GNYE"
    );
    assert!(p.retype(CableType::standard(4, true, 1.5).shielded()));
    assert_eq!(p.cable_type.name(), "4G1.5 shielded");
}

#[test]
fn core_edits_and_checks() {
    let mut s = small(&["1", "2", "PE"], "=A1+FLD-M3", &["U1", "V1", "PE"]);
    let proposals = propose_cables(s.doc.project());
    let (cables, _) = s
        .doc
        .transact("Accept", |tx| accept_proposals(tx, &proposals, None))
        .unwrap();
    let cable = cables[0];
    let w = s.wires.clone();
    // Without potentials U1 and V1 are signals: a numbered 3G cable.
    let names: Vec<String> = core_names(&s.doc, cable).into_iter().map(|c| c.0).collect();
    assert_eq!(names, ["1", "2", "GNYE"]);
    assert!(check(s.doc.project()).is_empty());

    // Swapping PE onto core 1 is flagged both ways.
    s.doc
        .transact("Swap", |tx| swap_cores(tx, cable, 0, 2))
        .unwrap();
    let findings = check(s.doc.project());
    let netlist = s.doc.project().netlist();
    assert_eq!(check_with(s.doc.project(), &netlist), findings);
    assert!(findings.contains(&Finding::ProtectiveOnOtherCore {
        cable,
        core: 0,
        wire: w[2]
    }));
    assert!(findings.contains(&Finding::ProtectiveCoreMisused {
        cable,
        core: 2,
        wire: w[0]
    }));
    s.doc.undo().unwrap();

    // A shield floats until it is connected.
    let (shield, _) = s
        .doc
        .transact("Shield", |tx| add_shield(tx, cable))
        .unwrap();
    assert_eq!(
        check(s.doc.project()),
        [Finding::ShieldUnconnected {
            cable,
            core: shield
        }]
    );
    s.doc
        .transact("Shield to PE", |tx| {
            connect_shield(tx, cable, shield, Some(ShieldConnection::Pe))
        })
        .unwrap();
    assert!(check(s.doc.project()).is_empty());
    let screen = cable_screen(s.doc.project(), cable).unwrap();
    assert_eq!(screen.rows[shield].ends[0].as_ref().unwrap().target, "PE");
    let strip_terminal = s.doc.project().terminals().next().unwrap().id;
    s.doc
        .transact("Shield to terminal", |tx| {
            connect_shield(
                tx,
                cable,
                shield,
                Some(ShieldConnection::Terminal(strip_terminal)),
            )
        })
        .unwrap();
    assert_eq!(
        novo_cables::shield_connection(s.doc.project().cable(cable).unwrap(), shield),
        Some(ShieldConnection::Terminal(strip_terminal))
    );
    assert!(check(s.doc.project()).is_empty());
    // Deleting that terminal leaves the shield floating again.
    s.doc
        .transact("Delete terminal", |tx| tx.remove(strip_terminal))
        .unwrap();
    assert!(
        check(s.doc.project()).contains(&Finding::ShieldUnconnected {
            cable,
            core: shield
        })
    );
    s.doc.undo().unwrap();
    assert!(check(s.doc.project()).is_empty());
    assert!(
        s.doc
            .transact("Not a shield", |tx| connect_shield(
                tx,
                cable,
                0,
                Some(ShieldConnection::Pe)
            ))
            .is_err()
    );

    // A spare core with a wire on it; then too few cores for the wires.
    s.doc
        .transact("Spare", |tx| set_spare(tx, cable, 1, true))
        .unwrap();
    assert_eq!(
        check(s.doc.project()),
        [Finding::SpareInUse { cable, core: 1 }]
    );
    s.doc
        .transact("Free core", |tx| unassign_core(tx, cable, 1))
        .unwrap();
    assert_eq!(
        check(s.doc.project()),
        [Finding::CoreCountExceeded {
            cable,
            unassigned: vec![w[1]],
            free: 0
        }]
    );
    let overview_row = &overview(s.doc.project())[0];
    assert_eq!((overview_row.used, overview_row.spare), (2, 1));
    // Assigning moves a wire within the cable.
    s.doc
        .transact("Spare off", |tx| {
            set_spare(tx, cable, 1, false)?;
            assign_core(tx, cable, 1, w[0])
        })
        .unwrap();
    let cores = core_names(&s.doc, cable);
    assert_eq!(cores[0].1, None);
    assert_eq!(cores[1].1, Some(w[0]));
}

#[test]
fn a_cable_is_retyped_renamed_and_deleted_in_one_undo_step_each() {
    let (mut doc, m) = gate1_without_cable();
    let proposals = propose_cables(doc.project());
    let (cables, _) = doc
        .transact("Accept", |tx| accept_proposals(tx, &proposals, None))
        .unwrap();
    let cable = cables[0];
    let w = m.motor_wires;

    // Retype to a shielded 5-core: the wires land on their usual cores.
    let depth = doc.undo_depth();
    let five = CableType::standard(5, true, 1.5).shielded();
    doc.transact("Retype", |tx| novo_cables::retype_cable(tx, cable, &five))
        .unwrap();
    assert_eq!(doc.undo_depth(), depth + 1);
    let cores = core_names(&doc, cable);
    assert_eq!(cores.len(), 6, "five conductors and the shield");
    assert_eq!(cores.iter().filter(|c| c.1.is_some()).count(), 4);
    assert_eq!(
        doc.project().cable(cable).unwrap().description,
        "5G1.5 shielded"
    );
    // A type the wires do not fit changes nothing.
    let before = doc.project().cable(cable).unwrap().clone();
    assert!(
        doc.transact("Retype", |tx| {
            novo_cables::retype_cable(tx, cable, &CableType::numbered(3, false, 1.5))
        })
        .is_err()
    );
    assert_eq!(doc.project().cable(cable).unwrap(), &before);
    doc.undo().unwrap();
    assert_eq!(core_names(&doc, cable).len(), 4);
    assert_eq!(core_names(&doc, cable)[3], ("GNYE".to_owned(), Some(w[3])));

    // Rename; a tag another cable has is refused.
    doc.transact("Rename", |tx| {
        novo_cables::rename_cable(tx, cable, tag("=A1-W7"))
    })
    .unwrap();
    assert_eq!(doc.project().cable(cable).unwrap().tag, tag("=A1-W7"));
    let (other, _) = doc
        .transact("Other", |tx| tx.add_cable(tag("=A1-W8"), Vec::new()))
        .unwrap();
    assert!(
        doc.transact("Clash", |tx| novo_cables::rename_cable(
            tx,
            other,
            tag("=A1-W7")
        ))
        .is_err()
    );

    // Delete frees the wires, which are proposed again; undo restores it.
    doc.transact("Delete", |tx| novo_cables::delete_cable(tx, cable))
        .unwrap();
    assert!(doc.project().cable(cable).is_none());
    assert_eq!(propose_cables(doc.project()).len(), 1);
    doc.undo().unwrap();
    assert_eq!(core_names(&doc, cable)[0].1, Some(w[0]));
}

#[test]
fn retype_keeps_connected_shields_and_refuses_to_drop_them() {
    let (mut doc, _m) = gate1_without_cable();
    let proposals = propose_cables(doc.project());
    let (cables, _) = doc
        .transact("Accept", |tx| accept_proposals(tx, &proposals, None))
        .unwrap();
    let cable = cables[0];
    let (sh, _) = doc.transact("Sh", |tx| add_shield(tx, cable)).unwrap();
    let (sh2, _) = doc.transact("Sh2", |tx| add_shield(tx, cable)).unwrap();
    doc.transact("Connect", |tx| {
        connect_shield(tx, cable, sh, Some(ShieldConnection::Pe))?;
        connect_shield(tx, cable, sh2, Some(ShieldConnection::Pe))
    })
    .unwrap();

    // To an unshielded type: refused, nothing changes.
    let before = doc.project().cable(cable).unwrap().clone();
    let plain = CableType::standard(4, true, 1.5);
    assert!(
        doc.transact("Retype", |tx| novo_cables::retype_cable(tx, cable, &plain))
            .is_err()
    );
    assert_eq!(doc.project().cable(cable).unwrap(), &before);

    // To a shielded type: both shields and their connections stay.
    let shielded = CableType::standard(4, true, 1.5).shielded();
    doc.transact("Retype", |tx| {
        novo_cables::retype_cable(tx, cable, &shielded)
    })
    .unwrap();
    let c = doc.project().cable(cable).unwrap();
    let shields: Vec<usize> = (0..c.cores.len()).filter(|&i| c.cores[i].shield).collect();
    assert_eq!(shields.len(), 2);
    for i in shields {
        assert_eq!(
            novo_cables::shield_connection(c, i),
            Some(ShieldConnection::Pe)
        );
    }

    // A shield that is not connected may go with an unshielded type, and no
    // stale shield property is left.
    let mut doc2 = doc;
    doc2.transact("Disconnect", |tx| {
        let c = tx.project().cable(cable).unwrap();
        let idx: Vec<usize> = (0..c.cores.len()).filter(|&i| c.cores[i].shield).collect();
        for i in idx {
            connect_shield(tx, cable, i, None)?;
        }
        Ok(())
    })
    .unwrap();
    doc2.transact("Retype", |tx| novo_cables::retype_cable(tx, cable, &plain))
        .unwrap();
    let c = doc2.project().cable(cable).unwrap();
    assert!(c.cores.iter().all(|x| !x.shield));
    assert!(c.properties.keys().all(|k| !k.starts_with("cable.shield.")));
}

#[test]
fn declined_wires_are_listed_until_restored() {
    let (mut doc, m) = gate1_without_cable();
    let proposals = propose_cables(doc.project());
    assert!(novo_cables::declined_wires(doc.project()).is_empty());
    doc.transact("Decline", |tx| dismiss_proposal(tx, &proposals[0]))
        .unwrap();
    assert_eq!(
        novo_cables::declined_wires(doc.project()).len(),
        m.motor_wires.len()
    );
    let declined = novo_cables::declined_wires(doc.project());
    doc.transact("Restore", |tx| restore_proposal(tx, &declined))
        .unwrap();
    assert!(novo_cables::declined_wires(doc.project()).is_empty());
}
