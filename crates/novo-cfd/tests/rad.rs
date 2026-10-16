#![allow(clippy::needless_range_loop)]

use novo_cfd::mesh::{Mesh, MeshSettings, Region};
use novo_cfd::rad::radiosity::{SIGMA, solve_with};
use novo_cfd::rad::*;
use novo_cfd::{Grid3, MaterialInfo, MaterialKind};

fn air() -> MaterialInfo {
    MaterialInfo {
        id: 0,
        kind: MaterialKind::Air,
        name: "Air".into(),
        conductivity_w_mk: 0.026,
    }
}

fn solid(id: u16, kind: MaterialKind) -> MaterialInfo {
    MaterialInfo {
        id,
        kind,
        name: format!("m{id}"),
        conductivity_w_mk: 1.0,
    }
}

/// Empty box: six wall patches, one per face.
fn empty_box(n: [usize; 3], size: [f64; 3]) -> Mesh {
    Mesh::classify(Grid3::uniform(n, size), &[], vec![air()])
}

fn whole_faces() -> RadSettings {
    RadSettings {
        patches: PatchSettings {
            block: Some(1000),
            ..PatchSettings::default()
        },
        view_factors: ViewFactorSettings {
            rays_per_patch: 40_000,
            ..ViewFactorSettings::default()
        },
        ..RadSettings::default()
    }
}

/// Patch index of the wall facing `dir` (`2 * axis + high`).
fn wall(model: &RadiationModel, mesh: &Mesh, dir: u8) -> usize {
    let g = &mesh.grid;
    let cell = match dir {
        0 => 0,
        1 => g.nx - 1,
        2 => 0,
        3 => (g.ny - 1) * g.nx,
        4 => 0,
        _ => (g.nz - 1) * g.nx * g.ny,
    };
    model.patches.patch_of(cell, dir).expect("wall patch")
}

fn parallel_rects(x: f64, y: f64) -> f64 {
    let (a, b) = (1.0 + x * x, 1.0 + y * y);
    2.0 / (std::f64::consts::PI * x * y)
        * (((a * b) / (1.0 + x * x + y * y)).sqrt().ln()
            + x * b.sqrt() * (x / b.sqrt()).atan()
            + y * a.sqrt() * (y / a.sqrt()).atan()
            - x * x.atan()
            - y * y.atan())
}

/// Two rectangles with a common edge of length `l`, at right angles.
fn perpendicular_rects(h: f64, w: f64) -> f64 {
    let (h2, w2) = (h * h, w * w);
    let r = (h2 + w2).sqrt();
    let t1 = (1.0 + w2) * (1.0 + h2) / (1.0 + w2 + h2);
    let t2 = (w2 * (1.0 + w2 + h2) / ((1.0 + w2) * (w2 + h2))).powf(w2);
    let t3 = (h2 * (1.0 + h2 + w2) / ((1.0 + h2) * (h2 + w2))).powf(h2);
    (w * (1.0 / w).atan() + h * (1.0 / h).atan() - r * (1.0 / r).atan()
        + 0.25 * (t1 * t2 * t3).ln())
        / (std::f64::consts::PI * w)
}

#[test]
fn analytic_formulas_match_the_textbook_cube_values() {
    assert!((parallel_rects(1.0, 1.0) - 0.19982).abs() < 1e-4);
    assert!((perpendicular_rects(1.0, 1.0) - 0.20004).abs() < 1e-4);
}

#[test]
fn cube_view_factors_match_the_analytic_values() {
    let mesh = empty_box([6, 6, 6], [100.0; 3]);
    let model = RadiationModel::build(&mesh, &whole_faces()).unwrap();
    assert_eq!(model.patches.len(), 6);
    let (bottom, top, left) = (
        wall(&model, &mesh, 4),
        wall(&model, &mesh, 5),
        wall(&model, &mesh, 0),
    );
    let vf = &model.view_factors;
    assert!(
        (vf.get(bottom, top) - 0.19982).abs() < 2e-3,
        "{}",
        vf.get(bottom, top)
    );
    assert!(
        (vf.get(bottom, left) - 0.20004).abs() < 2e-3,
        "{}",
        vf.get(bottom, left)
    );
    assert_eq!(vf.get(bottom, bottom), 0.0);
}

