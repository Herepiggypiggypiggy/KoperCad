//! Physics checks: each of these fails when the model is physically wrong.

mod common;

use common::*;
use novo_layout::Rect;
use novo_layout::sizing::{FAN_FACTOR, Material};
use novo_thermal::*;

fn spread(total_w: f64) -> Vec<Device> {
    // Twelve identical devices on a regular pattern: close to uniform heating.
    let mut v = Vec::new();
    let per = total_w / 12.0;
    let mut id = 0;
    for row in 0..4 {
        for col in 0..3 {
            id += 1;
            v.push(device(
                id,
                120.0 + col as f64 * 220.0,
                120.0 + row as f64 * 260.0,
                60.0,
                80.0,
                per,
            ));
        }
    }
    v
}

fn close(a: f64, b: f64, rel: f64, what: &str) {
    assert!(
        (a - b).abs() <= rel * b.abs().max(1e-9),
        "{what}: {a} vs {b} (allowed {:.0} percent)",
        rel * 100.0
    );
}

#[test]
fn no_heat_means_ambient_everywhere() {
    let r = simulate_steady(&scenario(vec![device(1, 300.0, 300.0, 100.0, 100.0, 0.0)])).unwrap();
    assert!((r.air_temp.max() - 25.0).abs() < 1e-6);
    assert!((r.air_temp.min() - 25.0).abs() < 1e-6);
    assert!(r.max_speed_ms < 1e-6);
}

#[test]
fn energy_balance_closes_closed_vented_and_with_fans() {
    let mut closed = scenario(spread(120.0));
    let r = simulate_steady(&closed).unwrap();
    assert!(r.converged);
    assert!(r.balance.residual_rel.abs() < 1e-3, "{:?}", r.balance);

    closed.vents = vec![
        Vent {
            opening: Opening {
                placement: Placement::Bottom { x_mm: 400.0 },
                width_mm: 200.0,
                height_mm: 100.0,
            },
            free_area_fraction: 0.6,
        },
        Vent {
            opening: Opening {
                placement: Placement::Top { x_mm: 400.0 },
                width_mm: 200.0,
                height_mm: 100.0,
            },
            free_area_fraction: 0.6,
        },
    ];
    let r = simulate_steady(&closed).unwrap();
    assert!(r.balance.residual_rel.abs() < 1e-3, "{:?}", r.balance);
    assert!(r.balance.openings_w > 1.0);

    closed.fans = vec![Fan {
        opening: Opening {
            placement: Placement::Front {
                x_mm: 200.0,
                y_mm: 1000.0,
            },
            width_mm: 120.0,
            height_mm: 120.0,
        },
        mode: FanMode::Intake,
        free_flow_m3h: 150.0,
        max_pressure_pa: 60.0,
    }];
    let r = simulate_steady(&closed).unwrap();
    assert!(r.balance.residual_rel.abs() < 1e-3, "{:?}", r.balance);
}

#[test]
fn well_mixed_rise_matches_the_iec_60890_estimate() {
    for material in [Material::Steel, Material::Aluminium] {
        // Same constants as the sizing helper: rise = P / (k * Ae). With the
        // devices heating only the air the two agree ...
        let mut sc = scenario(spread(120.0));
        sc.enclosure.material = material;
        sc.params.plate_contact_h = 0.0;
        let r = simulate_steady(&sc).unwrap();
        close(
            r.balance.mean_rise_k,
            r.balance.iec_rise_k,
            0.15,
            "mean rise, air only",
        );
        // ... and with the usual contact into the plate the air is cooler,
        // because the plate gives off heat to the outside directly. IEC 60890
        // itself is good to about 15 percent.
        let mut sc = scenario(spread(120.0));
        sc.enclosure.material = material;
        let r = simulate_steady(&sc).unwrap();
        assert!(r.balance.mean_rise_k < r.balance.iec_rise_k);
        close(
            r.balance.mean_rise_k,
            r.balance.iec_rise_k,
            0.15,
            "mean rise",
        );
    }
}

#[test]
fn doubling_the_losses_doubles_the_rise() {
    let a = simulate_steady(&scenario(spread(60.0))).unwrap();
    let b = simulate_steady(&scenario(spread(120.0))).unwrap();
    close(
        b.balance.mean_rise_k / a.balance.mean_rise_k,
        2.0,
        0.10,
        "rise ratio",
    );
}

