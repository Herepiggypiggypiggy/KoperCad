//! What the simulation returns: the contract with the drawing code.
//!
//! Same frame as [`crate::model`]: millimetres in the interior frame, `y`
//! down. Vector fields use the same axes (positive `vy` points down the
//! screen, so rising air has negative `vy`). Every field is sampled at cell
//! centres on the grid in [`GridInfo`].

use novo_layout::Rect;

/// Note shown next to every result.
pub const NOTE: &str = "Estimate from a reduced-order model. Check with the enclosure or cooling unit maker.";

/// The grid every field lives on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridInfo {
    pub cols: usize,
    pub rows: usize,
    /// Cell size, mm (cells are nearly square).
    pub cell_w_mm: f64,
    pub cell_h_mm: f64,
    /// The interior the grid covers, mm.
    pub width_mm: f64,
    pub height_mm: f64,
}

impl GridInfo {
    pub fn cells(&self) -> usize {
        self.cols * self.rows
    }
    /// Centre of a cell, mm.
    pub fn centre(&self, col: usize, row: usize) -> (f64, f64) {
        (
            (col as f64 + 0.5) * self.cell_w_mm,
            (row as f64 + 0.5) * self.cell_h_mm,
        )
    }
}

/// A scalar per cell, row by row from the top left.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub grid: GridInfo,
    pub values: Vec<f64>,
}

impl Field {
    pub fn at(&self, col: usize, row: usize) -> f64 {
        self.values[row * self.grid.cols + col]
    }

    pub fn min(&self) -> f64 {
        self.values.iter().copied().fold(f64::INFINITY, f64::min)
    }

    pub fn max(&self) -> f64 {
        self.values
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn mean(&self) -> f64 {
        self.values.iter().sum::<f64>() / self.values.len().max(1) as f64
    }

    /// Bilinear value at a point in mm (clamped to the grid), for smooth
    /// drawing between cell centres.
    pub fn sample(&self, x_mm: f64, y_mm: f64) -> f64 {
        let g = &self.grid;
        let fx = (x_mm / g.cell_w_mm - 0.5).clamp(0.0, (g.cols - 1) as f64);
        let fy = (y_mm / g.cell_h_mm - 0.5).clamp(0.0, (g.rows - 1) as f64);
        let (c0, r0) = (fx.floor() as usize, fy.floor() as usize);
        let (c1, r1) = ((c0 + 1).min(g.cols - 1), (r0 + 1).min(g.rows - 1));
        let (tx, ty) = (fx - c0 as f64, fy - r0 as f64);
        let top = self.at(c0, r0) * (1.0 - tx) + self.at(c1, r0) * tx;
        let bottom = self.at(c0, r1) * (1.0 - tx) + self.at(c1, r1) * tx;
        top * (1.0 - ty) + bottom * ty
    }
}

/// A vector per cell.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorField {
    pub grid: GridInfo,
    pub vx: Vec<f64>,
    pub vy: Vec<f64>,
}

impl VectorField {
    pub fn at(&self, col: usize, row: usize) -> (f64, f64) {
        let i = row * self.grid.cols + col;
        (self.vx[i], self.vy[i])
    }

    pub fn magnitude(&self, col: usize, row: usize) -> f64 {
        let (x, y) = self.at(col, row);
        x.hypot(y)
    }

    pub fn max_magnitude(&self) -> f64 {
        self.vx
            .iter()
            .zip(&self.vy)
            .map(|(x, y)| x.hypot(*y))
            .fold(0.0, f64::max)
    }

    /// Bilinear vector at a point in mm.
    pub fn sample(&self, x_mm: f64, y_mm: f64) -> (f64, f64) {
        let g = &self.grid;
        let fx = (x_mm / g.cell_w_mm - 0.5).clamp(0.0, (g.cols - 1) as f64);
        let fy = (y_mm / g.cell_h_mm - 0.5).clamp(0.0, (g.rows - 1) as f64);
        let (c0, r0) = (fx.floor() as usize, fy.floor() as usize);
        let (c1, r1) = ((c0 + 1).min(g.cols - 1), (r0 + 1).min(g.rows - 1));
        let (tx, ty) = (fx - c0 as f64, fy - r0 as f64);
        let lerp = |v: &Vec<f64>| {
            let a = v[r0 * g.cols + c0] * (1.0 - tx) + v[r0 * g.cols + c1] * tx;
            let b = v[r1 * g.cols + c0] * (1.0 - tx) + v[r1 * g.cols + c1] * tx;
            a * (1.0 - ty) + b * ty
        };
        (lerp(&self.vx), lerp(&self.vy))
    }

