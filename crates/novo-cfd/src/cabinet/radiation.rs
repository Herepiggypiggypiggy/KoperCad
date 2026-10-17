//! Surface-to-surface radiation coupled into the flow (the Fine level).
//!
//! The [`rad`](crate::rad) engine gives the net radiative flux of every
//! surface patch for a set of surface temperatures. Here that flux becomes a
//! heat source of the cells behind the surfaces, so a hot device gives heat
//! to the walls and to its neighbours instead of everything leaving through
//! a convective film:
//!
//! - a device face loses `q A` from the device cell, a face that absorbs
//!   (`q < 0`) heats its cell;
//! - an enclosure wall face looks into an air cell; what the wall absorbs is
//!   released into that air cell, which then loses it through the wall
//!   condition of the flow. The wall's own heat capacity and the radiation
//!   it re-emits to the outside are not resolved (the wall is a boundary
//!   condition, not a solid), so this is a model of the wall, not exact.
//!
//! The sum of all fluxes is zero up to the solver tolerance, so the coupling
//! moves heat around and never creates any.
//!
//! The lumped radiation term of the device films (emissivity times view
//! factor against the air temperature) is switched off while the coupling is
//! on, otherwise radiation would count twice.
//!
//! The view factors are built once; the radiosity system is solved again
//! every [`RadiationCoupling::every`] steps with the present temperatures and
//! the source is under-relaxed, because the flux is lagged against the
//! temperatures it depends on.

use crate::flow::{FlowSolver, ThermalBc};
use crate::rad::{PatchKind, RadSettings, RadiationModel};
use crate::result::Grid3;

/// Inner film of an enclosure wall, W/m2K, used to turn the wall condition
/// into a wall surface temperature (natural convection on a vertical
/// surface, IEC 60890 uses 5 to 10).
const WALL_INNER_H: f64 = 8.0;

/// Steps (or iterations) between two radiosity solutions.
pub const RAD_EVERY: usize = 5;

/// Share of the new source that enters the old one.
pub const RAD_RELAX: f64 = 0.5;

/// What the coupling did at the last refresh, for notes and tests.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RadiationReport {
    /// Net radiative power given off per mesh material id (0 = enclosure
    /// wall; negative = absorbs), W.
    pub power_by_material_w: Vec<(u16, f64)>,
    /// Heat exchanged by radiation, W (sum of what the hot patches give).
    pub exchanged_w: f64,
    /// Sum of all patch powers, W: zero when the radiosity solution is closed.
    pub imbalance_w: f64,
    pub patches: usize,
    pub refreshes: usize,
}

pub struct RadiationCoupling {
    model: RadiationModel,
    /// Steps between refreshes.
    pub every: usize,
    pub relax: f64,
    clock: usize,
    grid: Grid3,
    report: RadiationReport,
}

impl RadiationCoupling {
    /// `None` when radiation is off or the mesh has too little surface.
    /// Switches the lumped radiation of the device films off in `flow`.
    pub fn new(
        mesh: &crate::mesh::Mesh,
        flow: &mut FlowSolver,
        settings: &RadSettings,
    ) -> Option<RadiationCoupling> {
        let model = RadiationModel::build(mesh, settings)?;
        flow.cfg.device_view = 0.0;
        flow.extra_heat_w_m3 = Some(vec![0.0; flow.mesh.cells()]);
        let patches = model.patches.len();
        Some(RadiationCoupling {
            model,
            every: RAD_EVERY,
            relax: RAD_RELAX,
            clock: 0,
            grid: crate::turb::grid_of(flow),
            report: RadiationReport {
                patches,
                ..RadiationReport::default()
            },
        })
    }

    pub fn report(&self) -> &RadiationReport {
        &self.report
    }

    /// Call before every flow step: refreshes the source every `every` calls.
    pub fn tick(&mut self, flow: &mut FlowSolver) {
        if self.clock.is_multiple_of(self.every.max(1)) {
            self.refresh(flow);
        }
        self.clock += 1;
    }

    /// Surface temperature of an enclosure wall face seen from the air cell
    /// `cell` on side `dir`, kelvin.
    fn wall_temperature_k(&self, flow: &FlowSolver, cell: usize, dir: usize) -> f64 {
        let t_air = flow.t[cell];
        let t = match flow.cfg.faces[dir].thermal {
            ThermalBc::Adiabatic => t_air,
            ThermalBc::Fixed(tw) => tw,
            ThermalBc::Convective { t_out, .. } | ThermalBc::Overall { t_out, .. } => {
                // the heat that crosses the wall also crosses the inner film
                let u = match flow.cfg.faces[dir].thermal {
                    ThermalBc::Convective { h, .. } => h,
                    ThermalBc::Overall { u, .. } => u,
                    _ => 0.0,
                };
                let share = (u / WALL_INNER_H).clamp(0.0, 1.0);
                t_air - share * (t_air - t_out)
            }
        };
        t + 273.15
    }

    /// One radiosity solution with the present temperatures.
    pub fn refresh(&mut self, flow: &mut FlowSolver) {
        let patches = &self.model.patches;
        let mut t_k = patches.average(&self.grid, |f| {
            let cell = f.cell as usize;
            flow.t[cell] + 273.15
        });
        for (p, tk) in patches.patches.iter().zip(t_k.iter_mut()) {
            if p.kind == PatchKind::Wall {
                // area-weighted over the wall faces of the patch
                let (mut num, mut area) = (0.0, 0.0);
                for i in p.faces.clone() {
                    let f = patches.faces[i];
                    let ix = flow.mesh.unindex(f.cell as usize);
                    let a = flow.mesh.area(usize::from(f.dir / 2), ix);
                    num += a * self.wall_temperature_k(flow, f.cell as usize, usize::from(f.dir));
                    area += a;
                }
                if area > 0.0 {
                    *tk = num / area;
                }
            }
        }
        let res = self.model.solve(&t_k);
        let mut fresh = vec![0.0; flow.mesh.cells()];
        for (f, q) in patches.scatter(&res.net_flux_w_m2) {
            let cell = f.cell as usize;
            let ix = flow.mesh.unindex(cell);
            let a = flow.mesh.area(usize::from(f.dir / 2), ix);
            fresh[cell] -= q * a / flow.mesh.volume(ix);
        }
        let extra = flow
            .extra_heat_w_m3
            .get_or_insert_with(|| vec![0.0; fresh.len()]);
        let first = self.report.refreshes == 0;
        let w = if first {
            1.0
        } else {
            self.relax.clamp(0.0, 1.0)
        };
        for (e, n) in extra.iter_mut().zip(&fresh) {
            *e += w * (n - *e);
        }
        self.report.power_by_material_w = res.power_by_material(patches);
        self.report.exchanged_w = res.exchanged_w;
        self.report.imbalance_w = res.imbalance_w;
        self.report.refreshes += 1;
    }
}
