//! The contact table under a coil: one row per contact group with a small
//! contact symbol, its connection designations on either side of it and
//! where it is drawn, in aligned columns (the cross-reference contact image
//! of IEC 61082, laid out like a contact mirror).
//!
//! A contact reads `13 [symbol] 14  3.4`: the first terminal left of the
//! symbol, the second right of it, then page and column. A multi-pole
//! contact stacks one line per pole (`1 [symbol] 2`, `3 [symbol] 4`, ...)
//! tied by a dashed line, with the location once beside the group; a
//! changeover has its common terminal left and the two throws stacked on
//! the right. A contact the device has but the drawing does not use is a
//! spare row, drawn dim. Designations that do not split into terminals
//! (a lone pin, `A-B-C-D`) are printed as text right of the symbol.
//!
//! The layout is a pure function of the rows, the density and the text
//! height, so the canvas and the PDF draw the same table and tests can
//! check it without a window.

use glam::Vec2;
use novo_core::SymbolId;

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
    /// The contact symbol this row stands for: a double click on the row
    /// jumps there. `None` for a spare row.
    pub target: Option<SymbolId>,
    /// A contact the device has that no symbol draws: drawn dim.
    pub spare: bool,
}

impl NoteRow {
    pub fn new(glyph: NoteGlyph, pins: &str, location: &str) -> Self {
        NoteRow {
            glyph,
            pins: pins.into(),
            location: location.into(),
            target: None,
            spare: false,
        }
    }

    /// The poles of this row, or `None` when the designations do not split
    /// into terminals and are printed as text instead.
    fn poles(&self) -> Option<Vec<Pole>> {
        let throws = if self.glyph == NoteGlyph::Changeover {
            2
        } else {
            1
        };
        let groups: Vec<Vec<&str>> = self
            .pins
            .split_whitespace()
            .map(|g| g.split('-').filter(|t| !t.is_empty()).collect())
            .collect();
        if groups.is_empty() {
            // Nothing known (a spare contact): blank poles of the usual count.
            let count = if self.glyph == NoteGlyph::Power { 3 } else { 1 };
            return Some(vec![
                Pole {
                    left: String::new(),
                    right: vec![String::new(); throws],
                };
                count
            ]);
        }
        groups
            .iter()
            .map(|g| {
                (g.len() == 1 + throws).then(|| Pole {
                    left: g[0].into(),
                    right: g[1..].iter().map(|t| (*t).into()).collect(),
                })
            })
            .collect()
    }
}

/// One pole of a contact: the terminal on the left, the one or two on the
/// right (a changeover's normally closed throw above its normally open one).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Pole {
    left: String,
    right: Vec<String>,
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
    /// Top and bottom edge of each row (all its poles), for finding the row
    /// under the pointer.
    pub row_spans: Vec<(f32, f32)>,
    /// Left edge of the symbol column, of the right-hand terminal numbers
    /// and of the location column.
    pub glyph_x: f32,
    pub pins_x: f32,
    pub location_x: f32,
}

impl NoteTable {
    /// Bottom edge of the table, for text placed under it.
    pub fn bottom(&self) -> f32 {
        self.bounds.max.y
    }

    /// The row whose band contains `at`, if `at` is inside the table.
    pub fn row_at(&self, at: Vec2) -> Option<usize> {
        if !self.bounds.contains_point(at) {
            return None;
        }
        self.row_spans
            .iter()
            .position(|&(top, bottom)| at.y >= top && at.y < bottom)
    }
}

/// Space between a terminal number and the symbol, in text heights.
const GAP: f32 = 0.3;

/// Height of the text line of a changeover's throw, in text heights: the
/// numbers of its two throws sit this far apart.
const THROW: f32 = 1.2;

