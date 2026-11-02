//! Building blocks shared by the three plant examples (roller door, conveyor
//! line, filling line): the incoming supply page, a direct-on-line motor
//! feeder and the generic articles they use. Everything is drawn through the
//! same [`Kit`] as the other gallery examples. Sizes and ratings are
//! planning values, not a verified design.

use novo_core::{PotentialId, PotentialKind, Result as ModelResult, SymbolId, Tx};

use super::example_kit::{Article, Kit};

/// The potentials every plant example uses.
#[derive(Clone, Copy)]
pub(super) struct Pots {
    pub l1: PotentialId,
    pub l2: PotentialId,
    pub l3: PotentialId,
    pub n: PotentialId,
    pub pe: PotentialId,
    pub p24: PotentialId,
    pub p0: PotentialId,
}

impl Pots {
    pub fn add(tx: &mut Tx<'_>) -> ModelResult<Pots> {
        Ok(Pots {
            l1: tx.add_potential("L1", PotentialKind::Line)?,
            l2: tx.add_potential("L2", PotentialKind::Line)?,
            l3: tx.add_potential("L3", PotentialKind::Line)?,
            n: tx.add_potential("N", PotentialKind::Neutral)?,
            pe: tx.add_potential("PE", PotentialKind::ProtectiveEarth)?,
            p24: tx.add_potential("+24V", PotentialKind::DcPositive)?,
            p0: tx.add_potential("0V", PotentialKind::DcNegative)?,
        })
    }
}

pub(super) const TERM: &str = "iec/terminals/terminal";
pub(super) const TERM_PE: &str = "iec/terminals/terminal_pe";
pub(super) const TERM_N: &str = "iec/terminals/terminal_n";

/// Page "Incoming supply and 24 V": the supply terminals `X0`, the main
/// switch `Q0`, the control supply breaker `Q10` and the 24 V power supply
/// `G1`, in `cab` (`=DOR+CAB`). The supply cable is `{plant}-W0`, `mm2` big.
pub(super) fn supply_page(
    kit: &mut Kit<'_, '_>,
    p: &Pots,
    cab: &str,
    plant: &str,
    x0: novo_core::DeviceId,
    mm2: f64,
) -> ModelResult<()> {
    let mut supply_wires = Vec::new();
    let mut tb = Vec::new();
    for (i, (name, key)) in [
        ("L1", TERM),
        ("L2", TERM),
        ("L3", TERM),
        ("N", TERM_N),
        ("PE", TERM_PE),
    ]
    .into_iter()
    .enumerate()
    {
        tb.push(kit.term(x0, name, key, (40.0 + 16.0 * i as f64, 40.0)));
    }
    for (i, pot) in [p.l1, p.l2, p.l3, p.n].into_iter().enumerate() {
        let f = kit.feed(pot, (40.0 + 16.0 * i as f64, 12.0));
        supply_wires.push(kit.wire_mm2((f, "1"), (tb[i], "1"), mm2));
    }
    let f = kit.feed_pe(p.pe, (104.0, 12.0));
    supply_wires.push(kit.wire_mm2((f, "1"), (tb[4], "1"), mm2));
    let fnn_down = kit.feed_down(p.n, (88.0, 56.0));
    kit.wire_mm2((tb[3], "2"), (fnn_down, "1"), mm2);
    let fpe_down = kit.feed_down(p.pe, (104.0, 56.0));
    kit.wire_mm2((tb[4], "2"), (fpe_down, "1"), mm2);
    let q0 = kit.put(
        "iec/switches/switch_disconnector_3p",
        (40.0, 66.0),
        &format!("{cab}-Q0"),
    );
    for (i, t) in tb.iter().take(3).enumerate() {
        let pole = ["1", "3", "5"][i];
        let corners: &[(f64, f64)] = match i {
            1 => &[(56.0, 50.0), (48.0, 50.0)],
            2 => &[(72.0, 54.0), (56.0, 54.0)],
            _ => &[],
        };
        let w = kit.wire_via((*t, "2"), (q0, pole), corners);
        kit.set_mm2(w, mm2);
    }
    kit.describe(&format!("{cab}-Q0"), "Main switch");
    for (i, pot) in [p.l1, p.l2, p.l3].into_iter().enumerate() {
        let pin = ["2", "4", "6"][i];
        let f = kit.feed_down(pot, (40.0 + 8.0 * i as f64, 94.0));
        kit.wire_mm2((q0, pin), (f, "1"), mm2);
    }
    let fl = kit.feed(p.l1, (120.0, 56.0));
    let fnn = kit.feed(p.n, (128.0, 56.0));
    let q10 = kit.put(
        "iec/protection/mcb_2p",
        (120.0, 76.0),
        &format!("{cab}-Q10"),
    );
    kit.wire_mm2((fl, "1"), (q10, "1"), 1.5);
    kit.wire_mm2((fnn, "1"), (q10, "3"), 1.5);
    kit.describe(&format!("{cab}-Q10"), "Control supply breaker");
    let g1 = kit.put(
        "iec/supplies/power_supply_dc",
        (120.0, 116.0),
        &format!("{cab}-G1"),
    );
    kit.describe(&format!("{cab}-G1"), "24 V DC power supply");
    kit.wire_mm2((q10, "2"), (g1, "L"), 1.5);
    kit.wire_mm2((q10, "4"), (g1, "N"), 1.5);
    let fpe = kit.feed_pe(p.pe, (136.0, 92.0));
    kit.wire_mm2((fpe, "1"), (g1, "PE"), 1.5);
    let f24 = kit.feed_down(p.p24, (120.0, 148.0));
    let f0 = kit.feed_down(p.p0, (128.0, 148.0));
    kit.wire((g1, "L+"), (f24, "1"));
    kit.wire((g1, "M"), (f0, "1"));
    kit.cable(
        &format!("{plant}-W0"),
        &["PE", "N", "L3", "L2", "L1"],
        &supply_wires.iter().rev().copied().collect::<Vec<_>>(),
        mm2,
        12.0,
        "Incoming supply cable",
    )?;
    Ok(())
}

