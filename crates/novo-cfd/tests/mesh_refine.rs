//! Refinement around bodies with a cell budget (lane CFD-C1, round 2).
use novo_cfd::mesh::*;
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

/// The sealed reference cabinet of the cabinet tests: one 150 x 150 x 100 mm
/// device on a 2.5 mm plate in 600 x 800 x 300.
fn reference() -> Scenario {
    Scenario {
        enclosure: Enclosure {
            width_mm: 600.0,
            height_mm: 800.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: rect(40.0, 40.0, 510.0, 700.0),
        },
        ambient_c: 25.0,
        devices: vec![device(7, rect(200.0, 450.0, 150.0, 150.0), 100.0)],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn st(n: usize) -> MeshSettings {
    MeshSettings {
        target_cells: n,
        ..MeshSettings::default()
    }
}

#[test]
fn the_same_budget_resolves_the_device_at_every_level() {
    for n in [3000, 6000, 20_000, 50_000] {
        let plan = plan_grid_report(&reference(), &st(n)).unwrap();
        let ratio = plan.grid.cells() as f64 / n as f64;
        assert!(
            (0.9..=1.1).contains(&ratio),
            "{n}: {} cells",
            plan.grid.cells()
        );
        assert_eq!(plan.cells_across, 3, "{n}");
        assert!(!plan.over_budget);
        let m = Mesh::build(&reference(), &st(n)).unwrap();
        let dev = m.bodies.iter().find(|b| b.name == "Device 7").unwrap();
        assert!(dev.cells_across.iter().all(|c| *c >= 3), "{n}: {dev:?}");
        assert!(m.preview().unresolved_bodies.unwrap().is_empty(), "{n}");
        // the device is meshed on its own faces: no cut cells around it
        for v in [200.0, 350.0] {
            assert!(m.grid.x_edges_mm.iter().any(|e| (e - v).abs() < 1e-9));
        }
        for v in [450.0, 600.0] {
            assert!(m.grid.y_edges_mm.iter().any(|e| (e - v).abs() < 1e-9));
        }
    }
}

#[test]
fn empty_air_is_coarse_and_the_neighbour_ratio_is_bounded() {
    let s = st(6000);
    let m = Mesh::build(&reference(), &s).unwrap();
    let p = m.preview();
    assert!(
        p.max_grading_ratio <= s.max_ratio + 1e-9,
        "{}",
        p.max_grading_ratio
    );
    // coarser away from the device than inside it
    let dx = |g: &novo_cfd::Grid3, i: usize| g.x_edges_mm[i + 1] - g.x_edges_mm[i];
    let g = &m.grid;
    let inside = (0..g.nx)
        .filter(|&i| g.x_edges_mm[i] >= 200.0 && g.x_edges_mm[i + 1] <= 350.0)
        .map(|i| dx(g, i))
        .fold(f64::MAX, f64::min);
    let far = (0..g.nx).map(|i| dx(g, i)).fold(0.0, f64::max);
    assert!(far > inside, "{far} vs {inside}");
    assert!(p.max_aspect_ratio < 8.0, "aspect {}", p.max_aspect_ratio);
}

#[test]
fn volumes_stay_exact_on_the_refined_grid() {
    let scn = reference();
    let m = Mesh::build(&scn, &st(6000)).unwrap();
    let [w, h, d] = domain_mm(&scn);
    let rel = (m.total_volume_m3() - w * h * d * 1e-9).abs() / (w * h * d * 1e-9);
    assert!(rel < 1e-9, "{rel}");
    // the device is aligned, so its volume is its box exactly
    let dev = 150.0 * 150.0 * 100.0 * 1e-9;
    assert!((m.material_volume_m3[2] - dev).abs() < dev * 1e-9);
    // the thin plate is in cut cells and keeps its volume share
    let plate = 510.0 * 700.0 * 2.5 * 1e-9;
    assert!((m.material_volume_m3[1] - plate).abs() < plate * 1e-9);
}

#[test]
fn when_the_bodies_do_not_fit_the_requirement_is_lowered_and_flagged() {
    let mut scn = reference();
    scn.devices = (0..12)
        .map(|i| {
            let (c, r) = (i % 4, i / 4);
            device(
                i as u64 + 1,
                rect(
                    30.0 + 130.0 * c as f64,
                    40.0 + 220.0 * r as f64,
                    90.0,
                    160.0,
                ),
                70.0,
            )
        })
        .collect();
    let tight = plan_grid_report(&scn, &st(250)).unwrap();
    assert!(tight.cells_across < 3, "{}", tight.cells_across);
    let roomy = plan_grid_report(&scn, &st(30_000)).unwrap();
    assert_eq!(roomy.cells_across, 3);
    assert!(!roomy.over_budget);
    let m = Mesh::build(&scn, &st(30_000)).unwrap();
    assert!(m.preview().unresolved_bodies.unwrap().is_empty());
    // a hopeless budget is flagged, not silently exceeded
    let hopeless = plan_grid_report(&scn, &st(60)).unwrap();
    assert!(hopeless.over_budget);
    assert!(hopeless.grid.is_valid());
}

#[test]
fn thin_sheets_are_cut_cells_unless_asked_for() {
    let scn = reference();
    let off = Mesh::build(&scn, &st(6000)).unwrap();
    assert!(!off.grid.z_edges_mm.iter().any(|e| (e - 2.5).abs() < 1e-9));
    let on = Mesh::build(
        &scn,
        &MeshSettings {
            resolve_thin: true,
            ..st(6000)
        },
    )
    .unwrap();
    assert!(on.grid.z_edges_mm.iter().any(|e| (e - 2.5).abs() < 1e-9));
    let plate = on
        .bodies
        .iter()
        .find(|b| b.name == "Mounting plate")
        .unwrap();
    assert_eq!(plate.cells_across[2], 1);
    assert!(on.preview().max_aspect_ratio > off.preview().max_aspect_ratio);
}

#[test]
fn the_plate_can_be_left_out_for_a_sheet_model() {
    let scn = reference();
    let m = Mesh::build(
        &scn,
        &MeshSettings {
            plate_slab: false,
            ..st(6000)
        },
    )
    .unwrap();
    assert_eq!(m.material_volume_m3[1], 0.0);
    assert!(m.bodies.iter().all(|b| b.name != "Mounting plate"));
    assert!(
        (m.total_volume_m3() - {
            let [w, h, d] = domain_mm(&scn);
            w * h * d * 1e-9
        })
        .abs()
            < 1e-12
    );
}

#[test]
fn vents_get_cells_across_their_opening() {
    let mut scn = reference();
    scn.vents.push(Vent {
        opening: Opening {
            placement: Placement::Top { x_mm: 450.0 },
            width_mm: 120.0,
            height_mm: 60.0,
        },
        free_area_fraction: 0.6,
    });
    let g = plan_grid(&scn, &st(3000)).unwrap();
    let inside = g
        .x_edges_mm
        .windows(2)
        .filter(|w| w[0] >= 390.0 - 1e-9 && w[1] <= 510.0 + 1e-9)
        .count();
    assert!(inside >= 2, "{inside}");
    for v in [390.0, 510.0] {
        assert!(g.x_edges_mm.iter().any(|e| (e - v).abs() < 1e-9), "{v}");
    }
}

#[test]
fn ducts_and_rails_are_resolved_too_and_the_build_is_deterministic() {
    let mut scn = reference();
    scn.obstacles = vec![
        Obstacle {
            kind: ObstacleKind::Duct,
            rect: rect(100.0, 100.0, 40.0, 500.0),
            blockage: None,
        },
        Obstacle {
            kind: ObstacleKind::Rail,
            rect: rect(160.0, 640.0, 300.0, 35.0),
            blockage: None,
        },
    ];
    let a = Mesh::build(&scn, &st(12_000)).unwrap();
    let b = Mesh::build(&scn, &st(12_000)).unwrap();
    assert_eq!(a, b);
    for body in a.bodies.iter().filter(|b| b.name != "Mounting plate") {
        assert!(body.is_resolved(), "{body:?}");
    }
}
