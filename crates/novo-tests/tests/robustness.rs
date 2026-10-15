//! Bad input never makes a project unsaveable or unopenable (finding F1 in
//! docs/TEST_REPORT.md), and library terminals with several levels place
//! and connect as separate nets.

use novo_core::*;
use novo_symbols::Library;
use novo_tests::motor_starter;

/// The project as JSON without `next_id`: a refused edit may still use up
/// ids, which are never reused.
fn state(project: &Project) -> String {
    project
        .to_json()
        .lines()
        .filter(|l| !l.trim_start().starts_with("\"next_id\""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every way a non-finite number can enter the model is refused, the edit
/// leaves nothing behind, and the project still round-trips.
#[test]
fn non_finite_numbers_are_refused_everywhere() {
    let mut doc = Document::default();
    let s = motor_starter(&mut doc).unwrap();
    let before = state(doc.project());
    let depth = doc.undo_depth();

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let attempts: Vec<(&str, Result<Outcome>)> = vec![
            (
                "symbol placement",
                doc.execute(Command::MoveSymbol {
                    symbol: s.k1_coil,
                    placement: Placement::at(Point::new(bad, 0.0)),
                }),
            ),
            (
                "pin offset",
                doc.transact("pin", |tx| {
                    tx.place_symbol(
                        s.main_page,
                        "iec/contacts/no",
                        Placement::at(Point::new(200.0, 200.0)),
                        vec![Pin::new("13", 0.0, bad)],
                        SymbolRole::Unassigned,
                    )
                })
                .map(|_| Outcome::default()),
            ),
            (
                "junction",
                doc.execute(Command::AddJunction {
                    page: s.main_page,
                    position: Point::new(0.0, bad),
                }),
            ),
            (
                "wire route",
                doc.execute(Command::Connect {
                    from: Endpoint::pin(s.f1_sym, 0),
                    to: Endpoint::pin(s.f1_sym, 2),
                    route: vec![Point::new(bad, 10.0)],
                    page: None,
                }),
            ),
            (
                "potential voltage",
                doc.execute(Command::AddPotential {
                    name: "24V".into(),
                    kind: PotentialKind::DcPositive,
                    voltage: Some(bad),
                }),
            ),
            (
                "float property",
                doc.execute(Command::SetProperty {
                    target: s.m1.into(),
                    key: "power_kw".into(),
                    value: Some(Value::Float(bad)),
                }),
            ),
            (
                "cable length",
                doc.transact("len", |tx| {
                    tx.update::<Cable>(s.w1, |c| c.length_m = Some(bad))
                })
                .map(|_| Outcome::default()),
            ),
            (
                "core cross-section",
                doc.transact("mm2", |tx| {
                    tx.update::<Cable>(s.w1, |c| c.cores[0].cross_section_mm2 = Some(bad))
                })
                .map(|_| Outcome::default()),
            ),
        ];
        for (what, result) in attempts {
            assert!(result.is_err(), "{what} = {bad} was accepted");
        }
    }

    assert_eq!(state(doc.project()), before, "refused edits left traces");
    assert_eq!(doc.undo_depth(), depth, "refused edits reached the history");
    let json = doc.project().to_json();
    assert_eq!(Project::from_json(&json).unwrap().to_json(), json);
    // Undo still works after the refusals.
    doc.undo().unwrap();
    assert_eq!(doc.project().records().count(), 0);
}

/// A project file edited by hand to hold an out-of-range number is refused
/// on load with an error, not accepted half-way.
#[test]
fn json_with_an_overflowing_number_is_refused() {
    let mut doc = Document::default();
    let s = motor_starter(&mut doc).unwrap();
    doc.execute(Command::MoveSymbol {
        symbol: s.k1_coil,
        placement: Placement::at(Point::new(12345.5, 168.0)),
    })
    .unwrap();
    let json = doc.project().to_json();
    assert!(json.contains("12345.5"));
    let broken = json.replace("12345.5", "1e999");
    assert!(Project::from_json(&broken).is_err());
}

fn pins_from_library(symbol: &novo_symbols::Symbol) -> Vec<Pin> {
    symbol
        .connections
        .iter()
        .map(|c| {
            let direction = match c.dir {
                novo_symbols::Direction::Up => Direction::Up,
                novo_symbols::Direction::Down => Direction::Down,
                novo_symbols::Direction::Left => Direction::Left,
                novo_symbols::Direction::Right => Direction::Right,
            };
            Pin::new(c.id.clone(), c.at[0], c.at[1]).with_direction(direction)
        })
        .collect()
}

fn bridges_from_library(symbol: &novo_symbols::Symbol) -> Vec<Vec<u16>> {
    symbol
        .bridges()
        .into_iter()
        .map(|g| g.into_iter().map(|i| u16::try_from(i).unwrap()).collect())
        .collect()
}

/// Every multi-level terminal in the library places with one terminal per
/// bridge group, and its levels stay separate nets until jumpered.
#[test]
fn library_multi_level_terminals_keep_levels_apart() {
    let library = Library::builtin().unwrap();
    let multi: Vec<_> = library
        .iter()
        .filter(|s| s.category == "terminals" && s.bridges().len() > 1)
        .collect();
    assert!(!multi.is_empty(), "the library has a multi-level terminal");

    for symbol in multi {
        let bridges = bridges_from_library(symbol);
        let levels = bridges.len();
        let mut doc = Document::default();
        let (placed, _) = doc
            .transact("Place", |tx| {
                let page = tx.add_page("1")?;
                let strip = tx.add_terminal_strip(DeviceTag::parse("=A1+CAB-X2")?)?;
                let mut terminals = Vec::new();
                for level in 0..levels {
                    terminals.push(tx.add_terminal(strip, (level + 1).to_string())?);
                }
                // Bridges first, then the levels' terminals, so every step
                // passes the integrity check.
                let placed = tx.place_symbol(
                    page,
                    symbol.id.as_str(),
                    Placement::at(Point::new(40.0, 40.0)),
                    pins_from_library(symbol),
                    SymbolRole::Unassigned,
                )?;
                tx.update::<SymbolInstance>(placed, |s| s.bridges = bridges.clone())?;
                tx.update::<SymbolInstance>(placed, |s| {
                    s.role = SymbolRole::Terminal { terminals }
                })?;
                Ok(placed)
            })
            .unwrap_or_else(|e| panic!("`{}` does not place: {e}", symbol.id));

        let p = doc.project();
        let net = p.netlist();
        let id = &symbol.id;
        for (level, group) in bridges.iter().enumerate() {
            for pair in group.windows(2) {
                assert!(
                    net.connected(
                        Endpoint::pin(placed, pair[0]),
                        Endpoint::pin(placed, pair[1])
                    ),
                    "`{id}` level {level} conducts"
                );
            }
            let designation = p.pin_designation(PinRef::new(placed, group[0])).unwrap();
            assert_eq!(designation, format!("=A1+CAB-X2:{}", level + 1), "`{id}`");
        }
        for (a, b) in bridges.iter().zip(bridges.iter().skip(1)) {
            assert!(
                !net.connected(Endpoint::pin(placed, a[0]), Endpoint::pin(placed, b[0])),
                "`{id}` levels stay apart"
            );
        }
        // Pin directions come through and turn with the symbol.
        let instance = p.symbol(placed).unwrap();
        assert!(instance.pin_direction(0).is_some(), "`{id}` pin direction");
        let json = p.to_json();
        assert_eq!(Project::from_json(&json).unwrap().to_json(), json);

        // More terminals than levels is refused.
        let err = doc.transact("x", |tx| {
            let strip = tx.add_terminal_strip(DeviceTag::parse("=A1+CAB-X9")?)?;
            let extra = tx.add_terminal(strip, "9")?;
            tx.update::<SymbolInstance>(placed, |s| {
                if let SymbolRole::Terminal { terminals } = &mut s.role {
                    terminals.push(extra);
                }
            })
        });
        assert!(
            matches!(err, Err(ModelError::TerminalLevels { .. })),
            "`{id}` accepted more terminals than levels"
        );
        // One terminal for several levels is refused too (since #14): both
        // levels would carry the same designation.
        let err = doc.transact("x", |tx| {
            tx.update::<SymbolInstance>(placed, |s| {
                if let SymbolRole::Terminal { terminals } = &mut s.role {
                    terminals.truncate(1);
                }
            })
        });
        assert!(
            matches!(err, Err(ModelError::TerminalLevels { .. })),
            "`{id}` accepted one terminal for {levels} levels"
        );
    }
}
