//! Prints the grid and body resolution of the reference cabinet per budget.
//! `cargo run -p novo-cfd --example mesh_grid -- 1500 3000 6000`
use novo_cfd::mesh::*;
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn main() {
    let r = |x, y, w, h| Rect { x, y, w, h };
    let scn = Scenario {
        enclosure: Enclosure {
            width_mm: 600.0,
            height_mm: 800.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: r(40.0, 40.0, 510.0, 700.0),
        },
        ambient_c: 25.0,
        devices: vec![Device {
            id: 7,
            footprint: r(200.0, 450.0, 150.0, 150.0),
            height_mm: Some(100.0),
            loss_w: 50.0,
            rated_max_ambient_c: None,
            rated_max_surface_c: None,
            heat_capacity_j_per_k: None,
        }],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    };
    for n in std::env::args()
        .skip(1)
        .filter_map(|s| s.parse::<usize>().ok())
    {
        let st = MeshSettings {
            target_cells: n,
            ..Default::default()
        };
        let plan = plan_grid_report(&scn, &st).unwrap();
        println!(
            "plan: {} cells, {} cells across, over budget {}",
            plan.grid.cells(),
            plan.cells_across,
            plan.over_budget
        );
        let m = Mesh::build(&scn, &st).unwrap();
        let p = m.preview();
        println!(
            "{n}: {}  aspect {:.0} ratio {:.2}",
            p.summary(),
            p.max_aspect_ratio,
            p.max_grading_ratio
        );
        for b in &m.bodies {
            println!(
                "   {} across {:?} resolved {}",
                b.name,
                b.cells_across,
                b.is_resolved()
            );
        }
        let g = &m.grid;
        let w = |e: &[f64]| {
            e.windows(2)
                .map(|w| format!("{:.1}", w[1] - w[0]))
                .collect::<Vec<_>>()
                .join(" ")
        };
        println!("   x: {}", w(&g.x_edges_mm));
        println!("   y: {}", w(&g.y_edges_mm));
        let w = |e: &[f64]| {
            e.windows(2)
                .map(|w| format!("{:.1}", w[1] - w[0]))
                .collect::<Vec<_>>()
                .join(" ")
        };
        println!("   z: {}", w(&g.z_edges_mm));
    }
}
