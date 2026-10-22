//! In-memory layout of one panel: what the Cabinet view edits until the
//! model stores placements. Every edit returns nothing and never fails
//! silently: problems are reported by [`CabinetLayout::report`].

use std::collections::BTreeMap;

use crate::enclosure::{DoorReport, Enclosure, check};
use crate::openings::{Opening, OpeningConflict, conflicts};
use crate::plan::{Candidate, Plan, ROW_GAP_MM, place_all_below};
use crate::surfaces::MAIN_PLATE;
use crate::surfaces::Surfaces;
use crate::swing::{DoorDevice, PlateDevice, SweepSettings, SwingReport, sweep};
use crate::{
    Duct, DuctOverlap, Margins, Placement, Plate, Rail, Size, clearance_violations, collisions,
    duct_overlaps, off_centre, outside, rail_overflows,
};

/// Default snap grid for free placement (the Cabinet canvas grid).
pub const DEFAULT_GRID_MM: f64 = 5.0;

/// Everything wrong with a layout, for the warnings strip and Issues.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub collisions: Vec<(u64, u64)>,
    pub outside: Vec<u64>,
    pub too_close: Vec<(u64, u64, f64)>,
    pub rail_overflows: Vec<(u64, f64)>,
    /// Devices that cover a wiring opening of the plate.
    pub opening_conflicts: Vec<OpeningConflict>,
    /// Ducts lying over a rail or a device.
    pub duct_overlaps: Vec<DuctOverlap>,
    /// Placed devices whose size is the default guess.
    pub guessed: Vec<u64>,
    /// Devices on a rail but not centred on it: `(device, rail, offset)`.
    pub off_centre: Vec<(u64, u64, f64)>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.collisions.is_empty()
            && self.outside.is_empty()
            && self.too_close.is_empty()
            && self.rail_overflows.is_empty()
            && self.opening_conflicts.is_empty()
            && self.duct_overlaps.is_empty()
            && self.off_centre.is_empty()
    }
}

/// A panel's plate, rails and placements.
#[derive(Debug, Clone, PartialEq)]
pub struct CabinetLayout {
    pub plate: Plate,
    pub rails: Vec<Rail>,
    pub ducts: Vec<Duct>,
    pub placements: Vec<Placement>,
    pub grid: f64,
    /// Interior and door of the enclosure, once set.
    pub enclosure: Option<Enclosure>,
    /// Depth of the parts of placed devices, by device id, where known.
    pub depths: BTreeMap<u64, f64>,
    /// Wiring openings (`opening.<name>` structure properties).
    pub openings: Vec<Opening>,
    /// Opening properties that could not be read.
    pub opening_problems: Vec<String>,
    /// Devices on the inside of the door (door coordinates), swept when the
    /// door opens.
    pub door_devices: Vec<DoorDevice>,
    /// Placed devices that sit on a surface other than the mounting plate
    /// (door, walls, extra plates), by device id with the surface name. Their
    /// positions are in that surface's millimetres, so the plate checks and
    /// the swing obstacles leave them out.
    pub off_plate: BTreeMap<u64, String>,
    /// Width and height of every surface of the enclosure by name (the
    /// plate, door, walls, roof, floor and extra plates), for the drill
    /// sheets. Empty without an enclosure.
    pub surface_sizes: BTreeMap<String, (f64, f64)>,
}

impl CabinetLayout {
    pub fn new(plate: Plate) -> Self {
        CabinetLayout {
            plate,
            rails: Vec::new(),
            ducts: Vec::new(),
            placements: Vec::new(),
            grid: DEFAULT_GRID_MM,
            enclosure: None,
            depths: BTreeMap::new(),
            openings: Vec::new(),
            opening_problems: Vec::new(),
            door_devices: Vec::new(),
            off_plate: BTreeMap::new(),
            surface_sizes: BTreeMap::new(),
        }
    }

    /// The placements on the mounting plate itself: all of them except those
    /// listed in [`CabinetLayout::off_plate`].
    pub fn on_plate(&self) -> Vec<Placement> {
        self.placements
            .iter()
            .filter(|p| !self.off_plate.contains_key(&p.id))
            .cloned()
            .collect()
    }

    /// The depth and door checks; `None` until the enclosure has an interior.
    pub fn door_report(&self) -> Option<DoorReport> {
        let enclosure = self.enclosure.as_ref()?;
        let depths: Vec<(u64, Option<f64>)> = self
            .placements
            .iter()
            .map(|p| (p.id, self.depths.get(&p.id).copied()))
            .collect();
        Some(check(enclosure, &self.plate, &depths))
    }

