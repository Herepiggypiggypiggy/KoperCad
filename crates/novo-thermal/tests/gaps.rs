//! Leaks and filters against closed forms and against the solver.
mod common;
use common::*;
use novo_thermal::gaps::*;
use novo_thermal::*;

fn close(a: f64, b: f64, rel: f64, what: &str) {
    assert!(
        (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-12),
        "{what}: {a} vs {b}"
    );
}

#[test]
fn leak_flow_follows_the_two_thirds_power_law() {
    close(leak_flow(10.0, 100.0), 10.0, 1e-12, "q100 at 100 Pa");
    // 8x the pressure -> 4x the flow with exponent 2/3.
    close(
        leak_flow(10.0, 800.0) / leak_flow(10.0, 100.0),
        4.0,
        1e-9,
        "8x dp",
    );
    close(leak_flow(10.0, -50.0), leak_flow(10.0, 50.0), 1e-12, "sign");
}

#[test]
fn effective_area_passes_the_same_flow_at_4_pa() {
    let q100 = 36.0;
    let a = effective_leakage_area_m2(q100);
    // Orifice law with Cd 1 at 4 Pa gives back the power-law flow.
    let q = a * (2.0 * ELA_REF_PA / RHO_AIR).sqrt() * 3600.0;
    close(q, leak_flow(q100, ELA_REF_PA), 1e-9, "ELA round trip");
}

#[test]
fn a_tighter_class_leaks_less_and_glands_add() {
    let e = enclosure();
    let mut last = f64::INFINITY;
    for ip in IpClass::ALL {
        let t = total_q100_m3h(&e, &Seal { ip, glands: 0 });
        assert!(t < last, "{ip:?} should be tighter than the one before");
        last = t;
    }
    let s0 = Seal {
        ip: IpClass::Ip54,
        glands: 0,
    };
    let s6 = Seal {
        ip: IpClass::Ip54,
        glands: 6,
    };
    close(
        total_q100_m3h(&e, &s6) - total_q100_m3h(&e, &s0),
        6.0 * IpClass::Ip54.gland_q100_m3h(),
        1e-12,
        "six glands",
    );
    // 800 x 1200 door, 2 mm walls: seal length is the door perimeter.
    close(
        door_seal_length_m(&e),
        2.0 * (0.796 + 1.196),
        1e-12,
        "seal length",
    );
}

#[test]
fn leak_vents_carry_the_effective_area() {
    let e = enclosure();
    let p = Params::default();
    let seal = Seal {
        ip: IpClass::Ip43,
        glands: 4,
    };
    let vents = leak_vents(&e, &seal, &p);
    assert_eq!(vents.len(), 4);
    let ela: f64 = vents
        .iter()
        .map(|v| p.discharge_coefficient * v.free_area_fraction * v.opening.area_m2())
        .sum();
    let want = effective_leakage_area_m2(seal.ip.door_q100_m3h_per_m() * door_seal_length_m(&e))
        + effective_leakage_area_m2(4.0 * seal.ip.gland_q100_m3h());
    // The glands and the bottom edge are one vent: areas add linearly only
    // for the same exponent, so the total is within the sum of parts.
    close(ela, want, 0.02, "total leakage area");
}

#[test]
fn filter_drop_matches_its_table_and_rises_with_dust() {
    for c in FilterClass::ALL {
        let f = Filter::from_class(c);
        close(f.dp_pa(REF_SPEED_MS), f.dp_clean_pa, 1e-12, "clean at ref");
        close(
            f.with_loading(1.0).dp_pa(REF_SPEED_MS),
            f.dp_final_pa,
            1e-12,
            "final at ref",
        );
        assert!(f.dp_pa(2.0) > f.dp_pa(1.0), "faster, more drop");
        close(
            f.with_loading(0.5).dp_ref_pa(),
            0.5 * (f.dp_clean_pa + f.dp_final_pa),
            1e-12,
            "half load",
        );
    }
    // Finer mats cost more.
    let g3 = Filter::from_class(FilterClass::G3).dp_clean_pa;
    let f7 = Filter::from_class(FilterClass::F7).dp_clean_pa;
    assert!(f7 > g3);
}

#[test]
fn dust_load_is_mass_over_capacity() {
    let f = Filter::from_class(FilterClass::G4);
    // 200 m3/h, 0.2 mg/m3, 100 h: 4 g in air, 90 % caught = 3.6 g on 0.04 m2.
    let l = f.loading_after(100.0, DUST_WORKSHOP, 200.0, 0.04);
    close(l, 3.6 / (0.04 * 350.0), 1e-9, "loading");
    let h = f.hours_to_change(DUST_WORKSHOP, 200.0, 0.04).unwrap();
    close(
        f.loading_after(h, DUST_WORKSHOP, 200.0, 0.04),
        1.0,
        1e-9,
        "due at 1",
    );
    // Ten times the dust, a tenth of the time.
    let h10 = f
        .hours_to_change(10.0 * DUST_WORKSHOP, 200.0, 0.04)
        .unwrap();
    close(h10, h / 10.0, 1e-9, "time scales with dust");
}

#[test]
fn grille_and_mat_in_series_add_their_drops() {
    let area = 0.03;
    let a_eff = 0.6 * 0.6 * area;
    let f = Filter::from_class(FilterClass::G4).with_loading(0.4);
    for dp in [1.0, 5.0, 30.0, 120.0] {
        let q = series_flow_m3s(dp, a_eff, area, Some(&f));
        let orifice = 0.5 * RHO_AIR * (q / a_eff).powi(2);
        let mat = f.dp_pa(q / area);
        close(orifice + mat, dp, 1e-9, "drops add up to the total");
    }
    // No filter: the plain orifice law.
    let q = series_flow_m3s(10.0, a_eff, area, None);
    close(q, a_eff * (2.0 * 10.0 / RHO_AIR).sqrt(), 1e-12, "orifice");
}

