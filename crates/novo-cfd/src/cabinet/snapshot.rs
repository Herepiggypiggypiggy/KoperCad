//! From the state of the flow solver to a [`CfdSnapshot`].

use novo_layout::Rect;
use novo_thermal::{DeviceResult, HotSpot, HotSpotKind, Scenario, Status};

use super::statement::accuracy_for;
use crate::flow::{FlowSolver, Mesh as FlowMesh};
use crate::mesh::CellClass;
use crate::result::{CfdSnapshot, Fidelity, MaterialKind, Progress};
use crate::solids::{FlowSetup, Medium};

const AIR_K: f64 = 0.026;

/// Uncertainty of a device temperature rise above ambient, as a share of the
/// rise, per level. A planning value for the loss data and boundary
/// conditions (the 3D result itself is bounded by the benchmark and mesh
/// studies in the accuracy statement, not by this band).
fn band_share(f: Fidelity) -> f64 {
    match f {
        Fidelity::Coarse => 0.20,
        Fidelity::Medium => 0.15,
        _ => 0.12,
    }
}

/// Everything needed to turn solver state into snapshots, built once.
pub struct View<'a> {
    setup: &'a FlowSetup,
    scn: &'a Scenario,
    fidelity: Fidelity,
    /// Per device (scenario order): its cells with the share of the cell the
    /// device fills (1 for a pure cell), ascending by cell.
    device_cells: Vec<Vec<(usize, f64)>>,
    base: CfdSnapshot,
}

