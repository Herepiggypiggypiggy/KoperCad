//! The bottle filling line example (large): four belts and a capper on
//! direct-on-line starters, the filler pump on a frequency drive with a
//! screened motor cable, an emergency stop relay, a PLC with a CPU, two
//! 16-channel input cards, an 8-channel output card and an analog card,
//! field wiring through terminal strips, a laid-out cabinet and a parts
//! list, over twelve pages. Every PLC channel is drawn with the editor's own
//! `to plc` command, so the I/O list and the generated pages are the real
//! thing. Illustrative: sizes and ratings are planning values, not a
//! verified design.

use novo_core::{Document, Endpoint, PotentialKind, Result as ModelResult, Rotation, Tx, WireId};
use novo_plc::ChannelType;
use novo_plc::commands::{NewCardLayout, add_card};

use super::example_blocks::{self as b, Dol, Pots, TERM};
use super::example_kit::{Article, End, Finish, Kit, Parts, add_page, enclosure, style_strip, tag};
use super::example_plc::{card_part, input_column, pin_at, to_channel};

/// What the project is called.
pub(crate) const FILLING_EXAMPLE_NAME: &str = "Bottle filling line (example)";

const DESCRIPTION: &str = "Bottle filling line, the large example: infeed and outfeed belts, a \
    capper and a labeller on direct-on-line starters, the filler pump on a frequency drive with a \
    screened motor cable, an emergency stop relay, a PLC with CPU, 32 digital inputs, 8 digital \
    outputs and 4 analog inputs, field wiring on terminal strips, a laid-out cabinet with heat, \
    drilling and parts list. Ratings, parts without a maker and the 400 V / 10 kA supply are \
    planning values to replace with your own.";

const CAB: &str = "=FIL+CAB";
const FLD: &str = "=FIL+FLD";
const OP: &str = "=FIL+OP";

