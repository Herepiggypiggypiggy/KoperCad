use std::collections::BTreeSet;

use novo_core::{
    Document, Endpoint, Pin, Placement, Point, PotentialId, PotentialKind, SymbolId, SymbolRole, Tx,
};
use novo_sim::{Direction, MotorRun, PoleState, device_states, settle};

fn put(tx: &mut Tx<'_>, page: novo_core::PageId, key: &str, x: f64, pins: &[&str]) -> SymbolId {
    let pins = pins
        .iter()
        .enumerate()
        .map(|(i, n)| Pin::new(*n, i as f64 * 8.0, 0.0))
        .collect();
    tx.place_symbol(
        page,
        key,
        Placement::at(Point::new(x, 0.0)),
        pins,
        SymbolRole::Unassigned,
    )
    .unwrap()
}

fn feed(tx: &mut Tx<'_>, page: novo_core::PageId, x: f64, p: PotentialId) -> SymbolId {
    tx.place_symbol(
        page,
        "t/feed",
        Placement::at(Point::new(x, -20.0)),
        vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
        SymbolRole::PotentialFeed { potential: p },
    )
    .unwrap()
}

/// Wires `feeds[i]` to pin `i` of `load` and reports its state.
fn run(key: &str, pins: &[&str], feeds: &[(&str, PotentialKind)]) -> Doc {
    let mut doc = Document::default();
    let ((load, count), _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let load = put(tx, page, key, 0.0, pins);
            for (i, (name, kind)) in feeds.iter().enumerate() {
                let p = tx.add_potential(*name, *kind)?;
                let f = feed(tx, page, i as f64 * 8.0, p);
                tx.connect(Endpoint::pin(f, 1), Endpoint::pin(load, i as u16))?;
            }
            Ok((load, feeds.len()))
        })
        .unwrap();
    let _ = count;
    Doc { doc, load }
}

struct Doc {
    doc: Document,
    load: SymbolId,
}

impl Doc {
    fn motor(&self) -> MotorRun {
        let project = self.doc.project();
        let netlist = project.netlist();
        let none = BTreeSet::new();
        let settled = settle(project, &netlist, &PoleState::all_open(), &none, &none);
        device_states(project, &netlist, &settled).motors[&self.load]
    }

    fn lamp(&self) -> bool {
        let project = self.doc.project();
        let netlist = project.netlist();
        let none = BTreeSet::new();
        let settled = settle(project, &netlist, &PoleState::all_open(), &none, &none);
        device_states(project, &netlist, &settled).lamps[&self.load]
    }
}

use PotentialKind::{DcNegative, DcPositive, Line, Neutral};
const U: &[&str] = &["U1", "V1", "W1", "PE"];

#[test]
fn a_lamp_lights_between_two_potentials() {
    let on = run(
        "iec/signalling/lamp",
        &["X1", "X2"],
        &[("L1", Line), ("N", Neutral)],
    );
    assert!(on.lamp());
    let same = run("iec/signalling/lamp", &["X1", "X2"], &[("L1", Line)]);
    assert!(!same.lamp());
}

#[test]
fn a_motor_turns_with_the_phase_sequence() {
    let fwd = run(
        "iec/motors/motor_3ph",
        U,
        &[("L1", Line), ("L2", Line), ("L3", Line)],
    );
    assert_eq!(
        fwd.motor(),
        MotorRun::Running {
            direction: Direction::Forward,
            speed: 0
        }
    );
    // Rotations of the sequence still run forward; swapping two reverses.
    let rot = run(
        "iec/motors/motor_3ph",
        U,
        &[("L2", Line), ("L3", Line), ("L1", Line)],
    );
    assert_eq!(
        rot.motor(),
        MotorRun::Running {
            direction: Direction::Forward,
            speed: 0
        }
    );
    let rev = run(
        "iec/motors/motor_3ph",
        U,
        &[("L1", Line), ("L3", Line), ("L2", Line)],
    );
    assert_eq!(
        rev.motor(),
        MotorRun::Running {
            direction: Direction::Reverse,
            speed: 0
        }
    );
}

#[test]
fn a_motor_on_two_phases_hums_and_on_none_stands() {
    let two = run("iec/motors/motor_3ph", U, &[("L1", Line), ("L2", Line)]);
    assert_eq!(two.motor(), MotorRun::PhaseLoss);
    let one = run("iec/motors/motor_3ph", U, &[("L1", Line)]);
    assert_eq!(one.motor(), MotorRun::Stopped);
    let none = run("iec/motors/motor_3ph", U, &[]);
    assert_eq!(none.motor(), MotorRun::Stopped);
}

#[test]
fn single_phase_and_dc_motors() {
    let ac = run(
        "iec/motors/motor_1ph",
        &["U1", "U2", "PE"],
        &[("L1", Line), ("N", Neutral)],
    );
    assert_eq!(
        ac.motor(),
        MotorRun::Running {
            direction: Direction::Forward,
            speed: 0
        }
    );
    let dc = run(
        "iec/motors/motor_dc",
        &["A1", "A2"],
        &[("P", DcPositive), ("M", DcNegative)],
    );
    assert!(matches!(
        dc.motor(),
        MotorRun::Running {
            direction: Direction::Forward,
            ..
        }
    ));
    let back = run(
        "iec/motors/motor_dc",
        &["A1", "A2"],
        &[("M", DcNegative), ("P", DcPositive)],
    );
    assert!(matches!(
        back.motor(),
        MotorRun::Running {
            direction: Direction::Reverse,
            ..
        }
    ));
}

#[test]
fn a_dahlander_runs_at_the_speed_of_the_fed_set() {
    let pins = ["1U", "1V", "1W", "2U", "2V", "2W", "PE"];
    let low = run(
        "iec/motors/motor_3ph_dahlander",
        &pins,
        &[("L1", Line), ("L2", Line), ("L3", Line)],
    );
    assert!(matches!(low.motor(), MotorRun::Running { speed: 0, .. }));
}
