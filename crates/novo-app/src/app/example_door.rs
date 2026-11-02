//! The roller door example (small): a 0.75 kW door motor on a reversing
//! starter with two position switches and a safety edge in the field, 24 V
//! DC control, a laid-out cabinet and a parts list, over four pages.
//! Illustrative: sizes and ratings are planning values, not a verified design.

use novo_core::{Document, PotentialKind, Result as ModelResult, Tx};

use super::example_blocks::{self as b, Pots, TERM};
use super::example_kit::End;
use super::example_kit::{Finish, Kit, Parts, add_page, enclosure, style_strip, tag};

/// What the project is called.
pub(crate) const DOOR_EXAMPLE_NAME: &str = "Roller door (example)";

const DESCRIPTION: &str = "Roller door, the small example: a 0.75 kW door motor on a reversing \
    starter with mutual interlock, top and bottom position switches and a safety edge in the field, \
    24 V DC control, a laid-out cabinet with heat, drilling and parts list. Ratings and parts without \
    a maker are planning values to replace with your own.";

const CAB: &str = "=DOR+CAB";
const FLD: &str = "=DOR+FLD";

/// Draws the example into a project that has only its empty first page.
pub(crate) fn build_door_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
    let ((), changes) = doc.transact("Open example", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        draw(tx)?;
        super::example_kit::fill_sections(tx, 0.75)?;
        let (plate, enclosure) = enclosure((500.0, 600.0), (540.0, 640.0, 250.0));
        super::example_kit::finish(
            tx,
            &Finish {
                name: DOOR_EXAMPLE_NAME,
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
    let p = Pots::add(tx)?;
    // 24 V that is there while the top limit, bottom limit and safety edge
    // circuits are closed.
    let top_ok = tx.add_potential("+24V.TOP", PotentialKind::DcPositive)?;
    let bot_ok = tx.add_potential("+24V.BOT", PotentialKind::DcPositive)?;
    let edge_ok = tx.add_potential("+24V.EDGE", PotentialKind::DcPositive)?;

    let incoming = add_page(tx, "1", "Incoming supply and 24 V", CAB)?;
    let motor = add_page(tx, "2", "Door motor, reversing starter", CAB)?;
    let sensors = add_page(tx, "3", "Position switches and safety edge", CAB)?;
    let control = add_page(tx, "4", "Control: up, down, stop", CAB)?;

    let x0 = tx.add_terminal_strip(tag(&format!("{CAB}-X0")))?;
    let x1 = tx.add_terminal_strip(tag(&format!("{CAB}-X1")))?;
    let x2 = tx.add_terminal_strip(tag(&format!("{CAB}-X2")))?;
    let mut kit = Kit::new(tx, incoming);

    // ----- page 1 ---------------------------------------------------------
    b::supply_page(&mut kit, &p, CAB, "=DOR", x0, 2.5)?;

    // ----- page 2: reversing starter ----------------------------------------
    kit.on(motor);
    let ft: Vec<_> = [p.l1, p.l2, p.l3]
        .into_iter()
        .enumerate()
        .map(|(i, pot)| kit.feed(pot, (40.0 + 8.0 * i as f64, 24.0)))
        .collect();
    let q1 = kit.put(
        "iec/protection/motor_protection_breaker",
        (40.0, 48.0),
        &format!("{CAB}-Q1"),
    );
    kit.describe(&format!("{CAB}-Q1"), "Door motor protection");
    for (i, pin) in ["1", "3", "5"].into_iter().enumerate() {
        kit.wire_mm2((ft[i], "1"), (q1, pin), 1.5);
    }
    let k1 = kit.put("iec/contactors/main_3p", (40.0, 84.0), &format!("{CAB}-K1"));
    let k2 = kit.put(
        "iec/contactors/main_3p",
        (100.0, 84.0),
        &format!("{CAB}-K2"),
    );
    kit.describe(&format!("{CAB}-K1"), "Door up contactor");
    kit.describe(&format!("{CAB}-K2"), "Door down contactor");
    kit.wire3((q1, ["2", "4", "6"]), (k1, ["1", "3", "5"]), 1.5);
    // K2 swaps L1 and L3: the motor turns the other way.
    for (qp, kp, level, tx_) in [
        ("2", "5", 70.0, 116.0),
        ("4", "3", 74.0, 108.0),
        ("6", "1", 78.0, 100.0),
    ] {
        let qx = 40.0 + 8.0 * (f64::from(qp.parse::<u8>().unwrap()) / 2.0 - 1.0);
        let w = kit.wire_via((q1, qp), (k2, kp), &[(qx, level), (tx_, level)]);
        kit.set_mm2(w, 1.5);
    }
    let mut ts = Vec::new();
    for (i, (nm, key)) in [("U", TERM), ("V", TERM), ("W", TERM), ("PE", b::TERM_PE)]
        .into_iter()
        .enumerate()
    {
        ts.push(kit.term(x1, &format!("1{nm}"), key, (40.0 + 8.0 * i as f64, 124.0)));
    }
    // K1 and K2 outputs meet at a junction above each motor terminal.
    let jy = 112.0;
    for (i, (p1, p2, ty, jx2)) in [
        ("2", "6", 110.0, 116.0),
        ("4", "4", 107.0, 108.0),
        ("6", "2", 104.0, 100.0),
    ]
    .into_iter()
    .enumerate()
    {
        let jx = 40.0 + 8.0 * i as f64;
        let j = kit.junction((jx, jy));
        kit.link(End::Pin((k1, p1)), End::Junction(j), &[]);
        kit.link(End::Pin((k2, p2)), End::Junction(j), &[(jx2, ty), (jx, ty)]);
        let w = kit.link(End::Junction(j), End::Pin((ts[i], "1")), &[]);
        kit.set_mm2(w, 1.5);
    }
    let fpe = kit.feed_pe(p.pe, (64.0, 100.0));
    kit.wire_mm2((fpe, "1"), (ts[3], "1"), 1.5);
    let m = kit.put("iec/motors/motor_3ph", (40.0, 160.0), &format!("{FLD}-M1"));
    kit.describe(&format!("{FLD}-M1"), "Door motor 0.75 kW");
    kit.props(&format!("{FLD}-M1"), &[("power_kw", 0.75)]);
    let mut mw = Vec::new();
    for (i, pin) in ["U1", "V1", "W1"].into_iter().enumerate() {
        mw.push(kit.wire_mm2((ts[i], "2"), (m, pin), 1.5));
    }
    mw.push(kit.wire_mm2((ts[3], "2"), (m, "PE"), 1.5));
    kit.cable(
        "=DOR-W1",
        &["BK", "BN", "GY", "GNYE"],
        &mw,
        1.5,
        8.0,
        "Door motor cable 4G1.5",
    )?;

    // ----- page 3: position switches and safety edge -------------------------
    kit.on(sensors);
    let x2_term = |kit: &mut Kit<'_, '_>, name: &str, at: (f64, f64)| kit.term(x2, name, TERM, at);
    for (i, (name, what, article_tag, out)) in [
        ("B1", "Top limit switch", "B1", top_ok),
        ("B2", "Bottom limit switch", "B2", bot_ok),
        ("B3", "Safety edge", "B3", edge_ok),
    ]
    .into_iter()
    .enumerate()
    {
        let x = 40.0 + 70.0 * i as f64;
        let fa = kit.feed(p.p24, (x, 24.0));
        let ta = x2_term(&mut kit, &format!("{name}.1"), (x, 56.0));
        kit.wire_mm2((fa, "1"), (ta, "1"), 0.75);
        let s = kit.put(
            "iec/sensors/proximity_nc",
            (x, 92.0),
            &format!("{FLD}-{article_tag}"),
        );
        kit.describe(&format!("{FLD}-{article_tag}"), what);
        let tb = x2_term(&mut kit, &format!("{name}.2"), (x, 130.0));
        let w1 = kit.wire_mm2((ta, "2"), (s, "21"), 0.75);
        let w2 = kit.wire_mm2((s, "22"), (tb, "1"), 0.75);
        let fo = kit.feed_down(out, (x, 166.0));
        kit.wire_mm2((tb, "2"), (fo, "1"), 0.75);
        kit.cable(
            &format!("=DOR-W{}", i + 2),
            &["BN", "BU"],
            &[w1, w2],
            0.75,
            [6.0, 6.0, 5.0][i],
            &format!("{what} cable 2x0.75"),
        )?;
    }

    // ----- page 4: control ----------------------------------------------------
    kit.on(control);
    // Up: top limit closed, stop not pressed, motor protection and the down
    // contactor not in, start with self-hold.
    let mut c = kit.rung(top_ok, 30.0);
    kit.series(&mut c, "iec/operators/pushbutton_nc", &format!("{CAB}-S3"));
    kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-Q1"));
    kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-K2"));
    kit.hold(
        &mut c,
        "iec/operators/pushbutton_no",
        &format!("{CAB}-S1"),
        &format!("{CAB}-K1"),
    );
    kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K1"));
    kit.close(&c, p.p0);
    // Down: bottom limit closed and the safety edge relay in.
    let mut c = kit.rung(bot_ok, 100.0);
    kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-S3"));
    kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-Q1"));
    kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-K1"));
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K3"));
    kit.hold(
        &mut c,
        "iec/operators/pushbutton_no",
        &format!("{CAB}-S2"),
        &format!("{CAB}-K2"),
    );
    kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K2"));
    kit.close(&c, p.p0);
    // The safety edge relay: in while the edge circuit is closed.
    let mut c = kit.rung(edge_ok, 170.0);
    kit.series(&mut c, "iec/coils/relay", &format!("{CAB}-K3"));
    kit.close(&c, p.p0);
    // Moving lamp.
    let mut c = kit.rung(p.p24, 210.0);
    kit.either(
        &mut c,
        "iec/contacts/no",
        &format!("{CAB}-K1"),
        "iec/contacts/no",
        &format!("{CAB}-K2"),
    );
    kit.series(&mut c, "iec/signalling/lamp_green", &format!("{CAB}-H1"));
    kit.close(&c, p.p0);
    kit.describe(&format!("{CAB}-S1"), "Door up");
    kit.describe(&format!("{CAB}-S2"), "Door down");
    kit.describe(&format!("{CAB}-S3"), "Door stop");
    kit.describe(&format!("{CAB}-K3"), "Safety edge relay");
    kit.describe(&format!("{CAB}-H1"), "Door moving");

    // ----- parts ----------------------------------------------------------------
    let generic: [(&str, &super::example_kit::Article); 15] = [
        ("=DOR+CAB-Q0", &b::SWITCH_32),
        ("=DOR+CAB-Q10", &b::MCB2),
        ("=DOR+CAB-G1", &b::PSU_5),
        ("=DOR+CAB-Q1", &b::MPCB_2_5),
        ("=DOR+CAB-K1", &b::CONTACTOR_9),
        ("=DOR+CAB-K2", &b::CONTACTOR_9),
        ("=DOR+CAB-K3", &b::RELAY),
        ("=DOR+CAB-S1", &b::BTN_NO),
        ("=DOR+CAB-S2", &b::BTN_NO),
        ("=DOR+CAB-S3", &b::BTN_NC),
        ("=DOR+CAB-H1", &b::LAMP_GN),
        ("=DOR+FLD-M1", &b::MOTOR_0_75),
        ("=DOR+FLD-B1", &b::LIMIT_SWITCH),
        ("=DOR+FLD-B2", &b::LIMIT_SWITCH),
        ("=DOR+FLD-B3", &b::LIMIT_SWITCH),
    ];
    for (t, a) in generic {
        let d = kit.device(t);
        parts.generic(kit.tx, d, a)?;
    }
    for (strip, count) in [(x0, 5usize), (x1, 4), (x2, 6)] {
        style_strip(kit.tx, &mut parts, strip, count)?;
    }
    let cables: Vec<_> = kit
        .tx
        .project()
        .cables()
        .map(|c| (c.id, c.tag.to_string()))
        .collect();
    for (id, t) in cables {
        let part = match t.as_str() {
            "=DOR-W0" => parts.generic_part(kit.tx, &b::CABLE_5G2_5)?,
            "=DOR-W1" => parts.generic_part(kit.tx, &b::CABLE_MOTOR)?,
            _ => parts.generic_part(kit.tx, &b::CABLE_SENSOR)?,
        };
        kit.tx
            .update::<novo_core::Cable>(id, |c| c.part = Some(part))?;
    }
    Ok(())
}
