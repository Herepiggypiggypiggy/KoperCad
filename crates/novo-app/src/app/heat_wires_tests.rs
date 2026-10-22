//! Heat of the wires in the ducts, from the example project.

use super::heat_wires::*;

/// The example cabinet with every wire sized to `section` mm2.
fn example(section: f64) -> (novo_core::Project, novo_core::StructureId) {
    example_with(section, None)
}

fn example_with(section: f64, factor: Option<f64>) -> (novo_core::Project, novo_core::StructureId) {
    let mut doc = crate::backend::new_document("Example").unwrap();
    super::example::build_example(&mut doc).unwrap();
    let ids: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    doc.transact("Sizes", |tx| {
        for id in ids {
            tx.update::<novo_core::Wire>(id, |w| w.cross_section_mm2 = Some(section))?;
        }
        if let Some(f) = factor {
            tx.set_info(|i| {
                i.properties
                    .insert(WIRE_CURRENT_FACTOR.into(), novo_core::Value::Float(f));
            })?;
        }
        Ok(())
    })
    .unwrap();
    let project = doc.into_project();
    let cab = project
        .all::<novo_core::Structure>()
        .find(|s| s.enclosure)
        .map(|s| s.id)
        .unwrap();
    (project, cab)
}

#[test]
fn power_wires_in_ducts_make_heat_and_control_wires_are_named_as_left_out() {
    let (p, cab) = example(2.5);
    let h = wire_heat(&p, cab);
    let total = h.total_w();
    // Three power wires at 4.58 A, three conductors each; the longest route
    // of the example is 0.78 m, so at most 3 * 3 * 4.58^2 * 0.0206 * 0.78 / 2.5.
    let ceiling = 3.0 * 3.0 * 4.578_f64.powi(2) * 0.020_629 * 2.4 / 2.5;
    assert!(
        total > 0.05 && total < ceiling,
        "{total} W, ceiling {ceiling}"
    );
    assert!(
        h.notes[0].starts_with("Wires in ducts add "),
        "{:?}",
        h.notes
    );
    assert!(h.notes[0].contains("no diversity"), "{:?}", h.notes);
    assert!(
        h.notes[1].contains("calculated from the loads below"),
        "{:?}",
        h.notes
    );
    assert!(
        h.notes.iter().any(|n| n.contains("carry no current")),
        "{:?}",
        h.notes
    );
    assert!(h.by_duct.keys().all(|d| {
        novo_routing::model::ducts(&p, cab)
            .iter()
            .any(|x| x.id == *d)
    }));
}

#[test]
fn heat_goes_with_the_square_of_the_stated_share_of_current() {
    let (p, cab) = example(2.5);
    let full = wire_heat(&p, cab).total_w();
    let (p, cab) = example_with(2.5, Some(0.5));
    let half = wire_heat(&p, cab);
    assert!(
        (half.total_w() / full - 0.25).abs() < 1e-9,
        "{full} {}",
        half.total_w()
    );
    assert!(half.notes[0].contains("scaled to 50 %"), "{:?}", half.notes);
}

#[test]
fn wires_without_a_cross_section_are_named_and_add_nothing() {
    let mut doc = crate::backend::new_document("Example").unwrap();
    super::example::build_example(&mut doc).unwrap();
    let project = doc.into_project();
    let cab = project
        .all::<novo_core::Structure>()
        .find(|s| s.enclosure)
        .map(|s| s.id)
        .unwrap();
    let h = wire_heat(&project, cab);
    assert!(h.by_duct.is_empty());
    assert!(
        h.notes.iter().any(|n| n.contains("no cross-section")),
        "{:?}",
        h.notes
    );
}

#[test]
fn the_heat_reaches_the_scenario_on_the_duct_obstacles() {
    let (p, cab) = example(2.5);
    let h = wire_heat(&p, cab);
    let options = novo_thermal::bridge::BridgeOptions {
        duct_loss_w: h.by_duct.clone(),
        ..Default::default()
    };
    let b = novo_thermal::bridge::scenario_from_project(&p, cab, &options).unwrap();
    let on_ducts: f64 = b.scenario.obstacles.iter().map(|o| o.loss_w).sum();
    assert!((on_ducts - h.total_w()).abs() < 1e-9);
    assert!((b.report.duct_loss_w - h.total_w()).abs() < 1e-9);
}

/// Rewrites the QA fixture (the example cabinet with every wire sized to
/// 2.5 mm2, so its power wires give off heat in the ducts):
/// `cargo test -p novo-app write_wire_heat_fixture -- --ignored`.
#[test]
#[ignore = "writes qa/fixtures/cabinet-wire-heat.novo"]
fn write_wire_heat_fixture() {
    use crate::backend::ProjectBackend;
    let mut backend = crate::backend::StorageBackend::default();
    backend.new_project("Cabinet wire heat (QA)").unwrap();
    let doc = backend.document_mut().unwrap();
    super::example::build_example(doc).unwrap();
    let ids: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    doc.transact("Sizes", |tx| {
        for id in ids {
            tx.update::<novo_core::Wire>(id, |w| w.cross_section_mm2 = Some(2.5))?;
        }
        Ok(())
    })
    .unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/cabinet-wire-heat.novo");
    let _ = std::fs::remove_file(&path);
    backend.save(&path).unwrap();
}