impl<'a> View<'a> {
    pub fn new(setup: &'a FlowSetup, scn: &'a Scenario, fidelity: Fidelity) -> View<'a> {
        let cm = &setup.classified;
        let n = cm.grid.cells();
        let device_cells = scn
            .devices
            .iter()
            .map(|d| {
                let Some(id) = cm
                    .materials
                    .iter()
                    .find(|m| m.kind == MaterialKind::Device(d.id))
                    .map(|m| m.id)
                else {
                    return Vec::new();
                };
                let mut v: Vec<(usize, f64)> = Vec::new();
                for c in 0..n {
                    if cm.class[c] == CellClass::Solid && cm.material[c] == id {
                        v.push((c, 1.0));
                    }
                }
                for mc in &cm.mixed {
                    if let Some(&(_, share)) = mc.parts.iter().find(|p| p.0 == id) {
                        v.push((mc.cell, share));
                    }
                }
                v.sort_by_key(|x| x.0);
                v
            })
            .collect();
        let mut notes = vec![format!(
            "Cabinet solver, {}: {} cells, laminar buoyant flow with conduction in devices, plate, \
             rails and walls. Turbulence and radiation are not modelled yet.",
            fidelity.label(),
            n
        )];
        notes.push(
            "Heat conduction into the mounting plate is not resolved by the grid (the plate is thinner \
             than a cell): the plate share of every device reads 0 and its heat is counted as going \
             to the air."
                .into(),
        );
        notes.extend(super::march::lost_heat_note(setup, scn));
        let base = CfdSnapshot {
            grid: cm.grid.clone(),
            temperature_c: Vec::new(),
            velocity: Vec::new(),
            pressure_pa: Vec::new(),
            turb_visc_ratio: vec![0.0; n],
            material: cm.material.clone(),
            materials: cm.materials.clone(),
            wall_flux_w_m2: Vec::new(),
            devices: Vec::new(),
            hot_spots: Vec::new(),
            progress: Progress {
                fidelity,
                iteration: 0,
                residuals: Default::default(),
                converged: false,
                elapsed_s: 0.0,
                energy_balance_rel: 1.0,
                monitor_drift_k: 0.0,
            },
            notes,
            accuracy: accuracy_for(fidelity),
        };
        View {
            setup,
            scn,
            fidelity,
            device_cells,
            base,
        }
    }

    /// The full snapshot of the present state.
    pub fn build(&self, flow: &FlowSolver, progress: &Progress) -> CfdSnapshot {
        let mut s = self.base.clone();
        s.temperature_c = flow.t.iter().map(|t| *t as f32).collect();
        s.velocity = cell_velocity(&flow.mesh, &flow.vel);
        let rho = flow.cfg.rho;
        s.pressure_pa = flow.p.iter().map(|p| (p * rho) as f32).collect();
        let medium = self.setup.cfg.medium.as_ref();
        s.wall_flux_w_m2 =
            medium.map_or_else(|| vec![0.0; flow.t.len()], |m| self.surface_flux(flow, m));
        let (devices, unresolved) = self.devices(flow);
        s.devices = devices;
        for id in unresolved {
            s.notes.push(format!(
                "Device {id} is smaller than two cells across at this level: its surface temperature is \
                 not resolved (wide uncertainty band). Run a finer level."
            ));
        }
        s.hot_spots = self.hot_spots(flow, &s.devices);
        s.progress = progress.clone();
        s
    }

    /// Heat flux out of solid cells into the air, W/m2 of exposed face
    /// (cells without a face towards air carry 0).
    fn surface_flux(&self, flow: &FlowSolver, md: &Medium) -> Vec<f32> {
        let m = &flow.mesh;
        let mut out = vec![0.0f32; m.cells()];
        for k in 0..m.n[2] {
            for j in 0..m.n[1] {
                for i in 0..m.n[0] {
                    let ix = [i, j, k];
                    let c = m.cell(ix);
                    if md.fluid_frac[c] > 0.5 {
                        continue;
                    }
                    let (mut q, mut area) = (0.0, 0.0);
                    for_neighbours(m, ix, |n, a, dist_c, dist_n| {
                        if md.fluid_frac[n] > 0.5 {
                            let kc = md.cell_k(c, AIR_K);
                            let kn = AIR_K;
                            let r = dist_c / kc + dist_n / kn;
                            q += a * (flow.t[c] - flow.t[n]) / r;
                            area += a;
                        }
                    });
                    if area > 0.0 {
                        out[c] = (q / area) as f32;
                    }
                }
            }
        }
        out
    }

    /// Results per device, and the ids of those the grid does not resolve.
    fn devices(&self, flow: &FlowSolver) -> (Vec<DeviceResult>, Vec<u64>) {
        let Some(md) = self.setup.cfg.medium.as_ref() else {
            return (Vec::new(), Vec::new());
        };
        let mut unresolved = Vec::new();
        let m = &flow.mesh;
        let amb = self.scn.ambient_c;
        let share = band_share(self.fidelity);
        let out: Vec<DeviceResult> = self
            .scn
            .devices
            .iter()
            .zip(&self.device_cells)
            .map(|(d, cells)| {
                // area-weighted temperature of the housing and of the air
                // touching it; heat that leaves into other solids
                let (mut a_exp, mut t_surf, mut t_air, mut q_cond) = (0.0, 0.0, 0.0, 0.0);
                for &(c, w) in cells {
                    let ix = cell_ijk(m, c);
                    for_neighbours(m, ix, |n, a, dist_c, dist_n| {
                        if md.fluid_frac[n] > 0.5 {
                            a_exp += w * a;
                            t_surf += w * a * flow.t[c];
                            t_air += w * a * flow.t[n];
                        } else if cells.binary_search_by_key(&n, |x| x.0).is_err() {
                            let r = dist_c / md.cell_k(c, AIR_K) + dist_n / md.cell_k(n, AIR_K);
                            q_cond += w * a * (flow.t[c] - flow.t[n]) / r;
                        }
                    });
                }
                let (surface_c, air_c) = if a_exp > 0.0 {
                    (t_surf / a_exp, t_air / a_exp)
                } else if cells.is_empty() {
                    (amb, amb)
                } else {
                    let (sum, wsum) = cells
                        .iter()
                        .fold((0.0, 0.0), |(s, ws), &(c, w)| (s + w * flow.t[c], ws + w));
                    (sum / wsum, sum / wsum)
                };
                let loss = d.loss_w;
                let to_air = (loss - q_cond).max(0.0);
                let dt = surface_c - air_c;
                let h = if a_exp > 0.0 && dt > 0.05 {
                    to_air / (a_exp * dt)
                } else {
                    0.0
                };
                // A device the grid cuts into fewer than 2 x 2 x 2 cells, or whose
                // exposed faces do not add up to its geometry, has no reliable
                // housing temperature: the estimate is a cell value, not a
                // surface. Its band is the whole range from the air to twice
                // the estimated rise, and the snapshot says so.
                let (f, hgt) = (&d.footprint, d.height());
                let a_geom = (f.w * f.h + 2.0 * (f.w + f.h) * hgt) * 1e-6;
                let pure = cells.iter().filter(|c| c.1 >= 1.0).count();
                let resolved = pure >= 8 && (0.7..=1.3).contains(&(a_exp / a_geom.max(1e-12)));
                if !resolved {
                    unresolved.push(d.id);
                }
                let spread = if resolved { share } else { 1.0 };
                let band = |v: f64| {
                    let rise = (v - amb).max(0.0);
                    (v - spread * rise, v + spread * rise)
                };
                let mut r = DeviceResult {
                    id: d.id,
                    footprint: d.footprint,
                    loss_w: loss,
                    surface_c,
                    air_c,
                    h_w_m2k: h,
                    plate_share: if loss > 0.0 {
                        (q_cond / loss).clamp(0.0, 1.0)
                    } else {
                        0.0
                    },
                    surface_band_c: band(surface_c),
                    air_band_c: band(air_c),
                    rated_max_ambient_c: d.rated_max_ambient_c,
                    rated_max_surface_c: d.rated_max_surface_c,
                    margin_k: None,
                    margin_pessimistic_k: None,
                    status: Status::NoLimit,
                };
                apply_margins(&mut r);
                r
            })
            .collect();
        (out, unresolved)
    }

    fn hot_spots(&self, flow: &FlowSolver, devices: &[DeviceResult]) -> Vec<HotSpot> {
        let mut out = Vec::new();
        let mut ranked: Vec<&DeviceResult> = devices
            .iter()
            .filter(|d| matches!(d.status, Status::Over | Status::Marginal))
            .collect();
        ranked.sort_by(|a, b| {
            a.margin_k
                .unwrap_or(f64::MAX)
                .total_cmp(&b.margin_k.unwrap_or(f64::MAX))
        });
        let mean_air = self.mean_air(flow);
        for d in ranked {
            let f: Rect = d.footprint;
            out.push(HotSpot {
                kind: HotSpotKind::Device(d.id),
                x_mm: f.x + f.w / 2.0,
                y_mm: f.y + f.h / 2.0,
                temp_c: d.surface_c,
                above_mean_k: d.surface_c - mean_air,
                reason: match d.status {
                    Status::Over => "over its temperature limit".into(),
                    _ => "within its limit, but not at the pessimistic end of the estimate".into(),
                },
            });
        }
        // the hottest air cell, if it stands out
        let md = self.setup.cfg.medium.as_ref();
        let m = &flow.mesh;
        let (mut best, mut at) = (f64::NEG_INFINITY, None);
        for c in 0..m.cells() {
            if md.is_none_or(|md| md.fluid_frac[c] > 0.9) && flow.t[c] > best {
                best = flow.t[c];
                at = Some(c);
            }
        }
        if let Some(c) = at.filter(|_| best - mean_air > 5.0) {
            let ix = cell_ijk(m, c);
            out.push(HotSpot {
                kind: HotSpotKind::Air,
                x_mm: m.centre[0][ix[0]] * 1e3,
                y_mm: m.centre[1][ix[1]] * 1e3,
                temp_c: best,
                above_mean_k: best - mean_air,
                reason: "hottest air in the enclosure".into(),
            });
        }
        out
    }

    fn mean_air(&self, flow: &FlowSolver) -> f64 {
        let m = &flow.mesh;
        let md = self.setup.cfg.medium.as_ref();
        let (mut sum, mut vol) = (0.0, 0.0);
        for k in 0..m.n[2] {
            for j in 0..m.n[1] {
                for i in 0..m.n[0] {
                    let c = m.cell([i, j, k]);
                    let w = md.map_or(1.0, |md| md.fluid_frac[c]) * m.volume([i, j, k]);
                    sum += w * flow.t[c];
                    vol += w;
                }
            }
        }
        if vol > 0.0 {
            sum / vol
        } else {
            self.scn.ambient_c
        }
    }
}

