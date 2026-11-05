//! Enclosure and mounting-plate bodies read from a maker's STEP model.
//!
//! Rittal (and others) publish the real 3D body of an enclosure or a
//! mounting plate. This module holds what KoperCAD takes from such a body:
//! the kind, the size, and for a plate the holes and openings it is
//! delivered with ("factory holes"). The geometry itself is read by
//! `novo-interop` (`part3d::body`), which this crate cannot depend on, so
//! the import takes a [`BodyReader`] the way it takes a `StepMeasurer`.
//!
//! The values land on the catalog part under the [`keys`] below. They are
//! tagged `drill_basis = maker`, so the hand drilling plan can say they come
//! from the maker's model, and they stay in the user's own catalog: the
//! model files are personal (see `docs/RITTAL.md`), and the share bundle
//! never carries them.

use novo_core::{Properties, Value};

use crate::catalog::Dimensions;

/// Part data keys written by a body reading.
pub mod keys {
    /// `mounting_plate`, `enclosure` or absent.
    pub const BODY_KIND: &str = "body_kind";
    /// Plate width (x, to the right when seen from the front), mm.
    pub const PLATE_WIDTH_MM: &str = "plate_width_mm";
    /// Plate height (y, downward), mm.
    pub const PLATE_HEIGHT_MM: &str = "plate_height_mm";
    /// Sheet thickness, mm.
    pub const PLATE_THICKNESS_MM: &str = "plate_thickness_mm";
    /// Round holes the plate comes with, `x,y,d; x,y,d` in plate mm from
    /// the top left corner. Same text form as `hole_pattern`.
    pub const FACTORY_HOLES: &str = "factory_holes";
    /// Rectangular openings the plate comes with, `x,y,w,h; ...` (centre
    /// and size, plate mm).
    pub const FACTORY_CUTOUTS: &str = "factory_cutouts";
    /// Outer size of an enclosure, mm.
    pub const OUTER_WIDTH_MM: &str = "outer_width_mm";
    pub const OUTER_HEIGHT_MM: &str = "outer_height_mm";
    pub const OUTER_DEPTH_MM: &str = "outer_depth_mm";
}

/// What the body is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    /// A flat sheet with an outline and openings.
    MountingPlate,
    /// A box. Only told apart from other boxes by the part's description.
    Enclosure,
    /// Anything else.
    Other,
}

impl BodyKind {
    pub fn as_text(self) -> Option<&'static str> {
        match self {
            BodyKind::MountingPlate => Some("mounting_plate"),
            BodyKind::Enclosure => Some("enclosure"),
            BodyKind::Other => None,
        }
    }
}

/// What a STEP body read gives, in millimetres.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyReading {
    pub kind: BodyKind,
    /// Plate: width, height, thickness. Others: the box around the solids
    /// along the model's axes.
    pub size_mm: [f64; 3],
    /// Round openings (x, y, diameter), plate mm from the top left, seen
    /// from the front.
    pub holes: Vec<(f64, f64, f64)>,
    /// Rectangular openings (x, y, width, height), centre and size.
    pub cutouts: Vec<(f64, f64, f64, f64)>,
    /// Openings that are neither round nor rectangular (slots, shaped).
    pub other_openings: usize,
    /// What the reader could not decide, in plain words.
    pub notes: Vec<String>,
}

/// Reads STEP text into a body. `Err` is a reason in plain words.
pub type BodyReader = fn(&str) -> Result<BodyReading, String>;

/// Words in a description or type that make a box an enclosure.
const ENCLOSURE_WORDS: &[&str] = &[
    "enclosure",
    "wall-mounted",
    "wall mounted",
    "gehause",
    "gehaeuse",
    "gehäuse",
    "schaltschrank",
    "kast",
    "kasten",
    "cabinet",
    "terminal box",
    "klemmenkasten",
    "compact enclosure",
    "floor-standing",
];

/// Words that make a flat body a mounting plate.
const PLATE_WORDS: &[&str] = &[
    "mounting plate",
    "montageplatte",
    "montageplaat",
    "chassis",
    "back panel",
    "backplate",
];

fn has_word(text: &str, words: &[&str]) -> bool {
    let t = text.to_lowercase();
    words.iter().any(|w| t.contains(w))
}

impl BodyReading {
    /// The kind after the description is taken into account: a flat body is
    /// a mounting plate; a box is an enclosure only when the text says so.
    /// A description that says "mounting plate" for a body that is not flat
    /// is left as it was read (the file may be a kit).
    pub fn kind_with(&self, description: &str) -> BodyKind {
        match self.kind {
            BodyKind::MountingPlate => BodyKind::MountingPlate,
            _ if has_word(description, ENCLOSURE_WORDS) && !has_word(description, PLATE_WORDS) => {
                BodyKind::Enclosure
            }
            other => other,
        }
    }

