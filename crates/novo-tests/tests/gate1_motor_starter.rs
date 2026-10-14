//! Gate 1 scenario on the model: the direct-on-line motor starter over two
//! pages, checked for completeness, connectivity, cross-page devices, parts,
//! save/reopen and undo.

use std::collections::BTreeSet;

use novo_core::*;
use novo_tests::motor_starter::{DEVICE_COUNT, MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let s = motor_starter(&mut doc).expect("the Gate 1 project builds");
    (doc, s)
}

/// The project as JSON without `next_id`, which undo deliberately keeps.
fn state(project: &Project) -> String {
    project
        .to_json()
        .lines()
        .filter(|l| !l.trim_start().starts_with("\"next_id\""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn spans_two_pages_in_order() {
    let (doc, s) = starter();
    let p = doc.project();
    let pages: Vec<_> = p.pages_in_order().iter().map(|pg| pg.id).collect();
    assert_eq!(pages, vec![s.main_page, s.control_page]);
    assert_eq!(p.page(s.main_page).unwrap().title, "Main circuit");
    assert!(!p.page_contents(s.main_page).symbols.is_empty());
    assert!(!p.page_contents(s.control_page).symbols.is_empty());
}

#[test]
fn drawing_is_complete() {
    let (doc, _) = starter();
    let p = doc.project();
    assert_eq!(
        p.unconnected_pins(),
        vec![],
        "every connection point is wired"
    );
    assert_eq!(p.duplicate_tags(), vec![], "no tag is used twice");
    assert_eq!(p.devices().count(), DEVICE_COUNT);
}

#[test]
fn main_circuit_connectivity() {
    let (doc, s) = starter();
    let p = doc.project();
    let net = p.netlist();
    assert_eq!(net.potential_conflicts().count(), 0);

    // Supply reaches the fuse inputs.
    for (pin, potential) in [(0, s.l1), (2, s.l2), (4, s.l3)] {
        assert_eq!(
            net.potential_of(Endpoint::pin(s.f1_sym, pin)),
            Some(potential)
        );
    }
    // Fuses and contactor main contacts are switching elements: they do not
    // join nets, but L1 passes through them by name.
    assert!(!net.connected(Endpoint::pin(s.f1_sym, 0), Endpoint::pin(s.f1_sym, 1)));
    assert!(!net.connected(Endpoint::pin(s.k1_main, 0), Endpoint::pin(s.k1_main, 1)));
    for (pin, potential) in [(0, s.l1), (1, s.l2), (2, s.l3)] {
        assert_eq!(
            net.potential_of(Endpoint::pin(s.m1_sym, pin)),
            Some(potential),
            "motor pin {pin}"
        );
    }

    // Overload heaters (bridges) and terminals conduct, and the cable cores
    // carry the motor wires, so -K1:2 reaches -M1:U1 and so on.
    for phase in 0..3u16 {
        assert!(
            net.connected(
                Endpoint::pin(s.k1_main, phase * 2 + 1),
                Endpoint::pin(s.m1_sym, phase)
            ),
            "phase {phase} from -K1 to -M1"
        );
    }
    // PE runs through -X1:PE to the motor.
    assert_eq!(net.potential_of(Endpoint::pin(s.m1_sym, 3)), Some(s.pe));
    // Phases stay apart.
    assert!(!net.connected(Endpoint::pin(s.m1_sym, 0), Endpoint::pin(s.m1_sym, 1)));
}

#[test]
fn control_circuit_self_hold() {
    let (doc, s) = starter();
    let p = doc.project();
    let net = p.netlist();
    assert_eq!(net.potential_of(Endpoint::pin(s.f3_sym, 0)), Some(s.l));
    assert_eq!(net.potential_of(Endpoint::pin(s.k1_coil, 1)), Some(s.n));
    // Start and self-hold contact are in parallel.
    assert!(net.connected(Endpoint::pin(s.s1_sym, 0), Endpoint::pin(s.k1_aux, 0)));
    assert!(net.connected(Endpoint::pin(s.s1_sym, 1), Endpoint::pin(s.k1_aux, 1)));
    // Stop is in series ahead of them, the overload contact after them.
    assert!(net.connected(Endpoint::pin(s.s0_sym, 1), Endpoint::pin(s.s1_sym, 0)));
    assert!(net.connected(Endpoint::pin(s.s1_sym, 1), Endpoint::pin(s.f2_aux, 0)));
    assert!(net.connected(Endpoint::pin(s.f2_aux, 1), Endpoint::pin(s.k1_coil, 0)));
    // L passes through -F3, -S0, -S1/-K1 and -F2 95-96 to the coil, which
    // ends it: feed, three nets between switches, coil A1.
    assert_eq!(net.potential_of(Endpoint::pin(s.k1_coil, 0)), Some(s.l));
    assert_eq!(net.nets_with_potential(s.l).count(), 5);
    assert_eq!(net.nets_with_potential(s.n).count(), 1);
    // L1 covers three nets: feed to -F1, -F1 to -K1, and -K1 through the
    // overload heaters (a bridge) and -X1 to the motor. Control and main
    // circuits stay separate supplies with no conflicts.
    assert_eq!(net.nets_with_potential(s.l1).count(), 3);
    assert_eq!(net.potential_conflicts().count(), 0);
}

#[test]
fn devices_span_pages() {
    let (doc, s) = starter();
    let p = doc.project();
    // -K1: coil (main function first), main contacts, auxiliary contact.
    let k1: Vec<SymbolId> = p.symbols_of_device(s.k1).iter().map(|x| x.id).collect();
    assert_eq!(k1.len(), 3);
    assert_eq!(k1[0], s.k1_coil);
    let k1_set: BTreeSet<_> = k1.into_iter().collect();
    assert_eq!(k1_set, BTreeSet::from([s.k1_coil, s.k1_main, s.k1_aux]));
    let k1_pages: BTreeSet<_> = p.symbols_of_device(s.k1).iter().map(|x| x.page).collect();
    assert_eq!(k1_pages, BTreeSet::from([s.main_page, s.control_page]));
    // -F2: main path on page 1, 95-96 on page 2.
    assert_eq!(p.symbols_of_device(s.f2).len(), 2);
}

#[test]
fn designations_read_like_a_drawing() {
    let (doc, s) = starter();
    let p = doc.project();
    assert_eq!(
        p.pin_designation(PinRef::new(s.k1_coil, 0)).as_deref(),
        Some("=A1+CAB-K1:A1")
    );
    assert_eq!(
        p.pin_designation(PinRef::new(s.f2_aux, 1)).as_deref(),
        Some("=A1+CAB-F2:96")
    );
    assert_eq!(
        p.pin_designation(PinRef::new(s.m1_sym, 2)).as_deref(),
        Some("=A1+FLD-M1:W1")
    );
    let strip: Vec<String> = p
        .terminals_of_strip(s.x1)
        .iter()
        .map(|t| p.terminal_designation(t.id).unwrap())
        .collect();
    assert_eq!(
        strip,
        [
            "=A1+CAB-X1:1",
            "=A1+CAB-X1:2",
            "=A1+CAB-X1:3",
            "=A1+CAB-X1:PE"
        ]
    );
}

#[test]
fn terminal_kinds_match_their_use() {
    let (doc, s) = starter();
    let kinds: Vec<(String, TerminalKind)> = doc
        .project()
        .terminals_of_strip(s.x1)
        .into_iter()
        .map(|t| (t.name.clone(), t.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            ("1".to_string(), TerminalKind::Feedthrough),
            ("2".to_string(), TerminalKind::Feedthrough),
            ("3".to_string(), TerminalKind::Feedthrough),
            ("PE".to_string(), TerminalKind::ProtectiveEarth),
        ]
    );
}

#[test]
fn every_device_has_a_part_and_the_cable_its_cores() {
    let (doc, s) = starter();
    let p = doc.project();
    // The strip carries no part itself: its terminals' articles are the
    // strip style (see bom_counts_every_terminal_the_strip_shows).
    for device in p.devices().filter(|d| d.id != s.x1) {
        assert_eq!(device.parts.len(), 1, "{} has a part", device.tag);
    }
    assert!(novo_parts::missing_parts(p).is_empty());
    let cable = p.cable(s.w1).unwrap();
    assert!(cable.part.is_some());
    assert_eq!(cable.cores.len(), 4);
    for (core, wire) in s.motor_wires.iter().enumerate() {
        assert_eq!(p.core_of_wire(*wire), Some((s.w1, core)));
    }
}

#[test]
fn save_and_reopen_gives_an_identical_project() {
    let (doc, s) = starter();
    let p = doc.project();
    // JSON is the stand-in for the project file until novo-storage lands.
    let json = p.to_json();
    let reopened = Project::from_json(&json).unwrap();
    assert_eq!(reopened.to_json(), json);
    assert_eq!(reopened.next_id(), p.next_id());
    // Derived data is rebuilt, not stored.
    assert_eq!(
        reopened.netlist().potential_of(Endpoint::pin(s.m1_sym, 3)),
        Some(s.pe)
    );
    // Records (the storage hook) round-trip in any order.
    let mut records: Vec<Record> = p.records().collect();
    records.reverse();
    let rebuilt = Project::from_records(p.info().clone(), p.next_id(), records).unwrap();
    assert_eq!(rebuilt.to_json(), json);
    // A reopened project keeps editing without id clashes.
    let mut doc2 = Document::new(reopened);
    assert!(!doc2.is_dirty());
    doc2.transact("add", |tx| tx.add_page("3")).unwrap();
    assert_eq!(doc2.project().pages().count(), 3);
}

#[test]
fn replaying_the_recorded_edit_rebuilds_the_project() {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    // The undo step, serialized and applied to an empty project.
    let mut source = Project::default();
    let (_, recorded) = source
        .transact("Draw", |tx| {
            for record in doc.project().records() {
                tx.insert_record(record)?;
            }
            tx.set_info(|i| *i = doc.project().info().clone())
        })
        .unwrap();
    let wire = serde_json::to_string(&recorded).unwrap();
    let back: Transaction = serde_json::from_str(&wire).unwrap();
    let mut replayed = Project::default();
    replayed.apply(&back).unwrap();
    assert_eq!(state(&replayed), state(doc.project()));
    // And its inverse empties the project again.
    replayed.apply(&back.inverse()).unwrap();
    assert_eq!(replayed.records().count(), 0);
}

#[test]
fn whole_build_undoes_and_redoes() {
    let (mut doc, _) = starter();
    let built = state(doc.project());
    assert!(doc.is_dirty());
    doc.undo().unwrap();
    assert_eq!(doc.project().records().count(), 0);
    doc.redo().unwrap();
    assert_eq!(state(doc.project()), built);
}

#[test]
fn deleting_the_contactor_takes_its_symbols_and_wires_and_undo_restores_them() {
    let (mut doc, s) = starter();
    let built = state(doc.project());
    let wires_before = doc.project().wires().count();
    doc.transact("Delete -K1", |tx| tx.remove(s.k1)).unwrap();
    let p = doc.project();
    assert!(p.symbol(s.k1_coil).is_none());
    assert!(p.symbol(s.k1_main).is_none());
    assert!(p.symbol(s.k1_aux).is_none());
    // Main contacts had 6 wires, aux 2, coil 2.
    assert_eq!(p.wires().count(), wires_before - 10);
    assert!(!p.unconnected_pins().is_empty());
    doc.undo().unwrap();
    assert_eq!(state(doc.project()), built);
}

/// Terminal articles come from one place (`novo_terminals::terminal_part`):
/// the -X1 terminal blocks are the strip's style, so the strip editor shows
/// them on every row, the bill of materials counts one per terminal (not one
/// against -X1), and the missing-parts check agrees.
#[test]
fn bom_counts_every_terminal_the_strip_shows() {
    use novo_reports::{BillOfMaterials, Report};

    let (doc, s) = starter();
    let p = doc.project();
    assert!(p.device(s.x1).unwrap().parts.is_empty());

    let rows = novo_terminals::strip_view(p, s.x1).rows;
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().all(|r| r.part.is_some()));
    for row in &rows {
        assert_eq!(novo_parts::terminal_part(p, row.terminals[0]), row.part);
    }

    let bom = BillOfMaterials.generate(p);
    let parts = bom.sections.iter().find(|s| s.key == "parts").unwrap();
    let terminal_rows: Vec<_> = parts
        .rows
        .iter()
        .filter(|r| r.sources.iter().any(|id| matches!(id, AnyId::Terminal(_))))
        .collect();
    let quantity: usize = terminal_rows
        .iter()
        .map(|r| {
            r.sources
                .iter()
                .filter(|id| matches!(id, AnyId::Terminal(_)))
                .count()
        })
        .sum();
    assert_eq!(quantity, 4, "one terminal block per terminal");
    assert!(
        parts
            .rows
            .iter()
            .all(|r| !r.sources.contains(&AnyId::from(s.x1)))
    );
    assert!(bom.sections.iter().all(|s| s.key != "without_parts"));
    assert!(novo_parts::missing_parts(p).is_empty());
}
