#![allow(dead_code)]
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

pub fn enclosure() -> Enclosure {
    Enclosure {
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
    }
}

pub fn device(id: u64, x: f64, y: f64, w: f64, h: f64, loss: f64) -> Device {
    Device {
        id,
        footprint: Rect { x, y, w, h },
        height_mm: Some(60.0),
        loss_w: loss,
        rated_max_ambient_c: Some(55.0),
        rated_max_surface_c: None,
        heat_capacity_j_per_k: None,
    }
}

pub fn scenario(devices: Vec<Device>) -> Scenario {
    Scenario {
        enclosure: enclosure(),
        ambient_c: 25.0,
        devices,
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

/// One central steady run on a coarse grid: what most checks need.
pub fn quick(mut sc: Scenario) -> ThermalResult {
    sc.params.cell_mm = sc.params.cell_mm.max(40.0);
    simulate_steady_with(
        &sc,
        Options {
            bands: false,
            advice: false,
        },
    )
    .unwrap()
}
