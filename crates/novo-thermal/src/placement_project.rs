//! The project side of [`crate::placement`]: the rails of an enclosure as
//! search rows, and the accepted moves as one batch of placement commands.
//!
//! Scenario coordinates are in the interior frame (plate shifted by its
//! origin, see [`crate::bridge`]); the project stores plate coordinates, so
//! both directions shift by the same origin.

use novo_core::{Command, DeviceId, Project, RailId, Result, StructureId, Tx};
use novo_layout::RAIL_BAND_MM;
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
    let rail_index = |rail: u64| rails.iter().position(|r| r.id == rail);
    let row_of: Vec<Option<usize>> = scenario
        .devices
        .iter()
        .map(|d| {
            let rail = layout.placements.iter().find(|p| p.id == d.id)?.rail?;
            rail_index(rail)
        })
        .collect();
    // Half the height of the tallest device each rail carries: devices are
    // centred on the rail's line, so that much reaches up and down from it.
    let half: Vec<f64> = (0..rails.len())
        .map(|i| {
            scenario
                .devices
                .iter()
                .zip(&row_of)
                .filter(|(_, r)| **r == Some(i))
                .map(|(d, _)| d.footprint.h / 2.0)
                .fold(RAIL_BAND_MM / 2.0, f64::max)
        })
        .collect();
    let duct_tops: Vec<f64> = layout
        .ducts
        .iter()
        .filter(|d| !d.vertical)
        .map(|d| d.y - d.width / 2.0)
        .collect();
    let duct_bottoms: Vec<f64> = layout
        .ducts
        .iter()
        .filter(|d| !d.vertical)
        .map(|d| d.y + d.width / 2.0)
        .collect();
    let rows: Vec<Row> = rails
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let line = r.y;
            let up = (0..i)
                .filter(|j| rails[*j].y < line - 1.0)
                .map(|j| rails[j].y + half[j])
                .chain(duct_bottoms.iter().copied().filter(|y| *y < line - 1.0))
                .fold(usable.y, f64::max);
            let down = ((i + 1)..rails.len())
                .filter(|j| rails[*j].y > line + 1.0)
                .map(|j| rails[j].y - half[j])
                .chain(duct_tops.iter().copied().filter(|y| *y > line + 1.0))
                .fold(usable.bottom(), f64::min);
            let room = ((line - up).min(down - line) - ROW_CLEAR_MM).max(0.0);
            Row {
                id: r.id,
                x_min: r.x + ox,
                x_max: (r.x + r.length).min(usable.right()) + ox,
                y_line: line + oy,
                height_mm: 2.0 * room,
            }
        })
        .collect();
    Some(Rows { rows, row_of })
}

/// Puts the moved devices at their new places, on their new rails, as part
/// of the caller's transaction (one undo step). A swap is two moves in the
/// list. A device keeps the rotation it stands in. All or nothing: when a
/// target rail is gone, nothing is placed and the devices with a missing
/// rail are returned.
pub fn apply_moves(tx: &mut Tx<'_>, enclosure: StructureId, moves: &[Move]) -> Result<Vec<u64>> {
    let (ox, oy) = plate_origin(tx.project(), enclosure).unwrap_or((0.0, 0.0));
    let missing: Vec<u64> = moves
        .iter()
        .filter(|m| {
            tx.project()
                .get::<novo_core::Rail>(RailId::from(m.to_row))
                .is_none()
        })
        .map(|m| m.id)
        .collect();
    if !missing.is_empty() {
        return Ok(missing);
    }
    let rotated_now: Vec<(u64, bool)> = load(tx.project(), enclosure)
        .map(|l| l.placements.iter().map(|p| (p.id, p.rotated)).collect())
        .unwrap_or_default();
    for m in moves {
        let rail = RailId::from(m.to_row);
        let Some(model_rail) = tx.project().get::<novo_core::Rail>(rail) else {
            continue;
        };
        let (x, y) = (m.to.x - ox, m.to.y - oy);
        let offset = if model_rail.vertical {
            y - model_rail.y
        } else {
            x - model_rail.x
        };
        let rotated = rotated_now
            .iter()
            .find(|(id, _)| *id == m.id)
            .is_some_and(|(_, r)| *r);
        tx.execute(Command::PlaceDevice {
            device: DeviceId::from(m.id),
            structure: enclosure,
            x,
            y,
            rotated,
            rail: Some((rail, offset)),
        })?;
    }
    Ok(Vec::new())
}
