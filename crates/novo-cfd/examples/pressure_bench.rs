//! Pressure solve time per smoother and thread count.
//! `cargo run --release -p novo-cfd --example pressure_bench`

use std::time::Instant;

use novo_cfd::flow::*;
use novo_cfd::sched::{Threads, default_threads, install};

fn main() {
    for n in [48usize, 80, 128] {
        let mesh = Mesh::uniform([n, n, n], [1.0; 3]);
        let pi = std::f64::consts::PI;
        let mut b = vec![0.0; mesh.cells()];
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    let (x, y, z) = (mesh.centre[0][i], mesh.centre[1][j], mesh.centre[2][k]);
                    b[mesh.cell([i, j, k])] =
                        (pi * x).cos() * (pi * y).cos() * (pi * z).cos() * mesh.volume([i, j, k]);
                }
            }
        }
        for (t, sm) in [
            (1, Smoother::Lexicographic),
            (default_threads(), Smoother::Lexicographic),
            (1, Smoother::RedBlack),
            (default_threads(), Smoother::RedBlack),
        ] {
            let (iters, secs) = install(Threads::Fixed(t), || {
                let mut s = PressureSolver::new(&mesh);
                s.set_smoother(sm);
                let mut p = vec![0.0; mesh.cells()];
                let t0 = Instant::now();
                let it = s.solve(&b, &mut p, 1e-9, 200);
                (it, t0.elapsed().as_secs_f64())
            });
            println!("{n}^3 {t} thread(s) {sm:?}: {iters} iterations, {secs:.2} s");
        }
    }
}
