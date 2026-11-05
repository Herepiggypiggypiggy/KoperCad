//! The chosen enclosure reaches the drilling plan: change the spec and the
//! plan changes (handoff/cabinet-start/CHAIN.md).

use novo_core::{DeviceTag, MountingPlate, Project, Properties, StructureId};
use novo_layout::model::{drill_plan, set_plate};
use novo_layout::spec::{EnclosureSpec, Field, Source};

fn spec(width: f64, source: Source) -> EnclosureSpec {
    let f = |v| Some(Field::new(v, source));
    EnclosureSpec {
        article: "TEST.1".into(),
        series: "Test".into(),
        width: f(width),
        height: f(1000.0),
        depth: f(400.0),
        wall: f(2.0),
        plate_setback: f(25.0),
        ..EnclosureSpec::default()
    }
}

fn project() -> (Project, StructureId) {
    let mut p = Project::new("t");
    let cab = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 500.0,
                    height: 900.0,
                    margin: 10.0,
                },
            )?;
            Ok(cab)
        })
        .unwrap()
        .0;
    (p, cab)
}

fn choose(p: &mut Project, cab: StructureId, spec: &EnclosureSpec) {
    let mut props = Properties::new();
    spec.write(&mut props);
    p.transact("enclosure", |tx| {
        for (k, v) in props {
            tx.set_property(cab, &k, Some(v))?;
        }
        Ok(())
    })
    .unwrap();
}

fn lines(p: &Project, cab: StructureId) -> Vec<String> {
    drill_plan(p, cab, "t")
        .map(|d| d.enclosure)
        .unwrap_or_default()
}

#[test]
fn no_enclosure_leaves_the_plan_as_it_was() {
    let (p, cab) = project();
    assert!(lines(&p, cab).is_empty());
}

#[test]
fn changing_the_spec_changes_the_drill_plan() {
    let (mut p, cab) = project();
    choose(&mut p, cab, &spec(600.0, Source::Catalogue));
    let wide = lines(&p, cab);
    assert!(
        wide.iter().any(|l| l.contains("inside 596 x 996 x 396")),
        "{wide:?}"
    );
    assert!(!wide.iter().any(|l| l.contains("does not fit")));
    choose(&mut p, cab, &spec(400.0, Source::Step));
    let narrow = lines(&p, cab);
    assert!(
        narrow.iter().any(|l| l.contains("does not fit")),
        "{narrow:?}"
    );
    assert!(narrow.iter().any(|l| l.contains("from your STEP")));
    assert_ne!(wide, narrow);
}
