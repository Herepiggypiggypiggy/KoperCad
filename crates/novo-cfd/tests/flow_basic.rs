use novo_cfd::flow::*;

fn base(n: [usize; 3]) -> (Mesh, FlowConfig) {
    let mesh = Mesh::uniform(n, [0.5, 0.5, 0.5]);
    let mut cfg = FlowConfig::air();
    cfg.scheme = Scheme::VanLeer;
    (mesh, cfg)
}

#[test]
fn still_uniform_air_stays_still() {
    let (mesh, cfg) = base([8, 8, 8]);
    let mut s = FlowSolver::new(mesh, cfg, 30.0);
    for _ in 0..20 {
        s.step(0.1);
    }
    let vmax = s.vel.iter().flatten().fold(0.0_f64, |a, v| a.max(v.abs()));
    println!("still air: max velocity {vmax:.2e}");
    assert!(vmax < 1e-6, "velocity {vmax}");
    assert!(s.t.iter().all(|t| (t - 30.0).abs() < 1e-12));
}

#[test]
fn conduction_between_fixed_walls_is_linear() {
    // No flow (gravity off, nothing to drive it): steady 1D conduction,
    // T(x) = 20 + 40 x / L at the cell centres.
    let (mesh, mut cfg) = base([20, 3, 3]);
    cfg.gravity = [0.0; 3];
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(20.0);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(60.0);
    let mut s = FlowSolver::new(mesh, cfg, 20.0);
    for _ in 0..1500 {
        s.step(20.0);
    }
    let mut worst: f64 = 0.0;
    for i in 0..20 {
        let exact = 20.0 + 40.0 * s.mesh.centre[0][i] / 0.5;
        worst = worst.max((s.t[s.mesh.cell([i, 1, 1])] - exact).abs());
    }
    println!("conduction: max error {worst:.2e} K");
    assert!(worst < 1e-6);
}

#[test]
fn a_heated_box_conserves_energy() {
    // Adiabatic closed box with a uniform source: the mean temperature rises
    // by exactly source * time, whatever the air does.
    let (mesh, mut cfg) = base([10, 10, 10]);
    let cells = mesh.cells();
    let mut source = vec![0.0; cells];
    source[mesh.cell([5, 2, 5])] = 10.0;
    cfg.heat_source = Some(source);
    let mut s = FlowSolver::new(mesh, cfg, 25.0);
    for _ in 0..200 {
        s.step(0.2);
    }
    let mean = s.t.iter().sum::<f64>() / cells as f64;
    let expected = 25.0 + 10.0 * s.time / cells as f64;
    println!("energy: mean {mean:.9}, expected {expected:.9}");
    assert!((mean - expected).abs() < 1e-8 * expected);
    assert!(s.divergence_max() < 1e-8);
    let vmax = s.vel.iter().flatten().fold(0.0_f64, |a, v| a.max(v.abs()));
    assert!(vmax > 1e-4, "the plume should move air");
}
