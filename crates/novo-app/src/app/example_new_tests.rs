//! The three plant examples (roller door, conveyor line, filling line) build
//! clean and are complete: every wire routed through the ducts, a laid-out
//! cabinet, heat, drilling, a parts list without gaps.

use novo_core::{DeviceTag, Document, Result as ModelResult};
use novo_layout::model::load;
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};
use novo_thermal::{Options, simulate_steady_with};

type Build = fn(&mut Document) -> ModelResult<novo_core::ChangeSet>;

const NEW: &[(&str, Build)] = &[
    ("door", super::example_door::build_door_example as Build),
    (
        "conveyor",
        super::example_conveyor::build_conveyor_example as Build,
    ),
    (
        "filling",
        super::example_filling::build_filling_example as Build,
    ),
];

fn built(build: Build) -> Document {
    let mut doc = Document::default();
    build(&mut doc).unwrap();
    doc
}

fn cabinet(doc: &Document) -> novo_core::StructureId {
    let tag = DeviceTag::parse("+CAB").unwrap();
    doc.project()
        .structures()
        .find(|s| s.tag == tag)
        .expect("a cabinet")
        .id
}

#[test]
fn the_new_examples_open_without_a_warning_or_an_error() {
    for (name, build) in NEW {
        let doc = built(*build);
        let bad: Vec<_> = novo_checks::check(doc.project())
            .into_iter()
            .filter(|p| p.severity != novo_checks::Severity::Info)
            .map(|p| format!("{:?}: {}", p.severity, p.message))
            .collect();
        assert!(bad.is_empty(), "{name}: {bad:#?}");
    }
}

#[test]
fn the_new_examples_have_every_device_placed_and_every_wire_routed() {
    for (name, build) in NEW {
        let doc = built(*build);
        let p = doc.project();
        let cab = cabinet(&doc);
        let layout = load(p, cab).expect("a plate");
        assert!(
            !layout.ducts.is_empty() && !layout.rails.is_empty(),
            "{name}"
        );
        let overlaps = novo_layout::duct_overlaps(&layout.ducts, &layout.rails, &layout.placements);
        assert!(overlaps.is_empty(), "{name}: {overlaps:?}");
        let placed: std::collections::BTreeSet<u64> =
            layout.placements.iter().map(|q| q.id).collect();
        let loc = DeviceTag::parse("+CAB").unwrap().location;
        for d in p.devices() {
            if d.tag.location == loc {
                assert!(placed.contains(&d.id.raw()), "{name}: {} not placed", d.tag);
            }
        }
        let routed = novo_routing::model::route_project(p);
        println!(
            "{name}: {} routes, {} unrouted, {} cross legs, {} unknown height, {} estimated",
            routed.routes.len(),
            routed.unrouted.len(),
            routed.cross_legs.len(),
            routed.unknown_height.len(),
            routed.estimated.len()
        );
        for (_, plan) in &routed.plans {
            println!("{name}: plan problems {:?}", plan.problems);
        }
        // Every wire between two devices of the cabinet is routed. (Wires
        // to the field devices leave the plate and wires to potential feeds
        // are drawing conventions; neither has a route on the plate.)
        let in_cabinet = |pin: novo_core::PinRef| {
            novo_routing::model::device_of_pin(p, pin)
                .and_then(|d| p.device(d))
                .is_some_and(|d| d.tag.location == loc)
        };
        let missing: Vec<_> = novo_routing::model::wire_connections(p)
            .into_iter()
            .filter(|c| in_cabinet(c.from) && in_cabinet(c.to))
            .filter(|c| routed.unrouted.contains(&c.id))
            .map(|c| c.id)
            .collect();
        assert!(missing.is_empty(), "{name}: unrouted {missing:?}");
        let inside = novo_routing::model::wire_connections(p)
            .into_iter()
            .filter(|c| in_cabinet(c.from) && in_cabinet(c.to))
            .count();
        assert!(inside >= 10, "{name}: only {inside} cabinet wires");
    }
}

#[test]
fn the_new_examples_have_heat_inside_limits_drilling_and_a_parts_list_without_gaps() {
    use novo_reports::Report;
    for (name, build) in NEW {
        let doc = built(*build);
        let cab = cabinet(&doc);
        let layout = load(doc.project(), cab).expect("a plate");
        assert!(
            !novo_layout::drill::drill_table(&layout, &[])
                .summary()
                .is_empty(),
            "{name}: no drilling"
        );
        let bridge = scenario_from_project(doc.project(), cab, &BridgeOptions::default())
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let result = simulate_steady_with(
            &bridge.scenario,
            Options {
                bands: false,
                advice: false,
            },
        )
        .unwrap();
        let tightest = result
            .devices
            .iter()
            .filter_map(|d| d.margin_k)
            .fold(f64::INFINITY, f64::min);
        assert!(tightest >= 0.0, "{name}: tightest margin {tightest:.1} K");
        let out = novo_reports::BillOfMaterials.generate(doc.project());
        let sections: Vec<_> = out
            .sections
            .iter()
            .map(|s| (s.key.clone(), s.rows.len()))
            .collect();
        assert!(
            sections.iter().any(|(k, n)| k == "parts" && *n >= 10),
            "{name}: {sections:?}"
        );
        assert!(
            sections.iter().all(|(k, _)| k != "without_parts"),
            "{name}: {sections:?}"
        );
    }
}
