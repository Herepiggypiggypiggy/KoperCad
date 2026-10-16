//! Connection points of a part from its STEP model, and their assignment to
//! the terminals of the catalog part.
//!
//! The 3D view and wire routing need to know where a wire meets a part. A
//! maker's STEP file shows it: a wire entry is a round or rectangular opening
//! in the housing, a plug or solder pin is a small cylinder that sticks out.
//! This module finds those, puts them in the frame of the render mesh (see
//! the coordinate contract in `docs/design/part-geometry.md`), and matches
//! them to the part's terminals (`novo_parts::PartTerminal`).
//!
//! It is a reading of geometry, not a guarantee. Every result says how it was
//! found ([`Fix`]): `Measured` means the number of openings facing a side
//! equals the number of terminals the catalog lists there, so the match is by
//! order along that side; `Partial` means there were more openings than
//! terminals and the most regular run was taken; `Estimated` is an even
//! spacing along the side because the model had no usable openings (or none
//! that fit), and is never mistaken for a measurement.
//!
//! Positions are in the schema frame of `PartTerminal::position_mm`: origin
//! at the back-left-bottom corner of the outline, x to the right, y up, z
//! from the mounting plate outwards.

use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

pub use novo_parts::POSITIONS_SOURCE_KEY;
use novo_parts::{CatalogPart, ConnectionKind, DataSource, PartTerminal, Side};

use super::brep::{Face, Surface};
use super::build::{BuildError, StepOptions, step_soup};
use super::math::*;
use super::mesh::{Orientation, chord_tolerance, derive_xf};

/// What a feature of the model looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    /// A hole or slot in the housing where a wire goes in.
    Opening,
    /// A small cylinder that sticks out of the housing (plug, solder pin).
    Pin,
}

/// A feature of a model before it is placed in part space (model frame,
/// millimetres). Collected while the faces are read.
#[derive(Debug, Clone, Copy)]
pub struct RawFeature {
    pub kind: CandidateKind,
    /// Centre of the opening in its face plane; middle of the pin's axis.
    pub centre: V3,
    /// Face normal (opening) or axis (pin); the sign is not trusted.
    pub axis: V3,
    /// Diameter of the circle of the same area, in mm.
    pub size_mm: f64,
    pub round: bool,
    /// Length of a pin along its axis.
    pub length_mm: f64,
}

/// Openings and pins smaller or larger than this are not wire connections.
const OPENING_MIN_MM: f64 = 0.8;
const OPENING_MAX_MM: f64 = 14.0;
/// Slots longer than this many times their width are vents, not entries.
const MAX_ASPECT: f64 = 3.5;
const PIN_RADIUS_MM: (f64, f64) = (0.3, 3.0);
const PIN_MIN_LENGTH_MM: f64 = 1.2;

/// The connection-like features of one face, in the model frame.
pub fn face_features(face: &Face) -> Vec<RawFeature> {
    match face.surface {
        Surface::Plane(f) if face.loops.len() >= 2 => plane_openings(face, &f),
        Surface::Cylinder(f, r)
            if face.same_sense && r >= PIN_RADIUS_MM.0 && r <= PIN_RADIUS_MM.1 =>
        {
            cylinder_pin(face, &f, r).into_iter().collect()
        }
        _ => Vec::new(),
    }
}

