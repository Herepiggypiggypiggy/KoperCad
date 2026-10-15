use novo_core::{DeviceTag, MountingPlate, Project, RailKind, StructureId, Value};
use novo_layout::model::{apply_plan, candidates, load, set_plate};
use novo_view3d::build::{Options, model, refresh};
use novo_view3d::{ItemKind, Mark};

fn project() -> (Project, StructureId, novo_core::PartId) {
    let mut p = Project::new("t");
    let (r, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(90)))?;
            tx.set_property(part, "depth_mm", Some(Value::Float(55.0)))?;
            for tag in ["=A1+CAB1-K1", "=A2+CAB1-K2", "=A1+CAB1-Q1"] {
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
            Ok((cab, part))
        })
        .unwrap();
    (p, r.0, r.1)
}

#[test]
fn scene_follows_the_project_through_edits() {
    let (mut p, cab, part) = project();
    let o = Options::default();
    let mut scene = model::scene(&p, cab, &o).unwrap();
    assert_eq!(scene.items.len(), 3); // shell, plate, door

    let plan = load(&p, cab).unwrap().preview(&candidates(&p, cab));
    p.transact("Place all", |tx| apply_plan(tx, cab, &plan))
        .unwrap();
    refresh(&mut scene, model::scene(&p, cab, &o).unwrap());
    assert_eq!(scene.revision, 1);
    let devices: Vec<_> = scene
        .items
        .iter()
        .filter(|i| i.id.kind == ItemKind::Device)
        .collect();
    assert_eq!(devices.len(), 3);
    // Two have the part (55 deep), K2 has none: default depth, marked.
    assert_eq!(devices.iter().filter(|i| i.size.z == 55.0).count(), 2);
    assert_eq!(
        devices
            .iter()
            .filter(|i| i.mark == Mark::GuessedSize)
            .count(),
        1
    );
    assert!(scene.items.iter().any(|i| i.id.kind == ItemKind::Rail));
    // Labels come from the tags.
    assert!(devices.iter().all(|i| i.label.is_some()));
    assert!(devices.iter().any(|i| i.label.as_deref() == Some("-K1")));

    // Changing a part depth is an edit too.
    p.transact("deeper", |tx| {
        tx.set_property(part, "depth_mm", Some(Value::Float(80.0)))
    })
    .unwrap();
    refresh(&mut scene, model::scene(&p, cab, &o).unwrap());
    assert_eq!(scene.revision, 2);
    assert!(scene.items.iter().any(|i| i.size.z == 80.0));
}

#[test]
fn ducts_come_from_the_model() {
    let (mut p, cab, _) = project();
    p.transact("duct", |tx| {
        tx.execute(novo_core::Command::AddRail {
            structure: cab,
            x: 20.0,
            y: 200.0,
            length: 300.0,
            kind: RailKind::Duct,
            vertical: false,
            width: 30.0,
            height: 40.0,
        })
        .map(|_| ())
    })
    .unwrap();
    let s = model::scene(&p, cab, &Options::default()).unwrap();
    let duct = s
        .items
        .iter()
        .find(|i| i.id.kind == ItemKind::Duct)
        .unwrap();
    assert_eq!(duct.size.to_array(), [300.0, 30.0, 40.0]);
    assert_eq!(duct.mark, Mark::None);
}

#[test]
fn no_plate_no_scene() {
    let mut p = Project::new("t");
    let (s, _) = p
        .transact("s", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB2").unwrap(), "", true)
        })
        .unwrap();
    assert!(model::scene(&p, s, &Options::default()).is_none());
}