#[test]
fn clogging_cuts_the_flow_monotonically() {
    let area = 0.03;
    let a_eff = 0.36 * area;
    let mut last = f64::INFINITY;
    for l in [0.0, 0.25, 0.5, 0.75, 1.0, 1.25] {
        let f = Filter::from_class(FilterClass::F7).with_loading(l);
        let q = series_flow_m3s(4.0, a_eff, area, Some(&f));
        assert!(q < last);
        last = q;
    }
}

#[test]
fn the_equivalent_free_fraction_passes_the_same_flow() {
    let p = Params::default();
    let o = Opening {
        placement: Placement::Bottom { x_mm: 300.0 },
        width_mm: 200.0,
        height_mm: 150.0,
    };
    let f = Filter::from_class(FilterClass::G4).with_loading(0.6);
    let fa = 0.6;
    let eq = equivalent_free_fraction(fa, &o, &f, 8.0, &p);
    assert!(eq < fa);
    let a_eff = p.discharge_coefficient * fa * o.area_m2();
    let q_series = series_flow_m3s(8.0, a_eff, o.area_m2(), Some(&f));
    let q_eq = p.discharge_coefficient * eq * o.area_m2() * (2.0 * 8.0 / RHO_AIR).sqrt();
    close(q_series, q_eq, 1e-9, "same flow at the design drop");
}

#[test]
fn a_fan_behind_a_mat_runs_where_its_line_meets_the_mat_curve() {
    let (q_free, p_max, area) = (120.0, 60.0, 0.0225);
    let clean = Filter::from_class(FilterClass::G4);
    let q = fan_free_flow_with_filter(q_free, p_max, area, &clean);
    assert!(q < q_free && q > 0.3 * q_free);
    // On the fan line and on the mat curve at once.
    let qs = q / 3600.0;
    let fan_p = p_max * (1.0 - q / q_free);
    close(fan_p, clean.dp_pa(qs / area), 1e-9, "operating point");
    // A clogged mat takes more of the flow.
    let q_dirty = fan_free_flow_with_filter(q_free, p_max, area, &clean.with_loading(1.0));
    assert!(q_dirty < q);
}

fn hot_box(vents: Vec<Vent>) -> Scenario {
    let mut sc = scenario(vec![device(1, 150.0, 400.0, 200.0, 200.0, 150.0)]);
    sc.vents = vents;
    sc
}

#[test]
fn leaks_cool_a_little_and_a_tight_seal_changes_almost_nothing() {
    let e = enclosure();
    let p = Params::default();
    let sealed = quick(hot_box(vec![]));
    let leaky = quick(hot_box(leak_vents(
        &e,
        &Seal {
            ip: IpClass::Ip20,
            glands: 4,
        },
        &p,
    )));
    let tight = quick(hot_box(leak_vents(
        &e,
        &Seal {
            ip: IpClass::Ip66,
            glands: 4,
        },
        &p,
    )));
    let d = |r: &ThermalResult| r.devices[0].surface_c;
    assert!(d(&leaky) <= d(&sealed) + 1e-6, "a leak never heats");
    assert!(d(&sealed) - d(&leaky) < 8.0, "a leak is not a grille");
    assert!(
        (d(&sealed) - d(&tight)).abs() < 0.25 * (d(&sealed) - d(&leaky)).max(0.2),
        "IP66 stays close to sealed"
    );
    assert!(
        leaky.balance.residual_rel.abs() < 1e-3,
        "energy closes with leaks"
    );
}

#[test]
fn leaks_follow_the_stack_effect_in_low_and_out_high() {
    let e = enclosure();
    let p = Params::default();
    let sc = hot_box(leak_vents(
        &e,
        &Seal {
            ip: IpClass::Ip20,
            glands: 6,
        },
        &p,
    ));
    let r = quick(sc);
    // Vents are listed top, bottom, left, right.
    let flow = |i: usize| {
        r.openings
            .iter()
            .find(|o| !o.is_fan && o.index == i)
            .unwrap()
            .flow_m3h
    };
    assert!(flow(1) > 0.0, "bottom leak lets air in");
    assert!(flow(0) < 0.0, "top leak lets air out");
}

#[test]
fn a_clogged_filter_on_the_only_vents_heats_the_device() {
    let mk = |fa_in: f64, fa_out: f64| {
        let o = |pl| Opening {
            placement: pl,
            width_mm: 200.0,
            height_mm: 150.0,
        };
        vec![
            Vent {
                opening: o(Placement::Bottom { x_mm: 400.0 }),
                free_area_fraction: fa_in,
            },
            Vent {
                opening: o(Placement::Top { x_mm: 400.0 }),
                free_area_fraction: fa_out,
            },
        ]
    };
    let p = Params::default();
    let o = Opening {
        placement: Placement::Top { x_mm: 0.0 },
        width_mm: 200.0,
        height_mm: 150.0,
    };
    let eq = |l: f64| {
        equivalent_free_fraction(
            0.6,
            &o,
            &Filter::from_class(FilterClass::F7).with_loading(l),
            4.0,
            &p,
        )
    };
    let new = quick(hot_box(mk(eq(0.0), eq(0.0))));
    let old = quick(hot_box(mk(eq(1.0), eq(1.0))));
    let open = quick(hot_box(mk(0.6, 0.6)));
    let t = |r: &ThermalResult| r.devices[0].surface_c;
    assert!(t(&open) <= t(&new) + 1e-6);
    assert!(t(&new) < t(&old), "{} vs {}", t(&new), t(&old));
}
