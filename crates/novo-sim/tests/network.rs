//! The drawing as a circuit: a feeder, contactor and motor, and a DC lamp.

use novo_core::{
    DeviceTag, Document, Endpoint, Pin, Placement, Point, PotentialKind, PotentialSet, SymbolId,
    SymbolRole, Tx, Value,
};
use novo_sim::PoleState;
use novo_sim::network::{NoteKind, analyse};

fn close(got: f64, want: f64, rel: f64) {
    let tol = rel * want.abs().max(1e-12);
    assert!((got - want).abs() <= tol, "got {got}, want {want} (±{tol})");
}

fn put(
    tx: &mut Tx<'_>,
    page: novo_core::PageId,
    key: &str,
    y: f64,
    pins: &[&str],
    role: SymbolRole,
) -> SymbolId {
    let pins = pins
        .iter()
        .enumerate()
        .map(|(i, n)| Pin::new(*n, i as f64 * 8.0, 0.0))
        .collect();
    tx.place_symbol(page, key, Placement::at(Point::new(0.0, y)), pins, role)
        .unwrap()
}

struct Feeder {
    doc: Document,
    contactor: SymbolId,
    motor: SymbolId,
}

fn feeder(power_kw: Option<f64>, short_circuit_ka: Option<f64>) -> Feeder {
    let mut doc = Document::default();
    let (ids, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let (_, pots) = tx.add_potential_system(
                "400 V",
                PotentialSet::ThreePhase {
                    voltage: 400.0,
                    neutral: true,
                    pe: true,
                    pen: false,
                },
                Some(50.0),
                "",
            )?;
            if let Some(ka) = short_circuit_ka {
                for &p in &pots[..3] {
                    tx.set_property(p, "short_circuit_ka", Some(Value::Float(ka)))?;
                }
            }
            let k1 = tx.add_device(DeviceTag::product("K1"), "")?;
            let m1 = tx.add_device(DeviceTag::product("M1"), "")?;
            if let Some(kw) = power_kw {
                tx.set_property(m1, "power_kw", Some(Value::Float(kw)))?;
            }
            let contactor = put(
                tx,
                page,
                "iec/contacts/contactor_3p",
                20.0,
                &["1", "2", "3", "4", "5", "6"],
                SymbolRole::Device {
                    device: k1,
                    main: true,
                },
            );
            tx.update::<novo_core::SymbolInstance>(contactor, |s| {
                s.pass_through = vec![vec![0, 1], vec![2, 3], vec![4, 5]];
            })?;
            let motor = put(
                tx,
                page,
                "iec/motors/motor_3ph",
                40.0,
                &["U1", "V1", "W1", "PE"],
                SymbolRole::Device {
                    device: m1,
                    main: true,
                },
            );
            for (i, &p) in pots[..3].iter().enumerate() {
                let feed = put(
                    tx,
                    page,
                    "t/feed",
                    -20.0 - i as f64 * 8.0,
                    &["1"],
                    SymbolRole::PotentialFeed { potential: p },
                );
                tx.connect(
                    Endpoint::pin(feed, 0),
                    Endpoint::pin(contactor, 2 * i as u16),
                )?;
                tx.connect(
                    Endpoint::pin(contactor, 2 * i as u16 + 1),
                    Endpoint::pin(motor, i as u16),
                )?;
            }
            Ok((contactor, motor))
        })
        .unwrap();
    Feeder {
        doc,
        contactor: ids.0,
        motor: ids.1,
    }
}

