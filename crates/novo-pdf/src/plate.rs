//! Plate drawing PDF (R5 of `docs/PRO-PANEL-GAP-ANALYSIS.md`): per surface
//! a dimensioned drawing on an A3 landscape sheet at a standard scale, the
//! drill and cut-out tables beside it, and continuation sheets when the
//! tables are long. The marks come from `novo_layout::plate_drawing`; the
//! DXF writer draws the same ones.

use std::ops::Range;
use std::path::Path;

use novo_canvas::{Anchor, Color, Label, Primitive, Shape, Stroke, Vec2};
use novo_layout::plate_drawing::{
    DRAWING_AREA_MM, HAlign, Layer, Marks, PlateDocument, ROW_MM, Shape as MarkShape, TITLE_MM,
    TableBlock, VAlign, scale_text,
};
use pdf_writer::{Finish, Name, Pdf, Rect, Ref, TextStr};

use crate::draw::{PT_PER_MM, PageWriter};
use crate::{PdfError, font, inter};

/// A3 landscape, in mm.
const PAGE_MM: (f64, f64) = (420.0, 297.0);
/// Space around the sheet border.
const BORDER_MM: f64 = 10.0;
/// Title block: height at the bottom of the frame.
const TITLE_BLOCK_MM: f64 = 15.0;
/// Table column beside the drawing on the first sheet.
const TABLE_X_MM: f64 = 290.0;
/// Where tables stop (above the title block).
const TABLE_BOTTOM_MM: f64 = PAGE_MM.1 - BORDER_MM - TITLE_BLOCK_MM - 4.0;

/// One sheet before it is written: the drawing (first sheet only) and the
/// table marks in paper millimetres.
struct Sheet<'a> {
    doc: &'a PlateDocument,
    drawing: bool,
    tables: Marks,
}

/// Flows the document's tables onto the first sheet's column and then onto
/// continuation sheets, splitting a table between rows when it must.
fn sheets(doc: &PlateDocument) -> Vec<Sheet<'_>> {
    let mut out = vec![Sheet {
        doc,
        drawing: true,
        tables: Marks::default(),
    }];
    let (mut x, mut y) = (TABLE_X_MM, BORDER_MM);
    let mut fresh = true;
    for block in &doc.tables {
        let mut start = 0;
        while start < block.rows.len() {
            let room = TABLE_BOTTOM_MM - y - TITLE_MM - ROW_MM;
            let capacity = (room / ROW_MM).floor().max(0.0) as usize;
            let left = block.rows.len() - start;
            // A table with a single row left would sit alone below its
            // header; move on unless this is already an empty column.
            if capacity < left.min(2) && !fresh {
                out.push(Sheet {
                    doc,
                    drawing: false,
                    tables: Marks::default(),
                });
                (x, y, fresh) = (BORDER_MM, BORDER_MM, true);
                continue;
            }
            let n = capacity.min(left).max(1);
            let range: Range<usize> = start..start + n;
            out.last_mut()
                .unwrap()
                .tables
                .extend(block.layout((x, y), 1.0, range));
            y += TableBlock::height(n) + 4.0;
            start += n;
            fresh = false;
        }
    }
    out
}

pub(crate) fn stroke_width(layer: Layer) -> f32 {
    match layer {
        Layer::Plate => 0.5,
        Layer::Devices | Layer::Cutouts => 0.35,
        Layer::Rails | Layer::Holes => 0.25,
        Layer::Dimensions | Layer::Table => 0.18,
        Layer::Margin | Layer::Text => 0.15,
    }
}

pub(crate) fn layer_color(layer: Layer) -> Color {
    match layer {
        Layer::Devices | Layer::Rails => Color::rgb(90, 90, 90),
        Layer::Margin => Color::rgb(150, 150, 150),
        _ => Color::BLACK,
    }
}

pub(crate) fn anchor(h: HAlign, v: VAlign) -> Anchor {
    match (v, h) {
        (VAlign::Top, HAlign::Left) => Anchor::TopLeft,
        (VAlign::Top, HAlign::Center) => Anchor::TopCenter,
        (VAlign::Top, HAlign::Right) => Anchor::TopRight,
        (VAlign::Middle, HAlign::Left) => Anchor::CenterLeft,
        (VAlign::Middle, HAlign::Center) => Anchor::Center,
        (VAlign::Middle, HAlign::Right) => Anchor::CenterRight,
        (VAlign::Bottom, HAlign::Left) => Anchor::BottomLeft,
        (VAlign::Bottom, HAlign::Center) => Anchor::BottomCenter,
        (VAlign::Bottom, HAlign::Right) => Anchor::BottomRight,
    }
}

/// Draws marks onto a page. `map` takes mark coordinates to page
/// millimetres and `size` converts mark lengths (radii, text heights) to
/// paper.
pub(crate) fn render(
    w: &mut PageWriter<'_>,
    marks: &Marks,
    map: &dyn Fn((f64, f64)) -> Vec2,
    size: f64,
) {
    for (layer, shape) in &marks.shapes {
        let stroke = Stroke::world(stroke_width(*layer), layer_color(*layer));
        let shape = match *shape {
            MarkShape::Line { a, b } => Shape::line(map(a), map(b)),
            MarkShape::Rect { min, max } => Shape::rect(map(min), map(max)),
            MarkShape::Circle { centre, radius } => {
                Shape::circle(map(centre), (radius * size) as f32)
            }
        };
        w.primitive(&Primitive::stroked(shape, stroke));
    }
    for t in &marks.texts {
        let mut label = Label::new(map(t.pos), t.text.clone(), (t.height * size) as f32);
        label.anchor = anchor(t.h, t.v);
        label.rotation = t.rotation_deg.to_radians() as f32;
        label.color = layer_color(t.layer);
        label.bold = t.bold;
        w.label(&label);
    }
}

