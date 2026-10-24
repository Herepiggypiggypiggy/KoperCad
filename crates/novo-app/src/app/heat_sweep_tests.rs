//! The cooling advice window: it follows the project it was started for,
//! turns a candidate into the openings the Vents and fans list holds, and
//! says what it is (a model) in its window.

use eframe::egui;
use novo_thermal::model::{Opening, Placement};
use novo_thermal::sweep::{
    Candidate, Delivered, DeviceChange, Outcome, Pairing, Piece, Reading, Sweep,
};

use super::heat_settings::{HeatSettings, Kind, Wall};
use super::heat_sweep::{
    CoolingAdvice, Found, add_pair, real_fans, settings_for, spec_of, specs_of,
};
use novo_thermal::cooling::{Kind as FanKind, Product, Source};

fn product(name: &str, kind: FanKind, flow: f64, source: Source) -> Product {
    Product {
        name: name.into(),
        kind,
        airflow_m3h: flow,
        cooling_w: 0.0,
        source,
    }
}

fn fan(placement: Placement, side: f64, flow: f64) -> Piece {
    Piece {
        opening: Opening {
            placement,
            width_mm: side,
            height_mm: side,
        },
        fan_flow_m3h: Some(flow),
    }
}

fn vent(placement: Placement) -> Piece {
    Piece {
        opening: Opening {
            placement,
            width_mm: 200.0,
            height_mm: 100.0,
        },
        fan_flow_m3h: None,
    }
}

fn candidate() -> Candidate {
    Candidate {
        pairing: Pairing::FanInVentOut,
        inlet: fan(
            Placement::Front {
                x_mm: 400.0,
                y_mm: 1000.0,
            },
            150.0,
            400.0,
        ),
        outlet: vent(Placement::Top { x_mm: 300.0 }),
    }
}

#[test]
fn a_candidate_becomes_the_matching_openings() {
    let c = candidate();
    let [i, o] = specs_of(&c);
    assert_eq!(
        (i.kind, i.wall, i.exhaust),
        (Kind::FilterFan, Wall::Door, false)
    );
    assert_eq!((i.u_mm, i.v_mm, i.flow_m3h), (400.0, 1000.0, 400.0));
    assert_eq!((o.kind, o.wall, o.u_mm), (Kind::Grille, Wall::Top, 300.0));
    assert_eq!((o.width_mm, o.height_mm), (200.0, 100.0));
    // An exhaust fan is a fan that blows out.
    let e = spec_of(&fan(Placement::Left { y_mm: 200.0 }, 120.0, 250.0), false);
    assert_eq!((e.kind, e.wall, e.exhaust), (Kind::Fan, Wall::Left, true));
}

#[test]
fn adding_a_pair_is_one_undo_step() {
    let mut s = HeatSettings::default();
    add_pair(&mut s, &candidate());
    assert_eq!(s.vents.len(), 2);
    assert_eq!(s.selected, Some(1));
    s.undo();
    assert!(s.vents.is_empty(), "both leave with one undo");
}

fn reading(margin: f64, over: usize, air: f64) -> Reading {
    Reading {
        worst_margin_k: Some(margin),
        over,
        max_air_c: air,
        mean_air_c: air - 5.0,
        margins_k: vec![Some(margin)],
        surface_c: vec![60.0],
        unsettled: false,
    }
}

fn found(key: u64, gain: f64) -> Found {
    let c = candidate();
    Found::for_test(
        Sweep {
            before: reading(-14.6, 2, 61.0),
            best: vec![Outcome {
                candidate: c,
                reading: reading(-14.6 + gain, 0, 39.0),
                gain_k: gain,
                clear: gain >= 1.0,
                devices: vec![DeviceChange {
                    id: 1,
                    surface_before_c: 75.0,
                    surface_after_c: 52.0,
                    margin_before_k: Some(-14.6),
                    margin_after_k: Some(2.0),
                }],
                inlet: Some(Delivered {
                    is_fan: true,
                    flow_m3h: 246.0,
                    speed_ms: 1.2,
                }),
                outlet: Some(Delivered {
                    is_fan: false,
                    flow_m3h: -246.0,
                    speed_ms: 0.8,
                }),
                rank: 1,
            }],
            tried: 134,
            skipped: 0,
            sizing_flow_m3h: 410.0,
            partial: false,
            notes: vec!["Model estimate: planning values.".into()],
        },
        key,
    )
}

/// The text the window draws after it has settled (the first frames of a
/// window are not drawn).
fn render(a: &mut CoolingAdvice, key: u64) -> String {
    let ctx = egui::Context::default();
    let mut text = String::new();
    for _ in 0..4 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = a.show(ui.ctx(), 1, key, egui::pos2(10.0, 10.0));
        });
        out.textures_delta.clear();
        text = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    text
}

fn open(gain: f64) -> CoolingAdvice {
    let mut a = CoolingAdvice::default();
    a.request();
    a.set_found_for_test(found(7, gain));
    a
}

#[test]
fn the_window_names_the_model_and_the_numbers() {
    let text = render(&mut open(16.6), 7);
    for want in [
        "Cooling advice",
        "Model estimate",
        "-14.6 K",
        "+2.0 K",
        "Filter fan in, vent out",
        "gains 16.6 K",
        "Add these two to Vents and fans",
        "246 m³/h",
    ] {
        assert!(text.contains(want), "missing {want:?} in:\n{text}");
    }
}

