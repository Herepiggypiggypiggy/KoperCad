//! A crane travel axis behind a frequency drive (lane L5a part 3 fixture): a radio with
//! forward step 1 (S1), forward step 2 (S2) and reverse (S3), relays KA1 (forward), KA2
//! (reverse) and KA3 (step 2) that feed the drive's inputs, and a forward end limit S9 in the
//! forward circuit. The motor carries the moving mass, the input devices and the limit
//! position as "Travel drive" properties.
//!
//! The values are made up for the fixture: a generic 1.5 kW axis, not any manufacturer's data.

use novo_core::{
    DeviceId, DeviceTag, Document, Endpoint, Page, PageId, Pin, Placement, Point, PotentialKind,
    Result, SymbolId, SymbolInstance, SymbolRole, Tx,
};

/// Handles to the travel project.
#[derive(Debug, Clone)]
pub struct Travel {
    pub page: PageId,
    pub motor: DeviceId,
    pub forward_step1: SymbolId,
    pub forward_step2: SymbolId,
    pub reverse: SymbolId,
    pub limit: SymbolId,
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

/// Builds the travel project as one undo step.
pub fn travel(doc: &mut Document) -> Result<Travel> {
    Ok(doc.transact("Draw travel drive", build)?.0)
}

fn build(tx: &mut Tx<'_>) -> Result<Travel> {
    tx.set_info(|i| i.name = "Travel drive".into())?;
    let page = tx.add_page("1")?;
    tx.update::<Page>(page, |p| p.title = "Travel drive".into())?;
    let l = tx.add_potential("L", PotentialKind::Line)?;
    let n = tx.add_potential("N", PotentialKind::Neutral)?;

    let ka1 = tx.add_device(tag("=A1+CAB-KA1"), "Drive forward")?;
    let ka2 = tx.add_device(tag("=A1+CAB-KA2"), "Drive reverse")?;
    let ka3 = tx.add_device(tag("=A1+CAB-KA3"), "Drive step 2")?;
    let m1 = tx.add_device(tag("=A1+FLD-M1"), "Travel motor")?;
    let s1 = tx.add_device(tag("=A1+RAD-S1"), "Forward, step 1")?;
    let s2 = tx.add_device(tag("=A1+RAD-S2"), "Forward, step 2")?;
    let s3 = tx.add_device(tag("=A1+RAD-S3"), "Reverse")?;
    let s9 = tx.add_device(tag("=A1+FLD-S9"), "Forward end limit")?;

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
    let nc = "iec/contacts/nc";
    let coil = "iec/coils/relay";

    // The motor, drawn as a symbol of its own so the badge has somewhere to sit.
    let motor_sym = tx.place_symbol(
        page,
        "iec/motors/motor_3ph",
        at(60.0, 60.0),
        pins(&[("U1", 0.0, 0.0), ("V1", 8.0, 0.0), ("W1", 16.0, 0.0)]),
        role(m1, true),
    )?;

    // Rung 1: KA1 = (S1 or S2) and S9 (NC end limit).
    let x = 190.0;
    let f = feed(tx, l, x, 20.0)?;
    let j_top = tx.add_junction(page, Point::new(x, 44.0))?;
    let b1 = sym(tx, btn, x - 12.0, 52.0, s1, true)?;
    let b2 = sym(tx, btn, x + 12.0, 52.0, s2, true)?;
    let j_bot = tx.add_junction(page, Point::new(x, 76.0))?;
    let limit = sym(tx, nc, x, 84.0, s9, true)?;
    let ka1_coil = sym(tx, coil, x, 112.0, ka1, true)?;
    let fnn = feed(tx, n, x, 136.0)?;
    tx.connect(Endpoint::pin(f, 0), Endpoint::junction(j_top))?;
    for b in [b1, b2] {
        tx.connect(Endpoint::junction(j_top), Endpoint::pin(b, 0))?;
        tx.connect(Endpoint::pin(b, 1), Endpoint::junction(j_bot))?;
    }
    tx.connect(Endpoint::junction(j_bot), Endpoint::pin(limit, 0))?;
    tx.connect(Endpoint::pin(limit, 1), Endpoint::pin(ka1_coil, 0))?;
    tx.connect(Endpoint::pin(ka1_coil, 1), Endpoint::pin(fnn, 0))?;

    // Rung 2: KA2 = S3.
    let x = 238.0;
    let f = feed(tx, l, x, 20.0)?;
    let b3 = sym(tx, btn, x, 52.0, s3, true)?;
    let ka2_coil = sym(tx, coil, x, 112.0, ka2, true)?;
    let fnn = feed(tx, n, x, 136.0)?;
    tx.connect(Endpoint::pin(f, 0), Endpoint::pin(b3, 0))?;
    tx.connect(Endpoint::pin(b3, 1), Endpoint::pin(ka2_coil, 0))?;
    tx.connect(Endpoint::pin(ka2_coil, 1), Endpoint::pin(fnn, 0))?;

    // Rung 3: KA3 = S2 (the same radio contact, drawn again).
    let x = 286.0;
    let f = feed(tx, l, x, 20.0)?;
    let b2b = sym(tx, btn, x, 52.0, s2, false)?;
    let ka3_coil = sym(tx, coil, x, 112.0, ka3, true)?;
    let fnn = feed(tx, n, x, 136.0)?;
    tx.connect(Endpoint::pin(f, 0), Endpoint::pin(b2b, 0))?;
    tx.connect(Endpoint::pin(b2b, 1), Endpoint::pin(ka3_coil, 0))?;
    tx.connect(Endpoint::pin(ka3_coil, 1), Endpoint::pin(fnn, 0))?;

    // The travel axis: a moving mass makes the motor one; the end limit sits in the forward rung.
    for (key, value) in [
        ("trav_mass_kg", 5000.0.into()),
        ("trav_in_fwd", "=A1+CAB-KA1".into()),
        ("trav_in_rev", "=A1+CAB-KA2".into()),
        ("trav_in_high", "=A1+CAB-KA3".into()),
        ("trav_limit_fwd", "=A1+FLD-S9".into()),
        ("trav_limit_fwd_m", 6.0.into()),
    ] {
        tx.set_property(m1, key, Some(value))?;
    }

    Ok(Travel {
        page,
        motor: m1,
        forward_step1: b1,
        forward_step2: b2,
        reverse: b3,
        limit,
        motor_sym,
    })
}
