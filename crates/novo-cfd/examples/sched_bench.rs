//! Prints wall time of the scheduler per thread count.
//! `cargo run --release -p novo-cfd --example sched_bench`

use std::sync::Arc;
use std::time::Instant;

use novo_cfd::sched::{Plan, Scheduler, Threads, default_threads};
use novo_cfd::{Fidelity, StubSolver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn main() {
    let scn = Scenario {
        enclosure: Enclosure {
            width_mm: 800.0,
            height_mm: 1200.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 50.0,
                y: 50.0,
                w: 690.0,
                h: 1090.0,
            },
        },
        ambient_c: 25.0,
        devices: Vec::new(),
        obstacles: Vec::new(),
        vents: Vec::new(),
        fans: Vec::new(),
        params: Params::default(),
    };
    println!("cores for kernels (auto): {}", default_threads());
    for t in [1, 2, default_threads()] {
        let mut s = Scheduler::new(Arc::new(StubSolver));
        let mut plan = Plan::single(Fidelity::Coarse);
        plan.threads = Threads::Fixed(t);
        let t0 = Instant::now();
        s.start(scn.clone(), plan);
        while !s.current().is_some_and(|j| j.is_finished()) {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        println!("{t} thread(s): {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
    }
}