#[test]
fn a_changed_cabinet_makes_the_result_stale() {
    let text = render(&mut open(16.6), 8);
    assert!(text.contains("cabinet changed"), "{text}");
}

#[test]
fn a_gain_under_a_kelvin_is_called_noise() {
    let text = render(&mut open(0.4), 7);
    assert!(text.contains("within the model's noise"), "{text}");
    assert!(text.contains("No position or size wins 1 K"), "{text}");
}

/// Adding from the Cabinet, before the 3D view has looked at the list, keeps
/// the pair: it is written to the project and the 3D list and cabinet show it.
#[test]
fn a_pair_added_from_the_cabinet_reaches_the_3d_list_and_the_project() {
    use super::cabinet::Ask;
    use super::view3d_tests::drive_with_cabinet;
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let id = {
        let project = d.app.backend.document().unwrap().project();
        project
            .structures()
            .find(|s| s.enclosure && s.plate.is_some())
            .map(|s| s.id)
            .expect("a cabinet")
    };
    let c = Candidate {
        pairing: Pairing::VentInFanOut,
        inlet: vent(Placement::Left { y_mm: 1000.0 }),
        outlet: fan(Placement::Left { y_mm: 100.0 }, 150.0, 250.0),
    };
    d.app.cabinet_ask(Ask::CoolingPair(c), Some(id), None, &[]);
    assert!(
        !super::heat_openings_store::stored_text(d.app.backend.document().unwrap().project(), id)
            .is_empty(),
        "the project holds the pair"
    );
    assert_eq!(d.app.cabinet.heat.settings.vents.len(), 2);
    for _ in 0..8 {
        d.frame_sized(egui::vec2(1366.0, 768.0), Vec::new());
    }
    assert_eq!(
        d.app.cabinet.heat.settings.vents.len(),
        2,
        "the 3D view kept them"
    );
    assert_eq!(d.app.view3d.vent_markers.len(), 2);
}

#[test]
fn the_window_closes_when_the_view_changes() {
    let ctx = egui::Context::default();
    let mut a = open(16.6);
    for view in [1, 1, 2] {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = a.show(ui.ctx(), view, 7, egui::pos2(10.0, 10.0));
        });
        out.textures_delta.clear();
    }
    assert!(!a.is_open() && a.found.is_none());
}

/// The sweep's own "Model estimate" note is hidden as a duplicate of the
/// footer, so the footer has to carry what that note says about trust.
#[test]
fn the_footer_keeps_the_caveats_of_the_hidden_note() {
    for caveat in [
        "wire heat in the ducts",
        "depth of the cabinet",
        "Not validated against measurements",
    ] {
        assert!(super::heat_sweep::FOOTER.contains(caveat), "{caveat}");
    }
}

#[test]
fn the_ladder_comes_from_real_fans_and_falls_back_to_the_generic_one() {
    let generic = settings_for(&[]).fan_flows_m3h;
    assert_eq!(generic, novo_thermal::sweep::FAN_FLOWS_M3H.to_vec());
    let mine = vec![
        product("Fan B", FanKind::Fan, 300.0, Source::Mine),
        product("Fan A", FanKind::FilterFan, 120.0, Source::Part("A".into())),
        product("Fan A again", FanKind::Fan, 120.0, Source::Mine),
    ];
    assert_eq!(settings_for(&mine).fan_flows_m3h, vec![120.0, 300.0]);
}

#[test]
fn only_real_fans_count_not_planning_sizes_or_units() {
    let all = vec![
        product("Generic", FanKind::FilterFan, 250.0, Source::Planning),
        product("Unit", FanKind::CoolingUnit, 0.0, Source::Mine),
        product("Mine", FanKind::Fan, 90.0, Source::Mine),
    ];
    let real = real_fans(all);
    assert_eq!(real.len(), 1);
    assert_eq!(real[0].name, "Mine");
}

#[test]
fn many_real_sizes_are_thinned_to_a_bounded_ladder() {
    let many: Vec<Product> = (1..=30)
        .map(|i| product("F", FanKind::Fan, 50.0 * i as f64, Source::Mine))
        .collect();
    let flows = settings_for(&many).fan_flows_m3h;
    assert!(flows.len() <= 8);
    assert_eq!(flows.first(), Some(&50.0));
    assert_eq!(flows.last(), Some(&1500.0));
}

#[test]
fn the_window_names_the_article_behind_a_fan() {
    let mut a = CoolingAdvice::default();
    a.request();
    let fans = vec![product("SK 3238", FanKind::FilterFan, 400.0, Source::Mine)];
    a.set_found_for_test(found(7, 16.6).with_fans_for_test(fans));
    let text = render(&mut a, 7);
    assert!(text.contains("Fan: SK 3238 (400 m³/h free air)"), "{text}");
    assert!(text.contains("from 1 fan(s) in your parts"), "{text}");
    let plain = render(&mut open(16.6), 7);
    assert!(!plain.contains("Fan: "), "{plain}");
}
