//! Placed starter macros come out clean: protective earth is wired to every
//! motor, the only open points are the ones the user connects to a supply,
//! and no two wires lie on each other.

use novo_checks::ProblemKind;
use novo_core::{BlockDefinitionId, Document, Endpoint, Point, Project, Record, SymbolRole};
use novo_parts::{MacroFile, starter_macros};

fn placed(file: &MacroFile) -> Document {
    let mut doc = Document::new(Project::new(""));
    doc.transact("place", |tx| {
        let page = tx.add_page("1")?;
        let mut definition = file.definition.clone();
        definition.id = BlockDefinitionId(9_000_000);
        let def = definition.id;
        tx.insert_record(Record::BlockDefinition(definition))?;
        tx.place_block(def, page, Point::new(20.0, 20.0), vec![])?;
        Ok(())
    })
    .unwrap_or_else(|e| panic!("{}: {e}", file.name));
    doc
}

fn open_points(doc: &Document) -> Vec<String> {
    let mut open: Vec<String> = novo_checks::check(doc.project())
        .into_iter()
        .filter(|p| p.kind == ProblemKind::UnconnectedPin)
        .map(|p| {
            p.message
                .replace(" not connected", "")
                .replace("iec/terminals/", "")
        })
        .collect();
    open.sort();
    open
}

/// The points the user connects to a supply, per starter: the line
/// phases, the control supply, the PE entry and the N rail. Everything else
/// is wired inside the macro.
const ENTRIES: &[(&str, &[&str])] = &[
    (
        "Motor starter DOL",
        &["-F1: 1, 3, 5", "-F3: 1", "-K1: A2", "terminal_pe: 1"],
    ),
    (
        "Star-delta starter",
        &["-F1: 1, 3, 5", "-F3: 1", "terminal_pe: 1"],
    ),
    ("E-stop chain", &["-K1: 13, 14", "-K1: A2", "-S1: 11"]),
    (
        "Power supply 24 V",
        &["-F1: 1, 3", "-F2: 2", "-G1: M", "terminal_pe: 1"],
    ),
    (
        "Motor starter reversing",
        &["-F1: 1, 3, 5", "-F3: 1", "terminal_pe: 1"],
    ),
    ("Start/stop latch", &["-F1: 1"]),
    ("PLC input card", &["-A1: M"]),
    (
        "Frequency converter drive",
        &["-F1: 1, 3, 5", "terminal_pe: 1"],
    ),
    (
        "Soft starter with bypass",
        &["-F1: 1, 3, 5", "-K1: A1, A2", "terminal_pe: 1"],
    ),
    (
        "Single-phase supply with 24 V",
        &[
            "-F1: 1, N",
            "-F2: 2",
            "-G1: M",
            "terminal_pe: 1",
            "terminal_pe: 1",
        ],
    ),
    (
        "Motor with brake",
        &["-F1: 1, 3, 5", "-F3: 1", "terminal_pe: 1"],
    ),
    (
        "Contactor with indication",
        &["-F1: 1", "-K1: 1, 2, 3, 4, 5, 6"],
    ),
    (
        "Crane hoist up/down",
        &["-F1: 1, 3, 5", "-F3: 1", "terminal_pe: 1"],
    ),
    (
        "Pump with float switch",
        &["-F1: 1", "-K1: A2", "-Q1: 1, 3, 5", "terminal_pe: 1"],
    ),
    (
        "Heater with thermostat",
        &["-F1: 1, 3, 5", "-F2: 1", "-K1: A2"],
    ),
    (
        "Emergency stop with safety relay",
        &[
            "-K1: A1, A2, 13, 23, 33, 34, 41, 42",
            "-K2: A2",
            "-K3: A2",
            "-S2: 13",
        ],
    ),
    (
        "PLC network with switch",
        &[
            "-A1: L+, M, PE, P2, A, B, SH",
            "-A2: P2, P3, L+, M",
            "-T1: P2",
        ],
    ),
    (
        "Three-phase supply",
        &[
            "-F1: 2, 4, 6",
            "-F2: 2",
            "-Q1: 1, 3, 5",
            "terminal_n: 1, 2",
            "terminal_pe: 1, 2",
        ],
    ),
];

#[test]
fn only_the_supply_entries_are_left_open() {
    let all = starter_macros();
    assert_eq!(all.len(), ENTRIES.len(), "a starter has no entry list");
    for (_, file) in all {
        let (_, want) = ENTRIES
            .iter()
            .find(|(name, _)| *name == file.name)
            .unwrap_or_else(|| panic!("{} has no entry list", file.name));
        assert_eq!(open_points(&placed(&file)), *want, "{}", file.name);
    }
}

