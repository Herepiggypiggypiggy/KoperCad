//! Builds a thermal [`Scenario`] from a project: the placed devices of an
//! enclosure with their losses, depths and temperature limits, the rails and
//! ducts as obstacles, and a list of what the project does not say yet.
//!
//! The bridge never invents a loss. A device whose loss is unknown is placed
//! with 0 W (it still takes room and blocks air) and listed in
//! [`BridgeReport::without_loss`], so the UI can say "3 devices have no loss
//! data" instead of showing a cool, wrong cabinet.
//!
//! **Keys** (see [`keys`]; the part keys are the same as in
//! `novo_parts::keys`). A device property overrides the same key on its
//! parts; values that are not numbers, not finite or below zero count as
//! missing. The loss of a device with several parts is the sum over the parts
//! that give one, unless the device itself carries the key.
//!
//! **Frame.** The plate is centred in the interior sideways (the same rule as
//! `novo_layout::enclosure::Enclosure::plate_left`) and vertically; plate
//! coordinates are shifted by that origin into the interior frame of
//! [`crate::model`].
//!
//! **Depth.** A device's standoff from the plate is its `depth_mm`, device
//! property first, then the first part that has one. This is the same rule
//! the door checks use; once `novo_layout` exposes it as one shared function
//! this module calls that instead (see `handoff/thermal-engine`).

use novo_core::{Device, DeviceId, Part, Project, Properties, StructureId, Value};
use novo_layout::model::{enclosure_of, load};
use novo_layout::sizing::{Installation, Material};
use novo_layout::{Placement, Rect};

use crate::model::{Device as ThermalDevice, Enclosure, Obstacle, ObstacleKind, Params, Scenario};

/// Property keys the bridge reads.
pub mod keys {
    /// Power loss at rated load, W.
    pub const POWER_LOSS_W: &str = "power_loss_w";
    /// Share of the rated loss that occurs in operation, 0 to 1 (default 1).
    pub const LOAD_FACTOR: &str = "load_factor";
    /// Highest air temperature around the device the maker allows, degrees C.
    pub const MAX_AMBIENT_C: &str = "max_ambient_c";
    /// Highest housing surface temperature allowed, degrees C.
    pub const MAX_SURFACE_C: &str = "max_surface_c";
    /// How far the device stands out from the plate, mm.
    pub const DEPTH_MM: &str = "depth_mm";
}

/// Width of a DIN rail as an obstacle, mm.
pub const RAIL_WIDTH_MM: f64 = 35.0;
/// Outside ambient used when the caller gives none, degrees C.
pub const DEFAULT_AMBIENT_C: f64 = 35.0;
/// Wall thickness used when the caller gives none, mm.
pub const DEFAULT_WALL_MM: f64 = 2.0;

/// What the caller chooses about the surroundings; none of it is stored in
/// the project yet.
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeOptions {
    /// Highest air temperature outside the enclosure, degrees C.
    pub ambient_c: f64,
    pub material: Material,
    pub installation: Installation,
    pub wall_mm: f64,
    pub params: Params,
}

impl Default for BridgeOptions {
    fn default() -> Self {
        BridgeOptions {
            ambient_c: DEFAULT_AMBIENT_C,
            material: Material::Steel,
            installation: Installation::Wall,
            wall_mm: DEFAULT_WALL_MM,
            params: Params::default(),
        }
    }
}

/// Why no scenario could be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BridgeError {
    #[error("this is not an enclosure")]
    NotAnEnclosure,
    #[error("the enclosure has no mounting plate yet")]
    NoPlate,
    #[error("the enclosure interior (width, height, depth) is not set")]
    NoInterior,
}

/// A device the report points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRef {
    pub id: u64,
    pub tag: String,
}

/// What the bridge found missing or guessed, for the UI to say plainly.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BridgeReport {
    /// Devices in the scenario.
    pub placed: usize,
    /// Placed devices with no loss data (placed with 0 W).
    pub without_loss: Vec<DeviceRef>,
    /// Placed devices whose footprint is the default guess (no size on the part).
    pub guessed_size: Vec<DeviceRef>,
    /// Placed devices with no depth anywhere (the engine assumes 60 mm).
    pub without_depth: Vec<DeviceRef>,
    /// Devices of this enclosure's location that are not placed on the plate
    /// and so are not part of the heat picture.
    pub unplaced: Vec<DeviceRef>,
    /// Sum of effective loss of the placed devices, W.
    pub total_loss_w: f64,
}