/// Margin and status from the bands, the rule of `novo-thermal`.
fn apply_margins(d: &mut DeviceResult) {
    let margin = |amb: f64, surf: f64| -> Option<f64> {
        let a = d.rated_max_ambient_c.map(|m| m - amb);
        let b = d.rated_max_surface_c.map(|m| m - surf);
        match (a, b) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    };
    d.margin_k = margin(d.air_c, d.surface_c);
    d.margin_pessimistic_k = margin(d.air_band_c.1, d.surface_band_c.1);
    d.status = match (d.margin_k, d.margin_pessimistic_k) {
        (Some(m), _) if m < 0.0 => Status::Over,
        (Some(_), Some(p)) if p < 0.0 => Status::Marginal,
        (Some(_), _) => Status::Ok,
        _ => Status::NoLimit,
    };
}

fn cell_ijk(m: &FlowMesh, c: usize) -> [usize; 3] {
    let i = c % m.n[0];
    let j = (c / m.n[0]) % m.n[1];
    let k = c / (m.n[0] * m.n[1]);
    [i, j, k]
}

/// Calls `f(neighbour cell, face area, half size of this cell, half size of
/// the neighbour)` for each of the up to six neighbours.
fn for_neighbours(m: &FlowMesh, ix: [usize; 3], mut f: impl FnMut(usize, f64, f64, f64)) {
    for a in 0..3 {
        for high in [false, true] {
            let mut nb = ix;
            if high {
                if ix[a] + 1 >= m.n[a] {
                    continue;
                }
                nb[a] += 1;
            } else {
                if ix[a] == 0 {
                    continue;
                }
                nb[a] -= 1;
            }
            f(
                m.cell(nb),
                m.area(a, ix),
                0.5 * m.dx[a][ix[a]],
                0.5 * m.dx[a][nb[a]],
            );
        }
    }
}

/// Velocity at the cell centres from the face values.
pub fn cell_velocity(m: &FlowMesh, vel: &[Vec<f64>; 3]) -> Vec<[f32; 3]> {
    let mut out = Vec::with_capacity(m.cells());
    for k in 0..m.n[2] {
        for j in 0..m.n[1] {
            for i in 0..m.n[0] {
                let ix = [i, j, k];
                let mut v = [0.0f32; 3];
                for (a, va) in v.iter_mut().enumerate() {
                    let mut hi = ix;
                    hi[a] += 1;
                    *va = (0.5 * (vel[a][m.idx(a, ix)] + vel[a][m.idx(a, hi)])) as f32;
                }
                out.push(v);
            }
        }
    }
    out
}