#[test]
fn rectangular_box_matches_parallel_and_perpendicular_formulas() {
    // 200 x 100 x 50 mm.
    let mesh = empty_box([8, 4, 4], [200.0, 100.0, 50.0]);
    let model = RadiationModel::build(&mesh, &whole_faces()).unwrap();
    let vf = &model.view_factors;
    let (back, front) = (wall(&model, &mesh, 4), wall(&model, &mesh, 5));
    let want = parallel_rects(200.0 / 50.0, 100.0 / 50.0);
    assert!(
        (vf.get(back, front) - want).abs() < 3e-3,
        "{} vs {want}",
        vf.get(back, front)
    );
    // Back wall (200 x 100) to the left wall (100 x 50): common edge 100.
    let left = wall(&model, &mesh, 0);
    let want = perpendicular_rects(50.0 / 100.0, 200.0 / 100.0);
    assert!(
        (vf.get(back, left) - want).abs() < 3e-3,
        "{} vs {want}",
        vf.get(back, left)
    );
}

#[test]
fn reciprocity_and_summation_hold_to_round_off() {
    let mesh = empty_box([6, 6, 6], [100.0, 160.0, 70.0]);
    let st = RadSettings {
        view_factors: ViewFactorSettings {
            rays_per_patch: 500,
            ..ViewFactorSettings::default()
        },
        patches: PatchSettings {
            block: Some(2),
            ..PatchSettings::default()
        },
        ..RadSettings::default()
    };
    let model = RadiationModel::build(&mesh, &st).unwrap();
    assert!(model.patches.len() > 6);
    assert!(model.view_factors.reciprocity_error() < 1e-10);
    assert!(model.view_factors.summation_error() < 1e-10);
    // The raw sampling error is reported and small for this ray count.
    assert!(model.view_factors.stats.max_row_sum_error < 1e-9);
    assert_eq!(model.view_factors.stats.lost_rays, 0);
}

#[test]
fn raw_sampling_error_falls_with_more_rays() {
    let mesh = empty_box([6, 6, 6], [100.0; 3]);
    let err = |rays: usize| {
        let st = RadSettings {
            patches: PatchSettings {
                block: Some(2),
                ..PatchSettings::default()
            },
            view_factors: ViewFactorSettings {
                rays_per_patch: rays,
                enforce: false,
                ..ViewFactorSettings::default()
            },
            ..RadSettings::default()
        };
        let m = RadiationModel::build(&mesh, &st).unwrap();
        m.view_factors.stats.max_reciprocity_error
    };
    assert!(err(20_000) < 0.5 * err(500), "{} {}", err(20_000), err(500));
}

#[test]
fn view_factors_are_deterministic() {
    let mesh = empty_box([4, 4, 4], [100.0; 3]);
    let a = RadiationModel::build(&mesh, &RadSettings::default()).unwrap();
    let b = RadiationModel::build(&mesh, &RadSettings::default()).unwrap();
    assert_eq!(a.view_factors, b.view_factors);
}

#[test]
fn a_partition_blocks_the_view_and_shadows_part_of_it() {
    let materials = vec![air(), solid(1, MaterialKind::Wall)];
    let grid = Grid3::uniform([10, 10, 10], [100.0; 3]);
    let slab = |lo: [f64; 3], hi: [f64; 3]| Region {
        material: 1,
        lo,
        hi,
    };
    // Full-width slab at mid height splits the box in two.
    let full = Mesh::classify(
        grid.clone(),
        &[slab([0.0, 0.0, 40.0], [100.0, 100.0, 60.0])],
        materials.clone(),
    );
    let m = RadiationModel::build(&full, &whole_faces()).unwrap();
    let (b, t) = (wall(&m, &full, 4), wall(&m, &full, 5));
    assert_eq!(m.view_factors.get(b, t), 0.0);
    // Half-width slab: part of the top is visible, less than without.
    let half = Mesh::classify(
        grid.clone(),
        &[slab([0.0, 0.0, 40.0], [50.0, 100.0, 60.0])],
        materials,
    );
    let m = RadiationModel::build(&half, &whole_faces()).unwrap();
    let (b, t) = (wall(&m, &half, 4), wall(&m, &half, 5));
    let f = m.view_factors.get(b, t);
    assert!(f > 0.02 && f < 0.19, "{f}");
    // Rows still sum to one.
    assert!(m.view_factors.summation_error() < 1e-10);
}

