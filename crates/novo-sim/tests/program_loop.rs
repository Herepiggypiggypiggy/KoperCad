//! A PLC program running against a drawn circuit: the controller reads push
//! buttons through its input channels and switches a contactor through its
//! output channel, which closes a contact that lights a lamp.

use std::collections::BTreeSet;

use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, SymbolId, SymbolRole, Tx,
};
use novo_sim::program::{Bench, ClosedLoop, IoMap, Program};
use novo_sim::{PoleState, loads_on};

fn put(
    tx: &mut Tx<'_>,
    page: novo_core::PageId,
    key: &str,
    y: f64,
    pins: usize,
    role: SymbolRole,
) -> SymbolId {
    let pins = (0..pins)
        .map(|i| Pin::new(format!("{}", i + 1), 0.0, i as f64 * 8.0))
        .collect();
    let id = tx
        .place_symbol(page, key, Placement::at(Point::new(0.0, y)), pins, role)
        .unwrap();
    if key.contains("contact") {
        tx.update::<novo_core::SymbolInstance>(id, |s| s.pass_through = vec![vec![0, 1]])
            .unwrap();
    }
    id
}

struct Plant {
    doc: Document,
    start: SymbolId,
    stop: SymbolId,
    contactor: SymbolId,
    lamp: SymbolId,
    map: IoMap,
}

/// 24 V -> START (NO) -> DI0; 24 V -> STOP (NC, wired for wire-break safety)
/// -> DI1; DQ0 -> coil K1 -> 0 V; 24 V -> K1 contact -> lamp H1 -> 0 V.
fn plant() -> Plant {
    let mut doc = Document::default();
    let (p, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let v24 = tx.add_potential("24V", PotentialKind::DcPositive)?;
            let v0 = tx.add_potential("0V", PotentialKind::DcNegative)?;
            let plc = tx.add_device(DeviceTag::product("A1"), "")?;
            let k1 = tx.add_device(DeviceTag::product("K1"), "")?;
            let s1 = tx.add_device(DeviceTag::product("S1"), "")?;
            let s2 = tx.add_device(DeviceTag::product("S2"), "")?;
            let h1 = tx.add_device(DeviceTag::product("H1"), "")?;
            let dev = |device, main| SymbolRole::Device { device, main };
            let feed = |tx: &mut Tx<'_>, y, p| {
                put(
                    tx,
                    page,
                    "t/feed",
                    y,
                    2,
                    SymbolRole::PotentialFeed { potential: p },
                )
            };
            let f24 = feed(tx, 0.0, v24);
            let f0 = feed(tx, 10.0, v0);
            let start = put(tx, page, "t/contact/no", 20.0, 2, dev(s1, true));
            let stop = put(tx, page, "t/contact/nc", 30.0, 2, dev(s2, true));
            let di0 = put(tx, page, "iec/plc/digital_input", 40.0, 1, dev(plc, false));
            let di1 = put(tx, page, "iec/plc/digital_input", 50.0, 1, dev(plc, false));
            let dq0 = put(tx, page, "iec/plc/digital_output", 60.0, 1, dev(plc, false));
            let coil = put(tx, page, "t/coil", 70.0, 2, dev(k1, true));
            let k1c = put(tx, page, "t/contact/no", 80.0, 2, dev(k1, false));
            let lamp = put(tx, page, "t/lamp", 90.0, 2, dev(h1, true));
            tx.connect(Endpoint::pin(f24, 1), Endpoint::pin(start, 0))?;
            tx.connect(Endpoint::pin(f24, 1), Endpoint::pin(stop, 0))?;
            tx.connect(Endpoint::pin(f24, 1), Endpoint::pin(k1c, 0))?;
            tx.connect(Endpoint::pin(start, 1), Endpoint::pin(di0, 0))?;
            tx.connect(Endpoint::pin(stop, 1), Endpoint::pin(di1, 0))?;
            tx.connect(Endpoint::pin(dq0, 0), Endpoint::pin(coil, 0))?;
            tx.connect(Endpoint::pin(coil, 1), Endpoint::pin(f0, 1))?;
            tx.connect(Endpoint::pin(k1c, 1), Endpoint::pin(lamp, 0))?;
            tx.connect(Endpoint::pin(lamp, 1), Endpoint::pin(f0, 1))?;
            let mut map = IoMap::new();
            assert!(map.add_input("%IX0.0", di0));
            assert!(map.add_input("%I0.1", di1));
            assert!(map.add_output("Q0.0", dq0));
            Ok(Plant {
                doc: Document::default(),
                start,
                stop,
                contactor: coil,
                lamp,
                map,
            })
        })
        .unwrap();
    Plant { doc, ..p }
}

