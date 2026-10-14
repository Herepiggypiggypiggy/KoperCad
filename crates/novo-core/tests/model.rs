//! End-to-end tests of the model API: building circuits, integrity rules,
//! connectivity, undo/redo, commands and serialization.

use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

/// The project as JSON without `next_id`, which undo deliberately does not
/// roll back (ids are never reused).
fn state(project: &Project) -> String {
    project
        .to_json()
        .lines()
        .filter(|l| !l.trim_start().starts_with("\"next_id\""))
        .collect::<Vec<_>>()
        .join("\n")
}

fn two_pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, -5.0), Pin::new("2", 0.0, 5.0)]
}

fn at(x: f64, y: f64) -> Placement {
    Placement::at(Point::new(x, y))
}

/// Handles to the objects of the Gate 1 control circuit.
struct Starter {
    page1: PageId,
    page2: PageId,
    k1: DeviceId,
    coil: SymbolId,
    self_hold: SymbolId,
    stop: SymbolId,
    start: SymbolId,
    l: PotentialId,
    n: PotentialId,
    x1: DeviceId,
    x1_1: SymbolId,
    w_start_coil: WireId,
}

/// A direct-on-line starter control circuit: L -> -X1:1 -> -S0 (stop) ->
/// -S1 (start) parallel with -K1 13/14 (self-hold) -> -K1 coil A1/A2 -> N,
/// with the coil continued to page 2 through an interruption point.
fn build_starter(doc: &mut Document) -> Starter {
    let (s, _) = doc
        .transact("Draw starter", |tx| {
            let page1 = tx.add_page("1")?;
            let page2 = tx.add_page("2")?;
            let l = tx.add_potential("L", PotentialKind::Line)?;
            let n = tx.add_potential("N", PotentialKind::Neutral)?;
            let k1 = tx.add_device(tag("=A1-K1"), "Main contactor")?;
            let s0 = tx.add_device(tag("=A1-S0"), "Stop")?;
            let s1 = tx.add_device(tag("=A1-S1"), "Start")?;
            let x1 = tx.add_terminal_strip(tag("=A1-X1"))?;
            let t1 = tx.add_terminal(x1, "1")?;

            let feed_l = tx.place_symbol(
                page1,
                "iec/potential",
                at(10.0, 10.0),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::PotentialFeed { potential: l },
            )?;
            let x1_1 = tx.place_symbol(
                page1,
                "iec/terminal",
                at(10.0, 30.0),
                two_pins(),
                SymbolRole::Terminal {
                    terminals: vec![t1],
                },
            )?;
            let stop = tx.place_symbol(
                page1,
                "iec/contact_nc",
                at(10.0, 60.0),
                vec![Pin::new("11", 0.0, -5.0), Pin::new("12", 0.0, 5.0)],
                SymbolRole::Device {
                    device: s0,
                    main: true,
                },
            )?;
            let start = tx.place_symbol(
                page1,
                "iec/contact_no",
                at(10.0, 90.0),
                vec![Pin::new("13", 0.0, -5.0), Pin::new("14", 0.0, 5.0)],
                SymbolRole::Device {
                    device: s1,
                    main: true,
                },
            )?;
            let self_hold = tx.place_symbol(
                page1,
                "iec/contact_no",
                at(30.0, 90.0),
                vec![Pin::new("13", 0.0, -5.0), Pin::new("14", 0.0, 5.0)],
                SymbolRole::Device {
                    device: k1,
                    main: false,
                },
            )?;
            let to_page2 = tx.place_symbol(
                page1,
                "iec/interruption",
                at(10.0, 120.0),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Interruption {
                    name: "K1.A1".into(),
                },
            )?;
            let from_page1 = tx.place_symbol(
                page2,
                "iec/interruption",
                at(10.0, 10.0),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Interruption {
                    name: "K1.A1".into(),
                },
            )?;
            let coil = tx.place_symbol(
                page2,
                "iec/coil",
                at(10.0, 40.0),
                vec![Pin::new("A1", 0.0, -5.0), Pin::new("A2", 0.0, 5.0)],
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            )?;
            let feed_n = tx.place_symbol(
                page2,
                "iec/potential",
                at(10.0, 70.0),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::PotentialFeed { potential: n },
            )?;

            tx.connect(Endpoint::pin(feed_l, 0), Endpoint::pin(x1_1, 0))?;
            tx.connect(Endpoint::pin(x1_1, 1), Endpoint::pin(stop, 0))?;
            // Stop feeds both the start button and the self-hold contact.
            let split = tx.add_junction(page1, Point::new(10.0, 75.0))?;
            tx.connect(Endpoint::pin(stop, 1), split)?;
            tx.connect(split, Endpoint::pin(start, 0))?;
            tx.connect(split, Endpoint::pin(self_hold, 0))?;
            let join = tx.add_junction(page1, Point::new(10.0, 105.0))?;
            let w_start_coil = tx.connect(Endpoint::pin(start, 1), join)?;
            tx.connect(Endpoint::pin(self_hold, 1), join)?;
            tx.connect(join, Endpoint::pin(to_page2, 0))?;
            tx.connect(Endpoint::pin(from_page1, 0), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(feed_n, 0))?;
            Ok(Starter {
                page1,
                page2,
                k1,
                coil,
                self_hold,
                stop,
                start,
                l,
                n,
                x1,
                x1_1,
                w_start_coil,
            })
        })
        .unwrap();
    s
}

