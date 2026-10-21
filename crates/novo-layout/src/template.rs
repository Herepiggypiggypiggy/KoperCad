//! The 1:1 drilling template: the marks of one surface at true size (plate
//! millimetres, y down) with a cross hair on every hole and cut-out centre,
//! and the plan for printing it tiled on A4 or A3 sheets that overlap.
//!
//! Nothing here knows PDF. `novo-pdf` draws the marks and the tiles.

use crate::drill::{CutoutRow, CutoutShape, DeviceFeatures, DrillTable, HoleRow, Surface, mm};
use crate::plate_drawing::{HAlign, Layer, Marks, Shape, Text, VAlign, sheet_size, sheet_surfaces};
use crate::state::CabinetLayout;

/// Paper left unprinted around each sheet (most office printers cannot
/// print closer than 5 mm to the edge).
pub const PAGE_MARGIN_MM: f64 = 10.0;
/// Height of the strip at the foot of a sheet: tile name and scale-check ruler.
pub const FOOTER_MM: f64 = 16.0;
/// How far neighbouring tiles overlap, so they can be matched on the cross
/// marks inside the overlap.
pub const OVERLAP_MM: f64 = 10.0;
/// Length of the scale-check ruler.
pub const RULER_MM: f64 = 100.0;
/// Text height of tags and numbers on the template.
const TEXT_MM: f64 = 3.0;
/// The cross hair arm beyond a hole's edge, at least.
const CROSS_MIN_MM: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    A4,
    A3,
}

impl Paper {
    pub fn name(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::A3 => "A3",
        }
    }

    /// Width and height in portrait.
    pub fn portrait_mm(self) -> (f64, f64) {
        match self {
            Paper::A4 => (210.0, 297.0),
            Paper::A3 => (297.0, 420.0),
        }
    }
}

/// How one surface is cut into printed sheets.
#[derive(Debug, Clone, PartialEq)]
pub struct TilePlan {
    pub paper: Paper,
    pub landscape: bool,
    pub cols: usize,
    pub rows: usize,
    /// Surface size the plan covers.
    pub width: f64,
    pub height: f64,
}

impl TilePlan {
    /// Page size in mm as printed.
    pub fn page_mm(&self) -> (f64, f64) {
        let (w, h) = self.paper.portrait_mm();
        if self.landscape { (h, w) } else { (w, h) }
    }

    /// The area of a sheet that carries the drawing: inside the unprinted
    /// margin and above the footer. Its size equals the plate area one tile covers.
    pub fn content_mm(&self) -> (f64, f64) {
        let (w, h) = self.page_mm();
        (
            w - 2.0 * PAGE_MARGIN_MM,
            h - 2.0 * PAGE_MARGIN_MM - FOOTER_MM,
        )
    }

    pub fn count(&self) -> usize {
        self.cols * self.rows
    }

    /// Distance between the origins of neighbouring tiles.
    pub fn step_mm(&self) -> (f64, f64) {
        let (w, h) = self.content_mm();
        (w - OVERLAP_MM, h - OVERLAP_MM)
    }

    /// Plate position of the top left of tile `(col, row)`, counted from 0.
    pub fn origin(&self, col: usize, row: usize) -> (f64, f64) {
        let (sx, sy) = self.step_mm();
        (col as f64 * sx, row as f64 * sy)
    }

    /// `A1` for the first column and row, `B1` for the second column.
    pub fn name(col: usize, row: usize) -> String {
        let letter = char::from(b'A' + (col % 26) as u8);
        format!("{letter}{}", row + 1)
    }

    /// Plate x positions where a tile starts or ends, sorted. The cross marks
    /// of the tile registration sit on these lines.
    pub fn x_lines(&self) -> Vec<f64> {
        lines(self.cols, self.step_mm().0, self.content_mm().0, self.width)
    }

    pub fn y_lines(&self) -> Vec<f64> {
        lines(
            self.rows,
            self.step_mm().1,
            self.content_mm().1,
            self.height,
        )
    }
}

fn lines(n: usize, step: f64, size: f64, extent: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for i in 0..n {
        out.push(i as f64 * step);
        out.push(i as f64 * step + size);
    }
    // The last tile may reach past the plate; no cross mark lies out there.
    out.retain(|v| *v <= extent + 1e-9);
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    out
}

fn tiles_along(extent: f64, size: f64) -> usize {
    if extent <= size {
        1
    } else {
        ((extent - OVERLAP_MM) / (size - OVERLAP_MM)).ceil() as usize
    }
}