    /// Width, height and depth of an enclosure. A description such as
    /// `600 x 760 x 210` (width, height, depth, as makers write them) gives
    /// the order; without it the model's sizes are sorted (middle, largest,
    /// smallest), which is a guess and is reported in `notes`.
    pub fn outer_size(&self, description: &str) -> ([f64; 3], bool) {
        let mut sorted = self.size_mm;
        sorted.sort_by(f64::total_cmp);
        let nums = numbers_in(description);
        for w in nums.windows(3) {
            let mut s = [w[0], w[1], w[2]];
            s.sort_by(f64::total_cmp);
            if (0..3).all(|i| (s[i] - sorted[i]).abs() <= 1.5) {
                return ([w[0], w[1], w[2]], true);
            }
        }
        ([sorted[1], sorted[2], sorted[0]], false)
    }

    /// The part data this reading adds. Existing keys of the part are only
    /// replaced when they are body keys.
    pub fn data(&self, description: &str) -> Properties {
        let mut d = Properties::new();
        let kind = self.kind_with(description);
        let Some(text) = kind.as_text() else {
            return d;
        };
        d.insert(keys::BODY_KIND.into(), Value::Text(text.into()));
        match kind {
            BodyKind::MountingPlate => {
                let [w, h, t] = self.size_mm;
                d.insert(keys::PLATE_WIDTH_MM.into(), Value::Float(round2(w)));
                d.insert(keys::PLATE_HEIGHT_MM.into(), Value::Float(round2(h)));
                d.insert(keys::PLATE_THICKNESS_MM.into(), Value::Float(round2(t)));
                if !self.holes.is_empty() {
                    d.insert(
                        keys::FACTORY_HOLES.into(),
                        Value::Text(holes_text(&self.holes)),
                    );
                }
                if !self.cutouts.is_empty() {
                    d.insert(
                        keys::FACTORY_CUTOUTS.into(),
                        Value::Text(cutouts_text(&self.cutouts)),
                    );
                }
                d.insert("drill_basis".into(), Value::Text("maker".into()));
            }
            BodyKind::Enclosure => {
                let ([w, h, depth], _) = self.outer_size(description);
                d.insert(keys::OUTER_WIDTH_MM.into(), Value::Float(round2(w)));
                d.insert(keys::OUTER_HEIGHT_MM.into(), Value::Float(round2(h)));
                d.insert(keys::OUTER_DEPTH_MM.into(), Value::Float(round2(depth)));
            }
            BodyKind::Other => {}
        }
        d
    }

    /// Catalog dimensions for the kind: a plate is width x height x
    /// thickness, an enclosure its outer size.
    pub fn dimensions(&self, description: &str) -> Option<Dimensions> {
        match self.kind_with(description) {
            BodyKind::MountingPlate => Some(Dimensions {
                width_mm: round2(self.size_mm[0]),
                height_mm: round2(self.size_mm[1]),
                depth_mm: round2(self.size_mm[2]),
            }),
            BodyKind::Enclosure => {
                let ([w, h, d], _) = self.outer_size(description);
                Some(Dimensions {
                    width_mm: round2(w),
                    height_mm: round2(h),
                    depth_mm: round2(d),
                })
            }
            BodyKind::Other => None,
        }
    }

    /// Lines for the import summary.
    pub fn summary(&self, description: &str) -> Vec<String> {
        let mut out = Vec::new();
        match self.kind_with(description) {
            BodyKind::MountingPlate => {
                let [w, h, t] = self.size_mm;
                out.push(format!(
                    "Mounting plate read from the model: {} x {} mm, {} mm thick, {} round hole(s), {} rectangular opening(s).",
                    fmt(w), fmt(h), fmt(t), self.holes.len(), self.cutouts.len()
                ));
                if self.other_openings > 0 {
                    out.push(format!(
                        "{} slot or shaped opening(s) were found but are not listed: mark them from the maker's drawing.",
                        self.other_openings
                    ));
                }
                out.push(
                    "Positions are seen from the front with the origin at the top left, taken from the model's axes. Check one hole against the real plate before drilling.".into(),
                );
            }
            BodyKind::Enclosure => {
                let ([w, h, d], from_text) = self.outer_size(description);
                out.push(format!(
                    "Enclosure outer size {} x {} x {} mm (width x height x depth){}.",
                    fmt(w),
                    fmt(h),
                    fmt(d),
                    if from_text {
                        ""
                    } else {
                        ", ordered by size because the description does not give it: check it"
                    }
                ));
            }
            BodyKind::Other => {}
        }
        out.extend(self.notes.iter().cloned());
        out
    }
}