#[test]
fn a_mirrored_layout_gives_a_mirrored_field() {
    let w = enclosure().interior_width_mm();
    let d = |id: u64, x: f64| device(id, x, 500.0, 80.0, 120.0, 25.0);
    let sc = scenario(vec![d(1, 150.0), d(2, w - 150.0 - 80.0)]);
    let mut sc = sc;
    // Plate centred, so the whole model is mirror symmetric.
    sc.enclosure.plate = Rect {
        x: 50.0,
        y: 50.0,
        w: w - 100.0,
        h: 1090.0,
    };
    let r = simulate_steady(&sc).unwrap();
    let g = r.grid;
    let mut worst: f64 = 0.0;
    for row in 0..g.rows {
        for col in 0..g.cols / 2 {
            let a = r.air_temp.at(col, row);
            let b = r.air_temp.at(g.cols - 1 - col, row);
            worst = worst.max((a - b).abs());
            let (vx_a, vy_a) = r.velocity.at(col, row);
            let (vx_b, vy_b) = r.velocity.at(g.cols - 1 - col, row);
            assert!((vx_a + vx_b).abs() < 0.01 && (vy_a - vy_b).abs() < 0.01);
        }
    }
    assert!(worst < 0.05, "asymmetry {worst} K");
}

#[test]
fn warm_air_rises_above_a_hot_device() {
    let r = simulate_steady(&scenario(vec![device(1, 350.0, 700.0, 80.0, 120.0, 40.0)])).unwrap();
    // Cell column above the device centre: air moves up (vy < 0 on screen).
    let g = r.grid;
    let col = (390.0 / g.cell_w_mm) as usize;
    let row = (600.0 / g.cell_h_mm) as usize;
    let (_, vy) = r.velocity.at(col, row);
    assert!(vy < -0.01, "vy {vy}");
    // And it is warmer there than at the same height far to the side.
    assert!(r.air_temp.at(col, row) > r.air_temp.at(1, row) + 0.2);
}

#[test]
fn a_device_above_another_runs_hotter_than_beside_it() {
    let lower = device(1, 300.0, 800.0, 80.0, 120.0, 30.0);
    let stacked = simulate_steady(&scenario(vec![
        lower.clone(),
        device(2, 300.0, 600.0, 80.0, 120.0, 30.0),
    ]))
    .unwrap();
    let beside = simulate_steady(&scenario(vec![
        lower,
        device(2, 500.0, 800.0, 80.0, 120.0, 30.0),
    ]))
    .unwrap();
    let t = |r: &ThermalResult| r.devices[1].air_c;
    assert!(
        t(&stacked) > t(&beside) + 0.8,
        "{} vs {}",
        t(&stacked),
        t(&beside)
    );
}

#[test]
fn fan_flow_carries_the_heat_away() {
    // A fan pair sized by the sizing rule 3.1 W/K: V = 3.1 * P / dT.
    let p = 200.0;
    let dt_allowed = 10.0;
    let flow = FAN_FACTOR * p / dt_allowed;
    let mut sc = scenario(spread(p));
    sc.fans = vec![
        Fan {
            opening: Opening {
                placement: Placement::Bottom { x_mm: 200.0 },
                width_mm: 150.0,
                height_mm: 150.0,
            },
            mode: FanMode::Intake,
            free_flow_m3h: flow * 1.2,
            max_pressure_pa: 80.0,
        },
        Fan {
            opening: Opening {
                placement: Placement::Top { x_mm: 600.0 },
                width_mm: 150.0,
                height_mm: 150.0,
            },
            mode: FanMode::Exhaust,
            free_flow_m3h: flow * 1.2,
            max_pressure_pa: 80.0,
        },
    ];
    let r = simulate_steady(&sc).unwrap();
    assert!(r.balance.residual_rel.abs() < 1e-3);
    let q_in: f64 = r
        .openings
        .iter()
        .filter(|o| o.flow_m3h > 0.0)
        .map(|o| o.flow_m3h)
        .sum();
    let q_out: f64 = r
        .openings
        .iter()
        .filter(|o| o.flow_m3h < 0.0)
        .map(|o| -o.flow_m3h)
        .sum();
    close(q_in, q_out, 0.005, "mass in = mass out");
    assert!(
        q_in > 0.6 * flow && q_in <= flow * 1.2 * (1.0 + 1e-6),
        "flow {q_in} m3/h"
    );
    // Sensible heat: the air leaves warmer by P_air / (rho cp Q).
    let rho_cp = 101_325.0 / (287.05 * 298.15) * 1006.0;
    let dt_out = r.balance.openings_w / (rho_cp * q_out / 3600.0);
    assert!(
        dt_out > 0.3 * dt_allowed && dt_out < 1.05 * dt_allowed,
        "outlet rise {dt_out}"
    );
    // Most of the heat goes out by the air, and the interior stays well below
    // the closed-box rise.
    assert!(r.balance.openings_w > 0.5 * p);
    let closed = simulate_steady(&scenario(spread(p))).unwrap();
    assert!(r.balance.mean_rise_k < 0.5 * closed.balance.mean_rise_k);
}

