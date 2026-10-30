//! A closed-loop (encoder) winch (lane L2c fixture): a radio with up step 1 (S1) and up step 2 (S2),
//! relays KA1 (up) and KA3 (speed step 2) that feed the drive, the two STO channel relays STOA and
//! STOB, the encoder signal monitor ENC, and the drive's brake relay contact KR2 switching the
//! brake contactor KB. The motor carries the hook load and the devices as "Closed-loop winch"
//! properties, so a defect injected on STOA, STOB, ENC or KB (Simulate > Defects and walk-throughs)
//! reaches the drive.
//!
//! `closed_winch_pair` adds a second winch M2 on one beam with M1: its own STO channels STOC and
//! STOD, encoder signal monitor ENCB and brake contactor KB2 behind the drive's brake relay KR3.
//! M1 names M2 as its partner, so the run takes them as one pair; the radio commands both.
//!
//! `closed_winch_channel` adds a monitor ENCH for encoder channel B as a rung under the first one (drawn as one more
//! relay fed from L and N), named by the motor's `cw_encoder_b`: opening its coil leaves the signal
//! device ENC alone, so the encoder keeps its counts but loses the direction.
//!
//! `closed_winch_load` adds a monitor LCEL for the load cell signal and a rated load: the winch then
//! watches its load and inhibits lifting above the cut-out.
//!
//! The values are made up for the fixture: a generic 30 kW hoist motor, not any manufacturer's data.

use novo_core::{
    DeviceId, DeviceTag, Document, Endpoint, Page, PageId, Pin, Placement, Point, PotentialKind,
    Result, SymbolId, SymbolInstance, SymbolRole, Tx,
};

/// Handles to the closed-loop winch project.
#[derive(Debug, Clone)]
pub struct ClosedWinch {
    pub page: PageId,
    pub motor: DeviceId,
    pub up_step1: SymbolId,
    pub up_step2: SymbolId,
    pub motor_sym: SymbolId,
    /// The second winch (device and motor symbol) of the pair fixture.
    pub second: Option<(DeviceId, SymbolId)>,
}

fn at(x: f64, y: f64) -> Placement {
    Placement::at(Point::new(x, y))
}

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("fixture tags are valid")
}

fn pins(list: &[(&str, f64, f64)]) -> Vec<Pin> {
    list.iter().map(|&(n, x, y)| Pin::new(n, x, y)).collect()
}

fn role(device: DeviceId, main: bool) -> SymbolRole {
    SymbolRole::Device { device, main }
}

/// Builds the closed-loop winch project as one undo step.
pub fn closed_winch(doc: &mut Document) -> Result<ClosedWinch> {
    Ok(doc
        .transact("Draw closed-loop winch", |tx| build(tx, false, Extra::None))?
        .0)
}

/// Builds the project with two winches on one beam as one undo step.
pub fn closed_winch_pair(doc: &mut Document) -> Result<ClosedWinch> {
    Ok(doc
        .transact("Draw two closed-loop winches", |tx| {
            build(tx, true, Extra::None)
        })?
        .0)
}

/// Builds the winch project with a monitor for encoder channel B as one undo step.
pub fn closed_winch_channel(doc: &mut Document) -> Result<ClosedWinch> {
    Ok(doc
        .transact("Draw closed-loop winch with encoder channel", |tx| {
            build(tx, false, Extra::Channel)
        })?
        .0)
}

/// Builds the winch project with a rated load and a load cell signal device LCEL as one undo step.
pub fn closed_winch_load(doc: &mut Document, swl_kg: f64) -> Result<ClosedWinch> {
    Ok(doc
        .transact("Draw closed-loop winch with load cell", |tx| {
            build(tx, false, Extra::Load(swl_kg))
        })?
        .0)
}

/// What the single winch gets besides its rungs: a second encoder channel monitor, or a rated
/// load and the load cell signal device.
#[derive(Clone, Copy, PartialEq)]
enum Extra {
    None,
    Channel,
    Load(f64),
}

