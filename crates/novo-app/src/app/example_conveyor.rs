//! The conveyor line example (medium): three belts on direct-on-line
//! starters that start one after the other from the discharge end, three
//! photoelectric sensors in the field, an emergency stop, a collective
//! motor-protection relay, indication and a signal to a master controller,
//! 24 V DC control, a laid-out cabinet and a parts list, over five pages.
//! Illustrative: sizes and ratings are planning values, not a verified design.

use novo_core::{Document, PotentialKind, Result as ModelResult, Tx};

use super::example_blocks::{self as b, Dol, Pots, TERM};
use super::example_kit::{Article, Finish, Kit, Parts, add_page, enclosure, style_strip, tag};

/// What the project is called.
pub(crate) const CONVEYOR_EXAMPLE_NAME: &str = "Conveyor line (example)";

const DESCRIPTION: &str = "Conveyor line, the medium example: three belts (1.1, 1.1 and 2.2 kW) on \
    direct-on-line starters that start one after the other from the discharge end, an emergency \
    stop, a collective motor-protection relay, three photoelectric sensors, indication and a signal \
    to a master controller, 24 V DC control, a laid-out cabinet with heat, drilling and parts list. \
    Ratings and parts without a maker are planning values to replace with your own.";

const CAB: &str = "=CNV+CAB";
const FLD: &str = "=CNV+FLD";

