//! Wall distance, wall functions and the laminar passthrough (lane CFD-B).

use novo_cfd::result::Grid3;
use novo_cfd::turb::{
    FaceKind, TurbConfig, TurbMesh, TurbModel, TurbState, friction_velocity, thermal_t_plus,
    wall_quantities,
};

#[test]
fn wall_distance_of_a_closed_box_is_the_distance_to_the_nearest_wall() {
    let grid = Grid3::uniform([8, 6, 4], [800.0, 600.0, 400.0]);
    let m = TurbMesh::new(&grid, None, [FaceKind::Wall; 6]);
    let mut worst: f64 = 0.0;
    for k in 0..4 {
        for j in 0..6 {
            for i in 0..8 {
                let c = m.centre_of(i, j, k);
                let exact = (0..3)
                    .map(|a| c[a].min(m.size[a] - c[a]))
                    .fold(f64::INFINITY, f64::min);
                worst = worst.max((m.wall_dist[m.cell([i, j, k])] - exact).abs());
            }
        }
    }
    println!("wall distance, box: max error {worst:.3e} m");
    assert!(worst < 1e-12);
}

#[test]
fn solid_cells_are_walls_too() {
    // 1 x 10 x 1 column, solid at the top: distance grows away from it
    let grid = Grid3::uniform([1, 10, 1], [100.0, 1000.0, 100.0]);
    let mut solid = vec![false; 10];
    solid[9] = true;
    let mut kinds = [FaceKind::Symmetry; 6];
    kinds[2] = FaceKind::Wall;
    let m = TurbMesh::new(&grid, Some(&solid), kinds);
    // fluid cell 8 touches the solid cell: half a cell from it
    assert!((m.wall_dist[8] - 0.05).abs() < 1e-12);
    // cell 4 is 4.5 cells from the solid, 4.5 from the bottom wall
    assert!((m.wall_dist[4] - 0.45).abs() < 1e-12);
    assert_eq!(m.wall_dist[9], 0.0);
}

#[test]
fn friction_velocity_inverts_spalding_across_the_layers() {
    let nu = 1.5e-5;
    for &(u, d) in &[
        (0.01, 1e-3),
        (1.0, 1e-3),
        (5.0, 2e-3),
        (20.0, 0.01),
        (40.0, 0.05),
    ] {
        let ut = friction_velocity(u, d, nu);
        let yp = d * ut / nu;
        // viscous sublayer: u+ = y+; log layer: u+ = ln(9.8 y+)/kappa (approx)
        let up = u / ut;
        // only the layers with a closed form: sublayer and the far log layer
        let reference = if yp < 5.0 {
            yp
        } else if yp > 200.0 {
            (9.5 * yp).ln() / 0.41
        } else {
            up
        };
        println!("u {u} d {d}: y+ {yp:.2} u+ {up:.2} (reference {reference:.2})");
        assert!(ut > 0.0);
        assert!((up - reference).abs() / reference < 0.03);
    }
}

#[test]
fn wall_function_heat_transfer_limits() {
    // viscous limit T+ = Pr y+, log limit grows like 2.12 ln y+
    let pr = 0.71;
    assert!((thermal_t_plus(0.1, pr) - pr * 0.1).abs() / (pr * 0.1) < 0.01);
    let a = thermal_t_plus(1000.0, pr);
    let b = thermal_t_plus(2000.0, pr);
    assert!(((b - a) - 2.12 * 2.0_f64.ln()).abs() < 0.05);
    // wall diffusivity never below molecular, and rises with speed
    let (_, _, _, _, a1) = wall_quantities(0.5, 2e-3, 1.5e-5, pr);
    let (_, _, _, _, a2) = wall_quantities(5.0, 2e-3, 1.5e-5, pr);
    assert!(a1 >= 1.5e-5 / pr && a2 > a1);
}

#[test]
fn laminar_model_adds_no_viscosity() {
    let grid = Grid3::uniform([4, 4, 4], [100.0; 3]);
    let cfg = TurbConfig::air(TurbModel::Laminar);
    let mut s = TurbState::new(&grid, None, cfg, 1.0);
    let vel = std::array::from_fn(|c| vec![0.3; s.mesh.vel_len(c)]);
    let step = s.advance(&vel, None, 1.0, 2);
    assert_eq!(step.max_nu_t_ratio, 0.0);
    assert!(s.viscosity_ratio().iter().all(|v| *v == 0.0 || *v < 1e-3));
}

#[test]
fn still_air_stays_nearly_inviscid_and_stable() {
    for model in [TurbModel::KOmegaSst, TurbModel::KEpsilon] {
        let grid = Grid3::uniform([6, 6, 6], [400.0; 3]);
        let mut cfg = TurbConfig::air(model);
        cfg.intensity = 0.001;
        let mut s = TurbState::new(&grid, None, cfg, 0.1);
        let vel: [Vec<f64>; 3] = std::array::from_fn(|c| vec![0.0; s.mesh.vel_len(c)]);
        for _ in 0..200 {
            s.advance(&vel, None, 1.0, 2);
        }
        let max_ratio = s.viscosity_ratio().iter().fold(0.0_f32, |a, v| a.max(*v));
        println!("{model:?} still air: max nu_t/nu {max_ratio}");
        assert!(s.k.iter().all(|k| k.is_finite() && *k >= 0.0));
        assert!(max_ratio < 5.0);
    }
}
