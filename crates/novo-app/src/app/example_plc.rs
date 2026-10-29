//! The PLC cabinet example: a 24 V supply, a CPU with a 16 input, an 8 output
//! and a 4 analog input card, and the field wiring of a conveyor and mixing
//! line through terminal strips, over eight pages and a laid-out cabinet.
//! Every channel is drawn with the editor's own `to plc` command, so the I/O
//! list, the card addresses and the generated pages are the real thing.
//! Illustrative: sizes and ratings are planning values, not a verified design.

use novo_core::{
    DeviceId, Document, Endpoint, Placement, PotentialKind, Result as ModelResult, Rotation,
    SymbolId, Tx, Value, WireId,
};
use novo_plc::ChannelType;
use novo_plc::commands::{NewCardLayout, add_card, to_plc};
use novo_symbols::{Library, Pack};

use super::example_kit::{Article, End, Finish, Kit, Parts, add_page, enclosure, style_strip, tag};

/// What the project is called.
pub(crate) const PLC_EXAMPLE_NAME: &str = "PLC cabinet (example)";

const DESCRIPTION: &str = "PLC cabinet for a conveyor and mixing line: 230 V incoming, 24 V DC \
    supply with three protected branches, a CPU, a 16 input, an 8 output and a 4 analog input \
    card with the field wiring on terminal strips, a laid-out cabinet with heat and parts list. \
    Ratings, parts without a maker and the supply are planning values to replace with your own.";

const CAB: &str = "=PLC+CAB";
const FLD: &str = "=PLC+FLD";
const OP: &str = "=PLC+OP";

