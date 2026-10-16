//! The dimensioned mounting-plate drawing and its drill tables, as plain
//! marks (lines, rectangles, circles and text) that the PDF and DXF writers
//! only have to draw. Nothing here knows either format.
//!
//! Marks are in plate millimetres, y down. Annotation (text height,
//! dimension offsets) is sized in *paper* millimetres and multiplied by the
//! drawing's scale denominator, so a 600 mm plate at 1:5 carries text that
//! prints 2.5 mm high. A CAD user opening the DXF at that scale sees the same.

use crate::drill::{DeviceFeatures, DrillTable, Surface, mm};
use crate::state::CabinetLayout;
use crate::{Placement, Rect};

/// Text height on paper.
pub const TEXT_MM: f64 = 2.5;
/// Row height of the tables on paper.
pub const ROW_MM: f64 = 5.0;
/// Height of a table's title line on paper.
pub const TITLE_MM: f64 = 6.0;
/// Standard drawing scales (denominators), tried smallest first.
pub const SCALES: [f64; 9] = [1.0, 2.0, 2.5, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0];
/// Paper room the drawing may use on the PDF sheet (A3 landscape, beside the
/// table column and above the title block), also used to pick the DXF scale.
pub const DRAWING_AREA_MM: (f64, f64) = (275.0, 250.0);
/// Paper room the annotation needs around the plate: left, right, top, bottom.
const MARGINS_MM: (f64, f64, f64, f64) = (32.0, 8.0, 22.0, 18.0);

/// The smallest standard scale at which the plate and its annotation fit
/// the paper `area`; the smallest drawing scale (largest denominator) when
/// nothing fits.
pub fn choose_scale(width: f64, height: f64, area: (f64, f64)) -> f64 {
    let (l, r, t, b) = MARGINS_MM;
    SCALES
        .iter()
        .copied()
        .find(|d| width / d + l + r <= area.0 && height / d + t + b <= area.1)
        .unwrap_or(SCALES[SCALES.len() - 1])
}

/// Text of a scale for the title block: `1:5`.
pub fn scale_text(scale: f64) -> String {
    format!("1:{}", mm(scale))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    Plate,
    Margin,
    Rails,
    Devices,
    Holes,
    Cutouts,
    Dimensions,
    Text,
    Table,
}

