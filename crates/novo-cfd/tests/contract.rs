use novo_cfd::*;
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn scenario() -> Scenario {
    Scenario {
        enclosure: Enclosure {
            width_mm: 800.0,
            height_mm: 1200.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 50.0,
                y: 50.0,
                w: 690.0,
                h: 1090.0,
            },
        },
        ambient_c: 25.0,
        devices: vec![Device {
            id: 7,
            footprint: Rect {
                x: 200.0,
                y: 300.0,
                w: 120.0,
                h: 200.0,
            },
            height_mm: Some(60.0),
            loss_w: 80.0,
            rated_max_ambient_c: Some(55.0),
            rated_max_surface_c: None,
            heat_capacity_j_per_k: None,
        }],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

#[test]
fn uniform_grid_geometry() {
    let g = Grid3::uniform([4, 5, 2], [400.0, 500.0, 100.0]);
    assert!(g.is_valid());
    assert_eq!(g.cells(), 40);
    assert_eq!(g.index(3, 4, 1), 39);
    assert_eq!(g.centre_mm(0, 0, 0), [50.0, 50.0, 25.0]);
    let v: f64 = (0..2)
        .flat_map(|k| (0..5).flat_map(move |j| (0..4).map(move |i| (i, j, k))))
        .map(|(i, j, k)| g.volume_m3(i, j, k))
        .sum();
    assert!((v - 0.4 * 0.5 * 0.1).abs() < 1e-12);
}

#[test]
fn invalid_grids_are_detected() {
    let mut g = Grid3::uniform([2, 2, 2], [1.0, 1.0, 1.0]);
    g.y_edges_mm[1] = g.y_edges_mm[0];
    assert!(!g.is_valid());
}

#[test]
fn fidelity_levels_are_ordered() {
    assert!(Fidelity::Coarse < Fidelity::Medium && Fidelity::Medium < Fidelity::Fine);
    assert!(Fidelity::Coarse.target_cells() < Fidelity::Medium.target_cells());
}

#[test]
fn stub_publishes_a_consistent_snapshot() {
    let mut seen = 0;
    let snap = StubSolver
        .solve(&scenario(), Fidelity::Coarse, &Control::new(), &mut |s| {
            assert!(s.is_consistent());
            seen += 1;
        })
        .unwrap();
    assert_eq!(seen, 1);
    assert!(snap.is_consistent());
    assert_eq!(snap.devices.len(), 1);
    assert!(snap.max_temperature_c() > 25.0);
    assert!(!snap.notes.is_empty());
}

#[test]
fn cancel_and_unsupported_levels() {
    let ctl = Control::new();
    let clone = ctl.clone();
    clone.cancel();
    assert!(ctl.is_cancelled());
    let r = StubSolver.solve(&scenario(), Fidelity::Coarse, &ctl, &mut |_| {});
    assert!(matches!(r, Err(CfdError::Cancelled)));
    let r = StubSolver.solve(&scenario(), Fidelity::Fine, &Control::new(), &mut |_| {});
    assert!(matches!(r, Err(CfdError::Unsupported(Fidelity::Fine))));
}
