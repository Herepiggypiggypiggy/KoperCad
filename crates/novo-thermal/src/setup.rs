//! Turns a [`Scenario`] into grid arrays: blockage, plate coverage, wall
//! conductances, device and opening cells. Everything in SI units from here.

use novo_layout::Rect;
use novo_layout::sizing::Installation;

use crate::model::*;
use crate::result::GridInfo;

/// Why a scenario was refused.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("invalid thermal scenario: {0}")]
    Invalid(String),
}

/// Air properties at the ambient temperature.
#[derive(Debug, Clone, Copy)]
pub struct Air {
    pub rho: f64,
    pub cp: f64,
    pub k: f64,
    pub mu: f64,
    pub beta: f64,
    pub g: f64,
}

impl Air {
    pub fn at(ambient_c: f64) -> Air {
        let t = ambient_c + 273.15;
        Air {
            rho: 101_325.0 / (287.05 * t),
            cp: 1006.0,
            k: 0.026,
            mu: 1.85e-5,
            beta: 1.0 / t,
            g: 9.81,
        }
    }

    /// Heat capacity flow per volume flow, J/(m3 K).
    pub fn rho_cp(&self) -> f64 {
        self.rho * self.cp
    }
}

/// The six wall faces, in the order used for conductances.
pub const FRONT: usize = 0;
pub const REAR: usize = 1;
pub const LEFT: usize = 2;
pub const RIGHT: usize = 3;
pub const TOP: usize = 4;
pub const BOTTOM: usize = 5;

/// IEC 60890 surface factors per face, front, rear, left, right, top, bottom.
pub fn surface_factors(i: Installation) -> [f64; 6] {
    let pick = |covered: bool, free: f64, cov: f64| if covered { cov } else { free };
    [
        0.9,
        pick(i.rear_covered, 0.9, 0.5),
        pick(i.left_covered, 0.9, 0.5),
        pick(i.right_covered, 0.9, 0.5),
        pick(i.top_covered, 1.4, 0.7),
        if i.on_floor { 0.0 } else { 0.5 },
    ]
}

