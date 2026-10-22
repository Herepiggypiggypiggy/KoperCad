//! The project side of [`crate::placement`]: the rails of an enclosure as
//! search rows, and the accepted moves as one batch of placement commands.
//!
//! Scenario coordinates are in the interior frame (plate shifted by its
//! origin, see [`crate::bridge`]); the project stores plate coordinates, so
//! both directions shift by the same origin.

use novo_core::{Command, DeviceId, Project, RailId, Result, StructureId, Tx};
use novo_layout::model::{enclosure_of, load};

use crate::model::Scenario;
use crate::placement::{Move, Row};

/// Space kept under the last device of a row before the next duct or rail
/// row, mm.
const ROW_CLEAR_MM: f64 = 5.0;

/// A scenario's rails as search rows, and for every scenario device the
/// index of the row it stands on (`None` when it is on no horizontal rail).
#[derive(Debug, Clone, PartialEq)]
pub struct Rows {
    pub rows: Vec<Row>,
    pub row_of: Vec<Option<usize>>,
}

/// Origin of the plate in the interior frame.
pub fn plate_origin(project: &Project, enclosure: StructureId) -> Option<(f64, f64)> {
    let interior = enclosure_of(project, enclosure)?;
    let layout = load(project, enclosure)?;
    Some((
        interior.plate_left(&layout.plate),
        ((interior.height - layout.plate.height) / 2.0).max(0.0),
    ))
}

/// Rows of the horizontal rails of `enclosure`, for the devices of
/// `scenario` (built by [`crate::bridge::scenario_from_project`] from the
/// same project). Vertical rails are left out: their devices do not move.
pub fn rows(project: &Project, enclosure: StructureId, scenario: &Scenario) -> Option<Rows> {
    let layout = load(project, enclosure)?;
    let (ox, oy) = plate_origin(project, enclosure)?;
    let usable = layout.plate.usable();
    let mut rails: Vec<&novo_layout::Rail> = layout.rails.iter().filter(|r| !r.vertical).collect();
    rails.sort_by(|a, b| a.y.total_cmp(&b.y));
    let row_tops: Vec<f64> = rails.iter().map(|r| r.y).collect();
    let ducts: Vec<f64> = layout
        .ducts
        .iter()
        .filter(|d| !d.vertical)
        .map(|d| d.y - d.width / 2.0)
        .collect();
    let rows: Vec<Row> = rails
        .iter()
        .map(|r| {
            let below = row_tops
                .iter()
                .copied()
                .filter(|y| *y > r.y + 1.0)
                .chain(ducts.iter().copied().filter(|y| *y > r.y + 1.0))
                .fold(usable.bottom(), f64::min);
            Row {
                id: r.id,
                x_min: r.x + ox,
                x_max: (r.x + r.length).min(usable.right()) + ox,
                y_top: r.y + oy,
                height_mm: (below - r.y - ROW_CLEAR_MM).max(0.0),
            }
        })
        .collect();
    let row_of = scenario
        .devices
        .iter()
        .map(|d| {
            let rail = layout.placements.iter().find(|p| p.id == d.id)?.rail?;
            rows.iter().position(|row| row.id == rail)
        })
        .collect();
    Some(Rows { rows, row_of })
}

/// Puts the moved devices at their new places, on their new rails, as part
/// of the caller's transaction (one undo step). A swap is two moves in the
/// list. Returns the devices that could not be placed.
pub fn apply_moves(tx: &mut Tx<'_>, enclosure: StructureId, moves: &[Move]) -> Result<Vec<u64>> {
    let (ox, oy) = plate_origin(tx.project(), enclosure).unwrap_or((0.0, 0.0));
    let mut skipped = Vec::new();
    for m in moves {
        let rail = RailId::from(m.to_row);
        let Some(model_rail) = tx.project().get::<novo_core::Rail>(rail) else {
            skipped.push(m.id);
            continue;
        };
        let (x, y) = (m.to.x - ox, m.to.y - oy);
        let offset = if model_rail.vertical {
            y - model_rail.y
        } else {
            x - model_rail.x
        };
        tx.execute(Command::PlaceDevice {
            device: DeviceId::from(m.id),
            structure: enclosure,
            x,
            y,
            rotated: false,
            rail: Some((rail, offset)),
        })?;
    }
    Ok(skipped)
}
