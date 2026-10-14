//! WS-11 acceptance: WORKFLOW.md J5 plus the "Lessons from EPLAN" rows on
//! terminals.

use novo_core::{
    DeviceId, DeviceTag, Document, Endpoint, JoinLink, ModelError, PageId, Pin, Placement, Point,
    PotentialKind, Project, Result, SameName, SymbolId, SymbolRole, TerminalKind, TerminalRules,
    Tx, Value, WireId,
};
use novo_terminals::*;

/// The records an edit could touch, to show a refused edit changed nothing.
fn snapshot(
    p: &Project,
) -> (
    Vec<novo_core::Terminal>,
    Vec<novo_core::SymbolInstance>,
    Vec<novo_core::Wire>,
) {
    (
        p.terminals().cloned().collect(),
        p.symbols().cloned().collect(),
        p.wires().cloned().collect(),
    )
}

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

/// A device symbol with pins in a row at `y`, 8 mm apart.
fn row_of_pins(
    tx: &mut Tx<'_>,
    page: PageId,
    device: DeviceId,
    names: &[&str],
    x: f64,
    y: f64,
) -> Result<SymbolId> {
    let pins = names
        .iter()
        .enumerate()
        .map(|(i, n)| Pin::new(*n, 8.0 * i as f64, 0.0))
        .collect();
    tx.place_symbol(
        page,
        "test/block",
        Placement::at(Point::new(x, y)),
        pins,
        SymbolRole::Device { device, main: true },
    )
}

fn feed(
    tx: &mut Tx<'_>,
    page: PageId,
    potential: novo_core::PotentialId,
    x: f64,
    y: f64,
) -> Result<SymbolId> {
    tx.place_symbol(
        page,
        "iec/connectivity/potential_feed",
        Placement::at(Point::new(x, y)),
        vec![Pin::new("1", 0.0, 0.0)],
        SymbolRole::PotentialFeed { potential },
    )
}

struct Motor {
    page: PageId,
    wires: Vec<WireId>,
}

/// Contactor `-K1` at y = 20 wired straight down to motor `-M1` at y = 100:
/// L1, L2, L3 and PE, x = 40, 48, 56, 64.
fn motor_circuit(tx: &mut Tx<'_>) -> Result<Motor> {
    let page = tx.add_page("1")?;
    let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
    let l1 = tx.add_potential("L1", PotentialKind::Line)?;
    let k1 = tx.add_device(tag("-K1"), "Contactor")?;
    let m1 = tx.add_device(tag("-M1"), "Motor")?;
    let k = row_of_pins(tx, page, k1, &["2", "4", "6"], 40.0, 20.0)?;
    let m = row_of_pins(tx, page, m1, &["U1", "V1", "W1", "PE"], 40.0, 100.0)?;
    let pe_feed = feed(tx, page, pe, 64.0, 20.0)?;
    let l1_feed = feed(tx, page, l1, 40.0, 10.0)?;
    tx.connect(Endpoint::pin(l1_feed, 0), Endpoint::pin(k, 0))?;
    let mut wires = Vec::new();
    for i in 0..3u16 {
        wires.push(tx.connect(Endpoint::pin(k, i), Endpoint::pin(m, i))?);
    }
    wires.push(tx.connect(Endpoint::pin(pe_feed, 0), Endpoint::pin(m, 3))?);
    Ok(Motor { page, wires })
}

fn drops(wires: &[WireId], y: f64, project: &Project) -> Vec<Drop> {
    wires
        .iter()
        .map(|&w| {
            let x = project
                .endpoint_position(project.wire(w).unwrap().from)
                .unwrap()
                .x;
            Drop {
                wire: w,
                at: Point::new(x, y),
            }
        })
        .collect()
}

#[test]
fn a_drag_across_motor_wires_places_a_numbered_strip_in_one_step() {
    let mut doc = Document::default();
    let (motor, _) = doc.transact("Draw", motor_circuit).unwrap();
    let before = doc.project().clone();
    let (strip, _) = doc
        .transact("Terminals", |tx| {
            let strip = strip_in_use(tx, motor.page)?;
            let d = drops(&motor.wires, 60.0, tx.project());
            place_terminals(tx, strip, &d)?;
            Ok(strip)
        })
        .unwrap();
    let view = strip_view(doc.project(), strip);
    assert_eq!(view.tag, "-X1");
    let names: Vec<_> = view.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["1", "2", "3", "PE"]);
    let kinds: Vec<_> = view.rows.iter().map(|r| r.kind).collect();
    assert_eq!(kinds[..3], [TerminalKind::Feedthrough; 3]);
    assert_eq!(kinds[3], TerminalKind::ProtectiveEarth);
    for (row, (int, ext)) in view.rows.iter().zip([
        ("-K1:2", "-M1:U1"),
        ("-K1:4", "-M1:V1"),
        ("-K1:6", "-M1:W1"),
        ("PE", "-M1:PE"),
    ]) {
        assert_eq!(row.state(), State::Through, "{}", row.name);
        assert_eq!(row.external_targets()[0].designation, ext);
        assert_eq!(row.internal_targets()[0].designation, int);
        assert_eq!(row.build.clamps, 1);
    }
    // The PE terminal is drawn with the PE symbol.
    let pe_symbol = &view.rows[3].occurrences[0].symbol;
    assert_eq!(
        doc.project().symbol(*pe_symbol).unwrap().symbol.0,
        "iec/terminals/terminal_pe"
    );
    // Contactor and motor are still connected, now through the terminals.
    let net = doc.project().netlist();
    let k = doc
        .project()
        .symbols()
        .find(|s| s.symbol.0 == "test/block")
        .unwrap()
        .id;
    let m = doc
        .project()
        .symbols()
        .filter(|s| s.symbol.0 == "test/block")
        .last()
        .unwrap()
        .id;
    assert!(net.connected(Endpoint::pin(k, 0), Endpoint::pin(m, 0)));
    // The pins sit exactly on the old wire line.
    for row in &view.rows {
        let s = doc.project().symbol(row.occurrences[0].symbol).unwrap();
        let p1 = s.pin_position(0).unwrap();
        let p2 = s.pin_position(1).unwrap();
        assert_eq!(p1.x, p2.x);
        assert_eq!(p1.y, 56.0);
        assert_eq!(p2.y, 64.0);
    }
    // One undo step removes all of it.
    doc.undo().unwrap();
    assert_eq!(doc.project().terminals().count(), 0);
    assert_eq!(doc.project().wires().count(), before.wires().count());
}

