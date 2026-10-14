//! Assigning potentials to wires and inferring them from terminals.

use novo_core::*;

fn two_pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)]
}

/// A page with `n` free wires, each between two pins of its own symbols.
fn wires(doc: &mut Document, n: usize) -> (PageId, Vec<WireId>, Vec<SymbolId>) {
    doc.transact("Wires", |tx| {
        let page = tx.add_page("1")?;
        let mut wires = Vec::new();
        let mut loads = Vec::new();
        for i in 0..n {
            let x = 10.0 * i as f64;
            let a = tx.place_symbol(
                page,
                "iec/general/box",
                Placement::at(Point::new(x, 0.0)),
                two_pins(),
                SymbolRole::Unassigned,
            )?;
            let b = tx.place_symbol(
                page,
                "iec/general/box",
                Placement::at(Point::new(x, 50.0)),
                two_pins(),
                SymbolRole::Unassigned,
            )?;
            wires.push(tx.connect(Endpoint::pin(a, 1), Endpoint::pin(b, 0))?);
            loads.push(b);
        }
        Ok((page, wires, loads))
    })
    .unwrap()
    .0
}

#[test]
fn three_wires_get_a_three_phase_set() {
    let mut doc = Document::default();
    let (_, wires, loads) = wires(&mut doc, 3);
    let set = PotentialSet::ThreePhase {
        voltage: 400.0,
        neutral: true,
        pe: true,
        pen: false,
    };
    let created = doc
        .execute(Command::AssignPotentials {
            wires: wires.clone(),
            potentials: set.specs(),
        })
        .unwrap();
    // L1, L2, L3 are created for the three wires; N and PE are not needed.
    assert_eq!(created.created.len(), 3);
    let project = doc.project();
    let names: Vec<&str> = project.potentials().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["L1", "L2", "L3"]);
    assert!(project.potentials().all(|p| p.voltage == Some(400.0)));
    let net = project.netlist();
    let name_at = |load: SymbolId| {
        let id = net.potential_of(Endpoint::pin(load, 0)).unwrap();
        project.potential(id).unwrap().name.clone()
    };
    assert_eq!(
        loads.iter().map(|&l| name_at(l)).collect::<Vec<_>>(),
        ["L1", "L2", "L3"]
    );

    // Assigning again reuses the potentials by name.
    let created = doc
        .execute(Command::AssignPotentials {
            wires: vec![wires[2], wires[1]],
            potentials: set.specs(),
        })
        .unwrap();
    assert!(created.created.is_empty());
    assert_eq!(doc.project().potentials().count(), 3);

    // More wires than potentials is refused.
    let err = doc
        .execute(Command::AssignPotentials {
            wires: wires.clone(),
            potentials: PotentialSet::Dc { voltage: 24.0 }.specs(),
        })
        .unwrap_err();
    assert!(matches!(
        err,
        ModelError::TooFewPotentials {
            wires: 3,
            potentials: 2
        }
    ));

    // Undo takes back the swap, then the first assignment with its potentials.
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.project().potentials().count(), 0);
    assert!(doc.project().wires().all(|w| w.potential.is_none()));
}

#[test]
fn deleting_a_potential_clears_its_wires() {
    let mut doc = Document::default();
    let (_, wires, _) = wires(&mut doc, 1);
    doc.execute(Command::AssignPotentials {
        wires: wires.clone(),
        potentials: vec![PotentialSpec::new(
            "24V",
            PotentialKind::DcPositive,
            Some(24.0),
        )],
    })
    .unwrap();
    let potential = doc.project().potentials().next().unwrap().id;
    assert_eq!(
        doc.project().wire(wires[0]).unwrap().potential,
        Some(potential)
    );
    doc.transact("Delete", |tx| tx.remove(potential)).unwrap();
    assert_eq!(doc.project().wire(wires[0]).unwrap().potential, None);
    assert!(doc.project().wire(wires[0]).is_some());

    // A wire cannot point at a potential that doesn't exist.
    let err = doc
        .execute(Command::SetWirePotential {
            wire: wires[0],
            potential: Some(potential),
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::NotFound(_)));
}