/// Dense Gaussian elimination for `(I - rho F) J = E`.
fn direct(eps: &[f64], f: &ViewFactors, e: &[f64]) -> Vec<f64> {
    let n = eps.len();
    let mut a = vec![vec![0.0; n + 1]; n];
    for i in 0..n {
        for j in 0..n {
            a[i][j] = f64::from(u8::from(i == j)) - (1.0 - eps[i]) * f.get(i, j);
        }
        a[i][n] = e[i];
    }
    for c in 0..n {
        let p = (c..n)
            .max_by(|&x, &y| a[x][c].abs().total_cmp(&a[y][c].abs()))
            .unwrap();
        a.swap(c, p);
        for r in 0..n {
            if r != c {
                let k = a[r][c] / a[c][c];
                for q in c..=n {
                    a[r][q] -= k * a[c][q];
                }
            }
        }
    }
    (0..n).map(|i| a[i][n] / a[i][i]).collect()
}

#[test]
fn radiosity_solution_matches_a_direct_solve_and_balances() {
    // Analytic cube factors: opposite 0.19982, four neighbours 0.20004 each,
    // renormalised so the rows sum to one.
    let (po, pn) = (0.19982, 0.20004);
    let s = po + 4.0 * pn;
    let n = 6;
    let mut m = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            if i != j {
                m[i * n + j] = if i / 2 == j / 2 { po } else { pn } / s;
            }
        }
    }
    let area = 0.01;
    let vf = ViewFactors::from_matrix(vec![area; n], m);
    let eps = [0.9, 0.9, 0.3, 0.3, 0.6, 0.8];
    let t = [350.0, 300.0, 320.0, 310.0, 305.0, 295.0];
    let r = solve_with(&eps, &[area; 6], &vf, &t);
    let e: Vec<f64> = (0..n).map(|i| eps[i] * SIGMA * t[i].powi(4)).collect();
    let j = direct(&eps, &vf, &e);
    for i in 0..n {
        assert!((r.radiosity_w_m2[i] - j[i]).abs() < 1e-6 * j[i], "{i}");
    }
    assert!(
        r.imbalance_w.abs() < 1e-9 * r.exchanged_w.max(1.0),
        "{}",
        r.imbalance_w
    );
    // The hottest face loses heat, the coldest gains.
    assert!(r.net_flux_w_m2[0] > 0.0 && r.net_flux_w_m2[5] < 0.0);
    // Black body: net flux is the plain difference to the mean surroundings.
    let black = solve_with(&[1.0; 6], &[area; 6], &vf, &t);
    for i in 0..n {
        assert!((black.radiosity_w_m2[i] - SIGMA * t[i].powi(4)).abs() < 1e-9);
    }
}

#[test]
fn a_body_inside_an_enclosure_follows_the_two_surface_formula() {
    // 30 mm cube in a 300 mm cube: F(inner -> outer) = 1.
    let materials = vec![air(), solid(1, MaterialKind::Device(1))];
    let grid = Grid3::uniform([30, 30, 30], [300.0; 3]);
    let body = Region {
        material: 1,
        lo: [140.0; 3],
        hi: [170.0; 3],
    };
    // On grid planes: a cell counts as solid when most of it is, so a body
    // off the planes would come out smaller or larger.
    let mesh = Mesh::classify(grid, &[body], materials);
    let mut st = whole_faces();
    st.emissivity.device = 0.8;
    st.emissivity.enclosure_wall = 0.9;
    st.view_factors.rays_per_patch = 20_000;
    let model = RadiationModel::build(&mesh, &st).unwrap();
    assert_eq!(model.patches.len(), 12);
    let t: Vec<f64> = model
        .patches
        .patches
        .iter()
        .map(|p| {
            if p.kind == PatchKind::Solid {
                350.0
            } else {
                300.0
            }
        })
        .collect();
    let r = model.solve(&t);
    assert!(r.imbalance_w.abs() < 1e-9 * r.exchanged_w);
    let by = r.power_by_material(&model.patches);
    let q_body = by.iter().find(|e| e.0 == 1).unwrap().1;
    let q_wall = by.iter().find(|e| e.0 == 0).unwrap().1;
    assert!((q_body + q_wall).abs() < 1e-9 * q_body);
    let a1 = 6.0 * 0.03 * 0.03;
    let a2 = 6.0 * 0.3 * 0.3;
    let want = SIGMA * (350.0_f64.powi(4) - 300.0_f64.powi(4)) * a1
        / (1.0 / 0.8 + a1 / a2 * (1.0 / 0.9 - 1.0));
    // The textbook formula assumes uniform radiosity on the walls, which a
    // cube is not; for a small body the difference stays small.
    assert!((q_body - want).abs() < 0.02 * want, "{q_body} vs {want}");
}