#[test]
fn a_terminal_dropped_near_the_far_end_keeps_its_sides() {
    // The motor wires run 80 mm down from -K1 to -M1; a drop close to the
    // motor still has -K1 on pin 1 (internal) and -M1 on pin 2 (external),
    // and no wire runs through the terminal.
    let mut doc = Document::default();
    let (motor, _) = doc.transact("Draw", motor_circuit).unwrap();
    let ((strip, placed), _) = doc
        .transact("Terminals", |tx| {
            let strip = strip_in_use(tx, motor.page)?;
            let d = drops(&motor.wires, 92.0, tx.project());
            Ok((strip, place_terminals(tx, strip, &d)?))
        })
        .unwrap();
    let p = doc.project();
    let view = strip_view(p, strip);
    for (row, (int, ext)) in view.rows.iter().zip([
        ("-K1:2", "-M1:U1"),
        ("-K1:4", "-M1:V1"),
        ("-K1:6", "-M1:W1"),
        ("PE", "-M1:PE"),
    ]) {
        assert_eq!(row.internal_targets()[0].designation, int);
        assert_eq!(row.external_targets()[0].designation, ext);
    }
    for t in &placed {
        let s = p.symbol(t.symbol).unwrap();
        let (top, bottom) = (s.pin_position(0).unwrap(), s.pin_position(1).unwrap());
        assert!(top.y < bottom.y);
        // Each wire ends on the pin it reaches without crossing the symbol.
        let int = p.wire(t.internal_wire).unwrap();
        assert_eq!(int.to, Endpoint::pin(t.symbol, 0));
        assert!(p.endpoint_position(int.from).unwrap().y <= top.y);
        let ext = p.wire(t.external_wire).unwrap();
        assert_eq!(ext.from, Endpoint::pin(t.symbol, 1));
        assert!(p.endpoint_position(ext.to).unwrap().y >= bottom.y);
    }
}

#[test]
fn numbers_stay_unique_and_continuous() {
    let mut doc = Document::default();
    let (motor, _) = doc.transact("Draw", motor_circuit).unwrap();
    let (strip, _) = doc
        .transact("Terminals", |tx| {
            let strip = strip_by_tag(tx, "X2")?;
            let d = drops(&motor.wires[..3], 60.0, tx.project());
            place_terminals(tx, strip, &d)?;
            Ok(strip)
        })
        .unwrap();
    // Remove terminal 2; the next one fills the gap.
    let two = doc
        .project()
        .terminals_of_strip(strip)
        .iter()
        .find(|t| t.name == "2")
        .unwrap()
        .id;
    doc.transact("Remove", |tx| tx.remove(two)).unwrap();
    assert_eq!(next_free_number(doc.project(), strip), "2");
    doc.transact("Spares", |tx| add_spares(tx, strip, 2))
        .unwrap();
    let view = strip_view(doc.project(), strip);
    let names: Vec<_> = view.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["1", "3", "2", "4"]);
    assert!(view.rows[2].is_spare());
    // Renumber in strip order.
    doc.transact("Renumber", |tx| renumber(tx, strip)).unwrap();
    let view = strip_view(doc.project(), strip);
    let names: Vec<_> = view.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["1", "2", "3", "4"]);
    assert_eq!(view.rows[1].external_targets()[0].designation, "-M1:W1");
}

/// Two pages, each with one wire from `-K1` / `-K2` down to `-M1` / `-M2`.
fn two_pages(tx: &mut Tx<'_>) -> Result<[(PageId, WireId); 2]> {
    let mut out = Vec::new();
    for i in 1..=2 {
        let page = tx.add_page(i.to_string())?;
        let k = tx.add_device(tag(&format!("-K{i}")), "")?;
        let m = tx.add_device(tag(&format!("-M{i}")), "")?;
        let ks = row_of_pins(tx, page, k, &["14"], 40.0, 20.0)?;
        let ms = row_of_pins(tx, page, m, &["1"], 40.0, 100.0)?;
        out.push((
            page,
            tx.connect(Endpoint::pin(ks, 0), Endpoint::pin(ms, 0))?,
        ));
    }
    Ok(out.try_into().unwrap())
}

#[test]
fn same_name_on_two_pages_joins_into_one_terminal() {
    let mut doc = Document::default();
    let (pages, _) = doc.transact("Draw", two_pages).unwrap();
    let (placed, _) = doc
        .transact("Terminals", |tx| {
            let strip = strip_by_tag(tx, "-X2")?;
            let a = place_named(
                tx,
                strip,
                Drop {
                    wire: pages[0].1,
                    at: Point::new(40.0, 60.0),
                },
                "1",
                None,
            )?;
            let b = place_named(
                tx,
                strip,
                Drop {
                    wire: pages[1].1,
                    at: Point::new(40.0, 60.0),
                },
                "1",
                None,
            )?;
            Ok((strip, a, b))
        })
        .unwrap();
    let (strip, a, b) = placed;
    assert!(!a.joined);
    assert!(b.joined);
    assert_eq!(a.terminal, b.terminal);
    let view = strip_view(doc.project(), strip);
    assert_eq!(view.rows.len(), 1);
    let row = &view.rows[0];
    assert_eq!(row.occurrences.len(), 2);
    let int: Vec<_> = row
        .internal_targets()
        .iter()
        .map(|t| t.designation.clone())
        .collect();
    let ext: Vec<_> = row
        .external_targets()
        .iter()
        .map(|t| t.designation.clone())
        .collect();
    assert_eq!(int, ["-K1:14", "-K2:14"]);
    assert_eq!(ext, ["-M1:1", "-M2:1"]);
    // Built from two 1-in/1-out clamps and a connector.
    assert_eq!(
        row.build,
        Build {
            clamps: 2,
            links: 1,
            link: JoinLink::Connector
        }
    );
    // Electrically one node across the pages.
    let net = doc.project().netlist();
    assert!(net.connected(Endpoint::pin(a.symbol, 0), Endpoint::pin(b.symbol, 1)));
    // The rule is spelled out.
    let why = explain(doc.project(), a.terminal).join(" ");
    assert!(why.contains("drawn 2 times (pages 1, 2)"), "{why}");
    assert!(why.contains("2 clamps linked by 1 connector"), "{why}");
    assert!(
        check(doc.project(), &net).is_empty(),
        "{:?}",
        check(doc.project(), &net)
    );
    // The screen shows it as one line.
    let text = screen(doc.project(), strip).to_text();
    assert!(
        text.contains("-K1:14, -K2:14 | 1        | -M1:1, -M2:1"),
        "{text}"
    );
    assert_eq!(
        screen(doc.project(), strip).rows[0].build,
        "2 clamps + 1 connector"
    );
}

fn drop_at(wire: WireId) -> Drop {
    Drop {
        wire,
        at: Point::new(40.0, 60.0),
    }
}