#[test]
fn stack_effect_matches_the_chimney_formula() {
    let mut sc = scenario(spread(150.0));
    sc.params.clutter_drag_per_m = 0.1;
    let vent = |placement| Vent {
        opening: Opening {
            placement,
            width_mm: 250.0,
            height_mm: 100.0,
        },
        free_area_fraction: 1.0,
    };
    sc.vents = vec![
        vent(Placement::Bottom { x_mm: 400.0 }),
        vent(Placement::Top { x_mm: 400.0 }),
    ];
    let r = simulate_steady(&sc).unwrap();
    let a = 0.25 * 0.10; // geometric area of each opening, m2
    let a_star = a / 2f64.sqrt();
    let cd = sc.params.discharge_coefficient;
    let height = 1.196; // vent to vent, m: interior height
    let t_in = r.balance.mean_air_c + 273.15;
    let dt = r.balance.mean_air_c - 25.0;
    let q = cd * a_star * (2.0 * 9.81 * height * dt / t_in).sqrt() * 3600.0;
    let sim = r.openings.iter().map(|o| o.flow_m3h.abs()).sum::<f64>() / 2.0;
    let ratio = sim / q;
    assert!(
        (0.7..=1.3).contains(&ratio),
        "simulated {sim} m3/h, formula {q} m3/h"
    );
    // The inlet is the bottom vent: flow comes in low and goes out high.
    assert!(r.openings[0].flow_m3h > 0.0 && r.openings[1].flow_m3h < 0.0);
}

#[test]
fn heat_up_follows_the_lumped_time_constant() {
    let sc = scenario(spread(120.0));
    let t = simulate_transient(
        &sc,
        &TransientSettings {
            duration_s: 3.0 * 3600.0 * 1.5,
            steps: 120,
            ..TransientSettings::default()
        },
    )
    .unwrap();
    let tau = t.time_constant_s.expect("settles within the run");
    // C = air + walls + plate + devices, tau = C / (k Ae).
    let e = &sc.enclosure;
    let (w, h, d) = (
        e.width_mm / 1000.0,
        e.height_mm / 1000.0,
        e.depth_mm / 1000.0,
    );
    let air =
        1.19 * 1006.0 * e.interior_width_mm() * e.interior_height_mm() * e.interior_depth_mm()
            / 1e9;
    let walls = 7850.0 * 490.0 * 0.002 * 2.0 * (w * h + w * d + h * d);
    let plate = 7850.0 * 490.0 * 0.0025 * e.plate.w * e.plate.h / 1e6;
    let devices: f64 = sc
        .devices
        .iter()
        .map(|d| 500.0 * d.footprint.area() / 1e6 * 100.0)
        .sum();
    let c = air + walls + plate + devices;
    let k_ae = 5.5
        * novo_layout::sizing::effective_surface(
            e.width_mm,
            e.height_mm,
            e.depth_mm,
            e.installation,
        );
    // The air has to carry the heat to the wall mass, so the heat-up is a
    // little slower than the well mixed lumped estimate, never faster.
    let lumped = c / k_ae;
    assert!(
        tau > 0.9 * lumped && tau < 1.6 * lumped,
        "tau {tau} s, lumped {lumped} s"
    );
    // The final state is the steady state.
    let steady = simulate_steady(&sc).unwrap();
    close(
        t.last.balance.mean_air_c - 25.0,
        steady.balance.mean_rise_k,
        0.05,
        "final rise",
    );
    // The storage term of the last step is small once settled.
    assert!(t.last.balance.residual_rel.abs() < 0.02);
}

