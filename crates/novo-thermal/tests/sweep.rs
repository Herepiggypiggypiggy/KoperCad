//! The vent and fan sweep: what it reports must show up when the winning
//! pair is added by hand and the model is run again.

mod common;

use common::*;
use novo_thermal::sweep::{self, Control, Pairing, Settings};
use novo_thermal::*;

fn hot_cabinet() -> Scenario {
    let mut vfd = device(1, 300.0, 700.0, 120.0, 200.0, 165.0);
    vfd.height_mm = Some(150.0);
    vfd.rated_max_ambient_c = Some(40.0);
    let mut above = device(2, 300.0, 450.0, 100.0, 100.0, 25.0);
    above.rated_max_ambient_c = Some(45.0);
    let mut plc = device(3, 500.0, 500.0, 120.0, 150.0, 20.0);
    plc.rated_max_ambient_c = Some(55.0);
    let mut sc = scenario(vec![vfd, above, plc]);
    sc.ambient_c = 35.0;
    sc
}

#[test]
fn a_cabinet_with_margin_is_not_swept() {
    let sc = scenario(vec![device(1, 300.0, 300.0, 100.0, 100.0, 5.0)]);
    let s = sweep::sweep_with(&sc, &sweep::Settings::quick(), &Control::default()).unwrap();
    assert!(s.best.is_empty());
    assert_eq!(s.tried, 0);
    assert!(s.notes.iter().any(|n| n.contains("nothing to fix")));
    assert!(s.notes.iter().any(|n| n.contains("Model estimate")));
}

#[test]
fn the_sweep_ranks_and_its_numbers_reproduce() {
    let sc = hot_cabinet();
    let s = sweep::sweep_with(&sc, &sweep::Settings::quick(), &Control::default()).unwrap();
    assert!(!s.best.is_empty() && s.best.len() <= 4, "{}", s.best.len());
    // Ranked 1.. in order, and the list is ordered best first by overs then margin.
    for (i, o) in s.best.iter().enumerate() {
        assert_eq!(o.rank, i + 1);
    }
    let w = s.winner().expect("a clear winner on this cabinet");
    assert!(w.gain_k >= sweep::SIGNIFICANT_K);
    assert!(w.reading.worst_margin_k.unwrap() > s.before.worst_margin_k.unwrap());
    // Add the pair by hand and run the model on the finalist grid: same numbers.
    let mut by_hand = w.candidate.apply(&sc);
    by_hand.params.cell_mm = sweep::FINE_CELL_MM;
    let r = simulate_steady_with(
        &by_hand,
        Options {
            bands: false,
            advice: false,
        },
    )
    .unwrap();
    let margin = r
        .devices
        .iter()
        .filter_map(|d| d.margin_k)
        .fold(f64::MAX, f64::min);
    assert!(
        (margin - w.reading.worst_margin_k.unwrap()).abs() < 1.5,
        "reported {:?} vs by hand {margin}",
        w.reading.worst_margin_k
    );
    // The flow the openings really pass is reported for the winner.
    assert!(w.inlet.is_some() && w.outlet.is_some());
}

#[test]
fn passive_and_powered_options_both_show() {
    let s = sweep::sweep_with(
        &hot_cabinet(),
        &sweep::Settings::quick(),
        &Control::default(),
    )
    .unwrap();
    let kinds: Vec<Pairing> = s.best.iter().map(|o| o.candidate.pairing).collect();
    assert!(kinds.iter().any(|k| k.fans() == 0), "{kinds:?}");
    assert!(kinds.iter().any(|k| k.fans() > 0), "{kinds:?}");
    let mut seen = kinds.clone();
    seen.dedup();
    assert_eq!(seen.len(), kinds.len().min(4));
}

#[test]
fn a_stopped_sweep_says_so() {
    let control = Control::default();
    control
        .cancel
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let s = sweep::sweep_with(&hot_cabinet(), &Settings::default(), &control).unwrap();
    assert!(s.partial);
    assert!(s.notes.iter().any(|n| n.contains("stopped")));
}

#[test]
fn the_openings_stay_inside_their_walls() {
    let sc = hot_cabinet();
    let s = sweep::sweep_with(&sc, &sweep::Settings::quick(), &Control::default()).unwrap();
    let (w, h) = (
        sc.enclosure.interior_width_mm(),
        sc.enclosure.interior_height_mm(),
    );
    for o in &s.best {
        for p in [&o.candidate.inlet, &o.candidate.outlet] {
            match p.opening.placement {
                Placement::Top { x_mm } | Placement::Bottom { x_mm } => {
                    assert!(
                        x_mm - 0.5 * p.opening.width_mm >= 0.0
                            && x_mm + 0.5 * p.opening.width_mm <= w
                    );
                }
                Placement::Front { x_mm, y_mm } => {
                    assert!(
                        x_mm - 0.5 * p.opening.width_mm >= 0.0
                            && x_mm + 0.5 * p.opening.width_mm <= w
                    );
                    assert!(
                        y_mm - 0.5 * p.opening.height_mm >= 0.0
                            && y_mm + 0.5 * p.opening.height_mm <= h
                    );
                }
                Placement::Left { y_mm } | Placement::Right { y_mm } => {
                    assert!(
                        y_mm - 0.5 * p.opening.width_mm >= 0.0
                            && y_mm + 0.5 * p.opening.width_mm <= h
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "timing and listing; run with --ignored --nocapture"]
fn show_the_sweep() {
    let t = std::time::Instant::now();
    let s = sweep::sweep(&hot_cabinet()).unwrap();
    eprintln!("{} tries in {:.1} s", s.tried, t.elapsed().as_secs_f64());
    let e = hot_cabinet().enclosure;
    eprintln!(
        "before {:?} over {} air {:.1}",
        s.before.worst_margin_k, s.before.over, s.before.max_air_c
    );
    for o in &s.best {
        eprintln!(
            "#{} {} | {} | margin {:?} over {} air {:.1} gain {:.1} clear {} in {:?} out {:?}",
            o.rank,
            o.candidate.pairing.label(),
            o.candidate.describe(&e),
            o.reading.worst_margin_k,
            o.reading.over,
            o.reading.max_air_c,
            o.gain_k,
            o.clear,
            o.inlet.as_ref().map(|d| d.flow_m3h),
            o.outlet.as_ref().map(|d| d.flow_m3h)
        );
    }
    for n in &s.notes {
        eprintln!("note: {n}");
    }
}