#[test]
fn refuse_rule_refuses_a_second_record_and_flags_a_second_drawing() {
    let mut doc = Document::default();
    let (pages, _) = doc.transact("Draw", two_pages).unwrap();
    let ((strip, first), _) = doc
        .transact("Terminals", |tx| {
            set_rules(
                tx,
                TerminalRules {
                    same_name: SameName::Refuse,
                    join_link: JoinLink::Jumper,
                },
            )?;
            let strip = strip_by_tag(tx, "-X2")?;
            let first = place_named(tx, strip, drop_at(pages[0].1), "1", None)?;
            Ok((strip, first))
        })
        .unwrap();
    assert_eq!(rules(doc.project()).same_name, SameName::Refuse);
    assert_eq!(
        rule_sentence(rules(doc.project())),
        "Each terminal name may be used once per strip."
    );
    // A second record with the name is refused by novo-core; nothing changes.
    let before = doc.project().clone();
    let err = doc
        .transact("Again", |tx| {
            place_named(tx, strip, drop_at(pages[1].1), "1", None)
        })
        .unwrap_err();
    assert!(matches!(err, ModelError::DuplicateTerminal(_)), "{err:?}");
    assert_eq!(doc.project().terminals().count(), 1);
    assert_eq!(doc.project().wires().count(), before.wires().count());
    // Drawing the same record a second time is connected but flagged.
    doc.transact("Copy", |tx| {
        tx.place_symbol(
            pages[1].0,
            "iec/terminals/terminal",
            Placement::at(Point::new(120.0, 40.0)),
            terminal_pins(8.0),
            SymbolRole::Terminal {
                terminals: vec![first.terminal],
            },
        )
    })
    .unwrap();
    let issues = check(doc.project(), &doc.project().netlist());
    let duplicates: Vec<_> = issues
        .iter()
        .filter(|i| i.kind == IssueKind::DuplicateName)
        .collect();
    assert_eq!(duplicates.len(), 1, "{issues:?}");
    assert_eq!(duplicates[0].terminal, first.terminal);
    assert_ne!(duplicates[0].symbol, Some(first.symbol));
    let why = explain(doc.project(), first.terminal).join(" ");
    assert!(
        why.contains("All drawings are this one terminal and are connected"),
        "{why}"
    );
    assert!(!why.contains("only the first counts"), "{why}");
}

#[test]
fn duplicate_records_follow_the_core_rule() {
    // A project without a rule (an old file): same-named records are
    // separate terminals and the checker lists them.
    let mut doc = Document::new(Project::default());
    let (pages, _) = doc.transact("Draw", two_pages).unwrap();
    assert_eq!(rules(doc.project()).same_name, SameName::Off);
    let (strip, _) = doc
        .transact("Terminals", |tx| {
            let strip = strip_by_tag(tx, "-X2")?;
            for (_, wire) in pages {
                let placed = place_named(tx, strip, drop_at(wire), "7", None)?;
                assert!(!placed.joined);
            }
            Ok(strip)
        })
        .unwrap();
    let view = strip_view(doc.project(), strip);
    assert_eq!(view.rows.len(), 2);
    let issues = check(doc.project(), &doc.project().netlist());
    assert_eq!(
        issues.iter().map(|i| &i.kind).collect::<Vec<_>>(),
        [&IssueKind::DuplicateName]
    );
    assert_eq!(issues[0].severity, Severity::Warning);
    assert_eq!(issues[0].terminal, view.rows[1].terminals[0]);
    let symbols: Vec<_> = view.rows.iter().map(|r| r.occurrences[0].symbol).collect();
    let connected = |doc: &Document| {
        doc.project()
            .netlist()
            .connected(Endpoint::pin(symbols[0], 0), Endpoint::pin(symbols[1], 0))
    };
    assert!(!connected(&doc));
    assert!(
        explain(doc.project(), view.rows[0].terminals[0])
            .iter()
            .any(|l| l.contains("not connected"))
    );

    // Join: the netlist joins the records; one row, no finding.
    doc.transact("Join rule", |tx| {
        set_rules(
            tx,
            TerminalRules {
                same_name: SameName::Join,
                join_link: JoinLink::Connector,
            },
        )
    })
    .unwrap();
    assert!(connected(&doc));
    let view = strip_view(doc.project(), strip);
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].terminals.len(), 2);
    assert_eq!(view.rows[0].build.clamps, 2);
    assert!(check(doc.project(), &doc.project().netlist()).is_empty());

    // Refuse is refused while the strip has two records with the name.
    let refuse = TerminalRules {
        same_name: SameName::Refuse,
        join_link: JoinLink::Connector,
    };
    let err = doc
        .transact("Refuse rule", |tx| set_rules(tx, refuse))
        .unwrap_err();
    assert!(matches!(err, ModelError::DuplicateTerminal(_)), "{err:?}");

    // Merging the records keeps them connected and makes Refuse possible.
    let (removed, _) = doc
        .transact("Join", |tx| join_duplicates(tx, strip))
        .unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(doc.project().terminals_of_strip(strip).len(), 1);
    assert!(connected(&doc));
    doc.transact("Refuse rule", |tx| set_rules(tx, refuse))
        .unwrap();
    // Under Refuse the one record drawn twice is still flagged once.
    let issues = check(doc.project(), &doc.project().netlist());
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0].kind, IssueKind::DuplicateName);
    assert_eq!(issues[0].symbol, Some(symbols[1]));
}

#[test]
fn join_duplicates_carries_jumper_part_and_properties_over() {
    let mut doc = Document::new(Project::default());
    let (pages, _) = doc.transact("Draw", two_pages).unwrap();
    let ((strip, keep, gone, part), _) = doc
        .transact("Terminals", |tx| {
            let strip = strip_by_tag(tx, "-X2")?;
            let a = place_named(tx, strip, drop_at(pages[0].1), "7", None)?.terminal;
            let b = place_named(tx, strip, drop_at(pages[1].1), "7", None)?.terminal;
            let part = tx.add_part("Phoenix Contact", "3209510")?;
            tx.update::<novo_core::Terminal>(b, |t| {
                t.jumper = Some(3);
                t.part = Some(part);
            })?;
            tx.set_property(b, "note", Some("from b".into()))?;
            Ok((strip, a, b, part))
        })
        .unwrap();
    let (removed, _) = doc
        .transact("Join", |tx| join_duplicates(tx, strip))
        .unwrap();
    assert_eq!(removed, [gone]);
    let t = doc.project().terminal(keep).unwrap();
    assert_eq!(t.jumper, Some(3));
    assert_eq!(t.part, Some(part));
    assert_eq!(t.properties.get("note"), Some(&Value::from("from b")));
}

