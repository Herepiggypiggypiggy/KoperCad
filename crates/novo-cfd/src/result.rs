//! The result contract (design section 4).
//!
//! Frame of `novo-thermal`: millimetres in the interior, `x` right, `y` down,
//! `z` from the mounting plate towards the door. Cell data is row-major,
//! index `(k * ny + j) * nx + i`.

use novo_thermal::{DeviceResult, HotSpot};

/// A rectilinear grid given by its cell edges (non-uniform allowed).
#[derive(Debug, Clone, PartialEq)]
pub struct Grid3 {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    /// `nx + 1`, `ny + 1`, `nz + 1` increasing edges, mm.
    pub x_edges_mm: Vec<f64>,
    pub y_edges_mm: Vec<f64>,
    pub z_edges_mm: Vec<f64>,
}

impl Grid3 {
    /// Uniform grid over `size_mm = [width, height, depth]`.
    pub fn uniform(n: [usize; 3], size_mm: [f64; 3]) -> Grid3 {
        let edges = |n: usize, len: f64| (0..=n).map(|i| len * i as f64 / n as f64).collect();
        Grid3 {
            nx: n[0],
            ny: n[1],
            nz: n[2],
            x_edges_mm: edges(n[0], size_mm[0]),
            y_edges_mm: edges(n[1], size_mm[1]),
            z_edges_mm: edges(n[2], size_mm[2]),
        }
    }

    pub fn cells(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    pub fn index(&self, i: usize, j: usize, k: usize) -> usize {
        (k * self.ny + j) * self.nx + i
    }

    /// Centre of a cell, mm.
    pub fn centre_mm(&self, i: usize, j: usize, k: usize) -> [f64; 3] {
        let c = |e: &[f64], n: usize| 0.5 * (e[n] + e[n + 1]);
        [
            c(&self.x_edges_mm, i),
            c(&self.y_edges_mm, j),
            c(&self.z_edges_mm, k),
        ]
    }

    /// Cell volume, m3.
    pub fn volume_m3(&self, i: usize, j: usize, k: usize) -> f64 {
        let d = |e: &[f64], n: usize| (e[n + 1] - e[n]) * 1e-3;
        d(&self.x_edges_mm, i) * d(&self.y_edges_mm, j) * d(&self.z_edges_mm, k)
    }

    /// Edges are one more than cells, increasing, and fields fit.
    pub fn is_valid(&self) -> bool {
        let ok = |e: &[f64], n: usize| e.len() == n + 1 && e.windows(2).all(|w| w[1] > w[0]);
        self.nx > 0
            && self.ny > 0
            && self.nz > 0
            && ok(&self.x_edges_mm, self.nx)
            && ok(&self.y_edges_mm, self.ny)
            && ok(&self.z_edges_mm, self.nz)
    }
}

/// How good a snapshot is (design section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fidelity {
    /// The 2D estimate of `novo-thermal`.
    Preview2d,
    /// About 50 000 cells.
    Coarse,
    /// About 500 000 cells.
    Medium,
    /// 1 to 3 million cells, turbulence and radiation on.
    Fine,
}

impl Fidelity {
    /// Target cell count of the level.
    pub fn target_cells(self) -> usize {
        match self {
            Fidelity::Preview2d => 0,
            Fidelity::Coarse => 50_000,
            Fidelity::Medium => 500_000,
            Fidelity::Fine => 2_000_000,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Fidelity::Preview2d => "Preview (2D)",
            Fidelity::Coarse => "Coarse 3D",
            Fidelity::Medium => "Medium 3D",
            Fidelity::Fine => "Fine 3D",
        }
    }
}

/// Normalised residuals of the last iteration.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Residuals {
    pub momentum: f64,
    pub continuity: f64,
    pub energy: f64,
}

impl Residuals {
    pub fn max(&self) -> f64 {
        self.momentum.max(self.continuity).max(self.energy)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub fidelity: Fidelity,
    pub iteration: usize,
    pub residuals: Residuals,
    pub converged: bool,
    pub elapsed_s: f64,
    /// `|heat in - heat out| / heat in`; below 0.001 is the goal.
    pub energy_balance_rel: f64,
    /// Change of the monitored device temperatures over the last window, K.
    pub monitor_drift_k: f64,
}

/// What a material id in [`CfdSnapshot::material`] means (0 is air).
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialInfo {
    pub id: u16,
    pub kind: MaterialKind,
    pub name: String,
    pub conductivity_w_mk: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialKind {
    Air,
    /// Project device id.
    Device(u64),
    Plate,
    Wall,
    Rail,
    Duct,
}

/// A benchmark and the error this build reached on it.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkError {
    pub name: String,
    /// Relative error, as a fraction (0.02 = 2 percent).
    pub error: f64,
}

/// What the app may claim about a result (filled from the validation suite).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccuracyStatement {
    /// True only when the validation suite supports quoting the enclosure
    /// temperatures of this result (a cabinet against IEC 60890 and the 2D
    /// engine, not only the benchmarks of the flow core). The app shows
    /// "not validated" otherwise.
    pub validated: bool,
    pub headline: String,
    pub benchmarks: Vec<BenchmarkError>,
    /// What the model leaves out.
    pub limits: Vec<String>,
}

/// Where the heat of one device goes (convection is what conduction and
/// radiation leave of the total).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceHeatPaths {
    pub id: u64,
    pub total_w: f64,
    /// Into the mounting plate and the solids it touches.
    pub conduction_w: f64,
    /// Net radiation to the other surfaces; zero when radiation is off.
    pub radiation_w: f64,
}

impl DeviceHeatPaths {
    pub fn paths(&self) -> crate::rad::HeatPaths {
        crate::rad::HeatPaths::new(self.total_w, self.conduction_w, self.radiation_w)
    }

    /// Into the air by convection, W.
    pub fn convection_w(&self) -> f64 {
        self.paths().convection_w()
    }
}

/// One published state of the solution.
#[derive(Debug, Clone, PartialEq)]
pub struct CfdSnapshot {
    pub grid: Grid3,
    pub temperature_c: Vec<f32>,
    /// m/s, `y` down.
    pub velocity: Vec<[f32; 3]>,
    pub pressure_pa: Vec<f32>,
    /// Turbulent over molecular viscosity (zero for laminar runs).
    pub turb_visc_ratio: Vec<f32>,
    /// 0 = air, else an id from `materials`.
    pub material: Vec<u16>,
    pub materials: Vec<MaterialInfo>,
    /// On solid faces, for heat flux maps; same length as cells.
    pub wall_flux_w_m2: Vec<f32>,
    pub devices: Vec<DeviceResult>,
    pub hot_spots: Vec<HotSpot>,
    pub progress: Progress,
    pub notes: Vec<String>,
    pub accuracy: AccuracyStatement,
    /// Convection, conduction and radiation per device (empty when the
    /// solver does not split them).
    pub heat_paths: Vec<DeviceHeatPaths>,
    /// Surface-to-surface radiation is part of this result (the Simulate
    /// dialog's Radiation toggle was on and the level supports it).
    pub radiation_on: bool,
}

impl CfdSnapshot {
    /// Every cell field has one value per cell and the grid is valid.
    pub fn is_consistent(&self) -> bool {
        let n = self.grid.cells();
        self.grid.is_valid()
            && self.temperature_c.len() == n
            && self.velocity.len() == n
            && self.pressure_pa.len() == n
            && self.turb_visc_ratio.len() == n
            && self.material.len() == n
            && self.wall_flux_w_m2.len() == n
    }

    pub fn max_temperature_c(&self) -> f32 {
        self.temperature_c
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
    }
}