/// What a direct-on-line feeder needs to know.
pub(super) struct Dol<'a> {
    /// Device number: `Q{n}`, `K{n}`, `M{n}`, cable `{plant}-W{n}`.
    pub n: u32,
    /// Left edge on the sheet, mm.
    pub x: f64,
    pub what: &'a str,
    pub kw: f64,
    /// Motor cable length, m.
    pub cable_m: f64,
}

/// A direct-on-line feeder in a column: motor protection breaker `Q{n}`,
/// contactor `K{n}`, motor terminals on `strip`, the motor `M{n}` in `fld`
/// and its screened-free 4 core cable. Draws on the current page.
pub(super) fn dol_feeder(
    kit: &mut Kit<'_, '_>,
    p: &Pots,
    cab: &str,
    fld: &str,
    plant: &str,
    strip: novo_core::DeviceId,
    f: &Dol<'_>,
) -> ModelResult<()> {
    let (x, n_) = (f.x, f.n);
    let ft: Vec<SymbolId> = [p.l1, p.l2, p.l3]
        .into_iter()
        .enumerate()
        .map(|(i, pot)| kit.feed(pot, (x + 8.0 * i as f64, 24.0)))
        .collect();
    let q = kit.put(
        "iec/protection/motor_protection_breaker",
        (x, 48.0),
        &format!("{cab}-Q{n_}"),
    );
    let k = kit.put("iec/contactors/main_3p", (x, 84.0), &format!("{cab}-K{n_}"));
    kit.describe(
        &format!("{cab}-Q{n_}"),
        &format!("{} motor protection", f.what),
    );
    kit.describe(&format!("{cab}-K{n_}"), &format!("{} contactor", f.what));
    for (i, pin) in ["1", "3", "5"].into_iter().enumerate() {
        kit.wire_mm2((ft[i], "1"), (q, pin), 1.5);
    }
    kit.wire3((q, ["2", "4", "6"]), (k, ["1", "3", "5"]), 1.5);
    let fpe = kit.feed_pe(p.pe, (x + 24.0, 24.0));
    let mut ts = Vec::new();
    for (i, (nm, key)) in [("U", TERM), ("V", TERM), ("W", TERM), ("PE", TERM_PE)]
        .into_iter()
        .enumerate()
    {
        ts.push(kit.term(
            strip,
            &format!("{n_}{nm}"),
            key,
            (x + 8.0 * i as f64, 124.0),
        ));
    }
    for (i, pin) in ["2", "4", "6"].into_iter().enumerate() {
        kit.wire_mm2((k, pin), (ts[i], "1"), 1.5);
    }
    kit.wire_mm2((fpe, "1"), (ts[3], "1"), 1.5);
    let m = kit.put("iec/motors/motor_3ph", (x, 160.0), &format!("{fld}-M{n_}"));
    kit.describe(&format!("{fld}-M{n_}"), &format!("{} {} kW", f.what, f.kw));
    kit.props(&format!("{fld}-M{n_}"), &[("power_kw", f.kw)]);
    let mut mw = Vec::new();
    for (i, pin) in ["U1", "V1", "W1"].into_iter().enumerate() {
        mw.push(kit.wire_mm2((ts[i], "2"), (m, pin), 1.5));
    }
    mw.push(kit.wire_mm2((ts[3], "2"), (m, "PE"), 1.5));
    kit.cable(
        &format!("{plant}-W{n_}"),
        &["BK", "BN", "GY", "GNYE"],
        &mw,
        1.5,
        f.cable_m,
        &format!("{} motor cable 4G1.5", f.what),
    )?;
    Ok(())
}

