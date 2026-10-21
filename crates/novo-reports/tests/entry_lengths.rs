//! Wires that leave the enclosure are routed to its entry point; with a
//! stated cable run the length is exact on both sides (`docs/design/cabinet-entry.md`).

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, StructureId};
use novo_reports::{is_estimate, wire_cut_jobs};
use novo_routing::entry::{set_entry_point, set_outside_mm};
use novo_tests::{MotorStarter, motor_starter};

/// X1 and F1 and K1 on a plate with a duct; the motor M1 is in +FLD.
fn cabinet() -> (Document, MotorStarter, StructureId) {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).expect("fixture builds");
    let (cab, _) = doc
        .transact("Cabinet", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB")?, "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 600.0,
                    margin: 10.0,
                }),
            )?;
            tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, None)?;
            tx.place_device(h.k1, cab, Point::new(400.0, 100.0), false, None)?;
            tx.place_device(h.x1, cab, Point::new(300.0, 400.0), false, None)?;
            let duct = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
            tx.set_rail_size(duct, 700.0, 50.0, 60.0)?;
            Ok(cab)
        })
        .expect("layout");
    (doc, h, cab)
}

fn set_run(doc: &mut Document, h: &MotorStarter, metres: Option<f64>) {
    let cable = doc
        .project()
        .cable_of_wire(h.motor_wires[0])
        .expect("the fixture has the motor cable");
    doc.transact("Cable run", |tx| {
        tx.update::<novo_core::Cable>(cable, |c| c.length_m = metres)
    })
    .unwrap();
}

fn motor_job(doc: &Document) -> novo_reports::WireCutJob {
    wire_cut_jobs(doc.project())
        .into_iter()
        .find(|j| j.end_a.designation == "=A1+CAB-X1:1" && j.end_b.designation == "=A1+FLD-M1:U1")
        .expect("the X1 to M1 wire")
}

/// The service loop at one wire end (project default), mm.
const LOOP_MM: f64 = 50.0;

#[test]
fn a_field_wire_is_an_estimate_until_its_cable_states_a_length() {
    let (mut doc, h, _cab) = cabinet();
    set_run(&mut doc, &h, None);
    let job = motor_job(&doc);
    assert_eq!(job.basis, "estimate");
    assert!(is_estimate(&job.basis));
}

#[test]
fn a_stated_cable_length_is_the_core_length_with_no_loop_added() {
    let (mut doc, h, cab) = cabinet();
    set_run(&mut doc, &h, Some(10.0));
    let ten = motor_job(&doc);
    assert_eq!(ten.basis, "cable length");
    assert!(!is_estimate(&ten.basis));
    assert_eq!(ten.length_mm, 10_000.0);
    set_run(&mut doc, &h, Some(20.0));
    assert_eq!(
        motor_job(&doc).length_mm - ten.length_mm,
        10_000.0,
        "metre for metre"
    );
    // An entry point changes nothing for a wire on a cable: the cable is as long as it is.
    doc.transact("Entry", |tx| {
        set_entry_point(tx, cab, Some(novo_routing::Point::new(400.0, 600.0)))
    })
    .unwrap();
    let with_entry = motor_job(&doc);
    assert_eq!(with_entry.length_mm, 20_000.0);
    assert_eq!(with_entry.basis, "cable length");
}

fn feed_jobs(doc: &Document) -> Vec<novo_reports::WireCutJob> {
    wire_cut_jobs(doc.project())
        .into_iter()
        .filter(|j| {
            j.end_a.designation.ends_with("(feed)") || j.end_b.designation.ends_with("(feed)")
        })
        .collect()
}

