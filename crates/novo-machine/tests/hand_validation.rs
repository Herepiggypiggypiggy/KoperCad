//! Independent hand-calculation validation of the machine model and the co-simulation loop
//! (lane L7, cases 11, 12 and 15-21 of `docs/design/drive-crane-validation.md`).
//!
//! Same rules as `crates/novo-drive/tests/hand_validation.rs`: the numbers are worked out
//! here from the physics and geometry, never by calling the engine under test; a test without an
//! ignore marker runs against the merged engine; an `#[ignore = "Lx: ..."]` test waits for that
//! lane and its body names the API the lane has to offer; `oracle_*` tests check the hand
//! arithmetic itself. Values are generic (no customer data).

use novo_drive::GRAVITY;
use novo_machine::crane::{CraneAxes, pendulum_period_s};
use std::f64::consts::PI;

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{what}: got {a}, hand value {b}, tolerance {tol}"
    );
}

// ---------------------------------------------------------------- case 11: pendulum

#[test]
fn case11_pendulum_small_angle_period() {
    // T = 2 pi sqrt(l / g). l = 1 m: 2.00641 s; l = 4 m: 4.01282 s; l = g: exactly 2 pi.
    close(pendulum_period_s(1.0).unwrap(), 2.006_409, 1e-6, "1 m");
    close(pendulum_period_s(4.0).unwrap(), 4.012_819, 1e-6, "4 m");
    close(
        pendulum_period_s(GRAVITY).unwrap(),
        2.0 * PI,
        1e-12,
        "l = g",
    );
    // Period scales with the square root of length: four times the length, twice the period.
    close(
        pendulum_period_s(16.0).unwrap() / pendulum_period_s(4.0).unwrap(),
        2.0,
        1e-12,
        "sqrt scaling",
    );
}

#[test]
fn oracle_case11_large_amplitude_correction() {
    // Series T = T0 (1 + th^2/16 + 11 th^4/3072). 30 degrees (0.5236 rad), l = 4 m: 4.0827 s,
    // 1.74 % above the small-angle value.
    let th: f64 = 30f64.to_radians();
    let t = pendulum_period_s(4.0).unwrap() * (1.0 + th * th / 16.0 + 11.0 * th.powi(4) / 3072.0);
    close(t, 4.082_657, 1e-6, "30 degree period");
}

#[test]
#[ignore = "L4b: nonlinear pendulum option (swing amplitude) not in the engine yet"]
fn case11_nonlinear_pendulum_period() {
    todo!("L4b: 30 degree amplitude, l = 4 m, period 4.0827 s within 0.1 % (simulate 10 swings)");
}

// ---------------------------------------------------------------- case 12: end to end

#[test]
fn oracle_case12_stop_height_tolerance() {
    // Upper limit entered at 6.000 m, coarse speed 0.25 m/s, step 5 ms: the hook moves 1.25 mm
    // per sample, so the first sample at or past the limit is within [6.000, 6.00125] m.
    let (v, dt) = (0.25_f64, 0.005_f64);
    close(v * dt, 0.001_25, 1e-12, "distance per sample");
}

#[test]
#[ignore = "L5a: co-simulation loop with controller signals, schematic settle and hoist axis"]
fn case12_raise_button_stops_at_the_entered_limit() {
    // Raise button held, upper-limit NC contact in the enable chain, limit at 6.000 m, dt 5 ms:
    // the cut happens in the first sample at or past 6.000 m (so the cut position is within
    // 1.25 mm at 0.25 m/s); plus the hand-calculated brake overshoot of case 14.
    // Second run with the NC contact removed from the drawing: the hook passes 6.000 m and the
    // check "upper limit stops the hoist" must fail.
    todo!("L5a: run the fixture crane with and without the limit contact and compare");
}

// ---------------------------------------------------------------- case 15: coarse-only limit

#[test]
fn oracle_case15_coarse_only_timing() {
    // Hook starts at 0, coarse 0.25 m/s, fine 0.05 m/s (ideal, no ramps). The coarse-chain end
    // switch is at 5.800 m: coarse drops out, fine continues. The final limit at 6.000 m stops
    // fine. Time = 5.8 / 0.25 + 0.2 / 0.05 = 23.2 + 4.0 = 27.2 s. A plain stop at 5.8 m would
    // be wrong: the rule is "end switches stop coarse only".
    close(5.8 / 0.25 + (6.0 - 5.8) / 0.05, 27.2, 1e-9, "ideal time");
}

#[test]
#[ignore = "L2a + L5a: coarse-only end switch rule with two-speed contactors"]
fn case15_end_switch_stops_coarse_only() {
    todo!(
        "L5a: speed drops from coarse to fine at 5.800 m, hoist continues to the final limit at 6.000 m, ideal time 27.2 s, then stops; overshoot per case 14"
    );
}

// ---------------------------------------------------------------- case 16: two cranes, one runway

#[test]
fn oracle_case16_photocell_zone() {
    // Crane A at 10 m moves towards crane B parked at 30 m. Photocell zone 3 m: the coarse
    // drive must be cut when the gap is 3 m, so A's reference point is at 30 - 3 = 27 m. It may
    // then creep at fine speed up to the entered buffer gap 0.5 m, i.e. 29.5 m, never closer.
    // Times (ideal): coarse 17 m / 0.5 = 34 s; fine 2.5 m / 0.1 = 25 s; total 59 s.
    let (a0, b, zone, buffer) = (10.0_f64, 30.0_f64, 3.0_f64, 0.5_f64);
    let coarse_stop = b - zone;
    let fine_stop = b - buffer;
    close(coarse_stop, 27.0, 1e-12, "coarse stop position");
    close(fine_stop, 29.5, 1e-12, "fine stop position");
    close((coarse_stop - a0) / 0.5, 34.0, 1e-12, "coarse time");
    close((fine_stop - coarse_stop) / 0.1, 25.0, 1e-12, "fine time");
}