#[test]
fn starter_connectivity() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let p = doc.project();
    let net = p.netlist();

    // A terminal conducts end to end, so L reaches the stop button.
    assert_eq!(net.potential_of(Endpoint::pin(s.stop, 0)), Some(s.l));
    assert!(net.connected(Endpoint::pin(s.x1_1, 0), Endpoint::pin(s.x1_1, 1)));
    // Contacts do not conduct in the netlist: stop 11 and 12 are separate.
    assert!(!net.connected(Endpoint::pin(s.stop, 0), Endpoint::pin(s.stop, 1)));
    // Start and self-hold are in parallel.
    assert!(net.connected(Endpoint::pin(s.start, 0), Endpoint::pin(s.self_hold, 0)));
    assert!(net.connected(Endpoint::pin(s.start, 1), Endpoint::pin(s.self_hold, 1)));
    // The interruption point carries the net to the coil on page 2.
    let a1 = net.net_of(Endpoint::pin(s.coil, 0)).unwrap();
    assert!(net.connected(Endpoint::pin(s.start, 1), Endpoint::pin(s.coil, 0)));
    assert_eq!(net.pages_of_net(p, a1), vec![s.page1, s.page2]);
    assert_eq!(net.potential_of(Endpoint::pin(s.coil, 1)), Some(s.n));
    assert_eq!(net.potential_conflicts().count(), 0);
    assert_eq!(
        net.devices_on_net(p, a1),
        vec![s.k1, p.symbol(s.start).unwrap().device().unwrap()]
    );
    assert_eq!(net.nets_with_potential(s.l).count(), 1);
    assert!(net.is_current(p));

    // Every pin of the circuit is wired.
    assert!(p.unconnected_pins().is_empty());

    // Cross-reference style queries.
    let k1_symbols = p.symbols_of_device(s.k1);
    assert_eq!(k1_symbols.len(), 2);
    assert_eq!(k1_symbols[0].id, s.coil, "main function comes first");
    assert_eq!(
        p.pin_designation(PinRef::new(s.self_hold, 1)).as_deref(),
        Some("=A1-K1:14")
    );
    assert_eq!(
        p.pin_designation(PinRef::new(s.x1_1, 0)).as_deref(),
        Some("=A1-X1:1")
    );
    let contents = p.page_contents(s.page2);
    assert_eq!(contents.symbols.len(), 3);
    assert_eq!(contents.wires.len(), 2);
    assert!(contents.junctions.is_empty());
    assert_eq!(
        p.wires_at(Endpoint::pin(s.start, 1)).collect::<Vec<_>>(),
        vec![s.w_start_coil]
    );
}

#[test]
fn shorting_two_potentials_is_reported() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    // Wire L straight across to N on page 2 by bridging the coil.
    doc.transact("Short", |tx| {
        tx.update::<SymbolInstance>(s.coil, |c| c.bridges = vec![vec![0, 1]])
    })
    .unwrap();
    let net = doc.project().netlist();
    // Not a short yet: the stop and start contacts are open in the netlist.
    assert_eq!(net.potential_conflicts().count(), 0);
    doc.transact("Bypass", |tx| {
        tx.update::<SymbolInstance>(s.stop, |c| c.bridges = vec![vec![0, 1]])?;
        tx.update::<SymbolInstance>(s.start, |c| c.bridges = vec![vec![0, 1]])
    })
    .unwrap();
    let net = doc.project().netlist();
    let conflicts: Vec<_> = net.potential_conflicts().collect();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].potentials, vec![s.l, s.n]);
    assert_eq!(conflicts[0].potential(), None);
}

#[test]
fn undo_and_redo_restore_exact_states() {
    let mut doc = Document::default();
    let empty = state(doc.project());
    let s = build_starter(&mut doc);
    let built = doc.project().to_json();

    doc.transact("Rename and move", |tx| {
        tx.set_tag(s.k1, tag("=A1-K7"))?;
        tx.move_symbol(s.coil, at(50.0, 50.0))?;
        tx.remove(s.stop)
    })
    .unwrap();
    let edited = doc.project().to_json();
    assert_ne!(edited, built);
    assert_eq!(doc.undo_label(), Some("Rename and move"));

    doc.undo().unwrap();
    assert_eq!(doc.project().to_json(), built);
    doc.undo().unwrap();
    assert_eq!(state(doc.project()), empty);
    assert_eq!(doc.undo().unwrap(), None);

    doc.redo().unwrap();
    assert_eq!(doc.project().to_json(), built);
    doc.redo().unwrap();
    assert_eq!(doc.project().to_json(), edited);
    assert_eq!(doc.redo().unwrap(), None);
}

#[test]
fn ids_are_never_reused() {
    let mut doc = Document::default();
    let (first, _) = doc.transact("a", |tx| tx.add_page("1")).unwrap();
    doc.undo().unwrap();
    let (second, _) = doc.transact("b", |tx| tx.add_page("1")).unwrap();
    assert_ne!(first, second);
    assert!(!doc.can_redo(), "a new edit clears redo");
}

#[test]
fn failed_edit_leaves_nothing_behind() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let before = state(doc.project());
    let revision = doc.project().revision();
    let depth = doc.undo_depth();

    let err = doc
        .transact("Bad", |tx| {
            tx.add_page("3")?;
            tx.remove(s.coil)?;
            // Pin 9 does not exist on the self-hold contact.
            tx.connect(Endpoint::pin(s.self_hold, 9), Endpoint::pin(s.start, 0))
        })
        .unwrap_err();
    assert_eq!(
        err,
        ModelError::InvalidPin {
            symbol: s.self_hold,
            pin: 9
        }
    );
    assert_eq!(state(doc.project()), before);
    assert_eq!(doc.project().revision(), revision);
    assert_eq!(doc.undo_depth(), depth);
}

#[test]
fn integrity_rules() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);

    // Wires stay on one page.
    let err = doc
        .transact("x", |tx| {
            tx.connect(Endpoint::pin(s.coil, 0), Endpoint::pin(s.start, 0))
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::WrongPage { .. }));

    // No wire from a pin to itself.
    let err = doc
        .transact("x", |tx| {
            tx.connect(Endpoint::pin(s.coil, 0), Endpoint::pin(s.coil, 0))
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::SelfConnection(_)));

    // A symbol cannot move to another page while wired.
    let err = doc
        .transact("x", |tx| {
            tx.update::<SymbolInstance>(s.coil, |c| c.page = s.page1)
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::WrongPage { .. }));

    // A wired pin cannot be dropped from a symbol.
    let err = doc
        .transact("x", |tx| {
            tx.update::<SymbolInstance>(s.coil, |c| c.pins.truncate(1))
        })
        .unwrap_err();
    assert_eq!(
        err,
        ModelError::InvalidPin {
            symbol: s.coil,
            pin: 1
        }
    );

    // Bridges must name existing pins.
    let err = doc
        .transact("x", |tx| {
            tx.update::<SymbolInstance>(s.coil, |c| c.bridges = vec![vec![0, 5]])
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidPin { pin: 5, .. }));

    // Terminals only live on terminal strips.
    let err = doc
        .transact("x", |tx| tx.add_terminal(s.k1, "1"))
        .unwrap_err();
    assert_eq!(err, ModelError::NotATerminalStrip(s.k1));
    let err = doc
        .transact("x", |tx| {
            tx.update::<Device>(s.x1, |d| d.kind = DeviceKind::General)
        })
        .unwrap_err();
    assert_eq!(err, ModelError::NotATerminalStrip(s.x1));

    // References must exist.
    let err = doc
        .transact("x", |tx| tx.assign_part(s.k1, PartId(9999)))
        .unwrap_err();
    assert_eq!(err, ModelError::NotFound(PartId(9999).into()));

    // Ids cannot be changed through an update.
    let err = doc
        .transact("x", |tx| {
            tx.update::<Device>(s.k1, |d| d.id = DeviceId(4242))
        })
        .unwrap_err();
    assert!(matches!(
        err,
        ModelError::NotFound(_) | ModelError::IdChanged(_)
    ));
    assert!(doc.project().device(s.k1).is_some());
}

