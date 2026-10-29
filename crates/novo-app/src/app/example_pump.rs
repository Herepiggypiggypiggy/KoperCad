//! The pump station example: two variable-speed pumps, each with its motor
//! protection breaker, line contactor, frequency drive, screened motor cable
//! and motor, a pressure transmitter and a dry-run float switch in the field,
//! 24 V DC control and a laid-out cabinet, over four pages. Illustrative:
//! sizes and ratings are planning values, not a verified design.

use novo_core::{Document, PotentialKind, Result as ModelResult, Tx};

use super::example_kit::{Article, Finish, Kit, Parts, add_page, enclosure, style_strip, tag};

/// What the project is called.
pub(crate) const PUMP_EXAMPLE_NAME: &str = "Pump station (example)";

const DESCRIPTION: &str = "Pump station: 32 A incoming switch, two 2.2 kW pumps on frequency \
    drives with screened motor cables, a pressure transmitter and a dry-run float switch, 24 V DC \
    control, a laid-out cabinet with heat and parts list. Ratings, parts without a maker and the \
    400 V / 10 kA supply are planning values to replace with your own.";

const CAB: &str = "=PMP+CAB";
const FLD: &str = "=PMP+FLD";

const SWITCH: Article = Article {
    order: "SD-3P-32A",
    description: "Switch-disconnector 3 pole, 32 A, lockable handle",
    size: (54.0, 90.0, 70.0),
    loss_w: 5.0,
    props: &[("rated_current_a", 32.0), ("poles", 3.0)],
};
const MCB2: Article = Article {
    order: "MCB-2P-C6",
    description: "Miniature circuit breaker 2 pole, C 6 A, 6 kA",
    size: (36.0, 90.0, 70.0),
    loss_w: 3.0,
    props: &[("rated_current_a", 6.0), ("poles", 2.0)],
};
const DRIVE: Article = Article {
    order: "VFD-3KW-400V",
    description: "Frequency drive 3 kW, 400 V, built-in EMC filter, 24 V control inputs",
    size: (107.0, 245.0, 150.0),
    loss_w: 60.0,
    props: &[("rated_power_kw", 3.0), ("rated_current_a", 7.5)],
};
const RELAY: Article = Article {
    order: "RELAY-1CO-24VDC",
    description: "Interface relay 1 changeover, 24 V DC coil",
    size: (15.0, 78.0, 70.0),
    loss_w: 0.5,
    props: &[],
};
const ESTOP: Article = Article {
    order: "ESTOP-22MM-2NC",
    description: "Emergency stop mushroom 22 mm, twist release, 2 NC",
    size: (40.0, 60.0, 60.0),
    loss_w: 0.0,
    props: &[],
};
const SELECTOR: Article = Article {
    order: "SELECTOR-22MM-2POS",
    description: "Selector switch 22 mm, 2 position, 1 NO",
    size: (40.0, 60.0, 60.0),
    loss_w: 0.0,
    props: &[],
};
const LAMP: Article = Article {
    order: "LAMP-22MM-GN-24V",
    description: "Pilot light 22 mm, green, LED, 24 V",
    size: (30.0, 30.0, 40.0),
    loss_w: 0.1,
    props: &[],
};
const LAMP_RED: Article = Article {
    order: "LAMP-22MM-RD-24V",
    description: "Pilot light 22 mm, red, LED, 24 V",
    size: (30.0, 30.0, 40.0),
    loss_w: 0.1,
    props: &[],
};
const TRANSMITTER: Article = Article {
    order: "PT-0-10BAR-420MA",
    description: "Pressure transmitter 0 to 10 bar, 4 to 20 mA, 2 wire",
    size: (30.0, 30.0, 90.0),
    loss_w: 0.0,
    props: &[],
};
const FLOAT: Article = Article {
    order: "FLOAT-NC-10M",
    description: "Float switch, break contact, 10 m cable",
    size: (100.0, 100.0, 140.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_SUPPLY: Article = Article {
    order: "CABLE-5G10",
    description: "Power cable 5G10 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_MOTOR: Article = Article {
    order: "CABLE-4G1.5-SCREENED",
    description: "Motor cable 4G1.5 mm², screened, for frequency drives",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_SENSOR: Article = Article {
    order: "CABLE-2X0.75",
    description: "Sensor cable 2x0.75 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};

/// Draws the example into a project that has only its empty first page.
pub(crate) fn build_pump_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
    let ((), changes) = doc.transact("Open example", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        draw(tx)?;
        super::example_kit::fill_sections(tx, 0.75)?;
        let (plate, enclosure) = enclosure((760.0, 960.0), (800.0, 1000.0, 300.0));
        super::example_kit::finish(
            tx,
            &Finish {
                name: PUMP_EXAMPLE_NAME,
                description: DESCRIPTION,
                cabinet: "+CAB",
                plate,
                enclosure,
                supply_v: 400.0,
                short_circuit_ka: 10.0,
            },
        )
    })?;
    Ok(changes)
}

fn draw(tx: &mut Tx<'_>) -> ModelResult<()> {
    let mut parts = Parts::default();
    let l1 = tx.add_potential("L1", PotentialKind::Line)?;
    let l2 = tx.add_potential("L2", PotentialKind::Line)?;
    let l3 = tx.add_potential("L3", PotentialKind::Line)?;
    let n = tx.add_potential("N", PotentialKind::Neutral)?;
    let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
    let p24 = tx.add_potential("+24V", PotentialKind::DcPositive)?;
    let p0 = tx.add_potential("0V", PotentialKind::DcNegative)?;
    // 24 V that is there while the float switch says the level is sufficient.
    let p24l = tx.add_potential("+24V.LVL", PotentialKind::DcPositive)?;

    let incoming = add_page(tx, "1", "Incoming supply and 24 V", CAB)?;
    let drives_page = add_page(tx, "2", "Pump drives", CAB)?;
    let sensors = add_page(tx, "3", "Pressure and level sensors", CAB)?;
    let control = add_page(tx, "4", "Control: pumps and drive commands", CAB)?;
    let faults = add_page(tx, "5", "Drive fault signals", CAB)?;

    let x0 = tx.add_terminal_strip(tag(&format!("{CAB}-X0")))?;
    let x1 = tx.add_terminal_strip(tag(&format!("{CAB}-X1")))?;
    let x2 = tx.add_terminal_strip(tag(&format!("{CAB}-X2")))?;
    let term = "iec/terminals/terminal";
    let term_pe = "iec/terminals/terminal_pe";
    let term_n = "iec/terminals/terminal_n";

    let mut kit = Kit::new(tx, incoming);

    // ----- page 1: incoming switch and 24 V supply ------------------------
    let mut supply_wires = Vec::new();
    let mut tb = Vec::new();
    for (i, (name, key)) in [
        ("L1", term),
        ("L2", term),
        ("L3", term),
        ("N", term_n),
        ("PE", term_pe),
    ]
    .into_iter()
    .enumerate()
    {
        tb.push(kit.term(x0, name, key, (40.0 + 16.0 * i as f64, 40.0)));
    }
    // The supply comes in from the left of the sheet: feeds on top of the
    // incoming terminals, N and PE continue to feeds under them.
    for (i, pot) in [l1, l2, l3, n].into_iter().enumerate() {
        let f = kit.feed(pot, (40.0 + 16.0 * i as f64, 12.0));
        supply_wires.push(kit.wire_mm2((f, "1"), (tb[i], "1"), 10.0));
    }
    let f = kit.feed_pe(pe, (104.0, 12.0));
    supply_wires.push(kit.wire_mm2((f, "1"), (tb[4], "1"), 10.0));
    let fnn_down = kit.feed_down(n, (88.0, 56.0));
    kit.wire_mm2((tb[3], "2"), (fnn_down, "1"), 10.0);
    let fpe_down = kit.feed_down(pe, (104.0, 56.0));
    kit.wire_mm2((tb[4], "2"), (fpe_down, "1"), 10.0);
    let q0 = kit.put(
        "iec/switches/switch_disconnector_3p",
        (40.0, 66.0),
        &format!("{CAB}-Q0"),
    );
    for (i, t) in tb.iter().take(3).enumerate() {
        // The terminals sit 16 mm apart, the poles 8 mm: the outer two
        // wires step in at their own level so none cross.
        let pole = ["1", "3", "5"][i];
        let corners: &[(f64, f64)] = match i {
            1 => &[(56.0, 50.0), (48.0, 50.0)],
            2 => &[(72.0, 54.0), (56.0, 54.0)],
            _ => &[],
        };
        let w = kit.wire_via((*t, "2"), (q0, pole), corners);
        kit.set_mm2(w, 10.0);
    }
    kit.describe(&format!("{CAB}-Q0"), "Main switch");
    let feed_l = [l1, l2, l3];
    for (i, pot) in feed_l.into_iter().enumerate() {
        let pin = ["2", "4", "6"][i];
        let f = kit.feed_down(pot, (40.0 + 8.0 * i as f64, 94.0));
        kit.wire_mm2((q0, pin), (f, "1"), 10.0);
    }
    // Control supply: 2 pole breaker, then the 24 V DC power supply.
    let fl = kit.feed(l1, (120.0, 56.0));
    let fnn = kit.feed(n, (128.0, 56.0));
    let q10 = kit.put(
        "iec/protection/mcb_2p",
        (120.0, 76.0),
        &format!("{CAB}-Q10"),
    );
    kit.wire_mm2((fl, "1"), (q10, "1"), 1.5);
    kit.wire_mm2((fnn, "1"), (q10, "3"), 1.5);
    kit.describe(&format!("{CAB}-Q10"), "Control supply breaker");
    let g1 = kit.put(
        "iec/supplies/power_supply_dc",
        (120.0, 116.0),
        &format!("{CAB}-G1"),
    );
    kit.describe(&format!("{CAB}-G1"), "24 V DC power supply");
    kit.wire_mm2((q10, "2"), (g1, "L"), 1.5);
    kit.wire_mm2((q10, "4"), (g1, "N"), 1.5);
    let fpe = kit.feed_pe(pe, (136.0, 92.0));
    kit.wire_mm2((fpe, "1"), (g1, "PE"), 1.5);
    let f24 = kit.feed_down(p24, (120.0, 148.0));
    let f0 = kit.feed_down(p0, (128.0, 148.0));
    kit.wire((g1, "L+"), (f24, "1"));
    kit.wire((g1, "M"), (f0, "1"));
    kit.cable(
        "=PMP-W0",
        &["PE", "N", "L3", "L2", "L1"],
        &supply_wires.iter().rev().copied().collect::<Vec<_>>(),
        10.0,
        25.0,
        "Incoming supply cable 5G10",
    )?;

    // ----- page 2: two pump drives ------------------------------------------
    kit.on(drives_page);
    for (n_, what, x) in [("1", "Pump 1", 40.0), ("2", "Pump 2", 190.0)] {
        let ft: Vec<_> = [l1, l2, l3]
            .into_iter()
            .enumerate()
            .map(|(i, p)| kit.feed(p, (x + 8.0 * i as f64, 24.0)))
            .collect();
        let q = kit.put(
            "iec/protection/motor_protection_breaker",
            (x, 48.0),
            &format!("{CAB}-Q{n_}"),
        );
        kit.describe(&format!("{CAB}-Q{n_}"), &format!("{what} drive protection"));
        for (i, pin) in ["1", "3", "5"].into_iter().enumerate() {
            kit.wire_mm2((ft[i], "1"), (q, pin), 2.5);
        }
        let k = kit.put("iec/contactors/main_3p", (x, 84.0), &format!("{CAB}-K{n_}"));
        kit.describe(&format!("{CAB}-K{n_}"), &format!("{what} line contactor"));
        kit.wire3((q, ["2", "4", "6"]), (k, ["1", "3", "5"]), 2.5);
        let u = kit.put(
            "iec/drives/frequency_converter",
            (x, 120.0),
            &format!("{CAB}-U{n_}"),
        );
        kit.describe(
            &format!("{CAB}-U{n_}"),
            &format!("{what} frequency drive 3 kW"),
        );
        kit.wire3((k, ["2", "4", "6"]), (u, ["L1", "L2", "L3"]), 2.5);
        let fpe = kit.feed_pe(pe, (x + 24.0, 100.0));
        kit.wire_mm2((fpe, "1"), (u, "PE"), 2.5);
        let mut ts = Vec::new();
        for (i, (nm, key)) in [("U", term), ("V", term), ("W", term), ("PE", term_pe)]
            .into_iter()
            .enumerate()
        {
            ts.push(kit.term(x1, &format!("{n_}{nm}"), key, (x + 16.0 * i as f64, 168.0)));
        }
        // The drive pins are 8 mm apart, the terminals 16 mm: each wire steps
        // out at its own level so none cross.
        let fan_out: [&[(f64, f64)]; 3] = [
            &[],
            &[(x + 8.0, 150.0), (x + 16.0, 150.0)],
            &[(x + 16.0, 146.0), (x + 32.0, 146.0)],
        ];
        for (i, pin) in ["U", "V", "W"].into_iter().enumerate() {
            let w = kit.wire_via((u, pin), (ts[i], "1"), fan_out[i]);
            kit.set_mm2(w, 1.5);
        }
        let w = kit.wire_via(
            (u, "PE2"),
            (ts[3], "1"),
            &[(x + 24.0, 142.0), (x + 48.0, 142.0)],
        );
        kit.set_mm2(w, 1.5);
        let m = kit.put("iec/motors/motor_3ph", (x, 224.0), &format!("{FLD}-M{n_}"));
        kit.describe(&format!("{FLD}-M{n_}"), &format!("{what} 2.2 kW"));
        kit.props(&format!("{FLD}-M{n_}"), &[("power_kw", 2.2)]);
        let mut mw = Vec::new();
        let fan_in: [&[(f64, f64)]; 4] = [
            &[],
            &[(x + 16.0, 176.0), (x + 8.0, 176.0)],
            &[(x + 32.0, 180.0), (x + 16.0, 180.0)],
            &[(x + 48.0, 184.0), (x + 24.0, 184.0)],
        ];
        for (i, pin) in ["U1", "V1", "W1", "PE"].into_iter().enumerate() {
            let w = kit.wire_via((ts[i], "2"), (m, pin), fan_in[i]);
            kit.set_mm2(w, 1.5);
            mw.push(w);
        }
        // The screen is bonded to PE at the cabinet and at the motor.
        let t_sh = kit.term(x1, &format!("{n_}SH"), term_pe, (x + 64.0, 168.0));
        let f_sh = kit.feed_pe(pe, (x + 64.0, 148.0));
        kit.wire_mm2((f_sh, "1"), (t_sh, "1"), 1.5);
        mw.push(kit.wire_via(
            (t_sh, "2"),
            (m, "PE"),
            &[(x + 64.0, 206.0), (x + 24.0, 206.0)],
        ));
        let c = kit.cable(
            &format!("=PMP-W{n_}"),
            &["U", "V", "W", "GNYE", "SH"],
            &mw,
            1.5,
            if n_ == "1" { 22.0 } else { 28.0 },
            &format!("{what} motor cable, screened"),
        )?;
        kit.tx.update::<novo_core::Cable>(c, |c| {
            if let Some(shield) = c.cores.last_mut() {
                shield.shield = true;
            }
        })?;
    }

    // ----- page 3: pressure transmitter and float switch ---------------------
    kit.on(sensors);
    // Pressure transmitter: 24 V loop through the terminals, the loop
    // return is the analogue input of drive 1 (the duty pump follows it).
    let ai = kit.tx.add_potential("U1:AI1", PotentialKind::Signal)?;
    let fa = kit.feed(p24, (40.0, 24.0));
    let fb = kit.feed(ai, (56.0, 24.0));
    let ta = kit.term(x2, "B1+", term, (40.0, 56.0));
    let tb_ = kit.term(x2, "B1-", term, (56.0, 56.0));
    kit.wire_mm2((fa, "1"), (ta, "1"), 0.75);
    kit.wire_mm2((fb, "1"), (tb_, "1"), 0.75);
    let b1 = kit.put(
        "iec/sensors/transmitter",
        (40.0, 104.0),
        &format!("{FLD}-B1"),
    );
    kit.describe(&format!("{FLD}-B1"), "Discharge pressure transmitter");
    let w_plus = kit.wire_mm2((ta, "2"), (b1, "+"), 0.75);
    let w_minus = kit.wire_via((tb_, "2"), (b1, "-"), &[(56.0, 90.0), (48.0, 90.0)]);
    kit.set_mm2(w_minus, 0.75);
    kit.cable(
        "=PMP-W3",
        &["BN", "BU"],
        &[w_plus, w_minus],
        0.75,
        15.0,
        "Pressure transmitter cable 2x0.75",
    )?;
    // Float switch: closed while the level is sufficient, so its output is
    // the 24 V that releases the pumps.
    let fc = kit.feed(p24, (140.0, 24.0));
    let tc = kit.term(x2, "B2.1", term, (140.0, 56.0));
    kit.wire_mm2((fc, "1"), (tc, "1"), 0.75);
    let b2 = kit.put("iec/sensors/float_nc", (140.0, 92.0), &format!("{FLD}-B2"));
    kit.describe(&format!("{FLD}-B2"), "Dry-run protection float switch");
    let td = kit.term(x2, "B2.2", term, (140.0, 130.0));
    let w1 = kit.wire_mm2((tc, "2"), (b2, "21"), 0.75);
    let w2 = kit.wire_mm2((b2, "22"), (td, "1"), 0.75);
    let flv = kit.feed_down(p24l, (140.0, 166.0));
    kit.wire_mm2((td, "2"), (flv, "1"), 0.75);
    kit.cable(
        "=PMP-W4",
        &["BN", "BU"],
        &[w1, w2],
        0.75,
        10.0,
        "Float switch cable 2x0.75",
    )?;

    // ----- page 4: pump control and drive commands ---------------------------
    kit.on(control);
    for (idx, n_) in ["1", "2"].into_iter().enumerate() {
        let x0c = 30.0 + 190.0 * idx as f64;
        // Line contactor: emergency stop, protection breaker, coil.
        let mut c = kit.rung(p24, x0c);
        if idx == 0 {
            kit.series(&mut c, "iec/operators/emergency_stop", &format!("{CAB}-S0"));
        } else {
            kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-S0"));
        }
        kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-Q{n_}"));
        kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K{n_}"));
        kit.close(&c, p0);
        // Run request: level sufficient, selector on, line contactor in.
        let mut c = kit.rung(p24l, x0c + 45.0);
        kit.series(
            &mut c,
            "iec/operators/selector_no",
            &format!("{CAB}-S{n_}1"),
        );
        kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K{n_}"));
        kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K{n_}1"));
        kit.close(&c, p0);
        // Run command to the drive input.
        let run = kit
            .tx
            .add_potential(format!("U{n_}:DI1"), PotentialKind::Signal)?;
        let mut c = kit.rung(p24, x0c + 90.0);
        kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K{n_}1"));
        kit.close(&c, run);
        // Running lamp.
        let mut c = kit.rung(p24, x0c + 130.0);
        kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K{n_}1"));
        kit.series(&mut c, "iec/signalling/lamp_green", &format!("{CAB}-H{n_}"));
        kit.close(&c, p0);
        kit.describe(&format!("{CAB}-S{n_}1"), &format!("Pump {n_} on"));
        kit.describe(&format!("{CAB}-K{n_}1"), &format!("Pump {n_} run relay"));
        kit.describe(&format!("{CAB}-H{n_}"), &format!("Pump {n_} running"));
    }
    kit.describe(&format!("{CAB}-S0"), "Emergency stop");

    // ----- page 5: drive fault signals ---------------------------------------
    kit.on(faults);
    for (idx, n_) in ["1", "2"].into_iter().enumerate() {
        let x = 40.0 + 120.0 * idx as f64;
        // The drive's fault relay output picks up a relay in the cabinet.
        let flt = kit
            .tx
            .add_potential(format!("U{n_}:FLT"), PotentialKind::Signal)?;
        let mut c = kit.rung(flt, x);
        kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K{n_}2"));
        kit.close(&c, p0);
        let mut c = kit.rung(p24, x + 50.0);
        kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K{n_}2"));
        kit.series(&mut c, "iec/signalling/lamp_red", &format!("{CAB}-H{n_}1"));
        kit.close(&c, p0);
        kit.describe(&format!("{CAB}-K{n_}2"), &format!("Drive {n_} fault relay"));
        kit.describe(&format!("{CAB}-H{n_}1"), &format!("Pump {n_} drive fault"));
    }

    // ----- parts -------------------------------------------------------------
    let dev = |kit: &Kit<'_, '_>, t: &str| kit.device(t);
    let generic: [(&str, &Article); 17] = [
        ("=PMP+CAB-Q0", &SWITCH),
        ("=PMP+CAB-Q10", &MCB2),
        ("=PMP+CAB-U1", &DRIVE),
        ("=PMP+CAB-U2", &DRIVE),
        ("=PMP+CAB-K11", &RELAY),
        ("=PMP+CAB-K21", &RELAY),
        ("=PMP+CAB-K12", &RELAY),
        ("=PMP+CAB-K22", &RELAY),
        ("=PMP+CAB-H11", &LAMP_RED),
        ("=PMP+CAB-H21", &LAMP_RED),
        ("=PMP+CAB-S0", &ESTOP),
        ("=PMP+CAB-S11", &SELECTOR),
        ("=PMP+CAB-S21", &SELECTOR),
        ("=PMP+CAB-H1", &LAMP),
        ("=PMP+CAB-H2", &LAMP),
        ("=PMP+FLD-B1", &TRANSMITTER),
        ("=PMP+FLD-B2", &FLOAT),
    ];
    for (t, a) in generic {
        let d = dev(&kit, t);
        parts.generic(kit.tx, d, a)?;
    }
    for n_ in ["1", "2"] {
        let q = dev(&kit, &format!("{CAB}-Q{n_}"));
        parts.catalog(kit.tx, q, "Siemens", "3RV2011-1FA10")?;
        let k = dev(&kit, &format!("{CAB}-K{n_}"));
        parts.catalog(kit.tx, k, "Siemens", "3RT2016-1BB41")?;
        let m = dev(&kit, &format!("{FLD}-M{n_}"));
        parts.catalog(kit.tx, m, "Siemens", "1LE1003-0EB42-2AA4")?;
    }
    let g1 = dev(&kit, &format!("{CAB}-G1"));
    parts.catalog(kit.tx, g1, "Phoenix Contact", "2904601")?;
    // Terminal strips: blocks, footprints.
    for (strip, count) in [(x0, 5usize), (x1, 10), (x2, 4)] {
        style_strip(kit.tx, &mut parts, strip, count)?;
    }
    // Cables: parts for the supply, the motor and the sensor cables.
    let cables: Vec<_> = kit
        .tx
        .project()
        .cables()
        .map(|c| (c.id, c.tag.to_string()))
        .collect();
    for (id, t) in cables {
        let part = match t.as_str() {
            "=PMP-W0" => parts.generic_part(kit.tx, &CABLE_SUPPLY)?,
            "=PMP-W1" | "=PMP-W2" => parts.generic_part(kit.tx, &CABLE_MOTOR)?,
            _ => parts.generic_part(kit.tx, &CABLE_SENSOR)?,
        };
        kit.tx
            .update::<novo_core::Cable>(id, |c| c.part = Some(part))?;
    }
    Ok(())
}
