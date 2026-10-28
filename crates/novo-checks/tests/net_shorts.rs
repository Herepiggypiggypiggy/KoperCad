//! Shorts the potential definitions alone do not show: a `PE` or `N` pin on a
//! phase, and two phases joined through the poles of one breaker. Each rule
//! has a project that breaks it, one that does not and, where the drawing
//! cannot tell, one that documents what is left alone.

use novo_checks::{Problem, ProblemKind, check};
use novo_core::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn build<R>(edit: impl FnOnce(&mut Tx<'_>) -> Result<R>) -> (Project, R) {
    let mut project = Project::default();
    let (r, _) = project.transact("build", edit).unwrap();
    (project, r)
}

fn shorts(project: &Project) -> Vec<Problem> {
    check(project)
        .into_iter()
        .filter(|p| p.kind == ProblemKind::PotentialConflict)
        .collect()
}

fn feed(tx: &mut Tx<'_>, page: PageId, x: f64, potential: PotentialId) -> Result<SymbolId> {
    tx.place_symbol(
        page,
        "iec/connectivity/potential_feed",
        Placement::at(Point::new(x, 0.0)),
        vec![Pin::new("1", 0.0, 8.0)],
        SymbolRole::PotentialFeed { potential },
    )
}

fn device(
    tx: &mut Tx<'_>,
    page: PageId,
    key: &str,
    x: f64,
    t: &str,
    pins: &[&str],
) -> Result<SymbolId> {
    let device = tx.add_device(tag(t), "")?;
    let pins = pins
        .iter()
        .enumerate()
        .map(|(i, n)| Pin::new(*n, i as f64 * 8.0, 0.0))
        .collect();
    tx.place_symbol(
        page,
        key,
        Placement::at(Point::new(x, 40.0)),
        pins,
        SymbolRole::Device { device, main: true },
    )
}

fn poles(tx: &mut Tx<'_>, s: SymbolId, poles: Vec<Vec<u16>>) -> Result<()> {
    tx.update::<SymbolInstance>(s, |s| s.pass_through = poles)
}

fn wire(tx: &mut Tx<'_>, a: (SymbolId, u16), b: (SymbolId, u16)) -> Result<WireId> {
    tx.connect(Endpoint::pin(a.0, a.1), Endpoint::pin(b.0, b.1))
}

/// A converter whose `PE` pin is wired to a feed of `kind` named `name`.
fn pe_pin_on(name: &str, kind: PotentialKind) -> Project {
    build(|tx| {
        let page = tx.add_page("1a")?;
        let p = tx.add_potential(name, kind)?;
        let f = feed(tx, page, 0.0, p)?;
        let t1 = device(
            tx,
            page,
            "iec/drives/frequency_converter",
            0.0,
            "-T1",
            &["L1", "PE"],
        )?;
        wire(tx, (f, 0), (t1, 1))?;
        Ok(())
    })
    .0
}

#[test]
fn a_pe_pin_on_a_phase_is_a_short() {
    let found = shorts(&pe_pin_on("L1", PotentialKind::Line));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(
        found[0].message,
        "Protective earth pin of -T1 is on a net with L1 (page 1a): \
         protective earth is joined to a live conductor"
    );
    assert!(shorts(&pe_pin_on("PE", PotentialKind::ProtectiveEarth)).is_empty());
    assert!(shorts(&pe_pin_on("0V", PotentialKind::DcNegative)).is_empty());
}

#[test]
fn a_neutral_pin_on_a_phase_is_a_short() {
    let on = |name: &str, kind| {
        build(|tx| {
            let page = tx.add_page("1")?;
            let p = tx.add_potential(name, kind)?;
            let f = feed(tx, page, 0.0, p)?;
            let g1 = device(
                tx,
                page,
                "iec/supplies/power_supply_dc",
                0.0,
                "-G1",
                &["L", "N"],
            )?;
            wire(tx, (f, 0), (g1, 1))?;
            Ok(())
        })
        .0
    };
    let found = shorts(&on("L2", PotentialKind::Line));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.starts_with("Neutral pin of -G1"));
    assert!(shorts(&on("N", PotentialKind::Neutral)).is_empty());
}

