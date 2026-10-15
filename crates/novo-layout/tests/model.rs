use novo_core::{DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::model::{apply_plan, candidates, device_size, load, set_plate};

fn project() -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(90)))?;
            for tag in ["=A1+CAB1-K1", "=A2+CAB1-K2", "=A1+CAB1-Q1", "=A1+FIELD-M1"] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                if tag != "=A2+CAB1-K2" {
                    tx.assign_part(d, part)?;
                }
            }
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                },
            )?;
            Ok(cab)
        })
        .unwrap();
    (p, cab)
}

#[test]
fn candidates_are_the_devices_at_the_location_across_function_groups() {
    let (p, cab) = project();
    let c = candidates(&p, cab);
    assert_eq!(c.len(), 3); // not the FIELD motor
    let guessed = c.iter().filter(|c| c.size.guessed).count();
    assert_eq!(guessed, 1); // K2 has no part
    let d = p.all::<novo_core::Device>().next().unwrap();
    assert!(!device_size(&p, d).guessed);
}

#[test]
fn no_plate_means_no_layout() {
    let mut p = Project::new("t");
    let (s, _) = p
        .transact("s", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB2").unwrap(), "", true)
        })
        .unwrap();
    assert!(load(&p, s).is_none());
}

#[test]
fn place_all_is_one_undo_step_and_reloads_as_the_planned_layout() {
    let (mut p, cab) = project();
    let layout = load(&p, cab).unwrap();
    let cands = candidates(&p, cab);
    let plan = layout.preview(&cands);
    assert_eq!(plan.placements.len(), 3);
    let (_, tx) = p
        .transact("Place all", |tx| apply_plan(tx, cab, &plan))
        .unwrap();

    let after = load(&p, cab).unwrap();
    assert_eq!(after.placements.len(), 3);
    assert!(!after.rails.is_empty());
    assert!(after.placements.iter().all(|q| q.rail.is_some()));
    assert!(after.report().is_clean());
    assert!(after.preview(&cands).placements.is_empty());

    p.apply(&tx.inverse()).unwrap();
    let undone = load(&p, cab).unwrap();
    assert!(undone.placements.is_empty() && undone.rails.is_empty());
}

#[test]
fn a_second_place_all_skips_placed_devices_and_stays_below_them() {
    let (mut p, cab) = project();
    let cands = candidates(&p, cab);
    let first = load(&p, cab).unwrap().preview(&cands[..1]);
    p.transact("one", |tx| apply_plan(tx, cab, &first)).unwrap();
    let layout = load(&p, cab).unwrap();
    let placed_bottom = layout.placements[0].footprint().bottom();
    let plan = layout.preview(&cands);
    assert_eq!(plan.placements.len(), 2);
    assert!(plan.placements.iter().all(|q| q.y >= placed_bottom));
    p.transact("rest", |tx| apply_plan(tx, cab, &plan)).unwrap();
    assert!(load(&p, cab).unwrap().report().is_clean());
}
