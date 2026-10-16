//! The contact table under a coil: one row per contact group with a small
//! contact symbol, its connection designations and where it is drawn, in
//! aligned columns (the cross-reference contact image of IEC 61082, laid
//! out like a contact mirror).
//!
//! The layout is a pure function of the rows, the density and the text
//! height, so the canvas and the PDF draw the same table and tests can
//! check it without a window.

use glam::Vec2;

use crate::geom::Aabb;
use crate::shape::{Primitive, Shape};
use crate::style::{Color, Stroke};
use crate::text::{ADVANCE, Anchor, Label};

/// The contact symbol drawn in a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoteGlyph {
    /// Main contacts: three coupled blades.
    Power,
    NormallyOpen,
    NormallyClosed,
    Changeover,
}

/// One row of the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteRow {
    pub glyph: NoteGlyph,
    /// Connection designations, `13-14`.
    pub pins: String,
    /// Page and column, `3.4`.
    pub location: String,
}

/// How much room a table takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NoteDensity {
    /// Tight rows, small symbols: many contacts on a busy sheet.
    Compact,
    /// Roomy rows with a hairline between them.
    #[default]
    Full,
}

impl NoteDensity {
    pub const ALL: [NoteDensity; 2] = [NoteDensity::Compact, NoteDensity::Full];

    /// Row pitch, glyph width and glyph height as multiples of the text
    /// height.
    fn metrics(self) -> (f32, f32, f32) {
        match self {
            NoteDensity::Compact => (1.25, 1.5, 0.8),
            NoteDensity::Full => (1.75, 2.0, 1.1),
        }
    }

    /// Stable name for settings files.
    pub fn id(self) -> &'static str {
        match self {
            NoteDensity::Compact => "compact",
            NoteDensity::Full => "full",
        }
    }

    pub fn from_id(id: &str) -> Self {
        match id {
            "compact" => NoteDensity::Compact,
            _ => NoteDensity::Full,
        }
    }
}

/// A laid out table: what to draw and the box it takes.
#[derive(Clone, Debug)]
pub struct NoteTable {
    pub primitives: Vec<Primitive>,
    pub labels: Vec<Label>,
    pub bounds: Aabb,
    /// Where each row's text sits vertically, centre line.
    pub row_centres: Vec<f32>,
    /// Left edge of the pins and of the location column.
    pub pins_x: f32,
    pub location_x: f32,
}

impl NoteTable {
    /// Bottom edge of the table, for text placed under it.
    pub fn bottom(&self) -> f32 {
        self.bounds.max.y
    }
}

/// Lays `rows` out with the top left corner of the table at `origin`.
pub fn layout(
    rows: &[NoteRow],
    density: NoteDensity,
    origin: Vec2,
    h: f32,
    stroke: Stroke,
    text: Color,
) -> NoteTable {
    let (pitch, gw, gh) = density.metrics();
    let (pitch, gw, gh) = (pitch * h, gw * h, gh * h);
    let pin_cols = rows
        .iter()
        .map(|r| r.pins.chars().count())
        .max()
        .unwrap_or(0);
    let pins_x = origin.x + gw + 0.6 * h;
    let location_x = pins_x + pin_cols as f32 * h * ADVANCE + 0.9 * h;
    let loc_cols = rows
        .iter()
        .map(|r| r.location.chars().count())
        .max()
        .unwrap_or(0);
    let right = location_x + loc_cols as f32 * h * ADVANCE;
    let mut table = NoteTable {
        primitives: Vec::new(),
        labels: Vec::new(),
        bounds: Aabb::EMPTY,
        row_centres: Vec::new(),
        pins_x,
        location_x,
    };
    for (i, row) in rows.iter().enumerate() {
        let top = origin.y + i as f32 * pitch;
        let cy = top + pitch * 0.5;
        table.row_centres.push(cy);
        glyph(
            &mut table.primitives,
            row.glyph,
            origin.x,
            cy,
            gw,
            gh,
            stroke,
        );
        for (x, s) in [(pins_x, &row.pins), (location_x, &row.location)] {
            table.labels.push(
                Label::new(Vec2::new(x, cy), s.clone(), h)
                    .anchored(Anchor::CenterLeft)
                    .colored(text),
            );
        }
        if density == NoteDensity::Full && i > 0 {
            let hair = Stroke::world(stroke.width * 0.5, stroke.color);
            table.primitives.push(Primitive::stroked(
                Shape::line(Vec2::new(origin.x, top), Vec2::new(right, top)),
                hair,
            ));
        }
    }
    table.bounds = Aabb::from_corners(
        origin,
        Vec2::new(right, origin.y + rows.len() as f32 * pitch),
    );
    table
}

fn seg(out: &mut Vec<Primitive>, a: Vec2, b: Vec2, stroke: Stroke) {
    out.push(Primitive::stroked(Shape::line(a, b), stroke));
}