impl BridgeReport {
    /// The sentences the UI shows, most important first; empty when nothing
    /// is missing.
    pub fn messages(&self) -> Vec<String> {
        let line = |n: usize, what: &str| -> Option<String> {
            (n > 0).then(|| {
                let (noun, verb) = if n == 1 {
                    ("device", "has")
                } else {
                    ("devices", "have")
                };
                format!("{n} {noun} {verb} {what}")
            })
        };
        [
            line(self.without_loss.len(), "no loss data"),
            line(
                self.guessed_size.len(),
                "no size (a default footprint is used)",
            ),
            line(self.without_depth.len(), "no depth (60 mm is assumed)"),
            line(self.unplaced.len(), "not been placed on the plate"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    pub fn is_complete(&self) -> bool {
        self.messages().is_empty()
    }
}

/// A scenario plus what is missing from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Bridge {
    pub scenario: Scenario,
    pub report: BridgeReport,
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Float(v) => Some(*v),
        Value::Int(v) => Some(*v as f64),
        _ => None,
    }
}

fn usable(properties: &Properties, key: &str) -> Option<f64> {
    properties
        .get(key)
        .and_then(number)
        .filter(|v| v.is_finite() && *v >= 0.0)
}

fn any_number(properties: &Properties, key: &str) -> Option<f64> {
    properties
        .get(key)
        .and_then(number)
        .filter(|v| v.is_finite())
}

fn parts<'a>(project: &'a Project, device: &Device) -> Vec<&'a Part> {
    device
        .parts
        .iter()
        .filter_map(|p| project.get::<Part>(*p))
        .collect()
}

/// A device's own value for `key`, else the first part that has one.
fn first_of(
    project: &Project,
    device: &Device,
    key: &str,
    read: fn(&Properties, &str) -> Option<f64>,
) -> Option<f64> {
    read(&device.properties, key).or_else(|| {
        parts(project, device)
            .iter()
            .find_map(|p| read(&p.properties, key))
    })
}

/// Rated loss of a device, W: its own, else the sum over its parts; `None`
/// when nothing gives one.
pub fn device_loss_w(project: &Project, device: &Device) -> Option<f64> {
    usable(&device.properties, keys::POWER_LOSS_W).or_else(|| {
        let values: Vec<f64> = parts(project, device)
            .iter()
            .filter_map(|p| usable(&p.properties, keys::POWER_LOSS_W))
            .collect();
        (!values.is_empty()).then(|| values.iter().sum())
    })
}

/// Property a part or device carries when its `power_loss_w` is a planning
/// estimate rather than a maker figure (value `"estimated"`).
pub const LOSS_SOURCE_PROPERTY: &str = "power_loss_source";

fn says_estimated(properties: &Properties) -> bool {
    matches!(properties.get(LOSS_SOURCE_PROPERTY), Some(Value::Text(t)) if t == "estimated")
}

/// Whether the loss [`device_loss_w`] reads is, in whole or in part, a
/// planning estimate. False when the device has no loss figure.
pub fn device_loss_estimated(project: &Project, device: &Device) -> bool {
    if usable(&device.properties, keys::POWER_LOSS_W).is_some() {
        return says_estimated(&device.properties);
    }
    parts(project, device).iter().any(|p| {
        usable(&p.properties, keys::POWER_LOSS_W).is_some() && says_estimated(&p.properties)
    })
}

/// Standoff of a device from the plate, mm: its own `depth_mm`, else the
/// first part with a positive one.
pub fn device_depth_mm(project: &Project, device: &Device) -> Option<f64> {
    let positive =
        |properties: &Properties| usable(properties, keys::DEPTH_MM).filter(|d| *d > 0.0);
    positive(&device.properties).or_else(|| {
        parts(project, device)
            .iter()
            .find_map(|p| positive(&p.properties))
    })
}