#[test]
fn transient_energy_balance_closes_while_heating() {
    let sc = scenario(spread(120.0));
    let t = simulate_transient(
        &sc,
        &TransientSettings {
            duration_s: 600.0,
            steps: 30,
            ..TransientSettings::default()
        },
    )
    .unwrap();
    // Early in the heat-up most of the heat is stored, not lost.
    assert!(t.last.balance.storage_w > 0.5 * t.last.balance.loss_w);
    assert!(
        t.last.balance.residual_rel.abs() < 1e-3,
        "{:?}",
        t.last.balance
    );
}

#[test]
fn plate_temperature_decays_with_the_fin_length() {
    // A strip of heat across the full plate width near the top, strong
    // ventilation holding the air near ambient: along the plate the excess
    // temperature falls as exp(-y / L), L = sqrt(k t / U).
    let mut sc = scenario(vec![device(1, 60.0, 60.0, 660.0, 20.0, 30.0)]);
    sc.devices[0].height_mm = Some(10.0);
    sc.params.plate_contact_h = 2000.0; // the strip heats the plate directly
    sc.params.h_in = 60.0; // plate loses mostly to the (cooled) air
    sc.fans = vec![
        Fan {
            opening: Opening {
                placement: Placement::Bottom { x_mm: 400.0 },
                width_mm: 600.0,
                height_mm: 250.0,
            },
            mode: FanMode::Intake,
            free_flow_m3h: 6000.0,
            max_pressure_pa: 200.0,
        },
        Fan {
            opening: Opening {
                placement: Placement::Top { x_mm: 400.0 },
                width_mm: 600.0,
                height_mm: 250.0,
            },
            mode: FanMode::Exhaust,
            free_flow_m3h: 6000.0,
            max_pressure_pa: 200.0,
        },
    ];
    let r = simulate_steady(&sc).unwrap();
    let g = r.grid;
    let col = g.cols / 2;
    let excess = |y_mm: f64| {
        let row = (y_mm / g.cell_h_mm) as usize;
        r.plate_temp.at(col, row) - 25.0
    };
    let (y1, y2) = (200.0, 300.0);
    let (e1, e2) = (excess(y1), excess(y2));
    assert!(e1 > e2 && e2 > 0.0);
    let measured_l = (y2 - y1) / 1000.0 / (e1 / e2).ln();
    // U: plate to air at 60 plus plate to ambient through the rear wall.
    let kb = 5.5 * 0.5; // rear covered by the wall mount
    let h_air = 60.0f64.max(1.2 * kb);
    let u_pa = 1.0 / (1.0 / kb - 1.0 / h_air);
    let l = (50.0 * 0.0025 / (h_air + u_pa)).sqrt();
    close(measured_l, l, 0.25, "fin length");
}

#[test]
fn hotter_ambient_shifts_everything_equally() {
    let a = simulate_steady(&scenario(spread(100.0))).unwrap();
    let mut sc = scenario(spread(100.0));
    sc.ambient_c = 40.0;
    let b = simulate_steady(&sc).unwrap();
    close(
        b.balance.mean_rise_k,
        a.balance.mean_rise_k,
        0.03,
        "rise independent of ambient",
    );
}

#[test]
fn the_estimate_carries_a_band_and_a_note() {
    let r = simulate_steady(&scenario(vec![device(1, 300.0, 300.0, 100.0, 100.0, 30.0)])).unwrap();
    let d = &r.devices[0];
    assert!(d.surface_band_c.0 < d.surface_c && d.surface_c < d.surface_band_c.1);
    assert!(r.notes.iter().any(|n| n.contains("Estimate")));
    assert!(!r.assumptions.is_empty());
}

#[test]
fn bad_input_is_refused() {
    let mut sc = scenario(vec![device(1, 300.0, 300.0, 100.0, 100.0, -5.0)]);
    assert!(simulate_steady(&sc).is_err());
    sc.devices[0].loss_w = 5.0;
    sc.enclosure.width_mm = 10.0;
    assert!(simulate_steady(&sc).is_err());
}
