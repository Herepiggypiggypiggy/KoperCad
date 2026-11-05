//! Developer-side reference runs for the OpenFOAM cross-check (research
//! `research/openfoam/SUMMARY.md`, rows 1 and 2). Ignored by default.
//!
//! It runs the cabinet solver on a small set of plain box cases whose
//! boundaries are unambiguous (fixed wall temperature, or a resolved wall
//! film `Convective { h }`, never the IEC `Overall` coefficient), with no
//! radiation, no mounting plate and laminar flow, and writes
//!
//! - `cases.json`: the case definitions (single source for the OpenFOAM
//!   case writer in `scripts/openfoam/make_cases.py`),
//! - `ours.json`: one result row per case and mesh.
//!
//! ```text
//! OPENFOAM_REF_OUT=/tmp/ofref cargo test -p novo-cfd --release \
//!     --test openfoam_reference -- --ignored --nocapture
//! ```
//!
//! Optional `OPENFOAM_REF_CASES=sealed_fixed,vented` picks cases and
//! `OPENFOAM_REF_CELLS=9000,30000` the target cell counts.
//!
//! This is model against model: a second solver on the same geometry shows
//! whether the discretisation and boundary treatment agree, not whether a
//! real cabinet behaves this way. `ENCLOSURE_VALIDATED` does not depend on it.

use novo_cfd::Control;
use novo_cfd::flow::{FaceBc, ThermalBc};
use novo_cfd::mesh::MeshSettings;
use novo_cfd::solids::{FlowSetup, SolidsSettings};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;
use std::fmt::Write as _;

const WALL_MM: f64 = 2.0;
const AMBIENT_C: f64 = 25.0;
/// Devices start this far from the rear wall, mm.
const LIFT_MM: f64 = 50.0;

#[derive(Clone, Copy)]
enum Wall {
    Fixed,
    Film(f64),
}

struct Dev {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    depth: f64,
    loss_w: f64,
}

/// `face`: "top" | "bottom" (y faces, centred in depth), centre x, width,
/// depth extent (centred in z).
struct VentSpec {
    top: bool,
    x: f64,
    width: f64,
    depth: f64,
}

struct Case {
    name: &'static str,
    dom: [f64; 3],
    wall: Wall,
    devices: Vec<Dev>,
    vents: Vec<VentSpec>,
}

fn cases() -> Vec<Case> {
    let dom = [600.0, 800.0, 300.0];
    let dev = |x, y, w, h, loss_w| Dev {
        x,
        y,
        w,
        h,
        depth: 100.0,
        loss_w,
    };
    let vents = || {
        vec![
            VentSpec {
                top: false,
                x: 300.0,
                width: 200.0,
                depth: 100.0,
            },
            VentSpec {
                top: true,
                x: 300.0,
                width: 200.0,
                depth: 100.0,
            },
        ]
    };
    vec![
        Case {
            name: "sealed_fixed",
            dom,
            wall: Wall::Fixed,
            devices: vec![dev(200.0, 450.0, 150.0, 150.0, 50.0)],
            vents: vec![],
        },
        Case {
            name: "sealed_film",
            dom,
            wall: Wall::Film(8.0),
            devices: vec![dev(200.0, 450.0, 150.0, 150.0, 50.0)],
            vents: vec![],
        },
        Case {
            name: "vented",
            dom,
            wall: Wall::Film(8.0),
            devices: vec![dev(200.0, 450.0, 150.0, 150.0, 100.0)],
            vents: vents(),
        },
        Case {
            name: "three_devices",
            dom,
            wall: Wall::Film(8.0),
            devices: vec![
                dev(100.0, 450.0, 150.0, 150.0, 40.0),
                dev(350.0, 300.0, 150.0, 150.0, 60.0),
                dev(100.0, 100.0, 150.0, 100.0, 20.0),
            ],
            vents: vents(),
        },
    ]
}

/// Probe line: x 450 mm, z 150 mm, heights from the top, mm.
const PROBE_X: f64 = 450.0;
const PROBE_Z: f64 = 150.0;
fn probe_ys() -> Vec<f64> {
    (0..16).map(|i| 25.0 + 50.0 * i as f64).collect()
}

