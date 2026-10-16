//! The hooks a turbulence model uses: eddy viscosity and wall values.

use novo_cfd::flow::*;
use novo_cfd::solids::Medium;

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

#[test]
fn a_wall_function_next_to_a_solid_conserves_heat() {
    // Air | heated solid column, the air side wall fixed at 20 C, everything
    // else adiabatic. The wall function on the air side of the interface
    // changes the face conductance; the heat leaving the air cell must be
    // the heat entering the solid cell, so at steady state the wall takes
    // exactly the heating.
    let mesh = Mesh::uniform([12, 3, 3], [0.12, 0.03, 0.03]);
    let cells = mesh.cells();
    let mut md = Medium::air(cells);
    let heat = 5000.0; // W/m3 in the solid
    for k in 0..3 {
        for j in 0..3 {
            for i in 6..12 {
                let id = mesh.cell([i, j, k]);
                md.fluid_frac[id] = 0.0;
                md.k_solid[id] = 2.0;
                md.c_solid[id] = 2.0e6;
                md.heat_w_m3[id] = heat;
            }
        }
    }
    let mut cfg = FlowConfig::air();
    cfg.gravity = [0.0; 3];
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(20.0);
    cfg.medium = Some(md);
    let mut s = FlowSolver::new(mesh, cfg, 20.0);
    // the air cell at the interface: its upper x face gets a wall value
    // three times the molecular diffusivity
    let mut table = vec![[f64::NAN; 6]; cells];
    for k in 0..3 {
        for j in 0..3 {
            table[s.mesh.cell([5, j, k])][1] = 3.0 * 2.2e-5;
        }
    }
    s.wall_alpha = Some(table);
    let mut steps = 0;
    let mut dtdt = 0.0;
    for _ in 0..40000 {
        let info = s.step(2000.0);
        steps += 1;
        dtdt = info.max_dtdt;
        if dtdt < 1e-12 {
            break;
        }
    }
    println!("steps {steps}, dT/dt {dtdt:.2e}");
    let q_in = heat * 6.0 * 0.01 * 0.03 * 0.03;
    let q_out = s.wall_heat_w(Face::XMin as usize);
    println!("wall function at a solid: heating {q_in:.6} W, wall takes {q_out:.6} W");
    assert!((q_out - q_in).abs() / q_in < 1e-6, "{q_out} vs {q_in}");
}
