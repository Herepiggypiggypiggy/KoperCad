//! The Gate 1 project: a direct-on-line motor starter drawn over two pages.
//!
//! Page 1 is the main circuit: L1/L2/L3 through fuses `-F1`, contactor main
//! contacts `-K1`, overload relay `-F2` and terminals `-X1` to motor `-M1`,
//! with cable `-W1` from the terminals to the motor. Page 2 is the control
//! circuit: `L` through `-F3`, stop `-S0`, start `-S1` in parallel with the
//! self-hold contact `-K1 13/14`, the overload contact `-F2 95/96` and the
//! coil `-K1 A1/A2` to `N`.
//!
//! Symbol keys, connection designations and offsets follow the WS-05 symbol
//! library (`iec/...` ids, 4 mm grid, y down), so the fixture can be checked
//! against it once that library is on main. Every device carries a part, as
//! the WS-07 done-criterion asks.

use novo_core::{
    CableCore, CableId, Device, DeviceId, DeviceTag, Document, Endpoint, JunctionId, PageId, Pin,
    Placement, Point, PotentialId, PotentialKind, Result, SymbolId, SymbolInstance, SymbolRole, Tx,
    WireId,
};

/// Handles to the objects of the Gate 1 project.
#[derive(Debug, Clone)]
pub struct MotorStarter {
    pub main_page: PageId,
    pub control_page: PageId,

    pub l1: PotentialId,
    pub l2: PotentialId,
    pub l3: PotentialId,
    pub pe: PotentialId,
    pub l: PotentialId,
    pub n: PotentialId,

    pub f1: DeviceId,
    pub k1: DeviceId,
    pub f2: DeviceId,
    pub x1: DeviceId,
    pub m1: DeviceId,
    pub f3: DeviceId,
    pub s0: DeviceId,
    pub s1: DeviceId,

    pub f1_sym: SymbolId,
    pub k1_main: SymbolId,
    pub f2_main: SymbolId,
    /// `-X1:1`, `-X1:2`, `-X1:3`, `-X1:PE`.
    pub x1_syms: [SymbolId; 4],
    pub m1_sym: SymbolId,
    pub f3_sym: SymbolId,
    pub s0_sym: SymbolId,
    pub s1_sym: SymbolId,
    pub k1_aux: SymbolId,
    pub f2_aux: SymbolId,
    pub k1_coil: SymbolId,

    pub w1: CableId,
    /// The wires from `-X1` to `-M1` U1, V1, W1, PE (carried by `-W1`).
    pub motor_wires: [WireId; 4],
    pub hold_split: JunctionId,
    pub hold_join: JunctionId,
}

/// Number of devices in the fixture (including the terminal strip).
pub const DEVICE_COUNT: usize = 8;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("fixture tags are valid")
}

fn at(x: f64, y: f64) -> Placement {
    Placement::at(Point::new(x, y))
}

fn pins(list: &[(&str, f64, f64)]) -> Vec<Pin> {
    list.iter().map(|&(n, x, y)| Pin::new(n, x, y)).collect()
}

/// Pins of a three-pole device (`iec/protection/fuse_3p`,
/// `iec/contactors/main_3p`, `iec/protection/overload_relay`).
fn three_pole() -> Vec<Pin> {
    pins(&[
        ("1", 0.0, 0.0),
        ("2", 0.0, 16.0),
        ("3", 8.0, 0.0),
        ("4", 8.0, 16.0),
        ("5", 16.0, 0.0),
        ("6", 16.0, 16.0),
    ])
}

/// A two-pin vertical symbol 16 mm long (contacts, coils, 1-pole breakers).
fn two_pin(a: &str, b: &str) -> Vec<Pin> {
    pins(&[(a, 0.0, 0.0), (b, 0.0, 16.0)])
}

fn device(tx: &mut Tx<'_>, tag_text: &str, description: &str) -> Result<DeviceId> {
    tx.add_device(tag(tag_text), description)
}

fn role(device: DeviceId, main: bool) -> SymbolRole {
    SymbolRole::Device { device, main }
}

/// Builds the Gate 1 project as one undo step and returns its handles.
pub fn motor_starter(doc: &mut Document) -> Result<MotorStarter> {
    let (starter, _) = doc.transact("Draw Gate 1 motor starter", build)?;
    Ok(starter)
}

