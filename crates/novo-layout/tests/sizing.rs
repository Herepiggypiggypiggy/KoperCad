use novo_layout::sizing::*;

fn dev(w: f64, h: f64, loss: f64) -> SizingDevice {
    SizingDevice {
        width: w,
        height: h,
        depth: 0.0,
        power_loss: loss,
        load_factor: 1.0,
        clearance: None,
        guessed: false,
    }
}

fn catalog() -> EnclosureCatalog {
    let (c, errors) = EnclosureCatalog::from_csv(
        "name;width_mm;height_mm;depth_mm;plate_width_mm;plate_height_mm;margin_mm;material\n\
         Small;400;500;210;350;450;10;steel\n\
         Medium;600;800;300;550;750;10;steel\n\
         Large;800;1000;300;750;950;10;steel\n",
    );
    assert!(errors.is_empty(), "{errors:?}");
    c
}

#[test]
fn csv_accepts_spreadsheet_exports_and_reports_bad_rows() {
    let (c, e) =
        EnclosureCatalog::from_csv("Naam;b;h\nB;x;500;210;350;450\nC;1;2;3;4;5;0;unobtainium\n");
    assert_eq!(c.items.len(), 0);
    assert_eq!(e.len(), 3);
    let (c, e) = EnclosureCatalog::from_csv("A;400,5;500;210;350;450;;RVS\n");
    assert!(e.is_empty());
    assert_eq!(c.items[0].width, 400.5);
    assert_eq!(c.items[0].material, Material::Stainless);
}

#[test]
fn csv_round_trips_and_upsert_replaces() {
    let mut c = catalog();
    let again = EnclosureCatalog::from_csv(&c.to_csv()).0;
    assert_eq!(c, again);
    let mut m = c.items[0].clone();
    m.name = "SMALL".into();
    m.depth = 250.0;
    c.upsert(m);
    assert_eq!(c.items.len(), 3);
    assert_eq!(c.items[0].depth, 250.0);
    assert!(c.remove("large"));
    assert!(!c.remove("large"));
}

#[test]
fn surface_of_a_free_standing_box() {
    // 0.6 x 0.8 x 0.3 on the floor: top 0.18*1.4, no bottom, front/rear 0.48*0.9 each,
    // sides 0.24*0.9 each.
    let a = effective_surface(600.0, 800.0, 300.0, Installation::FreeStanding);
    let want = 0.18 * 1.4 + 2.0 * 0.48 * 0.9 + 2.0 * 0.24 * 0.9;
    assert!((a - want).abs() < 1e-9);
    assert!(effective_surface(600.0, 800.0, 300.0, Installation::WallInRow) < a);
}

#[test]
fn picks_the_smallest_enclosure_that_holds_the_devices() {
    let devices: Vec<_> = (0..10).map(|_| dev(45.0, 90.0, 5.0)).collect();
    let r = recommend(&devices, &catalog(), &SizingRequest::default());
    // 10 * 55*100 = 55000 / 0.8 / 0.8 = 85938 mm2; Small plate is 330*430 = 141900.
    assert!((r.required_plate_area - 85937.5).abs() < 1.0);
    assert_eq!(r.enclosure.unwrap().name, "Small");
    assert!(r.fill.unwrap() < 0.5);
}

#[test]
fn steps_up_when_area_runs_out() {
    let devices: Vec<_> = (0..30).map(|_| dev(45.0, 90.0, 0.0)).collect();
    let r = recommend(&devices, &catalog(), &SizingRequest::default());
    assert_eq!(r.enclosure.unwrap().name, "Medium");
    assert!(r.notes.contains(&Note::NoLossData));
}

#[test]
fn a_device_larger_than_every_plate_is_named() {
    let devices = vec![dev(45.0, 90.0, 1.0), dev(900.0, 900.0, 1.0)];
    let r = recommend(&devices, &catalog(), &SizingRequest::default());
    assert!(r.enclosure.is_none());
    assert!(r.notes.contains(&Note::NothingFits));
    assert!(r.notes.contains(&Note::DeviceTooBig(1)));
}

#[test]
fn depth_is_checked_against_the_enclosure() {
    let mut d = dev(100.0, 100.0, 0.0);
    d.depth = 250.0;
    let r = recommend(&[d], &catalog(), &SizingRequest::default());
    // 250 + 20 reserve > 210: Small is skipped.
    assert_eq!(r.enclosure.unwrap().name, "Medium");
}

#[test]
fn empty_catalog_still_gives_the_required_area() {
    let r = recommend(
        &[dev(100.0, 100.0, 0.0)],
        &EnclosureCatalog::new(),
        &SizingRequest::default(),
    );
    assert!(r.enclosure.is_none());
    assert!(r.required_plate_area > 0.0);
    assert!(r.notes.contains(&Note::NoCatalog));
}

#[test]
fn heat_within_wall_dissipation_needs_no_cooling() {
    let c = catalog();
    let medium = &c.items[1];
    let req = SizingRequest::default();
    let t = thermal(medium, 20.0, &req);
    assert!(t.natural_ok);
    assert_eq!(t.cooling_needed, 0.0);
    assert!(t.fan_airflow.is_none());
    assert!(t.inside_temperature < req.inside_max);
}

#[test]
fn heat_above_dissipation_needs_cooling_and_a_fan_flow() {
    let c = catalog();
    let medium = &c.items[1];
    let req = SizingRequest::default();
    let t = thermal(medium, 300.0, &req);
    assert!(!t.natural_ok);
    assert!((t.cooling_needed - (300.0 - t.dissipation)).abs() < 1e-9);
    let flow = t.fan_airflow.unwrap();
    assert!((flow - 3.1 * t.cooling_needed / 5.0).abs() < 1e-9);
}

#[test]
fn a_larger_enclosure_that_cools_naturally_is_offered() {
    // 8 small devices fit Small, but 40 W is too much for it and fine for Medium.
    let mut devices: Vec<_> = (0..8).map(|_| dev(45.0, 90.0, 5.0)).collect();
    devices[0].load_factor = 1.0;
    let r = recommend(&devices, &catalog(), &SizingRequest::default());
    assert_eq!(r.enclosure.as_ref().unwrap().name, "Small");
    assert!(!r.thermal.as_ref().unwrap().natural_ok);
    assert_eq!(r.natural.as_ref().unwrap().name, "Medium");
    assert!(
        r.notes
            .contains(&Note::LargerCoolsNaturally("Medium".into()))
    );
}

#[test]
fn load_factor_scales_the_loss() {
    let mut d = dev(10.0, 10.0, 100.0);
    d.load_factor = 0.5;
    assert_eq!(total_loss(&[d]), 50.0);
}

#[test]
fn free_standing_matches_the_handbook_closed_form() {
    let a = effective_surface(600.0, 800.0, 300.0, Installation::FreeStanding);
    let closed = 1.8 * 0.8 * (0.6 + 0.3) + 1.4 * 0.6 * 0.3;
    assert!((a - closed).abs() < 1e-9);
}
