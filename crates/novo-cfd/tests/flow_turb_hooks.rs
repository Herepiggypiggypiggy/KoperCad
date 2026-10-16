//! The hooks a turbulence model uses: eddy viscosity and wall values.

use novo_cfd::flow::*;

fn setup() -> (Mesh, FlowConfig) {
    let mesh = Mesh::uniform([10, 10, 3], [0.3, 0.3, 0.1]);
    let mut cfg = FlowConfig::air();
    cfg.nu = 1e-3;
    cfg.alpha = 1.4e-3;
    cfg.beta = 3.4e-3;
    cfg.faces[Face::YMin as usize].wall_velocity = [0.05, 0.0, 0.0];
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(40.0);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(10.0);
    (mesh, cfg)
}

/// `value` on the faces of the domain boundary, NaN elsewhere.
fn wall_table(m: &Mesh, value: f64) -> Vec<[f64; 6]> {
    let mut t = vec![[f64::NAN; 6]; m.cells()];
    for k in 0..m.n[2] {
        for j in 0..m.n[1] {
            for i in 0..m.n[0] {
                let ix = [i, j, k];
                for a in 0..3 {
                    if ix[a] == 0 {
                        t[m.cell(ix)][2 * a] = value;
                    }
                    if ix[a] + 1 == m.n[a] {
                        t[m.cell(ix)][2 * a + 1] = value;
                    }
                }
            }
        }
    }
    t
}

fn run(mut s: FlowSolver) -> FlowSolver {
    for _ in 0..40 {
        s.step(0.05);
    }
    s
}

fn same(a: &FlowSolver, b: &FlowSolver, tol: f64) -> bool {
    let dv = (0..3)
        .flat_map(|c| a.vel[c].iter().zip(&b.vel[c]).map(|(x, y)| (x - y).abs()))
        .fold(0.0, f64::max);
    let dt =
        a.t.iter()
            .zip(&b.t)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
    println!("max velocity difference {dv:.2e}, temperature {dt:.2e}");
    dv <= tol && dt <= tol
}

#[test]
fn zero_eddy_viscosity_is_bit_identical_to_laminar() {
    let (mesh, cfg) = setup();
    let cells = mesh.cells();
    let laminar = run(FlowSolver::new(mesh.clone(), cfg.clone(), 25.0));
    let mut s = FlowSolver::new(mesh, cfg, 25.0);
    s.nu_t = Some(vec![0.0; cells]);
    let with_hook = run(s);
    assert!(same(&laminar, &with_hook, 0.0));
}

#[test]
fn uniform_eddy_viscosity_acts_like_a_larger_molecular_one() {
    let (mesh, cfg) = setup();
    let cells = mesh.cells();
    let nu_t = 4e-3;
    let pr_t = cfg.prandtl_t;
    // reference: molecular values raised by the eddy part
    let mut big = cfg.clone();
    big.nu += nu_t;
    big.alpha += nu_t / pr_t;
    // keep rho cp: the conductivity grows with the diffusivity
    big.conductivity *= big.alpha / cfg.alpha;
    let reference = run(FlowSolver::new(mesh.clone(), big.clone(), 25.0));
    let laminar = run(FlowSolver::new(mesh.clone(), cfg.clone(), 25.0));
    assert!(
        !same(&laminar, &reference, 1e-6),
        "the eddy part must matter"
    );
    let mut s = FlowSolver::new(mesh, cfg, 25.0);
    s.nu_t = Some(vec![nu_t; cells]);
    // the walls must see the same values, as a wall function would set them
    let (wn, wa) = (wall_table(&s.mesh, big.nu), wall_table(&s.mesh, big.alpha));
    s.wall_nu = Some(wn);
    s.wall_alpha = Some(wa);
    let hooked = run(s);
    assert!(same(&reference, &hooked, 1e-12));
}

#[test]
fn nan_wall_entries_keep_the_molecular_value() {
    let (mesh, cfg) = setup();
    let cells = mesh.cells();
    let laminar = run(FlowSolver::new(mesh.clone(), cfg.clone(), 25.0));
    let mut s = FlowSolver::new(mesh, cfg, 25.0);
    s.wall_nu = Some(vec![[f64::NAN; 6]; cells]);
    s.wall_alpha = Some(vec![[f64::NAN; 6]; cells]);
    assert!(same(&laminar, &run(s), 0.0));
}