#[test]
fn deleting_cascades_and_detaches() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let (part, _) = doc
        .transact("Parts", |tx| {
            let part = tx.add_part("Siemens", "3RT2016-1AP01")?;
            tx.assign_part(s.k1, part)?;
            tx.assign_part(s.k1, part)?; // assigning twice keeps one entry
            Ok(part)
        })
        .unwrap();
    assert_eq!(doc.project().device(s.k1).unwrap().parts, vec![part]);

    // Removing a part unassigns it but keeps the device.
    doc.transact("x", |tx| tx.remove(part)).unwrap();
    assert!(doc.project().device(s.k1).unwrap().parts.is_empty());

    // Removing a device removes its symbols and their wires.
    let wires_before = doc.project().wires().count();
    doc.transact("x", |tx| tx.remove(s.k1)).unwrap();
    let p = doc.project();
    assert!(p.symbol(s.coil).is_none());
    assert!(p.symbol(s.self_hold).is_none());
    assert_eq!(p.wires().count(), wires_before - 4);

    // Removing a page removes what is drawn on it.
    doc.transact("x", |tx| tx.remove(s.page1)).unwrap();
    let p = doc.project();
    assert!(p.symbol(s.start).is_none());
    assert!(p.junctions().next().is_none());
    // The terminal strip and its terminal are not drawn things; they stay.
    assert_eq!(p.terminals_of_strip(s.x1).len(), 1);

    // Primitive removal of a referenced object is refused.
    let mut project = doc.into_project();
    let (_, t) = project
        .transact("x", |tx| {
            let strip = tx.add_terminal_strip(tag("-X9"))?;
            tx.add_terminal(strip, "1")?;
            Ok(strip)
        })
        .unwrap();
    let strip = t.ops.iter().find_map(|op| match op {
        Op::Insert {
            record: Record::Device(d),
        } => Some(d.clone()),
        _ => None,
    });
    let err = project
        .apply(&Transaction {
            label: "raw".into(),
            ops: vec![Op::Remove {
                record: Record::Device(strip.unwrap()),
            }],
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::StillReferenced { .. }));
}

#[test]
fn terminals_and_jumpers() {
    let mut doc = Document::default();
    let ((t1s, t2s, t3s), _) = doc
        .transact("Strip", |tx| {
            let page = tx.add_page("1")?;
            let x2 = tx.add_terminal_strip(tag("-X2"))?;
            let names = ["10", "2", "1"];
            let mut syms = Vec::new();
            for (i, name) in names.into_iter().enumerate() {
                let t = tx.add_terminal(x2, name)?;
                tx.update::<Terminal>(t, |t| t.jumper = (name != "10").then_some(1))?;
                syms.push(tx.place_symbol(
                    page,
                    "iec/terminal",
                    at(20.0 * i as f64, 0.0),
                    two_pins(),
                    SymbolRole::Terminal { terminals: vec![t] },
                )?);
            }
            let order: Vec<String> = tx
                .project()
                .terminals_of_strip(x2)
                .iter()
                .map(|t| t.name.clone())
                .collect();
            assert_eq!(order, ["1", "2", "10"]);
            Ok((syms[0], syms[1], syms[2]))
        })
        .unwrap();
    let net = doc.project().netlist();
    // "2" and "1" share jumper group 1; "10" has no jumper.
    assert!(net.connected(Endpoint::pin(t2s, 0), Endpoint::pin(t3s, 1)));
    assert!(!net.connected(Endpoint::pin(t1s, 0), Endpoint::pin(t2s, 0)));
}

#[test]
fn cables_carry_wires() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let (w1, _) = doc
        .transact("Cable", |tx| {
            let cores = vec![
                CableCore::new("BK"),
                CableCore::new("BU"),
                CableCore::new("GNYE"),
            ];
            let w1 = tx.add_cable(tag("=A1-W1"), cores)?;
            tx.assign_core(w1, 0, Some(s.w_start_coil))?;
            Ok(w1)
        })
        .unwrap();
    assert_eq!(doc.project().core_of_wire(s.w_start_coil), Some((w1, 0)));

    // One wire, one core.
    let err = doc
        .transact("x", |tx| tx.assign_core(w1, 1, Some(s.w_start_coil)))
        .unwrap_err();
    assert!(matches!(err, ModelError::WireAlreadyOnCore { .. }));
    let err = doc
        .transact("x", |tx| {
            let w2 = tx.add_cable(tag("-W2"), vec![CableCore::new("1")])?;
            tx.assign_core(w2, 0, Some(s.w_start_coil))
        })
        .unwrap_err();
    assert_eq!(
        err,
        ModelError::WireAlreadyOnCore {
            wire: s.w_start_coil,
            cable: w1
        }
    );
    let err = doc
        .transact("x", |tx| tx.assign_core(w1, 7, None))
        .unwrap_err();
    assert_eq!(err, ModelError::InvalidCore { cable: w1, core: 7 });

    // Deleting the wire frees the core.
    doc.transact("x", |tx| tx.remove(s.w_start_coil)).unwrap();
    assert_eq!(doc.project().cable(w1).unwrap().cores[0].wire, None);
    doc.undo().unwrap();
    assert_eq!(
        doc.project().cable(w1).unwrap().cores[0].wire,
        Some(s.w_start_coil)
    );
}

