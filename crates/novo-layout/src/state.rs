//! In-memory layout of one panel: what the Cabinet view edits until the
//! model stores placements. Every edit returns nothing and never fails
//! silently: problems are reported by [`CabinetLayout::report`].

use std::collections::BTreeMap;

use crate::enclosure::{DoorReport, Enclosure, check};
use crate::plan::{Candidate, Plan, ROW_GAP_MM, place_all_below};
use crate::{
    Placement, Plate, Rail, Size, clearance_violations, collisions, outside, rail_overflows,
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
    /// Placed devices whose size is the default guess.
    pub guessed: Vec<u64>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.collisions.is_empty()
            && self.outside.is_empty()
            && self.too_close.is_empty()
            && self.rail_overflows.is_empty()
    }
}

/// A panel's plate, rails and placements.
#[derive(Debug, Clone, PartialEq)]
pub struct CabinetLayout {
    pub plate: Plate,
    pub rails: Vec<Rail>,
    pub placements: Vec<Placement>,
    pub grid: f64,
    /// Interior and door of the enclosure, once set.
    pub enclosure: Option<Enclosure>,
    /// Depth of the parts of placed devices, by device id, where known.
    pub depths: BTreeMap<u64, f64>,
}

impl CabinetLayout {
    pub fn new(plate: Plate) -> Self {
        CabinetLayout {
            plate,
            rails: Vec::new(),
            placements: Vec::new(),
            grid: DEFAULT_GRID_MM,
            enclosure: None,
            depths: BTreeMap::new(),
        }
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
            .placements
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
                    rail: Some(p.rail),
                });
            }
        }
    }

    pub fn report(&self) -> Report {
        Report {
            collisions: collisions(&self.placements),
            outside: outside(&self.plate, &self.placements),
            too_close: clearance_violations(&self.placements),
            rail_overflows: rail_overflows(&self.rails, &self.placements),
            guessed: self
                .placements
                .iter()
                .filter(|p| p.size.guessed)
                .map(|p| p.id)
                .collect(),
        }
    }
}