#[test]
fn terminals_named_like_a_supply_potential_define_it() {
    let mut doc = Document::default();
    let (page, wires, loads) = wires(&mut doc, 1);
    let ((pe, x4_pe, x4_1), _) = doc
        .transact("Terminals", |tx| {
            let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
            // A signal potential named "1" must not claim terminal 1.
            tx.add_potential("1", PotentialKind::Signal)?;
            let x4 = tx.add_terminal_strip(DeviceTag::parse("-X4")?)?;
            let mut blocks = Vec::new();
            for (i, name) in ["PE", "1"].into_iter().enumerate() {
                let terminal = tx.add_terminal(x4, name)?;
                blocks.push(tx.place_symbol(
                    page,
                    "iec/terminals/terminal",
                    Placement::at(Point::new(100.0 + 10.0 * i as f64, 0.0)),
                    two_pins(),
                    SymbolRole::Terminal {
                        terminals: vec![terminal],
                    },
                )?);
            }
            Ok((pe, blocks[0], blocks[1]))
        })
        .unwrap();
    let net = doc.project().netlist();
    // Both ends of the PE terminal are PE; terminal 1 has nothing.
    assert_eq!(net.potential_of(Endpoint::pin(x4_pe, 1)), Some(pe));
    assert_eq!(net.potential_of(Endpoint::pin(x4_1, 0)), None);
    assert_eq!(net.potential_conflicts().count(), 0);

    // The rule is explicit: off, the name means nothing (what files without
    // the rule load as), and turning it back on is one undo step.
    doc.transact("Names off", |tx| {
        tx.set_info(|info| info.rules.potentials.from_terminal_names = false)
    })
    .unwrap();
    let off = doc.project().netlist();
    assert_eq!(off.potential_of(Endpoint::pin(x4_pe, 1)), None);
    let mut loaded = doc.project().to_json();
    assert!(!loaded.contains("from_terminal_names"));
    loaded = Project::from_json(&loaded).unwrap().to_json();
    assert!(!loaded.contains("from_terminal_names"));
    doc.undo().unwrap();
    assert!(
        doc.project()
            .to_json()
            .contains("\"from_terminal_names\": true")
    );
    assert_eq!(
        doc.project()
            .netlist()
            .potential_of(Endpoint::pin(x4_pe, 1)),
        Some(pe)
    );

    // Wiring the PE terminal to a wire marked L1 is a short.
    doc.execute(Command::AssignPotentials {
        wires: wires.clone(),
        potentials: vec![PotentialSpec::new("L1", PotentialKind::Line, None)],
    })
    .unwrap();
    doc.transact("Short", |tx| {
        tx.connect(Endpoint::pin(loads[0], 0), Endpoint::pin(x4_pe, 0))
            .map(|_| ())
    })
    .unwrap();
    assert_eq!(doc.project().netlist().potential_conflicts().count(), 1);
}

#[test]
fn potential_sets_and_wires_round_trip_through_json() {
    let set = PotentialSet::ThreePhase {
        voltage: 400.0,
        neutral: false,
        pe: false,
        pen: true,
    };
    let names: Vec<String> = set.specs().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["L1", "L2", "L3", "PEN"]);
    let json = serde_json::to_string(&set).unwrap();
    assert_eq!(serde_json::from_str::<PotentialSet>(&json).unwrap(), set);
    let single: Vec<String> = PotentialSet::SinglePhase { voltage: 230.0 }
        .specs()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(single, ["L", "N", "PE"]);

    let mut doc = Document::default();
    let (_, wires, _) = wires(&mut doc, 1);
    let command: Command = serde_json::from_str(&format!(
        r#"{{"command": "assign_potentials", "wires": [{}], "potentials": [{{"name": "PE", "kind": "protective_earth"}}]}}"#,
        serde_json::to_string(&wires[0]).unwrap()
    ))
    .unwrap();
    doc.execute(command).unwrap();
    let loaded = Project::from_json(&doc.project().to_json()).unwrap();
    assert!(loaded.wire(wires[0]).unwrap().potential.is_some());
}