    /// Streamlines of the field: for every seed (mm) a polyline followed
    /// downstream with a midpoint step of `step_mm` until it leaves the
    /// interior, stalls (speed below `min_speed`) or has `max_steps` points.
    /// Seeds in still air give a one point line, which the caller can drop.
    pub fn streamlines(
        &self,
        seeds: &[(f64, f64)],
        step_mm: f64,
        max_steps: usize,
        min_speed: f64,
    ) -> Vec<Vec<(f64, f64)>> {
        let (w, h) = (self.grid.width_mm, self.grid.height_mm);
        seeds
            .iter()
            .map(|&seed| {
                let mut line = vec![seed];
                let mut p = seed;
                for _ in 0..max_steps {
                    let (vx, vy) = self.sample(p.0, p.1);
                    let speed = vx.hypot(vy);
                    if speed < min_speed {
                        break;
                    }
                    let mid = (
                        p.0 + vx / speed * step_mm * 0.5,
                        p.1 + vy / speed * step_mm * 0.5,
                    );
                    let (mx, my) = self.sample(mid.0, mid.1);
                    let ms = mx.hypot(my);
                    if ms < min_speed {
                        break;
                    }
                    p = (p.0 + mx / ms * step_mm, p.1 + my / ms * step_mm);
                    if p.0 < 0.0 || p.1 < 0.0 || p.0 > w || p.1 > h {
                        break;
                    }
                    line.push(p);
                }
                line
            })
            .collect()
    }
}