/// Inner boundaries of a flat face that are the right size for a wire.
fn plane_openings(face: &Face, f: &Frame) -> Vec<RawFeature> {
    let flat: Vec<Vec<[f64; 2]>> = face
        .loops
        .iter()
        .map(|l| {
            l.iter()
                .map(|&p| {
                    let d = sub(p, f.o);
                    [dot(d, f.x), dot(d, f.y)]
                })
                .collect()
        })
        .collect();
    let areas: Vec<f64> = flat.iter().map(|l| signed_area(l).abs()).collect();
    let outer = (0..flat.len())
        .max_by(|&a, &b| areas[a].total_cmp(&areas[b]))
        .unwrap_or(0);
    let mut out = Vec::new();
    for (i, l) in flat.iter().enumerate() {
        if i == outer || l.len() < 3 || areas[i] < 1e-6 {
            continue;
        }
        let area = areas[i];
        let perim: f64 = (0..l.len())
            .map(|k| {
                let (a, b) = (l[k], l[(k + 1) % l.len()]);
                ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
            })
            .sum();
        let round = perim > 0.0 && 4.0 * PI * area / (perim * perim) > 0.88;
        let (short, long, c) = min_rect(l);
        if short < OPENING_MIN_MM || long > OPENING_MAX_MM || long > MAX_ASPECT * short {
            continue;
        }
        let (cx, cy) = if round { centroid(l) } else { (c[0], c[1]) };
        out.push(RawFeature {
            kind: CandidateKind::Opening,
            centre: add(f.o, add(scale(f.x, cx), scale(f.y, cy))),
            axis: f.z,
            size_mm: (4.0 * area / PI).sqrt(),
            round,
            length_mm: 0.0,
        });
    }
    out
}

/// A convex cylinder face as a pin.
fn cylinder_pin(face: &Face, f: &Frame, r: f64) -> Option<RawFeature> {
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for p in face.loops.iter().flatten() {
        let t = dot(sub(*p, f.o), f.z);
        lo = lo.min(t);
        hi = hi.max(t);
    }
    let length = hi - lo;
    if !length.is_finite() || length < PIN_MIN_LENGTH_MM || length < r {
        return None;
    }
    Some(RawFeature {
        kind: CandidateKind::Pin,
        centre: add(f.o, scale(f.z, (lo + hi) / 2.0)),
        axis: f.z,
        size_mm: 2.0 * r,
        round: true,
        length_mm: length,
    })
}