fn build(tx: &mut Tx<'_>) -> Result<MotorStarter> {
    tx.set_info(|i| i.name = "Gate 1 DOL starter".into())?;
    let main_page = tx.add_page("1")?;
    let control_page = tx.add_page("2")?;
    tx.update::<novo_core::Page>(main_page, |p| p.title = "Main circuit".into())?;
    tx.update::<novo_core::Page>(control_page, |p| p.title = "Control circuit".into())?;

    let l1 = tx.add_potential("L1", PotentialKind::Line)?;
    let l2 = tx.add_potential("L2", PotentialKind::Line)?;
    let l3 = tx.add_potential("L3", PotentialKind::Line)?;
    let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
    let l = tx.add_potential("L", PotentialKind::Line)?;
    let n = tx.add_potential("N", PotentialKind::Neutral)?;

    let f1 = device(tx, "=A1+CAB-F1", "Main circuit fuses")?;
    let k1 = device(tx, "=A1+CAB-K1", "Motor contactor")?;
    let f2 = device(tx, "=A1+CAB-F2", "Overload relay")?;
    let x1 = tx.add_terminal_strip(tag("=A1+CAB-X1"))?;
    let m1 = device(tx, "=A1+FLD-M1", "Pump motor 2.2 kW")?;
    let f3 = device(tx, "=A1+CAB-F3", "Control circuit breaker")?;
    let s0 = device(tx, "=A1+CAB-S0", "Stop")?;
    let s1 = device(tx, "=A1+CAB-S1", "Start")?;

    // ----- page 1: main circuit ------------------------------------------
    let feed = |tx: &mut Tx<'_>, page, potential, x| {
        tx.place_symbol(
            page,
            "iec/connectivity/potential_feed",
            at(x, 20.0),
            pins(&[("1", 0.0, 8.0)]),
            SymbolRole::PotentialFeed { potential },
        )
    };
    let feeds = [
        feed(tx, main_page, l1, 40.0)?,
        feed(tx, main_page, l2, 48.0)?,
        feed(tx, main_page, l3, 56.0)?,
    ];
    let feed_pe = feed(tx, main_page, pe, 72.0)?;

    let f1_sym = tx.place_symbol(
        main_page,
        "iec/protection/fuse_3p",
        at(40.0, 40.0),
        three_pole(),
        role(f1, true),
    )?;
    let k1_main = tx.place_symbol(
        main_page,
        "iec/contactors/main_3p",
        at(40.0, 72.0),
        three_pole(),
        role(k1, false),
    )?;
    let f2_main = tx.place_symbol(
        main_page,
        "iec/protection/overload_relay",
        at(40.0, 104.0),
        three_pole(),
        role(f2, true),
    )?;
    // The overload relay's heaters conduct; only its auxiliary contact is a
    // switching element.
    tx.update::<SymbolInstance>(f2_main, |s| {
        s.bridges = vec![vec![0, 1], vec![2, 3], vec![4, 5]]
    })?;

    let mut x1_syms = Vec::new();
    for (i, (name, x)) in [("1", 40.0), ("2", 48.0), ("3", 56.0), ("PE", 72.0)]
        .into_iter()
        .enumerate()
    {
        let terminal = tx.add_terminal(x1, name)?;
        let key = if i == 3 {
            "iec/terminals/terminal_pe"
        } else {
            "iec/terminals/terminal"
        };
        x1_syms.push(tx.place_symbol(
            main_page,
            key,
            at(x, 136.0),
            pins(&[("1", 0.0, 0.0), ("2", 0.0, 8.0)]),
            SymbolRole::Terminal { terminal },
        )?);
    }
    let x1_syms: [SymbolId; 4] = x1_syms.try_into().expect("four terminals");

    let m1_sym = tx.place_symbol(
        main_page,
        "iec/motors/motor_3ph",
        at(40.0, 180.0),
        pins(&[
            ("U1", 0.0, 0.0),
            ("V1", 8.0, 0.0),
            ("W1", 16.0, 0.0),
            ("PE", 24.0, 0.0),
        ]),
        role(m1, true),
    )?;

    for phase in 0..3u16 {
        let top = phase * 2;
        let bottom = top + 1;
        tx.connect(
            Endpoint::pin(feeds[usize::from(phase)], 0),
            Endpoint::pin(f1_sym, top),
        )?;
        tx.connect(Endpoint::pin(f1_sym, bottom), Endpoint::pin(k1_main, top))?;
        tx.connect(Endpoint::pin(k1_main, bottom), Endpoint::pin(f2_main, top))?;
        tx.connect(
            Endpoint::pin(f2_main, bottom),
            Endpoint::pin(x1_syms[usize::from(phase)], 0),
        )?;
    }
    tx.connect(Endpoint::pin(feed_pe, 0), Endpoint::pin(x1_syms[3], 0))?;
    let mut motor_wires = Vec::new();
    for (i, terminal) in x1_syms.iter().enumerate() {
        let pin = u16::try_from(i).expect("four motor pins");
        motor_wires.push(tx.connect(Endpoint::pin(*terminal, 1), Endpoint::pin(m1_sym, pin))?);
    }
    let motor_wires: [WireId; 4] = motor_wires.try_into().expect("four motor wires");

    let mut cores = Vec::new();
    for name in ["BK", "BN", "GY", "GNYE"] {
        let mut core = CableCore::new(name);
        core.cross_section_mm2 = Some(1.5);
        cores.push(core);
    }
    let w1 = tx.add_cable(tag("=A1-W1"), cores)?;
    for (core, wire) in motor_wires.iter().enumerate() {
        tx.assign_core(w1, core, Some(*wire))?;
    }

    // ----- page 2: control circuit ---------------------------------------
    let feed_l = feed(tx, control_page, l, 40.0)?;
    let f3_sym = tx.place_symbol(
        control_page,
        "iec/protection/mcb_1p",
        at(40.0, 40.0),
        two_pin("1", "2"),
        role(f3, true),
    )?;
    let s0_sym = tx.place_symbol(
        control_page,
        "iec/operators/pushbutton_nc",
        at(40.0, 72.0),
        two_pin("11", "12"),
        role(s0, true),
    )?;
    let s1_sym = tx.place_symbol(
        control_page,
        "iec/operators/pushbutton_no",
        at(40.0, 104.0),
        two_pin("13", "14"),
        role(s1, true),
    )?;
    let k1_aux = tx.place_symbol(
        control_page,
        "iec/contacts/no",
        at(56.0, 104.0),
        two_pin("13", "14"),
        role(k1, false),
    )?;
    let f2_aux = tx.place_symbol(
        control_page,
        "iec/contacts/thermal_nc",
        at(40.0, 136.0),
        two_pin("95", "96"),
        role(f2, false),
    )?;
    let k1_coil = tx.place_symbol(
        control_page,
        "iec/coils/relay",
        at(40.0, 168.0),
        two_pin("A1", "A2"),
        role(k1, true),
    )?;
    let feed_n = tx.place_symbol(
        control_page,
        "iec/connectivity/potential_feed",
        at(40.0, 192.0),
        pins(&[("1", 0.0, 8.0)]),
        SymbolRole::PotentialFeed { potential: n },
    )?;

    tx.connect(Endpoint::pin(feed_l, 0), Endpoint::pin(f3_sym, 0))?;
    tx.connect(Endpoint::pin(f3_sym, 1), Endpoint::pin(s0_sym, 0))?;
    let hold_split = tx.add_junction(control_page, Point::new(40.0, 96.0))?;
    tx.connect(Endpoint::pin(s0_sym, 1), Endpoint::junction(hold_split))?;
    tx.connect(Endpoint::junction(hold_split), Endpoint::pin(s1_sym, 0))?;
    tx.connect(Endpoint::junction(hold_split), Endpoint::pin(k1_aux, 0))?;
    let hold_join = tx.add_junction(control_page, Point::new(40.0, 128.0))?;
    tx.connect(Endpoint::pin(s1_sym, 1), Endpoint::junction(hold_join))?;
    tx.connect(Endpoint::pin(k1_aux, 1), Endpoint::junction(hold_join))?;
    tx.connect(Endpoint::junction(hold_join), Endpoint::pin(f2_aux, 0))?;
    tx.connect(Endpoint::pin(f2_aux, 1), Endpoint::pin(k1_coil, 0))?;
    tx.connect(Endpoint::pin(k1_coil, 1), Endpoint::pin(feed_n, 0))?;

    // ----- parts ----------------------------------------------------------
    // Fixture data: plausible catalogue entries, not a verified parts list.
    let parts: [(DeviceId, &str, &str); DEVICE_COUNT] = [
        (f1, "Siemens", "3NW7033"),
        (k1, "Siemens", "3RT2016-1AP01"),
        (f2, "Siemens", "3RU2116-1FB0"),
        (x1, "Phoenix Contact", "3044076"),
        (m1, "Siemens", "1LE1003-0EB42-2AA4"),
        (f3, "Siemens", "5SY6106-7"),
        (s0, "Siemens", "3SU1100-0AB20-1CA0"),
        (s1, "Siemens", "3SU1100-0AB40-1BA0"),
    ];
    for (device, manufacturer, order_number) in parts {
        let part = tx.add_part(manufacturer, order_number)?;
        tx.assign_part(device, part)?;
    }
    let cable_part = tx.add_part("Lapp", "1119304")?;
    tx.update::<novo_core::Cable>(w1, |c| {
        c.part = Some(cable_part);
        c.length_m = Some(12.0);
        c.description = "Motor cable 4G1.5".into();
    })?;
    tx.update::<Device>(m1, |d| {
        d.properties.insert("power_kw".into(), 2.2.into());
    })?;

    Ok(MotorStarter {
        main_page,
        control_page,
        l1,
        l2,
        l3,
        pe,
        l,
        n,
        f1,
        k1,
        f2,
        x1,
        m1,
        f3,
        s0,
        s1,
        f1_sym,
        k1_main,
        f2_main,
        x1_syms,
        m1_sym,
        f3_sym,
        s0_sym,
        s1_sym,
        k1_aux,
        f2_aux,
        k1_coil,
        w1,
        motor_wires,
        hold_split,
        hold_join,
    })
}