fn fmt(v: f64) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn holes_text(h: &[(f64, f64, f64)]) -> String {
    h.iter()
        .map(|(x, y, d)| format!("{},{},{}", fmt(*x), fmt(*y), fmt(*d)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn cutouts_text(c: &[(f64, f64, f64, f64)]) -> String {
    c.iter()
        .map(|(x, y, w, h)| format!("{},{},{},{}", fmt(*x), fmt(*y), fmt(*w), fmt(*h)))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Numbers in a text (`600 x 760 x 210 mm`, `600x760x210`, `600,5`).
fn numbers_in(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<f64>| {
        if let Ok(v) = cur
            .trim_matches(|c| c == '.' || c == ',')
            .replace(',', ".")
            .parse::<f64>()
        {
            out.push(v);
        }
        cur.clear();
    };
    for c in text.chars() {
        if c.is_ascii_digit() || ((c == '.' || c == ',') && !cur.is_empty()) {
            cur.push(c);
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// Reads `x,y,d; ...` back into holes (the [`keys::FACTORY_HOLES`] form).
pub fn parse_factory_holes(text: &str) -> Vec<(f64, f64, f64)> {
    text.split([';', '\n'])
        .filter_map(|e| {
            let v: Vec<f64> = e.split(',').filter_map(|n| n.trim().parse().ok()).collect();
            match v.as_slice() {
                [x, y, d] if *d > 0.0 => Some((*x, *y, *d)),
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate() -> BodyReading {
        BodyReading {
            kind: BodyKind::MountingPlate,
            size_mm: [500.0, 700.0, 3.0],
            holes: vec![(25.0, 25.0, 9.0), (475.0, 25.0, 9.0)],
            cutouts: vec![(250.0, 350.0, 40.0, 20.0)],
            other_openings: 2,
            notes: vec![],
        }
    }

    fn box_body() -> BodyReading {
        BodyReading {
            kind: BodyKind::Other,
            size_mm: [210.0, 600.0, 760.0],
            holes: vec![],
            cutouts: vec![],
            other_openings: 0,
            notes: vec![],
        }
    }

    #[test]
    fn a_plate_gives_size_holes_and_maker_basis() {
        let d = plate().data("Mounting plate for AE 600 x 800");
        assert_eq!(d["body_kind"], Value::Text("mounting_plate".into()));
        assert_eq!(d["plate_width_mm"], Value::Float(500.0));
        assert_eq!(d["factory_holes"], Value::Text("25,25,9; 475,25,9".into()));
        assert_eq!(d["factory_cutouts"], Value::Text("250,350,40,20".into()));
        assert_eq!(d["drill_basis"], Value::Text("maker".into()));
        assert_eq!(
            parse_factory_holes("25,25,9; 475,25,9"),
            vec![(25.0, 25.0, 9.0), (475.0, 25.0, 9.0)]
        );
    }

    #[test]
    fn a_box_is_an_enclosure_only_when_the_text_says_so() {
        let b = box_body();
        assert_eq!(
            b.kind_with("Compact enclosure, 600 x 760 x 210 mm"),
            BodyKind::Enclosure
        );
        assert_eq!(b.kind_with("Terminal block"), BodyKind::Other);
        assert!(b.data("Terminal block").is_empty());
    }

    #[test]
    fn the_description_orders_width_height_depth_else_sizes_are_sorted() {
        let b = box_body();
        let (s, from_text) = b.outer_size("Wall-mounted enclosure 600 x 760 x 210");
        assert_eq!(s, [600.0, 760.0, 210.0]);
        assert!(from_text);
        // Wide cabinet: the text order wins over the sorted guess.
        let wide = BodyReading {
            size_mm: [300.0, 1200.0, 760.0],
            ..box_body()
        };
        let (s, from_text) = wide.outer_size("Enclosure 1200 x 760 x 300");
        assert_eq!(s, [1200.0, 760.0, 300.0]);
        assert!(from_text);
        let (s, from_text) = b.outer_size("Enclosure");
        assert_eq!(s, [600.0, 760.0, 210.0]);
        assert!(!from_text);
        assert!(
            b.summary("Enclosure")
                .iter()
                .any(|l| l.contains("check it"))
        );
    }

    #[test]
    fn a_flat_body_stays_a_plate_whatever_the_text() {
        assert_eq!(plate().kind_with("enclosure"), BodyKind::MountingPlate);
        let s = plate().summary("");
        assert!(s.iter().any(|l| l.contains("500 x 700 mm, 3 mm thick")));
        assert!(s.iter().any(|l| l.contains("2 slot")));
    }
}
