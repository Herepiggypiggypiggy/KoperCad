//! STEP export of the cabinet (WS-24): the file reads back with our own STEP
//! reader as the right solids in the right places.

use std::sync::atomic::AtomicBool;

use novo_core::{DeviceTag, MountingPlate, Project, Value};
use novo_interop::part3d::{Quality, geometry_from_step};
use novo_interop::step::{StepOptions, export_enclosure};
use novo_layout::model::{apply_plan, candidates, load, set_plate};
use novo_view3d::build::Options;

pub fn cabinet() -> (Project, novo_core::StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(90)))?;
            tx.set_property(part, "depth_mm", Some(Value::Int(70)))?;
            for tag in ["=A1+CAB1-K1", "=A1+CAB1-K2"] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
            }
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 600.0,
                    height: 800.0,
                    margin: 10.0,
                },
            )?;
            Ok(cab)
        })
        .unwrap();
    let plan = load(&p, cab).unwrap().preview(&candidates(&p, cab));
    p.transact("place", |tx| apply_plan(tx, cab, &plan))
        .unwrap();
    (p, cab)
}

#[test]
fn the_cabinet_exports_as_named_solids() {
    let (p, cab) = cabinet();
    let step = export_enclosure(&p, cab, &Options::default(), &StepOptions::default()).unwrap();
    let t = &step.text;
    assert!(t.starts_with("ISO-10303-21;"));
    assert!(t.trim_end().ends_with("END-ISO-10303-21;"));
    assert!(t.contains("AUTOMOTIVE_DESIGN"));
    for name in ["Enclosure", "Mounting plate", "-K1", "-K2"] {
        assert!(t.contains(&format!(",'{name}','',(")), "{name}");
    }
    assert!(step.report.count_of("items") >= 5);
    // No placeholder survives patching, and every reference points at a line.
    assert!(!t.contains("ROOTITEMS"));
}

#[test]
fn the_export_is_deterministic() {
    let (p, cab) = cabinet();
    let a = export_enclosure(&p, cab, &Options::default(), &StepOptions::default()).unwrap();
    let b = export_enclosure(&p, cab, &Options::default(), &StepOptions::default()).unwrap();
    assert_eq!(a.text, b.text);
}

#[test]
fn an_enclosure_without_a_plate_exports_nothing() {
    let mut p = Project::new("t");
    let (id, _) = p
        .transact("s", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB2").unwrap(), "", true)
        })
        .unwrap();
    assert!(export_enclosure(&p, id, &Options::default(), &StepOptions::default()).is_none());
}

#[test]
fn our_own_reader_finds_the_solids_back() {
    let (p, cab) = cabinet();
    let step = export_enclosure(&p, cab, &Options::default(), &StepOptions::default()).unwrap();
    let g = geometry_from_step(
        step.text.as_bytes(),
        None,
        Quality::Normal,
        &AtomicBool::new(false),
        None,
    )
    .expect("the reader accepts the file");
    let size = g.size_mm();
    // The housing is 600 x 800 inside, 2 mm walls, 3 mm plate and 2 mm back
    // wall behind it, a 250 mm interior and a 2 mm door. The DIN rails run
    // through the middle of their devices, so none overhangs the plate. The
    // box: x -2..602, y -2..802, z -5..252 (it was 807.5 high while the top
    // rail's band stuck 7.5 mm out above the plate).
    let expect = [604.0, 804.0, 257.0];
    for (got, want) in size.iter().zip(expect) {
        assert!(
            (f64::from(*got) - want).abs() < 0.5,
            "{size:?} vs {expect:?}"
        );
    }
}

#[test]
fn every_enclosure_with_a_plate_gets_a_file_with_its_depth() {
    let (mut p, cab) = cabinet();
    p.transact("depth", |tx| {
        tx.set_property(cab, "depth_mm", Some(Value::Int(300)))?;
        tx.add_structure(DeviceTag::parse("+JB2").unwrap(), "no plate yet", true)?;
        Ok(())
    })
    .unwrap();
    let all = novo_interop::step::export_all(&p, &StepOptions::default());
    assert_eq!(
        all.len(),
        1,
        "the enclosure without a plate has nothing to model"
    );
    assert_eq!(all[0].tag, "+CAB1");
    assert!(all[0].export.text.contains("FILE_NAME('+CAB1.stp'"));
    // 300 deep: the door sits at z = 300 (the front face of the housing).
    assert!(
        all[0].export.text.contains(",300.)"),
        "{}",
        &all[0].export.text[..200]
    );
}