#[test]
fn no_starter_has_wires_on_each_other_or_a_contact_drawn_twice() {
    for (_, file) in starter_macros() {
        let doc = placed(&file);
        for p in novo_checks::check(doc.project()) {
            assert!(
                !matches!(
                    p.kind,
                    ProblemKind::WireOverlap
                        | ProblemKind::ZeroLengthWire
                        | ProblemKind::ContactDrawnTwice
                        | ProblemKind::OpenWire
                        | ProblemKind::DanglingJunction
                ),
                "{}: {:?}: {}",
                file.name,
                p.kind,
                p.message
            );
        }
    }
}

/// The connection point of a device's symbol, by tag, library key and name.
fn pin(doc: &Document, tag: &str, key: &str, name: &str) -> Endpoint {
    let p = doc.project();
    for s in p.symbols() {
        let SymbolRole::Device { device, .. } = s.role else {
            continue;
        };
        if p.device(device).is_some_and(|d| d.tag.to_string() == tag)
            && s.symbol.0.as_str() == key
            && let Some(i) = s.pins.iter().position(|x| x.name == name)
        {
            return Endpoint::pin(s.id, i as u16);
        }
    }
    panic!("no {tag} {key} pin {name}");
}

fn same_net(doc: &Document, a: Endpoint, b: Endpoint) -> bool {
    let nets = doc.project().netlist();
    let (a, b) = (nets.net_of(a), nets.net_of(b));
    a.is_some() && a == b
}

fn starter(name: &str) -> MacroFile {
    starter_macros()
        .into_iter()
        .map(|(_, f)| f)
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no starter {name}"))
}

#[test]
fn every_motor_and_drive_pe_is_on_the_pe_terminal() {
    let mut seen = 0;
    for (_, file) in starter_macros() {
        let doc = placed(&file);
        let p = doc.project();
        for s in p.symbols() {
            let key = s.symbol.0.as_str();
            if !(key.starts_with("iec/motors/") || key == "iec/drives/frequency_converter") {
                continue;
            }
            let i = s
                .pins
                .iter()
                .position(|x| x.name == "PE")
                .expect("a PE pin");
            let terminal = p
                .symbols()
                .find(|t| t.symbol.0.as_str() == "iec/terminals/terminal_pe")
                .expect("a PE terminal");
            assert!(
                same_net(
                    &doc,
                    Endpoint::pin(s.id, i as u16),
                    Endpoint::pin(terminal.id, 0)
                ),
                "{}: the PE of {key} is not on the PE terminal",
                file.name
            );
            seen += 1;
        }
    }
    assert!(seen >= 9, "{seen} motors and drives checked");
}

#[test]
fn star_delta_connects_delta_and_star_the_right_way() {
    let doc = placed(&starter("Star-delta starter"));
    let m = |n: &str| pin(&doc, "-M1", "iec/motors/motor_3ph_6t", n);
    let k = |tag: &str, n: &str| pin(&doc, tag, "iec/contactors/main_3p", n);
    // Line side: U1 V1 W1 come from the overload relay and feed the delta
    // contactor's top.
    for (winding, pole) in [("U1", "1"), ("V1", "3"), ("W1", "5")] {
        assert!(same_net(&doc, m(winding), k("-K2", pole)), "{winding}");
    }
    // Delta: U1-W2, V1-U2, W1-V2.
    for (pole, winding) in [("2", "W2"), ("4", "U2"), ("6", "V2")] {
        assert!(same_net(&doc, k("-K2", pole), m(winding)), "{winding}");
    }
    // Star: the star contactor's top ends are U2 V2 W2, its bottom ends meet.
    for (pole, winding) in [("1", "U2"), ("3", "V2"), ("5", "W2")] {
        assert!(same_net(&doc, k("-K3", pole), m(winding)), "{winding}");
    }
    assert!(same_net(&doc, k("-K3", "2"), k("-K3", "4")));
    assert!(same_net(&doc, k("-K3", "4"), k("-K3", "6")));
    assert!(!same_net(&doc, m("U1"), m("U2")));
    assert!(!same_net(&doc, k("-K3", "2"), m("U2")));
}

#[test]
fn reversing_starter_swaps_l1_and_l3_through_the_reverse_contactor() {
    let doc = placed(&starter("Motor starter reversing"));
    let k = |tag: &str, n: &str| pin(&doc, tag, "iec/contactors/main_3p", n);
    let f1 = |n: &str| pin(&doc, "-F1", "iec/protection/fuse_3p", n);
    assert!(same_net(&doc, f1("2"), k("-K1", "1")));
    assert!(same_net(&doc, f1("2"), k("-K2", "5")));
    assert!(same_net(&doc, f1("4"), k("-K2", "3")));
    assert!(same_net(&doc, f1("6"), k("-K2", "1")));
    assert!(!same_net(&doc, f1("2"), f1("6")));
}
