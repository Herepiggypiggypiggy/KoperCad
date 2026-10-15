//! Inputs of the thermal simulation: plain numbers, no project types.
//!
//! **Frame.** Everything is in millimetres in the *interior frame*: origin at
//! the top left of the enclosure interior seen from the front with the door
//! removed, `x` to the right, `y` **down** (the direction gravity pulls), the
//! same orientation as the Cabinet canvas. The caller converts plate
//! coordinates by adding the plate origin. Powers are watts, temperatures
//! degrees Celsius.

use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};

/// The enclosure and its mounting plate.
#[derive(Debug, Clone, PartialEq)]
pub struct Enclosure {
    /// Outside dimensions, mm. They set the cooling surface (IEC 60890
    /// effective surface, as in `novo_layout::sizing`).
    pub width_mm: f64,
    pub height_mm: f64,
    pub depth_mm: f64,
    /// Wall thickness, mm; the interior is the outside minus this on each side.
    pub wall_mm: f64,
    pub material: Material,
    pub installation: Installation,
    /// The mounting plate in the interior frame.
    pub plate: Rect,
}

impl Enclosure {
    pub fn interior_width_mm(&self) -> f64 {
        self.width_mm - 2.0 * self.wall_mm
    }
    pub fn interior_height_mm(&self) -> f64 {
        self.height_mm - 2.0 * self.wall_mm
    }
    pub fn interior_depth_mm(&self) -> f64 {
        self.depth_mm - 2.0 * self.wall_mm
    }
}

/// One heat-producing device on the plate.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    /// The caller's id (the project device id), echoed in the results.
    pub id: u64,
    /// Footprint on the plate, interior frame.
    pub footprint: Rect,
    /// How far it stands out from the plate, mm. `None` = 60 mm.
    pub height_mm: Option<f64>,
    /// Effective power loss, W (rated loss times load factor).
    pub loss_w: f64,
    /// Highest air temperature around the device the maker allows (the usual
    /// "ambient operating temperature" of a datasheet), degrees C.
    pub rated_max_ambient_c: Option<f64>,
    /// Highest housing surface temperature allowed, degrees C.
    pub rated_max_surface_c: Option<f64>,
    /// Heat capacity, J/K, for the transient run. `None` = estimated from the
    /// footprint (500 J/K per dm2).
    pub heat_capacity_j_per_k: Option<f64>,
}

impl Device {
    pub fn height(&self) -> f64 {
        self.height_mm.unwrap_or(60.0)
    }
}

/// Something that blocks or slows the air without making heat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObstacleKind {
    /// Slotted wire duct: air passes slowly through the slots.
    Duct,
    /// DIN rail or other low profile part.
    Rail,
    /// A solid partition or bulky part.
    Solid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Obstacle {
    pub kind: ObstacleKind,
    pub rect: Rect,
    /// Share of the air depth it blocks, 0 to 1. `None` = typical for the
    /// kind (duct 0.7, rail 0.15, solid 1.0).
    pub blockage: Option<f64>,
}

impl Obstacle {
    pub fn blockage(&self) -> f64 {
        self.blockage.unwrap_or(match self.kind {
            ObstacleKind::Duct => 0.7,
            ObstacleKind::Rail => 0.15,
            ObstacleKind::Solid => 1.0,
        })
    }
}

/// Where an opening is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// In the top wall, centred at `x_mm`.
    Top { x_mm: f64 },
    /// In the bottom wall, centred at `x_mm`.
    Bottom { x_mm: f64 },
    /// In the left wall, centred at `y_mm`.
    Left { y_mm: f64 },
    /// In the right wall, centred at `y_mm`.
    Right { y_mm: f64 },
    /// In the door (or rear wall), centred at (`x_mm`, `y_mm`).
    Front { x_mm: f64, y_mm: f64 },
}

/// Size of an opening: `width_mm` along x (or along y for a left or right
/// wall), `height_mm` the other way (for a top or bottom wall: the depth).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opening {
    pub placement: Placement,
    pub width_mm: f64,
    pub height_mm: f64,
}

impl Opening {
    pub fn area_m2(&self) -> f64 {
        self.width_mm * self.height_mm / 1.0e6
    }
}

/// A passive ventilation opening (grille, louvre).
#[derive(Debug, Clone, PartialEq)]
pub struct Vent {
    pub opening: Opening,
    /// Free share of the opening, 0 to 1 (a louvred grille is 0.5 to 0.7).
    pub free_area_fraction: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanMode {
    /// Blows outside air in.
    Intake,
    /// Draws inside air out.
    Exhaust,
}

/// A fan with a straight-line curve from (0 flow, `max_pressure_pa`) to
/// (`free_flow_m3h`, 0 pressure), the way a maker's datasheet is read at the
/// two ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Fan {
    pub opening: Opening,
    pub mode: FanMode,
    pub free_flow_m3h: f64,
    pub max_pressure_pa: f64,
}

