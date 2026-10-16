use novo_cfd::flow::{Mesh, PressureSolver};

fn stretched(n: usize, s: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| 0.5 * (1.0 + (s * (2.0 * i as f64 / n as f64 - 1.0)).tanh() / s.tanh()))
        .collect()
}

#[test]
fn poisson_solution_matches_the_manufactured_one() {
    // p = cos(pi x) cos(pi y) cos(pi z) on the unit cube has zero normal
    // gradient on the walls and -Lp = 3 pi^2 p.
    for (n, tol) in [([16, 16, 16], 0.02), ([32, 32, 32], 0.006)] {
        let mesh = Mesh::uniform(n, [1.0; 3]);
        let mut solver = PressureSolver::new(&mesh);
        assert!(solver.level_count() >= 3);
        let pi = std::f64::consts::PI;
        let exact = |x: [f64; 3]| (pi * x[0]).cos() * (pi * x[1]).cos() * (pi * x[2]).cos();
        let mut b = vec![0.0; mesh.cells()];
        let mut p_exact = vec![0.0; mesh.cells()];
        for k in 0..n[2] {
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let ix = [i, j, k];
                    let x = [mesh.centre[0][i], mesh.centre[1][j], mesh.centre[2][k]];
                    p_exact[mesh.cell(ix)] = exact(x);
                    b[mesh.cell(ix)] = 3.0 * pi * pi * exact(x) * mesh.volume(ix);
                }
            }
        }
        let mut p = vec![0.0; mesh.cells()];
        let iters = solver.solve(&b, &mut p, 1e-12, 100);
        let err = p
            .iter()
            .zip(&p_exact)
            .map(|(a, e)| (a - e).abs())
            .fold(0.0, f64::max);
        println!("poisson {n:?}: {iters} iterations, max error {err:.2e}");
        assert!(
            iters <= 40,
            "multigrid-preconditioned CG should converge fast"
        );
        assert!(err < tol, "discretisation error {err}");
    }
}

#[test]
fn nonuniform_mesh_converges_too() {
    let n = [12, 20, 8];
    let mesh = Mesh::from_edges_m([
        stretched(n[0], 1.5),
        stretched(n[1], 2.0),
        stretched(n[2], 1.0),
    ]);
    let mut solver = PressureSolver::new(&mesh);
    let mut b: Vec<f64> = (0..mesh.cells())
        .map(|c| ((c * 7919) % 101) as f64 - 50.0)
        .collect();
    let mean = b.iter().sum::<f64>() / b.len() as f64;
    b.iter_mut().for_each(|v| *v -= mean);
    let mut p = vec![0.0; mesh.cells()];
    let iters = solver.solve(&b, &mut p, 1e-10, 200);
    let mut ap = vec![0.0; mesh.cells()];
    solver.apply(&p, &mut ap);
    let res = ap
        .iter()
        .zip(&b)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt()
        / b.iter().map(|v| v * v).sum::<f64>().sqrt();
    println!("nonuniform: {iters} iterations, residual {res:.2e}");
    assert!(res < 1e-9);
}
