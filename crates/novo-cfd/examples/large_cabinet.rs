//! Loads a project file and reports what a whole-cabinet run needs, with and
//! without the reduced model; optionally solves it.
//!
//! `cargo run --release -p novo-cfd --example large_cabinet -- <file.novo> [coarse|medium|fine] [solve]`
//!
//! `LARGE_DEBUG=1` prints the pitch search of the reduction.

use novo_cfd::cabinet::Tuning;
use novo_cfd::large::{ReduceSettings, convergence, reduce};
use novo_cfd::mesh::{MeshSettings, plan_grid_report};
use novo_cfd::result::Fidelity;
use novo_cfd::{CabinetSolver, Control, Solver};
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let project = novo_storage::ProjectFile::read(&args[1]).expect("open the project file");
    let fid = match args.get(2).map(String::as_str) {
        Some("medium") => Fidelity::Medium,
        Some("fine") => Fidelity::Fine,
        _ => Fidelity::Coarse,
    };
    let cells = Tuning::default()
        .level(fid)
        .and_then(|l| l.target_cells)
        .expect("level target");
    for s in project.structures().filter(|s| s.enclosure) {
        let Ok(b) = scenario_from_project(&project, s.id, &BridgeOptions::default()) else {
            continue;
        };
        let scn = &b.scenario;
        println!(
            "enclosure {:?}: {} devices ({:.0} W), {} obstacles",
            s.id,
            scn.devices.len(),
            scn.devices.iter().map(|d| d.loss_w).sum::<f64>(),
            scn.obstacles.len()
        );
        let mesh = MeshSettings {
            target_cells: cells,
            ..MeshSettings::for_fidelity(fid).unwrap()
        };
        let full = plan_grid_report(scn, &mesh).unwrap();
        println!(
            "full grid {}x{}x{} = {} cells (target {cells})",
            full.grid.nx,
            full.grid.ny,
            full.grid.nz,
            full.grid.cells()
        );
        let red = reduce(
            scn,
            &ReduceSettings {
                mesh,
                ..ReduceSettings::for_cells(cells)
            },
        );
        println!("{}", red.report.summary());
        println!(
            "pitch {} mm, smeared {}, hot devices moved {:.0} mm, over budget {}",
            red.report.pitch_mm,
            red.report.smeared_obstacles,
            red.report.hot_shift_mm,
            red.report.over_budget
        );
        if args.get(3).map(String::as_str) != Some("solve") {
            continue;
        }
        let t = std::time::Instant::now();
        let r = CabinetSolver::default()
            .solve(scn, fid, &Control::new(), &mut |s| {
                eprintln!(
                    "  it {} residual {:.1e} balance {:.3} {:.0} s",
                    s.progress.iteration,
                    s.progress.residuals.max(),
                    s.progress.energy_balance_rel,
                    t.elapsed().as_secs_f64()
                );
            })
            .unwrap();
        println!("{:?}", convergence(&r).lines);
        println!(
            "solved {} cells in {:.0} s, hottest {:.1} C",
            r.grid.cells(),
            t.elapsed().as_secs_f64(),
            r.max_temperature_c()
        );
        for d in r.devices.iter().filter(|d| d.loss_w > 10.0) {
            println!(
                "  device {} {:.0} W surface {:.1} C air {:.1} C margin {:?}",
                d.id, d.loss_w, d.surface_c, d.air_c, d.margin_k
            );
        }
        let over = r
            .devices
            .iter()
            .filter(|d| d.status == novo_thermal::Status::Over)
            .count();
        println!("{over} devices over their limit");
    }
}
