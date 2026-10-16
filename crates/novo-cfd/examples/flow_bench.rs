//! Steps of the flow core per thread count (heated cavity, 48^3).
//! `cargo run --release -p novo-cfd --example flow_bench`

use std::time::Instant;

use novo_cfd::flow::*;
use novo_cfd::sched::{Threads, default_threads, install};

fn main() {
    for t in [1, 2, default_threads()] {
        let secs = install(Threads::Fixed(t), || {
            let mesh = Mesh::uniform([48, 48, 48], [0.5, 0.5, 0.5]);
            let mut cfg = FlowConfig::air();
            cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(60.0);
            cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(20.0);
            let mut s = FlowSolver::new(mesh, cfg, 40.0);
            let t0 = Instant::now();
            for _ in 0..10 {
                let dt = s.max_dt(0.5).min(0.5);
                s.step(dt);
            }
            t0.elapsed().as_secs_f64()
        });
        println!("{t} thread(s): {:.2} s for 10 steps", secs);
    }
}