#[test]
fn single_sided_and_unwired_terminals_are_visible() {
    let mut doc = Document::default();
    let (pages, _) = doc.transact("Draw", two_pages).unwrap();
    let ((strip, placed), _) = doc
        .transact("Terminals", |tx| {
            let strip = strip_by_tag(tx, "-X3")?;
            let p = place_named(
                tx,
                strip,
                Drop {
                    wire: pages[0].1,
                    at: Point::new(40.0, 60.0),
                },
                "1",
                Some(TerminalKind::Fuse),
            )?;
            Ok((strip, p))
        })
        .unwrap();
    assert_eq!(
        doc.project().symbol(placed.symbol).unwrap().symbol.0,
        "iec/terminals/terminal_fused"
    );
    assert_eq!(
        strip_view(doc.project(), strip).rows[0].state(),
        State::Through
    );
    doc.transact("Cut field side", |tx| tx.remove(placed.external_wire))
        .unwrap();
    let row = &strip_view(doc.project(), strip).rows[0];
    assert_eq!(row.state(), State::InternalEnd);
    let issues = check(doc.project(), &doc.project().netlist());
    assert_eq!(issues[0].kind, IssueKind::SingleSided);
    doc.transact("Cut cabinet side", |tx| tx.remove(placed.internal_wire))
        .unwrap();
    let issues = check(doc.project(), &doc.project().netlist());
    assert_eq!(issues[0].kind, IssueKind::Unwired);
    assert!(explain(doc.project(), placed.terminal)[0].contains("fuse terminal, not wired"));
}

#[test]
fn a_wire_on_a_pe_named_terminal_gets_a_pe_terminal() {
    let mut doc = Document::default();
    let ((strip, wire), _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
            let x4 = strip_by_tag(tx, "X4")?;
            let pe = tx.add_terminal(x4, "PE")?;
            tx.update::<novo_core::Terminal>(pe, |t| t.kind = TerminalKind::ProtectiveEarth)?;
            let t = tx.place_symbol(
                page,
                "iec/terminals/terminal_pe",
                Placement::at(Point::new(40.0, 10.0)),
                terminal_pins(8.0),
                SymbolRole::Terminal {
                    terminals: vec![pe],
                },
            )?;
            let m = tx.add_device(tag("-M1"), "")?;
            let ms = row_of_pins(tx, page, m, &["GND"], 40.0, 100.0)?;
            let wire = tx.connect(Endpoint::pin(t, 1), Endpoint::pin(ms, 0))?;
            Ok((strip_by_tag(tx, "X5")?, wire))
        })
        .unwrap();
    assert_eq!(
        kind_for_name(doc.project(), "PE"),
        Some(TerminalKind::ProtectiveEarth)
    );
    // With novo-core's rule off, the name defines nothing.
    doc.transact("Rule off", |tx| {
        tx.set_info(|i| i.rules.potentials.from_terminal_names = false)
    })
    .unwrap();
    assert_eq!(kind_for_name(doc.project(), "PE"), None);
    let (placed, _) = doc
        .transact("T", |tx| {
            place_terminals(
                tx,
                strip,
                &[Drop {
                    wire,
                    at: Point::new(40.0, 60.0),
                }],
            )
        })
        .unwrap();
    let t = doc.project().terminal(placed[0].terminal).unwrap();
    assert_eq!(t.kind, TerminalKind::Feedthrough);
    assert_eq!(t.name, "1");
    doc.undo().unwrap();
    doc.undo().unwrap();
    let (placed, _) = doc
        .transact("T", |tx| {
            place_terminals(
                tx,
                strip,
                &[Drop {
                    wire,
                    at: Point::new(40.0, 60.0),
                }],
            )
        })
        .unwrap();
    let t = doc.project().terminal(placed[0].terminal).unwrap();
    assert_eq!(t.kind, TerminalKind::ProtectiveEarth);
    assert_eq!(t.name, "PE");
}

#[test]
fn horizontal_wires_and_no_room() {
    let mut doc = Document::default();
    let (wires, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let a = tx.add_device(tag("-A1"), "")?;
            let b = tx.add_device(tag("-A2"), "")?;
            let sa = row_of_pins(tx, page, a, &["1"], 10.0, 50.0)?;
            let sb = row_of_pins(tx, page, b, &["1", "2"], 50.0, 50.0)?;
            let long = tx.connect(Endpoint::pin(sb, 0), Endpoint::pin(sa, 0))?;
            let short = tx.connect(Endpoint::pin(sb, 1), Endpoint::pin(sb, 0))?;
            Ok((long, short))
        })
        .unwrap();
    let (placed, _) = doc
        .transact("T", |tx| {
            let strip = strip_by_tag(tx, "X1")?;
            place_terminals(
                tx,
                strip,
                &[Drop {
                    wire: wires.0,
                    at: Point::new(12.0, 51.0),
                }],
            )
        })
        .unwrap();
    let s = doc.project().symbol(placed[0].symbol).unwrap();
    // Pushed inwards so both pins fit; pin 1 on the left.
    assert_eq!(s.pin_position(0), Some(Point::new(10.0, 50.0)));
    assert_eq!(s.pin_position(1), Some(Point::new(18.0, 50.0)));
    // The wire ran right to left, so the internal (left) part is the new one.
    let int = doc.project().wire(placed[0].internal_wire).unwrap();
    assert_eq!(int.from, Endpoint::pin(placed[0].symbol, 0));
    assert!(
        doc.transact("T", |tx| {
            let strip = strip_by_tag(tx, "X1")?;
            place_terminals(
                tx,
                strip,
                &[Drop {
                    wire: wires.1,
                    at: Point::new(54.0, 50.0),
                }],
            )
        })
        .is_err()
    );
}

#[test]
fn one_style_per_kind_changes_every_terminal() {
    let mut doc = Document::default();
    let (motor, _) = doc.transact("Draw", motor_circuit).unwrap();
    let ((strip, part), _) = doc
        .transact("Terminals", |tx| {
            let strip = strip_in_use(tx, motor.page)?;
            let d = drops(&motor.wires[..3], 60.0, tx.project());
            place_terminals(tx, strip, &d)?;
            let part = tx.add_part("Phoenix Contact", "3209510")?;
            Ok((strip, part))
        })
        .unwrap();
    // A symbol the library does not know is refused.
    let unknown = Style {
        symbol: "iec/terminals/terminal_2".into(),
        part: Some(part),
    };
    assert!(
        doc.transact("Style", |tx| {
            set_strip_style(tx, strip, TerminalKind::Feedthrough, &unknown)
        })
        .is_err()
    );
    // A longer symbol: the connection points are refitted, pin 2 moves down
    // the wire and the wire still ends on it.
    let style = Style {
        symbol: "iec/terminals/terminal_disconnect".into(),
        part: Some(part),
    };
    doc.transact("Style", |tx| {
        set_strip_style(tx, strip, TerminalKind::Feedthrough, &style)
    })
    .unwrap();
    let view = strip_view(doc.project(), strip);
    for row in &view.rows {
        assert_eq!(row.part, Some(part));
        let s = doc.project().symbol(row.occurrences[0].symbol).unwrap();
        assert_eq!(s.symbol.0, "iec/terminals/terminal_disconnect");
        assert_eq!(s.pin_position(0).unwrap().y, 56.0);
        assert_eq!(s.pin_position(1).unwrap().y, 72.0);
        let ext = doc
            .project()
            .wire(row.occurrences[0].external.wires[0])
            .unwrap();
        assert_eq!(doc.project().endpoint_position(ext.from).unwrap().y, 72.0);
    }
    assert!(overrides(doc.project(), strip).is_empty());
    // One terminal overridden on purpose is listed, and the check says so.
    let one = view.rows[1].terminals[0];
    doc.transact("Override", |tx| {
        tx.set_property(one, "symbol", Some("x/odd".into()))
    })
    .unwrap();
    assert_eq!(overrides(doc.project(), strip), [one]);
    let issues = check(doc.project(), &doc.project().netlist());
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, IssueKind::StyleOverride);
    assert_eq!(issues[0].severity, Severity::Info);
    doc.transact("Clear", |tx| clear_override(tx, one)).unwrap();
    assert!(overrides(doc.project(), strip).is_empty());
}