#[test]
fn a_source_declares_its_system_once() {
    let mut doc = Document::default();
    let (page, wires, _) = wires(&mut doc, 3);
    let created = doc
        .execute(Command::AddPotentialSystem {
            name: "400 V 3/N/PE AC".into(),
            set: PotentialSet::ThreePhase {
                voltage: 400.0,
                neutral: true,
                pe: true,
                pen: false,
            },
            frequency_hz: Some(50.0),
        })
        .unwrap()
        .created;
    let AnyId::PotentialSystem(system) = created[0] else {
        panic!("{created:?}")
    };
    assert_eq!(created.len(), 6, "the system and its five potentials");
    let project = doc.project();
    let members = project.potential_system(system).unwrap().members.clone();
    let names: Vec<&str> = members
        .iter()
        .map(|&p| project.potential(p).unwrap().name.as_str())
        .collect();
    assert_eq!(names, ["L1", "L2", "L3", "N", "PE"]);
    assert_eq!(project.system_of(members[0]).unwrap().id, system);
    assert_eq!(
        project.potential_system(system).unwrap().voltage,
        Some(400.0)
    );

    // A supply symbol with five outputs, wired to the three loads' wires.
    let (supply, _) = doc
        .transact("Supply", |tx| {
            let pins = (0..5)
                .map(|i| Pin::new(format!("{i}"), 10.0 * f64::from(i), 0.0))
                .collect();
            let supply = tx.place_symbol(
                page,
                "iec/sources/supply",
                Placement::at(Point::new(0.0, -50.0)),
                pins,
                SymbolRole::Unassigned,
            )?;
            for (i, &wire) in wires.iter().enumerate() {
                let start = tx.project().wire(wire).unwrap().from;
                tx.connect(Endpoint::pin(supply, i as u16), start)?;
            }
            Ok(supply)
        })
        .unwrap();
    doc.execute(Command::FeedSystem {
        symbol: supply,
        pins: vec![0, 1, 2, 3, 4],
        system,
    })
    .unwrap();
    let net = doc.project().netlist();
    for (i, &wire) in wires.iter().enumerate() {
        let start = doc.project().wire(wire).unwrap().from;
        assert_eq!(net.potential_of(start), Some(members[i]));
    }
    assert_eq!(net.potential_conflicts().count(), 0);

    // More pins than the system has potentials is refused.
    assert!(
        doc.execute(Command::FeedSystem {
            symbol: supply,
            pins: vec![0, 1, 2, 3, 4, 5],
            system,
        })
        .is_err()
    );
    // A pin outside the symbol is refused.
    assert!(
        doc.execute(Command::SetPinFeed {
            symbol: supply,
            pin: 9,
            potential: Some(members[0]),
        })
        .is_err()
    );

    // Deleting a potential removes it from the system and the source, not
    // the source itself; the save keeps the rest.
    doc.transact("Drop N", |tx| tx.remove(members[3])).unwrap();
    let project = doc.project();
    assert_eq!(project.potential_system(system).unwrap().members.len(), 4);
    let feeds = &project.symbol(supply).unwrap().feeds;
    assert_eq!(feeds.len(), 4);
    assert!(feeds.iter().all(|f| f.potential != members[3]));
    let loaded = Project::from_json(&project.to_json()).unwrap();
    assert_eq!(loaded.to_json(), project.to_json());
}

#[test]
fn a_supply_terminal_feeds_its_net() {
    let mut doc = Document::new(Project::default());
    let (page, _, _) = wires(&mut doc, 0);
    let ((terminal, block, l1), _) = doc
        .transact("Incoming", |tx| {
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let x0 = tx.add_terminal_strip(DeviceTag::parse("-X0")?)?;
            let terminal = tx.add_terminal(x0, "1")?;
            let block = tx.place_symbol(
                page,
                "iec/terminals/terminal",
                Placement::at(Point::new(0.0, 0.0)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                SymbolRole::Terminal {
                    terminals: vec![terminal],
                },
            )?;
            Ok((terminal, block, l1))
        })
        .unwrap();
    let potential_at = |doc: &Document| {
        doc.project()
            .netlist()
            .potential_of(Endpoint::pin(block, 0))
    };
    assert_eq!(potential_at(&doc), None);
    doc.execute(Command::SetTerminalFeed {
        terminal,
        potential: Some(l1),
    })
    .unwrap();
    assert_eq!(potential_at(&doc), Some(l1));
    doc.transact("Drop L1", |tx| tx.remove(l1)).unwrap();
    assert_eq!(doc.project().terminal(terminal).unwrap().feed, None);
}
