//! Wiring openings (PANEL-B2): holes and cut-outs that wires pass through, a
//! grommet hole in the plate, a slot between two plates, a cable entry in a
//! wall. An opening keeps devices off it, and an opening that joins two
//! surfaces tells routing where a wire may change from one plate to the other.
//!
//! No data model change: an opening is a structure property
//! `opening.<name>` = `"<surface> round D X Y"` or `"<surface> rect W H X Y"`,
//! optionally followed by `keep K` (extra free ring around it in mm, default
//! [`DEFAULT_KEEP_MM`]) and `to <surface> X Y` (the other end of a
//! pass-through). X and Y are the centre in the surface's own millimetres.
//!
//! In this slice the keep-out is only checked for devices on the main plate;
//! openings on other surfaces are valid and usable for pass-through, and an
//! opening whose surface (or exit surface) does not exist is reported by
//! [`unknown_surfaces`].

use crate::surfaces::{MAIN_PLATE, SurfaceKind};
use crate::{Placement, Rect};

/// Property prefix of opening definitions.
pub const OPENING_PREFIX: &str = "opening.";
/// Rule name of a device that covers a wiring opening.
pub const RULE_OPENING_KEEPOUT: &str = "opening.keepout";
/// Free ring kept around an opening when none is given.
pub const DEFAULT_KEEP_MM: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpeningShape {
    Round { dia: f64 },
    Rect { w: f64, h: f64 },
}

impl OpeningShape {
    fn half_extent(self) -> (f64, f64) {
        match self {
            OpeningShape::Round { dia } => (dia / 2.0, dia / 2.0),
            OpeningShape::Rect { w, h } => (w / 2.0, h / 2.0),
        }
    }
}

/// The far end of a pass-through.
#[derive(Debug, Clone, PartialEq)]
pub struct Exit {
    pub surface: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Opening {
    pub name: String,
    pub surface: String,
    pub shape: OpeningShape,
    pub x: f64,
    pub y: f64,
    pub keep: f64,
    pub exit: Option<Exit>,
}

/// Structure property key of the opening called `name`.
pub fn key(name: &str) -> String {
    format!("{OPENING_PREFIX}{}", name.trim().to_ascii_lowercase())
}

fn nums(t: &[&str]) -> Option<Vec<f64>> {
    t.iter()
        .map(|s| {
            s.replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
        })
        .collect()
}

impl Opening {
    /// Reads `text` of the property for `name`; the problem is a sentence.
    pub fn parse(name: &str, text: &str) -> Result<Opening, String> {
        let bad = || {
            format!(
                "Opening '{name}' should read '<surface> round D X Y' or '<surface> rect W H X Y', then optionally 'keep K' and 'to <surface> X Y'."
            )
        };
        let t: Vec<&str> = text.split_whitespace().collect();
        let surface = t.first().ok_or_else(bad)?.to_ascii_lowercase();
        let (shape, rest) = match t.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("round") => {
                let n = nums(t.get(2..5).ok_or_else(bad)?).ok_or_else(bad)?;
                if n[0] <= 0.0 {
                    return Err(bad());
                }
                (OpeningShape::Round { dia: n[0] }, (n[1], n[2], 5))
            }
            Some("rect") => {
                let n = nums(t.get(2..6).ok_or_else(bad)?).ok_or_else(bad)?;
                if n[0] <= 0.0 || n[1] <= 0.0 {
                    return Err(bad());
                }
                (OpeningShape::Rect { w: n[0], h: n[1] }, (n[2], n[3], 6))
            }
            _ => return Err(bad()),
        };
        let (x, y, mut i) = rest;
        let mut keep = DEFAULT_KEEP_MM;
        let mut exit = None;
        while i < t.len() {
            match t[i].to_ascii_lowercase().as_str() {
                "keep" => {
                    let n = nums(t.get(i + 1..i + 2).ok_or_else(bad)?).ok_or_else(bad)?;
                    if n[0] < 0.0 {
                        return Err(bad());
                    }
                    keep = n[0];
                    i += 2;
                }
                "to" => {
                    let n = nums(t.get(i + 2..i + 4).ok_or_else(bad)?).ok_or_else(bad)?;
                    exit = Some(Exit {
                        surface: t[i + 1].to_ascii_lowercase(),
                        x: n[0],
                        y: n[1],
                    });
                    i += 4;
                }
                _ => return Err(bad()),
            }
        }
        Ok(Opening {
            name: name.trim().to_ascii_lowercase(),
            surface,
            shape,
            x,
            y,
            keep,
            exit,
        })
    }

