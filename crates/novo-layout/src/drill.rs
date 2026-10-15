//! Drill holes and cut-outs: what a workshop machines into the mounting
//! plate (and the door) for a layout, as a numbered table.
//!
//! The data comes from three part property keys (R0 of
//! `docs/PRO-PANEL-GAP-ANALYSIS.md`): [`MOUNTING_KEY`], [`HOLE_PATTERN_KEY`]
//! and [`CUTOUT_KEY`]. Maker data fills them, or the user types them; nothing
//! is guessed. DIN rails get their own screw holes from fixed defaults.
//!
//! Coordinates are plate millimetres, origin top left, x right, y down,
//! like the rest of the layout engine.

use std::collections::HashMap;

use crate::state::CabinetLayout;
use crate::{Placement, Rail};

/// Part key: how the device is held. `door_cutout` puts its holes and
/// cut-out on the door; anything else (`din_rail`, `screw`, `plate`) on the
/// plate.
pub const MOUNTING_KEY: &str = "mounting";
/// Part key: screw holes as `x,y,d; x,y,d` (mm from the top left of the
/// unrotated device, diameter last).
pub const HOLE_PATTERN_KEY: &str = "hole_pattern";
/// Part key: a cut-out as `rect WxH` or `round D`, optionally `@x,y` for the
/// centre (mm from the top left of the unrotated device; the device centre
/// when left out).
pub const CUTOUT_KEY: &str = "cutout";

/// Diameter of the screw holes that fix a DIN rail (M5 clearance).
pub const RAIL_HOLE_DIA_MM: f64 = 5.5;
/// Distance from a rail end to its first and last hole.
pub const RAIL_HOLE_INSET_MM: f64 = 10.0;
/// Largest distance between two holes along a rail.
pub const RAIL_HOLE_PITCH_MM: f64 = 200.0;

/// The face a feature is machined into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Surface {
    Plate,
    Door,
}

impl Surface {
    pub fn name(self) -> &'static str {
        match self {
            Surface::Plate => "Mounting plate",
            Surface::Door => "Door",
        }
    }
}

/// One screw hole, relative to the top left of the unrotated device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hole {
    pub dx: f64,
    pub dy: f64,
    pub dia: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CutoutShape {
    Rect { w: f64, h: f64 },
    Round { dia: f64 },
}

impl CutoutShape {
    /// Text for the table: `22.5 x 22.5` or `⌀ 22.5`.
    pub fn describe(&self) -> String {
        match self {
            CutoutShape::Rect { w, h } => format!("{} x {}", mm(*w), mm(*h)),
            CutoutShape::Round { dia } => format!("dia {}", mm(*dia)),
        }
    }
}

/// A cut-out; `centre` is relative to the unrotated device's top left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cutout {
    pub shape: CutoutShape,
    pub centre: Option<(f64, f64)>,
}

/// What one device needs machined, read from its part.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceFeatures {
    /// The device id, as in [`Placement::id`].
    pub id: u64,
    /// The device tag as shown on the drawing.
    pub label: String,
    pub surface: Surface,
    pub holes: Vec<Hole>,
    pub cutout: Option<Cutout>,
    /// Pattern text that could not be read; reported, never silently dropped.
    pub problems: Vec<String>,
}

/// Formats a length without trailing zeros: `45`, `22.5`.
pub fn mm(v: f64) -> String {
    let s = format!("{:.2}", (v * 100.0).round() / 100.0);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Reads a hole pattern. Entries that are not `x,y,d` with `d > 0` go to the
/// second list.
pub fn parse_holes(text: &str) -> (Vec<Hole>, Vec<String>) {
    let (mut holes, mut bad) = (Vec::new(), Vec::new());
    for entry in text
        .split([';', '\n'])
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        let parts: Vec<_> = entry.split(',').map(num).collect();
        match parts.as_slice() {
            [Some(dx), Some(dy), Some(dia)] if *dia > 0.0 => holes.push(Hole {
                dx: *dx,
                dy: *dy,
                dia: *dia,
            }),
            _ => bad.push(format!("hole \"{entry}\" is not x,y,diameter")),
        }
    }
    (holes, bad)
}