/// How a device stands against its maker's limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// No limit known for the device.
    NoLimit,
    /// Within the limit, also at the pessimistic end of the uncertainty band.
    Ok,
    /// Within the limit at the central estimate but over it at the
    /// pessimistic end.
    Marginal,
    /// Over the limit at the central estimate.
    Over,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceResult {
    pub id: u64,
    pub footprint: Rect,
    pub loss_w: f64,
    /// Mean housing surface temperature, degrees C.
    pub surface_c: f64,
    /// Mean air temperature around the device, degrees C.
    pub air_c: f64,
    /// Surface heat transfer coefficient used, W/m2K.
    pub h_w_m2k: f64,
    /// Share of the loss that left through the mounting plate, 0 to 1.
    pub plate_share: f64,
    /// Uncertainty band of `surface_c` and `air_c`.
    pub surface_band_c: (f64, f64),
    pub air_band_c: (f64, f64),
    pub rated_max_ambient_c: Option<f64>,
    pub rated_max_surface_c: Option<f64>,
    /// Smallest margin to a limit at the central estimate, K (negative =
    /// over). `None` when no limit is known.
    pub margin_k: Option<f64>,
    /// The same at the pessimistic end of the band.
    pub margin_pessimistic_k: Option<f64>,
    pub status: Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotSpotKind {
    /// A device over or near its limit.
    Device(u64),
    /// A pocket of hot air.
    Air,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HotSpot {
    pub kind: HotSpotKind,
    pub x_mm: f64,
    pub y_mm: f64,
    pub temp_c: f64,
    /// How much hotter than the mean air, K.
    pub above_mean_k: f64,
    /// Plain-language reason, e.g. "air trapped above the drive".
    pub reason: String,
}

/// A path through the enclosure of one opening's flow.
#[derive(Debug, Clone, PartialEq)]
pub struct OpeningFlow {
    /// Index in `Scenario::vents` or `Scenario::fans`.
    pub index: usize,
    pub is_fan: bool,
    /// Volume flow, m3/h; positive = into the enclosure.
    pub flow_m3h: f64,
    /// Mean air speed through the opening, m/s.
    pub speed_ms: f64,
    /// Pressure rise at the opening against outside, Pa (fan operating point).
    pub pressure_pa: f64,
}

/// Energy balance of the whole enclosure, W. Closes by construction up to
/// the solver tolerance; `residual_w` shows how well.
#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    /// Heat made by the devices.
    pub loss_w: f64,
    /// Through the walls, per face.
    pub front_w: f64,
    pub rear_w: f64,
    pub left_w: f64,
    pub right_w: f64,
    pub top_w: f64,
    pub bottom_w: f64,
    /// Carried out by air through vents and fans (relative to ambient).
    pub openings_w: f64,
    /// Stored (zero in a steady run; heating rate in a transient one).
    pub storage_w: f64,
    /// `loss_w` minus everything that left or was stored.
    pub residual_w: f64,
    pub residual_rel: f64,
    pub mean_air_c: f64,
    pub max_air_c: f64,
    /// Mean air rise over ambient, K, and the IEC 60890 style estimate for
    /// comparison (loss over k times effective surface).
    pub mean_rise_k: f64,
    pub iec_rise_k: f64,
}

/// What was assumed, with the range used for the uncertainty band.
#[derive(Debug, Clone, PartialEq)]
pub struct Assumption {
    pub name: &'static str,
    pub value: f64,
    pub unit: &'static str,
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advice {
    /// Move a device to a cooler spot of the plate.
    MoveDevice { id: u64, to_x_mm: i64, to_y_mm: i64 },
    /// Add an intake fan low and an exhaust fan high.
    AddFanPair { flow_m3h: i64 },
    /// Add a bottom and a top vent for chimney flow.
    AddVentPair { area_cm2: i64 },
}

/// A suggestion that was tried in the model.
#[derive(Debug, Clone, PartialEq)]
pub struct Recommendation {
    pub advice: Advice,
    /// Sentence for the user.
    pub text: String,
    /// Hottest device margin before and after (K, negative = over); `None`
    /// when no device has a limit.
    pub worst_margin_before_k: Option<f64>,
    pub worst_margin_after_k: Option<f64>,
    /// Hottest air in the enclosure before and after, degrees C.
    pub max_air_before_c: f64,
    pub max_air_after_c: f64,
    /// Best first by the margin it wins.
    pub rank: usize,
}

/// One point of a transient run.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub t_s: f64,
    pub mean_air_c: f64,
    pub max_air_c: f64,
    /// Surface temperature per device, in `Scenario::devices` order.
    pub device_c: Vec<f64>,
    pub loss_w: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub t_s: f64,
    pub air_temp: Field,
    pub plate_temp: Field,
    pub velocity: VectorField,
}

/// The full answer of a steady run (and the last state of a transient run).
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalResult {
    pub grid: GridInfo,
    pub ambient_c: f64,
    /// Air temperature, degrees C.
    pub air_temp: Field,
    /// Mounting plate temperature; meaningful where `on_plate` is true.
    pub plate_temp: Field,
    pub on_plate: Vec<bool>,
    /// Air speed (open path, depth averaged), m/s.
    pub velocity: VectorField,
    /// Conduction in the plate, W/m (heat per metre of cut across the flux).
    pub plate_flux: VectorField,
    /// Enthalpy carried by the moving air relative to ambient, W/m.
    pub air_flux: VectorField,
    pub devices: Vec<DeviceResult>,
    pub hot_spots: Vec<HotSpot>,
    pub recommendations: Vec<Recommendation>,
    pub balance: Balance,
    pub openings: Vec<OpeningFlow>,
    /// Fastest air in the enclosure, m/s.
    pub max_speed_ms: f64,
    /// Air temperature at 0.5 and 1.0 of the interior height measured from
    /// the bottom, at the mean over the width (as in IEC 60890), degrees C.
    pub air_at_half_height_c: f64,
    pub air_at_top_c: f64,
    pub assumptions: Vec<Assumption>,
    /// The honesty line and further caveats for this run.
    pub notes: Vec<String>,
    /// Solver iterations used.
    pub iterations: usize,
    pub converged: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransientResult {
    pub samples: Vec<Sample>,
    pub snapshots: Vec<Snapshot>,
    /// The state at the end, in full.
    pub last: ThermalResult,
    /// Time for the mean air rise to reach 63 percent of its final value in
    /// a constant load run, s; `None` when it was not reached.
    pub time_constant_s: Option<f64>,
}