#[test]
fn closed_contactor_feeds_the_motor_its_rated_current() {
    let f = feeder(Some(7.5), None);
    let netlist = novo_core::Netlist::build(f.doc.project());
    let mut poles = PoleState::all_open();
    poles.set_all(f.doc.project(), f.contactor, true);
    let a = analyse(f.doc.project(), &netlist, &poles);
    let load = a.load(f.motor).unwrap_or_else(|| panic!("{:?}", a.notes));
    // IE3 typical values for 7.5 kW at 400 V: about 14.5 A, 8.3 kW in.
    assert!((13.5..15.5).contains(&load.current), "{}", load.current);
    assert!((8.0e3..8.8e3).contains(&load.power_w), "{}", load.power_w);
    assert!(
        load.reactive_var > 4.0e3,
        "inductive: {}",
        load.reactive_var
    );
    // Phase to neutral is 230.9 V on the motor side.
    let net = netlist
        .net_of(Endpoint::pin(f.motor, 0))
        .expect("motor pin is on a net");
    close(a.net(net).ac.abs(), 400.0 / 3f64.sqrt(), 1e-6);
    // The supply delivers what the motor takes; total over three lines.
    let delivered: f64 = a.supplies.iter().map(|s| s.current).sum();
    close(delivered, 3.0 * load.current, 1e-6);
    // Nothing the drawing leaves out is silent.
    assert!(
        a.notes
            .iter()
            .any(|n| n.kind == NoteKind::Assumed && n.symbol == Some(f.motor))
    );
}

#[test]
fn open_contactor_draws_nothing() {
    let f = feeder(Some(7.5), None);
    let netlist = novo_core::Netlist::build(f.doc.project());
    let a = analyse(f.doc.project(), &netlist, &PoleState::all_open());
    assert!(
        a.load(f.motor)
            .unwrap_or_else(|| panic!("{:?}", a.notes))
            .current
            < 1e-6
    );
    let net = netlist.net_of(Endpoint::pin(f.motor, 0)).unwrap();
    assert!(a.net(net).ac.abs() < 1e-3);
}

#[test]
fn a_motor_without_power_is_reported_not_guessed() {
    let f = feeder(None, None);
    let netlist = novo_core::Netlist::build(f.doc.project());
    let a = analyse(f.doc.project(), &netlist, &PoleState::all_open());
    assert!(a.load(f.motor).is_none());
    assert!(
        a.notes
            .iter()
            .any(|n| n.kind == NoteKind::Missing && n.symbol == Some(f.motor))
    );
}

#[test]
fn supply_impedance_drops_the_voltage_under_load() {
    // A weak 1.5 kA source: 7.5 kW motor sees a visible drop.
    let f = feeder(Some(7.5), Some(1.5));
    let netlist = novo_core::Netlist::build(f.doc.project());
    let mut poles = PoleState::all_open();
    poles.set_all(f.doc.project(), f.contactor, true);
    let a = analyse(f.doc.project(), &netlist, &poles);
    let net = netlist.net_of(Endpoint::pin(f.motor, 0)).unwrap();
    let v = a.net(net).ac.abs();
    let nominal = 400.0 / 3f64.sqrt();
    assert!(v < nominal && v > 0.9 * nominal, "{v}");
    // Z = V/Ik = 0.154 ohm; at 14.5 A the drop is about 2.2 V (partly reactive).
    assert!(nominal - v > 0.5);
}

#[test]
fn dc_lamp_draws_power_over_voltage() {
    let mut doc = Document::default();
    let (lamp, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let (_, pots) =
                tx.add_potential_system("24 V", PotentialSet::Dc { voltage: 24.0 }, None, "")?;
            let h1 = tx.add_device(DeviceTag::product("H1"), "")?;
            tx.set_property(h1, "power_w", Some(Value::Float(2.4)))?;
            let lamp = put(
                tx,
                page,
                "iec/signalling/lamp",
                0.0,
                &["X1", "X2"],
                SymbolRole::Device {
                    device: h1,
                    main: true,
                },
            );
            for (i, &p) in pots.iter().take(2).enumerate() {
                let feed = put(
                    tx,
                    page,
                    "t/feed",
                    -20.0 - i as f64 * 8.0,
                    &["1"],
                    SymbolRole::PotentialFeed { potential: p },
                );
                tx.connect(Endpoint::pin(feed, 0), Endpoint::pin(lamp, i as u16))?;
            }
            Ok(lamp)
        })
        .unwrap();
    let netlist = novo_core::Netlist::build(doc.project());
    let a = analyse(doc.project(), &netlist, &PoleState::all_open());
    let l = a.load(lamp).unwrap();
    close(l.current, 0.1, 1e-6);
    close(l.power_w, 2.4, 1e-6);
    let _ = PotentialKind::DcPositive;
}