#[test]
fn a_supply_lead_is_routed_to_the_entry_and_exact_only_with_a_stated_outside() {
    let (mut doc, _h, cab) = cabinet();
    let feeds = feed_jobs(&doc);
    assert!(!feeds.is_empty());
    assert!(feeds.iter().all(|j| j.basis == "estimate"), "no entry yet");
    doc.transact("Entry", |tx| {
        set_entry_point(tx, cab, Some(novo_routing::Point::new(400.0, 600.0)))
    })
    .unwrap();
    assert!(
        feed_jobs(&doc).iter().all(|j| j.basis == "estimate"),
        "no outside stated"
    );

    doc.transact("Outside", |tx| set_outside_mm(tx, cab, Some(800.0)))
        .unwrap();
    let routed = novo_routing::model::route_project(doc.project());
    let exact: Vec<_> = feed_jobs(&doc)
        .into_iter()
        .filter(|j| j.basis == "to entry + stated outside")
        .collect();
    assert!(
        !exact.is_empty(),
        "the placed fuse's lead is routed to the entry"
    );
    for job in &exact {
        assert!(!is_estimate(&job.basis));
        let inside = routed
            .routes
            .get(&job.wires[0].raw())
            .expect("the lead has a route to the entry")
            .length_mm;
        assert_eq!(
            job.length_mm,
            (inside + 800.0).ceil(),
            "inside route + stated outside"
        );
        assert!(inside > 2.0 * LOOP_MM, "a real route, not only the loops");
    }
    // A clear removes the exactness again.
    doc.transact("Outside", |tx| set_outside_mm(tx, cab, None))
        .unwrap();
    assert!(feed_jobs(&doc).iter().all(|j| j.basis == "estimate"));
}

#[test]
fn a_cable_is_counted_once_not_once_per_core() {
    use novo_reports::{Report, WireCut, cable_runs, hookup_cut_mm};
    let (mut doc, h, _cab) = cabinet();
    set_run(&mut doc, &h, Some(12.12));
    let project = doc.project();
    let jobs = wire_cut_jobs(project);
    let cores: Vec<_> = jobs.iter().filter(|j| j.cable.is_some()).collect();
    assert!(cores.len() >= 3, "the motor cable has several cores");
    assert!(jobs.iter().any(|j| j.cable.is_none()), "and hookup wires");

    // One cable, bought once at its stated length, with all its cores.
    let runs = cable_runs(project, &jobs);
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert_eq!(runs[0].length_mm, 12_120.0);
    assert_eq!(runs[0].cores, cores.len());

    // The metres to pull leave the cores out.
    let hookup: f64 = jobs
        .iter()
        .filter(|j| j.cable.is_none())
        .map(|j| j.cut_mm)
        .sum();
    assert_eq!(hookup_cut_mm(&jobs), hookup);
    let all: f64 = jobs.iter().map(|j| j.cut_mm).sum();
    assert!(all - hookup > cores.len() as f64 * 12_000.0);

    // Wire production: the stock holds hookup wire only, cables are apart.
    let out = WireCut.generate(project);
    let stock = out.sections.iter().find(|s| s.key == "stock").unwrap();
    let stock_m: f64 = stock
        .rows
        .iter()
        .map(|r| match &r.cells[3] {
            novo_reports::Cell::Number(m) => *m,
            other => panic!("{other:?}"),
        })
        .sum();
    assert!(stock_m <= hookup / 1000.0 + 0.05 * stock.rows.len() as f64);
    let cables = out.sections.iter().find(|s| s.key == "cables").unwrap();
    assert_eq!(cables.rows.len(), 1);
    // The job list still has every core.
    assert_eq!(out.sections[0].rows.len(), jobs.len());
}

#[test]
fn a_cable_shorter_than_the_route_to_the_entry_stays_an_estimate() {
    let (mut doc, h, cab) = cabinet();
    doc.transact("Entry", |tx| {
        set_entry_point(tx, cab, Some(novo_routing::Point::new(400.0, 600.0)))
    })
    .unwrap();
    set_run(&mut doc, &h, Some(0.1));
    let job = motor_job(&doc);
    assert_eq!(job.basis, "estimate", "{job:?}");
    assert!(is_estimate(&job.basis));
}

