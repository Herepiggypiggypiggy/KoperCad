//! Blocks: capturing a circuit as a definition, placing it with parameter
//! values, editing those values, and detaching on a structural edit.

use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn two_pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)]
}

struct Drawn {
    p1: PageId,
    p2: PageId,
    q1: SymbolId,
    m1: SymbolId,
    outside: SymbolId,
    note: TextId,
    terminal: SymbolId,
}

/// A direct-on-line starter on page 1 of `=A1`: breaker -Q1 wired to
/// motor -M1, a wire number label, a free note, and a wire to a lamp that
/// stays outside the block.
fn drawing(doc: &mut Document) -> Drawn {
    let (drawn, _) = doc
        .transact("Draw", |tx| {
            let p1 = tx.add_page("1")?;
            let p2 = tx.add_page("2")?;
            for page in [p1, p2] {
                tx.update::<Page>(page, |p| p.structure = tag("=A1"))?;
            }
            let q = tx.add_device(tag("=A1-Q1"), "Motor breaker")?;
            tx.set_property(q, "power_kw", Some(Value::Text("{power}".into())))?;
            let m = tx.add_device(tag("=A1-M1"), "Motor {power} kW")?;
            let place = |tx: &mut Tx, x: f64, role: SymbolRole| {
                tx.place_symbol(p1, "x", Placement::at(Point::new(x, 0.0)), two_pins(), role)
            };
            let q1 = place(
                tx,
                10.0,
                SymbolRole::Device {
                    device: q,
                    main: true,
                },
            )?;
            let m1 = place(
                tx,
                30.0,
                SymbolRole::Device {
                    device: m,
                    main: true,
                },
            )?;
            let outside = place(tx, 60.0, SymbolRole::Unassigned)?;
            let wire = tx.connect(Endpoint::pin(q1, 1), Endpoint::pin(m1, 0))?;
            tx.update::<Wire>(wire, |w| w.route = vec![Point::new(10.0, 20.0)])?;
            tx.connect(Endpoint::pin(m1, 1), Endpoint::pin(outside, 0))?;
            tx.add_text(
                p1,
                TextContent::WireNumber,
                TextPlacement::default(),
                Some(wire.into()),
            )?;
            let note = tx.add_text(
                p1,
                TextContent::Literal {
                    text: "{power} kW".into(),
                },
                TextPlacement {
                    offset: Point::new(20.0, -5.0),
                    ..Default::default()
                },
                None,
            )?;
            let strip = tx.add_terminal_strip(tag("=A1-X1"))?;
            let t = tx.add_terminal(strip, "1")?;
            let terminal = tx.place_symbol(
                p1,
                "terminal",
                Placement::at(Point::new(90.0, 0.0)),
                two_pins(),
                SymbolRole::Terminal { terminals: vec![t] },
            )?;
            Ok(Drawn {
                p1,
                p2,
                q1,
                m1,
                outside,
                note,
                terminal,
            })
        })
        .unwrap();
    drawn
}

fn power(default: f64) -> Vec<BlockParameter> {
    vec![BlockParameter {
        name: "power".into(),
        default: Value::Float(default),
        description: "Motor power in kW".into(),
    }]
}

fn device_of(project: &Project, block: &BlockInstance, letter: &str) -> Device {
    block
        .members
        .iter()
        .filter_map(|m| match m.actual {
            AnyId::Device(d) => project.device(d),
            _ => None,
        })
        .find(|d| d.tag.letter_code() == letter)
        .unwrap()
        .clone()
}