fn scenario(c: &Case) -> Scenario {
    let e = Enclosure {
        width_mm: c.dom[0] + 2.0 * WALL_MM,
        height_mm: c.dom[1] + 2.0 * WALL_MM,
        depth_mm: c.dom[2] + 2.0 * WALL_MM,
        wall_mm: WALL_MM,
        material: Material::Steel,
        installation: Installation::Wall,
        // no plate: devices stand free, lifted off the rear wall by `LIFT_MM`
        // so every device face touches air (as in the OpenFOAM case)
        plate: Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        },
    };
    let vent = |v: &VentSpec| Vent {
        opening: Opening {
            placement: if v.top {
                Placement::Top { x_mm: v.x }
            } else {
                Placement::Bottom { x_mm: v.x }
            },
            width_mm: v.width,
            height_mm: v.depth,
        },
        free_area_fraction: 0.6,
    };
    Scenario {
        enclosure: e,
        ambient_c: AMBIENT_C,
        devices: c
            .devices
            .iter()
            .enumerate()
            .map(|(i, d)| Device {
                id: i as u64 + 1,
                footprint: Rect {
                    x: d.x,
                    y: d.y,
                    w: d.w,
                    h: d.h,
                },
                height_mm: Some(d.depth),
                loss_w: d.loss_w,
                rated_max_ambient_c: None,
                rated_max_surface_c: None,
                heat_capacity_j_per_k: None,
            })
            .collect(),
        obstacles: vec![],
        vents: c.vents.iter().map(vent).collect(),
        fans: vec![],
        params: Params {
            plate_thickness_mm: LIFT_MM,
            ..Params::default()
        },
    }
}

fn cases_json(all: &[Case]) -> String {
    let mut s = String::from("{\n  \"ambient_c\": 25.0,\n  \"beta\": 0.003354,\n  \"nu\": 1.55e-5,\n  \"alpha\": 2.2e-5,\n  \"conductivity\": 0.026,\n  \"rho\": 1.18,\n  \"vent_free_area\": 0.6,\n  \"discharge_coefficient\": 0.6,\n  \"cases\": [\n");
    for (n, c) in all.iter().enumerate() {
        let wall = match c.wall {
            Wall::Fixed => "{\"kind\": \"fixed\", \"t_c\": 25.0}".to_string(),
            Wall::Film(h) => format!("{{\"kind\": \"film\", \"h\": {h}, \"t_out_c\": 25.0}}"),
        };
        let devs: Vec<String> = c
            .devices
            .iter()
            .map(|d| {
                format!(
                    "{{\"x\": {}, \"y\": {}, \"w\": {}, \"h\": {}, \"z0\": {}, \"depth\": {}, \"loss_w\": {}}}",
                    d.x, d.y, d.w, d.h, LIFT_MM, d.depth, d.loss_w
                )
            })
            .collect();
        let vents: Vec<String> = c
            .vents
            .iter()
            .map(|v| {
                format!(
                    "{{\"face\": \"{}\", \"x\": {}, \"width\": {}, \"depth\": {}}}",
                    if v.top { "top" } else { "bottom" },
                    v.x,
                    v.width,
                    v.depth
                )
            })
            .collect();
        let probes: Vec<String> = probe_ys()
            .iter()
            .map(|y| format!("[{PROBE_X}, {y}, {PROBE_Z}]"))
            .collect();
        let _ = write!(
            s,
            "    {{\"name\": \"{}\", \"domain_mm\": [{}, {}, {}], \"wall\": {wall}, \"devices\": [{}], \"vents\": [{}], \"probes_mm\": [{}]}}{}\n",
            c.name,
            c.dom[0],
            c.dom[1],
            c.dom[2],
            devs.join(", "),
            vents.join(", "),
            probes.join(", "),
            if n + 1 < all.len() { "," } else { "" }
        );
    }
    s.push_str("  ]\n}\n");
    s
}

fn locate(edges: &[f64], p: f64) -> usize {
    let n = edges.len() - 1;
    (0..n)
        .find(|&i| p >= edges[i] && p < edges[i + 1])
        .unwrap_or(n - 1)
}

