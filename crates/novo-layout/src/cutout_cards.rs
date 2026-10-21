//! Door cut-out cards and the print-scale check for the hand workshop.
//!
//! A template taped to a door is only as good as the print scale, so this
//! module holds two things next to the tiled template:
//!
//! * [`cutout_cards`]: one card per cut-out shape of a surface (a door, a
//!   wall), each listing where that shape goes as distances from the
//!   surface edges, so a tape measure can put the mark where the cross
//!   hair belongs even before the template is taped on, and
//! * [`scale_advice`]: what to do with the two sides of the printed check
//!   square once someone has measured them.
//!
//! Nothing here knows PDF. `novo-pdf` draws the cards and the check page.

use crate::drill::{CutoutRow, CutoutShape, Surface, mm};
use crate::template::TemplateSheet;

/// Side of the print-scale check square, mm.
pub const CHECK_SQUARE_MM: f64 = 100.0;
/// The print is accepted when both sides of the square are within this of
/// 100 mm (0.2 %: a 1 m door template is then off by 2 mm at most).
pub const SCALE_TOLERANCE_MM: f64 = 0.2;
/// Two sides outside the tolerance that differ by more than this are a printer that stretches one
/// way only; a single scale percentage cannot repair that.
pub const STRETCH_LIMIT_MM: f64 = 0.3;
/// Space a card keeps around the outline for the centre-line ticks, mm.
pub const CARD_ARM_MM: f64 = 15.0;
/// Largest cut-out side a card takes: an A4 sheet minus margins and the arms.
pub const CARD_MAX_MM: f64 = 150.0;

/// What the measured check square says about the print.
#[derive(Debug, Clone, PartialEq)]
pub enum ScaleVerdict {
    /// Both sides are 100 mm within the tolerance.
    Good,
    /// Both sides are off by about the same: print again at `percent` in the
    /// print dialog, an absolute scale that already counts the scale the
    /// measured print was made at (100.4 for a square that measured 99.6 on
    /// a print at 100 %).
    Rescale { percent: f64 },
    /// The sides differ: the printer stretches one way. Another printer or
    /// the PDF viewer's own print path is needed; no percentage helps.
    Stretched,
    /// A number that is not a length above zero.
    Invalid,
}

/// The answer for a measured square.
#[derive(Debug, Clone, PartialEq)]
pub struct ScaleAdvice {
    /// The scale the measured print was made at, percent (100 for the first
    /// round).
    pub printed_at: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub verdict: ScaleVerdict,
}

impl ScaleAdvice {
    /// One sentence for the user.
    pub fn sentence(&self) -> String {
        match &self.verdict {
            ScaleVerdict::Good => format!(
                "Good: {} x {} mm is within {} mm of 100 x 100. Use this print.",
                mm(self.width_mm),
                mm(self.height_mm),
                mm(SCALE_TOLERANCE_MM)
            ),
            ScaleVerdict::Rescale { percent } => format!(
                "Print again at {} % (custom scale in the print dialog, counted from 100 %) and measure again.",
                mm((percent * 10.0).round() / 10.0)
            ),
            ScaleVerdict::Stretched => format!(
                "The sides differ by {} mm: the printer stretches one way. Print from another program or printer; a scale percentage cannot fix this.",
                mm(((self.width_mm - self.height_mm).abs() * 100.0).round() / 100.0)
            ),
            ScaleVerdict::Invalid => {
                "Enter both sides in millimetres and the scale you printed at.".to_owned()
            }
        }
    }
}

/// Judges the measured sides of the printed check square. `printed_at` is
/// the print scale in percent that print was made at (100 for a first print);
/// the advised scale is absolute, so a second round lands on 100 as well.
pub fn scale_advice(width_mm: f64, height_mm: f64, printed_at: f64) -> ScaleAdvice {
    let valid = |v: f64| v.is_finite() && v > 0.0;
    let within = |v: f64| (v - CHECK_SQUARE_MM).abs() <= SCALE_TOLERANCE_MM + 1e-9;
    let verdict = if !valid(width_mm) || !valid(height_mm) || !valid(printed_at) {
        ScaleVerdict::Invalid
    } else if within(width_mm) && within(height_mm) {
        // Inside the tolerance is good, whatever the difference between sides.
        ScaleVerdict::Good
    } else if (width_mm - height_mm).abs() > STRETCH_LIMIT_MM {
        ScaleVerdict::Stretched
    } else {
        let mean = (width_mm + height_mm) / 2.0;
        ScaleVerdict::Rescale {
            percent: printed_at * CHECK_SQUARE_MM / mean,
        }
    };
    ScaleAdvice {
        printed_at,
        width_mm,
        height_mm,
        verdict,
    }
}

