//! Stack effect (chimney) against the textbook law, independent of CFD-C2's own check in `openings.rs`.
//!
//! A heat source `P` on the floor of a box with a low inlet and a high outlet, both square vents
//! with an orifice loss. With fully mixed exhaust air at `dT = P / (rho cp Q)` the pressure
//! head of the column is `rho g H dT / T` and the two vents in series give
//! `Q^3 = A_eq^2 * 2 g H P / (T rho cp)` with `A_eq = Cd A / sqrt(2)`.
//! The law is an idealisation (fully mixed exhaust, no plume, no entry loss beyond the orifice),
//! so the signed error is reported and the band is wide. The sweep checks three things:
//! heat leaves with the air (energy), `Q ~ P^(1/3)` (scaling), and the level against the law
//! (the mesh check is a separate ignored test). Run `-- --nocapture` to see the `VALIDATION` rows.

mod validation;
use novo_cfd::flow::*;
use novo_cfd::openings::*;
use validation::report;

const T_AMB: f64 = 25.0;
const RHO: f64 = 1.18;
const CP_RHO: f64 = 0.026 / 2.2e-5; // J/m3K from the air constants of FlowConfig::air
const CD: f64 = 0.6;
const SIDE: f64 = 0.1; // square vents, m
const H: f64 = 0.6; // box height, m

/// Mean volume flow (m3/s) and heat carried out by the air (W), averaged over the last quarter.
fn chimney(power_w: f64, cells: [usize; 3]) -> (f64, f64) {
    let (w, d) = (0.3, 0.3);
    let mesh = Mesh::uniform(cells, [w, H, d]);
    let vent = || OpenKind::vent(1.0, CD);
    // y is down: the floor is at y = H. Inlet low on the left wall, outlet high on the right wall.
    let patches = vec![
        OpenPatch {
            face: Face::XMin,
            lo: [H - 0.05 - SIDE, 0.1],
            hi: [H - 0.05, 0.1 + SIDE],
            kind: vent(),
            inflow_c: None,
        },
        OpenPatch {
            face: Face::XMax,
            lo: [0.05, 0.1],
            hi: [0.05 + SIDE, 0.1 + SIDE],
            kind: vent(),
            inflow_c: None,
        },
    ];
    let mut cfg = FlowConfig::air();
    cfg.openings = patches;
    cfg.t_ref = Some(T_AMB);
    cfg.rho = RHO;
    // A source block at the bottom centre, a fifth of the width and depth.
    let (ni, nj, nk) = (cells[0], cells[1], cells[2]);
    let (i0, i1) = (2 * ni / 5, 3 * ni / 5);
    let (k0, k1) = (2 * nk / 5, 3 * nk / 5);
    let (j0, j1) = (nj - nj / 8 - 1, nj - nj / 8 + 1);
    let mut vol = 0.0;
    for k in k0..k1 {
        for j in j0..j1 {
            for i in i0..i1 {
                vol += mesh.volume([i, j, k]);
            }
        }
    }
    let mut src = vec![0.0; mesh.cells()];
    for k in k0..k1 {
        for j in j0..j1 {
            for i in i0..i1 {
                src[mesh.cell([i, j, k])] = power_w / (CP_RHO * vol);
            }
        }
    }
    cfg.heat_source = Some(src);
    let mut s = FlowSolver::new(mesh, cfg, T_AMB);
    let (t_end, t_avg) = (240.0, 180.0);
    let (mut t, mut n) = (0.0, 0.0);
    let (mut q_sum, mut heat_sum) = (0.0, 0.0);
    while t < t_end {
        let dt = s.max_dt(0.4).min(0.5);
        s.step(dt);
        t += dt;
        if t > t_avg {
            let (mut num, mut den) = (0.0, 0.0);
            for f in s.open.faces.iter().filter(|f| f.patch == 1) {
                let u = f.outward() * s.vel[f.axis][f.vel];
                num += u * f.area * (s.t[f.cell] - T_AMB);
                den += u * f.area;
            }
            q_sum += den * dt;
            heat_sum += CP_RHO * num * dt;
            n += dt;
        }
    }
    (q_sum / n, heat_sum / n)
}

/// Ideal fully mixed stack law for the geometry above.
fn ideal(power_w: f64) -> f64 {
    let a = CD * SIDE * SIDE;
    let a_eq = a / 2.0_f64.sqrt();
    let h_centres = (H - 0.05 - SIDE / 2.0) - (0.05 + SIDE / 2.0);
    (a_eq * a_eq * 2.0 * 9.81 * h_centres * power_w / ((T_AMB + 273.15) * CP_RHO)).cbrt()
}

#[test]
fn the_chimney_follows_the_stack_law_in_level_scaling_and_energy() {
    let coarse = [8, 16, 8];
    let powers = [40.0, 160.0];
    let mut flows = Vec::new();
    for &p in &powers {
        let (q, heat) = chimney(p, coarse);
        let case = format!("chimney {p} W, 8x16x8 cells");
        let err = report(&case, "flow vs ideal stack law m3/s", q, ideal(p));
        report(&case, "heat carried out by the air W", heat, p);
        // measured -14.5 % on the coarse mesh; the ideal law is an idealisation (fully mixed exhaust, no plume); grid dependence not yet bounded
        assert!(err.abs() < 0.18, "{case}: flow error {err}");
        assert!((heat - p).abs() / p < 0.04, "{case}: energy {heat}");
        flows.push(q);
    }
    // Q ~ P^(1/3): a factor 4 in power is 4^(1/3) = 1.587 in flow.
    for w in flows.windows(2) {
        let ratio = w[1] / w[0];
        let want = 4.0_f64.cbrt();
        report(
            "chimney power sweep",
            "flow ratio per factor 4 in power",
            ratio,
            want,
        );
        assert!((ratio / want - 1.0).abs() < 0.08, "scaling {ratio}");
    }
}

/// Finer mesh at the middle power. Slow (about a minute), so it runs with `--ignored`.
#[test]
#[ignore = "slow"]
fn the_chimney_flow_moves_little_on_a_finer_mesh() {
    let (q_coarse, _) = chimney(40.0, [8, 16, 8]);
    let (qf, heat_f) = chimney(40.0, [12, 24, 12]);
    let case = "chimney 40 W, 12x24x12 cells";
    let err = report(case, "flow vs ideal stack law m3/s", qf, ideal(40.0));
    report(case, "heat carried out by the air W", heat_f, 40.0);
    report(case, "flow change from the coarse mesh", qf, q_coarse);
    assert!(err.abs() < 0.15, "fine mesh flow error {err}"); // measured -10.8 %
    assert!((qf / q_coarse - 1.0).abs() < 0.08, "mesh change");
    assert!((heat_f - 40.0).abs() / 40.0 < 0.04);
}
