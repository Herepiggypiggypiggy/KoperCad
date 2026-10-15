//! Advice is tried in the model: what it promises must show up when the
//! change is made and the full model is run again.

mod common;

use common::*;
use novo_thermal::*;

fn coarse(mut sc: Scenario) -> Scenario {
    sc.params.cell_mm = 40.0;
    sc
}

fn crowded() -> Scenario {
    let mut hot = device(1, 300.0, 700.0, 120.0, 200.0, 90.0);
    hot.height_mm = Some(150.0);
    hot.rated_max_surface_c = Some(80.0);
    let mut above = device(2, 300.0, 450.0, 100.0, 100.0, 25.0);
    above.rated_max_ambient_c = Some(40.0);
    coarse(scenario(vec![
        hot,
        above,
        device(3, 500.0, 700.0, 100.0, 100.0, 15.0),
    ]))
}

#[test]
fn nothing_to_fix_means_no_advice() {
    let r = simulate_steady_with_advice(&coarse(scenario(vec![device(
        1, 300.0, 300.0, 100.0, 100.0, 5.0,
    )])))
    .unwrap();
    assert!(r.recommendations.is_empty());
    assert!(r.hot_spots.iter().all(|h| h.kind == HotSpotKind::Air));
}

#[test]
fn an_overheating_device_gets_ranked_verified_advice() {
    let sc = crowded();
    let r = simulate_steady_with_advice(&sc).unwrap();
    assert_eq!(r.devices[0].status, Status::Over);
    assert!(!r.recommendations.is_empty());
    for (i, rec) in r.recommendations.iter().enumerate() {
        assert_eq!(rec.rank, i + 1);
        let (b, a) = (
            rec.worst_margin_before_k.unwrap(),
            rec.worst_margin_after_k.unwrap(),
        );
        assert!(rec.max_air_after_c <= rec.max_air_before_c + 1.0 || a > b);
        assert!(!rec.text.is_empty());
    }
    // The hot spot list names the overheating device first.
    assert_eq!(r.hot_spots[0].kind, HotSpotKind::Device(1));
}

#[test]
fn a_loss_cut_works_when_made() {
    let sc = crowded();
    let r = simulate_steady_with_advice(&sc).unwrap();
    let cut = r
        .recommendations
        .iter()
        .find_map(|x| match x.advice {
            Advice::ReduceLoss { id, max_loss_w } => Some((id, max_loss_w)),
            _ => None,
        })
        .expect("the housing limit calls for a lower loss");
    let mut lower = sc.clone();
    lower.devices[0].loss_w = cut.1 as f64;
    let after = simulate_steady(&lower).unwrap();
    // On the full grid the device is within a few kelvin of its limit.
    assert!(
        after.devices[0].margin_k.unwrap() > -3.0,
        "{:?}",
        after.devices[0].margin_k
    );
    assert!(after.devices[0].surface_c < r.devices[0].surface_c - 10.0);
}

#[test]
fn a_move_works_when_made() {
    let mut sc = crowded();
    // The upper device's problem is the air under it: the neighbour's plume.
    sc.devices[1].rated_max_ambient_c = Some(34.0);
    let r = simulate_steady_with_advice(&sc).unwrap();
    assert_eq!(r.devices[1].status, Status::Over);
    let mv = r.recommendations.iter().find_map(|x| match x.advice {
        Advice::MoveDevice {
            id: 2,
            to_x_mm,
            to_y_mm,
        } => Some((to_x_mm, to_y_mm)),
        _ => None,
    });
    let mv = mv.expect("moving the device away from the plume helps");
    let mut moved = sc.clone();
    moved.devices[1].footprint.x = mv.0 as f64;
    moved.devices[1].footprint.y = mv.1 as f64;
    let after = simulate_steady(&moved).unwrap();
    assert!(after.devices[1].margin_k.unwrap() > r.devices[1].margin_k.unwrap() + 1.0);
}

#[test]
fn the_band_brackets_the_estimate_and_widens_with_heat() {
    let r = simulate_steady(&crowded()).unwrap();
    for d in &r.devices {
        assert!(d.surface_band_c.0 <= d.surface_c && d.surface_c <= d.surface_band_c.1);
        assert!(d.air_band_c.0 <= d.air_c && d.air_c <= d.air_band_c.1);
    }
    let w = |d: &DeviceResult| d.surface_band_c.1 - d.surface_band_c.0;
    assert!(w(&r.devices[0]) > w(&r.devices[2]));
    // The tight one is over at the central estimate; the lightly loaded one is fine.
    assert_eq!(r.devices[2].status, Status::Ok);
}

#[test]
fn a_device_marginal_at_the_pessimistic_end_is_flagged() {
    let mut sc = coarse(scenario(vec![device(1, 300.0, 300.0, 100.0, 100.0, 30.0)]));
    let probe = simulate_steady(&sc).unwrap();
    let d = &probe.devices[0];
    // Limit between the central and the pessimistic air temperature.
    sc.devices[0].rated_max_ambient_c = Some(0.5 * (d.air_c + d.air_band_c.1));
    let r = simulate_steady(&sc).unwrap();
    assert_eq!(r.devices[0].status, Status::Marginal);
    assert!(matches!(r.hot_spots[0].kind, HotSpotKind::Device(1)));
}