/// Builds the scenario of one enclosure.
pub fn scenario_from_project(
    project: &Project,
    enclosure: StructureId,
    options: &BridgeOptions,
) -> Result<Bridge, BridgeError> {
    let structure = project
        .get::<novo_core::Structure>(enclosure)
        .filter(|s| s.enclosure)
        .ok_or(BridgeError::NotAnEnclosure)?;
    if structure.plate.is_none() {
        return Err(BridgeError::NoPlate);
    }
    let interior = enclosure_of(project, enclosure).ok_or(BridgeError::NoInterior)?;
    let layout = load(project, enclosure).ok_or(BridgeError::NoPlate)?;

    let plate_x = interior.plate_left(&layout.plate);
    let plate_y = ((interior.height - layout.plate.height) / 2.0).max(0.0);
    let shift = |r: Rect| Rect {
        x: r.x + plate_x,
        y: r.y + plate_y,
        w: r.w,
        h: r.h,
    };

    let wall = options.wall_mm.max(0.0);
    let enclosure_model = Enclosure {
        width_mm: interior.width + 2.0 * wall,
        height_mm: interior.height + 2.0 * wall,
        depth_mm: interior.depth + 2.0 * wall,
        wall_mm: wall,
        material: options.material,
        installation: options.installation,
        plate: shift(Rect {
            x: 0.0,
            y: 0.0,
            w: layout.plate.width,
            h: layout.plate.height,
        }),
    };

    let mut report = BridgeReport::default();
    let mut devices = Vec::new();
    let mut placements: Vec<&Placement> = layout.placements.iter().collect();
    placements.sort_by_key(|p| p.id);
    for p in placements {
        let Some(device) = project.get::<Device>(DeviceId::from(p.id)) else {
            continue;
        };
        let who = DeviceRef {
            id: p.id,
            tag: device.tag.to_string(),
        };
        let loss = device_loss_w(project, device);
        let depth = device_depth_mm(project, device);
        if loss.is_none() {
            report.without_loss.push(who.clone());
        }
        if p.size.guessed {
            report.guessed_size.push(who.clone());
        }
        if depth.is_none() {
            report.without_depth.push(who.clone());
        }
        let factor = first_of(project, device, keys::LOAD_FACTOR, usable)
            .unwrap_or(1.0)
            .min(1.0);
        let loss_w = loss.unwrap_or(0.0) * factor;
        report.total_loss_w += loss_w;
        devices.push(ThermalDevice {
            id: p.id,
            footprint: shift(p.footprint()),
            height_mm: depth,
            loss_w,
            rated_max_ambient_c: first_of(project, device, keys::MAX_AMBIENT_C, any_number),
            rated_max_surface_c: first_of(project, device, keys::MAX_SURFACE_C, any_number),
            heat_capacity_j_per_k: None,
        });
    }
    report.placed = devices.len();

    let placed: std::collections::HashSet<u64> = layout.placements.iter().map(|p| p.id).collect();
    if !structure.tag.location.is_empty() {
        let mut unplaced: Vec<DeviceRef> = project
            .all::<Device>()
            .filter(|d| d.tag.location == structure.tag.location && !placed.contains(&d.id.raw()))
            .map(|d| DeviceRef {
                id: d.id.raw(),
                tag: d.tag.to_string(),
            })
            .collect();
        unplaced.sort_by_key(|d| d.id);
        report.unplaced = unplaced;
    }

    let mut obstacles = Vec::new();
    for duct in &layout.ducts {
        obstacles.push(Obstacle {
            kind: ObstacleKind::Duct,
            rect: shift(duct.footprint()),
            blockage: None,
        });
    }
    for rail in &layout.rails {
        let (w, h) = if rail.vertical {
            (RAIL_WIDTH_MM, rail.length)
        } else {
            (rail.length, RAIL_WIDTH_MM)
        };
        let (x, y) = if rail.vertical {
            (rail.x - RAIL_WIDTH_MM / 2.0, rail.y)
        } else {
            (rail.x, rail.y - RAIL_WIDTH_MM / 2.0)
        };
        obstacles.push(Obstacle {
            kind: ObstacleKind::Rail,
            rect: shift(Rect { x, y, w, h }),
            blockage: None,
        });
    }

    Ok(Bridge {
        scenario: Scenario {
            enclosure: enclosure_model,
            ambient_c: options.ambient_c,
            devices,
            obstacles,
            vents: Vec::new(),
            fans: Vec::new(),
            params: options.params.clone(),
        },
        report,
    })
}