/// Lays `rows` out with the top left corner of the table at `origin`.
/// `dim` colours spare rows.
pub fn layout(
    rows: &[NoteRow],
    density: NoteDensity,
    origin: Vec2,
    h: f32,
    stroke: Stroke,
    text: Color,
    dim: Color,
) -> NoteTable {
    let (pitch, gw, gh) = density.metrics();
    let (pitch, gw, gh) = (pitch * h, gw * h, gh * h);
    let poles: Vec<Option<Vec<Pole>>> = rows.iter().map(NoteRow::poles).collect();
    let chars = |it: &mut dyn Iterator<Item = usize>| it.max().unwrap_or(0) as f32 * h * ADVANCE;
    let left_w = chars(
        &mut poles
            .iter()
            .flatten()
            .flatten()
            .map(|p| p.left.chars().count()),
    );
    let glyph_x = origin.x + left_w + if left_w > 0.0 { GAP * h } else { 0.0 };
    let pins_x = glyph_x + gw + GAP * h;
    let right_w = chars(&mut rows.iter().zip(&poles).flat_map(|(r, p)| {
        match p {
            Some(p) => p
                .iter()
                .flat_map(|p| p.right.iter().map(|t| t.chars().count()))
                .collect::<Vec<_>>(),
            None => vec![r.pins.chars().count()],
        }
    }));
    let location_x = pins_x + right_w + 0.9 * h;
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
        row_spans: Vec::new(),
        glyph_x,
        pins_x,
        location_x,
    };
    let label = |x: f32, cy: f32, s: &str, anchor: Anchor, color: Color| {
        Label::new(Vec2::new(x, cy), s.to_string(), h)
            .anchored(anchor)
            .inside()
            .colored(color)
    };
    let mut top = origin.y;
    for (i, row) in rows.iter().enumerate() {
        let (ink, color) = if row.spare {
            (Stroke::world(stroke.width, dim), dim)
        } else {
            (stroke, text)
        };
        let height;
        match &poles[i] {
            Some(poles) => {
                let one = if row.glyph == NoteGlyph::Changeover {
                    pitch.max(THROW * 2.0 * h)
                } else {
                    pitch
                };
                height = one * poles.len() as f32;
                for (k, pole) in poles.iter().enumerate() {
                    let cy = top + one * (k as f32 + 0.5);
                    pole_glyph(&mut table.primitives, row.glyph, glyph_x, cy, gw, h, ink);
                    if !pole.left.is_empty() {
                        table.labels.push(label(
                            glyph_x - GAP * h,
                            cy,
                            &pole.left,
                            Anchor::CenterRight,
                            color,
                        ));
                    }
                    let n = pole.right.len();
                    for (t, s) in pole.right.iter().enumerate().filter(|(_, s)| !s.is_empty()) {
                        let dy = if n == 2 {
                            (t as f32 - 0.5) * THROW * h
                        } else {
                            0.0
                        };
                        table
                            .labels
                            .push(label(pins_x, cy + dy, s, Anchor::CenterLeft, color));
                    }
                }
                if poles.len() > 1 && row.glyph != NoteGlyph::Changeover {
                    couple(
                        &mut table.primitives,
                        glyph_x + gw * 0.5,
                        top + one * 0.5,
                        top + one * (poles.len() as f32 - 0.5),
                        h,
                        ink,
                    );
                }
            }
            None => {
                height = pitch;
                let cy = top + pitch * 0.5;
                glyph(&mut table.primitives, row.glyph, glyph_x, cy, gw, gh, ink);
                table
                    .labels
                    .push(label(pins_x, cy, &row.pins, Anchor::CenterLeft, color));
            }
        }
        let cy = top + height * 0.5;
        table.row_centres.push(cy);
        table.row_spans.push((top, top + height));
        table.labels.push(label(
            location_x,
            cy,
            &row.location,
            Anchor::CenterLeft,
            color,
        ));
        if density == NoteDensity::Full && i > 0 {
            let hair = Stroke::world(stroke.width * 0.5, stroke.color);
            table.primitives.push(Primitive::stroked(
                Shape::line(Vec2::new(origin.x, top), Vec2::new(right, top)),
                hair,
            ));
        }
        top += height;
    }
    table.bounds = Aabb::from_corners(origin, Vec2::new(right, top));
    table
}

fn seg(out: &mut Vec<Primitive>, a: Vec2, b: Vec2, stroke: Stroke) {
    out.push(Primitive::stroked(Shape::line(a, b), stroke));
}

/// The dashed line that couples the poles of one contact, `x` across,
/// from the first pole's centre line `y0` to the last one's `y1`.
fn couple(out: &mut Vec<Primitive>, x: f32, y0: f32, y1: f32, h: f32, s: Stroke) {
    let dash = Stroke::world(s.width * 0.8, s.color);
    let (on, off) = (0.4 * h, 0.3 * h);
    let mut y = y0 - 0.25 * h;
    while y < y1 {
        seg(
            out,
            Vec2::new(x, y),
            Vec2::new(x, (y + on).min(y1 - 0.25 * h)),
            dash,
        );
        y += on + off;
    }
}

