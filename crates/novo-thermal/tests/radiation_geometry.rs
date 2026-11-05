//! Device radiation from computed view factors (lane H2), switched on with
//! `Params::radiation_geometry`. Model against model: these tests check
//! direction and size of the effects, not a measurement.

mod common;
use common::*;
use novo_thermal::*;

fn one(on: bool, neighbours: bool) -> ThermalResult {
    let mut devs = vec![device(1, 300.0, 500.0, 120.0, 150.0, 40.0)];
    if neighbours {
        // Cold boxes on three sides, taller than the hot device.
        for (i, (x, y)) in [(160.0, 500.0), (440.0, 500.0), (300.0, 350.0)]
            .into_iter()
            .enumerate()
        {
            let mut d = device(10 + i as u64, x, y, 120.0, 140.0, 0.0);
            d.height_mm = Some(150.0);
            devs.push(d);
        }
    }
    let mut sc = scenario(devs);
    sc.params.radiation_geometry = on;
    quick(sc)
}

#[test]
fn off_by_default_so_existing_results_do_not_move() {
    assert!(!Params::default().radiation_geometry);
}

#[test]
fn a_lone_device_radiates_more_than_the_fixed_half_view_says() {
    let (off, on) = (one(false, false), one(true, false));
    let (h_off, h_on) = (off.devices[0].h_w_m2k, on.devices[0].h_w_m2k);
    // Seen from a big painted box the device has an almost free view: the
    // radiative film is about twice the 0.85 x 0.5 constant.
    assert!(h_on > h_off + 1.0 && h_on < h_off + 6.0, "{h_off} {h_on}");
    assert!(on.devices[0].surface_c < off.devices[0].surface_c);
}

#[test]
fn neighbours_hide_the_walls() {
    let (alone, crowded) = (one(true, false), one(true, true));
    assert!(
        crowded.devices[0].h_w_m2k < alone.devices[0].h_w_m2k,
        "{} {}",
        crowded.devices[0].h_w_m2k,
        alone.devices[0].h_w_m2k
    );
}

#[test]
fn bright_walls_return_the_heat() {
    let painted = one(true, false);
    let mut sc = scenario(vec![device(1, 300.0, 500.0, 120.0, 150.0, 40.0)]);
    sc.params.radiation_geometry = true;
    let mut shell = sc.shell();
    for f in shell.faces.iter_mut() {
        f.emissivity_in = 0.1;
    }
    sc.params.shell = Some(shell);
    let bright = quick(sc);
    assert!(
        bright.devices[0].h_w_m2k < painted.devices[0].h_w_m2k,
        "{} {}",
        bright.devices[0].h_w_m2k,
        painted.devices[0].h_w_m2k
    );
    assert!(bright.devices[0].surface_c > painted.devices[0].surface_c);
}

#[test]
fn the_band_parameter_still_scales_the_computed_value() {
    let run = |view: f64| {
        let mut sc = scenario(vec![device(1, 300.0, 500.0, 120.0, 150.0, 40.0)]);
        sc.params.radiation_geometry = true;
        sc.params.radiation_view_factor = view;
        quick(sc).devices[0].h_w_m2k
    };
    let (low, mid, high) = (run(0.25), run(0.5), run(0.75));
    assert!(low < mid && mid < high, "{low} {mid} {high}");
}
