//! Turbulent natural convection at a heated vertical wall (lane CFD-B).
//!
//! A tall air cavity, hot wall on the left, cold wall on the right, free
//! slip in depth, adiabatic top and bottom. The hot wall is compared with
//! the Churchill-Chu plate correlation for the temperature difference
//! between the wall and the core at mid height, the standard way to use a
//! plate correlation on a closed cavity. Errors are printed (`--nocapture`).

use novo_cfd::flow::*;
use novo_cfd::turb::{TurbModel, TurbulentFlow};

const PR: f64 = 0.71;

/// Churchill and Chu (1975), whole range, mean Nusselt number of a
/// vertical isothermal plate.
fn churchill_chu(ra: f64) -> f64 {
    let d = (1.0 + (0.492 / PR).powf(9.0 / 16.0)).powf(8.0 / 27.0);
    (0.825 + 0.387 * ra.powf(1.0 / 6.0) / d).powi(2)
}

fn stretched(n: usize, s: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| 0.5 * (1.0 + (s * (2.0 * i as f64 / n as f64 - 1.0)).tanh() / s.tanh()))
        .collect()
}

struct Out {
    ra_wall_core: f64,
    nu_hot: f64,
    nu_cold: f64,
    nu_ref: f64,
    steps: usize,
    max_nut: f64,
}

fn run(model: TurbModel, height: f64, d_t: f64, nx: usize, ny: usize, sx: f64, sy: f64) -> Out {
    let nu = 1.55e-5;
    let alpha = nu / PR;
    let mesh = Mesh::from_edges_m([
        stretched(nx, sx).iter().map(|v| v * height).collect(),
        stretched(ny, sy).iter().map(|v| v * height).collect(),
        vec![0.0, 0.05],
    ]);
    let mut cfg = FlowConfig::air();
    cfg.nu = nu;
    cfg.alpha = alpha;
    cfg.beta = 1.0 / 298.15;
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(d_t / 2.0);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(-d_t / 2.0);
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    let flow = FlowSolver::new(mesh, cfg, 0.0);
    let u_scale = (9.81 * flow.cfg.beta * d_t * height).sqrt();
    let mut tf = TurbulentFlow::new(flow, model, 0.05 * u_scale, 0.02 * height);
    tf.turb.cfg.second_order = std::env::var("TURB_2ND").is_ok();
    let mut steps = 0;
    let mut settled = 0;
    while steps < 120_000 && settled < 50 {
        let dt = tf.flow.max_dt(0.4).min(0.5);
        let (accel, dtdt) = tf.step(dt);
        steps += 1;
        if steps > 200 && accel / u_scale < 1e-4 && dtdt / d_t < 1e-5 {
            settled += 1;
        } else {
            settled = 0;
        }
    }
    let m = &tf.flow.mesh;
    let s = &tf.flow;
    // wall heat flux per unit area at the hot / cold wall, from the cell next to
    // the wall with the wall-function diffusivity of the last step
    let flux = |side: usize| -> f64 {
        let (i, tw) = if side == 0 {
            (0, d_t / 2.0)
        } else {
            (nx - 1, -d_t / 2.0)
        };
        let mut q = 0.0;
        for j in 0..ny {
            let c = m.cell([i, j, 0]);
            let wc = tf
                .turb
                .walls()
                .iter()
                .find(|w| w.cell == c && w.axis == 0)
                .copied();
            let (a, dist) = match wc {
                Some(w) => (w.alpha_wall, w.dist),
                None => (alpha, 0.5 * m.dx[0][i]),
            };
            q += (a * (tw - s.t[c]) / dist).abs() * m.dx[1][j];
        }
        q / height // mean flux, K m/s
    };
    let (q_hot, q_cold) = (flux(0), flux(1));
    let nu_hot = q_hot * height / (alpha * (d_t / 2.0 - core_t(s, nx, ny)));
    let nu_cold = q_cold * height / (alpha * (core_t(s, nx, ny) + d_t / 2.0));
    let d_core = d_t / 2.0 - core_t(s, nx, ny);
    let ra = 9.81 * s.cfg.beta * d_core * height.powi(3) / (nu * alpha);
    Out {
        ra_wall_core: ra,
        nu_hot,
        nu_cold,
        nu_ref: churchill_chu(ra),
        steps,
        max_nut: tf.turb.nu_t.iter().fold(0.0_f64, |a, v| a.max(*v)) / nu,
    }
}

/// Temperature at the centre of the cavity.
fn core_t(s: &FlowSolver, nx: usize, ny: usize) -> f64 {
    s.t[s.mesh.cell([nx / 2, ny / 2, 0])]
}

fn report(tag: &str, o: &Out) -> f64 {
    let err = (o.nu_hot - o.nu_ref) / o.nu_ref;
    println!(
        "{tag}: Ra(wall-core) {:.2e}, Nu hot {:.1} cold {:.1}, Churchill-Chu {:.1}, error {:+.1} %, max nu_t/nu {:.0}, {} steps",
        o.ra_wall_core,
        o.nu_hot,
        o.nu_cold,
        o.nu_ref,
        100.0 * err,
        o.max_nut,
        o.steps
    );
    err
}

// These run for minutes each; `cargo test -p novo-cfd --test turb_plate --
// --ignored --nocapture` prints the errors (about 4 minutes, run in parallel).

/// Resolved wall (first cell centre y+ below 1): the case the numbers in the
/// accuracy statement come from. Measured: Nu +9.8 % at Ra(wall-core) 3.4e9;
/// +4.3 % at 1.0e9 on a finer grid (40 x 80, 40 000 steps).
#[test]
#[ignore = "slow"]
fn sst_hot_wall_ra_4e9() {
    let o = run(TurbModel::KOmegaSst, 1.2, 40.0, 28, 56, 2.6, 1.5);
    let err = report("SST resolved", &o);
    assert!(o.ra_wall_core > 1e9);
    assert!(err.abs() < 0.2, "error {err}");
}

/// k-epsilon with wall functions on the same resolved grid. It is not meant
/// for resolved buoyant layers and over-predicts (measured +34.5 %); this
/// test keeps that disagreement visible and bounded.
#[test]
#[ignore = "slow"]
fn k_epsilon_hot_wall_ra_4e9_disagrees_on_resolved_grid() {
    let o = run(TurbModel::KEpsilon, 1.2, 40.0, 28, 56, 2.6, 1.5);
    let err = report("k-epsilon (resolved grid)", &o);
    assert!(err > 0.0 && err < 0.5, "error {err}");
}

/// Coarse wall cells (first cell much thicker than the boundary layer):
/// both models agree with each other (Nu 85.6 vs 85.0) and both are about
/// 52 % low against Churchill-Chu. The forced-flow wall functions cannot
/// represent a buoyant boundary layer thinner than the cell. Printed only:
/// the limit is documented, not asserted.
#[test]
#[ignore = "slow"]
fn coarse_wall_cells_underpredict_natural_convection() {
    let a = run(TurbModel::KOmegaSst, 1.2, 40.0, 16, 48, 1.3, 1.5);
    let b = run(TurbModel::KEpsilon, 1.2, 40.0, 16, 48, 1.3, 1.5);
    report("SST coarse wall cells", &a);
    report("k-epsilon coarse wall cells", &b);
    assert!((a.nu_hot - b.nu_hot).abs() / a.nu_hot < 0.05);
}
