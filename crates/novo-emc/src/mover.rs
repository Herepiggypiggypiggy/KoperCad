//! Placement search for the move-device fixes (EMC-07 filter close to its
//! drive, EMC-17 sensitive device away from a noisy one, EMC-19 sensitive
//! device out of the field of a wire).
//!
//! The search looks for the free spot on the same plate nearest to where
//! the device stands now. A spot is free when the cabinet layout rules
//! (`novo-layout`: overlap, device clearances, plate margin, DIN rail,
//! wiring openings, ducts) report nothing for the device there. The first
//! few spots in order of distance are tried on a copy of the project with
//! the real EMC rules, and the one that clears the finding and brings the
//! fewest new ones is taken. What the search does not know: heat and
//! airflow, the swing of the door, the length of the wires to the new spot.

use std::collections::BTreeSet;

use novo_core::{CabinetPlacement, ModelError, Project, Result, StructureId};
use novo_layout::state::CabinetLayout;

use crate::geometry::gap_mm;
use crate::project::analyse_project;

/// What the new spot has to satisfy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Goal {
    /// The gap to the other device is at most this (mm).
    Within(f64),
    /// The gap to the other device is at least this (mm).
    Away(f64),
    /// The modelled field at the device is under this (µT); no other device.
    Field(f64),
}

/// A spot found for a device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spot {
    pub structure: StructureId,
    pub x: f64,
    pub y: f64,
    /// How far the device's corner moves (mm).
    pub moved_mm: f64,
    /// The gap to the other device there (mm).
    pub gap_mm: f64,
    /// The modelled field at the device there (µT), for the field goal.
    pub field_ut: Option<f64>,
}

/// Spots are tried on this grid (mm), from the plate's usable corner.
const STEP_MM: f64 = 10.0;
/// Spots that get the full layout report before the search gives up.
const MAX_REPORTS: usize = 400;
/// A field spot has to be under this share of the planning value.
const FIELD_MARGIN: f64 = 0.8;
/// Free spots tried with the real EMC rules.
const TRIED: usize = 12;

fn involves(layout: &CabinetLayout, id: u64) -> bool {
    use novo_layout::DuctHit;
    let r = layout.report();
    r.collisions.iter().any(|(a, b)| *a == id || *b == id)
        || r.outside.contains(&id)
        || r.too_close.iter().any(|(a, b, _)| *a == id || *b == id)
        || layout.placements.iter().any(|p| {
            p.id == id
                && p.rail
                    .is_some_and(|rail| r.rail_overflows.iter().any(|(rid, _)| *rid == rail))
        })
        || r.opening_conflicts.iter().any(|c| c.device == id)
        || r.duct_overlaps
            .iter()
            .any(|d| matches!(d.hit, DuctHit::Device { id: i, .. } if i == id))
}

fn finding_keys(project: &Project) -> BTreeSet<String> {
    analyse_project(project)
        .findings
        .iter()
        .map(|f| format!("{}{:?}", f.rule, f.subjects))
        .collect()
}

/// Whether the field rule (EMC-19) still warns about `device`.
fn still_over(project: &Project, device: u64) -> bool {
    analyse_project(project).findings.iter().any(|f| {
        f.rule == "EMC-19"
            && f.severity == crate::report::Severity::Warning
            && f.subjects.contains(&crate::report::Subject::Device(device))
    })
}