#[test]
fn a_net_that_defines_two_potentials_is_reported_once() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        let l1 = tx.add_potential("L1", PotentialKind::Line)?;
        let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
        let a = feed(tx, page, 0.0, l1)?;
        let b = feed(tx, page, 16.0, pe)?;
        let t1 = device(
            tx,
            page,
            "iec/drives/frequency_converter",
            0.0,
            "-T1",
            &["PE"],
        )?;
        wire(tx, (a, 0), (t1, 0))?;
        wire(tx, (b, 0), (t1, 0))?;
        Ok(())
    });
    let found = shorts(&project);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0]
            .message
            .ends_with("are defined on one net: a short")
    );
}

/// A three-pole breaker with the line feeds on its input pins.
fn breaker_on_phases(bridge: &[(u16, u16)]) -> Project {
    build(|tx| {
        let page = tx.add_page("2")?;
        let names = ["L1", "L2", "L3"];
        let q1 = device(
            tx,
            page,
            "iec/protection/mcb_3p",
            0.0,
            "-F4",
            &["1", "2", "3", "4", "5", "6"],
        )?;
        poles(tx, q1, vec![vec![0, 1], vec![2, 3], vec![4, 5]])?;
        for (i, name) in names.iter().enumerate() {
            let p = tx.add_potential(*name, PotentialKind::Line)?;
            let f = feed(tx, page, 40.0 + 16.0 * i as f64, p)?;
            wire(tx, (f, 0), (q1, 2 * i as u16))?;
        }
        for &(a, b) in bridge {
            wire(tx, (q1, a), (q1, b))?;
        }
        Ok(())
    })
    .0
}

#[test]
fn a_wire_bridging_two_poles_of_a_breaker_shorts_two_phases() {
    assert!(shorts(&breaker_on_phases(&[])).is_empty());

    let found = shorts(&breaker_on_phases(&[(1, 3)]));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(
        found[0].message,
        "L1 and L2 are joined by the poles of -F4 (page 2): a wire connects two of its \
         poles on one side while they carry different conductors, a short when it closes"
    );
    // The device is marked first, then the wires on it.
    assert!(matches!(found[0].at[0], AnyId::Symbol(_)));

    // All three joined: one problem naming all three, not three pair reports.
    let found = shorts(&breaker_on_phases(&[(1, 3), (3, 5)]));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.starts_with("L1, L2 and L3 are joined"));
}

#[test]
fn the_same_phase_on_two_poles_is_a_parallel_not_a_short() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        let l1 = tx.add_potential("L1", PotentialKind::Line)?;
        let f = feed(tx, page, 0.0, l1)?;
        let q = device(
            tx,
            page,
            "iec/protection/mcb_2p",
            0.0,
            "-F1",
            &["1", "2", "3", "4"],
        )?;
        poles(tx, q, vec![vec![0, 1], vec![2, 3]])?;
        wire(tx, (f, 0), (q, 0))?;
        wire(tx, (q, 0), (q, 2))?;
        wire(tx, (q, 1), (q, 3))?;
        Ok(())
    });
    assert!(shorts(&project).is_empty());
}

#[test]
fn supply_and_return_of_a_dc_supply_through_one_device_is_a_short() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        let plus = tx.add_potential("24V", PotentialKind::DcPositive)?;
        let zero = tx.add_potential("0V", PotentialKind::DcNegative)?;
        let a = feed(tx, page, 0.0, plus)?;
        let b = feed(tx, page, 16.0, zero)?;
        let q = device(
            tx,
            page,
            "iec/protection/mcb_2p",
            0.0,
            "-F7",
            &["1", "2", "3", "4"],
        )?;
        poles(tx, q, vec![vec![0, 1], vec![2, 3]])?;
        wire(tx, (a, 0), (q, 0))?;
        wire(tx, (b, 0), (q, 2))?;
        wire(tx, (q, 1), (q, 3))?;
        Ok(())
    });
    let found = shorts(&project);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.starts_with("0V and 24V are joined"));
}

