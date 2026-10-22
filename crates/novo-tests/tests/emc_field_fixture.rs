//! The EMC magnetic field QA fixture: a 25 A drive with its three motor wires
//! (U, V, W, side by side in one duct) to a motor, a PLC analog module next to
//! the drive, and an analog wire from it to a transmitter along the same duct.
//! The drive carries a rated current, so the field estimate has currents.
//!
//! Regenerate with `KOPERCAD_WRITE_FIXTURES=1 cargo test -p novo-tests --test emc_field_fixture`
//! and commit `qa/fixtures/emc-field.novo`.

use novo_core::*;

fn tag(t: &str) -> DeviceTag {
    DeviceTag::parse(t).unwrap()
}

struct Cab {
    project: Project,
}

/// A 800 x 400 plate with a horizontal duct at y = 300. A drive at the left
/// feeds a motor at the right, and an analog module at the left reads a
/// transmitter at the right: both wires run the length of the duct.
fn cabinet() -> Cab {
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
                if t == "-T1" {
                    tx.set_property(d, "rated_current_a", Some(Value::Float(25.0)))?;
                }
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
            let (_drive, s_drive) = place(
                "-T1",
                "iec/drives/frequency_converter",
                &["L1", "U", "V", "W"],
                40.0,
                190.0,
            )?;
            let (_m, s_motor) = place(
                "-M1",
                "iec/motors/motor_3ph",
                &["U1", "V1", "W1"],
                700.0,
                190.0,
            )?;
            let (_plc, s_plc) = place("-A1", "iec/plc/analog_input", &["AI1"], 110.0, 190.0)?;
            let (_b, s_b) = place("-B1", "iec/sensors/transmitter", &["1"], 640.0, 190.0)?;
            let u = tx.connect(Endpoint::pin(s_drive, 1), Endpoint::pin(s_motor, 0))?;
            let v = tx.connect(Endpoint::pin(s_drive, 2), Endpoint::pin(s_motor, 1))?;
            let w = tx.connect(Endpoint::pin(s_drive, 3), Endpoint::pin(s_motor, 2))?;
            tx.connect(Endpoint::pin(s_plc, 0), Endpoint::pin(s_b, 0))?;
            let _ = (u, v, w);
            Ok(())
        })
        .unwrap();
    let () = ids;
    Cab { project }
}

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../qa/fixtures/emc-field.novo")
}

#[test]
fn the_committed_fixture_has_currents_and_a_field_finding() {
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
    let field = report.field.as_ref().expect("the field estimate ran");
    assert_eq!(field.currents.len(), 3, "{:#?}", field.currents);
    let phases: Vec<f64> = field.currents.iter().map(|c| c.phase_deg).collect();
    assert!(phases.contains(&0.0) && phases.contains(&-120.0) && phases.contains(&120.0));
    assert!(field.currents.iter().all(|c| c.amps == 25.0 && c.routed));
    assert!(
        !field.devices.is_empty() && !field.victims.is_empty(),
        "{field:#?}"
    );
    // The PLC sits 30 mm from the duct line: the three phases laid 3 mm apart
    // still leave about 24 uT there at 25 A, about twice the planning value of 12.6 uT.
    assert!(
        report
            .of_rule("EMC-19")
            .any(|f| f.severity == novo_emc::Severity::Warning),
        "{:#?}",
        field.devices
    );
    assert!(report.of_rule("EMC-20").count() == 1);
}