fn signed_area(l: &[[f64; 2]]) -> f64 {
    let n = l.len();
    (0..n)
        .map(|i| {
            let (a, b) = (l[i], l[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

fn centroid(l: &[[f64; 2]]) -> (f64, f64) {
    let n = l.len();
    let (mut a2, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (p, q) = (l[i], l[(i + 1) % n]);
        let w = p[0] * q[1] - q[0] * p[1];
        a2 += w;
        cx += (p[0] + q[0]) * w;
        cy += (p[1] + q[1]) * w;
    }
    if a2.abs() < 1e-12 {
        let k = n as f64;
        return (
            l.iter().map(|p| p[0]).sum::<f64>() / k,
            l.iter().map(|p| p[1]).sum::<f64>() / k,
        );
    }
    (cx / (3.0 * a2), cy / (3.0 * a2))
}

/// Smallest enclosing rectangle by rotation: (short side, long side, centre).
fn min_rect(l: &[[f64; 2]]) -> (f64, f64, [f64; 2]) {
    let mut best = (f64::MAX, 0.0, 0.0, [0.0, 0.0]);
    for step in 0..30 {
        let a = f64::from(step) * PI / 2.0 / 30.0;
        let (s, c) = a.sin_cos();
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in l {
            let q = [p[0] * c + p[1] * s, -p[0] * s + p[1] * c];
            for k in 0..2 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
        if w * h < best.0 {
            let m = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
            // Back to the frame of the face.
            best = (w * h, w, h, [m[0] * c - m[1] * s, m[0] * s + m[1] * c]);
        }
    }
    (best.1.min(best.2), best.1.max(best.2), best.3)
}

/// One connection point found in the model, in part space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinCandidate {
    pub kind: CandidateKind,
    /// Schema frame (back-left-bottom origin), millimetres.
    pub position_mm: [f64; 3],
    /// The side of the part it faces.
    pub side: Side,
    pub size_mm: f64,
    pub round: bool,
}

/// All connection points found in one model.
#[derive(Debug, Clone, PartialEq)]
pub struct PinScan {
    /// Width, height, depth of the part in its own frame.
    pub size_mm: [f64; 3],
    pub orientation: Orientation,
    pub candidates: Vec<PinCandidate>,
}

impl PinScan {
    pub fn openings(&self) -> impl Iterator<Item = &PinCandidate> {
        self.candidates
            .iter()
            .filter(|c| c.kind == CandidateKind::Opening)
    }

    pub fn pins(&self) -> impl Iterator<Item = &PinCandidate> {
        self.candidates
            .iter()
            .filter(|c| c.kind == CandidateKind::Pin)
    }
}

/// Reads the connection points of a STEP model. `size_mm` (width, height,
/// depth from the catalog) fixes the axes exactly as the render mesh does,
/// so the points sit on the mesh.
pub fn scan_step(
    bytes: &[u8],
    size_mm: Option<[f64; 3]>,
    cancel: &AtomicBool,
    deadline: Option<Instant>,
) -> Result<PinScan, BuildError> {
    if bytes.len() > super::MAX_STEP_BYTES {
        return Err(BuildError::TooComplex);
    }
    let text = String::from_utf8_lossy(bytes);
    let guess = size_mm.map_or(60.0, |s| s.iter().cloned().fold(0.0, f64::max));
    let opts = StepOptions {
        chord_tol_mm: chord_tolerance(guess).max(0.05),
        deadline,
        ..StepOptions::default()
    };
    let (soup, _) = step_soup(&text, &opts, cancel)?;
    let (xf, orientation, size) = derive_xf(&soup, size_mm);
    let tol_env = |extent: f64| (0.10 * extent).max(0.8);
    let mut candidates: Vec<PinCandidate> = Vec::new();
    for raw in &soup.features {
        let c = xf.apply(raw.centre);
        let a = unit(xf.apply_dir(raw.axis));
        let lo = [-size[0] / 2.0, -size[1] / 2.0, 0.0];
        let hi = [size[0] / 2.0, size[1] / 2.0, size[2]];
        // The axis the feature points along, snapped; tilted ones are skipped.
        let k = (0..3)
            .max_by(|&i, &j| a[i].abs().total_cmp(&a[j].abs()))
            .unwrap_or(2);
        if a[k].abs() < 0.9 {
            continue;
        }
        let (position, kk, positive) = match raw.kind {
            CandidateKind::Opening => {
                // The envelope face it is on decides which way it faces, so a
                // face with a flipped normal in the file does not matter.
                let (dl, dh) = (c[k] - lo[k], hi[k] - c[k]);
                let positive = dh < dl;
                let d = dl.min(dh);
                if d > tol_env(size[k]) {
                    continue;
                }
                (c, k, positive)
            }
            CandidateKind::Pin => {
                let half = scale(a, raw.length_mm / 2.0);
                let (e0, e1) = (sub(c, half), add(c, half));
                let positive = e1[k] > e0[k];
                let tip = if positive { e1 } else { e0 };
                let beyond = if positive {
                    hi[k] - tip[k]
                } else {
                    tip[k] - lo[k]
                };
                // A pin reaches the outline; a screw sunk in a housing does not.
                if beyond.abs() > 0.5 {
                    continue;
                }
                (tip, k, positive)
            }
        };
        let side = match (kk, positive) {
            (0, true) => Side::Right,
            (0, false) => Side::Left,
            (1, true) => Side::Top,
            (1, false) => Side::Bottom,
            (_, true) => Side::Front,
            // Facing the mounting plate: not reachable once mounted.
            (_, false) => continue,
        };
        candidates.push(PinCandidate {
            kind: raw.kind,
            position_mm: [
                position[0] + size[0] / 2.0,
                position[1] + size[1] / 2.0,
                position[2],
            ],
            side,
            size_mm: raw.size_mm,
            round: raw.round,
        });
    }
    let candidates = dedupe(candidates);
    Ok(PinScan {
        size_mm: size,
        orientation,
        candidates,
    })
}

/// Stepped holes and split cylinders give several features on one axis; the
/// one nearest the outline is the entry.
fn dedupe(mut all: Vec<PinCandidate>) -> Vec<PinCandidate> {
    let outward = |c: &PinCandidate| match c.side {
        Side::Top => c.position_mm[1],
        Side::Bottom => -c.position_mm[1],
        Side::Right => c.position_mm[0],
        Side::Left => -c.position_mm[0],
        _ => c.position_mm[2],
    };
    all.sort_by(|a, b| outward(b).total_cmp(&outward(a)));
    let lateral = |c: &PinCandidate| -> [f64; 2] {
        let p = c.position_mm;
        match c.side {
            Side::Top | Side::Bottom => [p[0], p[2]],
            Side::Left | Side::Right => [p[1], p[2]],
            _ => [p[0], p[1]],
        }
    };
    let mut kept: Vec<PinCandidate> = Vec::new();
    for c in all {
        let l = lateral(&c);
        let dup = kept.iter().any(|k| {
            k.side == c.side && k.kind == c.kind && {
                let m = lateral(k);
                (m[0] - l[0]).hypot(m[1] - l[1]) < 0.6
            }
        });
        if !dup {
            kept.push(c);
        }
    }
    kept
}

/// How the position of a terminal was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fix {
    /// The model's openings facing this side equal the catalog's terminals
    /// there; matched in order along the side.
    Measured,
    /// The model has more openings than terminals on this side; the most
    /// evenly spaced run was taken.
    Partial,
    /// Spread evenly along the side because the model gave nothing to match.
    Estimated,
}

/// A terminal with the position found for it.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalFix {
    pub name: String,
    /// Schema frame, millimetres.
    pub position_mm: [f64; 3],
    pub fix: Fix,
}

/// The result of matching a scan to the terminals of a part.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PinMapping {
    pub fixes: Vec<TerminalFix>,
    /// Terminals left without a position (no side given, nothing to match).
    pub unplaced: Vec<String>,
    /// Candidates no terminal took.
    pub unused_candidates: usize,
}

impl PinMapping {
    /// The weakest way any terminal was placed; `None` when none was.
    pub fn weakest(&self) -> Option<Fix> {
        self.fixes.iter().map(|f| f.fix).max()
    }

    pub fn count(&self, fix: Fix) -> usize {
        self.fixes.iter().filter(|f| f.fix == fix).count()
    }
}

fn wants_pins(kind: ConnectionKind) -> bool {
    matches!(
        kind,
        ConnectionKind::Pin | ConnectionKind::SolderPin | ConnectionKind::Faston
    )
}

/// Reading order of candidates on one side: along x for top and bottom, top
/// to bottom for left and right, rows then columns for the front.
fn reading_order(side: Side, list: &mut [PinCandidate]) {
    match side {
        Side::Top | Side::Bottom => list.sort_by(|a, b| {
            a.position_mm[0]
                .total_cmp(&b.position_mm[0])
                .then(b.position_mm[2].total_cmp(&a.position_mm[2]))
        }),
        Side::Left | Side::Right => list.sort_by(|a, b| {
            b.position_mm[1]
                .total_cmp(&a.position_mm[1])
                .then(a.position_mm[2].total_cmp(&b.position_mm[2]))
        }),
        _ => list.sort_by(|a, b| {
            let rows = (b.position_mm[1] - a.position_mm[1]).abs() > 1.5;
            if rows {
                b.position_mm[1].total_cmp(&a.position_mm[1])
            } else {
                a.position_mm[0].total_cmp(&b.position_mm[0])
            }
        }),
    }
}

/// Spread of the gaps between consecutive candidates along the reading axis.
fn irregularity(list: &[PinCandidate], side: Side) -> f64 {
    let axis = match side {
        Side::Top | Side::Bottom | Side::Front => 0,
        _ => 1,
    };
    if list.len() < 3 {
        return 0.0;
    }
    let gaps: Vec<f64> = list
        .windows(2)
        .map(|w| (w[1].position_mm[axis] - w[0].position_mm[axis]).abs())
        .collect();
    let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
    gaps.iter().map(|g| (g - mean).powi(2)).sum::<f64>() / gaps.len() as f64
}

/// Matches the connection points of a model to the terminals of a part.
/// Terminals are taken in the catalog's order within each side. A terminal
/// without a `side` is placed only when every candidate of its kind is
/// spoken for, so it is left unplaced rather than guessed.
pub fn map_terminals(terminals: &[PartTerminal], scan: &PinScan) -> PinMapping {
    let mut out = PinMapping::default();
    let mut by_side: BTreeMap<u8, Vec<&PartTerminal>> = BTreeMap::new();
    let side_id = |s: Side| match s {
        Side::Top => 0u8,
        Side::Bottom => 1,
        Side::Left => 2,
        Side::Right => 3,
        Side::Front => 4,
        Side::Unknown => 5,
    };
    for t in terminals {
        by_side.entry(side_id(t.side)).or_default().push(t);
    }
    let mut used = 0usize;
    for (sid, list) in &by_side {
        let side = [
            Side::Top,
            Side::Bottom,
            Side::Left,
            Side::Right,
            Side::Front,
        ][(*sid).min(4) as usize];
        if *sid == 5 {
            out.unplaced.extend(list.iter().map(|t| t.name.clone()));
            continue;
        }
        let pins = list.iter().any(|t| wants_pins(t.kind));
        let mut cands: Vec<PinCandidate> = scan
            .candidates
            .iter()
            .filter(|c| c.side == side && (c.kind == CandidateKind::Pin) == pins)
            .copied()
            .collect();
        reading_order(side, &mut cands);
        let n = list.len();
        let (chosen, fix): (Vec<[f64; 3]>, Fix) = if cands.len() == n {
            (cands.iter().map(|c| c.position_mm).collect(), Fix::Measured)
        } else if cands.len() > n && n > 0 {
            let best = (0..=cands.len() - n)
                .min_by(|&a, &b| {
                    irregularity(&cands[a..a + n], side)
                        .total_cmp(&irregularity(&cands[b..b + n], side))
                })
                .unwrap_or(0);
            (
                cands[best..best + n]
                    .iter()
                    .map(|c| c.position_mm)
                    .collect(),
                Fix::Partial,
            )
        } else {
            (estimate(side, n, scan.size_mm), Fix::Estimated)
        };
        if fix != Fix::Estimated {
            used += n;
        }
        for (t, p) in list.iter().zip(chosen) {
            out.fixes.push(TerminalFix {
                name: t.name.clone(),
                position_mm: p,
                fix,
            });
        }
    }
    out.unused_candidates = scan.candidates.len().saturating_sub(used);
    out
}

/// `n` points spread evenly along a side, on the outline.
fn estimate(side: Side, n: usize, size: [f64; 3]) -> Vec<[f64; 3]> {
    let at = |i: usize| (i as f64 + 0.5) / n.max(1) as f64;
    (0..n)
        .map(|i| match side {
            Side::Top => [size[0] * at(i), size[1], size[2] / 2.0],
            Side::Bottom => [size[0] * at(i), 0.0, size[2] / 2.0],
            Side::Left => [0.0, size[1] * (1.0 - at(i)), size[2] / 2.0],
            Side::Right => [size[0], size[1] * (1.0 - at(i)), size[2] / 2.0],
            _ => [size[0] * at(i), size[1] / 2.0, size[2]],
        })
        .collect()
}

/// Writes the positions into the part's terminals and records how they were
/// found in `sources` (`MakerDatasheet` for positions read from the maker's
/// own model, `Estimated` when any is an even spacing). With
/// `with_estimates` false, estimated points are left out and those terminals
/// keep `position_mm` as it was. Returns how many terminals were written.
pub fn apply_to_part(part: &mut CatalogPart, mapping: &PinMapping, with_estimates: bool) -> usize {
    let mut written = 0;
    let mut any_estimate = false;
    for fix in &mapping.fixes {
        if fix.fix == Fix::Estimated {
            any_estimate = true;
            if !with_estimates {
                continue;
            }
        }
        if let Some(t) = part.terminals.iter_mut().find(|t| t.name == fix.name) {
            t.position_mm = Some(fix.position_mm);
            written += 1;
        }
    }
    if written > 0 {
        let source = if any_estimate && with_estimates {
            DataSource::Estimated
        } else {
            DataSource::MakerDatasheet
        };
        part.sources.insert(POSITIONS_SOURCE_KEY.into(), source);
    }
    written
}
