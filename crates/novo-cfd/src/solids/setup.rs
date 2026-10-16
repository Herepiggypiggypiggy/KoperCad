//! From a scenario to a runnable flow solver: the driver of lane CFD-C2.
//!
//! Mesh classification (lane C1) gives the materials, [`Medium`] the solid
//! properties and heat sources, `openings` the vents and fans, and [`Walls`]
//! the outside film of the enclosure.

use novo_thermal::Scenario;

use super::Medium;
use super::props::props_for;
use crate::flow::{Face, FaceBc, FlowConfig, FlowSolver, Mesh as FlowMesh, Scheme, ThermalBc};
use crate::mesh::{self, CellClass, MeshError, MeshSettings};
use crate::openings::{OpeningError, patches_from_scenario};
use crate::solver::Control;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SetupError {
    #[error(transparent)]
    Mesh(#[from] MeshError),
    #[error(transparent)]
    Opening(#[from] OpeningError),
}

/// The slotted wire duct as a porous medium.
///
/// A duct is a hollow channel with slots in both side walls. Air crosses it
/// through the slots: it passes two slotted walls in series, each with loss
/// `K u^2 / 2` and `K = 1 / (cd slot_free)^2` on the speed in the duct, so
/// `K u^2` in total, spread over the duct width `w` as drag `K / w`. Along the
/// duct it runs almost freely (the clutter drag of the scenario per metre). The lid and the plate close the third
/// direction, ten times the cross drag. Planning values, not a measurement of
/// a maker's duct.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuctModel {
    /// Air share of the duct volume (the rest is the PVC walls and fingers).
    pub porosity: f64,
    /// Free share of a slotted side wall.
    pub slot_free: f64,
    /// Discharge coefficient of a slot.
    pub cd: f64,
}

impl Default for DuctModel {
    fn default() -> Self {
        DuctModel {
            porosity: 0.85,
            slot_free: 0.45,
            cd: 0.6,
        }
    }
}

/// What to build.
#[derive(Debug, Clone, PartialEq)]
pub struct SolidsSettings {
    pub mesh: MeshSettings,
    /// Divides the heat capacity of the solids when marching to a steady
    /// state (a false transient: the steady result does not depend on it,
    /// the time to get there does). 1 = real capacities, for transients.
    pub solid_speedup: f64,
    pub scheme: Scheme,
    pub duct: DuctModel,
}

impl Default for SolidsSettings {
    fn default() -> Self {
        SolidsSettings {
            mesh: MeshSettings::default(),
            solid_speedup: 100.0,
            scheme: Scheme::VanLeer,
            duct: DuctModel::default(),
        }
    }
}

/// The six enclosure walls as heat paths to the outside air.
///
/// The IEC 60890 coefficient `k` (W/m2K) is the overall value from the
/// air inside to the air outside, wall sheet and both films included; the
/// effective surface weights each face by a factor `b` (covered faces give
/// off less, a floor-standing enclosure none through the bottom). The solver
/// applies `k b A_outside / A_inside` to each interior face as an overall
/// air-to-air coefficient (`ThermalBc::Overall`: no extra half cell of
/// conduction or inner film on top), so the sum over the faces is the IEC
/// conductance `k * A_effective` (tested) and a sealed enclosure does not
/// depend on the mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Walls {
    /// Film coefficient of each domain face in the order of [`Face`],
    /// W/m2K of the interior face; 0 = adiabatic.
    pub h: [f64; 6],
}

/// Wall films of `scn`, scaled by `Params::wall_k_scale`.
pub fn wall_conductances(scn: &Scenario) -> Walls {
    let e = &scn.enclosure;
    let inst = e.installation;
    let k = e.material.k() * scn.params.wall_k_scale;
    let (wo, ho, d_o) = (
        e.width_mm / 1000.0,
        e.height_mm / 1000.0,
        e.depth_mm / 1000.0,
    );
    let (wi, hi, di) = (
        e.interior_width_mm() / 1000.0,
        e.interior_height_mm() / 1000.0,
        e.interior_depth_mm() / 1000.0,
    );
    let pick = |covered: bool, free: f64, cov: f64| if covered { cov } else { free };
    // (face, b factor, outside area, inside area); IEC 60890 factors
    let faces = [
        (
            Face::XMin,
            pick(inst.left_covered, 0.9, 0.5),
            d_o * ho,
            di * hi,
        ),
        (
            Face::XMax,
            pick(inst.right_covered, 0.9, 0.5),
            d_o * ho,
            di * hi,
        ),
        (
            Face::YMin,
            pick(inst.top_covered, 1.4, 0.7),
            wo * d_o,
            wi * di,
        ),
        (
            Face::YMax,
            if inst.on_floor { 0.0 } else { 0.5 },
            wo * d_o,
            wi * di,
        ),
        (
            Face::ZMin,
            pick(inst.rear_covered, 0.9, 0.5),
            wo * ho,
            wi * hi,
        ),
        (Face::ZMax, 0.9, wo * ho, wi * hi),
    ];
    let mut h = [0.0; 6];
    for (f, b, a_out, a_in) in faces {
        h[f as usize] = k * b * a_out / a_in.max(1e-9);
    }
    Walls { h }
}