const PROGRAM: &str = r#"
VAR
    start   AT %IX0.0 : BOOL;
    stop_ok AT %IX0.1 : BOOL;   (* NC stop: TRUE while the circuit is healthy *)
    k1      AT %QX0.0 : BOOL;
END_VAR
RUNG "Contactor K1" { [start] | [k1] } -- [stop_ok] -- (k1);
"#;

struct Run<'a> {
    plant: &'a Plant,
    sim: ClosedLoop,
    netlist: novo_core::Netlist,
    hand: PoleState,
}

impl<'a> Run<'a> {
    fn new(plant: &'a Plant, program: &str) -> Run<'a> {
        let bench = Bench::new(Program::parse(program).unwrap(), plant.map.clone());
        let project = plant.doc.project();
        let mut hand = PoleState::all_open();
        // The stop button is normally closed.
        hand.set_all(project, plant.stop, true);
        Run {
            plant,
            sim: ClosedLoop::new(bench),
            netlist: project.netlist(),
            hand,
        }
    }

    fn run(&mut self, seconds: f64) {
        for _ in 0..(seconds / 0.02).round() as usize {
            self.sim.step(
                self.plant.doc.project(),
                &self.netlist,
                &self.hand,
                &BTreeSet::new(),
                0.02,
            );
        }
    }

    fn coil(&self) -> bool {
        self.sim
            .settled()
            .is_some_and(|s| s.energized.contains(&self.plant.contactor))
    }

    fn lamp(&self) -> bool {
        let s = self.sim.settled().unwrap();
        loads_on(self.plant.doc.project(), &self.netlist, &s.solution).contains(&self.plant.lamp)
    }

    fn hold(&mut self, start: bool, stop_pressed: bool) {
        let project = self.plant.doc.project();
        self.hand.set_all(project, self.plant.start, start);
        self.hand.set_all(project, self.plant.stop, !stop_pressed);
    }
}

#[test]
fn the_program_switches_the_contactor_through_the_drawing() {
    let plant = plant();
    let mut r = Run::new(&plant, PROGRAM);
    r.run(0.2);
    assert!(!r.coil() && !r.lamp());

    r.hold(true, false);
    r.run(0.2);
    assert!(r.coil(), "DQ0 energised K1");
    assert!(r.lamp(), "and K1's contact lit the lamp");

    // The button is released: the program's seal-in holds the contactor.
    r.hold(false, false);
    r.run(1.0);
    assert!(r.coil() && r.lamp());

    // Stop opens the NC contact, DI1 goes dead, the program drops K1.
    r.hold(false, true);
    r.run(0.2);
    assert!(!r.coil() && !r.lamp());
    // Releasing stop does not restart it.
    r.hold(false, false);
    r.run(0.5);
    assert!(!r.coil());
}

#[test]
fn a_timer_in_the_program_delays_the_lamp() {
    let plant = plant();
    let mut r = Run::new(
        &plant,
        r#"
        VAR start AT %IX0.0 : BOOL; k1 AT %QX0.0 : BOOL; t : TON; END_VAR
        t(IN := start, PT := T#3s);
        k1 := t.Q;
    "#,
    );
    r.hold(true, false);
    r.run(2.5);
    assert!(!r.lamp(), "before the delay");
    r.run(1.0);
    assert!(r.lamp(), "after 3 s");
    r.hold(false, false);
    r.run(0.2);
    assert!(!r.lamp());
}

#[test]
fn unbound_addresses_are_reported() {
    let plant = plant();
    let program = Program::parse(
        "VAR a AT %IX0.0 : BOOL; b AT %IX3.3 : BOOL; q AT %QX0.5 : BOOL; w AT %QW4 : INT; END_VAR q := a AND b;",
    )
    .unwrap();
    let bench = Bench::new(program, plant.map.clone());
    let unbound: Vec<String> = bench
        .unbound()
        .iter()
        .map(|u| format!("{} {}", if u.is_input { "in" } else { "out" }, u.address))
        .collect();
    assert_eq!(unbound, ["in %IX3.3", "out %QX0.5"]);
}

#[test]
fn a_faulted_program_keeps_the_outputs_and_reports_why() {
    let plant = plant();
    let mut r = Run::new(
        &plant,
        r#"
        VAR start AT %IX0.0 : BOOL; k1 AT %QX0.0 : BOOL; z : INT; END_VAR
        k1 := start;
        IF start THEN z := 1 / z; END_IF;
    "#,
    );
    r.run(0.1);
    r.hold(true, false);
    r.run(0.1);
    assert!(r.sim.bench.fault().is_some());
    assert!(!r.coil(), "the fault rolled the scan back before K1 closed");
}
