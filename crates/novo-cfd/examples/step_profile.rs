use novo_cfd::mesh::MeshSettings;
use novo_cfd::solids::{FlowSetup, SolidsSettings};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;
fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}
fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000);
    let dev = |id, rect, loss| Device {
        id,
        footprint: rect,
        height_mm: Some(100.0),
        loss_w: loss,
        rated_max_ambient_c: None,
        rated_max_surface_c: None,
        heat_capacity_j_per_k: None,
    };
    let scn = Scenario {
        enclosure: Enclosure {
            width_mm: 800.0,
            height_mm: 1200.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: r(50.0, 50.0, 690.0, 1090.0),
        },
        ambient_c: 25.0,
        devices: vec![
            dev(1, r(200.0, 300.0, 150.0, 200.0), 60.0),
            dev(2, r(400.0, 700.0, 120.0, 150.0), 40.0),
        ],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    };
    let st = SolidsSettings {
        mesh: MeshSettings {
            target_cells: n,
            ..MeshSettings::default()
        },
        ..SolidsSettings::default()
    };
    let t0 = std::time::Instant::now();
    let mut setup = FlowSetup::new(&scn, &st).unwrap();
    if let Some(t) = std::env::args().nth(2).and_then(|s| s.parse().ok()) {
        setup.cfg.pressure_tol = t;
    }
    println!(
        "setup {:.2}s cells {} threads {}",
        t0.elapsed().as_secs_f64(),
        setup.mesh.cells(),
        rayon::current_num_threads()
    );
    let t0 = std::time::Instant::now();
    let mut s = setup.solver();
    println!("solver new {:.2}s", t0.elapsed().as_secs_f64());
    for i in 0..60 {
        let dt = s.max_dt(0.4).clamp(1e-4, 5.0);
        let t0 = std::time::Instant::now();
        let info = s.step(dt);
        if i % 6 == 0 {
            println!(
                "step {i} dt {dt:.4} t {:.2} pit {} {:.3}s",
                s.time,
                info.pressure_iterations,
                t0.elapsed().as_secs_f64()
            );
        }
    }
}