#[test]
fn jumpers_and_reorder() {
    let mut doc = Document::default();
    let (motor, _) = doc.transact("Draw", motor_circuit).unwrap();
    let (strip, _) = doc
        .transact("Terminals", |tx| {
            let strip = strip_in_use(tx, motor.page)?;
            let d = drops(&motor.wires[..3], 60.0, tx.project());
            place_terminals(tx, strip, &d)?;
            Ok(strip)
        })
        .unwrap();
    let ids: Vec<_> = doc
        .project()
        .terminals_of_strip(strip)
        .iter()
        .map(|t| t.id)
        .collect();
    doc.transact("Reorder", |tx| reorder(tx, strip, &[ids[2], ids[0]]))
        .unwrap();
    let names: Vec<_> = strip_view(doc.project(), strip)
        .rows
        .iter()
        .map(|r| r.name.clone())
        .collect();
    assert_eq!(names, ["3", "1", "2"]);
    let j = free_jumper(doc.project(), strip);
    doc.transact("Jumper", |tx| set_jumper(tx, &ids[..2], Some(j)))
        .unwrap();
    assert!(
        explain(doc.project(), ids[0])
            .iter()
            .any(|l| l == "Jumpered to 2 (jumper 1).")
    );
    assert_eq!(free_jumper(doc.project(), strip), 2);
}

#[test]
fn terminal_symbol_geometry_matches_the_library() {
    let library = novo_symbols::Library::builtin().unwrap();
    for kind in [
        TerminalKind::Feedthrough,
        TerminalKind::ProtectiveEarth,
        TerminalKind::Neutral,
        TerminalKind::Fuse,
        TerminalKind::Disconnect,
    ] {
        let symbol = library.get(default_symbol(kind)).unwrap();
        let pins = terminal_pins(terminal_length(kind));
        assert_eq!(symbol.connections.len(), pins.len(), "{kind:?}");
        for (c, p) in symbol.connections.iter().zip(&pins) {
            assert_eq!(c.id, p.name);
            assert_eq!((c.at[0], c.at[1]), (p.offset.x, p.offset.y), "{kind:?}");
        }
    }
}

#[test]
fn gate1_terminal_screen_generates_with_no_manual_drawing() {
    let mut doc = Document::default();
    let starter = novo_tests::motor_starter(&mut doc).unwrap();
    let project = doc.project();
    assert_eq!(strips(project), [starter.x1]);
    let s = screen(project, starter.x1);
    assert_eq!(s.tag, "=A1+CAB-X1");
    let rows: Vec<_> = s
        .rows
        .iter()
        .map(|r| {
            (
                r.internal.join(","),
                r.terminal.as_str(),
                r.external.join(","),
                r.cables.join(","),
            )
        })
        .collect();
    assert_eq!(
        rows[0],
        (
            "=A1+CAB-F2:2".into(),
            "1",
            "=A1+FLD-M1:U1".into(),
            "=A1-W1 BK".into()
        )
    );
    assert_eq!(rows[3].1, "PE");
    assert_eq!(rows[3].0, "PE");
    assert_eq!(rows[3].2, "=A1+FLD-M1:PE");
    let text = s.to_text();
    assert!(text.starts_with("Terminal strip =A1+CAB-X1\n"), "{text}");
    assert_eq!(text.lines().count(), 2 + 1 + 4);
}

#[test]
fn gate1_checks_are_clean() {
    let mut doc = Document::default();
    let starter = novo_tests::motor_starter(&mut doc).unwrap();
    // The fixture's X1:PE is a protective earth terminal, so nothing is
    // flagged.
    let pe = &doc.project().terminals_of_strip(starter.x1)[3];
    assert_eq!(pe.kind, TerminalKind::ProtectiveEarth);
    let issues = check(doc.project(), &doc.project().netlist());
    let messages: Vec<_> = issues.iter().map(|i| &i.message).collect();
    assert!(issues.is_empty(), "{messages:?}");
}

