//! The chosen enclosure appears in the bill of materials and the cabinet
//! sheet, and follows the spec (handoff/cabinet-start/CHAIN.md).

use novo_core::{DeviceTag, MountingPlate, Project, Properties};
use novo_layout::spec::{EnclosureSpec, Field, Source};
use novo_reports::{BillOfMaterials, Report};

fn spec(width: f64, source: Source) -> EnclosureSpec {
    let f = |v| Some(Field::new(v, source));
    EnclosureSpec {
        article: "TEST.1".into(),
        series: "Test".into(),
        width: f(width),
        height: f(1800.0),
        depth: f(500.0),
        wall: f(2.0),
        ..EnclosureSpec::default()
    }
}

fn project(spec: Option<&EnclosureSpec>) -> Project {
    let mut p = Project::new("t");
    let mut props = Properties::new();
    if let Some(s) = spec {
        s.write(&mut props);
    }
    p.transact("cabinet", |tx| {
        let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
        novo_layout::model::set_plate(
            tx,
            cab,
            MountingPlate {
                width: 560.0,
                height: 1700.0,
                margin: 10.0,
            },
        )?;
        for (k, v) in props {
            tx.set_property(cab, &k, Some(v))?;
        }
        Ok(())
    })
    .unwrap();
    p
}

#[test]
fn a_project_without_an_enclosure_choice_has_no_extra_sections() {
    let out = BillOfMaterials.generate(&project(None));
    assert!(out.section("enclosure").is_none());
    assert!(out.section("cabinet").is_none());
}

#[test]
fn the_bom_and_cabinet_sheet_follow_the_spec() {
    let a = BillOfMaterials.generate(&project(Some(&spec(600.0, Source::Catalogue))));
    let b = BillOfMaterials.generate(&project(Some(&spec(800.0, Source::Step))));
    let rows = |o: &novo_reports::ReportOutput, k: &str| o.section(k).unwrap().text_rows();
    let (ea, eb) = (rows(&a, "enclosure"), rows(&b, "enclosure"));
    assert_eq!(ea.len(), 3, "enclosure, plate and the estimate notice");
    assert!(ea[2][3].starts_with("Estimate from catalogue"), "{ea:?}");
    assert_eq!(ea[0][2], "TEST.1");
    assert!(ea[0][3].contains("600 x 1800 x 500"), "{ea:?}");
    assert!(eb[0][3].contains("800 x 1800 x 500"), "{eb:?}");
    assert_eq!(ea[0][4], "catalogue");
    assert_eq!(eb[0][4], "from your STEP");
    assert_eq!(eb.len(), 2, "no notice for STEP numbers");
    assert_eq!(eb[0][4], "from your STEP");
    let (ca, cb) = (rows(&a, "cabinet"), rows(&b, "cabinet"));
    assert_ne!(ca, cb);
    assert!(
        ca.iter()
            .any(|r| r[1] == "Outside width" && r[2] == "600 mm" && r[3] == "catalogue")
    );
    assert!(
        cb.iter()
            .any(|r| r[1] == "Outside width" && r[2] == "800 mm" && r[3] == "from your STEP")
    );
}

#[test]
fn a_catalogue_and_typed_mix_shows_the_notice_and_the_weakest_source() {
    let mut s = spec(600.0, Source::Typed);
    s.wall = Some(Field::new(2.0, Source::Catalogue));
    let out = BillOfMaterials.generate(&project(Some(&s)));
    let e = out.section("enclosure").unwrap().text_rows();
    assert_eq!(e[0][4], "typed by you", "typed is weaker than catalogue");
    assert!(e.last().unwrap()[3].starts_with("Estimate from catalogue"));
    let c = out.section("cabinet").unwrap().text_rows();
    assert!(
        c.iter()
            .any(|r| r[1] == "Note" && r[2].starts_with("Estimate from catalogue"))
    );
}
