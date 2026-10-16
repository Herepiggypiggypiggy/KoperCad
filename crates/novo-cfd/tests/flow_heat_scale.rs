//! `FlowSolver::heat_scale` is the load share of a transient run: it scales
//! every heat source, so the heating of an adiabatic box scales with it.

use novo_cfd::flow::{FaceBc, FlowConfig, FlowSolver, Mesh};

fn heated_rise(scale: f64) -> f64 {
    let mesh = Mesh::uniform([6, 6, 6], [0.3, 0.3, 0.3]);
    let n = mesh.cells();
    let mut cfg = FlowConfig::air();
    for f in &mut cfg.faces {
        *f = FaceBc::WALL;
    }
    cfg.heat_source = Some(vec![0.01; n]);
    let mut s = FlowSolver::new(mesh, cfg, 20.0);
    s.heat_scale = scale;
    for _ in 0..40 {
        s.step(0.5);
    }
    s.mean_temperature() - 20.0
}

#[test]
fn heating_scales_with_the_load_share() {
    let (full, half, off) = (heated_rise(1.0), heated_rise(0.5), heated_rise(0.0));
    println!("HEAT_SCALE | rise at 100 % {full:.6} K, 50 % {half:.6} K, 0 % {off:.2e} K");
    // 20 s at 0.01 K/s in a closed adiabatic box
    assert!((full - 0.2).abs() < 1e-6, "{full}");
    assert!((half - 0.5 * full).abs() < 1e-9);
    assert!(off.abs() < 1e-12);
}