#[test]
fn split_wire_makes_a_t_junction() {
    let mut doc = Document::default();
    let ((a, b, wire), _) = doc
        .transact("x", |tx| {
            let page = tx.add_page("1")?;
            let a = tx.place_symbol(page, "s", at(0.0, 0.0), two_pins(), SymbolRole::Unassigned)?;
            let b = tx.place_symbol(
                page,
                "s",
                at(100.0, 0.0),
                two_pins(),
                SymbolRole::Unassigned,
            )?;
            // a:2 at (0,5) -> bend (0,50) -> (100,50) -> b:1 at (100,-5)
            let wire = tx.connect(Endpoint::pin(a, 1), Endpoint::pin(b, 0))?;
            tx.update::<Wire>(wire, |w| {
                w.route = vec![Point::new(0.0, 50.0), Point::new(100.0, 50.0)];
                w.number = Some("101".into());
            })?;
            Ok((a, b, wire))
        })
        .unwrap();
    let outcome = doc
        .execute(Command::SplitWire {
            wire,
            at: Point::new(40.0, 50.0),
        })
        .unwrap();
    let [AnyId::Junction(j), AnyId::Wire(second)] = outcome.created[..] else {
        panic!("unexpected {:?}", outcome.created);
    };
    let p = doc.project();
    let first = p.wire(wire).unwrap();
    let second = p.wire(second).unwrap();
    assert_eq!(first.to, Endpoint::junction(j));
    assert_eq!(first.route, vec![Point::new(0.0, 50.0)]);
    assert_eq!(second.from, Endpoint::junction(j));
    assert_eq!(second.to, Endpoint::pin(b, 0));
    assert_eq!(second.route, vec![Point::new(100.0, 50.0)]);
    assert_eq!(first.number.as_deref(), Some("101"));
    assert_eq!(second.number, None, "wire numbers are not duplicated");
    assert!(
        p.netlist()
            .connected(Endpoint::pin(a, 1), Endpoint::pin(b, 0))
    );
}

#[test]
fn commands_round_trip_through_json() {
    let mut doc = Document::default();
    let json = r#"{
        "command": "batch",
        "label": "Add motor",
        "commands": [
            { "command": "add_page", "name": "1", "title": "Motor" },
            { "command": "add_device", "tag": "=A1-M1", "description": "Pump motor" },
            { "command": "add_potential", "name": "24V", "kind": "dc_positive", "voltage": 24.0 }
        ]
    }"#;
    let command: Command = serde_json::from_str(json).unwrap();
    assert_eq!(command.label(), "Add motor");
    let outcome = doc.execute(command.clone()).unwrap();
    assert_eq!(outcome.created.len(), 3);
    let AnyId::Page(page) = outcome.created[0] else {
        panic!()
    };
    let AnyId::Device(m1) = outcome.created[1] else {
        panic!()
    };
    assert_eq!(doc.project().page(page).unwrap().title, "Motor");
    assert_eq!(doc.project().device(m1).unwrap().tag.to_string(), "=A1-M1");
    assert!(outcome.changes.added.contains(&AnyId::Device(m1)));
    assert!(outcome.changes.pages.contains(&page));
    assert_eq!(doc.undo_depth(), 1, "a batch is one undo step");

    let back: Command = serde_json::from_str(&serde_json::to_string(&command).unwrap()).unwrap();
    assert_eq!(back, command);

    // Place a symbol and set a property through commands.
    let outcome = doc
        .execute(Command::PlaceSymbol {
            page,
            symbol: "iec/motor".into(),
            placement: Placement {
                origin: Point::new(50.0, 50.0),
                rotation: Rotation::R90,
                mirrored: false,
            },
            role: SymbolRole::Device {
                device: m1,
                main: true,
            },
            pins: vec![Pin::new("U1", -5.0, 0.0)],
            bridges: vec![],
            pass_through: vec![],
        })
        .unwrap();
    let AnyId::Symbol(motor) = outcome.created[0] else {
        panic!()
    };
    assert_eq!(
        doc.project().endpoint_position(Endpoint::pin(motor, 0)),
        Some(Point::new(50.0, 55.0))
    );
    doc.execute(Command::SetProperty {
        target: m1.into(),
        key: "power_kw".into(),
        value: Some(Value::Float(4.0)),
    })
    .unwrap();
    assert_eq!(
        doc.project().device(m1).unwrap().properties.get("power_kw"),
        Some(&Value::Float(4.0))
    );
    doc.execute(Command::Remove { id: m1.into() }).unwrap();
    assert!(doc.project().symbol(motor).is_none());
    doc.undo().unwrap();
    assert!(doc.project().symbol(motor).is_some());
}

#[test]
fn json_export_round_trips() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    doc.transact("info", |tx| {
        tx.set_info(|i| {
            i.name = "Pump station".into();
            i.properties.insert("customer".into(), "ACME".into());
        })?;
        tx.set_property(s.page1, "drawn_by", Some("TT".into()))
    })
    .unwrap();
    let project = doc.project();
    let json = project.to_json();
    let loaded = Project::from_json(&json).unwrap();
    assert_eq!(loaded.to_json(), json);
    assert_eq!(loaded.next_id(), project.next_id());
    assert_eq!(loaded.info().name, "Pump station");
    // Derived data is rebuilt on load.
    assert_eq!(
        loaded.netlist().potential_of(Endpoint::pin(s.coil, 1)),
        Some(s.n)
    );
    assert_eq!(loaded.symbols_of_device(s.k1).len(), 2);

    // Records round-trip in any order.
    let mut records: Vec<Record> = project.records().collect();
    records.reverse();
    let rebuilt = Project::from_records(project.info().clone(), 0, records).unwrap();
    assert_eq!(rebuilt.to_json(), json);
}

#[test]
fn json_import_rejects_broken_projects() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let json = doc.project().to_json();

    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    value["format"] = 99.into();
    assert_eq!(
        Project::from_json(&value.to_string()).unwrap_err(),
        ModelError::UnsupportedFormat(99)
    );

    // A wire pointing at a symbol that is not in the file.
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let symbols = value["symbols"].as_array_mut().unwrap();
    symbols.retain(|sym| sym["id"] != s.coil.raw());
    assert_eq!(
        Project::from_json(&value.to_string()).unwrap_err(),
        ModelError::NotFound(s.coil.into())
    );

    assert!(matches!(
        Project::from_json("{ not json"),
        Err(ModelError::Format(_))
    ));
}

#[test]
fn tag_queries() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let p = doc.project();
    assert_eq!(p.next_tag_number(&tag("=A1"), "K"), 2);
    assert_eq!(p.next_tag_number(&tag("=A2"), "K"), 1);
    assert_eq!(p.next_tag_number(&tag("=A1"), "S"), 2);
    assert!(p.duplicate_tags().is_empty());
    assert_eq!(p.devices_by_tag(&tag("=A1-K1")), vec![s.k1]);

    doc.transact("dup", |tx| tx.add_cable(tag("=A1-K1"), vec![]))
        .unwrap();
    let dups = doc.project().duplicate_tags();
    assert_eq!(dups.len(), 1);
    assert_eq!(dups[0].0, tag("=A1-K1"));
    assert_eq!(dups[0].1.len(), 2);
}

