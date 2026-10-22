//! Heat of the wires in a duct: the engine puts it into the air of the duct
//! cells and the energy balance counts it.

mod common;

use common::*;
use novo_layout::Rect;
use novo_thermal::wires::{DuctWire, loss_by_duct};
use novo_thermal::*;

fn duct(loss_w: f64) -> Obstacle {
    Obstacle {
        kind: ObstacleKind::Duct,
        rect: Rect {
            x: 120.0,
            y: 500.0,
            w: 560.0,
            h: 60.0,
        },
        blockage: Some(0.3),
        loss_w,
    }
}

fn with_duct(loss_w: f64) -> Scenario {
    let mut sc = scenario(vec![]);
    sc.obstacles = vec![duct(loss_w)];
    sc
}

#[test]
fn the_heat_of_wires_in_a_duct_counts_in_the_balance_and_closes_it() {
    // Hand calculation: 8 conductors of 2.5 mm2, 3 m in the duct, 12 A,
    // copper at 70 C: R = 0.020629 * 3 / 2.5 = 0.024755 ohm,
    // P = 8 * 144 * 0.024755 = 28.52 W.
    let p = loss_by_duct(
        &(0..8)
            .map(|_| DuctWire {
                duct: 1,
                length_m: 3.0,
                section_mm2: 2.5,
                current_a: 12.0,
                temperature_c: 70.0,
                conductors: 1,
            })
            .collect::<Vec<_>>(),
    )[&1];
    assert!((p - 28.52).abs() < 0.02, "{p}");

    let r = quick(with_duct(p));
    assert!(r.converged);
    assert!((r.balance.loss_w - p).abs() < 1e-9, "{:?}", r.balance);
    assert!(r.balance.residual_rel.abs() < 1e-3, "{:?}", r.balance);
}

#[test]
fn wire_heat_raises_the_mean_air_like_the_same_watts_in_a_device() {
    let mut sc = with_duct(100.0);
    sc.params.plate_contact_h = 0.0;
    let wires = quick(sc);
    // Same watts in a device that only heats the air.
    let mut sc = scenario(vec![device(1, 300.0, 520.0, 100.0, 20.0, 100.0)]);
    sc.params.plate_contact_h = 0.0;
    let device = quick(sc);
    let (a, b) = (wires.balance.mean_rise_k, device.balance.mean_rise_k);
    assert!(a > 1.0, "{a}");
    assert!((a - b).abs() <= 0.1 * b, "wires {a} K, device {b} K");
    // IEC 60890 estimate: rise = P / (k Ae).
    assert!(
        (a - wires.balance.iec_rise_k).abs() <= 0.15 * wires.balance.iec_rise_k,
        "{a} vs {}",
        wires.balance.iec_rise_k
    );
}

#[test]
fn doubling_the_current_quadruples_the_wire_heat_and_the_rise() {
    let w = |i: f64| {
        DuctWire {
            duct: 1,
            length_m: 3.0,
            section_mm2: 1.5,
            current_a: i,
            temperature_c: 70.0,
            conductors: 3,
        }
        .loss_w()
    };
    let (low, high) = (w(8.0), w(16.0));
    assert!((high / low - 4.0).abs() < 1e-9);
    let a = quick(with_duct(low));
    let b = quick(with_duct(high));
    let ratio = b.balance.mean_rise_k / a.balance.mean_rise_k;
    assert!((ratio - 4.0).abs() < 0.4, "{ratio}");
}

#[test]
fn the_duct_air_is_warmer_than_the_cabinet_mean() {
    let r = quick(with_duct(150.0));
    assert!(
        r.balance.max_air_c > r.balance.mean_air_c + 0.5,
        "{:?}",
        r.balance
    );
}

#[test]
fn a_duct_without_wire_heat_changes_nothing() {
    let r = quick(with_duct(0.0));
    assert!(r.balance.mean_rise_k.abs() < 1e-6, "{:?}", r.balance);
}

#[test]
fn a_negative_wire_heat_is_refused() {
    assert!(simulate_steady(&with_duct(-5.0)).is_err());
    assert!(simulate_steady(&with_duct(f64::NAN)).is_err());
}
