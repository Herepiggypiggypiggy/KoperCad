//! Connected terminals: bridged symbol, jumper, wires and undo.

use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, Project, SymbolId, SymbolRole,
    TerminalId, WireId,
};
use novo_terminals::*;

struct Strip {
    terminals: Vec<TerminalId>,
    wires: Vec<WireId>,
}

/// Three wires from a block to a block, a terminal dropped in each of them
/// in a row 8 mm apart.
fn three_terminals(doc: &mut Document) -> Strip {
    let (wires, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let top = tx.add_device(DeviceTag::parse("-K1").unwrap(), "Contactor")?;
            let bottom = tx.add_device(DeviceTag::parse("-M1").unwrap(), "Motor")?;
            let pins = |names: &[&str]| -> Vec<Pin> {
                names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| Pin::new(*n, 8.0 * i as f64, 0.0))
                    .collect()
            };
            let k = tx.place_symbol(
                page,
                "test/block",
                Placement::at(Point::new(40.0, 20.0)),
                pins(&["2", "4", "6"]),
                SymbolRole::Device {
                    device: top,
                    main: true,
                },
            )?;
            let m = tx.place_symbol(
                page,
                "test/block",
                Placement::at(Point::new(40.0, 100.0)),
                pins(&["U1", "V1", "W1"]),
                SymbolRole::Device {
                    device: bottom,
                    main: true,
                },
            )?;
            let mut wires = Vec::new();
            for i in 0..3u16 {
                wires.push(tx.connect(Endpoint::pin(k, i), Endpoint::pin(m, i))?);
            }
            Ok((page, wires))
        })
        .unwrap();
    let (page, wires) = wires;
    let (placed, _) = doc
        .transact("Terminals", |tx| {
            let strip = strip_in_use(tx, page)?;
            let drops: Vec<Drop> = wires
                .iter()
                .map(|&w| {
                    let x = tx.project().wire(w).unwrap();
                    let from = tx.project().endpoint_position(x.from).unwrap();
                    Drop {
                        wire: w,
                        at: Point::new(from.x, 60.0),
                    }
                })
                .collect();
            place_terminals(tx, strip, &drops)
        })
        .unwrap();
    Strip {
        terminals: placed.iter().map(|p| p.terminal).collect(),
        wires,
    }
}

fn symbols_of(project: &Project, t: TerminalId) -> Vec<SymbolId> {
    project
        .symbols_of_terminal(t)
        .into_iter()
        .map(|s| s.id)
        .collect()
}

fn wire_ends_on(project: &Project, symbol: SymbolId) -> usize {
    project
        .wires()
        .flat_map(|w| [w.from, w.to])
        .filter(|e| matches!(e, Endpoint::Pin { symbol: s, .. } if *s == symbol))
        .count()
}

#[test]
fn connecting_draws_one_bridged_symbol_and_keeps_the_wires() {
    let mut doc = Document::default();
    let s = three_terminals(&mut doc);
    let wires_before = doc.project().wires().count();
    let (group, _) = doc
        .transact("Connect", |tx| connect(tx, &s.terminals))
        .unwrap();
    let project = doc.project();
    let drawn = symbols_of(project, s.terminals[0]);
    assert_eq!(drawn.len(), 1);
    assert_eq!(
        symbols_of(project, s.terminals[2]),
        drawn,
        "all three share one drawing"
    );
    let symbol = project.symbol(drawn[0]).unwrap();
    assert_eq!(symbol.symbol.0, "iec/terminals/terminal_bridged_3");
    assert_eq!(symbol.pins.len(), 6);
    assert_eq!(symbol.bridges, vec![vec![0, 1], vec![2, 3], vec![4, 5]]);
    assert_eq!(wire_ends_on(project, drawn[0]), 6);
    assert_eq!(project.wires().count(), wires_before);
    for &t in &s.terminals {
        assert_eq!(project.terminal(t).unwrap().jumper, Some(group));
    }
    for &w in &s.wires {
        assert!(project.wire(w).is_some());
    }
    doc.undo().unwrap();
    assert_eq!(symbols_of(doc.project(), s.terminals[0]).len(), 1);
    assert_eq!(doc.project().terminal(s.terminals[0]).unwrap().jumper, None);
}

