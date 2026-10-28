//! The motor control centre example: a 400 V incoming switch, three
//! direct-on-line feeders and one star-delta feeder, each with its motor
//! protection breaker, contactor, terminals, motor cable and 24 V DC control
//! circuit, over six pages and a laid-out cabinet. Illustrative: sizes and
//! ratings are planning values, not a verified design.

use novo_core::{Document, PotentialKind, Result as ModelResult, Tx};

use super::example_kit::{Article, End, Finish, Kit, Parts, add_page, enclosure, style_strip, tag};

/// What the project is called.
pub(crate) const MCC_EXAMPLE_NAME: &str = "Motor control centre (example)";

const DESCRIPTION: &str = "Motor control centre: 63 A incoming switch, three direct-on-line \
    feeders (pump, fan, compressor) and one star-delta feeder (11 kW), 24 V DC control, a laid-out \
    cabinet with heat and parts list. Ratings, parts without a maker and the 400 V / 10 kA supply \
    are planning values to replace with your own.";

const CAB: &str = "=MCC+CAB";
const FLD: &str = "=MCC+FLD";

const SWITCH: Article = Article {
    order: "SD-3P-63A",
    description: "Switch-disconnector 3 pole, 63 A, lockable handle",
    size: (54.0, 90.0, 70.0),
    loss_w: 9.0,
    props: &[("rated_current_a", 63.0), ("poles", 3.0)],
};
const MCB2: Article = Article {
    order: "MCB-2P-C6",
    description: "Miniature circuit breaker 2 pole, C 6 A, 6 kA",
    size: (36.0, 90.0, 70.0),
    loss_w: 3.0,
    props: &[("rated_current_a", 6.0), ("poles", 2.0)],
};
const MPCB_25: Article = Article {
    order: "MPCB-20-25A",
    description: "Motor protection circuit breaker, 20 to 25 A",
    size: (55.0, 97.0, 97.0),
    loss_w: 12.0,
    props: &[("rated_current_a", 25.0), ("poles", 3.0)],
};
const CONTACTOR_25: Article = Article {
    order: "CONT-3P-25A-24VDC",
    description: "Contactor 3 pole, AC-3 25 A / 11 kW at 400 V, coil 24 V DC",
    size: (45.0, 85.0, 93.0),
    loss_w: 5.5,
    props: &[
        ("ac3_current_a", 25.0),
        ("rated_power_kw", 11.0),
        ("poles", 3.0),
    ],
};
const TIMER: Article = Article {
    order: "TIMER-ON-24VDC",
    description: "Time relay, on-delay 0.5 to 30 s, 24 V DC, 1 changeover",
    size: (22.5, 78.0, 100.0),
    loss_w: 1.0,
    props: &[],
};
const ESTOP: Article = Article {
    order: "ESTOP-22MM-2NC",
    description: "Emergency stop mushroom 22 mm, twist release, 2 NC",
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
const MOTOR_4KW: Article = Article {
    order: "MOTOR-4KW-4P-IE3",
    description: "Three phase induction motor 4 kW, 4 pole, IE3, 400 V",
    size: (200.0, 160.0, 280.0),
    loss_w: 0.0,
    props: &[("rated_current_a", 8.5), ("rated_power_kw", 4.0)],
};
const MOTOR_11KW: Article = Article {
    order: "MOTOR-11KW-4P-IE3",
    description: "Three phase induction motor 11 kW, 4 pole, IE3, 400/690 V, 6 terminals",
    size: (260.0, 220.0, 400.0),
    loss_w: 0.0,
    props: &[("rated_current_a", 21.5), ("rated_power_kw", 11.0)],
};
const CABLE_SUPPLY: Article = Article {
    order: "CABLE-5G16",
    description: "Power cable 5G16 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_7G25: Article = Article {
    order: "CABLE-7G6",
    description: "Motor cable 7G6 mm² (6 cores and PE for star-delta)",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};

/// Draws the example into a project that has only its empty first page.
pub(crate) fn build_mcc_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
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
                name: MCC_EXAMPLE_NAME,
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

    let incoming = add_page(tx, "1", "Incoming supply and 24 V", CAB)?;
    let feeders = add_page(tx, "2", "Direct-on-line feeders", CAB)?;
    let stardelta = add_page(tx, "3", "Star-delta feeder", CAB)?;
    let control_dol = add_page(tx, "4", "Control: direct-on-line feeders", CAB)?;
    let control_sd = add_page(tx, "5", "Control: star-delta feeder", CAB)?;

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
        tb.push(kit.term(x0, name, key, (40.0 + 8.0 * i as f64, 30.0)));
    }
    // The supply comes in from the left of the sheet: feeds on top of the
    // incoming terminals, N and PE continue to feeds under them.
    for (i, pot) in [l1, l2, l3, n].into_iter().enumerate() {
        let f = kit.feed(pot, (40.0 + 8.0 * i as f64, 14.0));
        supply_wires.push(kit.wire_mm2((f, "1"), (tb[i], "1"), 16.0));
    }
    let f = kit.feed_pe(pe, (72.0, 14.0));
    supply_wires.push(kit.wire_mm2((f, "1"), (tb[4], "1"), 16.0));
    let fnn_down = kit.feed_down(n, (64.0, 46.0));
    kit.wire_mm2((tb[3], "2"), (fnn_down, "1"), 16.0);
    let fpe_down = kit.feed_down(pe, (72.0, 46.0));
    kit.wire_mm2((tb[4], "2"), (fpe_down, "1"), 16.0);
    let q0 = kit.put(
        "iec/switches/switch_disconnector_3p",
        (40.0, 56.0),
        &format!("{CAB}-Q0"),
    );
    for (i, t) in tb.iter().take(3).enumerate() {
        let pole = ["1", "3", "5"][i];
        kit.wire_mm2((*t, "2"), (q0, pole), 16.0);
    }
    kit.describe(&format!("{CAB}-Q0"), "Main switch");
    let feed_l = [l1, l2, l3];
    for (i, pot) in feed_l.into_iter().enumerate() {
        let pin = ["2", "4", "6"][i];
        let f = kit.feed_down(pot, (40.0 + 8.0 * i as f64, 84.0));
        kit.wire_mm2((q0, pin), (f, "1"), 16.0);
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
        "=MCC-W0",
        &["L1", "L2", "L3", "N", "PE"],
        &supply_wires,
        16.0,
        25.0,
        "Incoming supply cable",
    )?;

    // ----- page 2: three direct-on-line feeders ---------------------------
    kit.on(feeders);
    struct Dol {
        n: u32,
        x: f64,
        mpcb: &'static str,
        motor: &'static str,
        motor_article: Option<&'static Article>,
        what: &'static str,
        kw: f64,
    }
    let dols = [
        Dol {
            n: 1,
            x: 40.0,
            mpcb: "3RV2011-1JA10",
            motor: "",
            motor_article: Some(&MOTOR_4KW),
            what: "Feed pump",
            kw: 4.0,
        },
        Dol {
            n: 2,
            x: 150.0,
            mpcb: "3RV2011-1FA10",
            motor: "1LE1003-0EB42-2AA4",
            motor_article: None,
            what: "Cooling fan",
            kw: 2.2,
        },
        Dol {
            n: 3,
            x: 260.0,
            mpcb: "3RV2011-1JA10",
            motor: "",
            motor_article: Some(&MOTOR_4KW),
            what: "Compressor",
            kw: 4.0,
        },
    ];
    let mut motor_ties = Vec::new();
    for d in &dols {
        let x = d.x;
        let n_ = d.n;
        let fs = [l1, l2, l3].map(|p| p);
        let mut ft = Vec::new();
        for (i, p) in fs.into_iter().enumerate() {
            ft.push(kit.feed(p, (x + 8.0 * i as f64, 24.0)));
        }
        let q = kit.put(
            "iec/protection/motor_protection_breaker",
            (x, 48.0),
            &format!("{CAB}-Q{n_}"),
        );
        let k = kit.put("iec/contactors/main_3p", (x, 84.0), &format!("{CAB}-K{n_}"));
        kit.describe(
            &format!("{CAB}-Q{n_}"),
            &format!("{} motor protection", d.what),
        );
        kit.describe(&format!("{CAB}-K{n_}"), &format!("{} contactor", d.what));
        for (i, pin) in ["1", "3", "5"].into_iter().enumerate() {
            kit.wire_mm2((ft[i], "1"), (q, pin), 2.5);
        }
        kit.wire3((q, ["2", "4", "6"]), (k, ["1", "3", "5"]), 2.5);
        let fpe = kit.feed_pe(pe, (x + 24.0, 24.0));
        // Terminals U V W PE of this motor on strip X1.
        let mut ts = Vec::new();
        for (i, (nm, key)) in [("U", term), ("V", term), ("W", term), ("PE", term_pe)]
            .into_iter()
            .enumerate()
        {
            ts.push(kit.term(x1, &format!("{n_}{nm}"), key, (x + 8.0 * i as f64, 124.0)));
        }
        for (i, pin) in ["2", "4", "6"].into_iter().enumerate() {
            kit.wire_mm2((k, pin), (ts[i], "1"), 2.5);
        }
        kit.wire_mm2((fpe, "1"), (ts[3], "1"), 2.5);
        let m = kit.put("iec/motors/motor_3ph", (x, 160.0), &format!("{FLD}-M{n_}"));
        kit.describe(&format!("{FLD}-M{n_}"), &format!("{} {} kW", d.what, d.kw));
        kit.props(&format!("{FLD}-M{n_}"), &[("power_kw", d.kw)]);
        let mut mw = Vec::new();
        for (i, pin) in ["U1", "V1", "W1"].into_iter().enumerate() {
            mw.push(kit.wire_mm2((ts[i], "2"), (m, pin), 1.5));
        }
        mw.push(kit.wire_mm2((ts[3], "2"), (m, "PE"), 1.5));
        kit.cable(
            &format!("=MCC-W{n_}"),
            &["BK", "BN", "GY", "GNYE"],
            &mw,
            1.5,
            12.0 + 4.0 * f64::from(n_),
            &format!("{} motor cable 4G1.5", d.what),
        )?;
        motor_ties.push((n_, d.mpcb, d.motor, d.motor_article));
    }

    // ----- page 3: star-delta feeder --------------------------------------
    kit.on(stardelta);
    let x = 40.0;
    let mut ft = Vec::new();
    for (i, p) in [l1, l2, l3].into_iter().enumerate() {
        ft.push(kit.feed(p, (x + 8.0 * i as f64, 24.0)));
    }
    let q4 = kit.put(
        "iec/protection/motor_protection_breaker",
        (x, 48.0),
        &format!("{CAB}-Q4"),
    );
    kit.describe(&format!("{CAB}-Q4"), "Star-delta motor protection");
    for (i, pin) in ["1", "3", "5"].into_iter().enumerate() {
        kit.wire_mm2((ft[i], "1"), (q4, pin), 6.0);
    }
    let k4 = kit.put("iec/contactors/main_3p", (x, 84.0), &format!("{CAB}-K4"));
    let k6 = kit.put(
        "iec/contactors/main_3p",
        (100.0, 84.0),
        &format!("{CAB}-K6"),
    );
    let k5 = kit.put(
        "iec/contactors/main_3p",
        (160.0, 84.0),
        &format!("{CAB}-K5"),
    );
    kit.describe(&format!("{CAB}-K4"), "Mains contactor");
    kit.describe(&format!("{CAB}-K6"), "Delta contactor");
    kit.describe(&format!("{CAB}-K5"), "Star contactor");
    kit.wire3((q4, ["2", "4", "6"]), (k4, ["1", "3", "5"]), 6.0);
    for (i, (qp, kp)) in [("2", "1"), ("4", "3"), ("6", "5")].into_iter().enumerate() {
        let level = 70.0 + 4.0 * i as f64;
        let w = kit.wire_via(
            (q4, qp),
            (k6, kp),
            &[(x + 8.0 * i as f64, level), (100.0 + 8.0 * i as f64, level)],
        );
        kit.tx
            .update::<novo_core::Wire>(w, |w| w.cross_section_mm2 = Some(6.0))?;
    }
    // Main motor terminals U1 V1 W1 under K4.
    let mut t_main = Vec::new();
    for (i, nm) in ["U1", "V1", "W1"].into_iter().enumerate() {
        t_main.push(kit.term(x2, nm, term, (x + 8.0 * i as f64, 116.0)));
    }
    let t_pe = kit.term(x2, "PE", term_pe, (x + 24.0, 116.0));
    for (i, pin) in ["2", "4", "6"].into_iter().enumerate() {
        kit.wire_mm2((k4, pin), (t_main[i], "1"), 6.0);
    }
    let fpe = kit.feed_pe(pe, (x + 24.0, 24.0));
    kit.wire_mm2((fpe, "1"), (t_pe, "1"), 6.0);
    // Delta closure: L1-W2, L2-U2, L3-V2, with the star contactor behind.
    // Its bottom side meets the delta outputs, its top side is the star point.
    let mut t_delta = Vec::new();
    for (i, nm) in ["W2", "U2", "V2"].into_iter().enumerate() {
        t_delta.push(kit.term(x2, nm, term, (100.0 + 8.0 * i as f64, 124.0)));
    }
    let jy = [108.0, 112.0, 116.0];
    let mut star_points = Vec::new();
    for (i, pin) in ["2", "4", "6"].into_iter().enumerate() {
        let jx = 100.0 + 8.0 * i as f64;
        let j = kit.junction((jx, jy[i]));
        kit.link(End::Pin((k6, pin)), End::Junction(j), &[]);
        kit.link(End::Junction(j), End::Pin((t_delta[i], "1")), &[]);
        kit.link(
            End::Pin((k5, pin)),
            End::Junction(j),
            &[(160.0 + 8.0 * i as f64, jy[i])],
        );
        star_points.push(kit.junction((160.0 + 8.0 * i as f64, 76.0)));
    }
    for (i, pin) in ["1", "3", "5"].into_iter().enumerate() {
        kit.link(End::Pin((k5, pin)), End::Junction(star_points[i]), &[]);
    }
    kit.link(
        End::Junction(star_points[0]),
        End::Junction(star_points[1]),
        &[],
    );
    kit.link(
        End::Junction(star_points[1]),
        End::Junction(star_points[2]),
        &[],
    );
    // The six-terminal motor, reached through terminals under K4 and K6.
    let m4 = kit.put("iec/motors/motor_3ph_6t", (x, 160.0), &format!("{FLD}-M4"));
    kit.describe(&format!("{FLD}-M4"), "Screw compressor 11 kW, star-delta");
    kit.props(&format!("{FLD}-M4"), &[("power_kw", 11.0)]);
    let mut mw = Vec::new();
    for (i, pin) in ["U1", "V1", "W1"].into_iter().enumerate() {
        mw.push(kit.wire_mm2((t_main[i], "2"), (m4, pin), 6.0));
    }
    // U2 V2 W2: terminal order W2 U2 V2 left to right.
    let to_pin = [
        ("W2", 56.0, 200.0),
        ("U2", 40.0, 204.0),
        ("V2", 48.0, 208.0),
    ];
    for (i, (pin, mx, level)) in to_pin.into_iter().enumerate() {
        let tx_ = 100.0 + 8.0 * i as f64;
        mw.push(kit.wire_via((t_delta[i], "2"), (m4, pin), &[(tx_, level), (mx, level)]));
    }
    mw.push(kit.wire_mm2((t_pe, "2"), (m4, "PE"), 6.0));
    kit.cable(
        "=MCC-W4",
        &["U1", "V1", "W1", "W2", "U2", "V2", "GNYE"],
        &mw,
        6.0,
        18.0,
        "Compressor motor cable 7G6",
    )?;

    // ----- page 4: control, direct-on-line feeders ------------------------
    kit.on(control_dol);
    let estop_x = 40.0;
    for (idx, d) in dols.iter().enumerate() {
        let n_ = d.n;
        let x = estop_x + 110.0 * idx as f64;
        let mut col = kit.rung(p24, x);
        if idx == 0 {
            kit.series(
                &mut col,
                "iec/operators/emergency_stop",
                &format!("{CAB}-S0"),
            );
        } else {
            kit.series(&mut col, "iec/contacts/nc", &format!("{CAB}-S0"));
        }
        kit.series(
            &mut col,
            "iec/operators/pushbutton_nc",
            &format!("{CAB}-S{n_}1"),
        );
        kit.hold(
            &mut col,
            "iec/operators/pushbutton_no",
            &format!("{CAB}-S{n_}2"),
            &format!("{CAB}-K{n_}"),
        );
        kit.series(&mut col, "iec/contacts/nc", &format!("{CAB}-Q{n_}"));
        kit.series(&mut col, "iec/coils/contactor", &format!("{CAB}-K{n_}"));
        kit.close(&col, p0);
        // Run lamp from a second contact of the contactor.
        let mut lamp = kit.rung(p24, x + 48.0);
        kit.series(&mut lamp, "iec/contacts/no", &format!("{CAB}-K{n_}"));
        kit.series(
            &mut lamp,
            "iec/signalling/lamp_green",
            &format!("{CAB}-H{n_}"),
        );
        kit.close(&lamp, p0);
        kit.describe(&format!("{CAB}-S{n_}1"), &format!("{} stop", d.what));
        kit.describe(&format!("{CAB}-S{n_}1"), &format!("{} start", d.what));
        kit.describe(&format!("{CAB}-H{n_}"), &format!("{} running", d.what));
    }
    kit.describe(&format!("{CAB}-S0"), "Emergency stop");

    // ----- page 5: control, star-delta feeder -----------------------------
    kit.on(control_sd);
    let mut c = kit.rung(p24, 40.0);
    kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-S0"));
    kit.series(&mut c, "iec/operators/pushbutton_nc", &format!("{CAB}-S41"));
    kit.hold(
        &mut c,
        "iec/operators/pushbutton_no",
        &format!("{CAB}-S42"),
        &format!("{CAB}-K4"),
    );
    kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-Q4"));
    kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K4"));
    kit.close(&c, p0);
    let mut c = kit.rung(p24, 130.0);
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K4"));
    kit.series(&mut c, "iec/coils/timer_on_delay", &format!("{CAB}-K7"));
    kit.close(&c, p0);
    // Star: after K4, until the timer runs out, with K6 not pulled in.
    let mut c = kit.rung(p24, 200.0);
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K4"));
    kit.series(&mut c, "iec/contacts/nc_delay_on", &format!("{CAB}-K7"));
    kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-K6"));
    kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K5"));
    kit.close(&c, p0);
    // Delta: when the timer has run out, with K5 dropped.
    let mut c = kit.rung(p24, 270.0);
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K4"));
    kit.series(&mut c, "iec/contacts/no_delay_on", &format!("{CAB}-K7"));
    kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-K5"));
    kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K6"));
    kit.close(&c, p0);
    let mut c = kit.rung(p24, 340.0);
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K4"));
    kit.series(&mut c, "iec/signalling/lamp_green", &format!("{CAB}-H4"));
    kit.close(&c, p0);
    kit.describe(&format!("{CAB}-S41"), "Compressor 2 stop");
    kit.describe(&format!("{CAB}-S42"), "Compressor 2 start");
    kit.describe(&format!("{CAB}-K7"), "Star-delta changeover timer");
    kit.describe(&format!("{CAB}-H4"), "Compressor 2 running");

    // ----- parts ----------------------------------------------------------
    let dev = |kit: &Kit<'_, '_>, t: &str| kit.device(t);
    let generic: [(&str, &Article); 12] = [
        ("=MCC+CAB-Q0", &SWITCH),
        ("=MCC+CAB-Q10", &MCB2),
        ("=MCC+CAB-Q4", &MPCB_25),
        ("=MCC+CAB-K4", &CONTACTOR_25),
        ("=MCC+CAB-K5", &CONTACTOR_25),
        ("=MCC+CAB-K6", &CONTACTOR_25),
        ("=MCC+CAB-K7", &TIMER),
        ("=MCC+CAB-S0", &ESTOP),
        ("=MCC+CAB-H1", &LAMP),
        ("=MCC+CAB-H2", &LAMP),
        ("=MCC+CAB-H3", &LAMP),
        ("=MCC+CAB-H4", &LAMP),
    ];
    for (t, a) in generic {
        let d = dev(&kit, t);
        parts.generic(kit.tx, d, a)?;
    }
    for (n_, mpcb, motor, article) in motor_ties {
        let q = dev(&kit, &format!("{CAB}-Q{n_}"));
        parts.catalog(kit.tx, q, "Siemens", mpcb)?;
        let k = dev(&kit, &format!("{CAB}-K{n_}"));
        parts.catalog(kit.tx, k, "Siemens", "3RT2016-1BB41")?;
        let m = dev(&kit, &format!("{FLD}-M{n_}"));
        match article {
            Some(a) => parts.generic(kit.tx, m, a)?,
            None => parts.catalog(kit.tx, m, "Siemens", motor)?,
        }
    }
    let m4 = dev(&kit, &format!("{FLD}-M4"));
    parts.generic(kit.tx, m4, &MOTOR_11KW)?;
    let g1 = dev(&kit, &format!("{CAB}-G1"));
    parts.catalog(kit.tx, g1, "Phoenix Contact", "2904601")?;
    for (t, order) in [
        ("S11", "3SU1100-0AB20-1CA0"),
        ("S12", "3SU1100-0AB40-1BA0"),
        ("S21", "3SU1100-0AB20-1CA0"),
        ("S22", "3SU1100-0AB40-1BA0"),
        ("S31", "3SU1100-0AB20-1CA0"),
        ("S32", "3SU1100-0AB40-1BA0"),
        ("S41", "3SU1100-0AB20-1CA0"),
        ("S42", "3SU1100-0AB40-1BA0"),
    ] {
        let d = dev(&kit, &format!("{CAB}-{t}"));
        parts.catalog(kit.tx, d, "Siemens", order)?;
    }
    // Terminal strips: blocks, footprints.
    for (strip, count) in [(x0, 5usize), (x1, 12), (x2, 10)] {
        style_strip(kit.tx, &mut parts, strip, count)?;
    }
    // Cables: parts for the supply and the motor cables.
    let cables: Vec<_> = kit
        .tx
        .project()
        .cables()
        .map(|c| (c.id, c.tag.to_string()))
        .collect();
    for (id, t) in cables {
        let part = match t.as_str() {
            "=MCC-W0" => parts.generic_part(kit.tx, &CABLE_SUPPLY)?,
            "=MCC-W4" => parts.generic_part(kit.tx, &CABLE_7G25)?,
            _ => parts.catalog_part(kit.tx, "Lapp", "1119304")?,
        };
        kit.tx
            .update::<novo_core::Cable>(id, |c| c.part = Some(part))?;
    }
    Ok(())
}
