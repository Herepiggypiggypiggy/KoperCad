//! From a scenario to a runnable flow solver: the driver of lane CFD-C2.
//!
//! Mesh classification (lane C1) gives the materials, [`Medium`] the solid
//! properties and heat sources, `openings` the vents and fans, and [`Walls`]
//! the outside film of the enclosure.

use novo_thermal::Scenario;

use super::props::props_for;
use super::{Medium, NO_DEVICE};
use crate::flow::{
    Face, FaceBc, FlowConfig, FlowSolver, Mesh as FlowMesh, PlateSheet, Scheme, ThermalBc,
};
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

/// How the mounting plate enters the 3D model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlateModel {
    /// A thin sheet on the rear wall, one temperature per column, with
    /// in-plane conduction, a contact resistance to the devices standing on
    /// it and a film to the air, as the 2D engine does. Does not depend on
    /// the mesh resolving 2.5 mm.
    #[default]
    Sheet,
    /// A slab of cells. Only right when the mesh resolves the plate.
    Volume,
}

/// How devices exchange heat with the air.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceModel {
    /// A near-isothermal housing ([`LUMPED_DEVICE_K`]) whose outside is a
    /// surface film (the 2D engine's correlation, updated by
    /// [`FlowSetup::update_device_films`]) on every face it shares with air.
    #[default]
    Lumped,
    /// Plain cell conduction in a poorly conducting block. The surface
    /// temperature then depends on how the mesh cuts the device.
    Volume,
}

/// Conductivity of a lumped device, W/mK: housing, heat sink and internal
/// circulation make a real device nearly isothermal; a few kelvin of
/// internal drop remain at tens of watts.
pub const LUMPED_DEVICE_K: f64 = 40.0;
/// First guess of a device film coefficient, W/m2K (the 2D engine starts at 8).
const FILM_START: f64 = 8.0;

