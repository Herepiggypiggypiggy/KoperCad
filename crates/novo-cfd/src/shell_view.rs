//! Wall, door and plate temperatures of a 3D result (heat lane H1).
//!
//! The solver applies each wall as one overall air-to-air coefficient from the
//! layered shell (`novo_thermal::shell`). The surface temperatures follow from
//! the air next to each face and the same stack, exactly as in the 2D engine,
//! so both give one answer for the shell.

use crate::result::{CfdSnapshot, MaterialKind};
use novo_thermal::Scenario;
use novo_thermal::shell::{
    self, BOTTOM, FRONT, FaceAreas, FaceSamples, LEFT, REAR, RIGHT, ShellReport, TOP,
};

/// Shell report of `snap`: the first layer of air cells against each wall
/// (weighted by wall area) gives the inside temperature, the plate cells
/// give `plate_rear_c`.
pub fn shell_report(scn: &Scenario, snap: &CfdSnapshot) -> ShellReport {
    let g = &snap.grid;
    let model = scn.shell();
    let gf = model.conductances_w_per_k(&scn.enclosure, scn.params.wall_k_scale);
    let is_air = |idx: usize| {
        let m = snap.material.get(idx).copied().unwrap_or(0);
        m == 0
            || snap
                .materials
                .iter()
                .find(|mi| mi.id == m)
                .is_some_and(|mi| mi.kind == MaterialKind::Air)
    };
    let dx = |i: usize| g.x_edges_mm[i + 1] - g.x_edges_mm[i];
    let dy = |j: usize| g.y_edges_mm[j + 1] - g.y_edges_mm[j];
    let dz = |k: usize| g.z_edges_mm[k + 1] - g.z_edges_mm[k];
    let mut raw: [FaceSamples; 6] = Default::default();
    let mut add = |face: usize, idx: usize, area: f64| {
        if is_air(idx) {
            raw[face].push((area, f64::from(snap.temperature_c[idx])));
        }
    };
    for k in 0..g.nz {
        for j in 0..g.ny {
            add(LEFT, g.index(0, j, k), dy(j) * dz(k));
            add(RIGHT, g.index(g.nx - 1, j, k), dy(j) * dz(k));
        }
    }
    for k in 0..g.nz {
        for i in 0..g.nx {
            add(TOP, g.index(i, 0, k), dx(i) * dz(k));
            add(BOTTOM, g.index(i, g.ny - 1, k), dx(i) * dz(k));
        }
    }
    for j in 0..g.ny {
        for i in 0..g.nx {
            add(REAR, g.index(i, j, 0), dx(i) * dy(j));
            add(FRONT, g.index(i, j, g.nz - 1), dx(i) * dy(j));
        }
    }
    // Area weights become conductances: the face conductance split by area.
    let mut samples: [FaceSamples; 6] = std::array::from_fn(|f| {
        let total: f64 = raw[f].iter().map(|s| s.0).sum();
        raw[f]
            .iter()
            .map(|&(a, t)| (gf[f] * a / total.max(1e-12), t))
            .collect()
    });
    // The plate stands in front of the rear wall: its share of the rear
    // conductance sees the plate temperature, not the air (the plate's cover
    // of the rear wall by area; the snapshot keeps only the mean plate
    // temperature).
    let plate_rear = snap.plate_rear_c;
    if let Some(tp) = plate_rear {
        let e = &scn.enclosure;
        let a_in = FaceAreas::of(e).inner[REAR];
        let a_plate = (e.plate.w * e.plate.h / 1.0e6).min(a_in);
        let share = a_plate / a_in.max(1e-9);
        for s in &mut samples[REAR] {
            s.0 *= 1.0 - share;
        }
        // Same series path the solver uses behind the plate.
        let kb = gf[REAR] / a_in.max(1e-9);
        let h_air = scn.params.h_in.max(1.2 * kb);
        let u_out = model.plate_to_outside_or(kb, h_air);
        samples[REAR].push((u_out * a_plate, tp));
    }
    shell::report(
        &model,
        &scn.enclosure,
        scn.ambient_c,
        scn.params.wall_k_scale,
        &samples,
        plate_rear,
    )
}