    /// Door devices swept against the plate devices and the walls; `None`
    /// until the enclosure has an interior.
    pub fn swing_report(&self) -> Option<SwingReport> {
        let enclosure = self.enclosure.as_ref()?;
        let (surfaces, _) = Surfaces::from_enclosure(enclosure, &self.plate, &[]);
        let plate_devices: Vec<PlateDevice> = self
            .on_plate()
            .iter()
            .filter_map(|p| {
                Some(PlateDevice {
                    id: p.id,
                    surface: crate::surfaces::MAIN_PLATE.into(),
                    rect: p.footprint(),
                    depth: *self.depths.get(&p.id)?,
                    mesh: None,
                })
            })
            .collect();
        let mut report = sweep(
            enclosure,
            &surfaces,
            &self.door_devices,
            &plate_devices,
            &SweepSettings::default(),
        );
        report.unchecked_plate = self
            .on_plate()
            .iter()
            .filter(|p| !self.depths.contains_key(&p.id))
            .map(|p| p.id)
            .collect();
        Some(report)
    }

    /// A position snapped to the grid, for dropping and dragging.
    pub fn snapped(&self, x: f64, y: f64) -> (f64, f64) {
        (self.snap(x), self.snap(y))
    }

    fn snap(&self, v: f64) -> f64 {
        if self.grid > 0.0 {
            (v / self.grid).round() * self.grid
        } else {
            v
        }
    }

    /// Places a device from the drawer at `(x, y)`, snapped to the grid.
    /// A device already placed is moved instead.
    pub fn drop_device(&mut self, id: u64, size: Size, x: f64, y: f64) {
        let (x, y) = (self.snap(x), self.snap(y));
        if let Some(p) = self.placements.iter_mut().find(|p| p.id == id) {
            p.x = x;
            p.y = y;
            p.rail = None;
        } else {
            self.placements.push(Placement {
                id,
                x,
                y,
                rotated: false,
                size,
                clearance: None,
                margins: Margins::default(),
                rail: None,
            });
        }
    }

    /// Moves a placed device (drag), snapped; dragging takes it off its rail.
    pub fn move_to(&mut self, id: u64, x: f64, y: f64) {
        let (x, y) = (self.snap(x), self.snap(y));
        if let Some(p) = self.placements.iter_mut().find(|p| p.id == id) {
            p.x = x;
            p.y = y;
            p.rail = None;
        }
    }

    pub fn rotate(&mut self, id: u64) {
        if let Some(p) = self.placements.iter_mut().find(|p| p.id == id) {
            p.rotated = !p.rotated;
        }
    }

    pub fn remove(&mut self, id: u64) {
        self.placements.retain(|p| p.id != id);
    }

    /// Devices from `all` that are not placed yet, in the given order: the
    /// drawer's unplaced list.
    pub fn unplaced<'a>(&self, all: &'a [Candidate]) -> Vec<&'a Candidate> {
        all.iter()
            .filter(|c| !self.placements.iter().any(|p| p.id == c.id))
            .collect()
    }

    /// Proposes a layout for the unplaced devices on the empty plate. The
    /// plan sits below everything already placed and is only a proposal;
    /// [`Self::accept`] applies it.
    pub fn preview(&self, all: &[Candidate]) -> Plan {
        let unplaced: Vec<Candidate> = self.unplaced(all).into_iter().cloned().collect();
        let top = self
            .on_plate()
            .iter()
            .map(|p| p.footprint().bottom() + ROW_GAP_MM)
            .fold(0.0, f64::max);
        place_all_below(
            &self.plate,
            &unplaced,
            self.rails.iter().map(|r| r.id + 1).max().unwrap_or(1),
            top,
        )
    }

    /// Applies a plan. Because it is one call, it is one undo step in the UI.
    pub fn accept(&mut self, plan: &Plan, all: &[Candidate]) {
        self.rails.extend(plan.rails.iter().cloned());
        for p in &plan.placements {
            if self.placements.iter().any(|q| q.id == p.id) {
                continue;
            }
            if let Some(c) = all.iter().find(|c| c.id == p.id) {
                self.placements.push(Placement {
                    id: p.id,
                    x: p.x,
                    y: p.y,
                    rotated: false,
                    size: c.size,
                    clearance: None,
                    margins: Margins::default(),
                    rail: Some(p.rail),
                });
            }
        }
    }

    pub fn report(&self) -> Report {
        let on_plate = self.on_plate();
        Report {
            collisions: collisions(&on_plate),
            outside: outside(&self.plate, &on_plate),
            too_close: clearance_violations(&on_plate),
            rail_overflows: rail_overflows(&self.rails, &on_plate),
            opening_conflicts: conflicts(&self.openings, MAIN_PLATE, &on_plate),
            duct_overlaps: duct_overlaps(&self.ducts, &self.rails, &on_plate),
            off_centre: off_centre(&self.rails, &on_plate),
            guessed: on_plate
                .iter()
                .filter(|p| p.size.guessed)
                .map(|p| p.id)
                .collect(),
        }
    }
}