#[test]
fn wires_crossing_a_location_boundary_get_suggested_terminals() {
    let mut doc = Document::default();
    doc.transact("Draw", |tx| {
        let page = tx.add_page("1")?;
        let k1 = tx.add_device(tag("=A1+CAB-K1"), "")?;
        let k2 = tx.add_device(tag("=A1+CAB-K2"), "")?;
        let m1 = tx.add_device(tag("=A1+FLD-M1"), "")?;
        let ks = row_of_pins(tx, page, k1, &["2", "4"], 40.0, 20.0)?;
        let k2s = row_of_pins(tx, page, k2, &["A1"], 80.0, 20.0)?;
        let ms = row_of_pins(tx, page, m1, &["U1", "V1"], 40.0, 100.0)?;
        tx.connect(Endpoint::pin(ks, 0), Endpoint::pin(ms, 0))?;
        tx.connect(Endpoint::pin(ms, 1), Endpoint::pin(ks, 1))?;
        // Inside the cabinet: no terminal needed.
        tx.connect(Endpoint::pin(ks, 1), Endpoint::pin(k2s, 0))?;
        Ok(())
    })
    .unwrap();
    let suggestions = suggest_terminals(doc.project());
    assert_eq!(suggestions.len(), 2);
    assert!(
        suggestions
            .iter()
            .all(|s| s.strip.to_string() == "=A1+CAB-X1")
    );
    assert_eq!(suggestions[0].at, Point::new(40.0, 60.0));
    assert_eq!(
        summarize_suggestions(&suggestions),
        "2 terminals on =A1+CAB-X1"
    );
    let (placed, _) = doc
        .transact("Accept", |tx| accept_suggestions(tx, &suggestions))
        .unwrap();
    assert_eq!(placed.len(), 2);
    let strip = strips(doc.project())[0];
    let s = screen(doc.project(), strip);
    let rows: Vec<_> = s
        .rows
        .iter()
        .map(|r| {
            (
                r.internal.join(","),
                r.terminal.clone(),
                r.external.join(","),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("=A1+CAB-K1:2".into(), "1".into(), "=A1+FLD-M1:U1".into()),
            ("=A1+CAB-K1:4".into(), "2".into(), "=A1+FLD-M1:V1".into()),
        ]
    );
    // Accepted terminals end the suggestions.
    assert!(suggest_terminals(doc.project()).is_empty());
}

#[test]
fn the_side_with_a_strip_is_the_cabinet_and_dismissals_stick() {
    let mut doc = Document::default();
    let (wires, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            // A big field skid with more devices than the small cabinet.
            let k1 = tx.add_device(tag("+CAB-K1"), "")?;
            tx.add_terminal_strip(tag("+CAB-X5"))?;
            let mut field = Vec::new();
            for i in 1..=4 {
                field.push(tx.add_device(tag(&format!("+SKID-M{i}")), "")?);
            }
            let ks = row_of_pins(tx, page, k1, &["2", "4"], 40.0, 20.0)?;
            let m1 = row_of_pins(tx, page, field[0], &["U1", "V1"], 40.0, 100.0)?;
            let a = tx.connect(Endpoint::pin(m1, 0), Endpoint::pin(ks, 0))?;
            let b = tx.connect(Endpoint::pin(m1, 1), Endpoint::pin(ks, 1))?;
            Ok([a, b])
        })
        .unwrap();
    let suggestions = suggest_terminals(doc.project());
    assert_eq!(suggestions.len(), 2);
    assert_eq!(suggestions[0].strip.to_string(), "+CAB-X5");
    assert_eq!(
        doc.project()
            .device(suggestions[0].internal)
            .unwrap()
            .tag
            .to_string(),
        "+CAB-K1"
    );
    doc.transact("Decline", |tx| dismiss_suggestion(tx, wires[0]))
        .unwrap();
    let left = suggest_terminals(doc.project());
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].wire, wires[1]);
    doc.undo().unwrap();
    assert_eq!(suggest_terminals(doc.project()).len(), 2);
}

#[test]
fn rule_sentence_summary_and_rename_by_pattern() {
    let mut doc = Document::default();
    assert_eq!(
        rule_sentence(rules(doc.project())),
        "Terminals with the same name are one terminal, joined with connectors."
    );
    assert_eq!(
        rule_sentence(TerminalRules::default()),
        "Terminals with the same name are separate terminals; the checker lists them."
    );
    let (motor, _) = doc.transact("Draw", motor_circuit).unwrap();
    let (strip, _) = doc
        .transact("T", |tx| {
            let strip = strip_by_tag(tx, "X1")?;
            let d = drops(&motor.wires[..3], 60.0, tx.project());
            for (i, drop) in d.into_iter().enumerate() {
                place_named(tx, strip, drop, &format!("A{}", i + 1), None)?;
            }
            Ok(strip)
        })
        .unwrap();
    let preview = rename_preview(doc.project(), strip, "A", "B");
    assert_eq!(preview.len(), 3);
    assert!(preview.iter().all(|c| c.problem.is_none()));
    let (n, _) = doc
        .transact("Rename", |tx| rename(tx, strip, "A", "B"))
        .unwrap();
    assert_eq!(n, 3);
    let names: Vec<_> = strip_view(doc.project(), strip)
        .rows
        .iter()
        .map(|r| r.name.clone())
        .collect();
    assert_eq!(names, ["B1", "B2", "B3"]);
    doc.undo().unwrap();
    assert_eq!(strip_view(doc.project(), strip).rows[0].name, "A1");
}

/// Review item 1: a kind change redraws with the new symbol and refits its
/// connection points, so the drawn point still meets the wire.
#[test]
fn a_kind_change_refits_the_connection_points() {
    let mut doc = Document::default();
    let (pages, _) = doc.transact("Draw", two_pages).unwrap();
    let ((strip, placed), _) = doc
        .transact("T", |tx| {
            let strip = strip_by_tag(tx, "-X1")?;
            let p = place_named(tx, strip, drop_at(pages[0].1), "1", None)?;
            Ok((strip, p))
        })
        .unwrap();
    let pin2 = |doc: &Document| {
        let s = doc.project().symbol(placed.symbol).unwrap();
        (s.symbol.0.clone(), s.pin_position(1).unwrap())
    };
    let wire_end = |doc: &Document| {
        let w = doc.project().wire(placed.external_wire).unwrap();
        doc.project().endpoint_position(w.from).unwrap()
    };
    assert_eq!(pin2(&doc).1, Point::new(40.0, 64.0));
    doc.transact("Kind", |tx| {
        set_kind(tx, placed.terminal, TerminalKind::Fuse)
    })
    .unwrap();
    let (symbol, at) = pin2(&doc);
    assert_eq!(symbol, "iec/terminals/terminal_fused");
    assert_eq!(at, Point::new(40.0, 72.0));
    assert_eq!(wire_end(&doc), at);
    assert_eq!(
        strip_view(doc.project(), strip).rows[0].state(),
        State::Through
    );
    // And back.
    doc.transact("Kind", |tx| {
        set_kind(tx, placed.terminal, TerminalKind::Feedthrough)
    })
    .unwrap();
    assert_eq!(pin2(&doc).1, Point::new(40.0, 64.0));
    assert_eq!(wire_end(&doc), Point::new(40.0, 64.0));
    // The geometry comes from the library for every built-in kind.
    for kind in [
        TerminalKind::Feedthrough,
        TerminalKind::ProtectiveEarth,
        TerminalKind::Neutral,
        TerminalKind::Fuse,
        TerminalKind::Disconnect,
    ] {
        let key = novo_core::SymbolKey(default_symbol(kind).into());
        assert_eq!(symbol_length(&key).unwrap(), terminal_length(kind));
    }
}

/// Review item 1: when the wire leaves no room for the longer symbol, the
/// change is refused and nothing moves.
#[test]
fn a_kind_change_without_room_is_refused() {
    let mut doc = Document::default();
    let (terminal, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let k = tx.add_device(tag("-K1"), "")?;
            let m = tx.add_device(tag("-M1"), "")?;
            let ks = row_of_pins(tx, page, k, &["14"], 40.0, 20.0)?;
            let ms = row_of_pins(tx, page, m, &["1"], 40.0, 42.0)?;
            let strip = strip_by_tag(tx, "X1")?;
            let t = tx.add_terminal(strip, "1")?;
            let s = tx.place_symbol(
                page,
                default_symbol(TerminalKind::Feedthrough),
                Placement::at(Point::new(40.0, 30.0)),
                terminal_pins(8.0),
                SymbolRole::Terminal { terminals: vec![t] },
            )?;
            tx.connect(Endpoint::pin(ks, 0), Endpoint::pin(s, 0))?;
            // Only 4 mm below the terminal: a fuse terminal needs 8 more.
            tx.connect(Endpoint::pin(s, 1), Endpoint::pin(ms, 0))?;
            Ok(t)
        })
        .unwrap();
    let before = doc.project().clone();
    assert!(
        doc.transact("Kind", |tx| set_kind(tx, terminal, TerminalKind::Fuse))
            .is_err()
    );
    assert_eq!(snapshot(doc.project()), snapshot(&before));
}