/// A square that measures `measured` instead of 100 mm needs this print
/// scale (percent of the one used) to come out right.
pub fn correction_percent(measured_mm: f64) -> f64 {
    CHECK_SQUARE_MM / measured_mm * 100.0
}

/// One cut-out of a card's shape and where it goes.
#[derive(Debug, Clone, PartialEq)]
pub struct CardPosition {
    /// The number in the drill table (`C3`).
    pub no: usize,
    /// Centre from the surface's top left, as in the drill table.
    pub x: f64,
    pub y: f64,
    pub from_left: f64,
    pub from_right: f64,
    pub from_top: f64,
    pub from_bottom: f64,
    /// The device the part data names it for.
    pub source: String,
}

/// All cut-outs of one shape on one surface: one printed card.
#[derive(Debug, Clone, PartialEq)]
pub struct CutoutCard {
    pub title: String,
    pub surface: Surface,
    pub shape: CutoutShape,
    pub positions: Vec<CardPosition>,
    /// The surface the positions are measured on.
    pub surface_width: f64,
    pub surface_height: f64,
}

impl CutoutCard {
    /// Larger side of the outline.
    pub fn extent_mm(&self) -> f64 {
        match self.shape {
            CutoutShape::Rect { w, h } => w.max(h),
            CutoutShape::Round { dia } => dia,
        }
    }

    /// Whether the outline plus its tick arms fits an A4 card.
    pub fn fits_card(&self) -> bool {
        self.extent_mm() <= CARD_MAX_MM
    }
}

fn same_shape(a: &CutoutShape, b: &CutoutShape) -> bool {
    const EPS: f64 = 0.005;
    match (a, b) {
        (CutoutShape::Rect { w, h }, CutoutShape::Rect { w: w2, h: h2 }) => {
            (w - w2).abs() < EPS && (h - h2).abs() < EPS
        }
        (CutoutShape::Round { dia }, CutoutShape::Round { dia: d2 }) => (dia - d2).abs() < EPS,
        _ => false,
    }
}

fn position(c: &CutoutRow, width: f64, height: f64) -> CardPosition {
    let round = |v: f64| (v * 100.0).round() / 100.0;
    CardPosition {
        no: c.no,
        x: c.x,
        y: c.y,
        from_left: round(c.x),
        from_right: round(width - c.x),
        from_top: round(c.y),
        from_bottom: round(height - c.y),
        source: c.source.clone(),
    }
}