#[test]
fn page_order_is_natural() {
    let mut doc = Document::default();
    doc.transact("pages", |tx| {
        for name in ["10", "2", "1", "2.1"] {
            tx.add_page(name)?;
        }
        Ok(())
    })
    .unwrap();
    let names: Vec<&str> = doc
        .project()
        .pages_in_order()
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(names, ["1", "2", "2.1", "10"]);
}

#[test]
fn change_sets_name_touched_pages() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let (_, changes) = doc
        .transact("move", |tx| tx.move_symbol(s.coil, at(80.0, 80.0)))
        .unwrap();
    assert_eq!(
        changes.modified.iter().copied().collect::<Vec<_>>(),
        vec![AnyId::Symbol(s.coil)]
    );
    assert_eq!(
        changes.pages.iter().copied().collect::<Vec<_>>(),
        vec![s.page2]
    );
    assert!(changes.touches(EntityKind::Symbol));
    assert!(!changes.touches(EntityKind::Wire));

    // Adding then removing inside one edit nets out to nothing visible.
    let (_, changes) = doc
        .transact("noop", |tx| {
            let d = tx.add_device(tag("-Q9"), "")?;
            tx.remove(d)
        })
        .unwrap();
    assert!(changes.added.is_empty() && changes.removed.is_empty());

    // Editing to the same value records nothing.
    let depth = doc.undo_depth();
    doc.transact("same", |tx| tx.set_tag(s.k1, tag("=A1-K1")))
        .unwrap();
    assert_eq!(doc.undo_depth(), depth);

    let undo = doc.undo().unwrap().unwrap();
    assert!(undo.removed.is_empty());
}

#[test]
fn dirty_tracking_follows_undo() {
    let mut doc = Document::default();
    assert!(!doc.is_dirty());
    doc.transact("a", |tx| tx.add_page("1")).unwrap();
    assert!(doc.is_dirty());
    doc.mark_saved();
    assert!(!doc.is_dirty());
    doc.transact("b", |tx| tx.add_page("2")).unwrap();
    assert!(doc.is_dirty());
    doc.undo().unwrap();
    assert!(!doc.is_dirty(), "back at the saved state");
    doc.undo().unwrap();
    assert!(doc.is_dirty());
    doc.transact("c", |tx| tx.add_page("3")).unwrap();
    doc.undo().unwrap();
    doc.redo().unwrap();
    assert!(doc.is_dirty(), "the saved state is no longer reachable");
}

#[test]
fn history_limit_drops_oldest_steps() {
    let mut doc = Document::default();
    doc.set_history_limit(3);
    for i in 0..5 {
        doc.transact("page", |tx| tx.add_page(i.to_string()))
            .unwrap();
    }
    assert_eq!(doc.undo_depth(), 3);
    while doc.undo().unwrap().is_some() {}
    assert_eq!(doc.project().pages().count(), 2);
}

/// Random edits, then undo everything: the project must come back exactly.
#[test]
fn random_edits_undo_to_the_start() {
    let mut rng = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move |n: u64| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng % n
    };
    let mut doc = Document::default();
    build_starter(&mut doc);
    doc.clear_history();
    let start = state(doc.project());
    let mut snapshots = vec![start.clone()];
    for _ in 0..300 {
        let p = doc.project();
        let symbols: Vec<SymbolId> = p.symbols().map(|s| s.id).collect();
        let pages: Vec<PageId> = p.pages().map(|p| p.id).collect();
        let ids: Vec<AnyId> = p.records().map(|r| r.id()).collect();
        let result = doc.transact("random", |tx| match next(5) {
            0 if !pages.is_empty() => {
                let page = pages[next(pages.len() as u64) as usize];
                tx.place_symbol(
                    page,
                    "iec/contact_no",
                    at(next(400) as f64, next(280) as f64),
                    two_pins(),
                    SymbolRole::Unassigned,
                )
                .map(|_| ())
            }
            1 if symbols.len() >= 2 => {
                let a = symbols[next(symbols.len() as u64) as usize];
                let b = symbols[next(symbols.len() as u64) as usize];
                tx.connect(
                    Endpoint::pin(a, next(2) as u16),
                    Endpoint::pin(b, next(2) as u16),
                )
                .map(|_| ())
            }
            2 if !ids.is_empty() => tx.remove(ids[next(ids.len() as u64) as usize]),
            3 if !symbols.is_empty() => {
                let s = symbols[next(symbols.len() as u64) as usize];
                let rotation = Rotation::from_degrees(90 * next(4) as i32).unwrap();
                tx.update::<SymbolInstance>(s, |sym| sym.placement.rotation = rotation)
            }
            _ => tx.add_page(format!("{}", next(100))).map(|_| ()),
        });
        if result.is_ok() && doc.undo_depth() == snapshots.len() {
            snapshots.push(state(doc.project()));
        }
        // Every state must reload cleanly.
        Project::from_json(&doc.project().to_json()).unwrap();
    }
    // Walk back through every recorded state.
    while doc.can_undo() {
        doc.undo().unwrap();
        assert_eq!(state(doc.project()), snapshots[doc.undo_depth()]);
    }
    assert_eq!(state(doc.project()), start);
}