/// Reads a cut-out: `rect 22.5x22.5`, `round 22.5`, with optional `@x,y`.
pub fn parse_cutout(text: &str) -> Result<Cutout, String> {
    let lower = text.trim().to_lowercase();
    let err = || format!("cut-out \"{}\" is not `rect WxH` or `round D`", text.trim());
    let (shape_text, at) = match lower.split_once('@') {
        Some((s, a)) => (s.trim(), Some(a)),
        None => (lower.as_str(), None),
    };
    let centre = match at {
        None => None,
        Some(a) => {
            let (x, y) = a.split_once(',').ok_or_else(err)?;
            Some((num(x).ok_or_else(err)?, num(y).ok_or_else(err)?))
        }
    };
    let shape = if let Some(size) = shape_text.strip_prefix("rect") {
        let (w, h) = size.trim().split_once(['x', '×']).ok_or_else(err)?;
        let (w, h) = (num(w).ok_or_else(err)?, num(h).ok_or_else(err)?);
        if w <= 0.0 || h <= 0.0 {
            return Err(err());
        }
        CutoutShape::Rect { w, h }
    } else if let Some(d) = shape_text.strip_prefix("round") {
        let dia = num(d).filter(|d| *d > 0.0).ok_or_else(err)?;
        CutoutShape::Round { dia }
    } else {
        return Err(err());
    };
    Ok(Cutout { shape, centre })
}

/// Builds a device's features from the three part property texts.
pub fn features_from_text(
    id: u64,
    label: impl Into<String>,
    mounting: Option<&str>,
    holes: Option<&str>,
    cutout: Option<&str>,
) -> DeviceFeatures {
    let (holes, mut problems) = holes.map(parse_holes).unwrap_or_default();
    let cutout = cutout.and_then(|c| match parse_cutout(c) {
        Ok(c) => Some(c),
        Err(e) => {
            problems.push(e);
            None
        }
    });
    let surface = match mounting.map(|m| m.trim().to_lowercase()) {
        Some(m) if m == "door_cutout" => Surface::Door,
        _ => Surface::Plate,
    };
    DeviceFeatures {
        id,
        label: label.into(),
        surface,
        holes,
        cutout,
        problems,
    }
}