impl Medium {
    /// Materials, shares and heat sources of a classified grid. Heat that
    /// belongs to a device the mesh did not find (outside the enclosure) is
    /// not placed; the second value is its sum, W.
    pub fn from_mesh(
        cm: &mesh::Mesh,
        scn: &Scenario,
        solid_speedup: f64,
        duct: &DuctModel,
    ) -> (Medium, f64) {
        let n = cm.grid.cells();
        let mut md = Medium::air(n);
        let speed = solid_speedup.max(1.0);
        let table: Vec<(u16, super::SolidProps, f64)> = cm
            .materials
            .iter()
            .filter(|m| m.id != 0)
            .map(|m| {
                let vol = cm
                    .material_volume_m3
                    .get(m.id as usize)
                    .copied()
                    .unwrap_or(0.0);
                let p = props_for(m.kind, scn, vol);
                let loss = match m.kind {
                    crate::result::MaterialKind::Device(id) => scn
                        .devices
                        .iter()
                        .find(|d| d.id == id)
                        .map_or(0.0, |d| d.loss_w),
                    _ => 0.0,
                };
                // heat density inside the material, W/m3
                let q = if vol > 0.0 { loss / vol } else { 0.0 };
                (m.id, p, q)
            })
            .collect();
        let find = |id: u16| table.iter().find(|t| t.0 == id);
        // Ducts are porous: id = 2 + devices + index among the obstacles.
        let duct_of = |id: u16| -> Option<(usize, &novo_thermal::Obstacle)> {
            let idx = (id as usize).checked_sub(2 + scn.devices.len())?;
            let o = scn.obstacles.get(idx)?;
            (o.kind == novo_thermal::ObstacleKind::Duct).then_some((idx, o))
        };
        let mut extra_air = vec![0.0f64; n];
        let mut add = |cell: usize, id: u16, share: f64| {
            if let Some((_, p, q)) = find(id) {
                if let Some((_, o)) = duct_of(id) {
                    let solid = 1.0 - duct.porosity;
                    md.c_solid[cell] += share * solid * p.rho_cp / speed;
                    md.k_solid[cell] += share * solid * p.k;
                    extra_air[cell] += share * duct.porosity;
                    let w = (o.rect.w.min(o.rect.h) / 1000.0).max(1e-3);
                    let slot = (duct.cd * duct.slot_free).max(1e-3);
                    let cross = 1.0 / (slot * slot) / w;
                    let along = scn.params.clutter_drag_per_m;
                    let (long_axis, cross_axis) =
                        if o.rect.w >= o.rect.h { (0, 1) } else { (1, 0) };
                    md.drag[long_axis][cell] += share * along;
                    md.drag[cross_axis][cell] += share * cross;
                    md.drag[2][cell] += share * 10.0 * cross;
                    return;
                }
                md.c_solid[cell] += share * p.rho_cp / speed;
                md.k_solid[cell] += share * p.k;
                md.heat_w_m3[cell] += share * q;
            }
        };
        for cell in 0..n {
            match cm.class[cell] {
                CellClass::Fluid => {}
                CellClass::Solid => add(cell, cm.material[cell], 1.0),
                CellClass::Cut => {}
            }
        }
        for mc in &cm.mixed {
            for &(id, share) in &mc.parts {
                if id != 0 {
                    add(mc.cell, id, share);
                }
            }
        }
        for (cell, ff) in cm.fluid_fraction.iter().enumerate() {
            md.fluid_frac[cell] = (*ff + extra_air[cell]).min(1.0);
        }
        let lost: f64 = scn
            .devices
            .iter()
            .filter(|d| {
                !cm.materials.iter().any(|m| {
                    m.kind == crate::result::MaterialKind::Device(d.id)
                        && cm
                            .material_volume_m3
                            .get(m.id as usize)
                            .copied()
                            .unwrap_or(0.0)
                            > 0.0
                })
            })
            .map(|d| d.loss_w)
            .sum();
        (md, lost)
    }
}

/// A scenario ready to march: mesh, solver configuration and bookkeeping.
pub struct FlowSetup {
    pub mesh: FlowMesh,
    pub cfg: FlowConfig,
    pub t_init: f64,
    pub ambient_c: f64,
    /// Heat of devices that lie outside the enclosure and are not simulated, W.
    pub lost_heat_w: f64,
    /// Heat put into the grid, W.
    pub heat_w: f64,
    pub classified: mesh::Mesh,
}

