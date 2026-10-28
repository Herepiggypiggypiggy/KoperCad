//! A two-speed contactor hoist (lane L5a fixture): reversing pair KU/KD, speed pair KF (fine,
//! 12 poles) and KC (coarse, 2 poles) feeding a pole-change motor, a rocker pendant with
//! up fine S1, up coarse S2 and down S3, and an upper limit switch S9 in the coarse up circuit
//! only. The motor carries a hook load and the limit height as "Mechanical load" properties.
//!
//! The values are made up for the fixture: a generic hoist, not any manufacturer's data.

use novo_core::{
    DeviceId, DeviceTag, Document, Endpoint, Page, PageId, Pin, Placement, Point, PotentialKind,
    Result, SymbolId, SymbolInstance, SymbolRole, Tx,
};

/// Handles to the hoist project.
#[derive(Debug, Clone)]
pub struct Hoist {
    pub main_page: PageId,
    pub control_page: PageId,
    pub motor: DeviceId,
    pub up_fine: SymbolId,
    pub up_coarse: SymbolId,
    pub down: SymbolId,
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

fn three_pole() -> Vec<Pin> {
    pins(&[
        ("1", 0.0, 0.0),
        ("2", 0.0, 16.0),
        ("3", 8.0, 0.0),
        ("4", 8.0, 16.0),
        ("5", 16.0, 0.0),
        ("6", 16.0, 16.0),
    ])
}

fn two_pin(a: &str, b: &str) -> Vec<Pin> {
    pins(&[(a, 0.0, 0.0), (b, 0.0, 16.0)])
}

fn role(device: DeviceId, main: bool) -> SymbolRole {
    SymbolRole::Device { device, main }
}

/// Builds the hoist project as one undo step.
pub fn hoist(doc: &mut Document) -> Result<Hoist> {
    Ok(doc.transact("Draw two-speed hoist", build)?.0)
}

fn build(tx: &mut Tx<'_>) -> Result<Hoist> {
    tx.set_info(|i| i.name = "Two-speed hoist".into())?;
    let main_page = tx.add_page("1")?;
    // One page: the power circuit on the left, the pendant circuit on the right, so a held
    // pendant button and the motor badge are on screen together.
    let control_page = main_page;
    tx.update::<Page>(main_page, |p| p.title = "Two-speed hoist".into())?;
    let l1 = tx.add_potential("L1", PotentialKind::Line)?;
    let l2 = tx.add_potential("L2", PotentialKind::Line)?;
    let l3 = tx.add_potential("L3", PotentialKind::Line)?;
    let l = tx.add_potential("L", PotentialKind::Line)?;
    let n = tx.add_potential("N", PotentialKind::Neutral)?;

    let ku = tx.add_device(tag("=A1+CAB-KU"), "Up contactor")?;
    let kd = tx.add_device(tag("=A1+CAB-KD"), "Down contactor")?;
    let kf = tx.add_device(tag("=A1+CAB-KF"), "Fine (slow) contactor")?;
    let kc = tx.add_device(tag("=A1+CAB-KC"), "Coarse (fast) contactor")?;
    let m1 = tx.add_device(tag("=A1+FLD-M1"), "Hoist motor, 12/2 poles")?;
    let s1 = tx.add_device(tag("=A1+PEN-S1"), "Up, step 1")?;
    let s2 = tx.add_device(tag("=A1+PEN-S2"), "Up, step 2")?;
    let s3 = tx.add_device(tag("=A1+PEN-S3"), "Down")?;
    let s9 = tx.add_device(tag("=A1+FLD-S9"), "Upper limit switch")?;

    // ----- page 1: main circuit -------------------------------------------
    let feed = |tx: &mut Tx<'_>, page, potential, x, y| {
        tx.place_symbol(
            page,
            "iec/connectivity/potential_feed",
            at(x, y),
            pins(&[("1", 0.0, 8.0)]),
            SymbolRole::PotentialFeed { potential },
        )
    };
    let feeds = [
        feed(tx, main_page, l1, 40.0, 20.0)?,
        feed(tx, main_page, l2, 48.0, 20.0)?,
        feed(tx, main_page, l3, 56.0, 20.0)?,
    ];
    let main_3p = |tx: &mut Tx<'_>, x, y, device| {
        tx.place_symbol(
            main_page,
            "iec/contactors/main_3p",
            at(x, y),
            three_pole(),
            role(device, false),
        )
    };
    // The up contactor passes the phases straight, the down contactor swaps L2 and L3.
    let ku_main = main_3p(tx, 40.0, 44.0, ku)?;
    let kd_main = main_3p(tx, 80.0, 44.0, kd)?;
    let kf_main = main_3p(tx, 40.0, 84.0, kf)?;
    let kc_main = main_3p(tx, 80.0, 84.0, kc)?;
    let motor_sym = tx.place_symbol(
        main_page,
        "iec/motors/motor_3ph_dahlander",
        at(60.0, 132.0),
        pins(&[
            ("1U", 0.0, 0.0),
            ("1V", 8.0, 0.0),
            ("1W", 16.0, 0.0),
            ("2U", 0.0, 32.0),
            ("2V", 8.0, 32.0),
            ("2W", 16.0, 32.0),
            ("PE", 24.0, 0.0),
        ]),
        role(m1, true),
    )?;
    for phase in 0..3u16 {
        let (top, bottom) = (phase * 2, phase * 2 + 1);
        let swapped = [0u16, 2, 1][usize::from(phase)];
        tx.connect(
            Endpoint::pin(feeds[usize::from(phase)], 0),
            Endpoint::pin(ku_main, top),
        )?;
        tx.connect(
            Endpoint::pin(feeds[usize::from(phase)], 0),
            Endpoint::pin(kd_main, swapped * 2),
        )?;
        // Both reversing outputs join before the speed pair.
        let join = tx.add_junction(main_page, Point::new(40.0 + 8.0 * f64::from(phase), 68.0))?;
        tx.connect(Endpoint::pin(ku_main, bottom), Endpoint::junction(join))?;
        tx.connect(
            Endpoint::pin(kd_main, phase * 2 + 1),
            Endpoint::junction(join),
        )?;
        tx.connect(Endpoint::junction(join), Endpoint::pin(kf_main, top))?;
        tx.connect(Endpoint::junction(join), Endpoint::pin(kc_main, top))?;
        // Fine feeds winding 1, coarse feeds winding 2.
        tx.connect(
            Endpoint::pin(kf_main, bottom),
            Endpoint::pin(motor_sym, phase),
        )?;
        tx.connect(
            Endpoint::pin(kc_main, bottom),
            Endpoint::pin(motor_sym, phase + 3),
        )?;
    }

    // ----- page 2: control circuit ----------------------------------------
    let ctl_feed = |tx: &mut Tx<'_>, potential, x, y| feed(tx, control_page, potential, x, y);
    let sym = |tx: &mut Tx<'_>, key: &str, x, y, device, main| {
        let (a, b) = match key {
            "iec/coils/relay" => ("A1", "A2"),
            "iec/operators/pushbutton_no" => ("13", "14"),
            _ => ("11", "12"),
        };
        let id = tx.place_symbol(
            control_page,
            key,
            at(x, y),
            two_pin(a, b),
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

    // Rung 1: KU = (S1 or S2) and not KD.
    let x = 190.0;
    let f = ctl_feed(tx, l, x, 20.0)?;
    let j_top = tx.add_junction(control_page, Point::new(x, 44.0))?;
    let b1 = sym(tx, btn, x - 12.0, 52.0, s1, true)?;
    let b2 = sym(tx, btn, x + 12.0, 52.0, s2, true)?;
    let j_bot = tx.add_junction(control_page, Point::new(x, 76.0))?;
    let kd_nc = sym(tx, nc, x, 84.0, kd, false)?;
    let ku_coil = sym(tx, coil, x, 112.0, ku, true)?;
    let fnn = ctl_feed(tx, n, x, 136.0)?;
    tx.connect(Endpoint::pin(f, 0), Endpoint::junction(j_top))?;
    for b in [b1, b2] {
        tx.connect(Endpoint::junction(j_top), Endpoint::pin(b, 0))?;
        tx.connect(Endpoint::pin(b, 1), Endpoint::junction(j_bot))?;
    }
    tx.connect(Endpoint::junction(j_bot), Endpoint::pin(kd_nc, 0))?;
    tx.connect(Endpoint::pin(kd_nc, 1), Endpoint::pin(ku_coil, 0))?;
    tx.connect(Endpoint::pin(ku_coil, 1), Endpoint::pin(fnn, 0))?;

    // Rung 2: KD = S3 and not KU.
    let x = 238.0;
    let f = ctl_feed(tx, l, x, 20.0)?;
    let b3 = sym(tx, btn, x, 52.0, s3, true)?;
    let ku_nc = sym(tx, nc, x, 84.0, ku, false)?;
    let kd_coil = sym(tx, coil, x, 112.0, kd, true)?;
    let fnn = ctl_feed(tx, n, x, 136.0)?;
    tx.connect(Endpoint::pin(f, 0), Endpoint::pin(b3, 0))?;
    tx.connect(Endpoint::pin(b3, 1), Endpoint::pin(ku_nc, 0))?;
    tx.connect(Endpoint::pin(ku_nc, 1), Endpoint::pin(kd_coil, 0))?;
    tx.connect(Endpoint::pin(kd_coil, 1), Endpoint::pin(fnn, 0))?;

    // Rung 3: KF = (S1 or S3) and not KC. The same pendant contacts, drawn again.
    let x = 286.0;
    let f = ctl_feed(tx, l, x, 20.0)?;
    let j_top = tx.add_junction(control_page, Point::new(x, 44.0))?;
    let b1b = sym(tx, btn, x - 12.0, 52.0, s1, false)?;
    let b3b = sym(tx, btn, x + 12.0, 52.0, s3, false)?;
    let j_bot = tx.add_junction(control_page, Point::new(x, 76.0))?;
    let kc_nc = sym(tx, nc, x, 84.0, kc, false)?;
    let kf_coil = sym(tx, coil, x, 112.0, kf, true)?;
    let fnn = ctl_feed(tx, n, x, 136.0)?;
    tx.connect(Endpoint::pin(f, 0), Endpoint::junction(j_top))?;
    for b in [b1b, b3b] {
        tx.connect(Endpoint::junction(j_top), Endpoint::pin(b, 0))?;
        tx.connect(Endpoint::pin(b, 1), Endpoint::junction(j_bot))?;
    }
    tx.connect(Endpoint::junction(j_bot), Endpoint::pin(kc_nc, 0))?;
    tx.connect(Endpoint::pin(kc_nc, 1), Endpoint::pin(kf_coil, 0))?;
    tx.connect(Endpoint::pin(kf_coil, 1), Endpoint::pin(fnn, 0))?;

    // Rung 4: KC = S2 and upper limit S9 (coarse only) and not KF.
    let x = 334.0;
    let f = ctl_feed(tx, l, x, 20.0)?;
    let b2b = sym(tx, btn, x, 44.0, s2, false)?;
    let limit = sym(tx, nc, x, 68.0, s9, true)?;
    let kf_nc = sym(tx, nc, x, 92.0, kf, false)?;
    let kc_coil = sym(tx, coil, x, 116.0, kc, true)?;
    let fnn = ctl_feed(tx, n, x, 140.0)?;
    tx.connect(Endpoint::pin(f, 0), Endpoint::pin(b2b, 0))?;
    tx.connect(Endpoint::pin(b2b, 1), Endpoint::pin(limit, 0))?;
    tx.connect(Endpoint::pin(limit, 1), Endpoint::pin(kf_nc, 0))?;
    tx.connect(Endpoint::pin(kf_nc, 1), Endpoint::pin(kc_coil, 0))?;
    tx.connect(Endpoint::pin(kc_coil, 1), Endpoint::pin(fnn, 0))?;

    // The main contacts conduct pole by pole.
    let three = vec![vec![0, 1], vec![2, 3], vec![4, 5]];
    for main in [ku_main, kd_main, kf_main, kc_main] {
        tx.update::<SymbolInstance>(main, |s| s.pass_through = three.clone())?;
    }

    // The hoist: a hook load makes the motor one; the upper limit sits in the coarse up circuit.
    for (key, value) in [
        ("mech_mass_kg", 100.0.into()),
        ("mech_upper_limit", "=A1+FLD-S9".into()),
        ("mech_upper_limit_m", 3.0.into()),
    ] {
        tx.set_property(m1, key, Some(value))?;
    }

    Ok(Hoist {
        main_page,
        control_page,
        motor: m1,
        up_fine: b1,
        up_coarse: b2,
        down: b3,
        limit,
        motor_sym,
    })
}
