//! The saved JSON format, pinned by a fixture file.
//!
//! `fixtures/format_v1.json` is a project that uses every record type and
//! every optional field. Two promises are checked against it:
//! - files written by earlier builds keep loading (`fixture_loads`), and
//! - the format does not change by accident (`format_is_stable`).
//!
//! `fixtures/records_v1.jsonl` pins the per-record form that storage
//! (WS-04) writes into its rows and history: one [`Record`] per line, then
//! one [`Op`] of each kind.
//!
//! A deliberate format change bumps `JSON_FORMAT_VERSION`, keeps loading the
//! old file, and adds a new fixture. To rewrite the current fixture after an
//! additive change, run the tests with `NOVO_UPDATE_FIXTURES=1`.

use novo_core::*;

const FIXTURE: &str = include_str!("fixtures/format_v1.json");
const FIXTURE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/format_v1.json");
const RECORDS: &str = include_str!("fixtures/records_v1.jsonl");
const RECORDS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/records_v1.jsonl"
);

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn pin(name: &str, x: f64, y: f64, direction: Direction) -> Pin {
    Pin::new(name, x, y).with_direction(direction)
}

/// Builds the fixture project through the public edit API.
fn build() -> Project {
    let mut doc = Document::default();
    doc.transact("Fixture", |tx| {
        tx.set_info(|info| {
            info.name = "Format fixture".into();
            info.description = "Every record type and optional field".into();
            info.properties.insert("customer".into(), "ACME".into());
            info.properties.insert("revision".into(), Value::Int(3));
            info.rules.terminals.same_name = SameName::Join;
        })?;
        let power = tx.add_page("1")?;
        let control = tx.add_page("2")?;
        tx.update::<Page>(control, |p| {
            p.title = "Control".into();
            p.kind = PageKind::Generated {
                generator: "terminal_diagram".into(),
            };
            p.format = PageFormat {
                size: PaperSize::A4,
                ..p.format
            };
            p.structure = tag("=A2+CAB1");
            p.properties.insert("approved".into(), Value::Bool(true));
        })?;

        let part = tx.add_part("Siemens", "3RT2015-1BB41")?;
        tx.update::<Part>(part, |p| {
            p.type_number = "3RT2015".into();
            p.description = "Contactor 3 kW 24 V DC".into();
            p.properties.insert("width_mm".into(), Value::Float(45.0));
        })?;
        let k1 =
            tx.add_device_of_kind(tag("=A1+CAB1-K1"), "Main contactor", DeviceKind::General)?;
        tx.assign_part(k1, part)?;
        tx.set_property(k1, "note", Some("spare contact".into()))?;
        let x1 = tx.add_terminal_strip(tag("+CAB1-X1"))?;
        let upper = tx.add_terminal(x1, "1")?;
        let lower = tx.add_terminal(x1, "2")?;
        tx.update::<Terminal>(lower, |t| {
            t.kind = TerminalKind::ProtectiveEarth;
            t.level = 2;
            t.order = 5;
            t.jumper = Some(1);
        })?;
        let clamp = tx.add_part("Phoenix Contact", "3031212")?;
        tx.update::<Terminal>(upper, |t| {
            t.jumper = Some(1);
            t.part = Some(clamp);
        })?;

        let l1 = tx.add_potential("L1", PotentialKind::Line)?;
        tx.update::<Potential>(l1, |p| p.voltage = Some(400.0))?;
        let feed = tx.place_symbol(
            power,
            "iec/connectivity/potential_feed",
            Placement::at(Point::new(10.0, 10.0)),
            vec![pin("1", 0.0, 0.0, Direction::Down)],
            SymbolRole::PotentialFeed { potential: l1 },
        )?;
        let contact = tx.place_symbol(
            power,
            "iec/contacts/changeover",
            Placement {
                origin: Point::new(10.0, 40.0),
                rotation: Rotation::R90,
                mirrored: true,
            },
            vec![
                pin("11", 0.0, 0.0, Direction::Up),
                pin("12", -5.0, 10.0, Direction::Down),
                Pin::new("14", 5.0, 10.0),
            ],
            SymbolRole::Device {
                device: k1,
                main: false,
            },
        )?;
        tx.update::<SymbolInstance>(contact, |s| {
            s.pass_through = vec![vec![0, 1], vec![0, 2]];
            s.properties
                .insert("label_offset".into(), Value::Float(2.5));
        })?;
        let block = tx.execute(Command::PlaceSymbol {
            page: control,
            symbol: "iec/terminals/terminal_2level".into(),
            placement: Placement::at(Point::new(50.0, 50.0)),
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
            pass_through: Vec::new(),
        })?;
        let [AnyId::Symbol(block)] = block[..] else {
            unreachable!()
        };
        let off_page = |tx: &mut Tx<'_>, page| {
            tx.place_symbol(
                page,
                "iec/connectivity/interruption",
                Placement::at(Point::new(100.0, 10.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Interruption { name: "A".into() },
            )
        };
        let out = off_page(tx, power)?;
        let back = off_page(tx, control)?;
        tx.place_symbol(
            control,
            "iec/general/note",
            Placement::at(Point::new(0.0, 0.0)),
            Vec::new(),
            SymbolRole::Unassigned,
        )?;

        let junction = tx.add_junction(power, Point::new(10.0, 25.0))?;
        let w1 = tx.connect(Endpoint::pin(feed, 0), Endpoint::junction(junction))?;
        tx.update::<Wire>(w1, |w| {
            w.route = vec![Point::new(10.0, 20.0)];
            w.number = Some("101".into());
            w.color = Some("BK".into());
            w.cross_section_mm2 = Some(2.5);
            w.potential = Some(l1);
            w.properties.insert("twisted".into(), Value::Bool(false));
        })?;
        tx.connect(Endpoint::junction(junction), Endpoint::pin(contact, 0))?;
        tx.connect(Endpoint::pin(contact, 2), Endpoint::pin(out, 0))?;
        let w4 = tx.connect(Endpoint::pin(back, 0), Endpoint::pin(block, 0))?;

        let mut shield = CableCore::new("SH");
        shield.shield = true;
        let mut core = CableCore::new("1");
        core.cross_section_mm2 = Some(1.5);
        let cable = tx.add_cable(tag("+CAB1-W1"), vec![core, shield])?;
        tx.update::<Cable>(cable, |c| {
            c.description = "Control cable".into();
            c.part = Some(part);
            c.length_m = Some(12.5);
            c.properties.insert("route".into(), "tray 3".into());
        })?;
        tx.assign_core(cable, 0, Some(w4))?;
        let label = tx.add_text(
            power,
            TextContent::DeviceTag,
            TextPlacement {
                offset: Point::new(6.0, -2.0),
                rotation: Rotation::R90,
                anchor: TextAnchor::MiddleLeft,
            },
            Some(contact.into()),
        )?;
        tx.update::<Text>(label, |t| t.height_mm = 3.5)?;
        tx.add_text(
            control,
            TextContent::Literal {
                text: "Control circuit".into(),
            },
            TextPlacement::default(),
            None,
        )?;
        tx.add_potential_system("24 V DC", PotentialSet::Dc { voltage: 24.0 }, None, "")?;
        Ok(())
    })
    .unwrap();
    doc.project().clone()
}

#[test]
fn format_is_stable() {
    let json = build().to_json();
    if std::env::var_os("NOVO_UPDATE_FIXTURES").is_some() {
        std::fs::write(FIXTURE_PATH, &json).unwrap();
        return;
    }
    // Compare as values, so line endings from a Windows checkout don't matter.
    let expected: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let actual: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        expected == actual,
        "the saved JSON format changed; see the module docs of tests/format.rs.\n{json}"
    );
}

