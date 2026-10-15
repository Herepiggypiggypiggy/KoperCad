use novo_core::{Command, DeviceTag, MountingPlate, Project, RailKind};
use novo_layout::model::{load, set_plate};

#[test]
fn ducts_are_loaded_apart_from_din_rails_with_a_default_cross_section() {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                },
            )?;
            for (kind, vertical, width) in [
                (RailKind::DinRail, false, 0.0),
                (RailKind::Duct, false, 0.0),
                (RailKind::Duct, true, 60.0),
            ] {
                tx.execute(Command::AddRail {
                    structure: cab,
                    x: 20.0,
                    y: 50.0,
                    length: 200.0,
                    kind,
                    vertical,
                    width,
                    height: 0.0,
                })?;
            }
            Ok(cab)
        })
        .unwrap();
    let layout = load(&p, cab).unwrap();
    assert_eq!(layout.rails.len(), 1);
    assert_eq!(layout.ducts.len(), 2);
    let flat = layout.ducts.iter().find(|d| !d.vertical).unwrap();
    assert_eq!(flat.width, 30.0);
    assert_eq!((flat.footprint().w, flat.footprint().h), (200.0, 30.0));
    let tall = layout.ducts.iter().find(|d| d.vertical).unwrap();
    assert_eq!((tall.footprint().w, tall.footprint().h), (60.0, 200.0));
}