#[test]
fn terminals_that_are_not_side_by_side_are_refused_unchanged() {
    let mut doc = Document::default();
    let s = three_terminals(&mut doc);
    let before = (
        doc.project().symbols().cloned().collect::<Vec<_>>(),
        doc.project().terminals().cloned().collect::<Vec<_>>(),
        doc.project().wires().cloned().collect::<Vec<_>>(),
    );
    let err = doc
        .transact("Connect", |tx| {
            connect(tx, &[s.terminals[0], s.terminals[2]])
        })
        .unwrap_err();
    assert!(err.to_string().contains("side by side"), "{err}");
    let err = doc
        .transact("Connect", |tx| connect(tx, &s.terminals[..1]))
        .unwrap_err();
    assert!(err.to_string().contains("2 or 3"), "{err}");
    assert_eq!(
        before,
        (
            doc.project().symbols().cloned().collect::<Vec<_>>(),
            doc.project().terminals().cloned().collect::<Vec<_>>(),
            doc.project().wires().cloned().collect::<Vec<_>>(),
        )
    );
}

#[test]
fn disconnecting_restores_single_terminals_without_a_jumper() {
    let mut doc = Document::default();
    let s = three_terminals(&mut doc);
    doc.transact("Connect", |tx| connect(tx, &s.terminals[..2]))
        .unwrap();
    let bridged = symbols_of(doc.project(), s.terminals[0])[0];
    assert_eq!(
        doc.project().symbol(bridged).unwrap().symbol.0,
        "iec/terminals/terminal_bridged_2"
    );
    let (done, _) = doc
        .transact("Disconnect", |tx| disconnect(tx, bridged))
        .unwrap();
    assert!(done);
    let project = doc.project();
    assert!(project.symbol(bridged).is_none());
    for &t in &s.terminals[..2] {
        let drawn = symbols_of(project, t);
        assert_eq!(drawn.len(), 1);
        assert_eq!(
            project.symbol(drawn[0]).unwrap().symbol.0,
            "iec/terminals/terminal"
        );
        assert_eq!(wire_ends_on(project, drawn[0]), 2);
        assert_eq!(project.terminal(t).unwrap().jumper, None);
    }
}

#[test]
fn an_existing_jumper_group_is_kept_and_mixed_groups_are_refused() {
    let mut doc = Document::default();
    let s = three_terminals(&mut doc);
    let [t0, t1, t2] = [s.terminals[0], s.terminals[1], s.terminals[2]];
    doc.transact("Jumper", |tx| set_jumper(tx, &[t1, t2], Some(5)))
        .unwrap();
    doc.transact("Connect", |tx| connect(tx, &[t0, t1]))
        .unwrap();
    let group = |d: &Document, t: TerminalId| d.project().terminal(t).unwrap().jumper;
    assert_eq!(
        [group(&doc, t0), group(&doc, t1), group(&doc, t2)],
        [Some(5); 3]
    );
    let bridged = symbols_of(doc.project(), t0)[0];
    doc.transact("Disconnect", |tx| disconnect(tx, bridged))
        .unwrap();
    assert_eq!(
        [group(&doc, t0), group(&doc, t1), group(&doc, t2)],
        [None, Some(5), Some(5)],
        "only the join connect made is taken out"
    );

    // Two different groups: refused, nothing changed.
    doc.transact("Jumper", |tx| set_jumper(tx, &[t0], Some(9)))
        .unwrap();
    let before = doc.project().symbols().cloned().collect::<Vec<_>>();
    let err = doc
        .transact("Connect", |tx| connect(tx, &[t0, t1]))
        .unwrap_err();
    assert!(err.to_string().contains("different jumper groups"), "{err}");
    assert_eq!(before, doc.project().symbols().cloned().collect::<Vec<_>>());
    assert_eq!([group(&doc, t0), group(&doc, t1)], [Some(9), Some(5)]);
}

#[test]
fn terminals_of_two_strips_are_refused_unchanged() {
    let mut doc = Document::default();
    let s = three_terminals(&mut doc);
    doc.transact("Move", |tx| {
        let other = strip_by_tag(tx, "-X9")?;
        tx.update::<novo_core::Terminal>(s.terminals[1], |t| t.strip = other)
    })
    .unwrap();
    let before = (
        doc.project().symbols().cloned().collect::<Vec<_>>(),
        doc.project().terminals().cloned().collect::<Vec<_>>(),
    );
    let err = doc
        .transact("Connect", |tx| connect(tx, &s.terminals[..2]))
        .unwrap_err();
    assert!(err.to_string().contains("one strip"), "{err}");
    assert_eq!(
        before,
        (
            doc.project().symbols().cloned().collect::<Vec<_>>(),
            doc.project().terminals().cloned().collect::<Vec<_>>(),
        )
    );
}