#[test]
fn fixture_loads() {
    let project = Project::from_json(FIXTURE).unwrap();
    assert_eq!(project.info().name, "Format fixture");
    assert_eq!(project.pages().count(), 2);
    assert_eq!(project.symbols().count(), 6);
    assert_eq!(project.wires().count(), 4);

    let net = project.netlist();
    let l1 = project.potentials().next().unwrap().id;
    let k1 = project.devices_by_tag(&tag("=A1+CAB1-K1"))[0];
    let contact = project.symbols_of_device(k1)[0];
    // The feed reaches the contact's common by wire and both sides through
    // its poles; the make side continues to page 2 through the interruption
    // point, ending at the terminal block.
    for pin in 0..3 {
        assert_eq!(net.potential_of(Endpoint::pin(contact.id, pin)), Some(l1));
    }
    let block = project
        .symbols()
        .find(|s| matches!(s.role, SymbolRole::Terminal { .. }))
        .unwrap();
    assert_eq!(net.potential_of(Endpoint::pin(block.id, 1)), Some(l1));
    // Level 1 is jumpered to level 2, which joins the nets.
    assert!(net.connected(Endpoint::pin(block.id, 0), Endpoint::pin(block.id, 3)));
    assert_eq!(net.potential_conflicts().count(), 0);
    let wire = project.wires().find(|w| w.number.is_some()).unwrap();
    assert_eq!(project.cable_of_wire(wire.id), None);
    assert_eq!(
        project
            .pin_designation(PinRef::new(contact.id, 0))
            .as_deref(),
        Some("=A1+CAB1-K1:11")
    );
}

/// The records of the fixture project, one per line, then one op of each
/// kind built from them.
fn record_lines(project: &Project) -> String {
    let records: Vec<Record> = project.records().collect();
    let page = records
        .iter()
        .find(|r| matches!(r, Record::Page(_)))
        .unwrap()
        .clone();
    let mut renamed = page.clone();
    if let Record::Page(p) = &mut renamed {
        p.name = "1a".into();
    }
    let ops = [
        Op::Insert {
            record: page.clone(),
        },
        Op::Remove {
            record: page.clone(),
        },
        Op::Update {
            before: page,
            after: renamed,
        },
        Op::SetInfo {
            before: ProjectInfo::default(),
            after: project.info().clone(),
        },
    ];
    let mut lines = String::new();
    for record in &records {
        lines += &serde_json::to_string(record).unwrap();
        lines.push('\n');
    }
    for op in &ops {
        lines += &serde_json::to_string(op).unwrap();
        lines.push('\n');
    }
    lines
}

fn parse_lines(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn records_format_is_stable() {
    let lines = record_lines(&build());
    if std::env::var_os("NOVO_UPDATE_FIXTURES").is_some() {
        std::fs::write(RECORDS_PATH, &lines).unwrap();
        return;
    }
    assert!(
        parse_lines(RECORDS) == parse_lines(&lines),
        "the per-record JSON format changed; see the module docs of tests/format.rs.\n{lines}"
    );
}

#[test]
fn records_fixture_loads() {
    let project = Project::from_json(FIXTURE).unwrap();
    let mut records = Vec::new();
    let mut ops = Vec::new();
    for line in RECORDS.lines().filter(|l| !l.trim().is_empty()) {
        if line.contains("\"record\":") && !line.contains("\"op\":") {
            records.push(serde_json::from_str::<Record>(line).unwrap());
        } else {
            ops.push(serde_json::from_str::<Op>(line).unwrap());
        }
    }
    assert_eq!(ops.len(), 4);
    let rebuilt = Project::from_records(project.info().clone(), 0, records).unwrap();
    assert_eq!(rebuilt.to_json(), project.to_json());
}