/// How a steady march ended.
///
/// A laminar plume above a hot device may never settle to the last digit, so
/// the march judges steadiness on the heat stored in the grid, averaged over a
/// window, not on the largest local change.
#[derive(Debug, Clone, PartialEq)]
pub struct SteadyReport {
    pub steps: usize,
    pub time_s: f64,
    pub converged: bool,
    /// Rate of change of the stored heat over the last window, relative to
    /// the heating (0 = steady).
    pub storage_rate: f64,
    pub heat_in_w: f64,
    /// Window averages of the heat leaving through the walls and openings.
    pub heat_out_walls_w: f64,
    pub heat_out_openings_w: f64,
    /// `(in - out) / in` of the window averages, 0 = balanced.
    pub imbalance: f64,
}

/// Seconds of simulated time the steadiness check averages over.
pub const STEADY_WINDOW_S: f64 = 30.0;

impl FlowSetup {
    pub fn new(scn: &Scenario, st: &SolidsSettings) -> Result<FlowSetup, SetupError> {
        let classified = mesh::Mesh::build(scn, &st.mesh)?;
        let mesh = FlowMesh::from_grid(&classified.grid);
        let (medium, lost_heat_w) = Medium::from_mesh(&classified, scn, st.solid_speedup, &st.duct);
        let heat_w = medium.total_heat_w(&mesh);
        let walls = wall_conductances(scn);
        let mut cfg = FlowConfig::air();
        cfg.scheme = st.scheme;
        cfg.t_ref = Some(scn.ambient_c);
        for (f, bc) in cfg.faces.iter_mut().enumerate() {
            *bc = FaceBc {
                thermal: if walls.h[f] > 0.0 {
                    ThermalBc::Overall {
                        u: walls.h[f],
                        t_out: scn.ambient_c,
                    }
                } else {
                    ThermalBc::Adiabatic
                },
                ..FaceBc::WALL
            };
        }
        cfg.openings = patches_from_scenario(scn)?;
        cfg.medium = Some(medium);
        Ok(FlowSetup {
            mesh,
            cfg,
            t_init: scn.ambient_c,
            ambient_c: scn.ambient_c,
            lost_heat_w,
            heat_w,
            classified,
        })
    }

    pub fn solver(&self) -> FlowSolver {
        FlowSolver::new(self.mesh.clone(), self.cfg.clone(), self.t_init)
    }

    /// Marches in time until the stored heat stops changing: its rate over
    /// the last [`STEADY_WINDOW_S`] is below `tol` times the heating. Stops
    /// at `max_time_s` or `max_steps`, or when `control` cancels. `every` is
    /// called with the solver every `report_every` steps (0 = never).
    pub fn run_steady(
        &self,
        solver: &mut FlowSolver,
        tol: f64,
        limits: (f64, usize),
        control: &Control,
        mut every: impl FnMut(&FlowSolver, usize),
        report_every: usize,
    ) -> SteadyReport {
        let (max_time_s, max_steps) = limits;
        // (time, stored heat, wall heat, opening heat) sampled every 10 steps
        let mut samples: Vec<(f64, f64, f64, f64)> = Vec::new();
        let sample = |s: &FlowSolver| {
            let w: f64 = (0..6).map(|f| s.wall_heat_w(f)).sum();
            (
                s.time,
                s.stored_heat_j(self.ambient_c),
                w,
                s.open_heat_out_w(self.ambient_c),
            )
        };
        samples.push(sample(solver));
        let (mut steps, mut rate, mut converged) = (0usize, f64::INFINITY, false);
        while steps < max_steps && solver.time < max_time_s && !control.is_cancelled() {
            let dt = solver.max_dt(0.4).clamp(1e-4, 5.0);
            solver.step(dt);
            steps += 1;
            if report_every > 0 && steps % report_every == 0 {
                every(solver, steps);
            }
            if steps % 10 == 0 {
                samples.push(sample(solver));
                let now = samples.last().unwrap().0;
                let first = samples.partition_point(|x| x.0 < now - STEADY_WINDOW_S);
                if first > 0 && now - samples[first].0 > 0.5 * STEADY_WINDOW_S {
                    let (a, b) = (samples[first], *samples.last().unwrap());
                    rate = if self.heat_w > 0.0 {
                        (b.1 - a.1) / (b.0 - a.0) / self.heat_w
                    } else {
                        0.0
                    };
                    if rate.abs() < tol {
                        converged = true;
                        break;
                    }
                }
            }
        }
        let now = solver.time;
        let win: Vec<_> = samples
            .iter()
            .filter(|x| x.0 >= now - STEADY_WINDOW_S)
            .collect();
        let n = win.len().max(1) as f64;
        let walls = win.iter().map(|x| x.2).sum::<f64>() / n;
        let open = win.iter().map(|x| x.3).sum::<f64>() / n;
        SteadyReport {
            steps,
            time_s: now,
            converged,
            storage_rate: rate,
            heat_in_w: self.heat_w,
            heat_out_walls_w: walls,
            heat_out_openings_w: open,
            imbalance: if self.heat_w > 0.0 {
                (self.heat_w - walls - open) / self.heat_w
            } else {
                0.0
            },
        }
    }
}