/// Cuts a `width` x `height` surface into the fewest sheets of `paper`,
/// portrait or landscape; a tie goes to the orientation that fits the shape.
pub fn plan_tiles(width: f64, height: f64, paper: Paper) -> TilePlan {
    let make = |landscape: bool| {
        let mut plan = TilePlan {
            paper,
            landscape,
            cols: 1,
            rows: 1,
            width,
            height,
        };
        let (cw, ch) = plan.content_mm();
        plan.cols = tiles_along(width, cw);
        plan.rows = tiles_along(height, ch);
        plan
    };
    let (landscape, portrait) = (make(true), make(false));
    if portrait.count() < landscape.count()
        || (portrait.count() == landscape.count() && height > width)
    {
        portrait
    } else {
        landscape
    }
}

/// One surface at true size.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateSheet {
    pub title: String,
    pub surface: Surface,
    pub width: f64,
    pub height: f64,
    /// The drawing in plate millimetres at 1:1.
    pub marks: Marks,
    pub holes: Vec<HoleRow>,
    pub cutouts: Vec<CutoutRow>,
    /// Devices on this surface the part data gives no hole or cut-out for;
    /// the template shows their outline only.
    pub without_mounting_data: Vec<String>,
    /// Hole patterns that could not be read (plate sheet only): holes the
    /// template is missing.
    pub problems: Vec<String>,
}

fn push_line(m: &mut Marks, layer: Layer, a: (f64, f64), b: (f64, f64)) {
    m.shapes.push((layer, Shape::Line { a, b }));
}

fn push_text(m: &mut Marks, layer: Layer, pos: (f64, f64), h: HAlign, v: VAlign, text: String) {
    m.texts.push(Text {
        layer,
        pos,
        height: TEXT_MM,
        rotation_deg: 0.0,
        h,
        v,
        text,
        bold: false,
    });
}

fn cross(m: &mut Marks, layer: Layer, c: (f64, f64), arm: f64) {
    push_line(m, layer, (c.0 - arm, c.1), (c.0 + arm, c.1));
    push_line(m, layer, (c.0, c.1 - arm), (c.0, c.1 + arm));
}

/// The sheet of `surface`: edge, rails and device outlines (plate only),
/// then every hole and cut-out with a centre cross, number and size.
fn sheet(
    title: &str,
    layout: &CabinetLayout,
    features: &[DeviceFeatures],
    table: &DrillTable,
    surface: &Surface,
) -> TemplateSheet {
    let (w, h) = sheet_size(layout, surface);
    let mut m = Marks::default();
    m.shapes.push((
        Layer::Plate,
        Shape::Rect {
            min: (0.0, 0.0),
            max: (w, h),
        },
    ));
    let mut without = Vec::new();
    if *surface == Surface::Plate {
        for r in &layout.rails {
            for d in [-17.5, 17.5] {
                let (a, b) = if r.vertical {
                    ((r.x + d, r.y), (r.x + d, r.y + r.length))
                } else {
                    ((r.x, r.y + d), (r.x + r.length, r.y + d))
                };
                push_line(&mut m, Layer::Rails, a, b);
            }
        }
        for p in &layout.placements {
            let f = p.footprint();
            m.shapes.push((
                Layer::Devices,
                Shape::Rect {
                    min: (f.x, f.y),
                    max: (f.right(), f.bottom()),
                },
            ));
            let feature = features.iter().find(|f| f.id == p.id);
            let label = feature.map_or_else(|| format!("#{}", p.id), |f| f.label.clone());
            push_text(
                &mut m,
                Layer::Text,
                (f.x + f.w / 2.0, f.y + f.h / 2.0),
                HAlign::Center,
                VAlign::Middle,
                label.clone(),
            );
            if feature.is_some_and(|f| {
                f.surface == Surface::Plate && f.holes.is_empty() && f.cutout.is_none()
            }) {
                without.push(label);
            }
        }
    }

    let (holes, cutouts) = table.on(surface);
    for hr in &holes {
        let r = hr.dia / 2.0;
        m.shapes.push((
            Layer::Holes,
            Shape::Circle {
                centre: (hr.x, hr.y),
                radius: r,
            },
        ));
        let arm = r + CROSS_MIN_MM;
        cross(&mut m, Layer::Holes, (hr.x, hr.y), arm);
        push_text(
            &mut m,
            Layer::Holes,
            (hr.x + arm + 0.5, hr.y - 0.5),
            HAlign::Left,
            VAlign::Bottom,
            format!("{}  \u{d8}{}", hr.no, mm(hr.dia)),
        );
    }
    for c in &cutouts {
        let (half_w, half_h) = match c.shape {
            CutoutShape::Rect { w, h } => {
                m.shapes.push((
                    Layer::Cutouts,
                    Shape::Rect {
                        min: (c.x - w / 2.0, c.y - h / 2.0),
                        max: (c.x + w / 2.0, c.y + h / 2.0),
                    },
                ));
                (w / 2.0, h / 2.0)
            }
            CutoutShape::Round { dia } => {
                m.shapes.push((
                    Layer::Cutouts,
                    Shape::Circle {
                        centre: (c.x, c.y),
                        radius: dia / 2.0,
                    },
                ));
                (dia / 2.0, dia / 2.0)
            }
        };
        cross(
            &mut m,
            Layer::Cutouts,
            (c.x, c.y),
            half_w.min(half_h).max(CROSS_MIN_MM),
        );
        push_text(
            &mut m,
            Layer::Cutouts,
            (c.x + half_w + 1.0, c.y - half_h),
            HAlign::Left,
            VAlign::Top,
            format!("C{}  {}", c.no, c.shape.describe()),
        );
    }
    TemplateSheet {
        title: title.to_owned(),
        surface: surface.clone(),
        width: w,
        height: h,
        marks: m,
        holes: holes.into_iter().cloned().collect(),
        cutouts: cutouts.into_iter().cloned().collect(),
        without_mounting_data: without,
        problems: if *surface == Surface::Plate {
            table.problems.clone()
        } else {
            Vec::new()
        },
    }
}

