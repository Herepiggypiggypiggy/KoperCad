//! Runs a small control cabinet through the simulation and prints the
//! summary; with `--json` it prints the fields for plotting.
//!
//! `cargo run -p novo-thermal --release --example demo`

use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn device(id: u64, x: f64, y: f64, w: f64, h: f64, loss: f64, ambient: f64) -> Device {
    Device {
        id,
        footprint: Rect { x, y, w, h },
        height_mm: Some(70.0),
        loss_w: loss,
        rated_max_ambient_c: Some(ambient),
        rated_max_surface_c: None,
        heat_capacity_j_per_k: None,
    }
}

fn cabinet() -> Scenario {
    let mut drive = device(1, 150.0, 650.0, 140.0, 260.0, 95.0, 55.0);
    drive.height_mm = Some(200.0);
    drive.rated_max_surface_c = Some(90.0);
    Scenario {
        enclosure: Enclosure {
            width_mm: 800.0,
            height_mm: 1200.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 40.0,
                y: 40.0,
                w: 710.0,
                h: 1110.0,
            },
        },
        ambient_c: 35.0,
        devices: vec![
            drive,
            device(2, 150.0, 400.0, 120.0, 120.0, 30.0, 50.0), // PSU above the drive
            device(3, 450.0, 650.0, 90.0, 120.0, 12.0, 55.0),
            device(4, 450.0, 480.0, 90.0, 120.0, 8.0, 50.0),
            device(5, 600.0, 650.0, 60.0, 120.0, 5.0, 55.0),
            device(6, 450.0, 150.0, 200.0, 100.0, 3.0, 50.0),
        ],
        obstacles: vec![Obstacle {
            kind: ObstacleKind::Duct,
            rect: Rect {
                x: 330.0,
                y: 100.0,
                w: 60.0,
                h: 1000.0,
            },
            blockage: None,
        }],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn main() {
    let json = std::env::args().any(|a| a == "--json");
    let sc = cabinet();
    let t = std::time::Instant::now();
    let r = simulate_steady_with_advice(&sc).expect("valid scenario");
    if json {
        dump(&sc, &r);
        return;
    }
    println!("simulated in {:?} ({} iterations)", t.elapsed(), r.iterations);
    let b = &r.balance;
    println!(
        "loss {:.0} W: walls {:.0} W, mean air {:.1} C (IEC estimate {:.1} C), hottest air {:.1} C",
        b.loss_w,
        b.front_w + b.rear_w + b.left_w + b.right_w + b.top_w + b.bottom_w,
        b.mean_air_c,
        r.ambient_c + b.iec_rise_k,
        b.max_air_c
    );
    println!("energy residual {:.2e} of the loss", b.residual_rel);
    for d in &r.devices {
        println!(
            "device {}: surface {:.0} C ({:.0} to {:.0}), air {:.0} C, margin {:?} K, {:?}",
            d.id,
            d.surface_c,
            d.surface_band_c.0,
            d.surface_band_c.1,
            d.air_c,
            d.margin_k.map(|m| m.round()),
            d.status
        );
    }
    for h in &r.hot_spots {
        println!("hot spot {:?} at ({:.0}, {:.0}): {:.0} C, {}", h.kind, h.x_mm, h.y_mm, h.temp_c, h.reason);
    }
    for rec in &r.recommendations {
        println!("#{} {}", rec.rank, rec.text);
    }
    println!("{}", NOTE);
}

fn dump(sc: &Scenario, r: &ThermalResult) {
    let nums = |v: &[f64]| {
        v.iter()
            .map(|x| format!("{x:.4}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    let g = r.grid;
    println!("{{");
    println!(
        "\"cols\":{},\"rows\":{},\"w\":{},\"h\":{},\"ambient\":{},",
        g.cols, g.rows, g.width_mm, g.height_mm, r.ambient_c
    );
    println!("\"air\":[{}],", nums(&r.air_temp.values));
    println!("\"plate\":[{}],", nums(&r.plate_temp.values));
    println!("\"vx\":[{}],\"vy\":[{}],", nums(&r.velocity.vx), nums(&r.velocity.vy));
    println!(
        "\"pfx\":[{}],\"pfy\":[{}],",
        nums(&r.plate_flux.vx),
        nums(&r.plate_flux.vy)
    );
    let devs: Vec<String> = sc
        .devices
        .iter()
        .zip(&r.devices)
        .map(|(d, res)| {
            format!(
                "[{},{},{},{},{},{:.1},{}]",
                d.id,
                d.footprint.x,
                d.footprint.y,
                d.footprint.w,
                d.footprint.h,
                res.surface_c,
                res.status as u8
            )
        })
        .collect();
    println!("\"devices\":[{}],", devs.join(","));
    let obs: Vec<String> = sc
        .obstacles
        .iter()
        .map(|o| format!("[{},{},{},{}]", o.rect.x, o.rect.y, o.rect.w, o.rect.h))
        .collect();
    println!("\"obstacles\":[{}]", obs.join(","));
    println!("}}");
}
