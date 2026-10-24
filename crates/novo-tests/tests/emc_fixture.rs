//! The EMC QA fixture: a cabinet with a drive, a motor, an analog module and a
//! transmitter, the motor wire and the analog wire running the length of one duct.
//!
//! Regenerate with `KOPERCAD_WRITE_FIXTURES=1 cargo test -p novo-tests --test emc_fixture`
//! and commit `qa/fixtures/emc-cabinet.novo`.

use novo_core::*;

fn tag(t: &str) -> DeviceTag {
    DeviceTag::parse(t).unwrap()
}

#[allow(dead_code)] // the fields mirror the engine test builder
struct Cab {
    project: Project,
    drive: DeviceId,
    plc: DeviceId,
    motor_wire: WireId,
    analog_wire: WireId,
    duct: RailId,
    cab: StructureId,
}

/// A 800 x 400 plate with a horizontal duct at y = 300. A drive at the left
/// feeds a motor at the right, and an analog module at the left reads a
/// transmitter at the right: both wires run the length of the duct.
fn cabinet() -> Cab {
    cabinet_with(false)
}

/// With `bonding`: a painted plate and a drive 170 mm deep, for the bonding
/// and height layers of the EMC views.
fn cabinet_with(bonding: bool) -> Cab {
    let mut project = Project::new("EMC");
    let (ids, _) = project
        .transact("build", |tx| {
            let page = tx.add_page("1")?;
            let cab = tx.add_structure(tag("+CAB"), "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 400.0,
                    margin: 10.0,
                }),
            )?;
            let duct = tx.add_rail(cab, 20.0, 300.0, 760.0, RailKind::Duct)?;
            tx.set_rail_size(duct, 760.0, 60.0, 60.0)?;
            let mut place = |t: &str,
                             key: &str,
                             pins: &[&str],
                             x: f64,
                             y: f64|
             -> Result<(DeviceId, SymbolId)> {
                let d = tx.add_device(tag(t), "")?;
                tx.set_property(d, "width_mm", Some(Value::Float(60.0)))?;
                tx.set_property(d, "height_mm", Some(Value::Float(80.0)))?;
                tx.place_device(d, cab, Point::new(x, y), false, None)?;
                let s = tx.place_symbol(
                    page,
                    key,
                    Placement::at(Point::new(x, 10.0)),
                    pins.iter()
                        .enumerate()
                        .map(|(i, p)| Pin::new(*p, 8.0 * i as f64, 0.0))
                        .collect(),
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )?;
                Ok((d, s))
            };
            let (drive, s_drive) = place(
                "-T1",
                "iec/drives/frequency_converter",
                &["L1", "U"],
                40.0,
                190.0,
            )?;
            let (_m, s_motor) = place("-M1", "iec/motors/motor_3ph", &["U1"], 700.0, 190.0)?;
            let (plc, s_plc) = place("-A1", "iec/plc/analog_input", &["AI1"], 100.0, 190.0)?;
            let (_b, s_b) = place("-B1", "iec/sensors/transmitter", &["1"], 640.0, 190.0)?;
            if bonding {
                tx.set_property(cab, "emc.finish", Some(Value::Text("painted".into())))?;
                tx.set_property(drive, "depth_mm", Some(Value::Float(170.0)))?;
                tx.set_property(plc, "depth_mm", Some(Value::Float(110.0)))?;
            }
            let motor_wire = tx.connect(Endpoint::pin(s_drive, 1), Endpoint::pin(s_motor, 0))?;
            let analog_wire = tx.connect(Endpoint::pin(s_plc, 0), Endpoint::pin(s_b, 0))?;
            Ok((drive, plc, motor_wire, analog_wire, duct, cab))
        })
        .unwrap();
    Cab {
        project,
        drive: ids.0,
        plc: ids.1,
        motor_wire: ids.2,
        analog_wire: ids.3,
        duct: ids.4,
        cab: ids.5,
    }
}

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../qa/fixtures/emc-cabinet.novo")
}

#[test]
fn the_committed_fixture_has_findings_with_the_check_on() {
    if std::env::var_os("KOPERCAD_WRITE_FIXTURES").is_some() {
        let c = cabinet();
        let path = fixture_path();
        let _ = std::fs::remove_file(&path);
        novo_storage::ProjectFile::create(&path, &c.project).expect("fixture is written");
    }
    let project = novo_storage::ProjectFile::open(fixture_path())
        .and_then(|f| f.load())
        .expect("fixture opens");
    let report = novo_emc::project::analyse_project(&project);
    assert!(
        !report.findings.is_empty(),
        "the fixture must show findings: regenerate it"
    );
}

fn bonding_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/emc-cabinet-bonding.novo")
}

#[test]
fn the_bonding_fixture_has_a_weak_bond_and_stated_heights() {
    if std::env::var_os("KOPERCAD_WRITE_FIXTURES").is_some() {
        let c = cabinet_with(true);
        let path = bonding_path();
        let _ = std::fs::remove_file(&path);
        novo_storage::ProjectFile::create(&path, &c.project).expect("fixture is written");
    }
    let project = novo_storage::ProjectFile::open(bonding_path())
        .and_then(|f| f.load())
        .expect("fixture opens");
    let pic = novo_emc::project::picture_of(&project);
    let report = novo_emc::project::analyse_project(&project);
    assert_eq!(pic.bonds.len(), 1, "the drive on the painted plate");
    assert_eq!(pic.bonds[0].quality, novo_emc::BondQuality::Weak);
    let heights: Vec<_> = pic.zones.iter().map(|z| z.height_mm).collect();
    assert!(heights.contains(&Some(170.0)), "{heights:?}");
    assert!(report.findings.iter().any(|f| f.rule == "EMC-11"));
}