fn build(tx: &mut Tx<'_>, pair: bool, extra: Extra) -> Result<ClosedWinch> {
    let title = if pair {
        "Two closed-loop winches"
    } else {
        "Closed-loop winch"
    };
    tx.set_info(|i| i.name = title.into())?;
    let page = tx.add_page("1")?;
    tx.update::<Page>(page, |p| p.title = title.into())?;
    let l = tx.add_potential("L", PotentialKind::Line)?;
    let n = tx.add_potential("N", PotentialKind::Neutral)?;

    let ka1 = tx.add_device(tag("=A1+CAB-KA1"), "Drive up")?;
    let ka3 = tx.add_device(tag("=A1+CAB-KA3"), "Drive speed step 2")?;
    let sto_a = tx.add_device(tag("=A1+CAB-STOA"), "STO channel 1")?;
    let sto_b = tx.add_device(tag("=A1+CAB-STOB"), "STO channel 2")?;
    let enc = tx.add_device(tag("=A1+CAB-ENC"), "Encoder signal monitor")?;
    let kr2 = tx.add_device(tag("=A1+CAB-KR2"), "Drive brake relay")?;
    let kb = tx.add_device(tag("=A1+CAB-KB"), "Brake contactor")?;
    let m1 = tx.add_device(tag("=A1+FLD-M1"), "Hoist motor")?;
    let s1 = tx.add_device(tag("=A1+RAD-S1"), "Up, step 1")?;
    let s2 = tx.add_device(tag("=A1+RAD-S2"), "Up, step 2")?;

    let feed = |tx: &mut Tx<'_>, potential, x, y| {
        tx.place_symbol(
            page,
            "iec/connectivity/potential_feed",
            at(x, y),
            pins(&[("1", 0.0, 8.0)]),
            SymbolRole::PotentialFeed { potential },
        )
    };
    let sym = |tx: &mut Tx<'_>, key: &str, x, y, device, main| {
        let (a, b) = match key {
            "iec/coils/relay" => ("A1", "A2"),
            "iec/operators/pushbutton_no" => ("13", "14"),
            _ => ("11", "12"),
        };
        let id = tx.place_symbol(
            page,
            key,
            at(x, y),
            pins(&[(a, 0.0, 0.0), (b, 0.0, 16.0)]),
            role(device, main),
        )?;
        if key != "iec/coils/relay" {
            tx.update::<SymbolInstance>(id, |s| s.pass_through = vec![vec![0, 1]])?;
        }
        Ok::<_, novo_core::ModelError>(id)
    };
    // Column `k` of the rungs. The pair has ten, so they are drawn closer to stay inside the frame.
    let col = |k: usize| {
        if pair {
            96.0 + 34.0 * k as f64
        } else {
            190.0 + 48.0 * k as f64
        }
    };
    let btn = "iec/operators/pushbutton_no";
    let no = "iec/contacts/no";
    let coil = "iec/coils/relay";

    // The pair's motors sit below the rungs, clear of the Live panel and its readings.
    let motor_at = if pair { (150.0, 185.0) } else { (60.0, 60.0) };
    let motor_sym = tx.place_symbol(
        page,
        "iec/motors/motor_3ph",
        at(motor_at.0, motor_at.1),
        pins(&[("U1", 0.0, 0.0), ("V1", 8.0, 0.0), ("W1", 16.0, 0.0)]),
        role(m1, true),
    )?;

    // Rung 1: KA1 = S1 or S2.
    let x = col(0);
    let f = feed(tx, l, x, 20.0)?;
    let j_top = tx.add_junction(page, Point::new(x, 44.0))?;
    let b1 = sym(tx, btn, x - 12.0, 52.0, s1, true)?;
    let b2 = sym(tx, btn, x + 12.0, 52.0, s2, true)?;
    let j_bot = tx.add_junction(page, Point::new(x, 76.0))?;
    let ka1_coil = sym(tx, coil, x, 112.0, ka1, true)?;
    let fnn = feed(tx, n, x, 136.0)?;
    tx.connect(Endpoint::pin(f, 0), Endpoint::junction(j_top))?;
    for b in [b1, b2] {
        tx.connect(Endpoint::junction(j_top), Endpoint::pin(b, 0))?;
        tx.connect(Endpoint::pin(b, 1), Endpoint::junction(j_bot))?;
    }
    tx.connect(Endpoint::junction(j_bot), Endpoint::pin(ka1_coil, 0))?;
    tx.connect(Endpoint::pin(ka1_coil, 1), Endpoint::pin(fnn, 0))?;

    // Rung 2: KA3 = S2 (the same radio contact, drawn again).
    {
        let x = col(1);
        let f = feed(tx, l, x, 20.0)?;
        let b = sym(tx, btn, x, 52.0, s2, false)?;
        let k = sym(tx, coil, x, 112.0, ka3, true)?;
        let fnn = feed(tx, n, x, 136.0)?;
        tx.connect(Endpoint::pin(f, 0), Endpoint::pin(b, 0))?;
        tx.connect(Endpoint::pin(b, 1), Endpoint::pin(k, 0))?;
        tx.connect(Endpoint::pin(k, 1), Endpoint::pin(fnn, 0))?;
    }
    // Rungs 3 to 5: the STO channels and the encoder signal monitor, each a relay fed from L and
    // N. A defect on one of them (open coil, lost supply) is what the drive sees.
    for (x, device) in [(col(2), sto_a), (col(3), sto_b), (col(4), enc)] {
        let f = feed(tx, l, x, 20.0)?;
        let k = sym(tx, coil, x, 112.0, device, true)?;
        let fnn = feed(tx, n, x, 136.0)?;
        tx.connect(Endpoint::pin(f, 0), Endpoint::pin(k, 0))?;
        tx.connect(Endpoint::pin(k, 1), Endpoint::pin(fnn, 0))?;
    }

    // Rung 6: the drive's brake relay contact (written by the run) feeds the brake contactor KB.
    let x = col(5);
    let f = feed(tx, l, x, 20.0)?;
    let relay = sym(tx, no, x, 52.0, kr2, true)?;
    let kb_coil = sym(tx, coil, x, 112.0, kb, true)?;
    let fnn = feed(tx, n, x, 136.0)?;
    tx.connect(Endpoint::pin(f, 0), Endpoint::pin(relay, 0))?;
    tx.connect(Endpoint::pin(relay, 1), Endpoint::pin(kb_coil, 0))?;
    tx.connect(Endpoint::pin(kb_coil, 1), Endpoint::pin(fnn, 0))?;

    // Encoder channel B: a relay of its own, one rung under the first one.
    if extra == Extra::Channel {
        let ench = tx.add_device(tag("=A1+CAB-ENCH"), "Encoder channel B monitor")?;
        let x = col(0);
        let f = feed(tx, l, x, 160.0)?;
        let k = sym(tx, coil, x, 206.0, ench, true)?;
        let fnn = feed(tx, n, x, 230.0)?;
        tx.connect(Endpoint::pin(f, 0), Endpoint::pin(k, 0))?;
        tx.connect(Endpoint::pin(k, 1), Endpoint::pin(fnn, 0))?;
        tx.set_property(m1, "cw_encoder_b", Some("=A1+CAB-ENCH".into()))?;
    }

    // The load cell: a relay of its own, energised while its signal is present.
    if let Extra::Load(swl_kg) = extra {
        let lcel = tx.add_device(tag("=A1+CAB-LCEL"), "Load cell signal monitor")?;
        let x = col(0);
        let f = feed(tx, l, x, 160.0)?;
        let k = sym(tx, coil, x, 206.0, lcel, true)?;
        let fnn = feed(tx, n, x, 230.0)?;
        tx.connect(Endpoint::pin(f, 0), Endpoint::pin(k, 0))?;
        tx.connect(Endpoint::pin(k, 1), Endpoint::pin(fnn, 0))?;
        tx.set_property(m1, "cw_load_cell", Some("=A1+CAB-LCEL".into()))?;
        tx.set_property(m1, "cw_swl_kg", Some(swl_kg.into()))?;
    }

    // The second winch: STO channels and encoder monitor as relays, the drive's brake relay
    // contact written by the run, the brake contactor behind it.
    let mut second = None;
    if pair {
        let m2 = tx.add_device(tag("=A1+FLD-M2"), "Hoist motor B")?;
        let sto_c = tx.add_device(tag("=A1+CAB-STOC"), "STO channel 1, winch B")?;
        let sto_d = tx.add_device(tag("=A1+CAB-STOD"), "STO channel 2, winch B")?;
        let enc_b = tx.add_device(tag("=A1+CAB-ENCB"), "Encoder signal monitor, winch B")?;
        let kr3 = tx.add_device(tag("=A1+CAB-KR3"), "Drive brake relay, winch B")?;
        let kb2 = tx.add_device(tag("=A1+CAB-KB2"), "Brake contactor, winch B")?;
        let motor_b = tx.place_symbol(
            page,
            "iec/motors/motor_3ph",
            at(150.0, 235.0),
            pins(&[("U1", 0.0, 0.0), ("V1", 8.0, 0.0), ("W1", 16.0, 0.0)]),
            role(m2, true),
        )?;
        for (x, device) in [(col(6), sto_c), (col(7), sto_d), (col(8), enc_b)] {
            let f = feed(tx, l, x, 20.0)?;
            let k = sym(tx, coil, x, 112.0, device, true)?;
            let fnn = feed(tx, n, x, 136.0)?;
            tx.connect(Endpoint::pin(f, 0), Endpoint::pin(k, 0))?;
            tx.connect(Endpoint::pin(k, 1), Endpoint::pin(fnn, 0))?;
        }
        let x = col(9);
        let f = feed(tx, l, x, 20.0)?;
        let relay = sym(tx, no, x, 52.0, kr3, true)?;
        let kb_coil = sym(tx, coil, x, 112.0, kb2, true)?;
        let fnn = feed(tx, n, x, 136.0)?;
        tx.connect(Endpoint::pin(f, 0), Endpoint::pin(relay, 0))?;
        tx.connect(Endpoint::pin(relay, 1), Endpoint::pin(kb_coil, 0))?;
        tx.connect(Endpoint::pin(kb_coil, 1), Endpoint::pin(fnn, 0))?;
        for (key, value) in [
            ("cw_mass_kg", 2500.0.into()),
            ("cw_sto_a", "=A1+CAB-STOC".into()),
            ("cw_sto_b", "=A1+CAB-STOD".into()),
            ("cw_encoder", "=A1+CAB-ENCB".into()),
            ("cw_brake_relay", "=A1+CAB-KR3".into()),
            ("cw_brake_feed", "=A1+CAB-KB2".into()),
        ] {
            tx.set_property(m2, key, Some(value))?;
        }
        for (key, value) in [
            ("cw_pair_partner", "=A1+FLD-M2".into()),
            ("cw_beam_mass_kg", 5000.0.into()),
            ("cw_beam_length_m", 2.0.into()),
        ] {
            tx.set_property(m1, key, Some(value))?;
        }
        second = Some((m2, motor_b));
    }

    for (key, value) in [
        ("cw_mass_kg", 2500.0.into()),
        ("cw_in_up", "=A1+CAB-KA1".into()),
        ("cw_in_fast", "=A1+CAB-KA3".into()),
        ("cw_sto_a", "=A1+CAB-STOA".into()),
        ("cw_sto_b", "=A1+CAB-STOB".into()),
        ("cw_encoder", "=A1+CAB-ENC".into()),
        ("cw_brake_relay", "=A1+CAB-KR2".into()),
        ("cw_brake_feed", "=A1+CAB-KB".into()),
    ] {
        tx.set_property(m1, key, Some(value))?;
    }

    Ok(ClosedWinch {
        page,
        motor: m1,
        up_step1: b1,
        up_step2: b2,
        motor_sym,
        second,
    })
}