/// Turns an offset inside the unrotated footprint into one inside the
/// placed footprint (rotation is 90 degrees clockwise on the y-down plate).
fn place(dx: f64, dy: f64, p: &Placement) -> (f64, f64) {
    let f = p.footprint();
    let (x, y) = if p.rotated {
        (p.size.height - dy, dx)
    } else {
        (dx, dy)
    };
    (f.x + x, f.y + y)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[derive(Debug, Clone, PartialEq)]
pub struct HoleRow {
    pub no: usize,
    pub surface: Surface,
    pub dia: f64,
    pub x: f64,
    pub y: f64,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CutoutRow {
    pub no: usize,
    pub surface: Surface,
    pub shape: CutoutShape,
    pub x: f64,
    pub y: f64,
    pub source: String,
}

/// Every hole and cut-out of a layout, numbered per surface.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrillTable {
    pub holes: Vec<HoleRow>,
    pub cutouts: Vec<CutoutRow>,
    /// Pattern text that could not be read, with the device it belongs to.
    pub problems: Vec<String>,
}

impl DrillTable {
    /// Hole counts per surface and diameter, smallest diameter first.
    pub fn summary(&self) -> Vec<(Surface, f64, usize)> {
        let mut out: Vec<(Surface, f64, usize)> = Vec::new();
        for h in &self.holes {
            match out
                .iter_mut()
                .find(|(s, d, _)| *s == h.surface && *d == h.dia)
            {
                Some(e) => e.2 += 1,
                None => out.push((h.surface, h.dia, 1)),
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        out
    }

    pub fn on(&self, surface: Surface) -> (Vec<&HoleRow>, Vec<&CutoutRow>) {
        (
            self.holes.iter().filter(|h| h.surface == surface).collect(),
            self.cutouts
                .iter()
                .filter(|c| c.surface == surface)
                .collect(),
        )
    }
}

/// Screw holes of one rail: both ends and at most
/// [`RAIL_HOLE_PITCH_MM`] apart, on its centre line.
pub fn rail_holes(rail: &Rail) -> Vec<(f64, f64)> {
    let span = rail.length - 2.0 * RAIL_HOLE_INSET_MM;
    if span <= 0.0 {
        return vec![(rail.x + rail.length / 2.0, rail.y)];
    }
    let segments = (span / RAIL_HOLE_PITCH_MM).ceil().max(1.0) as usize;
    (0..=segments)
        .map(|i| {
            (
                rail.x + RAIL_HOLE_INSET_MM + span * i as f64 / segments as f64,
                rail.y,
            )
        })
        .collect()
}

/// The drill and cut-out table for a layout. Rows are grouped by diameter
/// (one tool change per size), then top to bottom, left to right.
pub fn drill_table(layout: &CabinetLayout, features: &[DeviceFeatures]) -> DrillTable {
    let by_id: HashMap<u64, &Placement> = layout.placements.iter().map(|p| (p.id, p)).collect();
    let mut holes: Vec<HoleRow> = Vec::new();
    let mut cutouts: Vec<CutoutRow> = Vec::new();
    let mut problems = Vec::new();

    let mut rails: Vec<&Rail> = layout.rails.iter().collect();
    rails.sort_by(|a, b| {
        a.y.total_cmp(&b.y)
            .then(a.x.total_cmp(&b.x))
            .then(a.id.cmp(&b.id))
    });
    for (i, rail) in rails.iter().enumerate() {
        for (x, y) in rail_holes(rail) {
            holes.push(HoleRow {
                no: 0,
                surface: Surface::Plate,
                dia: RAIL_HOLE_DIA_MM,
                x: round2(x),
                y: round2(y),
                source: format!("Rail {}", i + 1),
            });
        }
    }

    for f in features {
        problems.extend(f.problems.iter().map(|p| format!("{}: {p}", f.label)));
        let Some(p) = by_id.get(&f.id) else { continue };
        for h in &f.holes {
            let (x, y) = place(h.dx, h.dy, p);
            holes.push(HoleRow {
                no: 0,
                surface: f.surface,
                dia: h.dia,
                x: round2(x),
                y: round2(y),
                source: f.label.clone(),
            });
        }
        if let Some(c) = &f.cutout {
            let (cx, cy) = c
                .centre
                .unwrap_or((p.size.width / 2.0, p.size.height / 2.0));
            let (x, y) = place(cx, cy, p);
            let shape = match (c.shape, p.rotated) {
                (CutoutShape::Rect { w, h }, true) => CutoutShape::Rect { w: h, h: w },
                (s, _) => s,
            };
            cutouts.push(CutoutRow {
                no: 0,
                surface: f.surface,
                shape,
                x: round2(x),
                y: round2(y),
                source: f.label.clone(),
            });
        }
    }

    holes.sort_by(|a, b| {
        a.surface
            .cmp(&b.surface)
            .then(a.dia.total_cmp(&b.dia))
            .then(a.y.total_cmp(&b.y))
            .then(a.x.total_cmp(&b.x))
            .then(a.source.cmp(&b.source))
    });
    cutouts.sort_by(|a, b| {
        a.surface
            .cmp(&b.surface)
            .then(a.y.total_cmp(&b.y))
            .then(a.x.total_cmp(&b.x))
            .then(a.source.cmp(&b.source))
    });
    for surface in [Surface::Plate, Surface::Door] {
        for (n, h) in holes
            .iter_mut()
            .filter(|h| h.surface == surface)
            .enumerate()
        {
            h.no = n + 1;
        }
        for (n, c) in cutouts
            .iter_mut()
            .filter(|c| c.surface == surface)
            .enumerate()
        {
            c.no = n + 1;
        }
    }
    DrillTable {
        holes,
        cutouts,
        problems,
    }
}
