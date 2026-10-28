//! Field, door and plate: which devices take room in a cabinet.

use novo_core::{DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::enclosure::{Door, Enclosure, Hinge};
use novo_layout::model::{candidates, device_size, set_enclosure, set_plate, without_enclosure};
use novo_layout::site::{Site, SiteSource, site_of};
use novo_layout::sizing_model::sizing_devices;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

/// Cabinet +CP1 with a contactor, a motor, a push button and a 4 kW drive,
/// all without a location in their tag (the only enclosure takes them).
fn cooling_tower(door: bool) -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(tag("+CP1"), "Cabinet", true)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 750.0,
                    height: 950.0,
                    margin: 10.0,
                },
            )?;
            if door {
                set_enclosure(
                    tx,
                    cab,
                    &Enclosure {
                        width: 800.0,
                        height: 1000.0,
                        depth: 300.0,
                        plate_setback: 20.0,
                        door: Some(Door {
                            hinge: Hinge::Left,
                            ..Door::default()
                        }),
                        front_clearance: None,
                        hinge_side_clearance: None,
                    },
                )?;
            }
            for t in ["-K1", "-M1", "-S1", "-T1"] {
                let d = tx.add_device(tag(t), "")?;
                if t == "-T1" {
                    tx.set_property(d, "power_kw", Some(Value::Float(4.0)))?;
                }
            }
            Ok(cab)
        })
        .unwrap();
    (p, cab)
}

fn device<'a>(p: &'a Project, code: &str) -> &'a novo_core::Device {
    p.all::<novo_core::Device>()
        .find(|d| d.tag.device_code() == Some(code))
        .unwrap()
}

#[test]
fn a_motor_is_in_the_field_unless_the_person_says_otherwise() {
    let (mut p, _) = cooling_tower(false);
    assert_eq!(
        site_of(&p, device(&p, "M1")),
        (Site::Field, SiteSource::LetterCode)
    );
    assert_eq!(
        site_of(&p, device(&p, "K1")),
        (Site::Plate, SiteSource::Default)
    );
    let m1 = device(&p, "M1").id;
    p.transact("site", |tx| {
        tx.set_property(m1, "mounting_site", Some(Value::Text("plate".into())))
    })
    .unwrap();
    assert_eq!(
        site_of(&p, device(&p, "M1")),
        (Site::Plate, SiteSource::Stated)
    );
}

#[test]
fn place_all_leaves_the_motor_out_and_keeps_the_rest() {
    let (p, cab) = cooling_tower(false);
    let tags: Vec<String> = candidates(&p, cab)
        .iter()
        .map(|c| c.sort_key.clone())
        .collect();
    assert_eq!(tags.len(), 3, "{tags:?}");
    assert!(tags.iter().all(|t| !t.starts_with('M')), "{tags:?}");
    assert!(without_enclosure(&p, cab).is_empty());
}

#[test]
fn a_drive_is_sized_by_its_power_and_says_it_is_a_guess() {
    let (mut p, _) = cooling_tower(false);
    let size = device_size(&p, device(&p, "T1"));
    assert!(size.guessed);
    assert_eq!((size.width, size.height), (140.0, 185.0));
    // No power, no size to go by: the plain default, also a guess.
    let t1 = device(&p, "T1").id;
    p.transact("clear", |tx| tx.set_property(t1, "power_kw", None))
        .unwrap();
    let size = device_size(&p, device(&p, "T1"));
    assert!(size.guessed);
    assert_eq!((size.width, size.height), novo_layout::DEFAULT_FOOTPRINT_MM);
    // Text from a macro parameter is read too.
    p.transact("text", |tx| {
        tx.set_property(t1, "power_kw", Some(Value::Text("7,5".into())))
    })
    .unwrap();
    assert_eq!(device_size(&p, device(&p, "T1")).width, 180.0);
}

#[test]
fn sizing_counts_the_devices_place_all_would_place() {
    // No door yet: the push button stays on the plate and counts.
    let (p, cab) = cooling_tower(false);
    assert_eq!(candidates(&p, cab).len(), 3);
    assert_eq!(sizing_devices(&p, cab).len(), 3);
    // With a door the button goes there and takes no plate room; the motor
    // never counts.
    let (p, cab) = cooling_tower(true);
    assert_eq!(candidates(&p, cab).len(), 3);
    assert_eq!(sizing_devices(&p, cab).len(), 2);
}

#[test]
fn a_device_can_name_its_own_surface() {
    let (mut p, cab) = cooling_tower(true);
    let (rules, _) = novo_layout::rules::model::surface_rules(&p, cab);
    assert!(rules.standard);
    assert_eq!(rules.surface_for("S"), Some("door"));
    let s1 = device(&p, "S1").id;
    p.transact("plate", |tx| {
        tx.set_property(s1, "mounting_site", Some(Value::Text("plate".into())))
    })
    .unwrap();
    assert_eq!(sizing_devices(&p, cab).len(), 3);
    let specs = novo_layout::rules::model::specs(&p, cab);
    let s = specs.iter().find(|s| s.kind == "S").unwrap();
    assert_eq!(s.surface.as_deref(), Some("plate"));
}

#[test]
fn pilot_devices_wait_on_the_plate_when_the_enclosure_has_no_door() {
    use novo_layout::surface_rules::split;
    use novo_layout::surfaces::Surfaces;
    let (p, cab) = cooling_tower(true);
    let specs = novo_layout::rules::model::specs(&p, cab);
    let (rules, _) = novo_layout::rules::model::surface_rules(&p, cab);
    let plate = novo_layout::Plate {
        width: 750.0,
        height: 950.0,
        margin: 10.0,
    };
    let enclosure = |door: bool| Enclosure {
        width: 800.0,
        height: 1000.0,
        depth: 300.0,
        plate_setback: 20.0,
        door: door.then(Door::default),
        front_clearance: None,
        hinge_side_clearance: None,
    };
    let (with, _) = Surfaces::from_enclosure(&enclosure(true), &plate, &[]);
    let out = split(&specs, &rules, &with);
    assert_eq!(out.others.len(), 1);
    assert_eq!(out.others[0].1.len(), 1);
    assert!(out.problems.is_empty(), "{:?}", out.problems);

    let (without, _) = Surfaces::from_enclosure(&enclosure(false), &plate, &[]);
    let out = split(&specs, &rules, &without);
    assert!(out.others.is_empty());
    assert_eq!(out.plate.len(), specs.len());
    assert_eq!(
        out.problems,
        vec![
            "1 device belongs on the door, but this enclosure has no door yet; it stays on the plate."
                .to_owned()
        ]
    );
}