fn run(c: &Case, cells: usize) -> String {
    let scn = scenario(c);
    let st = SolidsSettings {
        mesh: MeshSettings {
            target_cells: cells,
            ..MeshSettings::default()
        },
        ..SolidsSettings::default()
    };
    let mut setup = FlowSetup::new(&scn, &st).unwrap();
    for bc in setup.cfg.faces.iter_mut() {
        *bc = FaceBc {
            thermal: match c.wall {
                Wall::Fixed => ThermalBc::Fixed(AMBIENT_C),
                Wall::Film(h) => ThermalBc::Convective {
                    h,
                    t_out: AMBIENT_C,
                },
            },
            ..FaceBc::WALL
        };
    }
    setup.cfg.radiation = None;
    let mut s = setup.solver();
    let t0 = std::time::Instant::now();
    let rep = setup.run_steady(
        &mut s,
        0.001,
        (6000.0, 120000),
        &Control::new(),
        |_, _| {},
        0,
    );
    let secs = t0.elapsed().as_secs_f64();
    let med = setup.cfg.medium.as_ref().unwrap();
    let m = &s.mesh;
    let (mut sum, mut vol, mut tmax) = (0.0, 0.0, f64::MIN);
    for k in 0..m.n[2] {
        for j in 0..m.n[1] {
            for i in 0..m.n[0] {
                let id = m.cell([i, j, k]);
                if med.fluid_frac[id] > 0.99 {
                    let v = m.volume([i, j, k]);
                    sum += v * s.t[id];
                    vol += v;
                    tmax = tmax.max(s.t[id]);
                }
            }
        }
    }
    let walls: Vec<String> = (0..6).map(|f| format!("{:.4}", s.wall_heat_w(f))).collect();
    // vent flow (m3/s, positive = out) and exhaust temperature
    let mut flows = Vec::new();
    let mut exhaust = Vec::new();
    for p in 0..c.vents.len() {
        flows.push(format!("{:.6}", s.patch_flow_m3s(p)));
        let (mut qt, mut q) = (0.0, 0.0);
        for f in s.open.faces.iter().filter(|f| f.patch == p) {
            let u = f.outward() * s.vel[f.axis][f.vel] * f.area;
            if u > 0.0 {
                qt += u * s.t[f.cell];
                q += u;
            }
        }
        exhaust.push(if q > 0.0 {
            format!("{:.4}", qt / q)
        } else {
            "null".to_string()
        });
    }
    let probes: Vec<String> = probe_ys()
        .iter()
        .map(|y| {
            let i = locate(&m.edges[0], PROBE_X * 1e-3);
            let j = locate(&m.edges[1], y * 1e-3);
            let k = locate(&m.edges[2], PROBE_Z * 1e-3);
            format!("{:.4}", s.t[m.cell([i, j, k])])
        })
        .collect();
    let row = format!(
        "{{\"case\": \"{}\", \"target_cells\": {cells}, \"cells\": {}, \"converged\": {}, \"imbalance\": {:.5}, \"seconds\": {secs:.1}, \"mean_air_c\": {:.4}, \"max_air_c\": {tmax:.4}, \"wall_heat_w\": [{}], \"vent_flow_out_m3s\": [{}], \"exhaust_c\": [{}], \"probes_c\": [{}]}}",
        c.name,
        m.cells(),
        rep.converged,
        rep.imbalance,
        sum / vol,
        walls.join(", "),
        flows.join(", "),
        exhaust.join(", "),
        probes.join(", ")
    );
    println!("OFREF | {row}");
    row
}

#[test]
#[ignore = "developer-side reference run, minutes per case"]
fn write_reference_cases_and_run_ours() {
    let out = std::env::var("OPENFOAM_REF_OUT").unwrap_or_else(|_| "/tmp/ofref".into());
    std::fs::create_dir_all(&out).unwrap();
    let all = cases();
    std::fs::write(format!("{out}/cases.json"), cases_json(&all)).unwrap();
    let pick: Option<Vec<String>> = std::env::var("OPENFOAM_REF_CASES")
        .ok()
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
    let targets: Vec<usize> = std::env::var("OPENFOAM_REF_CELLS")
        .ok()
        .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![9000, 30000, 70000]);
    let mut rows = Vec::new();
    for c in &all {
        if pick.as_ref().is_some_and(|p| !p.iter().any(|n| n == c.name)) {
            continue;
        }
        for &t in &targets {
            rows.push(run(c, t));
            std::fs::write(format!("{out}/ours.json"), format!("[\n{}\n]\n", rows.join(",\n")))
                .unwrap();
        }
    }
}