/// One pole of a contact in a box `w` wide whose left edge is `x` and
/// whose centre line is `cy`: the terminals at the left and right edge,
/// the blade lifted for a normally open contact. A changeover's throws sit
/// [`THROW`] text heights apart, where [`layout`] puts their numbers.
fn pole_glyph(
    out: &mut Vec<Primitive>,
    kind: NoteGlyph,
    x: f32,
    cy: f32,
    w: f32,
    h: f32,
    s: Stroke,
) {
    let xl = x + w * 0.3;
    let xr = x + w * 0.7;
    let x1 = x + w;
    let lift = h * 0.5;
    match kind {
        NoteGlyph::NormallyOpen | NoteGlyph::Power => {
            seg(out, Vec2::new(x, cy), Vec2::new(xl, cy), s);
            seg(out, Vec2::new(xr, cy), Vec2::new(x1, cy), s);
            seg(out, Vec2::new(xl, cy), Vec2::new(xr, cy - lift), s);
        }
        NoteGlyph::NormallyClosed => {
            seg(out, Vec2::new(x, cy), Vec2::new(xl, cy), s);
            seg(out, Vec2::new(xr, cy), Vec2::new(x1, cy), s);
            seg(out, Vec2::new(xl, cy), Vec2::new(xr, cy - lift), s);
            // The hook: the blade rests on the right terminal.
            seg(out, Vec2::new(xr, cy), Vec2::new(xr, cy - lift), s);
        }
        NoteGlyph::Changeover => {
            let level = THROW * h * 0.5;
            seg(out, Vec2::new(x, cy), Vec2::new(xl, cy), s);
            seg(out, Vec2::new(xr, cy - level), Vec2::new(x1, cy - level), s);
            seg(out, Vec2::new(xr, cy + level), Vec2::new(x1, cy + level), s);
            // Rests on the upper (normally closed) throw.
            seg(out, Vec2::new(xl, cy), Vec2::new(xr, cy - level), s);
            seg(
                out,
                Vec2::new(xr, cy - level),
                Vec2::new(xr, cy - level - h * 0.3),
                s,
            );
        }
    }
}