/// Records, ops, transactions and record-carrying commands must survive JSON:
/// storage, macros, Python and the AI assistant all exchange them as JSON.
#[test]
fn records_ops_and_commands_round_trip_through_json() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let mut project = doc.project().clone();

    // Every kind of record, including those with their own `kind` field.
    let records: Vec<Record> = project.records().collect();
    let kinds: std::collections::BTreeSet<EntityKind> = records.iter().map(|r| r.kind()).collect();
    assert!(kinds.contains(&EntityKind::Page) && kinds.contains(&EntityKind::Terminal));
    for record in &records {
        let json = serde_json::to_string(record).unwrap();
        let back: Record = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("{} did not round-trip: {e}\n{json}", record.id()));
        assert_eq!(&back, record);
    }

    // A transaction touching pages, devices, potentials and terminals.
    let (_, transaction) = project
        .transact("Edit", |tx| {
            tx.add_page("3")?;
            tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
            tx.update::<Device>(s.k1, |d| d.kind = DeviceKind::Plc)?;
            tx.update::<Page>(s.page1, |p| {
                p.kind = PageKind::Generated {
                    generator: "terminal_diagram".into(),
                }
            })?;
            let t = tx.add_terminal(s.x1, "2")?;
            tx.update::<Terminal>(t, |t| t.kind = TerminalKind::ProtectiveEarth)?;
            tx.remove(s.n)
        })
        .unwrap();
    let json = serde_json::to_string(&transaction).unwrap();
    let back: Transaction = serde_json::from_str(&json).unwrap();
    assert_eq!(back, transaction);
    // The deserialized inverse still undoes the edit.
    project.apply(&back.inverse()).unwrap();
    assert_eq!(state(&project), state(doc.project()));

    // Commands carrying records and ids.
    let device = project.record(s.k1.into()).unwrap();
    for command in [
        Command::Replace { record: device },
        Command::Insert {
            record: Record::Potential(Potential {
                id: PotentialId(9000),
                name: "24V".into(),
                kind: PotentialKind::DcPositive,
                voltage: Some(24.0),
                properties: Properties::new(),
            }),
        },
        Command::Remove { id: s.page2.into() },
    ] {
        let json = serde_json::to_string(&command).unwrap();
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), command);
    }
}

#[test]
fn split_wire_puts_the_junction_on_the_wire() {
    let mut doc = Document::default();
    let (wire, _) = doc
        .transact("x", |tx| {
            let page = tx.add_page("1")?;
            let a = tx.place_symbol(page, "s", at(0.0, 0.0), two_pins(), SymbolRole::Unassigned)?;
            let b = tx.place_symbol(
                page,
                "s",
                at(100.0, 0.0),
                two_pins(),
                SymbolRole::Unassigned,
            )?;
            // A horizontal wire along y = 5.
            tx.connect(Endpoint::pin(a, 1), Endpoint::pin(b, 1))
        })
        .unwrap();
    let ((junction, _), _) = doc
        .transact("split", |tx| tx.split_wire(wire, Point::new(50.0, 7.0)))
        .unwrap();
    assert_eq!(
        doc.project().junction(junction).unwrap().position,
        Point::new(50.0, 5.0)
    );
}

#[test]
fn ids_are_unique_across_kinds_and_bounded() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let potential = |id: u64| {
        Record::Potential(Potential {
            id: PotentialId(id),
            name: "X".into(),
            kind: PotentialKind::Signal,
            voltage: None,
            properties: Properties::new(),
        })
    };
    let before = state(doc.project());
    let next_id = doc.project().next_id();

    // Reserved ids are refused instead of overflowing the id counter.
    for id in [0, u64::MAX] {
        let err = doc
            .execute(Command::Insert {
                record: potential(id),
            })
            .unwrap_err();
        assert_eq!(err, ModelError::InvalidId(id));
    }
    // An id already used by another kind is refused.
    let err = doc
        .execute(Command::Insert {
            record: potential(s.k1.raw()),
        })
        .unwrap_err();
    assert_eq!(err, ModelError::AlreadyExists(s.k1.into()));
    assert_eq!(state(doc.project()), before);
    assert_eq!(doc.project().next_id(), next_id);

    // The largest usable id still works, and new ids never wrap around.
    doc.execute(Command::Insert {
        record: potential(u64::MAX - 1),
    })
    .unwrap();
    assert_eq!(doc.project().next_id(), u64::MAX);
    let err = doc.transact("x", |tx| tx.add_page("9")).unwrap_err();
    assert_eq!(err, ModelError::InvalidId(u64::MAX));

    // Loading refuses duplicate raw ids across kinds too.
    let mut records: Vec<Record> = doc.project().records().collect();
    records.push(potential(s.page1.raw()));
    let err = Project::from_records(ProjectInfo::default(), 1, records).unwrap_err();
    assert!(matches!(err, ModelError::AlreadyExists(_)));
}

#[test]
fn invalid_tags_cannot_enter_a_project() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let bad_tags = [
        DeviceTag {
            function: vec!["A-1".into()],
            location: vec![],
            product: vec!["K9".into()],
        },
        DeviceTag::product(""),
        DeviceTag::product("K#1"),
    ];
    for bad in bad_tags {
        assert!(bad.validate().is_err());
        let err = doc
            .transact("x", |tx| tx.add_device(bad.clone(), ""))
            .unwrap_err();
        assert!(matches!(err, ModelError::Tag(_)), "{err:?}");
        let err = doc
            .transact("x", |tx| tx.set_tag(s.k1, bad.clone()))
            .unwrap_err();
        assert!(matches!(err, ModelError::Tag(_)));
        let err = doc
            .transact("x", |tx| tx.add_cable(bad.clone(), vec![]))
            .unwrap_err();
        assert!(matches!(err, ModelError::Tag(_)));
    }
    assert!(tag("=A1+CAB1-K1.2_b").validate().is_ok());
    // The project still saves and loads.
    Project::from_json(&doc.project().to_json()).unwrap();
}

#[test]
fn update_cannot_change_an_id() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let stop_device = doc.project().symbol(s.stop).unwrap().device().unwrap();
    let before = state(doc.project());
    let err = doc
        .transact("x", |tx| {
            tx.update::<Device>(s.k1, |d| {
                d.id = stop_device;
                d.description = "overwritten".into();
            })
        })
        .unwrap_err();
    assert_eq!(err, ModelError::IdChanged(s.k1.into()));
    assert_eq!(state(doc.project()), before);
    assert_eq!(
        doc.project().device(stop_device).unwrap().description,
        "Stop"
    );
}

#[test]
fn stale_info_ops_are_refused() {
    let mut project = Project::new("A");
    let (_, rename) = project
        .transact("rename", |tx| tx.set_info(|i| i.name = "B".into()))
        .unwrap();
    // Applying the same rename again no longer matches the current info.
    assert_eq!(
        project.apply(&rename).unwrap_err(),
        ModelError::InfoConflict
    );
    project.apply(&rename.inverse()).unwrap();
    assert_eq!(project.info().name, "A");
}