/// The cards of one template sheet, in the order the shapes first appear
/// in the drill table; positions keep the table's numbering.
pub fn cutout_cards(sheet: &TemplateSheet) -> Vec<CutoutCard> {
    let mut cards: Vec<CutoutCard> = Vec::new();
    for c in &sheet.cutouts {
        let pos = position(c, sheet.width, sheet.height);
        match cards.iter_mut().find(|k| same_shape(&k.shape, &c.shape)) {
            Some(card) => card.positions.push(pos),
            None => cards.push(CutoutCard {
                title: sheet.title.clone(),
                surface: sheet.surface.clone(),
                shape: c.shape,
                positions: vec![pos],
                surface_width: sheet.width,
                surface_height: sheet.height,
            }),
        }
    }
    cards
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plate_drawing::Marks;

    fn row(no: usize, shape: CutoutShape, x: f64, y: f64) -> CutoutRow {
        CutoutRow {
            no,
            surface: Surface::Door,
            shape,
            x,
            y,
            source: format!("-H{no}"),
        }
    }

    fn door(cutouts: Vec<CutoutRow>) -> TemplateSheet {
        TemplateSheet {
            title: "Cab".into(),
            surface: Surface::Door,
            width: 600.0,
            height: 800.0,
            marks: Marks::default(),
            holes: Vec::new(),
            cutouts,
            without_mounting_data: Vec::new(),
            clip_on: Vec::new(),
            problems: Vec::new(),
        }
    }

    #[test]
    fn exact_square_is_good() {
        assert_eq!(
            scale_advice(100.0, 100.0, 100.0).verdict,
            ScaleVerdict::Good
        );
        assert_eq!(
            scale_advice(100.15, 99.9, 100.0).verdict,
            ScaleVerdict::Good
        );
        // Both sides inside the tolerance but 0.4 apart: still good.
        assert_eq!(scale_advice(100.2, 99.8, 100.0).verdict, ScaleVerdict::Good);
    }

    #[test]
    fn small_print_asks_for_a_larger_scale() {
        // 99.0 mm measured: 100 / 99 = 101.01 %.
        match scale_advice(99.0, 99.0, 100.0).verdict {
            ScaleVerdict::Rescale { percent } => assert!((percent - 101.0101).abs() < 1e-3),
            other => panic!("{other:?}"),
        }
        assert!((correction_percent(102.0) - 98.039).abs() < 1e-3);
    }

    #[test]
    fn rescaling_by_the_advice_lands_on_100() {
        let ScaleVerdict::Rescale { percent } = scale_advice(98.4, 98.5, 100.0).verdict else {
            panic!("expected a rescale");
        };
        let after = 98.45 * percent / 100.0;
        assert!((after - 100.0).abs() < 1e-9);
    }

    #[test]
    fn a_second_round_lands_on_100() {
        // First print at 100 %: 99.0 mm. Advice 101.01 %.
        let ScaleVerdict::Rescale { percent: first } = scale_advice(99.0, 99.0, 100.0).verdict
        else {
            panic!("expected a rescale");
        };
        // The printer scales linearly: the square comes out at 99 x first/100.
        let second = 99.0 * first / 100.0;
        assert!((second - 100.0).abs() < 1e-9);
        // The reviewer's case: printed at 101 %, measured 100.5: 100.5 %, not 99.5.
        let ScaleVerdict::Rescale { percent } = scale_advice(100.5, 100.5, 101.0).verdict else {
            panic!("expected a rescale");
        };
        assert!((percent - 100.5).abs() < 0.01, "{percent}");
        // And that lands on 100: 100.5 mm at 101 % is 99.5 per 100 %; at 100.5 % it is 100.0.
        let after = 100.5 * percent / 101.0;
        assert!((after - 100.0).abs() < 1e-9);
        assert_eq!(
            scale_advice(100.0, 100.0, 0.0).verdict,
            ScaleVerdict::Invalid
        );
    }

    #[test]
    fn one_way_stretch_is_not_a_percentage_job() {
        assert_eq!(
            scale_advice(100.0, 101.0, 100.0).verdict,
            ScaleVerdict::Stretched
        );
        assert!(
            scale_advice(100.0, 101.0, 100.0)
                .sentence()
                .contains("stretches one way")
        );
    }

    #[test]
    fn bad_numbers_are_invalid() {
        for (w, h) in [(0.0, 100.0), (-1.0, 100.0), (f64::NAN, 100.0)] {
            assert_eq!(scale_advice(w, h, 100.0).verdict, ScaleVerdict::Invalid);
        }
    }

    #[test]
    fn same_shapes_share_a_card_with_edge_distances() {
        let r = CutoutShape::Round { dia: 22.5 };
        let sheet = door(vec![
            row(1, r, 100.0, 150.0),
            row(2, CutoutShape::Rect { w: 92.0, h: 92.0 }, 300.0, 400.0),
            row(3, r, 200.0, 150.0),
        ]);
        let cards = cutout_cards(&sheet);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].positions.len(), 2);
        let p = &cards[0].positions[1];
        assert_eq!(
            (p.no, p.from_left, p.from_right, p.from_top, p.from_bottom),
            (3, 200.0, 400.0, 150.0, 650.0)
        );
        // Left plus right is always the surface width.
        for card in &cards {
            for p in &card.positions {
                assert!((p.from_left + p.from_right - 600.0).abs() < 1e-9);
                assert!((p.from_top + p.from_bottom - 800.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn a_rect_and_its_turned_twin_are_two_cards() {
        let sheet = door(vec![
            row(1, CutoutShape::Rect { w: 90.0, h: 60.0 }, 100.0, 100.0),
            row(2, CutoutShape::Rect { w: 60.0, h: 90.0 }, 200.0, 100.0),
        ]);
        assert_eq!(cutout_cards(&sheet).len(), 2);
    }

    #[test]
    fn big_cutouts_do_not_fit_a_card() {
        let sheet = door(vec![row(
            1,
            CutoutShape::Rect { w: 200.0, h: 120.0 },
            300.0,
            400.0,
        )]);
        assert!(!cutout_cards(&sheet)[0].fits_card());
        let small = door(vec![row(1, CutoutShape::Round { dia: 30.5 }, 50.0, 50.0)]);
        assert!(cutout_cards(&small)[0].fits_card());
    }

    #[test]
    fn no_cutouts_no_cards() {
        assert!(cutout_cards(&door(Vec::new())).is_empty());
    }
}
