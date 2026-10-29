//! An open-loop drive hoist (lane L2c-open fixture): a radio with up step 1 (S1) and up step 2 (S2),
//! relays KA1 (up), KA3 and KA4 (speed steps 2 and 3, both from S2) that feed the drive, and the
//! drive's brake relay contact KR2 switching the brake contactor KB. The motor carries the hook
//! load, the devices and a load sensing boost as "Drive hoist" properties. A light 200 kg hook
//! lets the load sensing run the top step above its nominal frequency.
//!
//! The values are made up for the fixture: a generic 5.5 kW hoist motor, not any manufacturer's data.

use novo_core::{
    DeviceId, DeviceTag, Document, Endpoint, Page, PageId, Pin, Placement, Point, PotentialKind,
    Result, SymbolId, SymbolInstance, SymbolRole, Tx,
};

/// Handles to the drive hoist project.
#[derive(Debug, Clone)]
pub struct DriveHoist {
    pub page: PageId,
    pub motor: DeviceId,
    pub up_step1: SymbolId,
    pub up_step2: SymbolId,
    pub motor_sym: SymbolId,
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

/// Builds the drive hoist project as one undo step.
pub fn drive_hoist(doc: &mut Document) -> Result<DriveHoist> {
    Ok(doc.transact("Draw drive hoist", |tx| build(tx, false))?.0)
}

/// The same project with an upper final limit (LS-F, normally closed, in the up rung) and an
/// upper slow-down switch (LS-S, normally closed, in the rungs of speed steps 2 and 3) that opens
/// 0.3 m before it: past the slow-down switch only step 1 runs, and the final limit stops the hoist.
pub fn drive_hoist_with_limits(doc: &mut Document) -> Result<DriveHoist> {
    Ok(doc
        .transact("Draw drive hoist with limits", |tx| {
            let h = build(tx, true)?;
            for (key, value) in [
                ("dh_limit_up", "=A1+LIM-LS-F"),
                ("dh_slow_up", "=A1+LIM-LS-S"),
            ] {
                tx.set_property(h.motor, key, Some(value.into()))?;
            }
            for (key, value) in [
                ("dh_limit_up_m", 1.1),
                ("dh_slow_up_m", 0.8),
                ("dh_clearance_up_m", 0.1),
            ] {
                tx.set_property(h.motor, key, Some(value.into()))?;
            }
            Ok(h)
        })?
        .0)
}

fn build(tx: &mut Tx<'_>, limits: bool) -> Result<DriveHoist> {
    tx.set_info(|i| i.name = "Drive hoist".into())?;
    let page = tx.add_page("1")?;
    tx.update::<Page>(page, |p| p.title = "Drive hoist".into())?;
    let l = tx.add_potential("L", PotentialKind::Line)?;
    let n = tx.add_potential("N", PotentialKind::Neutral)?;

    let ka1 = tx.add_device(tag("=A1+CAB-KA1"), "Drive up")?;
    let ka3 = tx.add_device(tag("=A1+CAB-KA3"), "Drive speed step 2")?;
    let ka4 = tx.add_device(tag("=A1+CAB-KA4"), "Drive speed step 3")?;
    let kr2 = tx.add_device(tag("=A1+CAB-KR2"), "Drive brake relay")?;
    let kb = tx.add_device(tag("=A1+CAB-KB"), "Brake contactor")?;
    let ls_f = limits
        .then(|| tx.add_device(tag("=A1+LIM-LS-F"), "Upper final limit"))
        .transpose()?;
    let ls_s = limits
        .then(|| tx.add_device(tag("=A1+LIM-LS-S"), "Upper slow-down switch"))
        .transpose()?;
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
    let btn = "iec/operators/pushbutton_no";
    let no = "iec/contacts/no";
    let coil = "iec/coils/relay";
    let nc = "iec/contacts/nc";

    let motor_sym = tx.place_symbol(
        page,
        "iec/motors/motor_3ph",
        at(60.0, 60.0),
        pins(&[("U1", 0.0, 0.0), ("V1", 8.0, 0.0), ("W1", 16.0, 0.0)]),
        role(m1, true),
    )?;

    // Rung 1: KA1 = S1 or S2.
    let x = 190.0;
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
    match ls_f {
        Some(ls) => {
            let limit = sym(tx, nc, x, 84.0, ls, true)?;
            tx.connect(Endpoint::junction(j_bot), Endpoint::pin(limit, 0))?;
            tx.connect(Endpoint::pin(limit, 1), Endpoint::pin(ka1_coil, 0))?;
        }
        None => {
            tx.connect(Endpoint::junction(j_bot), Endpoint::pin(ka1_coil, 0))?;
        }
    }
    tx.connect(Endpoint::pin(ka1_coil, 1), Endpoint::pin(fnn, 0))?;

    // Rungs 2 and 3: KA3 and KA4 = S2 (the same radio contact, drawn again).
    for (x, device, main_contact) in [(238.0, ka3, true), (286.0, ka4, false)] {
        let f = feed(tx, l, x, 20.0)?;
        let b = sym(tx, btn, x, 52.0, s2, false)?;
        let k = sym(tx, coil, x, 112.0, device, true)?;
        let fnn = feed(tx, n, x, 136.0)?;
        tx.connect(Endpoint::pin(f, 0), Endpoint::pin(b, 0))?;
        match ls_s {
            Some(ls) => {
                let limit = sym(tx, nc, x, 84.0, ls, main_contact)?;
                tx.connect(Endpoint::pin(b, 1), Endpoint::pin(limit, 0))?;
                tx.connect(Endpoint::pin(limit, 1), Endpoint::pin(k, 0))?;
            }
            None => {
                tx.connect(Endpoint::pin(b, 1), Endpoint::pin(k, 0))?;
            }
        }
        tx.connect(Endpoint::pin(k, 1), Endpoint::pin(fnn, 0))?;
    }

    // Rung 4: the drive's brake relay contact (written by the run) feeds the brake contactor KB.
    let x = 334.0;
    let f = feed(tx, l, x, 20.0)?;
    let relay = sym(tx, no, x, 52.0, kr2, true)?;
    let kb_coil = sym(tx, coil, x, 112.0, kb, true)?;
    let fnn = feed(tx, n, x, 136.0)?;
    tx.connect(Endpoint::pin(f, 0), Endpoint::pin(relay, 0))?;
    tx.connect(Endpoint::pin(relay, 1), Endpoint::pin(kb_coil, 0))?;
    tx.connect(Endpoint::pin(kb_coil, 1), Endpoint::pin(fnn, 0))?;

    for (key, value) in [
        ("dh_mass_kg", 200.0.into()),
        ("dh_in_up", "=A1+CAB-KA1".into()),
        ("dh_in_step2", "=A1+CAB-KA3".into()),
        ("dh_in_step3", "=A1+CAB-KA4".into()),
        ("dh_brake_relay", "=A1+CAB-KR2".into()),
        ("dh_brake_feed", "=A1+CAB-KB".into()),
        ("dh_sense_boost_hz", 80.0.into()),
    ] {
        tx.set_property(m1, key, Some(value))?;
    }

    Ok(DriveHoist {
        page,
        motor: m1,
        up_step1: b1,
        up_step2: b2,
        motor_sym,
    })
}