pub struct DeviceCells {
    pub id_index: usize,
    /// Air cells under the footprint with their share of it (sums to 1).
    pub air: Vec<(usize, f64)>,
    /// Plate cells under the footprint with their share of the plate path.
    pub plate: Vec<(usize, f64)>,
    pub footprint_m2: f64,
    pub exposed_m2: f64,
    /// Vertical extent of the footprint, m (for natural convection).
    pub length_m: f64,
    /// Share of the footprint on the plate, 0 to 1.
    pub plate_overlap: f64,
    pub capacity: f64,
    pub loss_w: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum PortKind {
    Vent {
        a_eff_m2: f64,
    },
    Fan {
        q_max: f64,
        p_max: f64,
        intake: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Port {
    pub cell: usize,
    pub kind: PortKind,
    /// Index in `Scenario::vents` or `Scenario::fans`.
    pub index: usize,
    pub is_fan: bool,
    /// Opening area share of this cell, m2.
    pub area_m2: f64,
}

pub struct Setup {
    pub air: Air,
    pub ambient: f64,
    pub grid: GridInfo,
    pub nx: usize,
    pub ny: usize,
    /// Cell size, m.
    pub hx: f64,
    pub hy: f64,
    /// Air depth, m.
    pub depth: f64,
    pub block: Vec<f64>,
    pub plate_frac: Vec<f64>,
    pub plate_idx: Vec<Option<usize>>,
    pub plate_cells: Vec<usize>,
    /// Air to ambient through the walls, W/K per cell and face (rear is only
    /// the part not behind the plate).
    pub g_wall: Vec<[f64; 6]>,
    /// Air to plate, W/K per cell.
    pub g_ap: Vec<f64>,
    /// Plate to ambient through the rear wall, W/K per cell.
    pub g_pa: Vec<f64>,
    pub devices: Vec<DeviceCells>,
    pub ports: Vec<Port>,
    pub params: Params,
    /// k times effective surface, W/K (IEC 60890 whole enclosure).
    pub k_ae: f64,
    pub plate_rect: Rect,
    pub on_plate: Vec<bool>,
    /// Heat capacity of an air cell, J/K, per cell (open volume).
    pub c_air: Vec<f64>,
    pub c_plate: Vec<f64>,
}

fn validate(s: &Scenario) -> Result<(), Error> {
    let e = &s.enclosure;
    let bad = |msg: &str| Err(Error::Invalid(msg.to_string()));
    let finite = |v: f64| v.is_finite();
    if !(finite(e.width_mm) && finite(e.height_mm) && finite(e.depth_mm) && finite(e.wall_mm)) {
        return bad("enclosure size is not a number");
    }
    if e.interior_width_mm() < 50.0 || e.interior_height_mm() < 50.0 || e.interior_depth_mm() < 20.0
    {
        return bad("enclosure interior is too small");
    }
    if !finite(s.ambient_c) || !(-60.0..=120.0).contains(&s.ambient_c) {
        return bad("ambient temperature is out of range");
    }
    for d in &s.devices {
        if !(finite(d.loss_w) && d.loss_w >= 0.0) {
            return bad("a device loss is negative or not a number");
        }
        if !(d.footprint.w > 0.0 && d.footprint.h > 0.0) {
            return bad("a device has no footprint");
        }
        if !(finite(d.footprint.x) && finite(d.footprint.y)) {
            return bad("a device position is not a number");
        }
    }
    for f in &s.fans {
        if !(f.free_flow_m3h > 0.0 && f.max_pressure_pa > 0.0) {
            return bad("a fan needs a positive free flow and maximum pressure");
        }
    }
    for v in &s.vents {
        if !(v.free_area_fraction > 0.0 && v.free_area_fraction <= 1.0) {
            return bad("a vent needs a free area between 0 and 1");
        }
    }
    let openings = s
        .vents
        .iter()
        .map(|v| &v.opening)
        .chain(s.fans.iter().map(|f| &f.opening));
    for o in openings {
        if !(o.width_mm > 0.0 && o.height_mm > 0.0) {
            return bad("an opening has no size");
        }
    }
    Ok(())
}

/// Overlap area of two rectangles in mm2.
fn overlap(a: &Rect, b: &Rect) -> f64 {
    let ox = (a.right().min(b.right()) - a.x.max(b.x)).max(0.0);
    let oy = (a.bottom().min(b.bottom()) - a.y.max(b.y)).max(0.0);
    ox * oy
}

impl Setup {
    pub fn new(s: &Scenario) -> Result<Setup, Error> {
        validate(s)?;
        let p = &s.params;
        let e = &s.enclosure;
        let (w_mm, h_mm) = (e.interior_width_mm(), e.interior_height_mm());
        let mut cell = p.cell_mm.max(2.0);
        while (w_mm / cell).round().max(4.0) * (h_mm / cell).round().max(4.0) > p.max_cells as f64 {
            cell *= 1.2;
        }
        let nx = (w_mm / cell).round().max(4.0) as usize;
        let ny = (h_mm / cell).round().max(4.0) as usize;
        let (hx_mm, hy_mm) = (w_mm / nx as f64, h_mm / ny as f64);
        let n = nx * ny;
        let grid = GridInfo {
            cols: nx,
            rows: ny,
            cell_w_mm: hx_mm,
            cell_h_mm: hy_mm,
            width_mm: w_mm,
            height_mm: h_mm,
        };
        let air = Air::at(s.ambient_c);
        let depth = e.interior_depth_mm() / 1000.0;
        let (hx, hy) = (hx_mm / 1000.0, hy_mm / 1000.0);
        let cell_rect = |c: usize, r: usize| Rect {
            x: c as f64 * hx_mm,
            y: r as f64 * hy_mm,
            w: hx_mm,
            h: hy_mm,
        };
        let cell_area_mm2 = hx_mm * hy_mm;
        // Cells a rectangle touches, with the share of the cell it covers.
        let cells_of = |rect: &Rect| -> Vec<(usize, f64)> {
            let c0 = (rect.x / hx_mm).floor().max(0.0) as usize;
            let c1 = ((rect.right() / hx_mm).ceil() as usize).min(nx);
            let r0 = (rect.y / hy_mm).floor().max(0.0) as usize;
            let r1 = ((rect.bottom() / hy_mm).ceil() as usize).min(ny);
            let mut out = Vec::new();
            for r in r0..r1 {
                for c in c0..c1 {
                    let a = overlap(&cell_rect(c, r), rect);
                    if a > 0.0 {
                        out.push((r * nx + c, a / cell_area_mm2));
                    }
                }
            }
            out
        };

        // Blockage.
        let mut block = vec![0.0; n];
        for ob in &s.obstacles {
            for (i, f) in cells_of(&ob.rect) {
                block[i] += f * ob.blockage().clamp(0.0, 1.0);
            }
        }
        for d in &s.devices {
            let b = (d.height() / e.interior_depth_mm()).clamp(0.0, 1.0);
            for (i, f) in cells_of(&d.footprint) {
                block[i] += f * b;
            }
        }
        for b in &mut block {
            *b = b.min(1.0);
        }

        // Plate.
        let mut plate_frac = vec![0.0; n];
        for (i, f) in cells_of(&e.plate) {
            plate_frac[i] = f.min(1.0);
        }
        let mut plate_idx = vec![None; n];
        let mut plate_cells = Vec::new();
        let mut on_plate = vec![false; n];
        for i in 0..n {
            if plate_frac[i] > 0.05 {
                plate_idx[i] = Some(plate_cells.len());
                plate_cells.push(i);
                on_plate[i] = true;
            } else {
                plate_frac[i] = 0.0;
            }
        }

        // Walls: conductance k b A of the outside faces, spread over the
        // cells next to each face, so the sum is the IEC 60890 k times the
        // effective surface.
        let k = e.material.k() * p.wall_k_scale;
        let b = surface_factors(e.installation);
        let (wo, ho, do_) = (
            e.width_mm / 1000.0,
            e.height_mm / 1000.0,
            e.depth_mm / 1000.0,
        );
        let k_ae = k
            * (b[FRONT] * wo * ho
                + b[REAR] * wo * ho
                + b[LEFT] * do_ * ho
                + b[RIGHT] * do_ * ho
                + b[TOP] * wo * do_
                + b[BOTTOM] * wo * do_);
        let mut g_wall = vec![[0.0; 6]; n];
        let mut g_ap = vec![0.0; n];
        let mut g_pa = vec![0.0; n];
        let cell_share = 1.0 / n as f64;
        let h_in = p.h_in;
        for r in 0..ny {
            for c in 0..nx {
                let i = r * nx + c;
                let g = &mut g_wall[i];
                g[FRONT] = k * b[FRONT] * wo * ho * cell_share;
                let g_rear = k * b[REAR] * wo * ho * cell_share;
                if plate_frac[i] > 0.0 {
                    let f = plate_frac[i];
                    g[REAR] = g_rear * (1.0 - f);
                    // Air to plate and plate to outside in series equal the
                    // rear coefficient of the covered part, exactly.
                    let kb = g_rear / (hx * hy);
                    let h_air = h_in.max(1.2 * kb);
                    g_ap[i] = h_air * hx * hy * f;
                    g_pa[i] = (1.0 / (1.0 / kb - 1.0 / h_air)) * hx * hy * f;
                } else {
                    g[REAR] = g_rear;
                }
                if c == 0 {
                    g[LEFT] = k * b[LEFT] * do_ * ho * (hy / (hy * ny as f64));
                }
                if c == nx - 1 {
                    g[RIGHT] = k * b[RIGHT] * do_ * ho * (hy / (hy * ny as f64));
                }
                if r == 0 {
                    g[TOP] = k * b[TOP] * wo * do_ * (hx / (hx * nx as f64));
                }
                if r == ny - 1 {
                    g[BOTTOM] = k * b[BOTTOM] * wo * do_ * (hx / (hx * nx as f64));
                }
            }
        }

        // Devices.
        let mut devices = Vec::new();
        for (index, d) in s.devices.iter().enumerate() {
            let foot_mm2 = d.footprint.area();
            let cells = cells_of(&d.footprint);
            let total: f64 = cells.iter().map(|c| c.1).sum();
            if total <= 0.0 {
                return Err(Error::Invalid(format!(
                    "device {} is outside the enclosure",
                    d.id
                )));
            }
            let air_w: Vec<(usize, f64)> = cells.iter().map(|&(i, f)| (i, f / total)).collect();
            // Plate path: footprint share over plate cells.
            let mut plate_w: Vec<(usize, f64)> = cells
                .iter()
                .filter(|&&(i, _)| plate_idx[i].is_some())
                .map(|&(i, f)| (i, f * plate_frac[i]))
                .collect();
            let ptotal: f64 = plate_w.iter().map(|c| c.1).sum();
            let plate_overlap = (ptotal / total).clamp(0.0, 1.0);
            for c in &mut plate_w {
                c.1 /= ptotal.max(1e-12);
            }
            let footprint_m2 = foot_mm2 / 1.0e6;
            let perimeter_m = 2.0 * (d.footprint.w + d.footprint.h) / 1000.0;
            let exposed_m2 = footprint_m2 + perimeter_m * d.height() / 1000.0;
            let capacity = d
                .heat_capacity_j_per_k
                .unwrap_or(500.0 * footprint_m2 * 100.0)
                .max(1.0);
            devices.push(DeviceCells {
                id_index: index,
                air: air_w,
                plate: plate_w,
                footprint_m2,
                exposed_m2,
                length_m: (d.footprint.h / 1000.0).max(0.02),
                plate_overlap,
                capacity,
                loss_w: d.loss_w,
            });
        }

        // Openings.
        let mut ports = Vec::new();
        let mut add_opening =
            |o: &Opening, index: usize, is_fan: bool, kind_of: &dyn Fn(f64) -> PortKind| {
                let area_total = o.area_m2();
                let cells: Vec<(usize, f64)> = match o.placement {
                    Placement::Top { x_mm } => span_cells(x_mm, o.width_mm, hx_mm, nx)
                        .into_iter()
                        .map(|(c, f)| (c, f))
                        .collect(),
                    Placement::Bottom { x_mm } => span_cells(x_mm, o.width_mm, hx_mm, nx)
                        .into_iter()
                        .map(|(c, f)| ((ny - 1) * nx + c, f))
                        .collect(),
                    Placement::Left { y_mm } => span_cells(y_mm, o.width_mm, hy_mm, ny)
                        .into_iter()
                        .map(|(r, f)| (r * nx, f))
                        .collect(),
                    Placement::Right { y_mm } => span_cells(y_mm, o.width_mm, hy_mm, ny)
                        .into_iter()
                        .map(|(r, f)| (r * nx + nx - 1, f))
                        .collect(),
                    Placement::Front { x_mm, y_mm } => {
                        let rect = Rect {
                            x: x_mm - o.width_mm / 2.0,
                            y: y_mm - o.height_mm / 2.0,
                            w: o.width_mm,
                            h: o.height_mm,
                        };
                        let cs = cells_of(&rect);
                        let tot: f64 = cs.iter().map(|c| c.1).sum();
                        cs.into_iter()
                            .map(|(i, f)| (i, f / tot.max(1e-12)))
                            .collect()
                    }
                };
                for (cell, share) in cells {
                    let a = area_total * share;
                    ports.push(Port {
                        cell,
                        kind: kind_of(share),
                        index,
                        is_fan,
                        area_m2: a,
                    });
                }
            };
        for (i, v) in s.vents.iter().enumerate() {
            let cd = p.discharge_coefficient;
            let fa = v.free_area_fraction;
            let area = v.opening.area_m2();
            add_opening(&v.opening, i, false, &|share| PortKind::Vent {
                a_eff_m2: cd * fa * area * share,
            });
        }
        for (i, f) in s.fans.iter().enumerate() {
            let q = f.free_flow_m3h / 3600.0;
            let intake = f.mode == FanMode::Intake;
            add_opening(&f.opening, i, true, &|share| PortKind::Fan {
                q_max: q * share,
                p_max: f.max_pressure_pa,
                intake,
            });
        }
        if ports.iter().any(|pt| match pt.kind {
            PortKind::Vent { a_eff_m2 } => a_eff_m2 <= 0.0,
            PortKind::Fan { .. } => false,
        }) {
            return Err(Error::Invalid("a vent has no free area".into()));
        }

        // Heat capacities.
        let rho_cp = air.rho_cp();
        // The walls store heat too; their temperature is taken to follow the
        // air next to them, so their capacity sits on the air cells.
        let wall_rho_cp = match e.material {
            novo_layout::sizing::Material::Steel | novo_layout::sizing::Material::Stainless => {
                7850.0 * 490.0
            }
            novo_layout::sizing::Material::Aluminium => 2700.0 * 900.0,
            novo_layout::sizing::Material::Plastic => 1500.0 * 1200.0,
        };
        let wall_area = 2.0 * (wo * ho + wo * do_ + ho * do_);
        let wall_c = wall_rho_cp * e.wall_mm / 1000.0 * wall_area / n as f64;
        let c_air: Vec<f64> = (0..n)
            .map(|i| rho_cp * hx * hy * depth * (1.0 - block[i]).max(0.02) + wall_c)
            .collect();
        let c_plate: Vec<f64> = (0..n)
            .map(|i| 7850.0 * 490.0 * p.plate_thickness_mm / 1000.0 * hx * hy * plate_frac[i])
            .collect();

        Ok(Setup {
            air,
            ambient: s.ambient_c,
            grid,
            nx,
            ny,
            hx,
            hy,
            depth,
            block,
            plate_frac,
            plate_idx,
            plate_cells,
            g_wall,
            g_ap,
            g_pa,
            devices,
            ports,
            params: p.clone(),
            k_ae,
            plate_rect: e.plate,
            on_plate,
            c_air,
            c_plate,
        })
    }

    pub fn n(&self) -> usize {
        self.nx * self.ny
    }
}

/// Cells along a wall covered by a span, with the share of the span each
/// takes.
fn span_cells(centre_mm: f64, width_mm: f64, cell_mm: f64, count: usize) -> Vec<(usize, f64)> {
    let (a, b) = (centre_mm - width_mm / 2.0, centre_mm + width_mm / 2.0);
    let c0 = (a / cell_mm).floor().max(0.0) as usize;
    let c1 = ((b / cell_mm).ceil() as usize).min(count);
    let mut out = Vec::new();
    for c in c0..c1 {
        let lo = (c as f64 * cell_mm).max(a);
        let hi = ((c + 1) as f64 * cell_mm).min(b);
        if hi > lo {
            out.push((c, hi - lo));
        }
    }
    let total: f64 = out.iter().map(|e| e.1).sum();
    if total <= 0.0 {
        // Off the wall: use the nearest end cell.
        let c = if centre_mm < 0.0 { 0 } else { count - 1 };
        return vec![(c, 1.0)];
    }
    out.into_iter().map(|(c, l)| (c, l / total)).collect()
}
