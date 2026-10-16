//! `TurbulentFlow`: the turbulence model in the hooks of the flow core.

use novo_cfd::flow::*;
use novo_cfd::turb::{TurbModel, TurbulentFlow};

fn lid_box() -> FlowSolver {
    let mesh = Mesh::uniform([12, 12, 3], [0.3, 0.3, 0.05]);
    let mut cfg = FlowConfig::air();
    cfg.faces[Face::YMin as usize].wall_velocity = [2.0, 0.0, 0.0];
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    FlowSolver::new(mesh, cfg, 20.0)
}

#[test]
fn laminar_model_is_bit_identical_to_the_bare_flow_solver() {
    let mut plain = lid_box();
    let mut coupled = TurbulentFlow::new(lid_box(), TurbModel::Laminar, 1.0, 0.05);
    for _ in 0..30 {
        plain.step(0.01);
        coupled.step(0.01);
    }
    assert!(coupled.flow.nu_t.is_none());
    for c in 0..3 {
        assert_eq!(plain.vel[c], coupled.flow.vel[c]);
    }
    assert_eq!(plain.t, coupled.flow.t);
}

#[test]
fn sst_runs_stably_and_adds_eddy_viscosity_in_a_driven_box() {
    for model in [TurbModel::KOmegaSst, TurbModel::KEpsilon] {
        let mut tf = TurbulentFlow::new(lid_box(), model, 2.0, 0.03);
        for _ in 0..200 {
            tf.step(0.01);
        }
        let ratio = tf.turb.viscosity_ratio();
        let max = ratio.iter().fold(0.0_f32, |a, v| a.max(*v));
        println!("{model:?}: max nu_t/nu {max:.1}");
        assert!(tf.flow.nu_t.is_some() && tf.flow.wall_nu.is_some());
        assert!(max > 1.0 && max.is_finite());
        assert!(tf.flow.vel[0].iter().all(|v| v.is_finite()));
        // wall values only on wall faces: none for the symmetry planes
        let walls = tf.turb.walls();
        assert!(!walls.is_empty() && walls.iter().all(|w| w.axis != 2));
    }
}