#[test]
fn cable_lines_say_when_the_length_is_estimated_and_the_cut_list_leaves_cores_out() {
    use novo_reports::{CutList, Report, cable_runs};
    let (mut doc, h, _cab) = cabinet();
    set_run(&mut doc, &h, None);
    let jobs = wire_cut_jobs(doc.project());
    let runs = cable_runs(doc.project(), &jobs);
    assert_eq!(runs.len(), 1);
    assert!(!runs[0].stated);
    assert_eq!(runs[0].basis(), "estimated");
    // Cores are counted once each, never more than the jobs on the cable.
    assert!(runs[0].cores <= jobs.iter().filter(|j| j.cable.is_some()).count());
    assert_eq!(runs[0].metres(), (runs[0].length_mm / 100.0).ceil() / 10.0);

    set_run(&mut doc, &h, Some(12.12));
    let runs = cable_runs(doc.project(), &wire_cut_jobs(doc.project()));
    assert!(runs[0].stated);
    assert_eq!(runs[0].basis(), "stated");
    assert_eq!(runs[0].metres(), 12.2);

    // Reports > Cut list: the group totals are hookup wire only.
    let out = CutList::default().generate(doc.project());
    let summary = out.sections.iter().find(|s| s.key == "summary").unwrap();
    let total_m: f64 = summary
        .rows
        .iter()
        .map(|r| match &r.cells[2] {
            novo_reports::Cell::Number(m) => *m,
            other => panic!("{other:?}"),
        })
        .sum();
    let jobs = wire_cut_jobs(doc.project());
    let hookup = novo_reports::hookup_cut_mm(&jobs) / 1000.0;
    assert!(
        (total_m - hookup).abs() < 0.01 * summary.rows.len() as f64 + 0.001,
        "{total_m} vs {hookup}"
    );
}

/// A second enclosure +FLD with the motor terminal box M1 on its plate.
fn with_field_cabinet(doc: &mut Document, h: &MotorStarter) -> StructureId {
    let (fld, _) = doc
        .transact("Field cabinet", |tx| {
            let fld = tx.add_structure(DeviceTag::parse("+FLD")?, "Field", true)?;
            tx.set_plate(
                fld,
                Some(MountingPlate {
                    width: 400.0,
                    height: 300.0,
                    margin: 10.0,
                }),
            )?;
            tx.place_device(h.m1, fld, Point::new(150.0, 100.0), false, None)?;
            Ok(fld)
        })
        .expect("field cabinet");
    fld
}

#[test]
fn a_wire_between_two_cabinets_is_routed_to_each_entry() {
    let (mut doc, h, cab) = cabinet();
    let fld = with_field_cabinet(&mut doc, &h);
    let none = novo_routing::model::route_project(doc.project());
    assert!(none.cross_legs.is_empty(), "no entry points yet");

    let (e_cab, e_fld) = (
        novo_routing::Point::new(400.0, 600.0),
        novo_routing::Point::new(200.0, 300.0),
    );
    doc.transact("Entries", |tx| {
        set_entry_point(tx, cab, Some(e_cab))?;
        set_entry_point(tx, fld, Some(e_fld))
    })
    .unwrap();
    let routed = novo_routing::model::route_project(doc.project());
    assert!(!routed.cross_legs.is_empty(), "the motor wires cross");
    for (id, plates) in &routed.cross_legs {
        assert_eq!(plates.len(), 2);
        assert!(
            routed.unrouted.contains(id),
            "no whole length is claimed for the run between the entries"
        );
        assert!(!routed.routes.contains_key(id));
        let leg = |plate| {
            routed
                .cross_routes
                .get(&(*id, plate))
                .unwrap_or_else(|| panic!("wire {id} has no leg on plate {plate:?}"))
        };
        let (near, far) = (leg(plates[0]), leg(plates[1]));
        assert_eq!(
            near.points.last(),
            Some(&e_cab),
            "ends at the cabinet entry"
        );
        assert_eq!(
            far.points.first(),
            Some(&e_fld),
            "starts at the field entry"
        );
    }

    // Only one entry point: the other end cannot be drawn, so nothing is claimed.
    doc.transact("Clear", |tx| set_entry_point(tx, fld, None))
        .unwrap();
    let half = novo_routing::model::route_project(doc.project());
    assert!(half.cross_legs.is_empty() && half.cross_routes.is_empty());
}