fn frame_marks(doc: &PlateDocument, number: usize, count: usize, project: &str) -> Marks {
    let (l, t) = (BORDER_MM, BORDER_MM);
    let (r, b) = (PAGE_MM.0 - BORDER_MM, PAGE_MM.1 - BORDER_MM);
    let top = b - TITLE_BLOCK_MM;
    let mut m = Marks::default();
    m.shapes.push((
        Layer::Plate,
        MarkShape::Rect {
            min: (l, t),
            max: (r, b),
        },
    ));
    m.shapes.push((
        Layer::Table,
        MarkShape::Line {
            a: (l, top),
            b: (r, top),
        },
    ));
    let cell = |m: &mut Marks, x: f64, text: String, bold: bool| {
        m.texts.push(novo_layout::plate_drawing::Text {
            layer: Layer::Text,
            pos: (x, top + TITLE_BLOCK_MM / 2.0),
            height: 3.0,
            rotation_deg: 0.0,
            h: HAlign::Left,
            v: VAlign::Middle,
            text,
            bold,
        });
    };
    let d = &doc.drawing;
    cell(
        &mut m,
        l + 3.0,
        format!("{} - {}", doc.title, d.surface.name()),
        true,
    );
    cell(&mut m, l + 150.0, project.to_string(), false);
    cell(
        &mut m,
        l + 260.0,
        format!("Scale {}", scale_text(d.scale)),
        false,
    );
    cell(
        &mut m,
        l + 300.0,
        format!("Sheet {number} of {count}"),
        false,
    );
    cell(&mut m, l + 345.0, "KoperCAD".to_string(), false);
    m
}

/// The plate drawings as one PDF: for each document its drawing sheet and
/// any table continuation sheets. Deterministic: the same documents give
/// the same bytes.
pub fn export_plate_pdf(docs: &[PlateDocument], project_name: &str) -> Vec<u8> {
    let sheets: Vec<Sheet<'_>> = docs.iter().flat_map(sheets).collect();

    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog_id = alloc();
    let tree_id = alloc();
    let font_id = alloc();
    let info_id = alloc();
    let ids: Vec<(Ref, Ref)> = sheets.iter().map(|_| (alloc(), alloc())).collect();

    let mut pdf = Pdf::new();
    pdf.catalog(catalog_id).pages(tree_id);
    pdf.pages(tree_id)
        .kids(ids.iter().map(|i| i.0))
        .count(ids.len() as i32);
    pdf.type1_font(font_id)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    let mut info = pdf.document_info(info_id);
    info.title(TextStr(&format!("{project_name} - plate drawing")));
    info.producer(TextStr("KoperCAD"));
    info.finish();

    let mut inter = inter::Inter::new();
    let mut inter_id: Option<Ref> = None;
    let mut bold_id: Option<Ref> = None;
    let (pw, ph) = (PAGE_MM.0 as f32, PAGE_MM.1 as f32);
    for (i, sheet) in sheets.iter().enumerate() {
        let (page_id, content_id) = ids[i];
        let mut w = PageWriter::new(ph, &mut inter);
        let identity = |p: (f64, f64)| Vec2::new(p.0 as f32, p.1 as f32);
        render(
            &mut w,
            &frame_marks(sheet.doc, i + 1, sheets.len(), project_name),
            &identity,
            1.0,
        );
        if sheet.drawing {
            let d = &sheet.doc.drawing;
            if let Some(b) = d.marks.bounds() {
                // Centre the drawing, annotation included, in its area.
                let s = 1.0 / d.scale;
                let ox = BORDER_MM + (DRAWING_AREA_MM.0 - b.w * s) / 2.0 - b.x * s;
                let oy = BORDER_MM + (DRAWING_AREA_MM.1 - b.h * s) / 2.0 - b.y * s;
                let map =
                    move |p: (f64, f64)| Vec2::new((ox + p.0 * s) as f32, (oy + p.1 * s) as f32);
                render(&mut w, &d.marks, &map, s);
            }
        }
        render(&mut w, &sheet.tables, &identity, 1.0);

        let mut page = pdf.page(page_id);
        page.parent(tree_id)
            .media_box(Rect::new(0.0, 0.0, pw * PT_PER_MM, ph * PT_PER_MM))
            .contents(content_id);
        let mut resources = page.resources();
        let mut fonts = resources.fonts();
        fonts.pair(Name(font::FONT_NAME), font_id);
        if w.uses_inter() {
            let id = *inter_id.get_or_insert_with(&mut alloc);
            fonts.pair(Name(inter::FONT_NAME), id);
        }
        if w.uses_bold() {
            let id = *bold_id.get_or_insert_with(&mut alloc);
            fonts.pair(Name(font::BOLD_NAME), id);
        }
        fonts.finish();
        resources.finish();
        page.finish();
        pdf.stream(content_id, &w.finish());
    }
    if let Some(id) = inter_id {
        inter.write(&mut pdf, id, &mut alloc);
    }
    if let Some(id) = bold_id {
        pdf.type1_font(id)
            .base_font(Name(b"Helvetica-Bold"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
    }
    pdf.finish()
}

/// Exports and writes the plate PDF to `path`.
pub fn write_plate_pdf(
    docs: &[PlateDocument],
    project_name: &str,
    path: impl AsRef<Path>,
) -> Result<(), PdfError> {
    std::fs::write(path, export_plate_pdf(docs, project_name))?;
    Ok(())
}