pub(super) const SWITCH_32: Article = Article {
    order: "SD-3P-32A",
    description: "Switch-disconnector 3 pole, 32 A, lockable handle",
    size: (54.0, 90.0, 70.0),
    loss_w: 5.0,
    props: &[("rated_current_a", 32.0), ("poles", 3.0)],
};
pub(super) const SWITCH_63: Article = Article {
    order: "SD-3P-63A",
    description: "Switch-disconnector 3 pole, 63 A, lockable handle",
    size: (72.0, 130.0, 80.0),
    loss_w: 12.0,
    props: &[("rated_current_a", 63.0), ("poles", 3.0)],
};
pub(super) const MCB2: Article = Article {
    order: "MCB-2P-C6",
    description: "Miniature circuit breaker 2 pole, C 6 A, 6 kA",
    size: (36.0, 90.0, 70.0),
    loss_w: 3.0,
    props: &[("rated_current_a", 6.0), ("poles", 2.0)],
};
pub(super) const MPCB_2_5: Article = Article {
    order: "MPCB-1.6-2.5A",
    description: "Motor protection circuit breaker, 1.6 to 2.5 A, with trip contact",
    size: (45.0, 97.0, 97.0),
    loss_w: 3.0,
    props: &[("rated_current_a", 2.5), ("poles", 3.0)],
};
pub(super) const MPCB_4: Article = Article {
    order: "MPCB-2.5-4A",
    description: "Motor protection circuit breaker, 2.5 to 4 A, with trip contact",
    size: (45.0, 97.0, 97.0),
    loss_w: 4.0,
    props: &[("rated_current_a", 4.0), ("poles", 3.0)],
};
pub(super) const MPCB_6: Article = Article {
    order: "MPCB-4-6.3A",
    description: "Motor protection circuit breaker, 4 to 6.3 A, with trip contact",
    size: (45.0, 97.0, 97.0),
    loss_w: 5.0,
    props: &[("rated_current_a", 6.3), ("poles", 3.0)],
};
pub(super) const CONTACTOR_9: Article = Article {
    order: "CONT-3P-9A-24VDC",
    description: "Contactor 3 pole, AC-3 9 A / 4 kW at 400 V, coil 24 V DC",
    size: (45.0, 85.0, 93.0),
    loss_w: 2.5,
    props: &[
        ("ac3_current_a", 9.0),
        ("rated_power_kw", 4.0),
        ("poles", 3.0),
    ],
};
pub(super) const PSU_5: Article = Article {
    order: "PSU-24V-5A",
    description: "Power supply 24 V DC, 5 A, 230 V AC input, DIN rail",
    size: (45.0, 125.0, 120.0),
    loss_w: 12.0,
    props: &[("rated_current_a", 5.0)],
};
pub(super) const PSU_10: Article = Article {
    order: "PSU-24V-10A",
    description: "Power supply 24 V DC, 10 A, 230 V AC input, DIN rail",
    size: (60.0, 125.0, 120.0),
    loss_w: 25.0,
    props: &[("rated_current_a", 10.0)],
};
pub(super) const RELAY: Article = Article {
    order: "RELAY-1CO-24VDC",
    description: "Interface relay 1 changeover, 24 V DC coil",
    size: (15.0, 78.0, 70.0),
    loss_w: 0.5,
    props: &[],
};
pub(super) const TIMER: Article = Article {
    order: "TIMER-ON-24VDC",
    description: "Timing relay, on-delay 0.1 to 30 s, 24 V DC",
    size: (22.0, 78.0, 70.0),
    loss_w: 1.0,
    props: &[],
};
pub(super) const ESTOP: Article = Article {
    order: "ESTOP-22MM-2NC",
    description: "Emergency stop mushroom 22 mm, twist release, 2 NC",
    size: (40.0, 60.0, 60.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const BTN_NO: Article = Article {
    order: "PB-22MM-GN-1NO",
    description: "Push button 22 mm, green, 1 NO",
    size: (40.0, 60.0, 60.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const BTN_NC: Article = Article {
    order: "PB-22MM-RD-1NC",
    description: "Push button 22 mm, red, 1 NC",
    size: (40.0, 60.0, 60.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const SELECTOR: Article = Article {
    order: "SELECTOR-22MM-2POS",
    description: "Selector switch 22 mm, 2 position, 1 NO",
    size: (40.0, 60.0, 60.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const LAMP_GN: Article = Article {
    order: "LAMP-22MM-GN-24V",
    description: "Pilot light 22 mm, green, LED, 24 V",
    size: (30.0, 30.0, 40.0),
    loss_w: 0.1,
    props: &[],
};
pub(super) const LAMP_RD: Article = Article {
    order: "LAMP-22MM-RD-24V",
    description: "Pilot light 22 mm, red, LED, 24 V",
    size: (30.0, 30.0, 40.0),
    loss_w: 0.1,
    props: &[],
};
pub(super) const LIMIT_SWITCH: Article = Article {
    order: "LIMIT-1NC-ROLLER",
    description: "Position switch, roller lever, 1 NC",
    size: (30.0, 60.0, 40.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const PROX_NO: Article = Article {
    order: "PROX-M18-PNP-NO",
    description: "Inductive proximity sensor M18, PNP, 1 NO",
    size: (20.0, 20.0, 70.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const PHOTOEYE: Article = Article {
    order: "PHOTO-RET-PNP-NO",
    description: "Photoelectric sensor, retro-reflective, PNP, 1 NO, 5 m",
    size: (20.0, 50.0, 40.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const CABLE_SUPPLY: Article = Article {
    order: "CABLE-5G6",
    description: "Power cable 5G6 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const CABLE_5G2_5: Article = Article {
    order: "CABLE-5G2.5",
    description: "Power cable 5G2.5 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const CABLE_MOTOR: Article = Article {
    order: "CABLE-4G1.5",
    description: "Motor cable 4G1.5 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const CABLE_SENSOR: Article = Article {
    order: "CABLE-2X0.75",
    description: "Sensor cable 2x0.75 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const CABLE_SENSOR_4: Article = Article {
    order: "CABLE-4X0.75",
    description: "Sensor cable 4x0.75 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};

pub(super) const MOTOR_0_75: Article = Article {
    order: "MOTOR-0.75KW-4P-IE3",
    description: "Three phase induction motor 0.75 kW, 4 pole, IE3",
    size: (150.0, 130.0, 260.0),
    loss_w: 0.0,
    props: &[("rated_current_a", 1.9), ("rated_power_kw", 0.75)],
};
pub(super) const MOTOR_1_1: Article = Article {
    order: "MOTOR-1.1KW-4P-IE3",
    description: "Three phase induction motor 1.1 kW, 4 pole, IE3, geared",
    size: (160.0, 140.0, 330.0),
    loss_w: 0.0,
    props: &[("rated_current_a", 2.6), ("rated_power_kw", 1.1)],
};
pub(super) const MOTOR_2_2: Article = Article {
    order: "MOTOR-2.2KW-4P-IE3",
    description: "Three phase induction motor 2.2 kW, 4 pole, IE3",
    size: (200.0, 160.0, 290.0),
    loss_w: 0.0,
    props: &[("rated_current_a", 4.8), ("rated_power_kw", 2.2)],
};
pub(super) const CABLE_SENSOR_3: Article = Article {
    order: "CABLE-3X0.75",
    description: "Sensor cable 3x0.75 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const LAMP_AMBER: Article = Article {
    order: "LAMP-22MM-AM-24V",
    description: "Pilot light 22 mm, amber, LED, 24 V",
    size: (30.0, 30.0, 40.0),
    loss_w: 0.1,
    props: &[],
};
pub(super) const DRIVE_3KW: Article = Article {
    order: "VFD-3KW-400V",
    description: "Frequency drive 3 kW, 400 V, built-in EMC filter, 24 V control inputs",
    size: (107.0, 245.0, 150.0),
    loss_w: 60.0,
    props: &[("rated_power_kw", 3.0), ("rated_current_a", 7.5)],
};
pub(super) const CABLE_MOTOR_SCREENED: Article = Article {
    order: "CABLE-4G1.5-SCREENED",
    description: "Motor cable 4G1.5 mm², screened, for frequency drives",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const CABLE_10: Article = Article {
    order: "CABLE-10X0.75",
    description: "Control cable 10x0.75 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const MCB1_DC: Article = Article {
    order: "MCB-1P-C2-DC",
    description: "Miniature circuit breaker 1 pole, C 2 A, for 24 V DC branches",
    size: (18.0, 90.0, 70.0),
    loss_w: 1.0,
    props: &[("rated_current_a", 2.0), ("poles", 1.0)],
};
pub(super) const PLC_CPU: Article = Article {
    order: "PLC-CPU-24VDC",
    description: "PLC CPU, 24 V DC, 2 Ethernet ports, fieldbus",
    size: (45.0, 117.0, 75.0),
    loss_w: 8.0,
    props: &[],
};
pub(super) const SAFETY_RELAY: Article = Article {
    order: "SAFETY-RELAY-24VDC",
    description: "Emergency stop relay, 24 V DC, 3 safety contacts, manual reset",
    size: (22.5, 100.0, 120.0),
    loss_w: 1.5,
    props: &[],
};
pub(super) const KEYSWITCH: Article = Article {
    order: "KEYSWITCH-22MM-2POS",
    description: "Key switch 22 mm, 2 position, 1 NO",
    size: (40.0, 60.0, 60.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const FLOAT_NC: Article = Article {
    order: "FLOAT-NC-10M",
    description: "Float switch, break contact, 10 m cable",
    size: (100.0, 100.0, 140.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const PRESSURE_SWITCH: Article = Article {
    order: "PS-0-10BAR-NC",
    description: "Pressure switch, adjustable 1 to 10 bar, 1 NC",
    size: (40.0, 40.0, 80.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const TRANSMITTER: Article = Article {
    order: "TX-420MA-2W",
    description: "Transmitter 4 to 20 mA, 2 wire",
    size: (30.0, 30.0, 90.0),
    loss_w: 0.0,
    props: &[],
};
pub(super) const VALVE: Article = Article {
    order: "VALVE-24VDC-2/2",
    description: "Solenoid valve 2/2 way, 24 V DC",
    size: (40.0, 60.0, 70.0),
    loss_w: 0.0,
    props: &[],
};
