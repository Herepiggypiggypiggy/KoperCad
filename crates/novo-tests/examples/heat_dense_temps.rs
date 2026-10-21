//! Temperatures of the first cabinet of the generated big project (60 placed
//! devices, 1.6 x 1.9 m plate), 2D solver, central run. Prints one table row
//! so two solver versions can be compared in kelvin. Uses only APIs that
//! older commits have too.
//!
//! ```text
//! cargo run --release -p novo-tests --example heat_dense_temps
//! ```

use novo_core::Document;
use novo_tests::{BigSpec, big_project};
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};
use novo_thermal::{Options, simulate_steady_with};

fn main() {
    let mut doc = Document::default();
    let spec = if std::env::args().any(|a| a == "--uniform") {
        BigSpec::SMALL.uniform()
    } else {
        BigSpec::SMALL
    };
    let big = big_project(&mut doc, spec).expect("generator");
    let s = scenario_from_project(doc.project(), big.cabinets[0], &BridgeOptions::default())
        .expect("scenario")
        .scenario;
    let start = std::time::Instant::now();
    let r = simulate_steady_with(
        &s,
        Options {
            bands: false,
            advice: false,
        },
    )
    .expect("solves");
    let hottest = r
        .devices
        .iter()
        .map(|d| d.surface_c)
        .fold(f64::MIN, f64::max);
    let mean_surface = r.devices.iter().map(|d| d.surface_c).sum::<f64>() / r.devices.len() as f64;
    let mean_air = r.devices.iter().map(|d| d.air_c).sum::<f64>() / r.devices.len() as f64;
    let stalled = r.notes.iter().any(|n| n.contains("stalled"));
    eprintln!("{:.1} s", start.elapsed().as_secs_f64());
    println!(
        "| generated cabinet {}, {} devices, {:.0} W | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {} | {} | {} |",
        if std::env::args().any(|a| a == "--uniform") {
            "(uniform)"
        } else {
            "(varied)"
        },
        r.devices.len(),
        r.devices.iter().map(|d| d.loss_w).sum::<f64>(),
        r.balance.mean_rise_k,
        r.air_at_top_c,
        hottest,
        mean_surface,
        mean_air,
        r.iterations,
        r.converged,
        stalled
    );
}
