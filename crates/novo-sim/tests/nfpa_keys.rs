//! The same circuits as in `devices.rs` and `plc.rs`, drawn with NFPA keys.

use std::collections::BTreeSet;

use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId, SymbolRole,
};
use novo_sim::{
    Clock, Direction, MotorRun, PoleState, device_states, inputs_seen, settle, settle_timed,
};

fn feed(
    tx: &mut novo_core::Tx<'_>,
    page: novo_core::PageId,
    x: f64,
    p: novo_core::PotentialId,
) -> SymbolId {
    tx.place_symbol(
        page,
        "t/feed",
        Placement::at(Point::new(x, -20.0)),
        vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
        SymbolRole::PotentialFeed { potential: p },
    )
    .unwrap()
}

/// A device fed by `feeds[i]` on pin `i`; returns its settled states.
fn drawn(key: &str, pins: &[&str], feeds: &[(&str, PotentialKind)]) -> (Document, SymbolId) {
    let mut doc = Document::default();
    let (load, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let pins = pins
                .iter()
                .enumerate()
                .map(|(i, n)| Pin::new(*n, i as f64 * 8.0, 0.0))
                .collect();
            let load = tx.place_symbol(
                page,
                key,
                Placement::at(Point::new(0.0, 0.0)),
                pins,
                SymbolRole::Unassigned,
            )?;
            for (i, (name, kind)) in feeds.iter().enumerate() {
                let p = tx.add_potential(*name, *kind)?;
                let f = feed(tx, page, i as f64 * 8.0, p);
                tx.connect(Endpoint::pin(f, 1), Endpoint::pin(load, i as u16))?;
            }
            Ok(load)
        })
        .unwrap();
    (doc, load)
}

fn states(doc: &Document) -> novo_sim::DeviceStates {
    let project = doc.project();
    let netlist = project.netlist();
    let none = BTreeSet::new();
    let settled = settle(project, &netlist, &PoleState::all_open(), &none, &none);
    device_states(project, &netlist, &settled)
}

#[test]
fn an_nfpa_lamp_lights() {
    use PotentialKind::{Line, Neutral};
    let (doc, lamp) = drawn(
        "nfpa/signalling/lamp_red",
        &["X1", "X2"],
        &[("L1", Line), ("N", Neutral)],
    );
    assert!(states(&doc).lamps[&lamp]);
}

#[test]
fn an_nfpa_motor_turns() {
    use PotentialKind::Line;
    let (doc, motor) = drawn(
        "nfpa/motors/motor_3ph",
        &["U1", "V1", "W1", "PE"],
        &[("L1", Line), ("L2", Line), ("L3", Line)],
    );
    assert_eq!(
        states(&doc).motors[&motor],
        MotorRun::Running {
            direction: Direction::Forward,
            speed: 0
        }
    );
}

#[test]
fn an_nfpa_plc_output_lights_the_input_it_is_wired_to() {
    let mut doc = Document::default();
    let ((dq, di), _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            tx.add_potential("24V", PotentialKind::DcPositive)?;
            let a = tx.add_device(DeviceTag::product("A1"), "")?;
            let b = tx.add_device(DeviceTag::product("A2"), "")?;
            let mut put = |key: &str, y: f64, device| {
                tx.place_symbol(
                    page,
                    key,
                    Placement::at(Point::new(0.0, y)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Device {
                        device,
                        main: false,
                    },
                )
            };
            let dq = put("nfpa/plc/digital_output", 0.0, a)?;
            let di = put("nfpa/plc/digital_input", 20.0, b)?;
            tx.connect(Endpoint::pin(dq, 0), Endpoint::pin(di, 0))?;
            Ok((dq, di))
        })
        .unwrap();
    let project = doc.project();
    let netlist = project.netlist();
    let settled = settle_timed(
        project,
        &netlist,
        &PoleState::all_open(),
        &BTreeSet::from([dq]),
        &BTreeSet::new(),
        &mut Clock::new(),
    );
    assert_eq!(
        inputs_seen(project, &netlist, &settled.solution),
        BTreeSet::from([di])
    );
}