impl Layer {
    /// The layer name in CAD files.
    pub fn name(self) -> &'static str {
        match self {
            Layer::Plate => "PLATE",
            Layer::Margin => "MARGIN",
            Layer::Rails => "RAILS",
            Layer::Devices => "DEVICES",
            Layer::Holes => "HOLES",
            Layer::Cutouts => "CUTOUTS",
            Layer::Dimensions => "DIMENSIONS",
            Layer::Text => "TEXT",
            Layer::Table => "TABLE",
        }
    }

    pub fn all() -> [Layer; 9] {
        [
            Layer::Plate,
            Layer::Margin,
            Layer::Rails,
            Layer::Devices,
            Layer::Holes,
            Layer::Cutouts,
            Layer::Dimensions,
            Layer::Text,
            Layer::Table,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    Line { a: (f64, f64), b: (f64, f64) },
    Rect { min: (f64, f64), max: (f64, f64) },
    Circle { centre: (f64, f64), radius: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Middle,
    Bottom,
}

/// One line of text. `rotation_deg` is clockwise on the y-down page
/// (`-90` reads bottom to top); the anchor is where the text box is pinned.
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub layer: Layer,
    pub pos: (f64, f64),
    pub height: f64,
    pub rotation_deg: f64,
    pub h: HAlign,
    pub v: VAlign,
    pub text: String,
    pub bold: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Marks {
    pub shapes: Vec<(Layer, Shape)>,
    pub texts: Vec<Text>,
}

impl Marks {
    fn line(&mut self, layer: Layer, a: (f64, f64), b: (f64, f64)) {
        self.shapes.push((layer, Shape::Line { a, b }));
    }

    fn rect(&mut self, layer: Layer, min: (f64, f64), max: (f64, f64)) {
        self.shapes.push((layer, Shape::Rect { min, max }));
    }

    fn circle(&mut self, layer: Layer, centre: (f64, f64), radius: f64) {
        self.shapes.push((layer, Shape::Circle { centre, radius }));
    }

    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        layer: Layer,
        pos: (f64, f64),
        height: f64,
        rotation_deg: f64,
        (h, v): (HAlign, VAlign),
        text: impl Into<String>,
        bold: bool,
    ) {
        self.texts.push(Text {
            layer,
            pos,
            height,
            rotation_deg,
            h,
            v,
            text: text.into(),
            bold,
        });
    }

    pub fn extend(&mut self, other: Marks) {
        self.shapes.extend(other.shapes);
        self.texts.extend(other.texts);
    }

    /// The box around every mark; text counts with an estimated width.
    pub fn bounds(&self) -> Option<Rect> {
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for (_, s) in &self.shapes {
            match *s {
                Shape::Line { a, b } => pts.extend([a, b]),
                Shape::Rect { min, max } => pts.extend([min, max]),
                Shape::Circle { centre, radius } => pts.extend([
                    (centre.0 - radius, centre.1 - radius),
                    (centre.0 + radius, centre.1 + radius),
                ]),
            }
        }
        for t in &self.texts {
            let w = t.text.chars().count() as f64 * t.height * 0.6;
            let (x0, x1) = match t.h {
                HAlign::Left => (0.0, w),
                HAlign::Center => (-w / 2.0, w / 2.0),
                HAlign::Right => (-w, 0.0),
            };
            let (y0, y1) = match t.v {
                VAlign::Top => (0.0, t.height),
                VAlign::Middle => (-t.height / 2.0, t.height / 2.0),
                VAlign::Bottom => (-t.height, 0.0),
            };
            let (s, c) = t.rotation_deg.to_radians().sin_cos();
            for (u, v) in [(x0, y0), (x0, y1), (x1, y0), (x1, y1)] {
                pts.push((t.pos.0 + u * c - v * s, t.pos.1 + u * s + v * c));
            }
        }
        let first = *pts.first()?;
        let mut r = Rect {
            x: first.0,
            y: first.1,
            w: 0.0,
            h: 0.0,
        };
        let (mut x1, mut y1) = first;
        for (x, y) in pts {
            r.x = r.x.min(x);
            r.y = r.y.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        r.w = x1 - r.x;
        r.h = y1 - r.y;
        Some(r)
    }
}

/// A table both writers draw alike.
#[derive(Debug, Clone, PartialEq)]
pub struct TableBlock {
    pub title: String,
    pub header: Vec<String>,
    /// Column widths on paper.
    pub widths: Vec<f64>,
    pub rows: Vec<Vec<String>>,
}

impl TableBlock {
    /// Paper height of a chunk of `rows` rows with its title and header.
    pub fn height(rows: usize) -> f64 {
        TITLE_MM + ROW_MM * (rows + 1) as f64
    }

    pub fn width(&self) -> f64 {
        self.widths.iter().sum()
    }

    /// Draws rows `range` with the title and header at `origin` (top left).
    /// `unit` is target millimetres per paper millimetre: 1 on a PDF page,
    /// the scale denominator in plate drawing space.
    pub fn layout(&self, origin: (f64, f64), unit: f64, range: std::ops::Range<usize>) -> Marks {
        let mut m = Marks::default();
        let rows = &self.rows[range.start.min(self.rows.len())..range.end.min(self.rows.len())];
        let cont = if range.start > 0 { " (continued)" } else { "" };
        let h = TEXT_MM * unit;
        m.text(
            Layer::Table,
            (origin.0, origin.1 + TITLE_MM * unit - 1.2 * unit),
            h,
            0.0,
            (HAlign::Left, VAlign::Bottom),
            format!("{}{cont}", self.title),
            true,
        );
        let top = origin.1 + TITLE_MM * unit;
        let width = self.width() * unit;
        let total = (rows.len() + 1) as f64 * ROW_MM * unit;
        m.rect(
            Layer::Table,
            (origin.0, top),
            (origin.0 + width, top + total),
        );
        let mut x = origin.0;
        for (i, w) in self.widths.iter().enumerate() {
            if i > 0 {
                m.line(Layer::Table, (x, top), (x, top + total));
            }
            x += w * unit;
        }
        for r in 0..=rows.len() {
            let y = top + r as f64 * ROW_MM * unit;
            if r > 0 {
                m.line(Layer::Table, (origin.0, y), (origin.0 + width, y));
            }
            let cells: &[String] = if r == 0 { &self.header } else { &rows[r - 1] };
            let mut cx = origin.0;
            for (cell, w) in cells.iter().zip(&self.widths) {
                m.text(
                    Layer::Table,
                    (cx + 1.0 * unit, y + ROW_MM * unit / 2.0),
                    h,
                    0.0,
                    (HAlign::Left, VAlign::Middle),
                    cell.clone(),
                    r == 0,
                );
                cx += w * unit;
            }
        }
        m
    }
}

/// The dimensioned drawing of one surface.
#[derive(Debug, Clone, PartialEq)]
pub struct PlateDrawing {
    pub surface: Surface,
    pub width: f64,
    pub height: f64,
    /// Scale denominator: 5 is 1:5.
    pub scale: f64,
    pub marks: Marks,
}

/// Everything the writers need for one surface: the drawing and its tables.
#[derive(Debug, Clone, PartialEq)]
pub struct PlateDocument {
    /// What the drawing is for, e.g. the enclosure tag.
    pub title: String,
    pub drawing: PlateDrawing,
    pub tables: Vec<TableBlock>,
}

/// The tables of a surface: hole counts per diameter, the numbered holes,
/// and the numbered cut-outs. Empty tables are left out.
pub fn tables(table: &DrillTable, surface: Surface) -> Vec<TableBlock> {
    let s = |v: &str| v.to_string();
    let (holes, cutouts) = table.on(surface);
    let mut out = Vec::new();
    let summary: Vec<_> = table
        .summary()
        .into_iter()
        .filter(|(sf, _, _)| *sf == surface)
        .collect();
    if !summary.is_empty() {
        out.push(TableBlock {
            title: s("Holes by diameter"),
            header: vec![s("Diameter"), s("Count")],
            widths: vec![30.0, 20.0],
            rows: summary
                .iter()
                .map(|(_, d, n)| vec![format!("{} mm", mm(*d)), n.to_string()])
                .collect(),
        });
    }
    if !holes.is_empty() {
        out.push(TableBlock {
            title: s("Drill holes"),
            header: vec![s("No"), s("Dia"), s("X"), s("Y"), s("Source")],
            widths: vec![10.0, 16.0, 22.0, 22.0, 40.0],
            rows: holes
                .iter()
                .map(|h| {
                    vec![
                        h.no.to_string(),
                        mm(h.dia),
                        mm(h.x),
                        mm(h.y),
                        h.source.clone(),
                    ]
                })
                .collect(),
        });
    }
    if !cutouts.is_empty() {
        out.push(TableBlock {
            title: s("Cut-outs"),
            header: vec![s("No"), s("Size"), s("X"), s("Y"), s("Device")],
            widths: vec![10.0, 30.0, 22.0, 22.0, 30.0],
            rows: cutouts
                .iter()
                .map(|c| {
                    vec![
                        format!("C{}", c.no),
                        c.shape.describe(),
                        mm(c.x),
                        mm(c.y),
                        c.source.clone(),
                    ]
                })
                .collect(),
        });
    }
    out
}

/// Distinct values (within 0.05 mm), sorted.
fn distinct(mut values: Vec<f64>) -> Vec<f64> {
    values.sort_by(f64::total_cmp);
    values.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    values
}

/// Width and height of a sheet: the plate, or the door, which takes the
/// inside size of the enclosure when that is set.
fn sheet_size(layout: &CabinetLayout, surface: Surface) -> (f64, f64) {
    match (surface, layout.enclosure) {
        (Surface::Door, Some(e)) if e.width > 0.0 && e.height > 0.0 => (e.width, e.height),
        _ => (layout.plate.width, layout.plate.height),
    }
}

/// Builds the drawing of `surface` at `scale` (denominator).
///
/// The plate drawing shows the plate edge and its margin, the DIN rails,
/// every placed device outline with its tag, the numbered drill holes and
/// cut-outs, overall dimensions, and ordinate dimensions (the X of every
/// feature along the top edge, the Y along the left edge). Ordinate labels
/// closer than one text height to the previous one are left out; the table
/// has every coordinate. The door drawing has the door edge, cut-outs and
/// holes only. The door is as large as the inside of the enclosure (the
/// door is taken as wide and high as the inside, see `enclosure`); without
/// enclosure fields it is taken to be as large as the plate.
pub fn plate_drawing(
    layout: &CabinetLayout,
    features: &[DeviceFeatures],
    table: &DrillTable,
    surface: Surface,
    scale: f64,
) -> PlateDrawing {
    let u = scale;
    let (w, h) = sheet_size(layout, surface);
    let t = TEXT_MM * u;
    let mut m = Marks::default();
    m.rect(Layer::Plate, (0.0, 0.0), (w, h));

    if surface == Surface::Plate {
        let usable = layout.plate.usable();
        if layout.plate.margin > 0.0 {
            m.rect(
                Layer::Margin,
                (usable.x, usable.y),
                (usable.right(), usable.bottom()),
            );
        }
        for r in &layout.rails {
            for d in [-17.5, 17.5] {
                let (a, b) = if r.vertical {
                    ((r.x + d, r.y), (r.x + d, r.y + r.length))
                } else {
                    ((r.x, r.y + d), (r.x + r.length, r.y + d))
                };
                m.line(Layer::Rails, a, b);
            }
        }
        let label = |p: &Placement| {
            features
                .iter()
                .find(|f| f.id == p.id)
                .map_or_else(|| format!("#{}", p.id), |f| f.label.clone())
        };
        for p in &layout.placements {
            let f = p.footprint();
            m.rect(Layer::Devices, (f.x, f.y), (f.right(), f.bottom()));
            m.text(
                Layer::Text,
                (f.x + f.w / 2.0, f.y + f.h / 2.0),
                t,
                0.0,
                (HAlign::Center, VAlign::Middle),
                label(p),
                false,
            );
        }
    }

    let (holes, cutouts) = table.on(surface);
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for hr in &holes {
        m.circle(Layer::Holes, (hr.x, hr.y), hr.dia / 2.0);
        let c = hr.dia / 2.0 + 1.5 * u;
        m.line(Layer::Holes, (hr.x - c, hr.y), (hr.x + c, hr.y));
        m.line(Layer::Holes, (hr.x, hr.y - c), (hr.x, hr.y + c));
        m.text(
            Layer::Holes,
            (hr.x + c, hr.y - 0.5 * u),
            t,
            0.0,
            (HAlign::Left, VAlign::Bottom),
            hr.no.to_string(),
            false,
        );
        xs.push(hr.x);
        ys.push(hr.y);
    }
    for c in &cutouts {
        let (half_w, half_h) = match c.shape {
            crate::drill::CutoutShape::Rect { w, h } => {
                m.rect(
                    Layer::Cutouts,
                    (c.x - w / 2.0, c.y - h / 2.0),
                    (c.x + w / 2.0, c.y + h / 2.0),
                );
                (w / 2.0, h / 2.0)
            }
            crate::drill::CutoutShape::Round { dia } => {
                m.circle(Layer::Cutouts, (c.x, c.y), dia / 2.0);
                (dia / 2.0, dia / 2.0)
            }
        };
        m.text(
            Layer::Cutouts,
            (c.x + half_w + 1.0 * u, c.y - half_h),
            t,
            0.0,
            (HAlign::Left, VAlign::Top),
            format!("C{}", c.no),
            false,
        );
        xs.push(c.x);
        ys.push(c.y);
    }

    // Ordinate dimensions: X along the top, Y along the left.
    let xs = distinct(xs);
    let mut last = f64::NEG_INFINITY;
    for x in xs {
        m.line(Layer::Dimensions, (x, -u), (x, -6.0 * u));
        if x - last >= t * 1.2 {
            m.text(
                Layer::Dimensions,
                (x, -7.0 * u),
                t,
                -90.0,
                (HAlign::Left, VAlign::Middle),
                mm(x),
                false,
            );
            last = x;
        }
    }
    let ys = distinct(ys);
    let mut last = f64::NEG_INFINITY;
    for y in ys {
        m.line(Layer::Dimensions, (-u, y), (-6.0 * u, y));
        if y - last >= t * 1.2 {
            m.text(
                Layer::Dimensions,
                (-7.0 * u, y),
                t,
                0.0,
                (HAlign::Right, VAlign::Middle),
                mm(y),
                false,
            );
            last = y;
        }
    }

    // Overall dimensions: width below, height at the far left.
    let by = h + 12.0 * u;
    m.line(Layer::Dimensions, (0.0, h + 2.0 * u), (0.0, by + 2.0 * u));
    m.line(Layer::Dimensions, (w, h + 2.0 * u), (w, by + 2.0 * u));
    m.line(Layer::Dimensions, (0.0, by), (w, by));
    for x in [0.0, w] {
        m.line(Layer::Dimensions, (x - u, by + u), (x + u, by - u));
    }
    m.text(
        Layer::Dimensions,
        (w / 2.0, by - 0.8 * u),
        t,
        0.0,
        (HAlign::Center, VAlign::Bottom),
        mm(w),
        false,
    );
    let bx = -22.0 * u;
    m.line(Layer::Dimensions, (-2.0 * u, 0.0), (bx - 2.0 * u, 0.0));
    m.line(Layer::Dimensions, (-2.0 * u, h), (bx - 2.0 * u, h));
    m.line(Layer::Dimensions, (bx, 0.0), (bx, h));
    for y in [0.0, h] {
        m.line(Layer::Dimensions, (bx - u, y + u), (bx + u, y - u));
    }
    m.text(
        Layer::Dimensions,
        (bx - 0.8 * u, h / 2.0),
        t,
        -90.0,
        (HAlign::Center, VAlign::Bottom),
        mm(h),
        false,
    );

    PlateDrawing {
        surface,
        width: w,
        height: h,
        scale,
        marks: m,
    }
}

/// The documents to export for a layout: always the mounting plate, and a
/// door sheet when any device has door holes or cut-outs. All sheets of one
/// export share the scale picked for the plate.
pub fn documents(
    title: &str,
    layout: &CabinetLayout,
    features: &[DeviceFeatures],
) -> (Vec<PlateDocument>, DrillTable) {
    let table = crate::drill::drill_table(layout, features);
    let (pw, ph) = sheet_size(layout, Surface::Plate);
    let (dw, dh) = sheet_size(layout, Surface::Door);
    let scale = choose_scale(pw.max(dw), ph.max(dh), DRAWING_AREA_MM);
    let mut docs = Vec::new();
    for surface in [Surface::Plate, Surface::Door] {
        let (holes, cutouts) = table.on(surface);
        if surface == Surface::Door && holes.is_empty() && cutouts.is_empty() {
            continue;
        }
        docs.push(PlateDocument {
            title: title.to_string(),
            drawing: plate_drawing(layout, features, &table, surface, scale),
            tables: tables(&table, surface),
        });
    }
    (docs, table)
}