/// What to build.
#[derive(Debug, Clone, PartialEq)]
pub struct SolidsSettings {
    pub plate: PlateModel,
    pub device: DeviceModel,
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
            plate: PlateModel::default(),
            device: DeviceModel::default(),
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
    pub fn from_mesh(cm: &mesh::Mesh, scn: &Scenario, st: &SolidsSettings) -> (Medium, f64) {
        let n = cm.grid.cells();
        let mut md = Medium::air(n);
        let speed = st.solid_speedup.max(1.0);
        let duct = &st.duct;
        let fm = FlowMesh::from_grid(&cm.grid);
        // what fills each cell besides air: (material id, share)
        let mut parts: Vec<Vec<(u16, f64)>> = vec![Vec::new(); n];
        for (cell, p) in parts.iter_mut().enumerate() {
            if cm.class[cell] == CellClass::Solid && cm.material[cell] != 0 {
                p.push((cm.material[cell], 1.0));
            }
        }
        for mc in &cm.mixed {
            parts[mc.cell] = mc.parts.iter().copied().filter(|p| p.0 != 0).collect();
        }
        let is_device = |id: u16| {
            matches!(
                cm.materials.get(id as usize).map(|m| m.kind),
                Some(crate::result::MaterialKind::Device(_))
            )
        };
        let lumped = st.device == DeviceModel::Lumped;
        let mut fluid: Vec<f64> = cm.fluid_fraction.clone();
        let mut volume: Vec<f64> = (0..cm.materials.len())
            .map(|i| cm.material_volume_m3.get(i).copied().unwrap_or(0.0))
            .collect();
        if st.plate == PlateModel::Sheet {
            // The plate is the sheet of the solver: take it out of the cells.
            // What is left of a cell is renormalised; a cell that is all
            // plate takes the content of the cell above it.
            let share_of = |p: &[(u16, f64)]| p.iter().find(|x| x.0 == 1).map_or(0.0, |x| x.1);
            let stripped = |p: &[(u16, f64)]| -> Vec<(u16, f64)> {
                let keep = 1.0 - share_of(p);
                p.iter()
                    .filter(|x| x.0 != 1)
                    .map(|x| (x.0, x.1 / keep))
                    .collect()
            };
            let mut next = parts.clone();
            for k in (0..fm.n[2]).rev() {
                for j in 0..fm.n[1] {
                    for i in 0..fm.n[0] {
                        let cell = fm.cell([i, j, k]);
                        let plate = share_of(&parts[cell]);
                        if plate <= 0.0 {
                            continue;
                        }
                        next[cell] = if plate < 1.0 - 1e-9 {
                            stripped(&parts[cell])
                        } else if k + 1 < fm.n[2] {
                            // already resolved: `next` above is final
                            next[fm.cell([i, j, k + 1])].clone()
                        } else {
                            Vec::new()
                        };
                    }
                }
            }
            parts = next;
        }
        if lumped {
            // A device is the block of the cells it fills by more than half
            // (a cut cell is all device or all air): the film and the
            // contact are scaled to the true surface afterwards, so the
            // mesh cutting through the device does not change the result.
            for p in parts.iter_mut() {
                let total: f64 = p.iter().filter(|x| is_device(x.0)).map(|x| x.1).sum();
                if total >= 0.5 {
                    let best = p
                        .iter()
                        .filter(|x| is_device(x.0))
                        .max_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|x| x.0)
                        .unwrap_or(0);
                    *p = vec![(best, 1.0)];
                } else {
                    p.retain(|x| !is_device(x.0));
                }
            }
        }
        if st.plate == PlateModel::Sheet || lumped {
            for (cell, f) in fluid.iter_mut().enumerate() {
                *f = (1.0 - parts[cell].iter().map(|x| x.1).sum::<f64>()).clamp(0.0, 1.0);
            }
            volume.iter_mut().for_each(|v| *v = 0.0);
            for k in 0..fm.n[2] {
                for j in 0..fm.n[1] {
                    for i in 0..fm.n[0] {
                        let cell = fm.cell([i, j, k]);
                        for &(id, sh) in &parts[cell] {
                            if let Some(v) = volume.get_mut(id as usize) {
                                *v += sh * fm.volume([i, j, k]);
                            }
                        }
                    }
                }
            }
        }
        let table: Vec<(u16, super::SolidProps, f64)> = cm
            .materials
            .iter()
            .filter(|m| m.id != 0)
            .map(|m| {
                let vol = volume.get(m.id as usize).copied().unwrap_or(0.0);
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
        // share of each cell that is a device: below half, a cut cell keeps
        // the poorly conducting block so it does not act as a fin
        let dev_share: Vec<f64> = parts
            .iter()
            .map(|p| p.iter().filter(|x| is_device(x.0)).map(|x| x.1).sum())
            .collect();
        let mut add = |cell: usize, id: u16, share: f64| {
            if let Some((_, p, q)) = find(id) {
                if let Some((_, o)) = duct_of(id) {
                    let porosity = duct.porosity.clamp(0.0, 1.0);
                    let solid = 1.0 - porosity;
                    md.c_solid[cell] += share * solid * p.rho_cp / speed;
                    md.k_solid[cell] += share * solid * p.k;
                    extra_air[cell] += share * porosity;
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
                let k = if lumped && is_device(id) && dev_share[cell] >= 0.5 {
                    LUMPED_DEVICE_K
                } else {
                    p.k
                };
                md.c_solid[cell] += share * p.rho_cp / speed;
                md.k_solid[cell] += share * k;
                md.heat_w_m3[cell] += share * q;
            }
        };
        let mut dev_best = vec![0.0f64; n];
        for (cell, p) in parts.iter().enumerate() {
            for &(id, share) in p {
                add(cell, id, share);
                if is_device(id) && share > dev_best[cell] {
                    dev_best[cell] = share;
                    md.dev_slot[cell] = id.saturating_sub(2);
                }
            }
            md.dev_frac[cell] = dev_share[cell].min(1.0);
        }
        for (cell, slot) in md.dev_slot.iter_mut().enumerate() {
            if dev_share[cell] <= 0.0 {
                *slot = NO_DEVICE;
            }
        }
        for (cell, ff) in fluid.iter().enumerate() {
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
    devices: Vec<FilmDevice>,
}

#[derive(Debug, Clone)]
struct FilmDevice {
    id: u64,
}

/// One device in a running solver, see [`FlowSetup::device_report`].
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceReport {
    pub id: u64,
    /// Mean temperature of the device housing, degrees C.
    pub surface_c: f64,
    /// Mean temperature of the air cells touching it, degrees C.
    pub air_c: f64,
    /// Surface film coefficient in use, W/m2K.
    pub film_h: f64,
    /// Heat into the mounting plate through the contact, W.
    pub to_plate_w: f64,
    /// Area of the faces to air that carry the film, m2 (the mesh's
    /// staircase of the device surface).
    pub film_area_m2: f64,
    /// Net heat the device gives off by radiation to the other surfaces, W
    /// (zero when radiation is not coupled, see
    /// [`FlowSetup::enable_radiation`]).
    pub radiation_w: f64,
}

/// The plate as a sheet on the rear face of the grid, if the scenario has a
/// plate and the rear wall loses heat.
fn plate_sheet(scn: &Scenario, cm: &mesh::Mesh, walls: &Walls, speedup: f64) -> Option<PlateSheet> {
    let t = scn.params.plate_thickness_mm / 1000.0;
    if t <= 0.0 || scn.params.plate_k <= 0.0 {
        return None;
    }
    let p = &scn.enclosure.plate;
    let overlap = |lo: f64, hi: f64, a: f64, b: f64| (hi.min(b) - lo.max(a)).max(0.0);
    let g = &cm.grid;
    let mut cover = Vec::with_capacity(g.nx * g.ny);
    for j in 0..g.ny {
        let (y0, y1) = (g.y_edges_mm[j], g.y_edges_mm[j + 1]);
        for i in 0..g.nx {
            let (x0, x1) = (g.x_edges_mm[i], g.x_edges_mm[i + 1]);
            let a = overlap(x0, x1, p.x, p.right()) * overlap(y0, y1, p.y, p.bottom());
            cover.push((a / ((x1 - x0) * (y1 - y0))).clamp(0.0, 1.0));
        }
    }
    if cover.iter().all(|c| *c <= 0.0) {
        return None;
    }
    // Air to plate and plate to outside in series are the rear coefficient of
    // the covered part, exactly as in the 2D engine.
    let kb = walls.h[Face::ZMin as usize];
    let h_air = scn.params.h_in.max(1.2 * kb);
    let u_out = if kb > 0.0 {
        1.0 / (1.0 / kb - 1.0 / h_air)
    } else {
        0.0
    };
    Some(PlateSheet {
        cover,
        kp: scn.params.plate_k * t,
        cap: 7850.0 * 490.0 * t / speedup.max(1.0),
        h_contact: scn.params.plate_contact_h,
        h_air,
        u_out,
        t_out: scn.ambient_c,
    })
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
        let (medium, lost_heat_w) = Medium::from_mesh(&classified, scn, st);
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
        if st.plate == PlateModel::Sheet {
            cfg.plate = plate_sheet(scn, &classified, &walls, st.solid_speedup);
        }
        if st.device == DeviceModel::Lumped {
            cfg.device_film_h = vec![FILM_START; scn.devices.len()];
            let plate = &scn.enclosure.plate;
            let (exposed, contact): (Vec<f64>, Vec<f64>) = scn
                .devices
                .iter()
                .map(|d| {
                    let f = &d.footprint;
                    let foot = f.w * f.h * 1e-6;
                    let on_plate = (f.right().min(plate.right()) - f.x.max(plate.x)).max(0.0)
                        * (f.bottom().min(plate.bottom()) - f.y.max(plate.y)).max(0.0)
                        * 1e-6;
                    let perimeter = 2.0 * (f.w + f.h) * 1e-3;
                    (foot + perimeter * d.height() * 1e-3, on_plate)
                })
                .unzip();
            cfg.device_length_m = scn
                .devices
                .iter()
                .map(|d| (d.footprint.h / 1000.0).max(0.02))
                .collect();
            cfg.device_emissivity = scn.params.emissivity;
            cfg.device_view = scn.params.radiation_view_factor;
            cfg.device_exposed_m2 = exposed;
            cfg.device_contact_m2 = contact;
        }
        cfg.medium = Some(medium);
        Ok(FlowSetup {
            mesh,
            cfg,
            t_init: scn.ambient_c,
            ambient_c: scn.ambient_c,
            lost_heat_w,
            heat_w,
            classified,
            devices: scn
                .devices
                .iter()
                .map(|d| FilmDevice { id: d.id })
                .collect(),
        })
    }

    /// Refreshes the surface film of every device now. The solver does it
    /// itself every few steps or iterations ([`crate::flow::FILM_EVERY`]), so
    /// calling this is only needed to prime a state read before marching.
    pub fn update_device_films(&self, solver: &mut FlowSolver) {
        solver.update_device_films();
    }

    /// What each device is doing in `solver`: temperature, surrounding air and
    /// the heat it gives to the plate. Needs [`DeviceModel::Lumped`] for the
    /// air side; the plate side needs [`PlateModel::Sheet`].
    pub fn device_report(&self, solver: &FlowSolver) -> Vec<DeviceReport> {
        let m = &solver.mesh;
        let md = self.cfg.medium.as_ref().expect("medium");
        let mut out = Vec::new();
        for (slot, d) in self.devices.iter().enumerate() {
            let (mut tv, mut v) = (0.0, 0.0);
            let mut to_plate = 0.0;
            for k in 0..m.n[2] {
                for j in 0..m.n[1] {
                    for i in 0..m.n[0] {
                        let cell = m.cell([i, j, k]);
                        if md.dev_slot[cell] as usize != slot {
                            continue;
                        }
                        let w = md.dev_frac[cell] * m.volume([i, j, k]);
                        tv += w * solver.t[cell];
                        v += w;
                        if k == 0 {
                            let (contact, _) = solver.plate_exchange_w(i + m.n[0] * j);
                            let solid = (1.0 - md.fluid_frac[cell]).max(1e-9);
                            to_plate += contact * (md.dev_frac[cell] / solid).min(1.0);
                        }
                    }
                }
            }
            let state = solver.device_film_state(slot);
            out.push(DeviceReport {
                id: d.id,
                surface_c: if v > 0.0 { tv / v } else { f64::NAN },
                air_c: state.map_or(f64::NAN, |s| s.1),
                film_h: solver
                    .cfg
                    .device_film_h
                    .get(slot)
                    .copied()
                    .unwrap_or(f64::NAN),
                to_plate_w: to_plate,
                film_area_m2: solver.device_film_area_m2(slot),
                radiation_w: self
                    .classified
                    .materials
                    .iter()
                    .find(|m| m.kind == crate::result::MaterialKind::Device(d.id))
                    .map_or(0.0, |m| solver.radiation_power_w(m.id)),
            });
        }
        out
    }

    /// Couples surface-to-surface radiation between the devices, the plate
    /// and the enclosure walls (lane CFD-D). The device films then carry
    /// convection only: the radiation that the 2D engine's film folds in
    /// (against the air temperature) is replaced by the exchange between the
    /// real surfaces, so the same heat is not counted twice. Returns whether
    /// radiation was switched on (`settings.enabled` and at least two
    /// surfaces).
    pub fn enable_radiation(&mut self, scn: &Scenario, settings: &crate::rad::RadSettings) -> bool {
        let mut st = settings.clone();
        st.emissivity.device = scn.params.emissivity;
        let plate = self.cfg.plate.as_ref().map(|p| crate::rad::RearPlate {
            cover: p.cover.clone(),
            material: 1,
            front_z_mm: scn.params.plate_thickness_mm,
        });
        let Some(model) =
            crate::rad::RadiationModel::build_with(&self.classified, &st, plate.as_ref())
        else {
            return false;
        };
        let plan = crate::rad::RadPlan::new(model, self.classified.grid.clone());
        self.cfg.radiation = Some(std::sync::Arc::new(plan));
        self.cfg.device_emissivity = 0.0;
        true
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