#[test]
fn multi_level_terminals_keep_levels_apart() {
    let mut doc = Document::default();
    let ((block, upper, lower), _) = doc
        .transact("2-level terminal", |tx| {
            let page = tx.add_page("1")?;
            let x3 = tx.add_terminal_strip(tag("-X3"))?;
            let upper = tx.add_terminal(x3, "1")?;
            let lower = tx.add_terminal(x3, "2")?;
            let created = tx.execute(Command::PlaceSymbol {
                page,
                symbol: "iec/terminals/terminal_2level".into(),
                placement: at(0.0, 0.0),
                role: SymbolRole::Terminal {
                    terminals: vec![upper, lower],
                },
                pins: vec![
                    Pin::new("1", 0.0, 0.0),
                    Pin::new("2", 0.0, 8.0),
                    Pin::new("3", 8.0, 0.0),
                    Pin::new("4", 8.0, 8.0),
                ],
                bridges: vec![vec![0, 1], vec![2, 3]],
                pass_through: vec![],
            })?;
            let [AnyId::Symbol(block)] = created[..] else {
                panic!("unexpected {created:?}")
            };
            Ok((block, upper, lower))
        })
        .unwrap();
    let p = doc.project();
    let net = p.netlist();
    assert!(net.connected(Endpoint::pin(block, 0), Endpoint::pin(block, 1)));
    assert!(net.connected(Endpoint::pin(block, 2), Endpoint::pin(block, 3)));
    assert!(!net.connected(Endpoint::pin(block, 1), Endpoint::pin(block, 2)));
    assert_eq!(p.symbol(block).unwrap().terminal_of_pin(3), Some(lower));
    assert_eq!(
        p.pin_designation(PinRef::new(block, 0)).as_deref(),
        Some("-X3:1")
    );
    assert_eq!(
        p.pin_designation(PinRef::new(block, 3)).as_deref(),
        Some("-X3:2")
    );
    assert_eq!(p.symbols_of_terminal(upper).len(), 1);
    assert_eq!(p.symbols_of_terminal(lower).len(), 1);

    // A jumper between the levels joins them.
    doc.transact("jumper", |tx| {
        tx.update::<Terminal>(upper, |t| t.jumper = Some(1))?;
        tx.update::<Terminal>(lower, |t| t.jumper = Some(1))
    })
    .unwrap();
    assert!(
        doc.project()
            .netlist()
            .connected(Endpoint::pin(block, 1), Endpoint::pin(block, 2))
    );

    // Levels and bridge groups must match one to one.
    let err = doc
        .transact("x", |tx| {
            tx.update::<SymbolInstance>(block, |s| s.bridges = vec![vec![0, 1, 2, 3]])
        })
        .unwrap_err();
    assert!(matches!(
        err,
        ModelError::TerminalLevels {
            terminals: 2,
            bridges: 1,
            ..
        }
    ));
    let err = doc
        .transact("x", |tx| {
            tx.update::<SymbolInstance>(block, |s| {
                s.role = SymbolRole::Terminal { terminals: vec![] }
            })
        })
        .unwrap_err();
    assert!(matches!(
        err,
        ModelError::TerminalLevels { terminals: 0, .. }
    ));
    // The same terminal cannot sit on two levels.
    let err = doc
        .transact("x", |tx| {
            tx.update::<SymbolInstance>(block, |s| {
                s.role = SymbolRole::Terminal {
                    terminals: vec![upper, upper],
                }
            })
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::TerminalLevels { .. }));

    // A single terminal owns every pin, bridged or not.
    doc.transact("x", |tx| {
        tx.update::<SymbolInstance>(block, |s| {
            s.role = SymbolRole::Terminal {
                terminals: vec![upper],
            }
        })
    })
    .unwrap();
    let p = doc.project();
    let s = p.symbol(block).unwrap();
    assert!((0..4).all(|pin| s.terminal_of_pin(pin) == Some(upper)));
    assert!(
        p.netlist()
            .connected(Endpoint::pin(block, 0), Endpoint::pin(block, 3))
    );
    doc.undo().unwrap();

    // Deleting one level's terminal deletes the block that draws it.
    doc.transact("x", |tx| tx.remove(lower)).unwrap();
    assert!(doc.project().symbol(block).is_none());
}

#[test]
fn pin_directions_turn_with_the_symbol() {
    let mut doc = Document::default();
    let (contact, _) = doc
        .transact("x", |tx| {
            let page = tx.add_page("1")?;
            tx.place_symbol(
                page,
                "iec/contacts/no",
                Placement {
                    origin: Point::new(40.0, 40.0),
                    rotation: Rotation::R90,
                    mirrored: false,
                },
                vec![
                    Pin::new("13", 0.0, 0.0).with_direction(Direction::Up),
                    Pin::new("14", 0.0, 16.0).with_direction(Direction::Down),
                    Pin::new("x", 4.0, 8.0),
                ],
                SymbolRole::Unassigned,
            )
        })
        .unwrap();
    let symbol = doc.project().symbol(contact).unwrap();
    // A vertical contact turned a quarter counter-clockwise lies horizontally.
    assert_eq!(symbol.pin_direction(0), Some(Direction::Left));
    assert_eq!(symbol.pin_direction(1), Some(Direction::Right));
    assert_eq!(symbol.pin_position(1), Some(Point::new(56.0, 40.0)));
    assert_eq!(symbol.pin_direction(2), None);
    assert_eq!(symbol.pin_direction(9), None);

    // Pins without a direction keep their old JSON form.
    let json = serde_json::to_value(&symbol.pins[2]).unwrap();
    assert!(json.get("direction").is_none());
    let json = serde_json::to_value(&symbol.pins[0]).unwrap();
    assert_eq!(json["direction"], "up");
}

#[test]
fn split_wire_refuses_its_own_ends() {
    let mut doc = Document::default();
    let (wire, _) = doc
        .transact("x", |tx| {
            let page = tx.add_page("1")?;
            let a = tx.place_symbol(page, "s", at(0.0, 0.0), two_pins(), SymbolRole::Unassigned)?;
            let b = tx.place_symbol(
                page,
                "s",
                at(100.0, 0.0),
                two_pins(),
                SymbolRole::Unassigned,
            )?;
            // a:2 at (0,5) -> bend (50,20) -> b:2 at (100,5).
            let wire = tx.connect(Endpoint::pin(a, 1), Endpoint::pin(b, 1))?;
            tx.update::<Wire>(wire, |w| w.route = vec![Point::new(50.0, 20.0)])?;
            Ok(wire)
        })
        .unwrap();
    let before = state(doc.project());
    // Clicks beyond either end clamp onto the end itself.
    for click in [Point::new(-10.0, 5.0), Point::new(130.0, 0.0)] {
        let err = doc
            .transact("split", |tx| tx.split_wire(wire, click))
            .unwrap_err();
        assert_eq!(err, ModelError::SplitAtEnd(wire));
    }
    assert_eq!(state(doc.project()), before);
    // Splitting at an interior bend is fine: both halves have length.
    let ((junction, _), _) = doc
        .transact("split", |tx| tx.split_wire(wire, Point::new(50.0, 25.0)))
        .unwrap();
    assert_eq!(
        doc.project().junction(junction).unwrap().position,
        Point::new(50.0, 20.0)
    );
}