/// A contact whose designations did not split into terminals: one box
/// `w` by `gh`, a main contact with its three coupled blades.
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
        NoteRow::new(glyph, pins, location)
    }

    fn lay(rows: &[NoteRow], density: NoteDensity) -> NoteTable {
        layout(
            rows,
            density,
            Vec2::new(10.0, 20.0),
            2.5,
            Stroke::world(0.25, Color::BLACK),
            Color::BLACK,
            Color::rgb(150, 150, 150),
        )
    }

    fn sample() -> Vec<NoteRow> {
        vec![
            row(NoteGlyph::Power, "1-2 3-4 5-6", "1.2"),
            row(NoteGlyph::NormallyOpen, "13-14", "2.3"),
            row(NoteGlyph::NormallyClosed, "21-22", "=A2/12.4"),
        ]
    }

    fn texts(t: &NoteTable) -> Vec<&str> {
        t.labels.iter().map(|l| l.text.as_str()).collect()
    }

    fn at<'t>(t: &'t NoteTable, text: &str) -> Vec<&'t Label> {
        t.labels.iter().filter(|l| l.text == text).collect()
    }

    #[test]
    fn a_contact_has_its_first_terminal_left_and_its_second_right_of_the_symbol() {
        let t = lay(
            &[row(NoteGlyph::NormallyOpen, "13-14", "2.3")],
            NoteDensity::Full,
        );
        assert_eq!(texts(&t), ["13", "14", "2.3"]);
        let (l, r, loc) = (&t.labels[0], &t.labels[1], &t.labels[2]);
        // 13 [symbol] 14  2.3, all on one centre line.
        assert!(l.pos.x < t.glyph_x && t.glyph_x < r.pos.x && r.pos.x < loc.pos.x);
        assert_eq!(l.pos.y, r.pos.y);
        assert_eq!(l.pos.y, loc.pos.y);
        assert_eq!(l.anchor, Anchor::CenterRight);
        assert_eq!(r.anchor, Anchor::CenterLeft);
        // The symbol sits between the numbers, clear of both.
        let (lb, rb) = (l.bounds(), r.bounds());
        let ink = t
            .primitives
            .iter()
            .fold(Aabb::EMPTY, |b, p| b.union(&p.shape.bounds()));
        assert!(lb.max.x <= ink.min.x + 1e-3, "{lb:?} {ink:?}");
        assert!(ink.max.x <= rb.min.x + 1e-3, "{ink:?} {rb:?}");
    }

    #[test]
    fn a_three_pole_main_contact_stacks_one_line_per_pole() {
        let t = lay(
            &[row(NoteGlyph::Power, "1-2 3-4 5-6", "1.2")],
            NoteDensity::Compact,
        );
        assert_eq!(texts(&t), ["1", "2", "3", "4", "5", "6", "1.2"]);
        // Three lines, the same pitch apart.
        let ys: Vec<f32> = t.labels[..6].chunks(2).map(|c| c[0].pos.y).collect();
        assert!(ys[0] < ys[1] && ys[1] < ys[2]);
        assert!((ys[1] - ys[0] - (ys[2] - ys[1])).abs() < 1e-4);
        // Both numbers of a pole share its line; the left ones line up.
        for c in t.labels[..6].chunks(2) {
            assert_eq!(c[0].pos.y, c[1].pos.y);
        }
        assert_eq!(t.labels[0].pos.x, t.labels[2].pos.x);
        // The location is given once, in the middle of the group.
        assert_eq!(at(&t, "1.2").len(), 1);
        assert_eq!(at(&t, "1.2")[0].pos.y, ys[1]);
        // One row: the click band covers all three poles.
        assert_eq!(t.row_spans.len(), 1);
        let (top, bottom) = t.row_spans[0];
        assert!(top < ys[0] && ys[2] < bottom);
    }

    #[test]
    fn a_four_pole_contact_draws_four_poles_and_the_poles_are_coupled() {
        let n = |pins: &str| lay(&[row(NoteGlyph::Power, pins, "1.1")], NoteDensity::Compact);
        assert_eq!(texts(&n("1-2 3-4 5-6 7-8")).len(), 9);
        // One pole has no coupling line; two or more do.
        assert_eq!(n("1-2").primitives.len(), 3);
        assert!(n("1-2 3-4").primitives.len() > 6);
    }

    #[test]
    fn a_changeover_has_its_common_terminal_left_and_two_throws_stacked_right() {
        let t = lay(
            &[row(NoteGlyph::Changeover, "11-12-14", "3.1")],
            NoteDensity::Compact,
        );
        assert_eq!(texts(&t), ["11", "12", "14", "3.1"]);
        let (common, nc, no) = (&t.labels[0], &t.labels[1], &t.labels[2]);
        assert!(common.pos.x < t.glyph_x);
        assert_eq!(nc.pos.x, no.pos.x);
        assert!(
            nc.pos.y < no.pos.y,
            "the normally closed throw is the upper one"
        );
        assert_eq!(common.pos.y, (nc.pos.y + no.pos.y) * 0.5);
        // The two numbers do not overlap each other.
        assert!(nc.bounds().max.y <= no.bounds().min.y + 1e-3);
    }

    #[test]
    fn designations_that_do_not_split_are_printed_as_text_beside_the_symbol() {
        let t = lay(
            &[row(NoteGlyph::NormallyOpen, "13", "2.3")],
            NoteDensity::Full,
        );
        assert_eq!(texts(&t), ["13", "2.3"]);
        assert_eq!(t.labels[0].pos.x, t.pins_x);
        assert_eq!(t.glyph_x, 10.0, "no room is kept on the left");
        // Letters and a lone pin of a main contact too.
        let t = lay(
            &[row(NoteGlyph::Power, "L1-T1-L2", "1.1")],
            NoteDensity::Full,
        );
        assert_eq!(texts(&t), ["L1-T1-L2", "1.1"]);
    }

    #[test]
    fn a_spare_contact_is_dim_and_a_used_one_is_not() {
        let mut spare = row(NoteGlyph::NormallyOpen, "23-24", "spare");
        spare.spare = true;
        let dim = Color::rgb(150, 150, 150);
        let t = lay(
            &[row(NoteGlyph::NormallyOpen, "13-14", "2.3"), spare],
            NoteDensity::Compact,
        );
        for l in &t.labels[..3] {
            assert_eq!(l.color, Color::BLACK, "{}", l.text);
        }
        for l in &t.labels[3..] {
            assert_eq!(l.color, dim, "{}", l.text);
        }
        let colors: Vec<Color> = t
            .primitives
            .iter()
            .filter_map(|p| p.stroke.map(|s| s.color))
            .collect();
        assert_eq!(colors.iter().filter(|&&c| c == dim).count(), 3);
        assert_eq!(colors.iter().filter(|&&c| c == Color::BLACK).count(), 3);
    }

    #[test]
    fn an_unnumbered_spare_contact_is_drawn_without_numbers() {
        let mut spare = row(NoteGlyph::Power, "", "spare");
        spare.spare = true;
        let t = lay(&[spare], NoteDensity::Compact);
        assert_eq!(texts(&t), ["spare"]);
        assert_eq!(t.row_spans.len(), 1);
        // Three poles tall, as a main contact is.
        let (top, bottom) = t.row_spans[0];
        assert!((bottom - top - 3.0 * 1.25 * 2.5).abs() < 1e-4);
    }

    #[test]
    fn one_row_per_contact_group_in_order() {
        let t = lay(&sample(), NoteDensity::Full);
        assert_eq!(
            texts(&t),
            [
                "1", "2", "3", "4", "5", "6", "1.2", "13", "14", "2.3", "21", "22", "=A2/12.4"
            ]
        );
        assert_eq!(t.row_centres.len(), 3);
        assert!(t.row_centres.windows(2).all(|w| w[0] < w[1]));
        // The bands follow each other without a gap and fill the table.
        assert_eq!(t.row_spans[0].0, t.bounds.min.y);
        assert_eq!(t.row_spans[2].1, t.bounds.max.y);
        assert_eq!(t.row_spans[0].1, t.row_spans[1].0);
        assert_eq!(t.row_spans[1].1, t.row_spans[2].0);
    }

    #[test]
    fn pins_and_locations_line_up_in_columns() {
        let t = lay(&sample(), NoteDensity::Full);
        for l in &t.labels {
            let x = l.pos.x;
            let known = [t.pins_x, t.location_x];
            let is_left = l.anchor == Anchor::CenterRight;
            assert!(is_left || known.contains(&x), "{} at {x}", l.text);
        }
        // The location column starts clear of the widest right-hand number.
        let widest = Label::new(Vec2::ZERO, "22", 2.5).size().x;
        assert!(t.location_x >= t.pins_x + widest);
        // The left numbers end at the same place, right aligned.
        let left: Vec<f32> = t
            .labels
            .iter()
            .filter(|l| l.anchor == Anchor::CenterRight)
            .map(|l| l.pos.x)
            .collect();
        assert!(left.windows(2).all(|w| w[0] == w[1]));
    }

    #[test]
    fn compact_is_tighter_than_full() {
        let c = lay(&sample(), NoteDensity::Compact);
        let f = lay(&sample(), NoteDensity::Full);
        assert!(c.bounds.size().y < f.bounds.size().y);
        assert!(c.bounds.size().x < f.bounds.size().x);
        // Full has hairlines between rows, compact has none.
        let lines = |t: &NoteTable| t.primitives.len();
        assert!(lines(&f) > lines(&c));
        let aux = &sample()[1..];
        assert_eq!(
            lay(aux, NoteDensity::Full).primitives.len(),
            lay(aux, NoteDensity::Compact).primitives.len() + 1
        );
        assert_eq!(f.bounds.min, Vec2::new(10.0, 20.0));
    }

    #[test]
    fn each_glyph_is_drawn_differently() {
        let n = |g| {
            let t = lay(&[row(g, "1-2", "1.1")], NoteDensity::Compact);
            t.primitives.len()
        };
        assert_eq!(n(NoteGlyph::NormallyOpen), 3);
        assert_eq!(n(NoteGlyph::Power), 3);
        assert_eq!(n(NoteGlyph::NormallyClosed), 4);
        let co = lay(
            &[row(NoteGlyph::Changeover, "11-12-14", "1.1")],
            NoteDensity::Compact,
        );
        assert_eq!(co.primitives.len(), 5);
        // The old text form keeps its three coupled blades.
        let power = lay(&[row(NoteGlyph::Power, "1", "1.1")], NoteDensity::Compact);
        assert!(power.primitives.len() > 6);
    }

    #[test]
    fn the_row_under_a_point_is_found_by_its_band() {
        let t = lay(&sample(), NoteDensity::Compact);
        let x = t.bounds.min.x + 1.0;
        for (i, &(top, bottom)) in t.row_spans.iter().enumerate() {
            assert_eq!(t.row_at(Vec2::new(x, (top + bottom) * 0.5)), Some(i));
        }
        assert_eq!(t.row_at(Vec2::new(x, t.bounds.max.y + 1.0)), None);
        assert_eq!(
            t.row_at(Vec2::new(t.bounds.min.x - 1.0, t.row_centres[0])),
            None
        );
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