#[test]
fn cabinet_scenario_balances_and_hot_devices_radiate_to_cool_surfaces() {
    use novo_layout::Rect;
    use novo_layout::sizing::{Installation, Material};
    use novo_thermal::*;
    let rect = |x, y, w, h| Rect { x, y, w, h };
    let dev = |id, r: Rect| Device {
        id,
        footprint: r,
        height_mm: Some(70.0),
        loss_w: 60.0,
        rated_max_ambient_c: None,
        rated_max_surface_c: None,
        heat_capacity_j_per_k: None,
    };
    let scn = Scenario {
        enclosure: Enclosure {
            width_mm: 600.0,
            height_mm: 800.0,
            depth_mm: 250.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: rect(30.0, 30.0, 530.0, 740.0),
        },
        ambient_c: 25.0,
        devices: vec![
            dev(1, rect(100.0, 100.0, 120.0, 160.0)),
            dev(2, rect(300.0, 400.0, 100.0, 120.0)),
        ],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    };
    let mesh = Mesh::build(
        &scn,
        &MeshSettings {
            target_cells: 20_000,
            ..MeshSettings::default()
        },
    )
    .unwrap();
    let st = RadSettings {
        view_factors: ViewFactorSettings {
            rays_per_patch: 1500,
            ..ViewFactorSettings::default()
        },
        ..RadSettings::default()
    };
    let model = RadiationModel::build(&mesh, &st).expect("model");
    assert!(model.view_factors.reciprocity_error() < 1e-9);
    assert!(model.view_factors.summation_error() < 1e-9);
    assert!(model.view_factors.stats.lost_rays == 0);
    let t: Vec<f64> = model
        .patches
        .patches
        .iter()
        .map(|p| match p.material {
            0 => 308.0,
            1 => 305.0,
            _ => 340.0,
        })
        .collect();
    let r = model.solve(&t);
    assert!(r.imbalance_w.abs() < 1e-9 * r.exchanged_w);
    let by = r.power_by_material(&model.patches);
    for (id, w) in by {
        match id {
            0 | 1 => assert!(w < 0.0, "material {id} should gain heat: {w}"),
            _ => assert!(w > 0.0, "device {id} should lose heat: {w}"),
        }
    }
    // Switched off: no model.
    let off = RadSettings {
        enabled: false,
        ..st
    };
    assert!(RadiationModel::build(&mesh, &off).is_none());
}

#[test]
fn heat_paths_add_up() {
    let p = HeatPaths::new(60.0, 9.0, 15.0);
    assert!((p.convection_w() - 36.0).abs() < 1e-12);
    let (cv, cd, rd) = p.shares();
    assert!((cv + cd + rd - 1.0).abs() < 1e-12);
    assert_eq!(HeatPaths::default().shares(), (0.0, 0.0, 0.0));
}

#[test]
fn faces_scatter_back_to_the_cells() {
    let mesh = empty_box([3, 3, 3], [90.0; 3]);
    let st = RadSettings {
        patches: PatchSettings {
            block: Some(1),
            ..PatchSettings::default()
        },
        ..RadSettings::default()
    };
    let model = RadiationModel::build(&mesh, &st).unwrap();
    // 6 walls x 9 faces, one patch each.
    assert_eq!(model.patches.len(), 54);
    let flux = vec![5.0; model.patches.len()];
    assert_eq!(model.patches.scatter(&flux).len(), 54);
    let t = model.patches.average(&mesh.grid, |_| 300.0);
    assert!(t.iter().all(|v| (v - 300.0).abs() < 1e-9));
}