type Edit = dyn Fn(&mut Tx<'_>) -> Result<()>;

/// NaN and infinity have no JSON or SQLite form: a project holding one
/// saves but cannot be reopened. Every edit path must refuse them.
#[test]
fn non_finite_numbers_are_refused() {
    let mut doc = Document::default();
    let s = build_starter(&mut doc);
    let before = state(doc.project());
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let edits: Vec<(&str, Box<Edit>)> = vec![
            (
                "placement",
                Box::new(move |tx| tx.move_symbol(s.coil, at(bad, 0.0))),
            ),
            (
                "pins",
                Box::new(move |tx| {
                    tx.update::<SymbolInstance>(s.coil, |c| c.pins[0].offset.y = bad)
                }),
            ),
            (
                "position",
                Box::new(move |tx| tx.add_junction(s.page1, Point::new(0.0, bad)).map(|_| ())),
            ),
            (
                "route",
                Box::new(move |tx| {
                    tx.update::<Wire>(s.w_start_coil, |w| w.route = vec![Point::new(bad, 1.0)])
                }),
            ),
            (
                "cross_section_mm2",
                Box::new(move |tx| {
                    tx.update::<Wire>(s.w_start_coil, |w| w.cross_section_mm2 = Some(bad))
                }),
            ),
            (
                "voltage",
                Box::new(move |tx| tx.update::<Potential>(s.l, |p| p.voltage = Some(bad))),
            ),
            (
                "cores",
                Box::new(move |tx| {
                    let mut core = CableCore::new("1");
                    core.cross_section_mm2 = Some(bad);
                    tx.add_cable(tag("-W9"), vec![core]).map(|_| ())
                }),
            ),
            (
                "properties",
                Box::new(move |tx| tx.set_property(s.k1, "rating", Some(Value::Float(bad)))),
            ),
        ];
        for (field, edit) in edits {
            let err = doc.transact("bad", |tx| edit(tx)).unwrap_err();
            assert!(
                matches!(err, ModelError::NonFinite { field: f, .. } if f == field),
                "{field}: {err:?}"
            );
        }
        let err = doc
            .transact("bad", |tx| {
                tx.set_info(|i| {
                    i.properties.insert("x".into(), Value::Float(bad));
                })
            })
            .unwrap_err();
        assert_eq!(err, ModelError::NonFiniteInfo);
    }
    assert_eq!(state(doc.project()), before);
    // What is left still saves and reopens.
    Project::from_json(&doc.project().to_json()).unwrap();

    // Commands from scripts take the same path.
    let err = doc
        .execute(Command::AddJunction {
            page: s.page1,
            position: Point::new(f64::NAN, 0.0),
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::NonFinite { .. }));
}

#[test]
fn potentials_pass_through_switched_poles() {
    let mut doc = Document::default();
    let ((l1, feed, q1, k1, k1_coil, motor), _) = doc
        .transact("Main circuit", |tx| {
            let page = tx.add_page("1")?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let feed = tx.place_symbol(
                page,
                "iec/connectivity/potential_feed",
                at(0.0, 0.0),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::PotentialFeed { potential: l1 },
            )?;
            // Breaker -Q1 and contactor contact -K1: switched poles.
            let mut q1 = None;
            let mut k1 = None;
            for (key, y, slot) in [
                ("iec/protection/mcb", 20.0, &mut q1),
                ("iec/contacts/main", 50.0, &mut k1),
            ] {
                let id =
                    tx.place_symbol(page, key, at(0.0, y), two_pins(), SymbolRole::Unassigned)?;
                tx.update::<SymbolInstance>(id, |s| s.pass_through = vec![vec![0, 1]])?;
                *slot = Some(id);
            }
            let (q1, k1) = (q1.unwrap(), k1.unwrap());
            // A coil ends the potential.
            let k1_coil = tx.place_symbol(
                page,
                "iec/coils/relay",
                at(40.0, 0.0),
                two_pins(),
                SymbolRole::Unassigned,
            )?;
            let motor = tx.place_symbol(
                page,
                "iec/motors/ac3",
                at(0.0, 80.0),
                two_pins(),
                SymbolRole::Unassigned,
            )?;
            tx.connect(Endpoint::pin(feed, 0), Endpoint::pin(q1, 0))?;
            tx.connect(Endpoint::pin(q1, 1), Endpoint::pin(k1, 0))?;
            tx.connect(Endpoint::pin(k1, 1), Endpoint::pin(motor, 0))?;
            tx.connect(Endpoint::pin(q1, 1), Endpoint::pin(k1_coil, 0))?;
            Ok((l1, feed, q1, k1, k1_coil, motor))
        })
        .unwrap();
    let net = doc.project().netlist();
    // L1 reaches the motor through the breaker and the open contact...
    assert_eq!(net.potential_of(Endpoint::pin(motor, 0)), Some(l1));
    assert_eq!(net.potential_of(Endpoint::pin(k1_coil, 0)), Some(l1));
    // ...but the poles are not connected, and coils and motors end it.
    assert!(!net.connected(Endpoint::pin(q1, 0), Endpoint::pin(q1, 1)));
    assert!(!net.connected(Endpoint::pin(k1, 0), Endpoint::pin(k1, 1)));
    assert_eq!(net.potential_of(Endpoint::pin(k1_coil, 1)), None);
    assert_eq!(net.potential_of(Endpoint::pin(motor, 1)), None);
    // Only the feed's own net defines L1.
    let motor_net = net.net(net.net_of(Endpoint::pin(motor, 0)).unwrap());
    assert!(motor_net.defined.is_empty());
    let feed_net = net.net(net.net_of(Endpoint::pin(feed, 0)).unwrap());
    assert_eq!(feed_net.defined, vec![l1]);

    // Pass-through groups must name existing pins and survive JSON.
    let err = doc
        .transact("x", |tx| {
            tx.update::<SymbolInstance>(k1, |s| s.pass_through = vec![vec![0, 7]])
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidPin { pin: 7, .. }));
    let loaded = Project::from_json(&doc.project().to_json()).unwrap();
    assert_eq!(loaded.symbol(k1).unwrap().pass_through, vec![vec![0, 1]]);
}