/// Modelling choices and the rough values behind them. Every field is a
/// stated assumption; `Params::cooling_favourable` and
/// `Params::cooling_unfavourable` give the two ends of the range used for
/// the uncertainty band.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    /// Grid cell size, mm. Coarsened when the grid would exceed `max_cells`.
    pub cell_mm: f64,
    pub max_cells: usize,
    /// Drag of the loose clutter (wires, rails, bundles) on the air, per metre
    /// of path. Quadratic: pressure drop = rho * c * u^2 per metre.
    pub clutter_drag_per_m: f64,
    /// Unresolved mixing (3D eddies the 2D slice cannot show), m2/s.
    pub mixing_m2s: f64,
    /// Plate-to-air heat transfer coefficient (convection plus radiation), W/m2K.
    pub h_in: f64,
    /// Mounting plate thickness, mm, and conductivity, W/mK.
    pub plate_thickness_mm: f64,
    pub plate_k: f64,
    /// Conductance from device footprint to plate, W/m2K of footprint.
    pub plate_contact_h: f64,
    /// Surface emissivity of devices and share of the radiation that finds a
    /// surface at about the surroundings' temperature.
    pub emissivity: f64,
    pub radiation_view_factor: f64,
    /// Discharge coefficient of vents and fan openings.
    pub discharge_coefficient: f64,
    /// Multiplier on the wall heat transfer coefficient (IEC 60890 k).
    pub wall_k_scale: f64,
    /// Solver stop: largest temperature change between iterations, K.
    pub tolerance_k: f64,
    pub max_iterations: usize,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            cell_mm: 20.0,
            max_cells: 25_000,
            clutter_drag_per_m: 1.5,
            mixing_m2s: 2.0e-3,
            h_in: 7.0,
            plate_thickness_mm: 2.5,
            plate_k: 50.0,
            plate_contact_h: 15.0,
            emissivity: 0.85,
            radiation_view_factor: 0.5,
            discharge_coefficient: 0.6,
            wall_k_scale: 1.0,
            tolerance_k: 0.01,
            max_iterations: 60,
        }
    }
}

impl Params {
    /// The end of the plausible range that cools best.
    pub fn cooling_favourable(&self) -> Params {
        Params {
            clutter_drag_per_m: self.clutter_drag_per_m * 0.5,
            mixing_m2s: self.mixing_m2s * 2.0,
            h_in: self.h_in * 1.4,
            plate_contact_h: self.plate_contact_h * 2.0,
            radiation_view_factor: (self.radiation_view_factor * 1.5).min(1.0),
            wall_k_scale: self.wall_k_scale * 1.1,
            ..self.clone()
        }
    }

    /// The end of the plausible range that cools worst.
    pub fn cooling_unfavourable(&self) -> Params {
        Params {
            clutter_drag_per_m: self.clutter_drag_per_m * 2.0,
            mixing_m2s: self.mixing_m2s * 0.5,
            h_in: self.h_in * 0.6,
            plate_contact_h: self.plate_contact_h * 0.5,
            radiation_view_factor: self.radiation_view_factor * 0.5,
            wall_k_scale: self.wall_k_scale * 0.9,
            ..self.clone()
        }
    }
}

/// Everything the simulation needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Scenario {
    pub enclosure: Enclosure,
    /// Air temperature outside, degrees C (use the highest ambient).
    pub ambient_c: f64,
    pub devices: Vec<Device>,
    pub obstacles: Vec<Obstacle>,
    pub vents: Vec<Vent>,
    pub fans: Vec<Fan>,
    pub params: Params,
}

/// How a transient run starts and runs.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientSettings {
    pub duration_s: f64,
    /// Number of time steps (implicit, so any number is stable).
    pub steps: usize,
    /// Start from the steady state at `initial_load` instead of a cold
    /// cabinet at ambient.
    pub start_from_steady: bool,
    /// Load factor (0 to 1, multiplies every loss) applied from the given
    /// time on, as `(from_s, factor)`. Empty = full load for the whole run.
    pub load_schedule: Vec<(f64, f64)>,
    /// Keep a field snapshot this often, s. `None` = first, middle, last.
    pub snapshot_every_s: Option<f64>,
}

impl Default for TransientSettings {
    fn default() -> Self {
        TransientSettings {
            duration_s: 3600.0,
            steps: 120,
            start_from_steady: false,
            load_schedule: Vec::new(),
            snapshot_every_s: None,
        }
    }
}