/// Review item 2: find and replace refuses empty names and names that are
/// taken, and the preview shows why before anything is applied.
#[test]
fn rename_refuses_empty_and_taken_names() {
    let mut doc = Document::default();
    let ((strip, ids), _) = doc
        .transact("Spares", |tx| {
            let strip = strip_by_tag(tx, "X1")?;
            let ids = add_spares(tx, strip, 3)?;
            tx.update::<novo_core::Terminal>(ids[2], |t| t.name = "21".into())?;
            Ok((strip, ids))
        })
        .unwrap();
    // "1" -> "" empties terminal 1 and turns 21 into 2, which is taken.
    let preview = rename_preview(doc.project(), strip, "1", "");
    let problems: Vec<_> = preview
        .iter()
        .map(|c| (c.old.as_str(), c.problem))
        .collect();
    assert_eq!(
        problems,
        [
            ("1", Some(RenameProblem::Empty)),
            ("21", Some(RenameProblem::Taken(ids[1]))),
        ]
    );
    assert_eq!(
        strings::rename_problem("2", RenameProblem::Taken(ids[1]), "-X1:2"),
        "2 is already used by -X1:2."
    );
    let before = doc.project().clone();
    let err = doc
        .transact("Rename", |tx| rename(tx, strip, "1", ""))
        .unwrap_err();
    assert!(
        matches!(err, ModelError::InvalidValue { field: "name", .. }),
        "{err:?}"
    );
    assert_eq!(snapshot(doc.project()), snapshot(&before));
    let err = doc
        .transact("Rename", |tx| rename(tx, strip, "21", "2"))
        .unwrap_err();
    assert_eq!(err, ModelError::AlreadyExists(ids[1].into()));
    // Two renamed terminals may not end up with the same name either.
    let ((x2, other), _) = doc
        .transact("Strip", |tx| {
            let x2 = strip_by_tag(tx, "X2")?;
            let ids = add_spares(tx, x2, 2)?;
            tx.update::<novo_core::Terminal>(ids[0], |t| t.name = "aa1".into())?;
            tx.update::<novo_core::Terminal>(ids[1], |t| t.name = "a1".into())?;
            Ok((x2, ids))
        })
        .unwrap();
    let preview = rename_preview(doc.project(), x2, "a", "");
    assert_eq!(preview[0].problem, Some(RenameProblem::Taken(other[1])));
    assert_eq!(preview[1].problem, Some(RenameProblem::Taken(other[0])));
    let before = doc.project().clone();
    assert!(
        doc.transact("Rename", |tx| rename(tx, x2, "a", ""))
            .is_err()
    );
    assert_eq!(snapshot(doc.project()), snapshot(&before));
    // A clean rename still works.
    doc.transact("Rename", |tx| rename(tx, strip, "21", "5"))
        .unwrap();
    assert_eq!(doc.project().terminal(ids[2]).unwrap().name, "5");
}

/// Review item 2: `set_level` refuses level 0, a name already on that
/// level, and more levels than a multi-level drawing has.
#[test]
fn set_level_refuses_duplicates_and_missing_levels() {
    let mut doc = Document::default();
    let ((a, b), _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let strip = strip_by_tag(tx, "X1")?;
            let a = tx.add_terminal(strip, "1")?;
            let b = tx.add_terminal(strip, "1")?;
            tx.update::<novo_core::Terminal>(b, |t| t.level = 2)?;
            let s = tx.place_symbol(
                page,
                "iec/terminals/terminal_2level",
                Placement::at(Point::new(40.0, 30.0)),
                vec![
                    Pin::new("1", 0.0, 0.0),
                    Pin::new("2", 0.0, 8.0),
                    Pin::new("3", 8.0, 0.0),
                    Pin::new("4", 8.0, 8.0),
                ],
                SymbolRole::Terminal { terminals: vec![a] },
            )?;
            tx.update::<novo_core::SymbolInstance>(s, |s| {
                s.role = SymbolRole::Terminal {
                    terminals: vec![a, b],
                };
                s.bridges = vec![vec![0, 1], vec![2, 3]];
            })?;
            Ok((a, b))
        })
        .unwrap();
    let before = doc.project().clone();
    let err = doc.transact("Level", |tx| set_level(tx, a, 0)).unwrap_err();
    assert!(matches!(
        err,
        ModelError::InvalidValue { field: "level", .. }
    ));
    let err = doc.transact("Level", |tx| set_level(tx, a, 2)).unwrap_err();
    assert_eq!(err, ModelError::AlreadyExists(b.into()));
    let err = doc.transact("Level", |tx| set_level(tx, b, 3)).unwrap_err();
    assert!(matches!(
        err,
        ModelError::InvalidValue { field: "level", .. }
    ));
    assert_eq!(snapshot(doc.project()), snapshot(&before));
}

/// Review item 3: the rule lives in novo-core's typed rule; projects saved
/// with the former property keys are migrated onto it.
#[test]
fn legacy_rule_properties_are_migrated_to_the_core_rule() {
    let mut doc = Document::new(Project::default());
    doc.transact("Old file", |tx| {
        tx.set_info(|i| {
            i.properties
                .insert(LEGACY_SAME_NAME_KEY.into(), "refuse".into());
            i.properties
                .insert(LEGACY_JOIN_LINK_KEY.into(), "jumper".into());
        })
    })
    .unwrap();
    let (changed, _) = doc.transact("Migrate", migrate_legacy_rules).unwrap();
    assert!(changed);
    assert_eq!(
        rules(doc.project()),
        TerminalRules {
            same_name: SameName::Refuse,
            join_link: JoinLink::Jumper,
        }
    );
    assert!(doc.project().info().properties.is_empty());
    let (changed, _) = doc.transact("Migrate", migrate_legacy_rules).unwrap();
    assert!(!changed);
    // One undo step brings the old properties back.
    doc.undo().unwrap();
    assert_eq!(rules(doc.project()).same_name, SameName::Off);
}

