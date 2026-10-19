//! The mounting plate sheets of a project export: the cabinet layout.
//!
//! A crane configurator project carries one page per cabinet that shows the
//! mounting plate to scale: a rectangle for the plate, rectangles for ducts,
//! two nested rectangles for each DIN rail, one rectangle per device with its
//! tag as text, all in sheet millimetres, scaled to fit the sheet. This module
//! reads those pages back into plate coordinates (millimetres, origin top
//! left of the plate, y down) so the cabinet view gets a real plate, rails,
//! ducts and placed devices.
//!
//! Exact: where each rectangle sits relative to the plate, after undoing the
//! sheet's scale (read from the plate's drawn width and the size in the page
//! title). Inferred: which rectangle is a rail (a pair of rectangles with the
//! same span and centre, 35 and 20 mm high), which is a duct (any other thin
//! rectangle drawn with the thin pen), which device a tag text belongs to (the
//! text starting nearest inside or just above the rectangle, the biggest of
//! the nested outlines), the scale when the title has no size (from the
//! 35 mm rails), the duct's
//! cross-section (the length across what is drawn, 75 mm deep), and a margin
//! of 15 mm for the plate's free edge. The device sizes are the sheet's
//! catalogue widths, not the parts' outlines (the generator says so on the
//! sheet itself).

use super::graphics::{PxfShape, ShapeKind};
use super::model::{PxfFile, PxfText};