    /// The area no device may cover: the opening plus its free ring. A round
    /// opening gets a square, so a device touching a corner of the square
    /// but clear of the circle is flagged too (conservative).
    pub fn keep_out(&self) -> Rect {
        let (hx, hy) = self.shape.half_extent();
        Rect {
            x: self.x - hx - self.keep,
            y: self.y - hy - self.keep,
            w: 2.0 * (hx + self.keep),
            h: 2.0 * (hy + self.keep),
        }
    }

    /// True when this opening is on the main plate, with no surface named.
    fn on(&self, surface: &str) -> bool {
        self.surface == surface
    }
}

/// All openings of a structure from its `(name, text)` properties; a
/// property that cannot be read becomes a problem, never silently dropped.
pub fn parse_all(props: &[(String, String)]) -> (Vec<Opening>, Vec<String>) {
    let (mut list, mut problems) = (Vec::new(), Vec::new());
    for (name, text) in props {
        match Opening::parse(name, text) {
            Ok(o) => list.push(o),
            Err(p) => problems.push(p),
        }
    }
    (list, problems)
}

/// One sentence per opening whose surface or exit surface is not among
/// `known` (surface names, lower case), so a typo is never silently inert.
pub fn unknown_surfaces(openings: &[Opening], known: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for o in openings {
        let ends = std::iter::once(&o.surface).chain(o.exit.iter().map(|e| &e.surface));
        for s in ends {
            if !known.iter().any(|k| k == s) {
                out.push(format!(
                    "Opening '{}' names the surface '{s}', which this enclosure does not have (known: {}).",
                    o.name,
                    known.join(", ")
                ));
            }
        }
    }
    out
}

/// A device that covers an opening's keep-out.
#[derive(Debug, Clone, PartialEq)]
pub struct OpeningConflict {
    pub device: u64,
    pub opening: String,
}

impl OpeningConflict {
    pub fn rule(&self) -> &'static str {
        RULE_OPENING_KEEPOUT
    }

    pub fn message(&self, tag_of: &impl Fn(u64) -> String) -> String {
        format!(
            "{} covers the wiring opening '{}' [{}].",
            tag_of(self.device),
            self.opening,
            RULE_OPENING_KEEPOUT
        )
    }
}

/// Devices of `surface` (placements on the main plate when `surface` is
/// [`MAIN_PLATE`]) whose footprint overlaps an opening's keep-out there.
pub fn conflicts(
    openings: &[Opening],
    surface: &str,
    placements: &[Placement],
) -> Vec<OpeningConflict> {
    let mut out = Vec::new();
    for o in openings.iter().filter(|o| o.on(surface)) {
        let zone = o.keep_out();
        for p in placements {
            if p.footprint().overlaps(&zone) {
                out.push(OpeningConflict {
                    device: p.id,
                    opening: o.name.clone(),
                });
            }
        }
    }
    out
}

/// A way for a wire to get from one surface to another.
#[derive(Debug, Clone, PartialEq)]
pub struct Passage<'a> {
    pub opening: &'a Opening,
    /// Where the wire enters and leaves, in each surface's millimetres.
    pub enter: (f64, f64),
    pub leave: (f64, f64),
    /// In-surface path length to and from the opening, in mm; the distance
    /// between the two surfaces is not included, so this is a ranking
    /// figure and not a wire length.
    pub detour: f64,
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// The pass-through from `from` to `to` that adds the least wire for a wire
/// running between the points `a` (on `from`) and `b` (on `to`). The
/// straight line is not measurable across surfaces, so the detour is the
/// path length through the opening, `a` to the opening to `b`. `None` when
/// no opening joins the two surfaces, in either direction.
pub fn best_passage<'a>(
    openings: &'a [Opening],
    from: &str,
    to: &str,
    a: (f64, f64),
    b: (f64, f64),
) -> Option<Passage<'a>> {
    openings
        .iter()
        .filter_map(|o| {
            let exit = o.exit.as_ref()?;
            let (enter, leave) = if o.surface == from && exit.surface == to {
                ((o.x, o.y), (exit.x, exit.y))
            } else if o.surface == to && exit.surface == from {
                ((exit.x, exit.y), (o.x, o.y))
            } else {
                return None;
            };
            Some(Passage {
                opening: o,
                enter,
                leave,
                detour: dist(a, enter) + dist(leave, b),
            })
        })
        .min_by(|p, q| p.detour.total_cmp(&q.detour))
}

/// Surface kind of a name, for callers that need to know whether a surface
/// is a plate (a pass-through between plates) or something else.
pub fn is_plate(surface: &str) -> bool {
    surface == MAIN_PLATE || SurfaceKind::from_name(surface) == Some(SurfaceKind::Plate)
}
