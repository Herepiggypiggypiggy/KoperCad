//! Internal circulation fans and the film upgrades of lane H8.
//!
//! What is exact and what is model: continuity and the fan curve are exact
//! properties of the solved field; the size of the film gain a fan brings is
//! a model (2D depth-averaged flow, flat-plate correlation).

mod common;
use common::*;
use novo_thermal::forced::Heatsink;
use novo_thermal::*;

fn fan(x: f64, y: f64, dir: BlowDirection, q_m3h: f64, p_pa: f64) -> CirculationFan {
    CirculationFan {
        x_mm: x,
        y_mm: y,
        diameter_mm: 120.0,
        direction: dir,
        free_flow_m3h: q_m3h,
        max_pressure_pa: p_pa,
        installed_factor: 1.0,
        z_mm: None,
    }
}

fn with_fan(q: f64, dir: BlowDirection) -> Scenario {
    let mut sc = scenario(vec![device(1, 400.0, 500.0, 120.0, 100.0, 60.0)]);
    sc.params.circulation_fans = vec![fan(200.0, 550.0, dir, q, 40.0)];
    sc
}

#[test]
fn the_fan_sits_on_its_curve_and_air_goes_round_in_a_closed_box() {
    let r = quick(with_fan(200.0, BlowDirection::Right));
    let c = &r.circulation[0];
    let q_max = 200.0;
    // flow = q_max (1 - dp / p_max)
    let on_curve = q_max * (1.0 - c.pressure_pa / 40.0);
    assert!(
        (c.flow_m3h - on_curve).abs() < 0.02 * q_max,
        "{} vs {}",
        c.flow_m3h,
        on_curve
    );
    assert!(c.flow_m3h > 0.0 && c.flow_m3h < q_max);
    assert!(c.share_of_free_flow > 0.0 && c.share_of_free_flow < 1.0);
    // No opening: nothing leaves or enters the enclosure.
    assert!(r.openings.is_empty());
}

#[test]
fn mass_is_conserved_across_a_cut_through_the_fan() {
    let r = quick(with_fan(200.0, BlowDirection::Right));
    let g = r.grid;
    let col = (200.0 / g.cell_w_mm) as usize;
    let (mut net, mut gross) = (0.0, 0.0);
    for row in 0..g.rows {
        let (vx, _) = r.velocity.at(col, row);
        net += vx;
        gross += vx.abs();
    }
    assert!(gross > 0.0);
    // A closed box: what goes through the cut one way comes back the other.
    assert!(net.abs() < 0.02 * gross, "net {net} gross {gross}");
}

#[test]
fn reversing_the_fan_reverses_the_air_at_the_fan_and_keeps_the_flow() {
    let a = quick(with_fan(200.0, BlowDirection::Right));
    let b = quick(with_fan(200.0, BlowDirection::Left));
    let g = a.grid;
    let (col, row) = (
        (200.0 / g.cell_w_mm) as usize,
        (550.0 / g.cell_h_mm) as usize,
    );
    let (va, _) = a.velocity.at(col, row);
    let (vb, _) = b.velocity.at(col, row);
    assert!(va > 0.05 && vb < -0.05, "{va} {vb}");
    assert!((a.circulation[0].flow_m3h - b.circulation[0].flow_m3h).abs() < 5.0);
}

#[test]
fn a_weaker_installation_delivers_less_air() {
    let strong = quick(with_fan(200.0, BlowDirection::Right));
    let mut sc = with_fan(200.0, BlowDirection::Right);
    sc.params.circulation_fans[0].installed_factor = 0.5;
    let weak = quick(sc);
    assert!(weak.circulation[0].flow_m3h < 0.7 * strong.circulation[0].flow_m3h);
}

#[test]
fn a_circulation_fan_cools_the_device_in_its_stream() {
    let off = quick(scenario(vec![device(1, 400.0, 500.0, 120.0, 100.0, 60.0)]));
    let on = quick(with_fan(250.0, BlowDirection::Right));
    let (d0, d1) = (&off.devices[0], &on.devices[0]);
    assert!(
        d1.h_w_m2k > 1.5 * d0.h_w_m2k,
        "{} vs {}",
        d1.h_w_m2k,
        d0.h_w_m2k
    );
    assert!(
        d1.surface_c < d0.surface_c - 2.0,
        "{} vs {}",
        d1.surface_c,
        d0.surface_c
    );
    // Heat out of the box is the same (nothing leaves but through the walls),
    // so the mean air is not cooled by stirring alone.
    assert!((on.balance.mean_air_c - off.balance.mean_air_c).abs() < 6.0);
}