/// Documented limit: two phases that meet through separate single-pole
/// devices are how a changeover is drawn, so they are not reported.
#[test]
fn separate_single_pole_devices_are_left_alone() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        let l1 = tx.add_potential("L1", PotentialKind::Line)?;
        let l2 = tx.add_potential("L2", PotentialKind::Line)?;
        let a = feed(tx, page, 0.0, l1)?;
        let b = feed(tx, page, 16.0, l2)?;
        let k1 = device(tx, page, "iec/contactors/main_1p", 0.0, "-K1", &["1", "2"])?;
        let k2 = device(tx, page, "iec/contactors/main_1p", 40.0, "-K2", &["1", "2"])?;
        poles(tx, k1, vec![vec![0, 1]])?;
        poles(tx, k2, vec![vec![0, 1]])?;
        wire(tx, (a, 0), (k1, 0))?;
        wire(tx, (b, 0), (k2, 0))?;
        wire(tx, (k1, 1), (k2, 1))?;
        Ok(())
    });
    assert!(shorts(&project).is_empty());
}

/// A changeover switch has a common pin shared by both its paths.
#[test]
fn a_changeover_switch_is_not_a_bridge() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        let l1 = tx.add_potential("L1", PotentialKind::Line)?;
        let l2 = tx.add_potential("L2", PotentialKind::Line)?;
        let a = feed(tx, page, 0.0, l1)?;
        let b = feed(tx, page, 16.0, l2)?;
        let s = device(
            tx,
            page,
            "iec/switches/changeover_switch_3p",
            0.0,
            "-S1",
            &["11", "12", "14"],
        )?;
        poles(tx, s, vec![vec![0, 1], vec![0, 2]])?;
        wire(tx, (a, 0), (s, 1))?;
        wire(tx, (b, 0), (s, 2))?;
        Ok(())
    });
    assert!(shorts(&project).is_empty());
}

/// The star point of a star-delta starter: the star contactor joins the three
/// winding ends, and the phases reach those ends through the delta contactor,
/// which is open while the star contactor is closed. Not a short.
#[test]
fn a_star_contactor_fed_through_another_contactor_is_not_a_short() {
    let (project, ()) = build(|tx| {
        let page = tx.add_page("1")?;
        let k2 = device(
            tx,
            page,
            "iec/contactors/main_3p",
            0.0,
            "-K2",
            &["1", "2", "3", "4", "5", "6"],
        )?;
        let k3 = device(
            tx,
            page,
            "iec/contactors/main_3p",
            80.0,
            "-K3",
            &["1", "2", "3", "4", "5", "6"],
        )?;
        for k in [k2, k3] {
            poles(tx, k, vec![vec![0, 1], vec![2, 3], vec![4, 5]])?;
        }
        for (i, name) in ["L1", "L2", "L3"].iter().enumerate() {
            let p = tx.add_potential(*name, PotentialKind::Line)?;
            let f = feed(tx, page, 16.0 * i as f64, p)?;
            wire(tx, (f, 0), (k2, 2 * i as u16))?;
            // The delta contactor's output feeds the star contactor's input.
            wire(tx, (k2, 2 * i as u16 + 1), (k3, 2 * i as u16))?;
        }
        // The star point: the other side of the three poles joined.
        wire(tx, (k3, 1), (k3, 3))?;
        wire(tx, (k3, 3), (k3, 5))?;
        Ok(())
    });
    assert!(shorts(&project).is_empty(), "{:#?}", shorts(&project));
}
