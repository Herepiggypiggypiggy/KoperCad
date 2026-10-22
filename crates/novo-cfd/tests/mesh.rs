use novo_cfd::mesh::*;
use novo_cfd::{Fidelity, Grid3, MaterialKind};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}

fn device(id: u64, r: Rect, height: f64) -> Device {
    Device {
        id,
        footprint: r,
        height_mm: Some(height),
        loss_w: 50.0,
        rated_max_ambient_c: None,
        rated_max_surface_c: None,
        heat_capacity_j_per_k: None,
    }
}

fn scenario() -> Scenario {
    Scenario {
        enclosure: Enclosure {
            width_mm: 800.0,
            height_mm: 1200.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: rect(50.0, 50.0, 690.0, 1090.0),
        },
        ambient_c: 25.0,
        devices: vec![
            device(7, rect(200.0, 300.0, 120.0, 200.0), 60.0),
            device(9, rect(401.3, 622.7, 83.1, 57.9), 74.4),
        ],
        obstacles: vec![
            Obstacle {
                kind: ObstacleKind::Duct,
                rect: rect(100.0, 200.0, 30.0, 700.0),
                blockage: None,
                loss_w: 0.0,
            },
            // Overlaps device 7 on purpose.
            Obstacle {
                kind: ObstacleKind::Rail,
                rect: rect(180.0, 380.0, 200.0, 35.0),
                blockage: None,
                loss_w: 0.0,
            },
        ],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn settings(n: usize) -> MeshSettings {
    MeshSettings {
        target_cells: n,
        ..MeshSettings::default()
    }
}

/// Analytic union volume of the regions, mm3 per material (priority wins).
fn expected_mm3(regs: &[Region], mat: u16) -> f64 {
    // Sample-free: inclusion by exact slab decomposition on all face planes.
    let cuts: [Vec<f64>; 3] = std::array::from_fn(|a| {
        let mut c: Vec<f64> = regs.iter().flat_map(|r| [r.lo[a], r.hi[a]]).collect();
        c.sort_by(|x, y| x.total_cmp(y));
        c.dedup();
        c
    });
    let mut v = 0.0;
    for z in cuts[2].windows(2) {
        for y in cuts[1].windows(2) {
            for x in cuts[0].windows(2) {
                let c = [
                    0.5 * (x[0] + x[1]),
                    0.5 * (y[0] + y[1]),
                    0.5 * (z[0] + z[1]),
                ];
                let owner = regs
                    .iter()
                    .find(|r| (0..3).all(|a| c[a] > r.lo[a] && c[a] < r.hi[a]));
                if owner.is_some_and(|r| r.material == mat) {
                    v += (x[1] - x[0]) * (y[1] - y[0]) * (z[1] - z[0]);
                }
            }
        }
    }
    v
}

#[test]
fn volumes_are_conserved_and_match_the_geometry() {
    let scn = scenario();
    let mesh = Mesh::build(&scn, &settings(50_000)).unwrap();
    let [w, h, d] = domain_mm(&scn);
    let domain = w * h * d * 1e-9;
    let rel = (mesh.total_volume_m3() - domain).abs() / domain;
    assert!(rel < 1e-9, "total volume off by {rel}");

    let (regs, mats) = regions(&scn);
    for m in mats.iter().filter(|m| m.id != 0) {
        let want = expected_mm3(&regs, m.id) * 1e-9;
        let got = mesh.material_volume_m3[m.id as usize];
        let tol = 1e-9 * want.max(1e-9);
        assert!(
            (got - want).abs() <= tol + 1e-15,
            "{}: {got} vs {want}",
            m.name
        );
    }
}

#[test]
fn fractions_are_in_range_and_shares_add_up() {
    // A uniform grid does not line up with the bodies: cut cells.
    let (regs, mats) = regions(&scenario());
    let mesh = Mesh::classify(
        Grid3::uniform([40, 60, 20], [796.0, 1196.0, 296.0]),
        &regs,
        mats,
    );
    assert!(mesh.fluid_fraction.iter().all(|f| (0.0..=1.0).contains(f)));
    assert!(!mesh.mixed.is_empty());
    for c in &mesh.mixed {
        let sum: f64 = c.parts.iter().map(|p| p.1).sum();
        assert!((sum - 1.0).abs() < 1e-12);
        assert!(c.parts.iter().all(|p| p.1 > 0.0 && p.1 <= 1.0));
        let air = c.parts.iter().find(|p| p.0 == 0).map_or(0.0, |p| p.1);
        assert!((air - mesh.fluid_fraction[c.cell]).abs() < 1e-12);
    }
    let (f, s, c) = mesh.class_counts();
    assert_eq!(f + s + c, mesh.grid.cells());
    assert!(s > 0 && c > 0);
}

#[test]
fn deterministic() {
    let a = Mesh::build(&scenario(), &settings(50_000)).unwrap();
    let b = Mesh::build(&scenario(), &settings(50_000)).unwrap();
    assert_eq!(a, b);
}

#[test]
fn grading_is_bounded_and_edges_valid() {
    let st = settings(50_000);
    let mesh = Mesh::build(&scenario(), &st).unwrap();
    assert!(mesh.grid.is_valid());
    let p = mesh.preview();
    assert!(
        p.max_grading_ratio <= st.max_ratio + 1e-9,
        "{}",
        p.max_grading_ratio
    );
    assert!(p.min_size_mm < p.max_size_mm);
    // Cells are fine at the wall and grow away from it.
    let x = &mesh.grid.x_edges_mm;
    let first = x[1] - x[0];
    let widest = x.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max);
    assert!(first < widest, "{first} !< {widest}");
}

#[test]
fn device_faces_lie_on_grid_planes() {
    let scn = scenario();
    let mesh = Mesh::build(&scn, &settings(50_000)).unwrap();
    for v in [200.0, 320.0] {
        assert!(mesh.grid.x_edges_mm.iter().any(|e| (e - v).abs() < 1e-9));
    }
    for v in [300.0, 500.0] {
        assert!(mesh.grid.y_edges_mm.iter().any(|e| (e - v).abs() < 1e-9));
    }
}

#[test]
fn presets_hit_their_cell_counts() {
    let scn = scenario();
    for f in [Fidelity::Coarse, Fidelity::Medium, Fidelity::Fine] {
        let st = MeshSettings::for_fidelity(f).unwrap();
        let g = plan_grid(&scn, &st).unwrap();
        let ratio = g.cells() as f64 / st.target_cells as f64;
        assert!((0.9..=1.1).contains(&ratio), "{f:?}: {} cells", g.cells());
        assert!(g.is_valid());
    }
    assert_eq!(
        MeshSettings::for_fidelity(Fidelity::Preview2d),
        Err(MeshError::NoMeshForFidelity(Fidelity::Preview2d))
    );
}

#[test]
fn aligned_box_is_exact_and_pure() {
    // A 2x2x2 uniform cell box standing exactly on cell planes.
    let grid = Grid3::uniform([10, 10, 10], [100.0, 100.0, 100.0]);
    let regs = [Region {
        material: 2,
        lo: [20.0, 20.0, 20.0],
        hi: [40.0, 40.0, 40.0],
    }];
    let mats = regions(&scenario()).1;
    let m = Mesh::classify(grid, &regs, mats);
    assert_eq!(m.class_counts(), (992, 8, 0));
    assert!((m.material_volume_m3[2] - 8e-6).abs() < 1e-18);
}

#[test]
fn unaligned_box_makes_cut_cells_with_exact_shares() {
    let grid = Grid3::uniform([10, 10, 10], [100.0, 100.0, 100.0]);
    let regs = [Region {
        material: 2,
        lo: [15.0, 20.0, 20.0],
        hi: [35.0, 30.0, 30.0],
    }];
    let m = Mesh::classify(grid, &regs, regions(&scenario()).1);
    // x: cells 1 (half), 2 (full), 3 (half) -> 2 full-y/z cells... one cell
    // thick in y and z: 1 solid, 2 cut.
    assert_eq!(m.class_counts().1, 1);
    assert_eq!(m.class_counts().2, 2);
    for c in &m.mixed {
        assert_eq!(c.parts, vec![(0, 0.5), (2, 0.5)]);
    }
    assert!((m.material_volume_m3[2] - 2000.0 * 1e-9).abs() < 1e-18);
}

#[test]
fn priority_decides_overlaps() {
    let grid = Grid3::uniform([4, 4, 4], [40.0, 40.0, 40.0]);
    let a = Region {
        material: 2,
        lo: [0.0; 3],
        hi: [20.0, 40.0, 40.0],
    };
    let b = Region {
        material: 3,
        lo: [10.0, 0.0, 0.0],
        hi: [30.0, 40.0, 40.0],
    };
    let mats = regions(&scenario()).1;
    let m = Mesh::classify(grid, &[a, b], mats);
    // a keeps 0..20, b gets only 20..30.
    assert!((m.material_volume_m3[2] - 20.0 * 40.0 * 40.0 * 1e-9).abs() < 1e-18);
    assert!((m.material_volume_m3[3] - 10.0 * 40.0 * 40.0 * 1e-9).abs() < 1e-18);
    assert!((m.total_volume_m3() - 40.0_f64.powi(3) * 1e-9).abs() < 1e-15);
}

#[test]
fn material_table_and_kinds() {
    let (_, mats) = regions(&scenario());
    assert_eq!(mats[0].kind, MaterialKind::Air);
    assert_eq!(mats[1].kind, MaterialKind::Plate);
    assert!(mats.iter().any(|m| m.kind == MaterialKind::Device(9)));
    assert!(mats.iter().any(|m| m.kind == MaterialKind::Duct));
    assert!(mats.iter().any(|m| m.kind == MaterialKind::Rail));
}

#[test]
fn locate_and_preview() {
    let mesh = Mesh::build(&scenario(), &settings(50_000)).unwrap();
    let [i, j, k] = locate(&mesh.grid, [250.0, 400.0, 30.0]).unwrap();
    assert!(mesh.grid.x_edges_mm[i] <= 250.0 && 250.0 <= mesh.grid.x_edges_mm[i + 1]);
    assert!(mesh.grid.y_edges_mm[j] <= 400.0 && 400.0 <= mesh.grid.y_edges_mm[j + 1]);
    assert!(mesh.grid.z_edges_mm[k] <= 30.0 && 30.0 <= mesh.grid.z_edges_mm[k + 1]);
    assert_eq!(locate(&mesh.grid, [-1.0, 0.0, 0.0]), None);
    // A point inside device 7 is solid.
    let idx = mesh.grid.index(i, j, k);
    assert_eq!(mesh.material[idx], 2, "device 7 expected");
    let p = mesh.preview();
    assert_eq!(p.cells, mesh.grid.cells());
    assert!(p.summary().contains("cells"));
    // Quick preview without classification agrees on the grid.
    let q = MeshPreview::from_grid(&plan_grid(&scenario(), &settings(50_000)).unwrap());
    assert_eq!(q.cells, p.cells);
    assert!(q.class_counts.is_none() && p.class_counts.is_some());
}

#[test]
fn openings_add_planes_and_bad_input_is_rejected() {
    let mut scn = scenario();
    scn.vents.push(Vent {
        opening: Opening {
            placement: Placement::Top { x_mm: 600.0 },
            width_mm: 100.0,
            height_mm: 100.0,
        },
        free_area_fraction: 0.6,
    });
    let g = plan_grid(&scn, &settings(50_000)).unwrap();
    for v in [550.0, 650.0] {
        assert!(g.x_edges_mm.iter().any(|e| (e - v).abs() < 1e-9), "{v}");
    }
    assert!(matches!(
        plan_grid(&scn, &settings(2)),
        Err(MeshError::Settings(_))
    ));
    scn.enclosure.wall_mm = 700.0;
    assert_eq!(
        plan_grid(&scn, &settings(50_000)),
        Err(MeshError::EmptyDomain)
    );
}
