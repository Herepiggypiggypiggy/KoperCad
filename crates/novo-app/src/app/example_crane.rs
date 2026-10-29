//! The crane example: an overhead crane control cabinet with a two-speed
//! hoist (pole-changing motor, spring brake on a rectifier), bridge and crab
//! travel on frequency drives, a 48 V AC control circuit and a radio
//! receiver with two steps per motion. Drawn from generic, public knowledge of
//! how such cabinets are built; it carries no data from any customer's
//! project. Illustrative: ratings and articles without a maker are planning
//! values, not a verified design.

use novo_core::{Document, PotentialKind, Result as ModelResult, Tx};

use super::example_kit::{Article, End, Finish, Kit, Parts, add_page, enclosure, style_strip, tag};

/// What the project is called.
pub(crate) const CRANE_EXAMPLE_NAME: &str = "Overhead crane cabinet (example)";

const DESCRIPTION: &str = "Overhead crane control cabinet: 2-speed hoist with spring brake, \
    bridge and crab travel on frequency drives, 48 V AC control and a radio receiver with two \
    steps per motion, over six pages and a laid-out cabinet. Ratings, parts without a maker and \
    the 400 V / 10 kA supply are planning values to replace with your own.";

const CAB: &str = "=CRN+CAB";
const FLD: &str = "=CRN+FLD";

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
const MPCB_HOIST: Article = Article {
    order: "MPCB-9-14A",
    description: "Motor protection circuit breaker, 9 to 14 A, with trip contact",
    size: (45.0, 97.0, 97.0),
    loss_w: 8.0,
    props: &[("rated_current_a", 14.0), ("poles", 3.0)],
};
const MPCB_DRIVE: Article = Article {
    order: "MPCB-4-6.3A",
    description: "Motor protection circuit breaker, 4 to 6.3 A, with trip contact",
    size: (45.0, 97.0, 97.0),
    loss_w: 5.0,
    props: &[("rated_current_a", 6.3), ("poles", 3.0)],
};
const CONTACTOR: Article = Article {
    order: "CONT-3P-9A-48VAC",
    description: "Contactor 3 pole, AC-3 9 A / 4 kW at 400 V, coil 48 V AC",
    size: (45.0, 85.0, 93.0),
    loss_w: 3.0,
    props: &[
        ("ac3_current_a", 9.0),
        ("rated_power_kw", 4.0),
        ("poles", 3.0),
    ],
};
const CONTACTOR_16: Article = Article {
    order: "CONT-3P-16A-48VAC",
    description: "Contactor 3 pole, AC-3 16 A / 7.5 kW at 400 V, coil 48 V AC",
    size: (45.0, 85.0, 93.0),
    loss_w: 4.0,
    props: &[
        ("ac3_current_a", 16.0),
        ("rated_power_kw", 7.5),
        ("poles", 3.0),
    ],
};
const CONTACTOR_25: Article = Article {
    order: "CONT-3P-25A-48VAC",
    description: "Contactor 3 pole, AC-3 25 A / 11 kW at 400 V, coil 48 V AC (main contactor)",
    size: (45.0, 85.0, 93.0),
    loss_w: 5.5,
    props: &[
        ("ac3_current_a", 25.0),
        ("rated_power_kw", 11.0),
        ("poles", 3.0),
    ],
};
const CONTACTOR_2P: Article = Article {
    order: "CONT-2P-9A-48VAC",
    description: "Contactor 2 pole, AC-3 9 A, coil 48 V AC (brake supply)",
    size: (45.0, 85.0, 93.0),
    loss_w: 2.5,
    props: &[("ac3_current_a", 9.0), ("poles", 2.0)],
};
const RELAY: Article = Article {
    order: "RELAY-2CO-48VAC",
    description: "Interposing relay 2 changeover, coil 48 V AC, with socket",
    size: (15.5, 78.0, 70.0),
    loss_w: 1.0,
    props: &[],
};
const TRANSFORMER: Article = Article {
    order: "TRAFO-400-48-400VA",
    description: "Control transformer 400 V / 48 V, 400 VA",
    size: (120.0, 120.0, 110.0),
    loss_w: 25.0,
    props: &[("rated_power_va", 400.0)],
};
const RECEIVER: Article = Article {
    order: "RADIO-RX-48VAC-14R",
    description: "Radio remote receiver, 48 V AC supply, 14 relay outputs",
    size: (160.0, 130.0, 60.0),
    loss_w: 6.0,
    props: &[],
};
const RECEIVER_MODULE: Article = Article {
    order: "RADIO-OUT-48VAC-6R",
    description: "Radio receiver output module, 6 relay outputs, 48 V AC",
    size: (80.0, 110.0, 60.0),
    loss_w: 3.0,
    props: &[],
};
const DRIVE: Article = Article {
    order: "VFD-3KW-400V",
    description: "Frequency drive 3 kW, 400 V, 3 phase, built-in brake chopper",
    size: (90.0, 190.0, 160.0),
    loss_w: 60.0,
    props: &[("rated_current_a", 6.5), ("rated_power_kw", 3.0)],
};
const RECTIFIER: Article = Article {
    order: "RECT-BRAKE-400V",
    description: "Brake rectifier, 400 V AC to 180 V DC, 1 A",
    size: (25.0, 40.0, 30.0),
    loss_w: 3.0,
    props: &[],
};
const ESTOP: Article = Article {
    order: "ESTOP-22MM-2NC",
    description: "Emergency stop mushroom 22 mm, twist release, 2 NC",
    size: (40.0, 60.0, 60.0),
    loss_w: 0.0,
    props: &[],
};
const LIMIT: Article = Article {
    order: "LIMIT-ROT-1NC",
    description: "Rotary limit switch, 1 NC, for hoist end position",
    size: (60.0, 80.0, 60.0),
    loss_w: 0.0,
    props: &[],
};
const MOTOR_HOIST: Article = Article {
    order: "MOTOR-2SPEED-4/16P",
    description: "Pole-changing motor 4/16 pole, 5.5 / 1.4 kW, with spring brake",
    size: (300.0, 240.0, 460.0),
    loss_w: 0.0,
    props: &[("rated_current_a", 12.0), ("rated_power_kw", 5.5)],
};
const MOTOR_BRIDGE: Article = Article {
    order: "MOTOR-3KW-4P-IE3",
    description: "Three phase induction motor 3 kW, 4 pole, IE3, for drive operation",
    size: (240.0, 190.0, 330.0),
    loss_w: 0.0,
    props: &[("rated_current_a", 6.4), ("rated_power_kw", 3.0)],
};
const BRAKE: Article = Article {
    order: "BRAKE-SPRING-180VDC",
    description: "Spring-applied holding brake, coil 180 V DC",
    size: (150.0, 150.0, 80.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_SUPPLY: Article = Article {
    order: "CABLE-4G6",
    description: "Power cable 4G6 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_HOIST: Article = Article {
    order: "CABLE-7G1.5",
    description: "Motor cable 7G1.5 mm² (two speeds and PE)",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_BRAKE: Article = Article {
    order: "CABLE-2X1.5",
    description: "Brake cable 2x1.5 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_VFD: Article = Article {
    order: "CABLE-4G1.5-SH",
    description: "Screened motor cable 4G1.5 mm² for drives, copper braid screen",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};

/// Draws the example into a project that has only its empty first page.
pub(crate) fn build_crane_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
    let ((), changes) = doc.transact("Open example", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        draw(tx)?;
        super::example_kit::fill_sections(tx, 0.75)?;
        let (plate, enclosure) = enclosure((960.0, 1360.0), (1000.0, 1400.0, 300.0));
        super::example_kit::finish(
            tx,
            &Finish {
                name: CRANE_EXAMPLE_NAME,
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
    let line = |tx: &mut Tx<'_>, name: &str| tx.add_potential(name, PotentialKind::Line);
    let (l1, l2, l3) = (line(tx, "L1")?, line(tx, "L2")?, line(tx, "L3")?);
    // Behind the main contactor.
    let (s1, s2, s3) = (line(tx, "L1.1")?, line(tx, "L2.1")?, line(tx, "L3.1")?);
    let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
    let p48 = tx.add_potential("48V", PotentialKind::Line)?;
    let p0 = tx.add_potential("0V48", PotentialKind::Neutral)?;
    // 48 V behind the main contactor.
    let p48e = tx.add_potential("48V.E", PotentialKind::Line)?;
    // The hoist's motor supply after the reversing pair, and the slow-speed
    // winding feed for the star-point contactor.
    let sig = |tx: &mut Tx<'_>, name: &str| tx.add_potential(name, PotentialKind::Signal);
    let (hu, hv, hw) = (sig(tx, "HU")?, sig(tx, "HV")?, sig(tx, "HW")?);
    let (mu, mv, mw) = (sig(tx, "M1U")?, sig(tx, "M1V")?, sig(tx, "M1W")?);
    let d1 = tx.add_potential("24V.U21", PotentialKind::DcPositive)?;
    let d2 = tx.add_potential("24V.U22", PotentialKind::DcPositive)?;

    let p_supply = add_page(tx, "1", "Supply, main contactor and 48 V", CAB)?;
    let p_hoist = add_page(tx, "2", "Hoist: two-speed motor and brake", CAB)?;
    let p_travel = add_page(tx, "3", "Travel: bridge and crab drives", CAB)?;
    let p_c_hoist = add_page(tx, "4", "Control 48 V: hoist", CAB)?;
    let p_c_travel = add_page(tx, "5", "Control 48 V: travel relays", CAB)?;
    let p_c_drive = add_page(tx, "6", "Drive commands", CAB)?;

    let x0 = tx.add_terminal_strip(tag(&format!("{CAB}-X0")))?;
    let x11 = tx.add_terminal_strip(tag(&format!("{CAB}-X11")))?;
    let x2 = tx.add_terminal_strip(tag(&format!("{CAB}-X2")))?;
    let x3 = tx.add_terminal_strip(tag(&format!("{CAB}-X3")))?;
    let term = "iec/terminals/terminal";
    let term_pe = "iec/terminals/terminal_pe";

    let mut kit = Kit::new(tx, p_supply);

    // ----- page 1: supply, main contactor, control transformer -------------
    let mut supply_wires = Vec::new();
    let mut tb = Vec::new();
    for (i, (name, key)) in [("L1", term), ("L2", term), ("L3", term), ("PE", term_pe)]
        .into_iter()
        .enumerate()
    {
        tb.push(kit.term(x0, name, key, (40.0 + 16.0 * i as f64, 40.0)));
    }
    for (i, pot) in [l1, l2, l3].into_iter().enumerate() {
        let f = kit.feed(pot, (40.0 + 16.0 * i as f64, 12.0));
        supply_wires.push(kit.wire_mm2((f, "1"), (tb[i], "1"), 6.0));
    }
    let f = kit.feed_pe(pe, (88.0, 12.0));
    supply_wires.push(kit.wire_mm2((f, "1"), (tb[3], "1"), 6.0));
    let fpe = kit.feed_down(pe, (88.0, 56.0));
    kit.wire_mm2((tb[3], "2"), (fpe, "1"), 6.0);
    let q0 = kit.put(
        "iec/switches/switch_disconnector_3p",
        (40.0, 66.0),
        &format!("{CAB}-Q0"),
    );
    kit.describe(&format!("{CAB}-Q0"), "Main switch");
    // The terminals sit 16 mm apart, the poles 8 mm: the outer two wires
    // step in at their own level so none cross.
    for (i, pole) in ["1", "3", "5"].into_iter().enumerate() {
        let corners: &[(f64, f64)] = match i {
            1 => &[(56.0, 50.0), (48.0, 50.0)],
            2 => &[(72.0, 54.0), (56.0, 54.0)],
            _ => &[],
        };
        let w = kit.wire_via((tb[i], "2"), (q0, pole), corners);
        kit.set_mm2(w, 6.0);
    }
    let k0 = kit.put(
        "iec/contactors/main_3p",
        (40.0, 106.0),
        &format!("{CAB}-K0"),
    );
    kit.describe(&format!("{CAB}-K0"), "Main contactor");
    kit.wire3((q0, ["2", "4", "6"]), (k0, ["1", "3", "5"]), 6.0);
    for (i, pot) in [s1, s2, s3].into_iter().enumerate() {
        let f = kit.feed_down(pot, (40.0 + 8.0 * i as f64, 134.0));
        kit.wire_mm2((k0, ["2", "4", "6"][i]), (f, "1"), 6.0);
    }
    // Control transformer from L1 and L2 ahead of the main contactor.
    let fa = kit.feed(l1, (120.0, 40.0));
    let fb = kit.feed(l2, (128.0, 40.0));
    let q1 = kit.put("iec/protection/mcb_2p", (120.0, 56.0), &format!("{CAB}-Q1"));
    kit.describe(&format!("{CAB}-Q1"), "Control transformer primary breaker");
    kit.wire_mm2((fa, "1"), (q1, "1"), 1.5);
    kit.wire_mm2((fb, "1"), (q1, "3"), 1.5);
    let t1 = kit.put(
        "iec/transformers/transformer_1ph",
        (120.0, 96.0),
        &format!("{CAB}-T1"),
    );
    kit.describe(&format!("{CAB}-T1"), "Control transformer 400/48 V");
    kit.wire_mm2((q1, "2"), (t1, "1.1"), 1.5);
    kit.wire_mm2((q1, "4"), (t1, "1.2"), 1.5);
    let q2 = kit.put(
        "iec/protection/mcb_2p",
        (120.0, 144.0),
        &format!("{CAB}-Q2"),
    );
    kit.describe(&format!("{CAB}-Q2"), "48 V control breaker");
    kit.wire_mm2((t1, "2.1"), (q2, "1"), 1.5);
    kit.wire_mm2((t1, "2.2"), (q2, "3"), 1.5);
    let f48 = kit.feed_down(p48, (120.0, 180.0));
    let f0 = kit.feed_down(p0, (128.0, 180.0));
    kit.wire_mm2((q2, "2"), (f48, "1"), 1.5);
    kit.wire_mm2((q2, "4"), (f0, "1"), 1.5);
    kit.cable(
        "=CRN-W0",
        &["PE", "L3", "L2", "L1"],
        &supply_wires.iter().rev().copied().collect::<Vec<_>>(),
        6.0,
        30.0,
        "Supply cable 4G6",
    )?;

    // ----- page 2: hoist ---------------------------------------------------
    kit.on(p_hoist);
    let hx = 40.0;
    let feeds: Vec<_> = [s1, s2, s3]
        .into_iter()
        .enumerate()
        .map(|(i, p)| kit.feed(p, (hx + 8.0 * i as f64, 24.0)))
        .collect();
    let q11 = kit.put(
        "iec/protection/motor_protection_breaker",
        (hx, 48.0),
        &format!("{CAB}-Q11"),
    );
    kit.describe(&format!("{CAB}-Q11"), "Hoist motor protection");
    for (i, pin) in ["1", "3", "5"].into_iter().enumerate() {
        kit.wire_mm2((feeds[i], "1"), (q11, pin), 2.5);
    }
    let k11 = kit.put("iec/contactors/main_3p", (hx, 84.0), &format!("{CAB}-K11"));
    let k12 = kit.put(
        "iec/contactors/main_3p",
        (100.0, 84.0),
        &format!("{CAB}-K12"),
    );
    kit.describe(&format!("{CAB}-K11"), "Hoist up");
    kit.describe(&format!("{CAB}-K12"), "Hoist down");
    kit.wire3((q11, ["2", "4", "6"]), (k11, ["1", "3", "5"]), 2.5);
    // Down swaps two phases: L1 to pole 3, L3 to pole 1.
    for (i, (qp, kp)) in [("2", "5"), ("4", "3"), ("6", "1")].into_iter().enumerate() {
        let level = 70.0 + 4.0 * i as f64;
        let ix = [hx, hx + 8.0, hx + 16.0][i];
        let kx = [116.0, 108.0, 100.0][i];
        let w = kit.wire_via((q11, qp), (k12, kp), &[(ix, level), (kx, level)]);
        kit.tx
            .update::<novo_core::Wire>(w, |w| w.cross_section_mm2 = Some(2.5))?;
    }
    for (i, pot) in [hu, hv, hw].into_iter().enumerate() {
        let up = kit.feed_down(pot, (hx + 8.0 * i as f64, 108.0));
        kit.wire_mm2((k11, ["2", "4", "6"][i]), (up, "1"), 2.5);
    }
    for (i, (pot, pin)) in [(hw, "2"), (hv, "4"), (hu, "6")].into_iter().enumerate() {
        let down = kit.feed_down(pot, (100.0 + 8.0 * i as f64, 108.0));
        kit.wire_mm2((k12, pin), (down, "1"), 2.5);
    }
    // Speed contactors: slow (K13) and fast (K14, two phases swapped).
    let k13 = kit.put("iec/contactors/main_3p", (hx, 140.0), &format!("{CAB}-K13"));
    let k14 = kit.put(
        "iec/contactors/main_3p",
        (110.0, 140.0),
        &format!("{CAB}-K14"),
    );
    kit.describe(&format!("{CAB}-K13"), "Hoist slow speed (many poles)");
    kit.describe(&format!("{CAB}-K14"), "Hoist fast speed (few poles)");
    for (i, pot) in [hu, hv, hw].into_iter().enumerate() {
        let f = kit.feed(pot, (hx + 8.0 * i as f64, 124.0));
        kit.wire_mm2((f, "1"), (k13, ["1", "3", "5"][i]), 2.5);
    }
    for (i, pot) in [hw, hv, hu].into_iter().enumerate() {
        let f = kit.feed(pot, (110.0 + 8.0 * i as f64, 124.0));
        kit.wire_mm2((f, "1"), (k14, ["1", "3", "5"][i]), 2.5);
    }
    // Terminals of the hoist motor on X11.
    let mut t1x = Vec::new();
    for (i, n_) in ["1U", "1V", "1W"].into_iter().enumerate() {
        t1x.push(kit.term(x11, n_, term, (hx + 8.0 * i as f64, 176.0)));
    }
    let mut t2x = Vec::new();
    for (i, n_) in ["2W", "2V", "2U"].into_iter().enumerate() {
        t2x.push(kit.term(x11, n_, term, (110.0 + 8.0 * i as f64, 176.0)));
    }
    let t_pe = kit.term(x11, "PE", term_pe, (hx + 24.0, 176.0));
    // Slow outputs reach the terminals through a junction that also feeds the
    // star-point contactor K15 (potentials M1U..M1W).
    let k15 = kit.put(
        "iec/contactors/main_3p",
        (170.0, 140.0),
        &format!("{CAB}-K15"),
    );
    kit.describe(&format!("{CAB}-K15"), "Fast speed star point");
    for (i, pot) in [mu, mv, mw].into_iter().enumerate() {
        let jx = hx + 8.0 * i as f64;
        let j = kit.junction((jx, 164.0));
        kit.link(End::Pin((k13, ["2", "4", "6"][i])), End::Junction(j), &[]);
        kit.link(End::Junction(j), End::Pin((t1x[i], "1")), &[]);
        let f = kit.feed_down(pot, (jx + 4.0, 164.0));
        kit.link(End::Junction(j), End::Pin((f, "1")), &[]);
        let up = kit.feed(pot, (170.0 + 8.0 * i as f64, 124.0));
        kit.wire_mm2((up, "1"), (k15, ["1", "3", "5"][i]), 2.5);
    }
    for (i, pin) in ["2", "4", "6"].into_iter().enumerate() {
        kit.wire_mm2((k14, pin), (t2x[i], "1"), 2.5);
    }
    let star = kit.put("iec/connectivity/bridge_3", (170.0, 164.0), "");
    for (i, pin) in ["2", "4", "6"].into_iter().enumerate() {
        kit.wire_mm2((k15, pin), (star, ["1", "2", "3"][i]), 2.5);
    }
    let fpe = kit.feed_pe(pe, (hx + 24.0, 140.0));
    kit.wire_mm2((fpe, "1"), (t_pe, "1"), 2.5);
    // The motor and its cable.
    let m11 = kit.put(
        "iec/motors/motor_3ph_dahlander",
        (hx, 216.0),
        &format!("{FLD}-M11"),
    );
    kit.describe(
        &format!("{FLD}-M11"),
        "Hoist motor 5.5 / 1.4 kW, two speeds",
    );
    kit.props(&format!("{FLD}-M11"), &[("power_kw", 5.5)]);
    let mut mws = Vec::new();
    for (i, pin) in ["1U", "1V", "1W"].into_iter().enumerate() {
        mws.push(kit.wire_mm2((t1x[i], "2"), (m11, pin), 2.5));
    }
    for (i, (pin, mx, level)) in [
        ("2W", 56.0, 266.0),
        ("2V", 48.0, 270.0),
        ("2U", 40.0, 274.0),
    ]
    .into_iter()
    .enumerate()
    {
        let tx_ = 110.0 + 8.0 * i as f64;
        mws.push(kit.wire_via((t2x[i], "2"), (m11, pin), &[(tx_, level), (mx, level)]));
    }
    mws.push(kit.wire_mm2((t_pe, "2"), (m11, "PE"), 2.5));
    kit.cable(
        "=CRN-W11",
        &["1U", "1V", "1W", "2W", "2V", "2U", "GNYE"],
        &mws,
        2.5,
        20.0,
        "Hoist motor cable 7G2.5",
    )?;
    // Brake: breaker, brake contactor, rectifier, terminals, coil.
    let fa = kit.feed(s1, (210.0, 24.0));
    let fb = kit.feed(s2, (218.0, 24.0));
    let q12 = kit.put(
        "iec/protection/mcb_2p",
        (210.0, 48.0),
        &format!("{CAB}-Q12"),
    );
    kit.describe(&format!("{CAB}-Q12"), "Brake supply breaker");
    kit.wire_mm2((fa, "1"), (q12, "1"), 1.5);
    kit.wire_mm2((fb, "1"), (q12, "3"), 1.5);
    let k16 = kit.put(
        "iec/contactors/main_2p",
        (210.0, 84.0),
        &format!("{CAB}-K16"),
    );
    kit.describe(&format!("{CAB}-K16"), "Brake contactor");
    kit.wire_mm2((q12, "2"), (k16, "1"), 1.5);
    kit.wire_mm2((q12, "4"), (k16, "3"), 1.5);
    let g2 = kit.put(
        "iec/passive/bridge_rectifier",
        (210.0, 112.0),
        &format!("{CAB}-G2"),
    );
    kit.describe(&format!("{CAB}-G2"), "Brake rectifier");
    kit.wire_mm2((k16, "2"), (g2, "~1"), 1.5);
    kit.wire_mm2((k16, "4"), (g2, "~2"), 1.5);
    let tb1 = kit.term(x11, "B1", term, (210.0, 144.0));
    let tb2 = kit.term(x11, "B2", term, (218.0, 144.0));
    kit.wire_mm2((g2, "+"), (tb1, "1"), 1.5);
    kit.wire_mm2((g2, "-"), (tb2, "1"), 1.5);
    let y11 = kit.put("iec/coils/brake", (250.0, 216.0), &format!("{FLD}-Y11"));
    kit.describe(&format!("{FLD}-Y11"), "Hoist holding brake");
    let b1 = kit.wire_mm2((tb1, "2"), (y11, "A1"), 1.5);
    let b2 = kit.wire_via((tb2, "2"), (y11, "A2"), &[(218.0, 248.0), (250.0, 248.0)]);
    kit.cable(
        "=CRN-W12",
        &["B1", "B2"],
        &[b1, b2],
        1.5,
        20.0,
        "Hoist brake cable 2x1.5",
    )?;

    // ----- page 3: travel drives -------------------------------------------
    kit.on(p_travel);
    let drives = [("21", "Bridge", 40.0, 3), ("22", "Crab", 190.0, 3)];
    for (n_, what, x, _) in drives {
        let ft: Vec<_> = [s1, s2, s3]
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
            kit.wire_mm2((ft[i], "1"), (q, pin), 1.5);
        }
        let k = kit.put("iec/contactors/main_3p", (x, 84.0), &format!("{CAB}-K{n_}"));
        kit.describe(
            &format!("{CAB}-K{n_}"),
            &format!("{what} drive line contactor"),
        );
        kit.wire3((q, ["2", "4", "6"]), (k, ["1", "3", "5"]), 1.5);
        let u = kit.put(
            "iec/drives/frequency_converter",
            (x, 120.0),
            &format!("{CAB}-U{n_}"),
        );
        kit.describe(
            &format!("{CAB}-U{n_}"),
            &format!("{what} travel drive 3 kW"),
        );
        kit.wire3((k, ["2", "4", "6"]), (u, ["L1", "L2", "L3"]), 1.5);
        let fpe = kit.feed_pe(pe, (x + 24.0, 100.0));
        kit.wire_mm2((fpe, "1"), (u, "PE"), 1.5);
        let mut ts = Vec::new();
        for (i, (nm, key)) in [("U", term), ("V", term), ("W", term), ("PE", term_pe)]
            .into_iter()
            .enumerate()
        {
            ts.push(kit.term(x2, &format!("{n_}{nm}"), key, (x + 16.0 * i as f64, 168.0)));
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
        kit.describe(
            &format!("{FLD}-M{n_}"),
            &format!("{what} travel motor 3 kW"),
        );
        kit.props(&format!("{FLD}-M{n_}"), &[("power_kw", 3.0)]);
        let mut mw_ = Vec::new();
        let fan_in: [&[(f64, f64)]; 4] = [
            &[],
            &[(x + 16.0, 176.0), (x + 8.0, 176.0)],
            &[(x + 32.0, 180.0), (x + 16.0, 180.0)],
            &[(x + 48.0, 184.0), (x + 24.0, 184.0)],
        ];
        for (i, pin) in ["U1", "V1", "W1", "PE"].into_iter().enumerate() {
            let w = kit.wire_via((ts[i], "2"), (m, pin), fan_in[i]);
            kit.set_mm2(w, 1.5);
            mw_.push(w);
        }
        // The screen is bonded to PE at the cabinet and at the motor.
        let t_sh = kit.term(x2, &format!("{n_}SH"), term_pe, (x + 64.0, 168.0));
        let f_sh = kit.feed_pe(pe, (x + 64.0, 148.0));
        kit.wire_mm2((f_sh, "1"), (t_sh, "1"), 1.5);
        mw_.push(kit.wire_via(
            (t_sh, "2"),
            (m, "PE"),
            &[(x + 64.0, 206.0), (x + 24.0, 206.0)],
        ));
        let c = kit.cable(
            &format!("=CRN-W{n_}"),
            &["U", "V", "W", "GNYE", "SH"],
            &mw_,
            1.5,
            if n_ == "21" { 35.0 } else { 15.0 },
            &format!("{what} motor cable, screened"),
        )?;
        kit.tx.update::<novo_core::Cable>(c, |c| {
            if let Some(shield) = c.cores.last_mut() {
                shield.shield = true;
            }
        })?;
    }

    // ----- page 4: hoist control, 48 V AC ----------------------------------
    kit.on(p_c_hoist);
    let a1 = format!("{CAB}-A1");
    let a2 = format!("{CAB}-A2");
    let a3 = format!("{CAB}-A3");
    let no = "iec/contacts/no";
    let nc = "iec/contacts/nc";
    let coil = "iec/coils/contactor";
    let k = |n_: &str| format!("{CAB}-K{n_}");
    // Receiver supply.
    let mut c = kit.rung(p48, 30.0);
    kit.series(&mut c, "iec/coils/relay", &a1);
    kit.close(&c, p0);
    kit.describe(&a1, "Radio receiver: enable and hoist outputs");
    // Main contactor: the stop chain and the receiver's enable output.
    let mut c = kit.rung(p48, 70.0);
    kit.series(&mut c, "iec/operators/emergency_stop", &format!("{CAB}-S0"));
    kit.series(&mut c, no, &a1);
    kit.series(&mut c, coil, &k("0"));
    kit.close(&c, p0);
    kit.describe(&format!("{CAB}-S0"), "Emergency stop (cabinet)");
    // The enabled 48 V rail: everything below runs only with K0 closed.
    let mut c = kit.rung(p48, 110.0);
    kit.series(&mut c, no, &k("0"));
    let fe = kit.feed_down(p48e, (c.x, c.y));
    kit.link(c.last, End::Pin((fe, "1")), &[]);
    // K11 up: receiver step 1, trip contact, upper limit, K12 locked out.
    let mut c = kit.rung(p48e, 150.0);
    kit.series(&mut c, no, &a1);
    kit.series(&mut c, nc, &format!("{CAB}-Q11"));
    kit.series(&mut c, "iec/sensors/proximity_nc", &format!("{CAB}-S11"));
    kit.series(&mut c, nc, &k("12"));
    kit.series(&mut c, coil, &k("11"));
    kit.close(&c, p0);
    kit.describe(&format!("{CAB}-S11"), "Hoist upper limit");
    // K12 down.
    let mut c = kit.rung(p48e, 190.0);
    kit.series(&mut c, no, &a1);
    kit.series(&mut c, nc, &format!("{CAB}-Q11"));
    kit.series(&mut c, "iec/sensors/proximity_nc", &format!("{CAB}-S12"));
    kit.series(&mut c, nc, &k("11"));
    kit.series(&mut c, coil, &k("12"));
    kit.close(&c, p0);
    kit.describe(&format!("{CAB}-S12"), "Hoist lower limit");
    // K13 slow: while moving and not fast.
    let mut c = kit.rung(p48e, 230.0);
    kit.either(&mut c, no, &k("11"), no, &k("12"));
    kit.series(&mut c, nc, &k("14"));
    kit.series(&mut c, coil, &k("13"));
    kit.close(&c, p0);
    // K14 fast: step 2 up or down, while moving.
    let mut c = kit.rung(p48e, 270.0);
    kit.either(&mut c, no, &a1, no, &a1);
    kit.either(&mut c, no, &k("11"), no, &k("12"));
    kit.series(&mut c, nc, &k("13"));
    kit.series(&mut c, coil, &k("14"));
    kit.close(&c, p0);
    // K15 star point with K14.
    let mut c = kit.rung(p48e, 310.0);
    kit.series(&mut c, no, &k("14"));
    kit.series(&mut c, coil, &k("15"));
    kit.close(&c, p0);
    // K16 brake: while either speed runs.
    let mut c = kit.rung(p48e, 350.0);
    kit.either(&mut c, no, &k("13"), no, &k("14"));
    kit.series(&mut c, coil, &k("16"));
    kit.close(&c, p0);

    // ----- page 5: travel relays and line contactors -----------------------
    kit.on(p_c_travel);
    for (i, (dev, what)) in [(&a2, "bridge"), (&a3, "crab")].into_iter().enumerate() {
        let mut c = kit.rung(p48, 20.0 + 38.0 * i as f64);
        kit.series(&mut c, "iec/coils/relay", dev);
        kit.close(&c, p0);
        kit.describe(dev, &format!("Radio receiver output module: {what} travel"));
    }
    let relays = [
        ("23", "Bridge forward", &a2),
        ("24", "Bridge back", &a2),
        ("25", "Bridge fast", &a2),
        ("26", "Crab forward", &a3),
        ("27", "Crab back", &a3),
        ("28", "Crab fast", &a3),
    ];
    for (i, (n_, what, module)) in relays.into_iter().enumerate() {
        let mut c = kit.rung(p48e, 96.0 + 38.0 * i as f64);
        kit.either(&mut c, no, module, no, module);
        kit.series(&mut c, "iec/coils/relay", &k(n_));
        kit.close(&c, p0);
        kit.describe(&k(n_), what);
    }
    for (i, (n_, a, b)) in [("21", "23", "24"), ("22", "26", "27")]
        .into_iter()
        .enumerate()
    {
        let mut c = kit.rung(p48e, 332.0 + 38.0 * i as f64);
        kit.either(&mut c, no, &k(a), no, &k(b));
        kit.series(&mut c, coil, &k(n_));
        kit.close(&c, p0);
    }

    // ----- page 6: drive commands to X3 ------------------------------------
    kit.on(p_c_drive);
    for (j, (pot, relays)) in [(d1, ["23", "24", "25"]), (d2, ["26", "27", "28"])]
        .into_iter()
        .enumerate()
    {
        for (i, rel) in relays.into_iter().enumerate() {
            let x = 40.0 + 40.0 * (3 * j + i) as f64;
            let mut c = kit.rung(pot, x);
            kit.series(&mut c, no, &k(rel));
            let label = ["FWD", "REV", "FAST"][i];
            let drive = if j == 0 { "U21" } else { "U22" };
            kit.series_term(&mut c, x3, &format!("{drive}.{label}"));
            // On to the drive's own input.
            let input = kit
                .tx
                .add_potential(format!("{drive}:{label}"), PotentialKind::Signal)?;
            let f = kit.feed_down(input, (c.x, c.y));
            kit.link(c.last, End::Pin((f, "1")), &[]);
        }
    }

    // ----- parts -----------------------------------------------------------
    let dev = |kit: &Kit<'_, '_>, t: &str| kit.device(t);
    let mut generic: Vec<(String, &Article)> = vec![
        (format!("{CAB}-Q0"), &SWITCH),
        (format!("{CAB}-Q1"), &MCB2),
        (format!("{CAB}-Q2"), &MCB2),
        (format!("{CAB}-Q12"), &MCB2),
        (format!("{CAB}-Q11"), &MPCB_HOIST),
        (format!("{CAB}-Q21"), &MPCB_DRIVE),
        (format!("{CAB}-Q22"), &MPCB_DRIVE),
        (format!("{CAB}-T1"), &TRANSFORMER),
        (format!("{CAB}-A1"), &RECEIVER),
        (format!("{CAB}-A2"), &RECEIVER_MODULE),
        (format!("{CAB}-A3"), &RECEIVER_MODULE),
        (format!("{CAB}-G2"), &RECTIFIER),
        (format!("{CAB}-S0"), &ESTOP),
        (format!("{CAB}-S11"), &LIMIT),
        (format!("{CAB}-S12"), &LIMIT),
        (format!("{CAB}-K16"), &CONTACTOR_2P),
        (format!("{CAB}-U21"), &DRIVE),
        (format!("{CAB}-U22"), &DRIVE),
        (format!("{FLD}-M11"), &MOTOR_HOIST),
        (format!("{FLD}-M21"), &MOTOR_BRIDGE),
        (format!("{FLD}-M22"), &MOTOR_BRIDGE),
        (format!("{FLD}-Y11"), &BRAKE),
    ];
    generic.push((format!("{CAB}-K0"), &CONTACTOR_25));
    for n_ in ["11", "12", "13", "14", "15"] {
        generic.push((format!("{CAB}-K{n_}"), &CONTACTOR_16));
    }
    for n_ in ["21", "22"] {
        generic.push((format!("{CAB}-K{n_}"), &CONTACTOR));
    }
    for n_ in ["23", "24", "25", "26", "27", "28"] {
        generic.push((format!("{CAB}-K{n_}"), &RELAY));
    }
    for (t, a) in generic {
        let d = dev(&kit, &t);
        parts.generic(kit.tx, d, a)?;
    }
    for (strip, count) in [(x0, 4usize), (x11, 14), (x2, 8), (x3, 6)] {
        style_strip(kit.tx, &mut parts, strip, count)?;
    }
    let cables: Vec<_> = kit
        .tx
        .project()
        .cables()
        .map(|c| (c.id, c.tag.to_string()))
        .collect();
    for (id, t) in cables {
        let article = match t.as_str() {
            "=CRN-W0" => &CABLE_SUPPLY,
            "=CRN-W11" => &CABLE_HOIST,
            "=CRN-W12" => &CABLE_BRAKE,
            _ => &CABLE_VFD,
        };
        let part = parts.generic_part(kit.tx, article)?;
        kit.tx
            .update::<novo_core::Cable>(id, |c| c.part = Some(part))?;
    }
    Ok(())
}