const MCB2: Article = Article {
    order: "MCB-2P-B10",
    description: "Miniature circuit breaker 2 pole, B 10 A, 6 kA",
    size: (36.0, 90.0, 70.0),
    loss_w: 3.0,
    props: &[("rated_current_a", 10.0), ("poles", 2.0)],
};
const MCB1_DC: Article = Article {
    order: "MCB-1P-C2-DC",
    description: "Miniature circuit breaker 1 pole, C 2 A, 24 V DC",
    size: (18.0, 90.0, 70.0),
    loss_w: 0.5,
    props: &[("rated_current_a", 2.0), ("poles", 1.0)],
};
const PSU: Article = Article {
    order: "PSU-24V-10A",
    description: "Power supply 24 V DC, 10 A, 230 V AC input, DIN rail",
    size: (60.0, 125.0, 120.0),
    loss_w: 25.0,
    props: &[("rated_current_a", 10.0)],
};
const CPU: Article = Article {
    order: "PLC-CPU-24VDC",
    description: "PLC CPU 24 V DC, Ethernet, serial fieldbus",
    size: (45.0, 100.0, 75.0),
    loss_w: 6.0,
    props: &[],
};
const RELAY_SAFETY: Article = Article {
    order: "SAFETY-RELAY-24VDC",
    description: "Emergency stop safety relay, 24 V DC, 3 safety contacts",
    size: (22.5, 100.0, 120.0),
    loss_w: 2.0,
    props: &[],
};
const PUSHBUTTON: Article = Article {
    order: "PB-22MM-NO-24V",
    description: "Pushbutton 22 mm, 1 NO",
    size: (30.0, 30.0, 40.0),
    loss_w: 0.0,
    props: &[],
};
const PUSHBUTTON_NC: Article = Article {
    order: "PB-22MM-NC-24V",
    description: "Pushbutton 22 mm, 1 NC",
    size: (30.0, 30.0, 40.0),
    loss_w: 0.0,
    props: &[],
};
const SELECTOR: Article = Article {
    order: "SELECTOR-22MM-2POS",
    description: "Selector switch 22 mm, 2 position, 1 NO",
    size: (30.0, 30.0, 40.0),
    loss_w: 0.0,
    props: &[],
};
const KEYSWITCH: Article = Article {
    order: "KEYSWITCH-22MM-2POS",
    description: "Key switch 22 mm, 2 position, 1 NO",
    size: (30.0, 30.0, 40.0),
    loss_w: 0.0,
    props: &[],
};
const LAMP_GREEN: Article = Article {
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
const CONTACTOR: Article = Article {
    order: "CONT-3P-9A-24VDC",
    description: "Contactor 3 pole, AC-3 9 A, coil 24 V DC",
    size: (45.0, 85.0, 93.0),
    loss_w: 2.5,
    props: &[("ac3_current_a", 9.0), ("poles", 3.0)],
};
const PROXIMITY: Article = Article {
    order: "PROX-M18-PNP-NO",
    description: "Inductive proximity sensor M18, PNP, 1 NO",
    size: (18.0, 18.0, 60.0),
    loss_w: 0.0,
    props: &[],
};
const FLOW: Article = Article {
    order: "FLOW-SWITCH-NO",
    description: "Flow switch, 1 NO",
    size: (40.0, 40.0, 90.0),
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
const PRESSURE_SWITCH: Article = Article {
    order: "PS-NC-6BAR",
    description: "Pressure switch 0.5 to 6 bar, 1 NC",
    size: (40.0, 40.0, 80.0),
    loss_w: 0.0,
    props: &[],
};
const THERMISTOR: Article = Article {
    order: "THERMO-NC-120C",
    description: "Thermal contact in the motor winding, 1 NC, 120 C",
    size: (10.0, 10.0, 20.0),
    loss_w: 0.0,
    props: &[],
};
const VALVE: Article = Article {
    order: "VALVE-24VDC-2/2",
    description: "Solenoid valve 2/2 way, 24 V DC",
    size: (40.0, 60.0, 70.0),
    loss_w: 0.0,
    props: &[],
};
const TRANSMITTER: Article = Article {
    order: "TX-420MA-2W",
    description: "Transmitter 4 to 20 mA, 2 wire",
    size: (30.0, 30.0, 90.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_12: Article = Article {
    order: "CABLE-12X0.75",
    description: "Control cable 12x0.75 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_8: Article = Article {
    order: "CABLE-8X0.75",
    description: "Control cable 8x0.75 mm²",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};
const CABLE_4: Article = Article {
    order: "CABLE-4G0.75-SCREENED",
    description: "Signal cable 4x0.75 mm², screened",
    size: (0.0, 0.0, 0.0),
    loss_w: 0.0,
    props: &[],
};

/// A PLC card's catalog-like part: planning values and its channel layout.
fn card_part(
    tx: &mut Tx<'_>,
    order: &str,
    description: &str,
    channels: &str,
    loss_w: f64,
) -> ModelResult<novo_core::PartId> {
    let part = tx.add_part("Generic", order)?;
    tx.update::<novo_core::Part>(part, |p| {
        p.description = description.into();
        p.type_number = order.into();
        p.properties
            .insert("plc.channels".into(), Value::from(channels));
        for (key, value) in [("width_mm", 22.5), ("height_mm", 117.0), ("depth_mm", 75.0)] {
            p.properties.insert(key.into(), Value::Float(value));
        }
        p.properties
            .insert("power_loss_w".into(), Value::Float(loss_w));
        p.properties
            .insert("max_ambient_c".into(), Value::Float(60.0));
        p.properties.insert(
            novo_thermal::bridge::LOSS_SOURCE_PROPERTY.into(),
            Value::Text("estimated".into()),
        );
    })?;
    Ok(part)
}

/// Draws the example into a project that has only its empty first page.
pub(crate) fn build_plc_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
    let ((), changes) = doc.transact("Open example", |tx: &mut Tx<'_>| {
        let empty: Vec<_> = tx.project().pages().map(|p| p.id).collect();
        for page in empty {
            tx.delete_page(page)?;
        }
        draw(tx)?;
        super::example_kit::fill_sections(tx, 0.75)?;
        let (plate, enclosure) = enclosure((600.0, 760.0), (600.0, 800.0, 250.0));
        super::example_kit::finish(
            tx,
            &Finish {
                name: PLC_EXAMPLE_NAME,
                description: DESCRIPTION,
                cabinet: "+CAB",
                plate,
                enclosure,
                supply_v: 230.0,
                short_circuit_ka: 6.0,
            },
        )
    })?;
    Ok(changes)
}

/// Where a pin is on its page.
fn pin_at(kit: &Kit<'_, '_>, symbol: SymbolId, pin: u16) -> (f64, f64) {
    let at = kit
        .tx
        .project()
        .endpoint_position(Endpoint::pin(symbol, pin))
        .expect("the pin has a position");
    (at.x, at.y)
}

/// `to plc`: the next free channel of `kind` on `card`, drawn with its pin at
/// `pin` of `symbol` and wired to it. `origin` is where the channel symbol
/// goes, `rotation` how it is turned.
#[allow(clippy::too_many_arguments)]
fn to_channel(
    kit: &mut Kit<'_, '_>,
    card: DeviceId,
    signal: (SymbolId, u16),
    kind: ChannelType,
    key: &str,
    origin: (f64, f64),
    rotation: Rotation,
) -> ModelResult<()> {
    let library = Library::shared(Pack::Iec);
    let channel = library.get(key).expect("the channel symbol");
    let pins = novo_editor::catalog::model_pins(channel);
    let placement = Placement {
        origin: novo_core::Point::new(origin.0, origin.1),
        rotation,
        mirrored: false,
    };
    to_plc(
        kit.tx,
        library,
        card,
        Endpoint::pin(signal.0, signal.1),
        kind,
        key,
        placement,
        pins,
    )?;
    Ok(())
}

/// One digital input column at `x`: 24 V, a terminal, the contact in the
/// field, a terminal back and the DI channel under it. Returns the two wires
/// that go out to the field (they run in a cable).
#[allow(clippy::too_many_arguments)]
fn input_column(
    kit: &mut Kit<'_, '_>,
    card: DeviceId,
    strip: DeviceId,
    feed: novo_core::PotentialId,
    x: f64,
    symbol: &str,
    device: &str,
    name: &str,
) -> ModelResult<[WireId; 2]> {
    let term = "iec/terminals/terminal";
    let f = kit.feed(feed, (x, 24.0));
    let t1 = kit.term(strip, &format!("{name}.1"), term, (x, 44.0));
    kit.link(End::Pin((f, "1")), End::Pin((t1, "1")), &[]);
    let y = pin_at(kit, t1, 1).1 + 20.0;
    let dev = kit.put(symbol, (x, y), device);
    let out = kit.link(End::Pin((t1, "2")), End::Idx(dev, 0), &[]);
    let y = pin_at(kit, dev, 1).1 + 20.0;
    let t2 = kit.term(strip, &format!("{name}.2"), term, (x, y));
    let back = kit.link(End::Idx(dev, 1), End::Pin((t2, "1")), &[]);
    let (_, y) = pin_at(kit, t2, 1);
    to_channel(
        kit,
        card,
        (t2, 1),
        ChannelType::DigitalInput,
        "iec/plc/digital_input",
        (x, y + 16.0),
        Rotation::R0,
    )?;
    Ok([out, back])
}

fn draw(tx: &mut Tx<'_>) -> ModelResult<()> {
    let mut parts = Parts::default();
    let l = tx.add_potential("L", PotentialKind::Line)?;
    let n = tx.add_potential("N", PotentialKind::Neutral)?;
    let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
    let p24 = tx.add_potential("+24V", PotentialKind::DcPositive)?;
    let p0 = tx.add_potential("0V", PotentialKind::DcNegative)?;
    let p24p = tx.add_potential("+24V.PLC", PotentialKind::DcPositive)?;
    let p24i = tx.add_potential("+24V.IN", PotentialKind::DcPositive)?;
    let p24o = tx.add_potential("+24V.OUT", PotentialKind::DcPositive)?;

    let supply = add_page(tx, "1", "Incoming supply and 24 V", CAB)?;
    let plc = add_page(tx, "2", "PLC and cards", CAB)?;
    let di_a = add_page(tx, "3", "Digital inputs: operator panel", CAB)?;
    let di_b = add_page(tx, "4", "Digital inputs: machine", CAB)?;
    let dq = add_page(tx, "5", "Digital outputs", CAB)?;
    let ai = add_page(tx, "6", "Analog inputs", CAB)?;

    let x0 = tx.add_terminal_strip(tag(&format!("{CAB}-X0")))?;
    let x1 = tx.add_terminal_strip(tag(&format!("{CAB}-X1")))?;
    let x2 = tx.add_terminal_strip(tag(&format!("{CAB}-X2")))?;
    let x3 = tx.add_terminal_strip(tag(&format!("{CAB}-X3")))?;
    let x4 = tx.add_terminal_strip(tag(&format!("{CAB}-X4")))?;
    let term = "iec/terminals/terminal";
    let term_pe = "iec/terminals/terminal_pe";
    let term_n = "iec/terminals/terminal_n";

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
    let card_di = add_card(
        tx,
        tag(&format!("{CAB}-A2")),
        "Digital input card",
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

    let mut kit = Kit::new(tx, supply);

    // ----- page 1: incoming 230 V and the 24 V branches ----------------------
    let mut tb = Vec::new();
    for (i, (name, key)) in [("L", term), ("N", term_n), ("PE", term_pe)]
        .into_iter()
        .enumerate()
    {
        tb.push(kit.term(x0, name, key, (40.0 + 16.0 * i as f64, 40.0)));
    }
    for (i, pot) in [l, n].into_iter().enumerate() {
        let f = kit.feed(pot, (40.0 + 16.0 * i as f64, 12.0));
        kit.wire_mm2((f, "1"), (tb[i], "1"), 2.5);
    }
    let f = kit.feed_pe(pe, (72.0, 12.0));
    kit.wire_mm2((f, "1"), (tb[2], "1"), 2.5);
    let fpe_down = kit.feed_down(pe, (72.0, 56.0));
    kit.wire_mm2((tb[2], "2"), (fpe_down, "1"), 2.5);
    let q1 = kit.put("iec/protection/mcb_2p", (40.0, 70.0), &format!("{CAB}-Q1"));
    kit.describe(&format!("{CAB}-Q1"), "Incoming breaker");
    let w = kit.wire_via((tb[0], "2"), (q1, "1"), &[]);
    kit.set_mm2(w, 2.5);
    let w = kit.wire_via((tb[1], "2"), (q1, "3"), &[(56.0, 52.0), (48.0, 52.0)]);
    kit.set_mm2(w, 2.5);
    let g1 = kit.put(
        "iec/supplies/power_supply_dc",
        (40.0, 116.0),
        &format!("{CAB}-G1"),
    );
    kit.describe(&format!("{CAB}-G1"), "24 V DC power supply");
    kit.wire_mm2((q1, "2"), (g1, "L"), 2.5);
    kit.wire_mm2((q1, "4"), (g1, "N"), 2.5);
    let fpe = kit.feed_pe(pe, (56.0, 92.0));
    kit.wire_mm2((fpe, "1"), (g1, "PE"), 2.5);
    let f24 = kit.feed_down(p24, (40.0, 148.0));
    let f0 = kit.feed_down(p0, (48.0, 148.0));
    kit.wire((g1, "L+"), (f24, "1"));
    kit.wire((g1, "M"), (f0, "1"));
    // Three protected 24 V branches: the PLC, the inputs and the outputs.
    for (i, (pot, what)) in [(p24p, "PLC"), (p24i, "inputs"), (p24o, "outputs")]
        .into_iter()
        .enumerate()
    {
        let x = 140.0 + 24.0 * i as f64;
        let f = kit.feed(p24, (x, 24.0));
        let q = kit.put(
            "iec/protection/mcb_1p",
            (x, 52.0),
            &format!("{CAB}-Q1{}", i + 1),
        );
        kit.describe(
            &format!("{CAB}-Q1{}", i + 1),
            &format!("24 V branch: {what}"),
        );
        kit.wire_mm2((f, "1"), (q, "1"), 1.5);
        let out = kit.feed_down(pot, (x, 92.0));
        kit.wire_mm2((q, "2"), (out, "1"), 1.5);
    }

    // ----- page 2: CPU, card supplies and fieldbus ---------------------------
    kit.on(plc);
    let fa = kit.feed(p24p, (40.0, 24.0));
    let fb = kit.feed(p0, (48.0, 24.0));
    let fc = kit.feed_pe(pe, (56.0, 24.0));
    let cpu = kit.put("iec/plc/cpu", (40.0, 60.0), &format!("{CAB}-A1"));
    kit.describe(&format!("{CAB}-A1"), "PLC CPU");
    kit.wire_mm2((fa, "1"), (cpu, "L+"), 1.5);
    kit.wire_mm2((fb, "1"), (cpu, "M"), 1.5);
    kit.wire_mm2((fc, "1"), (cpu, "PE"), 1.5);
    // Fieldbus on terminal strip X4: A, B and the screen.
    let mut bus = Vec::new();
    for (i, (name, pin, sig)) in [
        ("A", "A", "BUS.A"),
        ("B", "B", "BUS.B"),
        ("SH", "SH", "BUS.SH"),
    ]
    .into_iter()
    .enumerate()
    {
        let t = kit.term(x4, name, term, (40.0 + 8.0 * i as f64, 112.0));
        kit.wire_mm2((cpu, pin), (t, "1"), 0.75);
        let s = kit.tx.add_potential(sig, PotentialKind::Signal)?;
        let f = kit.feed_down(s, (40.0 + 8.0 * i as f64, 140.0));
        kit.wire_mm2((t, "2"), (f, "1"), 0.75);
        bus.push(t);
    }
    // Ethernet ports to the network.
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
    // The cards' supplies: inputs, outputs and the analog card.
    for (i, (card, pot, label)) in [
        ("A2", p24i, "Digital input card"),
        ("A3", p24o, "Digital output card"),
        ("A4", p24i, "Analog input card"),
    ]
    .into_iter()
    .enumerate()
    {
        let x = 150.0 + 40.0 * i as f64;
        let f1 = kit.feed(pot, (x, 24.0));
        let f2 = kit.feed(p0, (x + 8.0, 24.0));
        let s = kit.put("iec/plc/power_supply", (x, 56.0), &format!("{CAB}-{card}"));
        kit.wire_mm2((f1, "1"), (s, "L+"), 1.0);
        kit.wire_mm2((f2, "1"), (s, "M"), 1.0);
        let _ = label;
    }

    // The emergency stop safety relay: its coil is on the 24 V branch of the
    // inputs, its contact is the "emergency stop ok" input of page 3.
    let mut c = kit.rung(p24i, 290.0);
    kit.series(&mut c, "iec/coils/contactor", &format!("{CAB}-K1"));
    kit.close(&c, p0);

    // ----- pages 3 and 4: digital inputs -------------------------------------
    let inputs_a: [(&str, &str, &str, &str); 6] = [
        ("iec/operators/pushbutton_no", "S1", "Start", OP),
        ("iec/operators/pushbutton_nc", "S2", "Stop", OP),
        ("iec/operators/pushbutton_no", "S3", "Reset", OP),
        ("iec/operators/selector_no", "S4", "Auto", OP),
        ("iec/operators/keyswitch_no", "S5", "Maintenance", OP),
        ("iec/contacts/no", "K1", "Emergency stop ok", CAB),
    ];
    let inputs_b: [(&str, &str, &str, &str); 6] = [
        (
            "iec/sensors/proximity_no",
            "B1",
            "Conveyor start position",
            FLD,
        ),
        (
            "iec/sensors/proximity_no",
            "B2",
            "Conveyor end position",
            FLD,
        ),
        ("iec/sensors/flow_no", "B3", "Water flow", FLD),
        ("iec/sensors/float_nc", "B4", "Tank level low", FLD),
        ("iec/sensors/pressure_nc", "B5", "Air pressure", FLD),
        (
            "iec/sensors/temperature_nc",
            "B8",
            "Mixer motor thermistor",
            FLD,
        ),
    ];
    for (page, inputs, cable, count) in [
        (di_a, &inputs_a, "=PLC-W1", "Operator panel cable 10x0.75"),
        (di_b, &inputs_b, "=PLC-W2", "Machine sensor cable 12x0.75"),
    ]
    .map(|(p, i, c, d)| (p, i, c, d))
    {
        kit.on(page);
        let mut wires = Vec::new();
        for (i, (symbol, name, what, loc)) in inputs.iter().enumerate() {
            let x = 40.0 + 36.0 * i as f64;
            let device = format!("{loc}-{name}");
            let w = input_column(&mut kit, card_di, x1, p24i, x, symbol, &device, name)?;
            kit.describe(&device, what);
            if *loc != CAB {
                wires.extend(w);
            }
        }
        let cores: Vec<String> = (1..=wires.len()).map(|i| i.to_string()).collect();
        let names: Vec<&str> = cores.iter().map(String::as_str).collect();
        kit.cable(cable, &names, &wires, 0.75, 25.0, count)?;
    }

    // ----- page 5: digital outputs -------------------------------------------
    kit.on(dq);
    let outputs: [(&str, &str, &str, &str); 6] = [
        ("iec/coils/contactor", "K21", "Conveyor contactor", CAB),
        ("iec/coils/contactor", "K22", "Mixer contactor", CAB),
        ("iec/coils/solenoid_valve", "Y1", "Water inlet valve", FLD),
        ("iec/coils/solenoid_valve", "Y2", "Drain valve", FLD),
        ("iec/signalling/lamp_green", "H1", "Line running", OP),
        ("iec/signalling/lamp_red", "H2", "Line fault", OP),
    ];
    let mut wires = Vec::new();
    for (i, (symbol, name, what, loc)) in outputs.iter().enumerate() {
        let x = 40.0 + 36.0 * i as f64;
        let device = format!("{loc}-{name}");
        let first = if *loc == CAB {
            let d = kit.put(symbol, (x, 64.0), &device);
            let y = pin_at(&kit, d, 1).1;
            let f = kit.feed_down(p0, (x, y + 20.0));
            kit.link(End::Idx(d, 1), End::Pin((f, "1")), &[]);
            d
        } else {
            let t1 = kit.term(x2, &format!("{name}.1"), term, (x, 64.0));
            let y = pin_at(&kit, t1, 1).1 + 20.0;
            let d = kit.put(symbol, (x, y), &device);
            wires.push(kit.link(End::Pin((t1, "2")), End::Idx(d, 0), &[]));
            let y = pin_at(&kit, d, 1).1 + 20.0;
            let t2 = kit.term(x2, &format!("{name}.2"), term, (x, y));
            wires.push(kit.link(End::Idx(d, 1), End::Pin((t2, "1")), &[]));
            let y = pin_at(&kit, t2, 1).1;
            let f = kit.feed_down(p0, (x, y + 20.0));
            kit.link(End::Pin((t2, "2")), End::Pin((f, "1")), &[]);
            t1
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
    let cores: Vec<String> = (1..=wires.len()).map(|i| i.to_string()).collect();
    let names: Vec<&str> = cores.iter().map(String::as_str).collect();
    kit.cable(
        "=PLC-W3",
        &names,
        &wires,
        0.75,
        20.0,
        "Valves and lamps cable 8x0.75",
    )?;

    // ----- page 6: analog inputs ---------------------------------------------
    kit.on(ai);
    let mut wires = Vec::new();
    for (i, (name, what)) in [("B6", "Tank pressure"), ("B7", "Tank level")]
        .into_iter()
        .enumerate()
    {
        let x = 60.0 + 80.0 * i as f64;
        let device = format!("{FLD}-{name}");
        let f = kit.feed(p24i, (x, 24.0));
        let t1 = kit.term(x3, &format!("{name}+"), term, (x, 44.0));
        kit.link(End::Pin((f, "1")), End::Pin((t1, "1")), &[]);
        let y = pin_at(&kit, t1, 1).1 + 24.0;
        let tr = kit.put("iec/sensors/transmitter", (x, y), &device);
        kit.describe(&device, what);
        wires.push(kit.link(End::Pin((t1, "2")), End::Pin((tr, "+")), &[]));
        // The loop returns through a second terminal beside it, down to the
        // AI channel.
        let (px, py) = pin_at(&kit, tr, 1);
        let t2 = kit.term(x3, &format!("{name}-"), term, (x + 24.0, py + 24.0));
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
        let _ = f;
    }
    kit.cable(
        "=PLC-W4",
        &["1", "2", "3", "4"],
        &wires,
        0.75,
        18.0,
        "Transmitter cable 4x0.75",
    )?;

    // ----- parts --------------------------------------------------------------
    let dev = |kit: &Kit<'_, '_>, t: &str| kit.device(t);
    let generic: Vec<(String, &Article)> = vec![
        (format!("{CAB}-Q1"), &MCB2),
        (format!("{CAB}-Q11"), &MCB1_DC),
        (format!("{CAB}-Q12"), &MCB1_DC),
        (format!("{CAB}-Q13"), &MCB1_DC),
        (format!("{CAB}-G1"), &PSU),
        (format!("{CAB}-A1"), &CPU),
        (format!("{CAB}-K1"), &RELAY_SAFETY),
        (format!("{CAB}-K21"), &CONTACTOR),
        (format!("{CAB}-K22"), &CONTACTOR),
        (format!("{OP}-S1"), &PUSHBUTTON),
        (format!("{OP}-S2"), &PUSHBUTTON_NC),
        (format!("{OP}-S3"), &PUSHBUTTON),
        (format!("{OP}-S4"), &SELECTOR),
        (format!("{OP}-S5"), &KEYSWITCH),
        (format!("{OP}-H1"), &LAMP_GREEN),
        (format!("{OP}-H2"), &LAMP_RED),
        (format!("{FLD}-B1"), &PROXIMITY),
        (format!("{FLD}-B2"), &PROXIMITY),
        (format!("{FLD}-B3"), &FLOW),
        (format!("{FLD}-B4"), &FLOAT),
        (format!("{FLD}-B5"), &PRESSURE_SWITCH),
        (format!("{FLD}-B6"), &TRANSMITTER),
        (format!("{FLD}-B7"), &TRANSMITTER),
        (format!("{FLD}-B8"), &THERMISTOR),
        (format!("{FLD}-Y1"), &VALVE),
        (format!("{FLD}-Y2"), &VALVE),
    ];
    for (t, a) in generic {
        let d = dev(&kit, &t);
        parts.generic(kit.tx, d, a)?;
    }
    for (strip, count) in [(x0, 3usize), (x1, 24), (x2, 8), (x3, 4), (x4, 3)] {
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
            "=PLC-W1" | "=PLC-W2" => parts.generic_part(kit.tx, &CABLE_12)?,
            "=PLC-W3" => parts.generic_part(kit.tx, &CABLE_8)?,
            _ => parts.generic_part(kit.tx, &CABLE_4)?,
        };
        kit.tx
            .update::<novo_core::Cable>(id, |c| c.part = Some(part))?;
    }
    Ok(())
}
