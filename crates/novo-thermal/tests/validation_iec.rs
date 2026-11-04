//! IEC 60890 hand calculation for real enclosure sizes (lane H6).
//!
//! The reference is computed here from the surface factors b of the standard
//! (top 1.4 / 0.7 covered, sides, front and rear 0.9 / 0.5 covered, bottom 0.5
//! or 0 on the floor) and the wall coefficient k of the sheet material, not
//! through `novo_layout::sizing`. A sealed enclosure with its real outside
//! size and installation (the numbers a catalogue record provides) must give
//! the same effective surface, the same k * Ae and the same rise P / (k Ae).
//! The mean air rise of the 2D engine is then compared against it: it reads
//! 9 to 22 % low on these cases (the plate's direct path), never high.
//!
//! This follows the surface-factor method of IEC 60890 (1987) as the app
//! implements it, not the temperature-rise curves of IEC TR 60890:2022; the
//! two differ in shape (see docs/thermal-validation.md). Hand calculation,
//! not a measurement.

mod common;

use common::*;
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

struct Case {
    name: &'static str,
    w: f64,
    h: f64,
    d: f64,
    material: Material,
    install: Installation,
    loss_w: f64,
}

fn hand_ae(c: &Case) -> f64 {
    let (w, h, d) = (c.w / 1000.0, c.h / 1000.0, c.d / 1000.0);
    let i = c.install;
    let b = |free: f64, cov: f64, covered: bool| if covered { cov } else { free };
    w * d * b(1.4, 0.7, i.top_covered)
        + if i.on_floor { 0.0 } else { w * d * 0.5 }
        + w * h * 0.9
        + w * h * b(0.9, 0.5, i.rear_covered)
        + d * h * b(0.9, 0.5, i.left_covered)
        + d * h * b(0.9, 0.5, i.right_covered)
}

fn cases() -> Vec<Case> {
    let row = Installation::WallInRow;
    let mut side = Installation::Wall;
    side.left_covered = true;
    vec![
        Case {
            name: "600x760x210 wall, 100 W",
            w: 600.0,
            h: 760.0,
            d: 210.0,
            material: Material::Steel,
            install: Installation::Wall,
            loss_w: 100.0,
        },
        Case {
            name: "600x760x210 wall, one side covered",
            w: 600.0,
            h: 760.0,
            d: 210.0,
            material: Material::Steel,
            install: side,
            loss_w: 100.0,
        },
        Case {
            name: "600x760x210 row mounted",
            w: 600.0,
            h: 760.0,
            d: 210.0,
            material: Material::Steel,
            install: row,
            loss_w: 100.0,
        },
        Case {
            name: "800x1800x400 floor standing, 300 W",
            w: 800.0,
            h: 1800.0,
            d: 400.0,
            material: Material::Steel,
            install: Installation::FreeStanding,
            loss_w: 300.0,
        },
        Case {
            name: "800x2000x600 floor standing, 600 W",
            w: 800.0,
            h: 2000.0,
            d: 600.0,
            material: Material::Steel,
            install: Installation::FreeStanding,
            loss_w: 600.0,
        },
        Case {
            name: "600x760x210 aluminium wall, 100 W",
            w: 600.0,
            h: 760.0,
            d: 210.0,
            material: Material::Aluminium,
            install: Installation::Wall,
            loss_w: 100.0,
        },
    ]
}

fn scenario_for(c: &Case) -> Scenario {
    let mut sc = scenario(vec![]);
    sc.enclosure = Enclosure {
        width_mm: c.w,
        height_mm: c.h,
        depth_mm: c.d,
        wall_mm: 2.0,
        material: c.material,
        installation: c.install,
        plate: Rect {
            x: 40.0,
            y: 40.0,
            w: c.w - 90.0,
            h: c.h - 90.0,
        },
    };
    // a single device near the middle of the plate, 150 mm square
    sc.devices = vec![device(
        1,
        0.5 * (c.w - 150.0),
        0.5 * (c.h - 150.0),
        150.0,
        150.0,
        c.loss_w,
    )];
    sc
}

#[test]
fn effective_surface_and_rise_equal_the_hand_calculation() {
    for c in cases() {
        let k = match c.material {
            Material::Steel => 5.5,
            Material::Aluminium => 12.0,
            _ => unreachable!("{}", c.name),
        };
        let ae = hand_ae(&c);
        let sc = scenario_for(&c);
        let r = simulate_steady(&sc).unwrap();
        let hand = c.loss_w / (k * ae);
        println!(
            "VALIDATION | {} | IEC rise K | {:.4} | {:.4} | {:+.4}%",
            c.name,
            r.balance.iec_rise_k,
            hand,
            100.0 * (r.balance.iec_rise_k - hand) / hand
        );
        assert!(
            (r.balance.iec_rise_k - hand).abs() < 1e-6 * hand,
            "{}: {} vs hand {hand}",
            c.name,
            r.balance.iec_rise_k
        );
        // the engine's mean air rise reads 9 to 22 % below the hand value (the
        // mounting plate also gives heat to the outside directly, so the air
        // is cooler than the all-air-path IEC estimate); never above it
        let rel = (r.balance.mean_rise_k - hand) / hand;
        println!(
            "VALIDATION | {} | 2D mean air rise K | {:.3} | {:.3} | {:+.2}%",
            c.name,
            r.balance.mean_rise_k,
            hand,
            100.0 * rel
        );
        assert!(
            rel < 0.02 && rel > -0.25,
            "{}: mean air rise off by {rel}",
            c.name
        );
    }
}

#[test]
fn a_bigger_enclosure_or_more_covered_faces_changes_the_rise_the_right_way() {
    let cs = cases();
    let rise = |c: &Case| {
        simulate_steady(&scenario_for(c))
            .unwrap()
            .balance
            .iec_rise_k
    };
    // wall (rear covered) -> one more side covered -> both sides: less surface, hotter
    assert!(rise(&cs[1]) > rise(&cs[0]));
    assert!(rise(&cs[2]) > rise(&cs[1]));
    // aluminium is cooler than steel for the same box and loss
    assert!(rise(&cs[5]) < rise(&cs[0]));
}