#[test]
#[ignore = "L4a (geometry rule) and L5b (end to end through the photocell circuit)"]
fn case16_two_cranes_on_one_runway() {
    todo!(
        "L4a: zone rule cuts coarse at 27.0 m, fine allowed to 29.5 m, request beyond refused; L5b: same through the photocell circuit, total 59 s ideal"
    );
}

// ---------------------------------------------------------------- case 17: two crabs, one girder

#[test]
fn oracle_case17_crab_separation() {
    // Crab 1 at 4.0 m, crab 2 at 9.0 m, minimum separation 1.5 m: crab 1 may reach 7.5 m.
    // A request for 7.6 m is refused; the reported separation at the refusal is exactly 1.5 m.
    let (c2, min_sep) = (9.0_f64, 1.5_f64);
    close(c2 - min_sep, 7.5, 1e-12, "limit position");
}

#[test]
#[ignore = "L4a: interference rules engine (crab versus crab)"]
fn case17_crab_refuses_move_at_min_separation() {
    todo!(
        "L4a: move to 7.5 m accepted; move to 7.6 m refused with a finding naming both crabs and separation 1.5 m"
    );
}

// ---------------------------------------------------------------- case 18: two hoists, one crab

#[test]
fn oracle_case18_independent_hoists() {
    // Hoist A runs 5 s at 0.2 m/s: 1.000 m. Hoist B does not move. With the contactor groups
    // (bindings) swapped, B moves 1.000 m and A stays.
    close(0.2 * 5.0, 1.0, 1e-12, "travel");
}

#[test]
#[ignore = "L5a: per-instance bindings and the fixed-step loop"]
fn case18_hoist_motion_follows_its_own_bindings() {
    todo!("L5a: A 1.000 m, B 0.000 m; swap the bindings table and the result swaps");
}

// ---------------------------------------------------------------- case 19: tandem

#[test]
fn oracle_case19_tandem_sharing_and_tilt() {
    // 10 t on a spreader beam of 4 m, centre of load 1/3 along the beam from hoist 1:
    // hoist 1 carries 2/3 = 6.667 t, hoist 2 carries 1/3 = 3.333 t (moment balance).
    let (m, frac) = (10_000.0_f64, 1.0_f64 / 3.0);
    close(m * (1.0 - frac), 6_666.667, 1e-3, "hoist 1 share");
    close(m * frac, 3_333.333, 1e-3, "hoist 2 share");
    // Hook height difference 0.12 m over the 4 m beam: tilt = asin(0.12 / 4) = 0.0300045 rad
    // = 1.7191 degrees.
    let tilt = (0.12_f64 / 4.0).asin();
    close(tilt, 0.030_004_5, 1e-7, "tilt rad");
    close(tilt.to_degrees(), 1.719_13, 1e-5, "tilt degrees");
}

#[test]
#[ignore = "L4a: tandem module (spreader beam, load sharing, tilt)"]
fn case19_tandem_load_sharing() {
    todo!(
        "L4a: shares 6666.67 kg and 3333.33 kg; dh 0.12 m over 4 m gives 1.7191 degrees; sharing beyond the entered limit is a finding"
    );
}

// ---------------------------------------------------------------- case 20: composition

#[test]
fn oracle_case20_world_position_by_hand_addition() {
    // Bridge on the runway at x = 12.5 m (rail height z = 9.0 m); crab at y = 3.2 m along the
    // girder; hook bay offset on the crab (0.4, -0.3, -0.6) m; hook 2.5 m below the drum.
    // World = (12.5 + 0.4, 3.2 - 0.3, 9.0 - 0.6 - 2.5) = (12.9, 2.9, 5.9).
    let world = (12.5 + 0.4, 3.2 - 0.3, 9.0 - 0.6 - 2.5);
    close(world.0, 12.9, 1e-12, "x");
    close(world.1, 2.9, 1e-12, "y");
    close(world.2, 5.9, 1e-12, "z");
}

#[test]
fn case20_axes_default_to_zero() {
    // The axes record starts at rest at the origin: a cheap guard for the data type the
    // composition rule will extend.
    assert_eq!(
        CraneAxes::default(),
        CraneAxes {
            bridge_mm: 0.0,
            trolley_mm: 0.0,
            hoist_mm: 0.0
        }
    );
}

#[test]
#[ignore = "L4a: module tree with mount points and transform composition"]
fn case20_transforms_compose() {
    todo!(
        "L4a: world position (12.9, 2.9, 5.9) m; property test: adding or removing a module leaves the world position of every unrelated module unchanged (1000 random trees)"
    );
}

// ---------------------------------------------------------------- case 21: scale

#[test]
#[ignore = "L5a: fixed-step loop over a module tree"]
fn case21_scale_and_determinism() {
    // 3 cranes, 6 crabs, 10 hoists (19 axes), dt 5 ms, 60 s of simulated time = 12000 steps.
    // Budget (to be confirmed with L5a): one step under 0.5 ms in a release build. Two runs
    // from the same inputs must give bit-identical positions (compare f64 bits).
    todo!(
        "L5a: build the tree, drive all axes with the same scripted inputs twice, compare to_bits, check the step budget"
    );
}