/// Finds the free spot nearest to `device` that meets `goal` against
/// `other` (none for the field goal). Errors, in words the Apply card shows,
/// when the devices are not on the same plate or no spot works.
pub fn find_spot(project: &Project, device: u64, other: Option<u64>, goal: Goal) -> Result<Spot> {
    let place = |id: u64| {
        project
            .all::<CabinetPlacement>()
            .find(|p| p.device.raw() == id)
    };
    let (Some(mine), theirs) = (place(device), other.map(place)) else {
        return Err(ModelError::Format(
            "The device has to stand on the mounting plate before the program can move it"
                .to_owned(),
        ));
    };
    if let Some(None) = theirs {
        return Err(ModelError::Format(
            "Both devices have to stand on the mounting plate before the program can move one"
                .to_owned(),
        ));
    }
    if theirs
        .flatten()
        .is_some_and(|t| mine.structure != t.structure)
    {
        return Err(ModelError::Format(
            "The two devices are on different plates: move one by hand".to_owned(),
        ));
    }
    let structure = mine.structure;
    let Some(mut layout) = novo_layout::model::load(project, structure) else {
        return Err(ModelError::Format("The plate has no layout".to_owned()));
    };
    if layout.off_plate.contains_key(&device)
        || other.is_some_and(|o| layout.off_plate.contains_key(&o))
    {
        return Err(ModelError::Format(
            "A device on the door or another surface is moved by hand".to_owned(),
        ));
    }
    let Some(at) = layout.placements.iter().position(|p| p.id == device) else {
        return Err(ModelError::Format("The device is not placed".to_owned()));
    };
    let them = match other {
        Some(o) => {
            let Some(t) = layout.placements.iter().find(|p| p.id == o) else {
                return Err(ModelError::Format(
                    "The other device is not placed".to_owned(),
                ));
            };
            Some(t.footprint())
        }
        None => None,
    };
    let probe = match goal {
        Goal::Field(_) => {
            let netlist = project.netlist();
            let input = crate::project::input_of(project, &netlist);
            let setup = crate::project::field_setup_of(project, &netlist, &input);
            crate::field::FieldProbe::new(&input, &setup, structure.raw())
        }
        _ => None,
    };
    let rect = |r: novo_layout::Rect| (r.x, r.y, r.w, r.h);
    let (w, h) = {
        let f = layout.placements[at].footprint();
        (f.w, f.h)
    };
    let (x0, y0) = (layout.placements[at].x, layout.placements[at].y);
    let usable = layout.plate.usable();

    let others: Vec<_> = layout
        .on_plate()
        .into_iter()
        .filter(|p| p.id != device)
        .collect();

    // Every grid spot that meets the goal, nearest first.
    let mut spots: Vec<(f64, f64, f64, f64)> = Vec::new();
    let mut y = usable.y;
    while y + h <= usable.y + usable.h + 1e-9 {
        let mut x = usable.x;
        while x + w <= usable.x + usable.w + 1e-9 {
            let gap = them.map_or(0.0, |t| gap_mm((x, y, w, h), rect(t)));
            // Occupied cells first: sampling the field costs more.
            let here = novo_layout::Rect { x, y, w, h };
            if others.iter().any(|q| here.overlaps(&q.footprint())) {
                x += STEP_MM;
                continue;
            }
            let meets = match goal {
                Goal::Within(mm) => gap <= mm,
                Goal::Away(mm) => gap >= mm,
                // Some room under the value: the model is an estimate.
                Goal::Field(limit) => probe
                    .as_ref()
                    .is_none_or(|p| p.at_rect((x, y, w, h)).0 < limit * FIELD_MARGIN),
            };
            if meets && (x, y) != (x0, y0) {
                spots.push((((x - x0).powi(2) + (y - y0).powi(2)).sqrt(), x, y, gap));
            }
            x += STEP_MM;
        }
        y += STEP_MM;
    }
    spots.sort_by(|a, b| a.0.total_cmp(&b.0));

    let before = finding_keys(project);
    let mut best: Option<(usize, Spot)> = None;
    let mut tried = 0;
    layout.placements[at].rail = None;
    let mut reports = 0;
    let mut capped = false;
    for (moved, x, y, gap) in spots {
        layout.placements[at].x = x;
        layout.placements[at].y = y;
        // A cheap test first: the full layout report costs as much as the
        // plate has devices, and a crowded plate has few free cells.
        let me = layout.placements[at].clone();
        let at_rect = me.footprint();
        if !novo_layout::outside(&layout.plate, std::slice::from_ref(&me)).is_empty()
            || others.iter().any(|q| at_rect.overlaps(&q.footprint()))
        {
            continue;
        }
        if reports >= MAX_REPORTS {
            capped = true;
            break;
        }
        reports += 1;
        if involves(&layout, device) {
            continue;
        }
        // The real rules, on a copy.
        let mut copy = project.clone();
        let done = copy.transact("EMC move preview", |tx| {
            novo_layout::model::place_device(
                tx,
                structure,
                device,
                x,
                y,
                layout.placements[at].rotated,
            )
        });
        tried += 1;
        // The device may have been mounted on a rail by `place_device`: the
        // layout rules apply again where it stands now.
        let mounted_ok = done.is_ok()
            && novo_layout::model::load(&copy, structure).is_some_and(|l| !involves(&l, device));
        let cleared = !matches!(goal, Goal::Field(_)) || !still_over(&copy, device);
        if mounted_ok && cleared {
            let after = finding_keys(&copy);
            let added = after.difference(&before).count();
            let spot = Spot {
                structure,
                x,
                y,
                moved_mm: moved,
                gap_mm: gap,
                field_ut: probe.as_ref().map(|p| p.at_rect((x, y, w, h)).0),
            };
            if best.as_ref().is_none_or(|(n, _)| added < *n) {
                best = Some((added, spot));
            }
            if added == 0 {
                break;
            }
        }
        if tried >= TRIED {
            break;
        }
    }
    best.map(|(_, s)| s).ok_or_else(|| {
        if capped {
            return ModelError::Format(format!(
                "The search stopped after checking {MAX_REPORTS} spots nearest first, without finding a free one: make room or move the device by hand"
            ));
        }
        ModelError::Format(match goal {
            Goal::Within(mm) => format!(
                "No free spot on the plate puts the device within {} mm of the other: free room next to it, or move devices by hand",
                mm.round() as i64
            ),
            Goal::Away(mm) => format!(
                "No free spot on the plate keeps the device {} mm from the other: move devices by hand or use a second plate",
                mm.round() as i64
            ),
            Goal::Field(_) => "No free spot on the plate has a modelled field under the planning value: move the wire or the device by hand, or screen the wire".to_owned(),
        })
    })
}
