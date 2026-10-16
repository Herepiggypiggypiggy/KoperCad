//! The threaded kernels give the same bits as one thread (lane CFD-E).

use novo_cfd::flow::*;
use novo_cfd::sched::{Threads, install};

fn run() -> (FlowSolver, f64, f64) {
    let mesh = Mesh::uniform([24, 24, 24], [0.5, 0.5, 0.5]);
    let mut cfg = FlowConfig::air();
    cfg.scheme = Scheme::VanLeer;
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(60.0);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(20.0);
    let mut s = FlowSolver::new(mesh, cfg, 40.0);
    for _ in 0..6 {
        let dt = s.max_dt(0.5).min(0.5);
        s.step(dt);
    }
    let (dt, div) = (s.max_dt(0.5), s.divergence_max());
    (s, dt, div)
}

fn checksum(s: &FlowSolver) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let all = s.vel.iter().flatten().chain(&s.t).chain(&s.p);
    for v in all {
        h = (h ^ v.to_bits()).wrapping_mul(0x100000001b3);
    }
    h
}

#[test]
fn one_and_many_threads_agree_bit_for_bit() {
    let (a, dt_a, div_a) = install(Threads::Fixed(1), run);
    let (b, dt_b, div_b) = install(Threads::Fixed(4), run);
    println!("checksum {:016x}", checksum(&a));
    assert_eq!(a.vel, b.vel);
    assert_eq!(a.t, b.t);
    assert_eq!(a.p, b.p);
    assert_eq!(dt_a.to_bits(), dt_b.to_bits());
    assert_eq!(div_a.to_bits(), div_b.to_bits());
    assert!(
        a.vel.iter().flatten().any(|v| v.abs() > 1e-4),
        "flow developed"
    );
}
