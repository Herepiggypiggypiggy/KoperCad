//! Plate drawing to DXF (R5 of `docs/PRO-PANEL-GAP-ANALYSIS.md`): the
//! dimensioned mounting plate or door, with its drill and cut-out tables
//! beside it.
//!
//! Units are millimetres, 1:1 with the plate. The plate's top left corner is
//! the origin and y points up as DXF wants, so the plate lies below the
//! x axis and a point at (x, y) in the drill table is at (x, -y) in the file.
//! Text is sized for the scale in the title line (a 2.5 mm text at 1:5 is
//! 12.5 mm high), so it reads the same as on the PDF when plotted at that
//! scale. Layers: PLATE, MARGIN, RAILS, DEVICES, HOLES, CUTOUTS, DIMENSIONS,
//! TEXT, TABLE.

use novo_layout::plate_drawing::{
    HAlign as MarkH, Layer as MarkLayer, Marks, PlateDocument, Shape, VAlign as MarkV, scale_text,
};

use crate::dxf::{Drawing, Entity, EntityKind, HAlign, Layer, Point, TextAlign, VAlign};

/// Plot-scale gap between the plate and its tables, in paper millimetres.
const TABLE_GAP_MM: f64 = 15.0;

fn color(layer: MarkLayer) -> u16 {
    match layer {
        MarkLayer::Plate | MarkLayer::Text | MarkLayer::Table => 7,
        MarkLayer::Margin | MarkLayer::Devices => 8,
        MarkLayer::Rails => 5,
        MarkLayer::Holes => 1,
        MarkLayer::Cutouts => 4,
        MarkLayer::Dimensions => 3,
    }
}

fn pt(p: (f64, f64)) -> Point {
    Point::new(p.0, -p.1)
}

fn push_marks(d: &mut Drawing, marks: &Marks) {
    for (layer, shape) in &marks.shapes {
        let kind = match *shape {
            Shape::Line { a, b } => EntityKind::Line { a: pt(a), b: pt(b) },
            Shape::Rect { min, max } => EntityKind::Polyline {
                points: vec![pt(min), pt((max.0, min.1)), pt(max), pt((min.0, max.1))],
                closed: true,
            },
            Shape::Circle { centre, radius } => EntityKind::Circle {
                center: pt(centre),
                radius,
            },
        };
        d.push(Entity::new(layer.name(), kind));
    }
    for t in &marks.texts {
        d.push(Entity::new(
            t.layer.name(),
            EntityKind::Text {
                pos: pt(t.pos),
                height: t.height,
                rotation_deg: -t.rotation_deg,
                text: t.text.clone(),
                align: TextAlign {
                    h: match t.h {
                        MarkH::Left => HAlign::Left,
                        MarkH::Center => HAlign::Center,
                        MarkH::Right => HAlign::Right,
                    },
                    v: match t.v {
                        MarkV::Top => VAlign::Top,
                        MarkV::Middle => VAlign::Middle,
                        MarkV::Bottom => VAlign::Bottom,
                    },
                },
            },
        ));
    }
}

/// The DXF drawing of one plate document. Pass it to
/// [`crate::dxf::write_dxf`].
pub fn plate_dxf(doc: &PlateDocument) -> Drawing {
    let mut d = Drawing::default();
    for layer in MarkLayer::all() {
        d.layers.push(Layer::new(layer.name(), color(layer)));
    }
    let drawing = &doc.drawing;
    push_marks(&mut d, &drawing.marks);

    // Tables to the right of the plate, stacked, at the plot scale.
    let u = drawing.scale;
    let x = drawing.width + TABLE_GAP_MM * u;
    let mut y = 0.0;
    let mut tables = Marks::default();
    for block in &doc.tables {
        tables.extend(block.layout((x, y), u, 0..block.rows.len()));
        y += (novo_layout::plate_drawing::TableBlock::height(block.rows.len())
            + novo_layout::plate_drawing::ROW_MM)
            * u;
    }
    push_marks(&mut d, &tables);

    // Title under the dimensions.
    let mut title = Marks::default();
    title.texts.push(novo_layout::plate_drawing::Text {
        layer: MarkLayer::Text,
        pos: (0.0, drawing.height + 24.0 * u),
        height: 3.5 * u,
        rotation_deg: 0.0,
        h: MarkH::Left,
        v: MarkV::Top,
        text: format!(
            "{} - {} - {} x {} mm - {}",
            doc.title,
            drawing.surface.name(),
            novo_layout::drill::mm(drawing.width),
            novo_layout::drill::mm(drawing.height),
            scale_text(u)
        ),
        bold: true,
    });
    push_marks(&mut d, &title);
    d
}