/// One contact symbol in a box `w` by `gh` whose left edge is `x` and
/// whose centre line is `cy`: horizontal, left terminal to right terminal,
/// the blade lifted for a normally open contact.
fn glyph(out: &mut Vec<Primitive>, kind: NoteGlyph, x: f32, cy: f32, w: f32, gh: f32, s: Stroke) {
    let xl = x + w * 0.3;
    let xr = x + w * 0.7;
    let x1 = x + w;
    match kind {
        NoteGlyph::NormallyOpen | NoteGlyph::NormallyClosed => {
            let lift = gh * 0.45;
            seg(out, Vec2::new(x, cy), Vec2::new(xl, cy), s);
            seg(out, Vec2::new(xr, cy), Vec2::new(x1, cy), s);
            seg(out, Vec2::new(xl, cy), Vec2::new(xr, cy - lift), s);
            if kind == NoteGlyph::NormallyClosed {
                // The hook: the blade rests on the right terminal.
                seg(out, Vec2::new(xr, cy), Vec2::new(xr, cy - lift), s);
            }
        }
        NoteGlyph::Changeover => {
            let level = gh * 0.32;
            seg(out, Vec2::new(x, cy), Vec2::new(xl, cy), s);
            seg(out, Vec2::new(xr, cy - level), Vec2::new(x1, cy - level), s);
            seg(out, Vec2::new(xr, cy + level), Vec2::new(x1, cy + level), s);
            // Rests on the upper (normally closed) terminal.
            seg(out, Vec2::new(xl, cy), Vec2::new(xr, cy - level), s);
            seg(
                out,
                Vec2::new(xr, cy - level),
                Vec2::new(xr, cy - level - gh * 0.12),
                s,
            );
        }
        NoteGlyph::Power => {
            let pitch = gh * 0.36;
            let lift = gh * 0.2;
            for k in [-1.0, 0.0, 1.0] {
                let y = cy + k * pitch;
                seg(out, Vec2::new(x, y), Vec2::new(xl, y), s);
                seg(out, Vec2::new(xr, y), Vec2::new(x1, y), s);
                seg(out, Vec2::new(xl, y), Vec2::new(xr, y - lift), s);
            }
            // The coupling through the blades.
            let mid = Vec2::new((xl + xr) * 0.5, 0.0);
            let dash = Stroke::world(s.width * 0.6, s.color);
            seg(
                out,
                Vec2::new(mid.x, cy - pitch - lift * 0.5),
                Vec2::new(mid.x, cy + pitch - lift * 0.5),
                dash,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(glyph: NoteGlyph, pins: &str, location: &str) -> NoteRow {
        NoteRow {
            glyph,
            pins: pins.into(),
            location: location.into(),
        }
    }

    fn lay(rows: &[NoteRow], density: NoteDensity) -> NoteTable {
        layout(
            rows,
            density,
            Vec2::new(10.0, 20.0),
            2.5,
            Stroke::world(0.25, Color::BLACK),
            Color::BLACK,
        )
    }

    fn sample() -> Vec<NoteRow> {
        vec![
            row(NoteGlyph::Power, "1-2 3-4 5-6", "1.2"),
            row(NoteGlyph::NormallyOpen, "13-14", "2.3"),
            row(NoteGlyph::NormallyClosed, "21-22", "=A2/12.4"),
        ]
    }

    #[test]
    fn one_row_per_contact_group_in_order() {
        let t = lay(&sample(), NoteDensity::Full);
        let texts: Vec<&str> = t.labels.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            texts,
            ["1-2 3-4 5-6", "1.2", "13-14", "2.3", "21-22", "=A2/12.4"]
        );
        assert_eq!(t.row_centres.len(), 3);
        assert!(t.row_centres.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn pins_and_locations_line_up_in_columns() {
        let t = lay(&sample(), NoteDensity::Full);
        for (i, l) in t.labels.iter().enumerate() {
            let want = if i % 2 == 0 { t.pins_x } else { t.location_x };
            assert_eq!(l.pos.x, want);
            assert_eq!(l.pos.y, t.row_centres[i / 2]);
        }
        // The location column starts clear of the longest pins text.
        let widest = Label::new(Vec2::ZERO, "1-2 3-4 5-6", 2.5).size().x;
        assert!(t.location_x >= t.pins_x + widest);
    }

    #[test]
    fn compact_is_tighter_than_full() {
        let c = lay(&sample(), NoteDensity::Compact);
        let f = lay(&sample(), NoteDensity::Full);
        assert!(c.bounds.size().y < f.bounds.size().y);
        assert!(c.bounds.size().x < f.bounds.size().x);
        // Full has hairlines between rows, compact has none.
        let lines = |t: &NoteTable| t.primitives.len();
        assert!(lines(&f) > lines(&c) - 1);
        assert_eq!(f.bounds.min, Vec2::new(10.0, 20.0));
    }

    #[test]
    fn each_glyph_is_drawn_differently() {
        let n = |g| {
            let t = lay(&[row(g, "1", "1.1")], NoteDensity::Compact);
            t.primitives.len()
        };
        assert_eq!(n(NoteGlyph::NormallyOpen), 3);
        assert_eq!(n(NoteGlyph::NormallyClosed), 4);
        assert_eq!(n(NoteGlyph::Changeover), 5);
        assert!(n(NoteGlyph::Power) > 6);
    }

    #[test]
    fn empty_table_takes_no_room() {
        let t = lay(&[], NoteDensity::Full);
        assert!(t.labels.is_empty() && t.primitives.is_empty());
        assert_eq!(t.bottom(), 20.0);
    }

    #[test]
    fn density_ids_round_trip() {
        for d in NoteDensity::ALL {
            assert_eq!(NoteDensity::from_id(d.id()), d);
        }
        assert_eq!(NoteDensity::from_id(""), NoteDensity::Full);
    }
}