#[test]
fn a_pen_wire_gets_a_pe_terminal_named_pen() {
    let mut doc = Document::default();
    let (wire, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let pen = tx.add_potential("PEN", PotentialKind::Pen)?;
            let m = tx.add_device(tag("-M1"), "")?;
            let f = feed(tx, page, pen, 40.0, 20.0)?;
            let ms = row_of_pins(tx, page, m, &["PEN"], 40.0, 100.0)?;
            tx.connect(Endpoint::pin(f, 0), Endpoint::pin(ms, 0))
        })
        .unwrap();
    let (placed, _) = doc
        .transact("T", |tx| {
            let strip = strip_by_tag(tx, "X1")?;
            place_terminals(tx, strip, &[drop_at(wire)])
        })
        .unwrap();
    let t = doc.project().terminal(placed[0].terminal).unwrap();
    assert_eq!(t.name, "PEN");
    assert_eq!(t.kind, TerminalKind::ProtectiveEarth);
    assert_eq!(
        doc.project().symbol(placed[0].symbol).unwrap().symbol.0,
        "iec/terminals/terminal_pe"
    );
    assert!(check(doc.project(), &doc.project().netlist()).is_empty());
    // A PEN conductor on a feed-through terminal is flagged.
    doc.transact("Kind", |tx| {
        set_kind(tx, placed[0].terminal, TerminalKind::Feedthrough)
    })
    .unwrap();
    let issues = check(doc.project(), &doc.project().netlist());
    assert!(issues.iter().any(|i| matches!(
        &i.kind,
        IssueKind::PotentialDiffersFromKind { potential } if potential == "PEN"
    )));
}

#[test]
fn jumpers_stay_on_one_strip_and_strips_sort_by_number() {
    let mut doc = Document::default();
    let ((x2, x10), _) = doc
        .transact("Strips", |tx| {
            let x10 = strip_by_tag(tx, "X10")?;
            let x2 = strip_by_tag(tx, "X2")?;
            Ok((x2, x10))
        })
        .unwrap();
    assert_eq!(strips(doc.project()), [x2, x10]);
    let (ids, _) = doc
        .transact("Spares", |tx| {
            let mut ids = add_spares(tx, x2, 1)?;
            ids.extend(add_spares(tx, x10, 1)?);
            Ok(ids)
        })
        .unwrap();
    assert!(
        doc.transact("Jumper", |tx| set_jumper(tx, &ids, Some(1)))
            .is_err()
    );
    // Removing jumpers needs no common strip.
    doc.transact("Jumper", |tx| set_jumper(tx, &ids, None))
        .unwrap();
}

/// Sets the Refuse rule and returns strip `-X1`.
fn refuse_strip(tx: &mut Tx<'_>) -> Result<DeviceId> {
    set_rules(
        tx,
        TerminalRules {
            same_name: SameName::Refuse,
            join_link: JoinLink::Jumper,
        },
    )?;
    strip_by_tag(tx, "X1")
}

fn names_of(project: &Project, strip: DeviceId) -> Vec<String> {
    strip_view(project, strip)
        .rows
        .iter()
        .map(|r| r.name.clone())
        .collect()
}

/// Review of PR #30: under Refuse, novo-core checks each terminal update on
/// its own. Renumbering `[1, 3, 2, 4]` must not stop at `3 -> 2` while the
/// record named `2` still exists.
#[test]
fn renumber_after_a_gap_works_under_refuse() {
    let mut doc = Document::default();
    let (motor, _) = doc.transact("Draw", motor_circuit).unwrap();
    let (strip, _) = doc
        .transact("Terminals", |tx| {
            let strip = refuse_strip(tx)?;
            let d = drops(&motor.wires[..3], 60.0, tx.project());
            place_terminals(tx, strip, &d)?;
            Ok(strip)
        })
        .unwrap();
    let two = doc
        .project()
        .terminals_of_strip(strip)
        .iter()
        .find(|t| t.name == "2")
        .unwrap()
        .id;
    doc.transact("Remove", |tx| tx.remove(two)).unwrap();
    doc.transact("Spares", |tx| add_spares(tx, strip, 2))
        .unwrap();
    assert_eq!(names_of(doc.project(), strip), ["1", "3", "2", "4"]);
    let depth = doc.undo_depth();
    doc.transact("Renumber", |tx| renumber(tx, strip)).unwrap();
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(names_of(doc.project(), strip), ["1", "2", "3", "4"]);
    assert_eq!(
        strip_view(doc.project(), strip).rows[1].external_targets()[0].designation,
        "-M1:W1"
    );
    // One undo step restores every name.
    doc.undo().unwrap();
    assert_eq!(names_of(doc.project(), strip), ["1", "3", "2", "4"]);
}

/// Swapping two numbers is a cycle: no order of single updates avoids an
/// in-between duplicate.
#[test]
fn renumber_swaps_two_terminals_under_refuse() {
    let mut doc = Document::default();
    let ((strip, ids), _) = doc
        .transact("Spares", |tx| {
            let strip = refuse_strip(tx)?;
            let ids = add_spares(tx, strip, 3)?;
            Ok((strip, ids))
        })
        .unwrap();
    doc.transact("Reorder", |tx| reorder(tx, strip, &[ids[1], ids[0]]))
        .unwrap();
    assert_eq!(names_of(doc.project(), strip), ["2", "1", "3"]);
    doc.transact("Renumber", |tx| renumber(tx, strip)).unwrap();
    assert_eq!(names_of(doc.project(), strip), ["1", "2", "3"]);
    let name = |id| doc.project().terminal(id).unwrap().name.clone();
    assert_eq!([name(ids[0]), name(ids[1]), name(ids[2])], ["2", "1", "3"]);
    doc.undo().unwrap();
    assert_eq!(names_of(doc.project(), strip), ["2", "1", "3"]);
}

/// A rename whose new name is another renamed record's old name (`X -> XX`
/// while `XX -> XXXX`) is fine at the end but clashes in between.
#[test]
fn rename_into_a_name_being_renamed_works_under_refuse() {
    let mut doc = Document::default();
    let (strip, _) = doc
        .transact("Spares", |tx| {
            let strip = refuse_strip(tx)?;
            let ids = add_spares(tx, strip, 3)?;
            for (id, name) in ids.into_iter().zip(["X", "XX", "XXX"]) {
                tx.update::<novo_core::Terminal>(id, |t| t.name = name.into())?;
            }
            Ok(strip)
        })
        .unwrap();
    let preview = rename_preview(doc.project(), strip, "X", "XX");
    assert!(preview.iter().all(|c| c.problem.is_none()));
    let depth = doc.undo_depth();
    let (n, _) = doc
        .transact("Rename", |tx| rename(tx, strip, "X", "XX"))
        .unwrap();
    assert_eq!(n, 3);
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(names_of(doc.project(), strip), ["XX", "XXXX", "XXXXXX"]);
    doc.undo().unwrap();
    assert_eq!(names_of(doc.project(), strip), ["X", "XX", "XXX"]);
}