#[test]
fn a_fan_across_the_depth_only_stirs() {
    let r = quick(with_fan(250.0, BlowDirection::ToDoor));
    assert_eq!(r.circulation[0].flow_m3h, 0.0);
    let off = quick(scenario(vec![device(1, 400.0, 500.0, 120.0, 100.0, 60.0)]));
    assert!(r.devices[0].h_w_m2k > off.devices[0].h_w_m2k);
}

#[test]
fn without_internal_fans_nothing_changes() {
    let a = quick(scenario(vec![device(1, 400.0, 500.0, 120.0, 100.0, 60.0)]));
    assert!(a.circulation.is_empty());
}

#[test]
fn a_part_upstream_raises_the_film_when_wake_turbulence_is_on() {
    let blocker = Obstacle {
        kind: ObstacleKind::Solid,
        rect: novo_layout::Rect {
            x: 300.0,
            y: 480.0,
            w: 40.0,
            h: 140.0,
        },
        blockage: None,
        loss_w: 0.0,
    };
    let mut sc = with_fan(250.0, BlowDirection::Right);
    sc.obstacles = vec![blocker];
    let plain = quick(sc.clone());
    sc.params.wake_turbulence = true;
    let wake = quick(sc);
    assert!(
        wake.devices[0].h_w_m2k > plain.devices[0].h_w_m2k,
        "{} vs {}",
        wake.devices[0].h_w_m2k,
        plain.devices[0].h_w_m2k
    );
    assert!(wake.devices[0].surface_c < plain.devices[0].surface_c);
}

#[test]
fn a_heatsink_lowers_the_surface_temperature() {
    let mut sc = scenario(vec![device(1, 400.0, 500.0, 120.0, 100.0, 60.0)]);
    let bare = quick(sc.clone());
    sc.params.heatsinks = vec![(
        1,
        Heatsink {
            fins: 12,
            fin_height_mm: 30.0,
            fin_thickness_mm: 2.0,
            gap_mm: 6.0,
            length_mm: 100.0,
            k_w_mk: 200.0,
        },
    )];
    let sunk = quick(sc);
    assert!(sunk.devices[0].surface_c < bare.devices[0].surface_c - 3.0);
}

#[test]
fn a_bad_circulation_fan_is_refused() {
    let mut sc = with_fan(200.0, BlowDirection::Right);
    sc.params.circulation_fans[0].max_pressure_pa = 0.0;
    assert!(simulate_steady(&sc).is_err());
    let mut sc = with_fan(200.0, BlowDirection::Right);
    sc.params.circulation_fans[0].x_mm = 5000.0;
    assert!(simulate_steady(&sc).is_err());
}

#[test]
fn a_fan_blowing_at_the_plate_cools_the_device_under_it_more_than_one_beside_it() {
    let dev = |x: f64| device(1, x, 500.0, 120.0, 100.0, 60.0);
    let mut under = scenario(vec![dev(400.0)]);
    let mut beside = scenario(vec![dev(400.0)]);
    for (sc, fx) in [(&mut under, 460.0), (&mut beside, 160.0)] {
        let mut f = fan(fx, 550.0, BlowDirection::ToPlate, 150.0, 40.0);
        f.z_mm = Some(220.0);
        sc.params.circulation_fans = vec![f];
    }
    let none = quick(scenario(vec![dev(400.0)]));
    let (u, b) = (quick(under), quick(beside));
    assert!(u.devices[0].h_w_m2k > b.devices[0].h_w_m2k);
    assert!(u.devices[0].h_w_m2k > 1.5 * none.devices[0].h_w_m2k);
}

#[test]
fn the_in_plane_jet_weakens_with_distance_from_the_fan() {
    let h_at = |x: f64| {
        let mut sc = scenario(vec![device(1, x, 500.0, 100.0, 100.0, 60.0)]);
        // The jet axis skims just above the device fronts, so what changes with distance is
        // the decay, not the spreading toward the plate.
        let mut f = fan(100.0, 550.0, BlowDirection::Right, 250.0, 40.0);
        f.z_mm = Some(70.0);
        sc.params.circulation_fans = vec![f];
        quick(sc).devices[0].h_w_m2k
    };
    let (near, mid, far) = (h_at(260.0), h_at(500.0), h_at(680.0));
    assert!(near > mid && mid > far, "{near} {mid} {far}");
}
