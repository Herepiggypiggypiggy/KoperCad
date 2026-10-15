use novo_core::{DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::thermal::{
    self, DEFAULT_BAND_LIMITS, Enclosure, HeatSource, Installation, Verdict, balance, band_of,
    heat_map, hot_devices, legend, top_third_share,
};
use novo_layout::{Plate, Rect};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

#[test]
fn free_standing_area_matches_the_closed_form() {
    let e = Enclosure::new(800.0, 2000.0, 600.0);
    let (w, h, d) = (0.8, 2.0, 0.6);
    close(e.effective_area(), 1.8 * h * (w + d) + 1.4 * w * d);
}

#[test]
fn wall_mounted_area_matches_the_closed_form() {
    let mut e = Enclosure::new(600.0, 800.0, 250.0);
    e.installation = Installation::Wall;
    let (w, h, d) = (0.6, 0.8, 0.25);
    close(e.effective_area(), 1.4 * w * (h + d) + 1.8 * d * h);
}

#[test]
fn covered_sides_and_top_reduce_the_area() {
    let free = Enclosure::new(600.0, 800.0, 250.0).effective_area();
    let mut e = Enclosure::new(600.0, 800.0, 250.0);
    e.installation = Installation::RowMiddle;
    let middle = e.effective_area();
    close(free - middle, 2.0 * 0.4 * 0.25 * 0.8);
    e.top_covered = true;
    close(middle - e.effective_area(), 0.7 * 0.6 * 0.25);
}

#[test]
fn balance_hand_calculation() {
    // 600 x 800 x 250 wall mounted: Ae = 1.4*0.6*1.05 + 1.8*0.25*0.8 = 1.242 m2.
    let mut e = Enclosure::new(600.0, 800.0, 250.0);
    e.installation = Installation::Wall;
    let t = balance(&e, 200.0, 3, 0);
    close(t.effective_area_m2, 1.242);
    close(t.wall_dissipation_w, 5.5 * 1.242 * 5.0);
    close(t.rise_k, 200.0 / (5.5 * 1.242));
    close(t.cooling_w, 200.0 - 5.5 * 1.242 * 5.0);
    // Sea level: about 3.1 m3K/Wh.
    let v = t.airflow_m3h.unwrap();
    close(v, 3600.0 * t.cooling_w / (1.146 * 1005.0 * 5.0));
    assert!((v / t.cooling_w * 5.0 - 3.126).abs() < 0.01);
    assert_eq!(t.verdict, Verdict::NeedsCooling);
}

#[test]
fn small_loss_needs_no_cooling() {
    let e = Enclosure::new(600.0, 800.0, 250.0);
    let t = balance(&e, 20.0, 2, 0);
    assert_eq!(t.cooling_w, 0.0);
    assert_eq!(t.airflow_m3h, None);
    assert_eq!(t.verdict, Verdict::Ok);
}

#[test]
fn no_fan_when_the_limit_is_not_above_ambient() {
    let mut e = Enclosure::new(600.0, 800.0, 250.0);
    e.max_internal_c = 35.0;
    let t = balance(&e, 100.0, 1, 0);
    close(t.cooling_w, 100.0);
    assert_eq!(t.airflow_m3h, None);
}

#[test]
fn altitude_needs_more_air() {
    let mut e = Enclosure::new(600.0, 800.0, 250.0);
    let sea = balance(&e, 300.0, 1, 0).airflow_m3h.unwrap();
    e.altitude_m = 2000.0;
    assert!(balance(&e, 300.0, 1, 0).airflow_m3h.unwrap() > sea);
}

#[test]
fn missing_data_or_a_guessed_size_is_incomplete() {
    let e = Enclosure::new(600.0, 800.0, 250.0);
    assert_eq!(balance(&e, 300.0, 2, 1).verdict, Verdict::Incomplete);
    let g = Enclosure::around_plate(&Plate {
        width: 500.0,
        height: 700.0,
        margin: 0.0,
    });
    assert!(g.guessed);
    assert_eq!(balance(&g, 300.0, 2, 0).verdict, Verdict::Incomplete);
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
fn estimate_is_incomplete_with_a_guessed_enclosure_and_missing_data() {
    let (p, cab) = project();
    let e = thermal::model::enclosure(&p, cab).unwrap();
    assert!(e.guessed);
    close(e.width_mm, 600.0);
    let t = thermal::model::estimate(&p, cab).unwrap();
    assert_eq!((t.known, t.missing), (2, 2));
    assert_eq!(t.verdict, Verdict::Incomplete);
    close(t.loss_w, 62.0);
}

#[test]
fn enclosure_properties_override_the_guess() {
    let (mut p, cab) = project();
    p.transact("props", |tx| {
        let id = novo_core::AnyId::Structure(cab);
        for (k, v) in [
            ("width_mm", Value::Float(600.0)),
            ("height_mm", Value::Int(800)),
            ("depth_mm", Value::Int(250)),
            ("installation", Value::Text("wall".into())),
            ("material", Value::Text("aluminium".into())),
            ("top_covered", Value::Bool(true)),
            ("ambient_c", Value::Int(30)),
            ("max_internal_c", Value::Int(45)),
            ("altitude_m", Value::Int(500)),
        ] {
            tx.set_property(id, k, Some(v))?;
        }
        Ok(())
    })
    .unwrap();
    let e = thermal::model::enclosure(&p, cab).unwrap();
    assert!(!e.guessed);
    assert_eq!(e.installation, Installation::Wall);
    assert_eq!(e.material, thermal::Material::Aluminium);
    assert!(e.top_covered);
    close(e.max_internal_c, 45.0);
    close(e.altitude_m, 500.0);
}