/// The template sheets for a layout: the mounting plate first, then every
/// other surface with holes or cut-outs.
pub fn template_sheets(
    title: &str,
    layout: &CabinetLayout,
    features: &[DeviceFeatures],
) -> (Vec<TemplateSheet>, DrillTable) {
    let table = crate::drill::drill_table(layout, features);
    let sheets = sheet_surfaces(&table)
        .iter()
        .map(|s| sheet(title, layout, features, &table, s))
        .collect();
    (sheets, table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_plate_is_one_sheet() {
        let plan = plan_tiles(150.0, 200.0, Paper::A4);
        assert_eq!(plan.count(), 1);
        assert!(!plan.landscape, "a tall plate is printed upright");
    }

    #[test]
    fn tiles_cover_the_plate_with_the_overlap() {
        for (w, h) in [(600.0, 800.0), (1200.0, 300.0), (190.0, 277.1)] {
            for paper in [Paper::A4, Paper::A3] {
                let plan = plan_tiles(w, h, paper);
                let (cw, ch) = plan.content_mm();
                let (sx, sy) = plan.step_mm();
                let reach_x = (plan.cols - 1) as f64 * sx + cw;
                let reach_y = (plan.rows - 1) as f64 * sy + ch;
                assert!(
                    reach_x >= w - 1e-9 && reach_y >= h - 1e-9,
                    "{w}x{h} {paper:?}"
                );
                if plan.cols > 1 {
                    // Dropping the last column would leave part of the plate bare.
                    assert!((plan.cols - 2) as f64 * sx + cw < w);
                }
                if plan.rows > 1 {
                    assert!((plan.rows - 2) as f64 * sy + ch < h);
                }
            }
        }
    }

    #[test]
    fn a3_needs_fewer_sheets_than_a4() {
        let a4 = plan_tiles(600.0, 800.0, Paper::A4).count();
        let a3 = plan_tiles(600.0, 800.0, Paper::A3).count();
        assert!(a3 < a4, "{a3} vs {a4}");
    }

    #[test]
    fn orientation_with_fewer_sheets_wins() {
        // 270 x 170 mm fits one A4 sheet turned, not upright.
        let plan = plan_tiles(270.0, 170.0, Paper::A4);
        assert!(plan.landscape);
        assert_eq!(plan.count(), 1);
    }

    #[test]
    fn tile_names_run_along_the_columns() {
        assert_eq!(TilePlan::name(0, 0), "A1");
        assert_eq!(TilePlan::name(1, 2), "B3");
    }

    #[test]
    fn registration_lines_sit_in_the_overlaps() {
        let plan = plan_tiles(600.0, 400.0, Paper::A4);
        let xs = plan.x_lines();
        let (sx, _) = plan.step_mm();
        // Tile 2 starts at `sx`, which lies inside tile 1 (it ends at sx + overlap).
        assert!(xs.contains(&sx));
        assert!(xs.contains(&(sx + OVERLAP_MM)));
    }
}