/// Draws the example into a project that has only its empty first page.
pub(crate) fn build_conveyor_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
    let ((), changes) = doc.transact("Open example", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        draw(tx)?;
        super::example_kit::fill_sections(tx, 0.75)?;
        let (plate, enclosure) = enclosure((600.0, 800.0), (640.0, 840.0, 260.0));
        super::example_kit::finish(
            tx,
            &Finish {
                name: CONVEYOR_EXAMPLE_NAME,
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
    let b1 = tx.add_potential("B1:OUT", PotentialKind::Signal)?;
    let b2 = tx.add_potential("B2:OUT", PotentialKind::Signal)?;
    let b3 = tx.add_potential("B3:OUT", PotentialKind::Signal)?;
    let to_master = tx.add_potential("FULL.OUT", PotentialKind::Signal)?;

    let incoming = add_page(tx, "1", "Incoming supply and 24 V", CAB)?;
    let feeders = add_page(tx, "2", "Belt motor feeders", CAB)?;
    let sensors = add_page(tx, "3", "Photoelectric sensors", CAB)?;
    let control = add_page(tx, "4", "Control: stop, start and belt sequence", CAB)?;
    let signals = add_page(tx, "5", "Protection, indication and signal out", CAB)?;

    let x0 = tx.add_terminal_strip(tag(&format!("{CAB}-X0")))?;
    let x1 = tx.add_terminal_strip(tag(&format!("{CAB}-X1")))?;
    let x2 = tx.add_terminal_strip(tag(&format!("{CAB}-X2")))?;
    let x3 = tx.add_terminal_strip(tag(&format!("{CAB}-X3")))?;
    let mut kit = Kit::new(tx, incoming);

    // ----- page 1 -------------------------------------------------------------
    b::supply_page(&mut kit, &p, CAB, "=CNV", x0, 2.5)?;

    // ----- page 2: three belt feeders -------------------------------------------
    kit.on(feeders);
    let belts = [
        (1, 40.0, "Belt 1 (infeed)", 1.1, 9.0),
        (2, 120.0, "Belt 2 (transfer)", 1.1, 11.0),
        (3, 200.0, "Belt 3 (discharge)", 2.2, 14.0),
    ];
    for (n, x, what, kw, cable_m) in belts {
        b::dol_feeder(
            &mut kit,
            &p,
            CAB,
            FLD,
            "=CNV",
            x1,
            &Dol {
                n,
                x,
                what,
                kw,
                cable_m,
            },
        )?;
    }

    // ----- page 3: photoelectric sensors -------------------------------------------
    kit.on(sensors);
    for (i, (name, what, out)) in [
        ("B1", "Belt 1 full", b1),
        ("B2", "Transfer occupied", b2),
        ("B3", "End of line full", b3),
    ]
    .into_iter()
    .enumerate()
    {
        let x = 40.0 + 70.0 * i as f64;
        let fp = kit.feed(p.p24, (x, 24.0));
        let f0 = kit.feed(p.p0, (x + 16.0, 24.0));
        let tp = kit.term(x2, &format!("{name}+"), TERM, (x, 56.0));
        let t0 = kit.term(x2, &format!("{name}-"), TERM, (x + 16.0, 56.0));
        kit.wire_mm2((fp, "1"), (tp, "1"), 0.75);
        kit.wire_mm2((f0, "1"), (t0, "1"), 0.75);
        let s = kit.put(
            "iec/sensors/photoelectric_pnp",
            (x, 92.0),
            &format!("{FLD}-{name}"),
        );
        kit.describe(&format!("{FLD}-{name}"), what);
        let ts = kit.term(x2, &format!("{name}S"), TERM, (x + 8.0, 130.0));
        let w1 = kit.wire_mm2((tp, "2"), (s, "1"), 0.75);
        let w3 = kit.wire_mm2((t0, "2"), (s, "3"), 0.75);
        let w4 = kit.wire_mm2((s, "4"), (ts, "1"), 0.75);
        let fo = kit.feed_down(out, (x + 8.0, 166.0));
        kit.wire_mm2((ts, "2"), (fo, "1"), 0.75);
        kit.cable(
            &format!("=CNV-W{}", i + 4),
            &["BN", "BU", "BK"],
            &[w1, w3, w4],
            0.75,
            [6.0, 7.0, 8.0][i],
            &format!("{what} sensor cable 3x0.75"),
        )?;
    }

    // ----- page 4: control ------------------------------------------------------------
    kit.on(control);
    // Line enable: emergency stop, stop, motor protection healthy, start with self-hold.
    let mut c = kit.rung(p.p24, 30.0);
    kit.series(&mut c, "iec/operators/emergency_stop", &format!("{CAB}-S0"));
    kit.series(&mut c, "iec/operators/pushbutton_nc", &format!("{CAB}-S1"));
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K14"));
    kit.hold(
        &mut c,
        "iec/operators/pushbutton_no",
        &format!("{CAB}-S2"),
        &format!("{CAB}-K10"),
    );
    kit.series(&mut c, "iec/coils/relay", &format!("{CAB}-K10"));
    kit.close(&c, p.p0);
    // Belt 3 (discharge) first.
    let mut c = kit.rung(p.p24, 100.0);
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K10"));
    kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K3"));
    kit.close(&c, p.p0);
    // Five seconds later belt 2 ...
    let mut c = kit.rung(p.p24, 150.0);
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K3"));
    kit.series(&mut c, "iec/coils/timer_on_delay", &format!("{CAB}-K11"));
    kit.close(&c, p.p0);
    let mut c = kit.rung(p.p24, 200.0);
    kit.series(&mut c, "iec/contacts/no_delay_on", &format!("{CAB}-K11"));
    kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K2"));
    kit.close(&c, p.p0);
    // ... and five seconds after that belt 1, unless it is full.
    let mut c = kit.rung(p.p24, 250.0);
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K2"));
    kit.series(&mut c, "iec/coils/timer_on_delay", &format!("{CAB}-K12"));
    kit.close(&c, p.p0);
    let mut c = kit.rung(p.p24, 300.0);
    kit.series(&mut c, "iec/contacts/no_delay_on", &format!("{CAB}-K12"));
    kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-K13"));
    kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K1"));
    kit.close(&c, p.p0);
    kit.describe(&format!("{CAB}-S0"), "Emergency stop");
    kit.describe(&format!("{CAB}-S1"), "Line stop");
    kit.describe(&format!("{CAB}-S2"), "Line start");
    kit.describe(&format!("{CAB}-K10"), "Line run relay");
    kit.describe(&format!("{CAB}-K11"), "Belt 2 start delay");
    kit.describe(&format!("{CAB}-K12"), "Belt 1 start delay");

    // ----- page 5: protection, indication, signal out ----------------------------------
    kit.on(signals);
    // Collective motor protection: in while all three breakers are healthy.
    let mut c = kit.rung(p.p24, 30.0);
    for q in ["Q1", "Q2", "Q3"] {
        kit.series(&mut c, "iec/contacts/nc", &format!("{CAB}-{q}"));
    }
    kit.series(&mut c, "iec/coils/relay", &format!("{CAB}-K14"));
    kit.close(&c, p.p0);
    // Sensor relays.
    for (x, sig, relay) in [(90.0, b1, "K13"), (130.0, b2, "K15"), (170.0, b3, "K16")] {
        let mut c = kit.rung(sig, x);
        kit.series(&mut c, "iec/coils/relay", &format!("{CAB}-{relay}"));
        kit.close(&c, p.p0);
    }
    // Indication.
    for (x, contact, key, lamp) in [
        (230.0, "K10", "iec/signalling/lamp_green", "H1"),
        (270.0, "K13", "iec/signalling/lamp_amber", "H2"),
        (310.0, "K14", "iec/signalling/lamp_red", "H3"),
        (350.0, "K15", "iec/signalling/lamp_amber", "H4"),
    ] {
        let mut c = kit.rung(p.p24, x);
        let contact_key = if contact == "K14" {
            "iec/contacts/nc"
        } else {
            "iec/contacts/no"
        };
        kit.series(&mut c, contact_key, &format!("{CAB}-{contact}"));
        kit.series(&mut c, key, &format!("{CAB}-{lamp}"));
        kit.close(&c, p.p0);
    }
    // Signal to the master controller through terminals on X3: end of line full.
    let mut c = kit.rung(p.p24, 390.0);
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K16"));
    kit.series_term(&mut c, x3, "FULL");
    kit.close(&c, to_master);
    kit.describe(&format!("{CAB}-K13"), "Belt 1 full relay");
    kit.describe(&format!("{CAB}-K14"), "Motor protection healthy relay");
    kit.describe(&format!("{CAB}-K15"), "Transfer occupied relay");
    kit.describe(&format!("{CAB}-K16"), "End of line full relay");
    kit.describe(&format!("{CAB}-H1"), "Line running");
    kit.describe(&format!("{CAB}-H2"), "Belt 1 full");
    kit.describe(&format!("{CAB}-H3"), "Motor protection tripped");
    kit.describe(&format!("{CAB}-H4"), "Transfer occupied");

    // ----- parts ----------------------------------------------------------------------------
    let generic: [(&str, &Article); 29] = [
        ("=CNV+CAB-Q0", &b::SWITCH_32),
        ("=CNV+CAB-Q10", &b::MCB2),
        ("=CNV+CAB-G1", &b::PSU_5),
        ("=CNV+CAB-Q1", &b::MPCB_4),
        ("=CNV+CAB-Q2", &b::MPCB_4),
        ("=CNV+CAB-Q3", &b::MPCB_6),
        ("=CNV+CAB-K1", &b::CONTACTOR_9),
        ("=CNV+CAB-K2", &b::CONTACTOR_9),
        ("=CNV+CAB-K3", &b::CONTACTOR_9),
        ("=CNV+CAB-K10", &b::RELAY),
        ("=CNV+CAB-K11", &b::TIMER),
        ("=CNV+CAB-K12", &b::TIMER),
        ("=CNV+CAB-K13", &b::RELAY),
        ("=CNV+CAB-K14", &b::RELAY),
        ("=CNV+CAB-K15", &b::RELAY),
        ("=CNV+CAB-K16", &b::RELAY),
        ("=CNV+CAB-S0", &b::ESTOP),
        ("=CNV+CAB-S1", &b::BTN_NC),
        ("=CNV+CAB-S2", &b::BTN_NO),
        ("=CNV+CAB-H1", &b::LAMP_GN),
        ("=CNV+CAB-H2", &b::LAMP_AMBER),
        ("=CNV+CAB-H3", &b::LAMP_RD),
        ("=CNV+CAB-H4", &b::LAMP_AMBER),
        ("=CNV+FLD-M1", &b::MOTOR_1_1),
        ("=CNV+FLD-M2", &b::MOTOR_1_1),
        ("=CNV+FLD-M3", &b::MOTOR_2_2),
        ("=CNV+FLD-B1", &b::PHOTOEYE),
        ("=CNV+FLD-B2", &b::PHOTOEYE),
        ("=CNV+FLD-B3", &b::PHOTOEYE),
    ];
    for (t, a) in generic {
        let d = kit.device(t);
        parts.generic(kit.tx, d, a)?;
    }
    for (strip, count) in [(x0, 5usize), (x1, 12), (x2, 9), (x3, 1)] {
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
            "=CNV-W0" => parts.generic_part(kit.tx, &b::CABLE_5G2_5)?,
            "=CNV-W1" | "=CNV-W2" | "=CNV-W3" => parts.generic_part(kit.tx, &b::CABLE_MOTOR)?,
            _ => parts.generic_part(kit.tx, &b::CABLE_SENSOR_3)?,
        };
        kit.tx
            .update::<novo_core::Cable>(id, |c| c.part = Some(part))?;
    }
    Ok(())
}