#[test]
fn a_placed_block_gets_new_tags_and_its_values() {
    let mut doc = Document::default();
    let d = drawing(&mut doc);
    let (def, _) = doc
        .transact("Define", |tx| {
            tx.define_block(
                "dol",
                &[d.q1.into(), d.m1.into(), d.note.into()],
                Point::new(10.0, 0.0),
                power(4.0),
            )
        })
        .unwrap();
    let definition = doc.project().block_definition(def).unwrap();
    // Two devices, two symbols, the inner wire, its label and the note;
    // the wire to the lamp outside is how the block connects, so it stays out.
    assert_eq!(definition.members.len(), 7);
    assert!(
        definition
            .members
            .iter()
            .all(|m| m.page().is_none_or(|p| p.raw() == 0))
    );

    let (block, _) = doc
        .transact("Place", |tx| {
            tx.place_block(
                def,
                d.p2,
                Point::new(100.0, 50.0),
                vec![("power".into(), Value::Float(7.5))],
            )
        })
        .unwrap();
    let project = doc.project();
    let instance = project.block_instance(block).unwrap();
    assert_eq!(instance.members.len(), 7);
    assert!(instance.detached.is_none());
    let q = device_of(project, instance, "Q");
    let m = device_of(project, instance, "M");
    assert_eq!(q.tag, tag("=A1-Q2"));
    assert_eq!(m.tag, tag("=A1-M2"));
    // A property that is exactly `{power}` keeps the value's type.
    assert_eq!(q.properties["power_kw"], Value::Float(7.5));
    assert_eq!(m.description, "Motor 7.5 kW");
    // Positions are relative to the origin: -Q1 was at x 10, the origin.
    let symbols: Vec<&SymbolInstance> = project.symbols().filter(|s| s.page == d.p2).collect();
    assert_eq!(symbols.len(), 2);
    assert!(
        symbols
            .iter()
            .any(|s| s.placement.origin == Point::new(100.0, 50.0))
    );
    let wire = project
        .wires()
        .find(|w| w.page == d.p2)
        .expect("the inner wire is placed");
    assert_eq!(wire.route, [Point::new(100.0, 70.0)]);
    assert_eq!(project.block_of(wire.id).map(|b| b.id), Some(block));
    let notes: Vec<String> = project
        .texts()
        .filter(|t| t.page == d.p2 && t.attached_to.is_none())
        .map(|t| match &t.content {
            TextContent::Literal { text } => text.clone(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(notes, ["7.5 kW"]);

    // A second placement with the default takes the next numbers.
    doc.execute(Command::PlaceBlock {
        definition: def,
        page: d.p2,
        origin: Point::new(200.0, 50.0),
        values: Vec::new(),
    })
    .unwrap();
    let project = doc.project();
    let second = project.block_instances().find(|b| b.id != block).unwrap();
    assert_eq!(device_of(project, second, "M").description, "Motor 4 kW");
    assert_eq!(device_of(project, second, "Q").tag, tag("=A1-Q3"));
    assert_eq!(
        project.block_values(second.id),
        [("power".into(), Value::Float(4.0))]
    );

    // Placing is one undo step.
    doc.undo().unwrap();
    assert_eq!(doc.project().block_instances().count(), 1);
    assert_eq!(
        doc.project().symbols().filter(|s| s.page == d.p2).count(),
        2
    );

    // The project saves and loads with its blocks.
    let loaded = Project::from_json(&doc.project().to_json()).unwrap();
    assert_eq!(loaded.block_definitions().count(), 1);
    assert_eq!(
        loaded.block_instance(block),
        doc.project().block_instance(block)
    );
}

#[test]
fn parameter_edits_fill_in_again_except_hand_edits() {
    let mut doc = Document::default();
    let d = drawing(&mut doc);
    let ((block, note), _) = doc
        .transact("Place", |tx| {
            let def = tx.define_block(
                "dol",
                &[d.q1.into(), d.m1.into(), d.note.into()],
                Point::default(),
                power(4.0),
            )?;
            let block = tx.place_block(def, d.p2, Point::default(), Vec::new())?;
            let note = tx
                .project()
                .block_instance(block)
                .unwrap()
                .members
                .iter()
                .find_map(|m| match m.actual {
                    AnyId::Text(t) if tx.project().text(t)?.attached_to.is_none() => Some(t),
                    _ => None,
                })
                .unwrap();
            Ok((block, note))
        })
        .unwrap();
    // The user rewrites the note by hand and marks it so.
    doc.transact("Edit note", |tx| {
        tx.set_text(
            note,
            Some(TextContent::Literal {
                text: "Pump".into(),
            }),
            None,
        )?;
        tx.set_block_override(block, note.into(), BlockOverride::Value, true)
    })
    .unwrap();
    // A user property on the device survives a parameter edit too.
    let m = device_of(
        doc.project(),
        doc.project().block_instance(block).unwrap(),
        "M",
    );
    doc.transact("Note", |tx| {
        tx.set_property(m.id, "note", Some("spare".into()))
    })
    .unwrap();

    doc.execute(Command::SetBlockValues {
        instance: block,
        values: vec![("power".into(), Value::Float(11.0))],
    })
    .unwrap();
    let project = doc.project();
    let instance = project.block_instance(block).unwrap();
    let m = device_of(project, instance, "M");
    assert_eq!(m.description, "Motor 11 kW");
    assert_eq!(m.properties["note"], Value::from("spare"));
    assert_eq!(
        device_of(project, instance, "Q").properties["power_kw"],
        Value::Float(11.0)
    );
    assert_eq!(
        project.text(note).unwrap().content,
        TextContent::Literal {
            text: "Pump".into()
        }
    );
    // Unknown parameters are refused.
    let err = doc
        .execute(Command::SetBlockValues {
            instance: block,
            values: vec![("speed".into(), Value::Int(3))],
        })
        .unwrap_err();
    assert!(matches!(
        err,
        ModelError::InvalidValue {
            field: "values",
            ..
        }
    ));
}

#[test]
fn deleting_a_member_detaches_the_block() {
    let mut doc = Document::default();
    let d = drawing(&mut doc);
    let (block, _) = doc
        .transact("Place", |tx| {
            let def = tx.define_block(
                "dol",
                &[d.q1.into(), d.m1.into()],
                Point::default(),
                power(4.0),
            )?;
            tx.place_block(def, d.p2, Point::default(), Vec::new())
        })
        .unwrap();
    let m = device_of(
        doc.project(),
        doc.project().block_instance(block).unwrap(),
        "M",
    );
    doc.transact("Delete", |tx| tx.remove(m.id)).unwrap();
    let instance = doc.project().block_instance(block).unwrap();
    assert_eq!(instance.detached.as_deref(), Some("=A1-M2 was deleted"));
    // The device, its symbol, the wire and the wire's label went with it.
    assert_eq!(instance.members.len(), 2);
    assert!(
        instance
            .members
            .iter()
            .all(|m| doc.project().contains(m.actual))
    );
    doc.undo().unwrap();
    let instance = doc.project().block_instance(block).unwrap();
    assert!(instance.detached.is_none());
    assert_eq!(instance.members.len(), 6);

    // Deleting the page deletes the placement record with the drawing.
    doc.transact("Delete page", |tx| tx.remove(d.p2)).unwrap();
    assert_eq!(doc.project().block_instances().count(), 0);
    assert_eq!(doc.project().block_definitions().count(), 1);
}

#[test]
fn a_connection_inside_the_block_detaches_it() {
    let mut doc = Document::default();
    let d = drawing(&mut doc);
    let (block, _) = doc
        .transact("Place", |tx| {
            let def = tx.define_block(
                "dol",
                &[d.q1.into(), d.m1.into()],
                Point::default(),
                power(4.0),
            )?;
            tx.place_block(def, d.p2, Point::default(), Vec::new())
        })
        .unwrap();
    let project = doc.project();
    let symbol_of = |letter: &str| {
        let device = device_of(project, project.block_instance(block).unwrap(), letter);
        project.symbols_of_device(device.id)[0].id
    };
    let (q, m) = (symbol_of("Q"), symbol_of("M"));
    // A wire out to the rest of the circuit is how a block is used.
    doc.transact("Wire out", |tx| {
        let lamp = tx.place_symbol(
            d.p2,
            "x",
            Placement::at(Point::new(60.0, 0.0)),
            two_pins(),
            SymbolRole::Unassigned,
        )?;
        tx.connect(Endpoint::pin(m, 1), Endpoint::pin(lamp, 0))
            .map(|_| ())
    })
    .unwrap();
    assert!(
        doc.project()
            .block_instance(block)
            .unwrap()
            .detached
            .is_none()
    );
    // A new connection between two members changes the circuit.
    doc.transact("Wire in", |tx| {
        tx.connect(Endpoint::pin(q, 0), Endpoint::pin(m, 1))
            .map(|_| ())
    })
    .unwrap();
    assert_eq!(
        doc.project()
            .block_instance(block)
            .unwrap()
            .detached
            .as_deref(),
        Some("connection =A1-Q2:1 to =A1-M2:2 added")
    );
    doc.undo().unwrap();
    assert!(
        doc.project()
            .block_instance(block)
            .unwrap()
            .detached
            .is_none()
    );

    // A new device with its first wire in a member stays linked ...
    let detached = |doc: &Document| {
        doc.project()
            .block_instance(block)
            .unwrap()
            .detached
            .clone()
    };
    let k9 = doc
        .transact("Relay", |tx| {
            let k = tx.add_device(tag("=A1-K9"), "Relay")?;
            let s = tx.place_symbol(
                d.p2,
                "x",
                Placement::at(Point::new(20.0, 40.0)),
                two_pins(),
                SymbolRole::Device {
                    device: k,
                    main: true,
                },
            )?;
            tx.connect(Endpoint::pin(s, 0), Endpoint::pin(q, 0))?;
            Ok(s)
        })
        .unwrap()
        .0;
    assert_eq!(detached(&doc), None);
    // ... and is inside once its wires all land on members.
    doc.transact("Wire in", |tx| {
        tx.connect(Endpoint::pin(k9, 1), Endpoint::pin(m, 0))
            .map(|_| ())
    })
    .unwrap();
    assert_eq!(
        detached(&doc).as_deref(),
        Some("device =A1-K9 added inside the block")
    );
}

#[test]
fn what_a_block_cannot_hold() {
    let mut doc = Document::default();
    let d = drawing(&mut doc);
    let err = doc
        .transact("Define", |tx| {
            tx.define_block("t", &[d.terminal.into()], Point::default(), Vec::new())
        })
        .unwrap_err();
    assert_eq!(err, ModelError::NotInBlock(d.terminal.into()));
    let err = doc
        .transact("Define", |tx| {
            tx.define_block("a", &[d.outside.into()], Point::default(), Vec::new())?;
            tx.define_block("a", &[d.outside.into()], Point::default(), Vec::new())
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::DuplicateBlock(_)));
    let err = doc
        .transact("Define", |tx| {
            let mut twice = power(4.0);
            twice.extend(power(5.0));
            tx.define_block("b", &[d.outside.into()], Point::default(), twice)
        })
        .unwrap_err();
    assert!(matches!(
        err,
        ModelError::InvalidValue {
            field: "parameters",
            ..
        }
    ));
    assert_eq!(doc.project().block_definitions().count(), 0);
    let _ = d.p1;
}