/// The pen of the plate outline, the thin pen of ducts and rails, and the
/// pen of device rectangles, as the generator writes them.
const PEN_PLATE: f64 = 0.35;
const PEN_THIN: f64 = 0.13;
/// The sheet drawing area the generator fits the plate into.
const SHEET_X: (f64, f64) = (0.0, 420.0);
const RAIL_OUTER: f64 = 35.0;
const RAIL_INNER: f64 = 20.0;
const PLATE_MARGIN: f64 = 15.0;
const EPS: f64 = 0.6;

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutRail {
    /// Start of the centre line, plate mm.
    pub x: f64,
    pub y: f64,
    pub length: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutDuct {
    /// Start of the centre line, plate mm.
    pub x: f64,
    pub y: f64,
    pub length: f64,
    /// Across the run.
    pub width: f64,
    pub vertical: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutDevice {
    /// `-Q011`, the device part of the tag.
    pub name: String,
    /// The function code the sheet prints under the tag (`=A1` without `=`).
    pub function: String,
    /// Top left of the footprint, plate mm.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// A terminal strip, with the number of terminals the sheet states.
    pub strip: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutSheet {
    /// Index into the file's pages.
    pub page: usize,
    /// `CAB1`, without the plus.
    pub location: String,
    /// The cabinet type from the title, e.g. `Boxco BX 1060`.
    pub cabinet: String,
    pub width: f64,
    pub height: f64,
    pub margin: f64,
    pub rails: Vec<LayoutRail>,
    pub ducts: Vec<LayoutDuct>,
    pub devices: Vec<LayoutDevice>,
    /// Page millimetres per plate millimetre.
    pub scale: f64,
    /// Things the person should know about this sheet.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct R {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    pen: f64,
}

impl R {
    fn w(&self) -> f64 {
        self.x1 - self.x0
    }
    fn h(&self) -> f64 {
        self.y1 - self.y0
    }
}

fn pen_is(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.02
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= EPS
}

/// `Kast +CAB1: Boxco BX 1060 600x760x210 mm  (note)`.
fn parse_title(t: &str) -> Option<(String, String, f64, f64)> {
    let rest = t.trim().strip_prefix("Kast")?.trim_start();
    let rest = rest.strip_prefix('+').unwrap_or(rest);
    let (loc, rest) = rest.split_once(':')?;
    let toks: Vec<&str> = rest.split_whitespace().collect();
    let i = toks.iter().position(|t| {
        let p: Vec<&str> = t.split('x').collect();
        p.len() == 3
            && p.iter()
                .all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    })?;
    let dims: Vec<f64> = toks[i].split('x').filter_map(|n| n.parse().ok()).collect();
    Some((
        loc.trim().to_string(),
        toks[..i].join(" "),
        *dims.first()?,
        *dims.get(1)?,
    ))
}

fn rects(file: &PxfFile, page: usize) -> Vec<R> {
    let pen = |p: &PxfShape| p.pen.unwrap_or(PEN_THIN);
    file.shapes
        .iter()
        .filter(|s| s.page == Some(page))
        .filter_map(|s| match &s.kind {
            ShapeKind::Rect { a, b } => {
                let r = R {
                    x0: a.0.min(b.0),
                    y0: a.1.min(b.1),
                    x1: a.0.max(b.0),
                    y1: a.1.max(b.1),
                    pen: pen(s),
                };
                // The sheets keep a full-size copy far to the right.
                (r.x0 >= SHEET_X.0 - EPS && r.x1 <= SHEET_X.1 + EPS).then_some(r)
            }
            _ => None,
        })
        .collect()
}

fn texts(file: &PxfFile, page: usize) -> Vec<(&PxfText, String, (f64, f64))> {
    file.texts
        .iter()
        .filter(|t| t.page == Some(page))
        .filter_map(|t| {
            let at = t.position?;
            let s = t.text.get("en_US").trim().to_string();
            (!s.is_empty()).then_some((t, s, at))
        })
        .collect()
}

/// Rail pairs: two nested thin rectangles with the same span and centre
/// whose heights are as 35 to 20. Indexes into `rs`.
fn rail_pairs(rs: &[R]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut used = vec![false; rs.len()];
    for (i, r) in rs.iter().enumerate() {
        if used[i] || !pen_is(r.pen, PEN_THIN) || r.w() < 4.0 * r.h() {
            continue;
        }
        let yc = (r.y0 + r.y1) / 2.0;
        let tol = (r.h() * 0.02).max(0.02);
        let inner = rs.iter().enumerate().find(|(j, q)| {
            *j != i
                && !used[*j]
                && pen_is(q.pen, PEN_THIN)
                && (q.x0 - r.x0).abs() <= tol * 20.0
                && (q.x1 - r.x1).abs() <= tol * 20.0
                && (((q.y0 + q.y1) / 2.0) - yc).abs() <= tol * 5.0
                && ((r.h() / q.h()) - RAIL_OUTER / RAIL_INNER).abs() < 0.06
        });
        if let Some((j, _)) = inner {
            used[i] = true;
            used[j] = true;
            out.push((i, j));
        }
    }
    out
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied()
}

/// Most rectangles or texts one sheet may hold for the layout read; the
/// matching below is quadratic, and a real sheet has a few hundred to a few
/// thousand.
const MAX_SHEET_OBJECTS: usize = 4000;
/// A cabinet plate the title may state, in millimetres.
const PLATE_RANGE: std::ops::RangeInclusive<f64> = 100.0..=5000.0;

/// Reads every mounting plate sheet in the file; the second value says which
/// sheets were left alone and why.
pub(super) fn read_sheets(file: &PxfFile) -> (Vec<LayoutSheet>, Vec<String>) {
    let mut skipped = Vec::new();
    let mut out = Vec::new();
    for (pi, page) in file.pages.iter().enumerate() {
        let ts = texts(file, pi);
        let titled = ts
            .iter()
            .find_map(|(_, s, _)| parse_title(s))
            .filter(|(_, _, w, h)| PLATE_RANGE.contains(w) && PLATE_RANGE.contains(h));
        let named = page
            .title
            .get("en_US")
            .to_lowercase()
            .contains("montageplaat");
        if titled.is_none() && !named {
            continue;
        }
        let rs = rects(file, pi);
        if rs.len() > MAX_SHEET_OBJECTS || ts.len() > MAX_SHEET_OBJECTS {
            skipped.push(format!(
                "Sheet {} has {} rectangles and {} texts; the cabinet layout is not read from sheets that large (limit {MAX_SHEET_OBJECTS}).",
                page.label(),
                rs.len(),
                ts.len()
            ));
            continue;
        }
        // The plate: the biggest rectangle drawn with the plate pen.
        let Some(plate) = rs
            .iter()
            .filter(|r| pen_is(r.pen, PEN_PLATE))
            .max_by(|a, b| (a.w() * a.h()).total_cmp(&(b.w() * b.h())))
            .copied()
        else {
            continue;
        };
        let mut notes = Vec::new();
        let inside_idx: Vec<usize> = (0..rs.len())
            .filter(|&i| {
                let r = &rs[i];
                r.x0 >= plate.x0 - EPS
                    && r.x1 <= plate.x1 + EPS
                    && r.y0 >= plate.y0 - EPS
                    && r.y1 <= plate.y1 + EPS
                    && !(near(r.x0, plate.x0) && near(r.x1, plate.x1) && near(r.y0, plate.y0))
            })
            .collect();
        let inside: Vec<R> = inside_idx.iter().map(|&i| rs[i]).collect();
        let pairs = rail_pairs(&inside);
        // The scale: from the size in the title when there is one, else from
        // the rails, which are 35 mm high.
        let (location, cabinet, s, width, height) = match &titled {
            Some((loc, cab, cw, ch)) if plate.w() > 0.0 && *cw > 0.0 => {
                let s = plate.w() / cw;
                let drawn_h = plate.h() / s;
                if (drawn_h - ch).abs() > 1.0 {
                    notes.push(format!(
                        "The title says {ch:.0} mm high; the drawn plate is {drawn_h:.0} mm at the scale of its width. The drawing is used."
                    ));
                }
                (loc.clone(), cab.clone(), s, *cw, drawn_h)
            }
            _ => {
                let Some(s) = median(
                    pairs
                        .iter()
                        .map(|(o, _)| inside[*o].h() / RAIL_OUTER)
                        .collect(),
                ) else {
                    continue;
                };
                notes.push(format!(
                    "Sheet {}: no cabinet size in the title; the scale ({:.4}) comes from the 35 mm DIN rails, so the plate size ({:.0} x {:.0} mm) is derived, not stated.",
                    page.label(),
                    s,
                    (plate.w() / s).round(),
                    (plate.h() / s).round()
                ));
                (
                    page.location.clone(),
                    String::new(),
                    s,
                    (plate.w() / s).round(),
                    (plate.h() / s).round(),
                )
            }
        };
        let location = location.trim_start_matches('+').to_string();
        if location.is_empty() || s <= 0.0 {
            continue;
        }
        // Page to plate: x right, y down from the plate's top left.
        let px = |x: f64| (x - plate.x0) / s;
        let py = |y: f64| (plate.y1 - y) / s;
        let mut used = vec![false; inside.len()];
        let mut rails = Vec::new();
        for (o, i) in &pairs {
            used[*o] = true;
            used[*i] = true;
            let r = &inside[*o];
            rails.push(LayoutRail {
                x: px(r.x0),
                y: py((r.y0 + r.y1) / 2.0),
                length: r.w() / s,
            });
        }
        let mut ducts = Vec::new();
        for (i, r) in inside.iter().enumerate() {
            if used[i] || !pen_is(r.pen, PEN_THIN) {
                continue;
            }
            let (w, h) = (r.w() / s, r.h() / s);
            let vertical = h > w;
            let (len, across) = if vertical { (h, w) } else { (w, h) };
            if !(15.0..=60.0).contains(&across) || len < 150.0 || len < 2.5 * across {
                continue;
            }
            used[i] = true;
            ducts.push(if vertical {
                LayoutDuct {
                    x: px((r.x0 + r.x1) / 2.0),
                    y: py(r.y1),
                    length: len,
                    width: across,
                    vertical,
                }
            } else {
                LayoutDuct {
                    x: px(r.x0),
                    y: py((r.y0 + r.y1) / 2.0),
                    length: len,
                    width: across,
                    vertical,
                }
            });
        }
        // Devices: for each tag text, the biggest rectangle that starts just
        // left of the text (the outline, not the parts drawn inside it).
        let plate_area = plate.w() * plate.h();
        let mut taken = vec![false; ts.len()];
        let mut devices = Vec::new();
        let mut order: Vec<usize> = (0..ts.len()).collect();
        order.sort_by(|&a, &b| {
            ts[b]
                .2
                .1
                .total_cmp(&ts[a].2.1)
                .then(ts[a].2.0.total_cmp(&ts[b].2.0))
        });
        let mut claimed = vec![false; inside.len()];
        for ti in order {
            let (_, label, at) = &ts[ti];
            if !label.starts_with('-') || taken[ti] {
                continue;
            }
            let reach = 22.0 * s;
            let mut best: Option<(usize, f64)> = None;
            for (i, r) in inside.iter().enumerate() {
                if used[i] || claimed[i] || r.w() * r.h() > 0.25 * plate_area {
                    continue;
                }
                if at.0 < r.x0 - EPS || at.0 > r.x0 + 3.0 * s + EPS {
                    continue;
                }
                if at.1 < r.y0 - EPS || at.1 > r.y1 + reach {
                    continue;
                }
                if r.w() < 4.0 * s || r.h() < 4.0 * s {
                    continue;
                }
                // Inside the rectangle beats above it; then the biggest.
                let d = if at.1 <= r.y1 { 0.0 } else { at.1 - r.y1 };
                let score = d * 1000.0 - r.w() * r.h();
                if best.is_none_or(|(_, bs)| score < bs) {
                    best = Some((i, score));
                }
            }
            let Some((ri, _)) = best else { continue };
            let r = &inside[ri];
            claimed[ri] = true;
            taken[ti] = true;
            let (name, strip) = match label.split_once("  (") {
                Some((n, rest)) => (
                    n.trim().to_string(),
                    rest.split_whitespace().next().and_then(|n| n.parse().ok()),
                ),
                None => (label.trim().to_string(), None),
            };
            // The `=code` text beside the tag, inside the rectangle.
            let function = ts
                .iter()
                .enumerate()
                .find(|(i, (_, t, at))| {
                    !taken[*i]
                        && t.starts_with('=')
                        && at.0 >= r.x0 - EPS
                        && at.0 <= r.x0 + 3.0 * s + EPS
                        && at.1 >= r.y0 - EPS
                        && at.1 <= r.y1 + EPS
                })
                .map(|(i, (_, t, _))| (i, t.trim_start_matches('=').to_string()));
            let function = match function {
                Some((i, f)) => {
                    taken[i] = true;
                    f
                }
                None => String::new(),
            };
            devices.push(LayoutDevice {
                name,
                function,
                x: px(r.x0),
                y: py(r.y1),
                w: r.w() / s,
                h: r.h() / s,
                strip,
            });
        }
        out.push(LayoutSheet {
            page: pi,
            location,
            cabinet,
            width,
            height,
            margin: PLATE_MARGIN,
            rails,
            ducts,
            devices,
            scale: s,
            notes,
        });
    }
    (out, skipped)
}