/// Draws the example into a project that has only its empty first page.
pub(crate) fn build_filling_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
    let ((), changes) = doc.transact("Open example", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        draw(tx)?;
        super::example_kit::fill_sections(tx, 0.75)?;
        let (plate, enclosure) = enclosure((900.0, 1100.0), (940.0, 1140.0, 300.0));
        super::example_kit::finish(
            tx,
            &Finish {
                name: FILLING_EXAMPLE_NAME,
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

fn names_of(count: usize) -> Vec<String> {
    (1..=count).map(|i| i.to_string()).collect()
}

fn draw(tx: &mut Tx<'_>) -> ModelResult<()> {
    let mut parts = Parts::default();
    let p = Pots::add(tx)?;
    let p24p = tx.add_potential("+24V.PLC", PotentialKind::DcPositive)?;
    let p24i = tx.add_potential("+24V.IN", PotentialKind::DcPositive)?;
    let p24o = tx.add_potential("+24V.OUT", PotentialKind::DcPositive)?;

    let supply = add_page(tx, "1", "Incoming supply and 24 V", CAB)?;
    let belts = add_page(tx, "2", "Belts, capper and labeller", CAB)?;
    let filler = add_page(tx, "3", "Filler pump drive", CAB)?;
    let safety = add_page(tx, "4", "Emergency stop and drive command", CAB)?;
    let plc = add_page(tx, "5", "PLC and cards", CAB)?;
    let di_a = add_page(tx, "6", "Digital inputs: operator panel", CAB)?;
    let di_b = add_page(tx, "7", "Digital inputs: machine", CAB)?;
    let di_c = add_page(tx, "8", "Digital inputs: motor protection", CAB)?;
    let dq = add_page(tx, "9", "Digital outputs", CAB)?;
    let ai = add_page(tx, "10", "Analog inputs", CAB)?;

    let x0 = tx.add_terminal_strip(tag(&format!("{CAB}-X0")))?;
    let x1 = tx.add_terminal_strip(tag(&format!("{CAB}-X1")))?;
    let x2 = tx.add_terminal_strip(tag(&format!("{CAB}-X2")))?;
    let x3 = tx.add_terminal_strip(tag(&format!("{CAB}-X3")))?;
    let x4 = tx.add_terminal_strip(tag(&format!("{CAB}-X4")))?;
    let x5 = tx.add_terminal_strip(tag(&format!("{CAB}-X5")))?;

    // The cards come first so their channels can be drawn on the pages.
    let di_part = card_part(
        tx,
        "PLC-DI16-24VDC",
        "Digital input card, 16 channels, 24 V DC",
        "DI16",
        2.0,
    )?;
    let dq_part = card_part(
        tx,
        "PLC-DQ8-24VDC",
        "Digital output card, 8 channels, 24 V DC, 0.5 A",
        "DQ8",
        3.0,
    )?;
    let ai_part = card_part(
        tx,
        "PLC-AI4-420MA",
        "Analog input card, 4 channels, 4 to 20 mA",
        "AI4",
        2.0,
    )?;
    let card_di1 = add_card(
        tx,
        tag(&format!("{CAB}-A2")),
        "Digital input card 1",
        NewCardLayout::Part(di_part),
        "",
        Some(1),
    )?;
    let card_dq = add_card(
        tx,
        tag(&format!("{CAB}-A3")),
        "Digital output card",
        NewCardLayout::Part(dq_part),
        "",
        Some(2),
    )?;
    let card_ai = add_card(
        tx,
        tag(&format!("{CAB}-A4")),
        "Analog input card",
        NewCardLayout::Part(ai_part),
        "",
        Some(3),
    )?;
    let card_di2 = add_card(
        tx,
        tag(&format!("{CAB}-A5")),
        "Digital input card 2",
        NewCardLayout::Part(di_part),
        "",
        Some(4),
    )?;

    let mut kit = Kit::new(tx, supply);

    // ----- page 1: incoming supply, 24 V and its three branches --------------------
    b::supply_page(&mut kit, &p, CAB, "=FIL", x0, 6.0)?;
    for (i, (pot, what)) in [(p24p, "PLC"), (p24i, "inputs"), (p24o, "outputs")]
        .into_iter()
        .enumerate()
    {
        let x = 200.0 + 24.0 * i as f64;
        let f = kit.feed(p.p24, (x, 24.0));
        let q = kit.put(
            "iec/protection/mcb_1p",
            (x, 52.0),
            &format!("{CAB}-Q2{}", i + 1),
        );
        kit.describe(
            &format!("{CAB}-Q2{}", i + 1),
            &format!("24 V branch: {what}"),
        );
        kit.wire_mm2((f, "1"), (q, "1"), 1.5);
        let out = kit.feed_down(pot, (x, 92.0));
        kit.wire_mm2((q, "2"), (out, "1"), 1.5);
    }

    // ----- page 2: four direct-on-line feeders -----------------------------------------
    kit.on(belts);
    for (n, x, what, kw, cable_m) in [
        (1, 40.0, "Infeed belt", 1.1, 8.0),
        (2, 110.0, "Outfeed belt", 1.1, 12.0),
        (3, 180.0, "Capper", 0.75, 10.0),
        (4, 250.0, "Labeller", 0.75, 14.0),
    ] {
        b::dol_feeder(
            &mut kit,
            &p,
            CAB,
            FLD,
            "=FIL",
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

    // ----- page 3: the filler pump on a drive ----------------------------------------------
    kit.on(filler);
    let x = 40.0;
    let ft: Vec<_> = [p.l1, p.l2, p.l3]
        .into_iter()
        .enumerate()
        .map(|(i, pot)| kit.feed(pot, (x + 8.0 * i as f64, 24.0)))
        .collect();
    let q = kit.put(
        "iec/protection/motor_protection_breaker",
        (x, 48.0),
        &format!("{CAB}-Q5"),
    );
    kit.describe(&format!("{CAB}-Q5"), "Filler pump drive protection");
    for (i, pin) in ["1", "3", "5"].into_iter().enumerate() {
        kit.wire_mm2((ft[i], "1"), (q, pin), 2.5);
    }
    let k = kit.put("iec/contactors/main_3p", (x, 84.0), &format!("{CAB}-K5"));
    kit.describe(&format!("{CAB}-K5"), "Filler pump line contactor");
    kit.wire3((q, ["2", "4", "6"]), (k, ["1", "3", "5"]), 2.5);
    let u = kit.put(
        "iec/drives/frequency_converter",
        (x, 120.0),
        &format!("{CAB}-U5"),
    );
    kit.describe(&format!("{CAB}-U5"), "Filler pump frequency drive 3 kW");
    kit.wire3((k, ["2", "4", "6"]), (u, ["L1", "L2", "L3"]), 2.5);
    let fpe = kit.feed_pe(p.pe, (x + 24.0, 100.0));
    kit.wire_mm2((fpe, "1"), (u, "PE"), 2.5);
    let mut ts = Vec::new();
    for (i, (nm, key)) in [("U", TERM), ("V", TERM), ("W", TERM), ("PE", b::TERM_PE)]
        .into_iter()
        .enumerate()
    {
        ts.push(kit.term(x1, &format!("5{nm}"), key, (x + 16.0 * i as f64, 168.0)));
    }
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
    let m = kit.put("iec/motors/motor_3ph", (x, 224.0), &format!("{FLD}-M5"));
    kit.describe(&format!("{FLD}-M5"), "Filler pump 2.2 kW");
    kit.props(&format!("{FLD}-M5"), &[("power_kw", 2.2)]);
    let mut mw = Vec::new();
    let fan_in: [&[(f64, f64)]; 4] = [
        &[],
        &[(x + 16.0, 176.0), (x + 8.0, 176.0)],
        &[(x + 32.0, 180.0), (x + 16.0, 180.0)],
        &[(x + 48.0, 184.0), (x + 24.0, 184.0)],
    ];
    for (i, pin) in ["U1", "V1", "W1", "PE"].into_iter().enumerate() {
        mw.push(kit.wire_via((ts[i], "2"), (m, pin), fan_in[i]));
        kit.set_mm2(*mw.last().expect("a wire"), 1.5);
    }
    let t_sh = kit.term(x1, "5SH", b::TERM_PE, (x + 64.0, 168.0));
    let f_sh = kit.feed_pe(p.pe, (x + 64.0, 148.0));
    kit.wire_mm2((f_sh, "1"), (t_sh, "1"), 1.5);
    mw.push(kit.wire_via(
        (t_sh, "2"),
        (m, "PE"),
        &[(x + 64.0, 206.0), (x + 24.0, 206.0)],
    ));
    let c = kit.cable(
        "=FIL-W5",
        &["U", "V", "W", "GNYE", "SH"],
        &mw,
        1.5,
        16.0,
        "Filler pump motor cable, screened",
    )?;
    kit.tx.update::<novo_core::Cable>(c, |c| {
        if let Some(shield) = c.cores.last_mut() {
            shield.shield = true;
        }
    })?;

    // ----- page 4: emergency stop relay and the drive's run command ------------------------
    kit.on(safety);
    let mut c = kit.rung(p24i, 40.0);
    kit.series(&mut c, "iec/operators/emergency_stop", &format!("{OP}-S0"));
    kit.series(&mut c, "iec/coils/relay", &format!("{CAB}-K9"));
    kit.close(&c, p.p0);
    kit.describe(&format!("{OP}-S0"), "Emergency stop");
    kit.describe(&format!("{CAB}-K9"), "Emergency stop relay");
    let run = kit.tx.add_potential("U5:DI1", PotentialKind::Signal)?;
    let mut c = kit.rung(p.p24, 100.0);
    kit.series(&mut c, "iec/contacts/no", &format!("{CAB}-K10"));
    kit.close(&c, run);
    kit.describe(&format!("{CAB}-K10"), "Filler pump run relay");

    // ----- page 5: CPU, card supplies and fieldbus ---------------------------------------
    kit.on(plc);
    let fa = kit.feed(p24p, (40.0, 24.0));
    let fb = kit.feed(p.p0, (48.0, 24.0));
    let fc = kit.feed_pe(p.pe, (56.0, 24.0));
    let cpu = kit.put("iec/plc/cpu", (40.0, 60.0), &format!("{CAB}-A1"));
    kit.describe(&format!("{CAB}-A1"), "PLC CPU");
    kit.wire_mm2((fa, "1"), (cpu, "L+"), 1.5);
    kit.wire_mm2((fb, "1"), (cpu, "M"), 1.5);
    kit.wire_mm2((fc, "1"), (cpu, "PE"), 1.5);
    for (i, (name, pin, sig)) in [
        ("A", "A", "BUS.A"),
        ("B", "B", "BUS.B"),
        ("SH", "SH", "BUS.SH"),
    ]
    .into_iter()
    .enumerate()
    {
        let t = kit.term(x4, name, TERM, (40.0 + 8.0 * i as f64, 112.0));
        kit.wire_mm2((cpu, pin), (t, "1"), 0.75);
        let s = kit.tx.add_potential(sig, PotentialKind::Signal)?;
        let f = kit.feed_down(s, (40.0 + 8.0 * i as f64, 140.0));
        kit.wire_mm2((t, "2"), (f, "1"), 0.75);
    }
    for (i, pin) in ["P1", "P2"].into_iter().enumerate() {
        let s = kit
            .tx
            .add_potential(format!("ETH{}", i + 1), PotentialKind::Signal)?;
        let f = kit.feed(s, (96.0, 60.0 + 12.0 * i as f64));
        kit.wire_via(
            (cpu, pin),
            (f, "1"),
            &[
                (88.0, 60.0 + 12.0 * i as f64 + 6.0),
                (96.0, 60.0 + 12.0 * i as f64 + 6.0),
            ],
        );
    }
    for (i, (card, pot)) in [("A2", p24i), ("A3", p24o), ("A4", p24i), ("A5", p24i)]
        .into_iter()
        .enumerate()
    {
        let x = 150.0 + 40.0 * i as f64;
        let f1 = kit.feed(pot, (x, 24.0));
        let f2 = kit.feed(p.p0, (x + 8.0, 24.0));
        let s = kit.put("iec/plc/power_supply", (x, 56.0), &format!("{CAB}-{card}"));
        kit.wire_mm2((f1, "1"), (s, "L+"), 1.0);
        kit.wire_mm2((f2, "1"), (s, "M"), 1.0);
    }

    // ----- pages 6 to 8: digital inputs -----------------------------------------------------------
    type Input = (&'static str, &'static str, &'static str, &'static str);
    let panel: [Input; 6] = [
        ("iec/operators/pushbutton_no", "S1", "Start", OP),
        ("iec/operators/pushbutton_nc", "S2", "Stop", OP),
        ("iec/operators/pushbutton_no", "S3", "Reset", OP),
        ("iec/operators/selector_no", "S4", "Auto", OP),
        ("iec/operators/keyswitch_no", "S5", "Maintenance", OP),
        ("iec/contacts/no", "K9", "Emergency stop ok", CAB),
    ];
    let machine: [Input; 5] = [
        ("iec/sensors/proximity_no", "B1", "Bottle at infeed", FLD),
        ("iec/sensors/proximity_no", "B2", "Bottle at filler", FLD),
        ("iec/sensors/proximity_no", "B3", "Bottle at capper", FLD),
        ("iec/sensors/float_nc", "B4", "Product tank level low", FLD),
        ("iec/sensors/pressure_nc", "B5", "Air pressure ok", FLD),
    ];
    let protection: [Input; 5] = [
        ("iec/contacts/no", "Q1", "Infeed belt protection ok", CAB),
        ("iec/contacts/no", "Q2", "Outfeed belt protection ok", CAB),
        ("iec/contacts/no", "Q3", "Capper protection ok", CAB),
        ("iec/contacts/no", "Q4", "Labeller protection ok", CAB),
        ("iec/contacts/no", "Q5", "Filler drive protection ok", CAB),
    ];
    let plan: [(_, &[Input], _, _, _, _, bool); 3] = [
        (di_a, &panel, card_di1, x5, "", "", false),
        (
            di_b,
            &machine,
            card_di1,
            x2,
            "=FIL-W6",
            "Machine sensor cable 10x0.75",
            false,
        ),
        (di_c, &protection, card_di2, x5, "", "", true),
    ];
    for (page, inputs, card, strip, cable, text, direct) in plan {
        kit.on(page);
        let mut wires: Vec<WireId> = Vec::new();
        for (i, (symbol, name, what, loc)) in inputs.iter().enumerate() {
            let x = 40.0 + 36.0 * i as f64;
            let device = format!("{loc}-{name}");
            if direct {
                // Auxiliary contacts of the motor protection breakers sit in
                // the same cabinet: 24 V, the contact, the input channel.
                let f = kit.feed(p24i, (x, 24.0));
                let d = kit.put(symbol, (x, 44.0), &device);
                kit.link(End::Pin((f, "1")), End::Idx(d, 0), &[]);
                let y = pin_at(&kit, d, 1).1;
                to_channel(
                    &mut kit,
                    card,
                    (d, 1),
                    ChannelType::DigitalInput,
                    "iec/plc/digital_input",
                    (x, y + 16.0),
                    Rotation::R0,
                )?;
                kit.describe(&device, what);
                continue;
            }
            let w = input_column(&mut kit, card, strip, p24i, x, symbol, &device, name)?;
            kit.describe(&device, what);
            if *loc == FLD {
                wires.extend(w);
            }
        }
        if !cable.is_empty() {
            let cores = names_of(wires.len());
            let names: Vec<&str> = cores.iter().map(String::as_str).collect();
            kit.cable(cable, &names, &wires, 0.75, 22.0, text)?;
        }
    }

    // ----- page 9: digital outputs ---------------------------------------------------------------
    kit.on(dq);
    type Output = (&'static str, &'static str, &'static str, &'static str);
    let outputs: [Output; 8] = [
        ("iec/coils/contactor", "K1", "Infeed belt contactor", CAB),
        ("iec/coils/contactor", "K2", "Outfeed belt contactor", CAB),
        ("iec/coils/contactor", "K3", "Capper contactor", CAB),
        ("iec/coils/contactor", "K4", "Labeller contactor", CAB),
        (
            "iec/coils/contactor",
            "K5",
            "Filler pump line contactor",
            CAB,
        ),
        ("iec/coils/relay", "K10", "Filler pump run relay", CAB),
        ("iec/coils/solenoid_valve", "Y1", "Filler valve", FLD),
        ("iec/signalling/lamp_green", "H1", "Line running", OP),
    ];
    let mut wires = Vec::new();
    for (i, (symbol, name, what, loc)) in outputs.iter().enumerate() {
        let x = 40.0 + 36.0 * i as f64;
        let device = format!("{loc}-{name}");
        let first = if *loc == FLD {
            let t1 = kit.term(x2, &format!("{name}.1"), TERM, (x, 64.0));
            let y = pin_at(&kit, t1, 1).1 + 20.0;
            let d = kit.put(symbol, (x, y), &device);
            wires.push(kit.link(End::Pin((t1, "2")), End::Idx(d, 0), &[]));
            let y = pin_at(&kit, d, 1).1 + 20.0;
            let t2 = kit.term(x2, &format!("{name}.2"), TERM, (x, y));
            wires.push(kit.link(End::Idx(d, 1), End::Pin((t2, "1")), &[]));
            let y = pin_at(&kit, t2, 1).1;
            let f = kit.feed_down(p.p0, (x, y + 20.0));
            kit.link(End::Pin((t2, "2")), End::Pin((f, "1")), &[]);
            t1
        } else {
            let d = kit.put(symbol, (x, 64.0), &device);
            let y = pin_at(&kit, d, 1).1;
            let f = kit.feed_down(p.p0, (x, y + 20.0));
            kit.link(End::Idx(d, 1), End::Pin((f, "1")), &[]);
            d
        };
        let (_, y) = pin_at(&kit, first, 0);
        to_channel(
            &mut kit,
            card_dq,
            (first, 0),
            ChannelType::DigitalOutput,
            "iec/plc/digital_output",
            (x, y - 24.0),
            Rotation::R0,
        )?;
        kit.describe(&device, what);
    }
    kit.cable(
        "=FIL-W7",
        &["1", "2"],
        &wires,
        0.75,
        12.0,
        "Filler valve cable 2x0.75",
    )?;

    // ----- page 10: analog inputs -------------------------------------------------------------------
    kit.on(ai);
    let mut wires = Vec::new();
    for (i, (name, what)) in [("B6", "Product tank level"), ("B7", "Filler pressure")]
        .into_iter()
        .enumerate()
    {
        let x = 60.0 + 80.0 * i as f64;
        let device = format!("{FLD}-{name}");
        let f = kit.feed(p24i, (x, 24.0));
        let t1 = kit.term(x3, &format!("{name}+"), TERM, (x, 44.0));
        kit.link(End::Pin((f, "1")), End::Pin((t1, "1")), &[]);
        let y = pin_at(&kit, t1, 1).1 + 24.0;
        let tr = kit.put("iec/sensors/transmitter", (x, y), &device);
        kit.describe(&device, what);
        wires.push(kit.link(End::Pin((t1, "2")), End::Pin((tr, "+")), &[]));
        let (px, py) = pin_at(&kit, tr, 1);
        let t2 = kit.term(x3, &format!("{name}-"), TERM, (x + 24.0, py + 24.0));
        wires.push(kit.link(
            End::Pin((tr, "-")),
            End::Pin((t2, "1")),
            &[(px, py - 8.0), (x + 24.0, py - 8.0)],
        ));
        let (_, ty) = pin_at(&kit, t2, 1);
        to_channel(
            &mut kit,
            card_ai,
            (t2, 1),
            ChannelType::AnalogInput,
            "iec/plc/analog_input",
            (x + 24.0, ty + 16.0),
            Rotation::R0,
        )?;
    }
    kit.cable(
        "=FIL-W8",
        &["1", "2", "3", "4"],
        &wires,
        0.75,
        18.0,
        "Transmitter cable 4x0.75",
    )?;
    let _ = Endpoint::pin;

    // ----- parts -----------------------------------------------------------------------------------------
    let generic: Vec<(String, &Article)> = vec![
        (format!("{CAB}-Q0"), &b::SWITCH_63),
        (format!("{CAB}-Q10"), &b::MCB2),
        (format!("{CAB}-G1"), &b::PSU_10),
        (format!("{CAB}-Q21"), &b::MCB1_DC),
        (format!("{CAB}-Q22"), &b::MCB1_DC),
        (format!("{CAB}-Q23"), &b::MCB1_DC),
        (format!("{CAB}-Q1"), &b::MPCB_4),
        (format!("{CAB}-Q2"), &b::MPCB_4),
        (format!("{CAB}-Q3"), &b::MPCB_2_5),
        (format!("{CAB}-Q4"), &b::MPCB_2_5),
        (format!("{CAB}-Q5"), &b::MPCB_6),
        (format!("{CAB}-K1"), &b::CONTACTOR_9),
        (format!("{CAB}-K2"), &b::CONTACTOR_9),
        (format!("{CAB}-K3"), &b::CONTACTOR_9),
        (format!("{CAB}-K4"), &b::CONTACTOR_9),
        (format!("{CAB}-K5"), &b::CONTACTOR_9),
        (format!("{CAB}-K9"), &b::SAFETY_RELAY),
        (format!("{CAB}-K10"), &b::RELAY),
        (format!("{CAB}-U5"), &b::DRIVE_3KW),
        (format!("{CAB}-A1"), &b::PLC_CPU),
        (format!("{OP}-S0"), &b::ESTOP),
        (format!("{OP}-S1"), &b::BTN_NO),
        (format!("{OP}-S2"), &b::BTN_NC),
        (format!("{OP}-S3"), &b::BTN_NO),
        (format!("{OP}-S4"), &b::SELECTOR),
        (format!("{OP}-S5"), &b::KEYSWITCH),
        (format!("{OP}-H1"), &b::LAMP_GN),
        (format!("{FLD}-M1"), &b::MOTOR_1_1),
        (format!("{FLD}-M2"), &b::MOTOR_1_1),
        (format!("{FLD}-M3"), &b::MOTOR_0_75),
        (format!("{FLD}-M4"), &b::MOTOR_0_75),
        (format!("{FLD}-M5"), &b::MOTOR_2_2),
        (format!("{FLD}-B1"), &b::PROX_NO),
        (format!("{FLD}-B2"), &b::PROX_NO),
        (format!("{FLD}-B3"), &b::PROX_NO),
        (format!("{FLD}-B4"), &b::FLOAT_NC),
        (format!("{FLD}-B5"), &b::PRESSURE_SWITCH),
        (format!("{FLD}-B6"), &b::TRANSMITTER),
        (format!("{FLD}-B7"), &b::TRANSMITTER),
        (format!("{FLD}-Y1"), &b::VALVE),
    ];
    for (t, a) in generic {
        let d = kit.device(&t);
        parts.generic(kit.tx, d, a)?;
    }
    for (strip, count) in [(x0, 5usize), (x1, 21), (x2, 14), (x3, 4), (x4, 3), (x5, 12)] {
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
            "=FIL-W0" => parts.generic_part(kit.tx, &b::CABLE_SUPPLY)?,
            "=FIL-W1" | "=FIL-W2" | "=FIL-W3" | "=FIL-W4" => {
                parts.generic_part(kit.tx, &b::CABLE_MOTOR)?
            }
            "=FIL-W5" => parts.generic_part(kit.tx, &b::CABLE_MOTOR_SCREENED)?,
            "=FIL-W6" => parts.generic_part(kit.tx, &b::CABLE_10)?,
            "=FIL-W7" => parts.generic_part(kit.tx, &b::CABLE_SENSOR)?,
            _ => parts.generic_part(kit.tx, &b::CABLE_SENSOR_4)?,
        };
        kit.tx
            .update::<novo_core::Cable>(id, |c| c.part = Some(part))?;
    }
    Ok(())
}
