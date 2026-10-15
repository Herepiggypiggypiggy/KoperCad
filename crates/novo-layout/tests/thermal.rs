use novo_core::{DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::thermal::{
    self, DEFAULT_BAND_LIMITS, HeatSource, band_of, heat_map, hot_devices, legend, top_third_share,
};
use novo_layout::{Plate, Rect};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn plate() -> Plate {
    Plate {
        width: 400.0,
        height: 300.0,
        margin: 0.0,
    }
}

fn source(id: u64, x: f64, y: f64, w: f64, h: f64, loss: f64) -> HeatSource {
    HeatSource {
        id,
        footprint: Rect { x, y, w, h },
        loss_w: loss,
    }
}

#[test]
fn heat_map_conserves_the_loss_with_and_without_blur() {
    let s = [source(1, 100.0, 50.0, 50.0, 100.0, 40.0)];
    close(heat_map(&plate(), &s, 10.0, 0.0).total_w(), 40.0);
    close(heat_map(&plate(), &s, 10.0, 40.0).total_w(), 40.0);
}

#[test]
fn unblurred_map_is_the_footprint_density() {
    // 50 x 100 mm = 0.5 dm2, 10 W -> 20 W/dm2.
    let s = [source(1, 100.0, 50.0, 50.0, 100.0, 10.0)];
    let m = heat_map(&plate(), &s, 10.0, 0.0);
    close(m.at(11, 6), 20.0);
    close(m.at(0, 0), 0.0);
    close(m.max(), 20.0);
}

#[test]
fn neighbours_read_hotter_than_the_same_devices_far_apart() {
    let close_pair = [
        source(1, 100.0, 100.0, 40.0, 40.0, 20.0),
        source(2, 145.0, 100.0, 40.0, 40.0, 20.0),
    ];
    let far_pair = [
        source(1, 20.0, 100.0, 40.0, 40.0, 20.0),
        source(2, 320.0, 100.0, 40.0, 40.0, 20.0),
    ];
    let a = heat_map(&plate(), &close_pair, 10.0, 40.0).max();
    let b = heat_map(&plate(), &far_pair, 10.0, 40.0).max();
    assert!(a > b);
}

#[test]
fn bands_and_legend() {
    assert_eq!(band_of(0.0, &DEFAULT_BAND_LIMITS), 0);
    assert_eq!(band_of(0.5, &DEFAULT_BAND_LIMITS), 1);
    assert_eq!(band_of(9.9, &DEFAULT_BAND_LIMITS), 3);
    assert_eq!(band_of(50.0, &DEFAULT_BAND_LIMITS), 4);
    let l = legend(&DEFAULT_BAND_LIMITS);
    assert_eq!(l[0].from, 0.0);
    assert_eq!(l[4].from, 10.0);
    assert_eq!(l[4].label, "very hot");
}

#[test]
fn ranking_and_top_third() {
    let s = [
        source(1, 0.0, 0.0, 40.0, 40.0, 30.0),
        source(2, 0.0, 200.0, 40.0, 40.0, 70.0),
    ];
    let r = hot_devices(&s);
    assert_eq!(r[0].0, 2);
    close(r[0].1, 0.7);
    close(top_third_share(&plate(), &s), 0.3);
    assert_eq!(top_third_share(&plate(), &[]), 0.0);
}

fn project() -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let drive = tx.add_part("ACME", "VFD")?;
            tx.set_property(drive, "power_loss_w", Some(Value::Float(120.0)))?;
            tx.set_property(drive, "load_factor", Some(Value::Float(0.5)))?;
            let relay = tx.add_part("ACME", "R1")?;
            tx.set_property(relay, "power_loss_w", Some(Value::Int(2)))?;
            let bare = tx.add_part("ACME", "NOLOSS")?;
            for (tag, part) in [
                ("=A1+CAB1-U1", Some(drive)),
                ("=A1+CAB1-K1", Some(relay)),
                ("=A1+CAB1-K2", Some(bare)),
                ("=A1+CAB1-K3", None),
                ("=A1+FIELD-M1", Some(drive)),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                if let Some(part) = part {
                    tx.assign_part(d, part)?;
                }
            }
            tx.execute(novo_core::Command::SetPlateSize {
                structure: cab,
                plate: Some(MountingPlate {
                    width: 500.0,
                    height: 700.0,
                    margin: 10.0,
                }),
            })?;
            Ok(cab)
        })
        .unwrap();
    (p, cab)
}

#[test]
fn losses_read_parts_with_load_factor_and_flag_the_unknown() {
    let (p, cab) = project();
    let l = thermal::model::losses(&p, cab);
    assert_eq!(l.len(), 4); // not the FIELD motor
    let known: Vec<f64> = l.iter().filter_map(|x| x.1).collect();
    assert_eq!(known.len(), 2);
    close(known.iter().sum::<f64>(), 60.0 + 2.0);
}

#[test]
fn a_device_property_overrides_the_part_and_bad_values_count_as_missing() {
    let (mut p, cab) = project();
    let ids: Vec<u64> = thermal::model::losses(&p, cab)
        .iter()
        .map(|l| l.0)
        .collect();
    p.transact("override", |tx| {
        // K1 (2 W part): device says 10 W. K2 (no loss): a negative value is missing.
        let k1 = novo_core::AnyId::Device(novo_core::DeviceId::from(ids[1]));
        tx.set_property(k1, "power_loss_w", Some(Value::Float(10.0)))?;
        let k2 = novo_core::AnyId::Device(novo_core::DeviceId::from(ids[2]));
        tx.set_property(k2, "power_loss_w", Some(Value::Float(-4.0)))?;
        Ok(())
    })
    .unwrap();
    let l = thermal::model::losses(&p, cab);
    assert_eq!(l[1].1, Some(10.0));
    assert_eq!(l[2].1, None);
}

#[test]
fn bands_for_the_overlay() {
    let s = [source(1, 100.0, 50.0, 50.0, 100.0, 10.0)];
    let m = heat_map(&plate(), &s, 10.0, 0.0);
    let b = m.bands(&DEFAULT_BAND_LIMITS);
    assert_eq!(b.len(), m.cols * m.rows);
    assert_eq!(b[11 + 6 * m.cols], 4);
    assert_eq!(b[0], 0);
}

#[test]
fn fine_cells_are_coarsened_and_negative_loss_is_zero() {
    let big = Plate {
        width: 4000.0,
        height: 3000.0,
        margin: 0.0,
    };
    let m = heat_map(&big, &[], 1.0, 40.0);
    assert!(m.cols * m.rows <= thermal::MAX_CELLS);
    assert!(m.cell_mm >= thermal::MIN_CELL_MM);
    let p = novo_layout::Placement {
        id: 1,
        x: 0.0,
        y: 0.0,
        rotated: false,
        size: novo_layout::Size::from_part(Some(40.0), Some(40.0)),
        clearance: None,
        rail: None,
    };
    let s = thermal::sources(&[p], |_| Some(-5.0));
    assert_eq!(s[0].loss_w, 0.0);
}
