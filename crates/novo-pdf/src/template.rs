//! The 1:1 drilling template as a PDF: per surface one overview sheet
//! (map of the tiles, how to print and lay them, the scale-check ruler, the
//! hole list) and then the tiles, each at true size on A4 or A3 with a
//! scale-check ruler at its foot. The marks come from
//! `novo_layout::template`.
//!
//! True size depends on the print dialog: every sheet says so and carries a
//! 100 mm ruler to check it. Output is deterministic.

use novo_canvas::Vec2;
use novo_layout::drill::mm;
use novo_layout::plate_drawing::{HAlign, Layer, Marks, Shape as MarkShape, Text, VAlign};
use novo_layout::template::{
    FOOTER_MM, OVERLAP_MM, PAGE_MARGIN_MM, Paper, RULER_MM, TemplateSheet, TilePlan, plan_tiles,
};
use pdf_writer::{Finish, Name, Pdf, Rect, Ref, TextStr};

use crate::draw::{PT_PER_MM, PageWriter};
use crate::plate::render;
use crate::{font, inter};

/// Sheets of one surface: its tile plan and the sheets in print order
/// (overview first, then tiles row by row).
pub fn tile_plan(sheet: &TemplateSheet, paper: Paper) -> TilePlan {
    plan_tiles(sheet.width, sheet.height, paper)
}

/// Number of PDF pages one sheet takes: the overview and its tiles.
pub fn page_count(sheet: &TemplateSheet, paper: Paper) -> usize {
    tile_plan(sheet, paper).count().saturating_add(1)
}

type Draw<'a> = Box<dyn Fn(&mut PageWriter<'_>) + 'a>;

struct Page<'a> {
    size: (f64, f64),
    draw: Draw<'a>,
}

fn text(m: &mut Marks, pos: (f64, f64), height: f64, h: HAlign, bold: bool, s: impl Into<String>) {
    m.texts.push(Text {
        layer: Layer::Text,
        pos,
        height,
        rotation_deg: 0.0,
        h,
        v: VAlign::Top,
        text: s.into(),
        bold,
    });
}

fn line(m: &mut Marks, layer: Layer, a: (f64, f64), b: (f64, f64)) {
    m.shapes.push((layer, MarkShape::Line { a, b }));
}

/// The scale-check ruler: `RULER_MM` long with a tick every 10 mm, its left
/// end at `x`, its line at `y`; the caption sits below.
fn ruler(m: &mut Marks, x: f64, y: f64) {
    line(m, Layer::Table, (x, y), (x + RULER_MM, y));
    for i in 0..=10 {
        let tx = x + i as f64 * 10.0;
        let up = if i % 5 == 0 { 3.0 } else { 1.8 };
        line(m, Layer::Table, (tx, y - up), (tx, y));
    }
    for i in [0, 5, 10] {
        m.texts.push(Text {
            layer: Layer::Text,
            pos: (x + i as f64 * 10.0, y + 0.8),
            height: 2.2,
            rotation_deg: 0.0,
            h: HAlign::Center,
            v: VAlign::Top,
            text: format!("{}", i * 10),
            bold: false,
        });
    }
    text(
        m,
        (x + RULER_MM + 2.0, y - 2.2),
        2.5,
        HAlign::Left,
        false,
        "Must measure 100 mm",
    );
}

fn tile_page<'a>(
    sheet: &'a TemplateSheet,
    plan: &'a TilePlan,
    (col, row): (usize, usize),
    number: usize,
    total: usize,
) -> Page<'a> {
    let (pw, ph) = plan.page_mm();
    let (cw, ch) = plan.content_mm();
    let origin = plan.origin(col, row);
    let name = TilePlan::name(col, row);
    let draw = move |w: &mut PageWriter<'_>| {
        let (ox, oy) = origin;
        let map = move |p: (f64, f64)| {
            Vec2::new(
                (PAGE_MARGIN_MM + p.0 - ox) as f32,
                (PAGE_MARGIN_MM + p.1 - oy) as f32,
            )
        };
        // Everything of the plate stays inside the content frame.
        w.begin_clip(
            PAGE_MARGIN_MM as f32,
            PAGE_MARGIN_MM as f32,
            cw as f32,
            ch as f32,
        );
        render(w, &sheet.marks, &map, 1.0);
        let mut reg = Marks::default();
        for &x in &plan.x_lines() {
            for &y in &plan.y_lines() {
                if x < ox || x > ox + cw || y < oy || y > oy + ch {
                    continue;
                }
                reg.shapes.push((
                    Layer::Margin,
                    MarkShape::Circle {
                        centre: (x, y),
                        radius: 1.5,
                    },
                ));
                line(&mut reg, Layer::Margin, (x - 5.0, y), (x + 5.0, y));
                line(&mut reg, Layer::Margin, (x, y - 5.0), (x, y + 5.0));
                // Keep the label on the sheet: left of the cross near the right
                // edge, above it near the bottom.
                let (lx, ha) = if x > ox + cw - 30.0 {
                    (x - 2.0, HAlign::Right)
                } else {
                    (x + 2.0, HAlign::Left)
                };
                let ly = if y > oy + ch - 8.0 { y - 4.0 } else { y + 1.5 };
                text(
                    &mut reg,
                    (lx, ly),
                    2.2,
                    ha,
                    false,
                    format!("{} / {}", mm(x), mm(y)),
                );
            }
        }
        render(w, &reg, &map, 1.0);
        w.end_clip();

        // Frame, coordinate ticks along the edges, footer.
        let mut m = Marks::default();
        m.shapes.push((
            Layer::Table,
            MarkShape::Rect {
                min: (PAGE_MARGIN_MM, PAGE_MARGIN_MM),
                max: (PAGE_MARGIN_MM + cw, PAGE_MARGIN_MM + ch),
            },
        ));
        let first = |o: f64| (o / 10.0).ceil() as i64;
        for k in first(ox)..=((ox + cw) / 10.0).floor() as i64 {
            let v = k as f64 * 10.0;
            let x = PAGE_MARGIN_MM + v - ox;
            let major = k % 5 == 0;
            let len = if major { 3.0 } else { 1.5 };
            line(
                &mut m,
                Layer::Table,
                (x, PAGE_MARGIN_MM),
                (x, PAGE_MARGIN_MM + len),
            );
            if major {
                text(
                    &mut m,
                    (x + 0.6, PAGE_MARGIN_MM + 3.2),
                    2.0,
                    HAlign::Left,
                    false,
                    mm(v),
                );
            }
        }
        for k in first(oy)..=((oy + ch) / 10.0).floor() as i64 {
            let v = k as f64 * 10.0;
            let y = PAGE_MARGIN_MM + v - oy;
            let major = k % 5 == 0;
            let len = if major { 3.0 } else { 1.5 };
            line(
                &mut m,
                Layer::Table,
                (PAGE_MARGIN_MM, y),
                (PAGE_MARGIN_MM + len, y),
            );
            if major {
                text(
                    &mut m,
                    (PAGE_MARGIN_MM + 0.8, y + 0.4),
                    2.0,
                    HAlign::Left,
                    false,
                    mm(v),
                );
            }
        }
        let foot = ph - PAGE_MARGIN_MM - FOOTER_MM;
        text(
            &mut m,
            (PAGE_MARGIN_MM, foot + 1.5),
            3.5,
            HAlign::Left,
            true,
            format!("Tile {name}"),
        );
        text(
            &mut m,
            (PAGE_MARGIN_MM + 22.0, foot + 2.0),
            2.8,
            HAlign::Left,
            false,
            format!(
                "{} - {}   plate x {} to {}, y {} to {} mm   1:1, print at 100%",
                sheet.title,
                sheet.surface.name(),
                mm(ox),
                mm((ox + cw).min(sheet.width)),
                mm(oy),
                mm((oy + ch).min(sheet.height)),
            ),
        );
        text(
            &mut m,
            (pw - PAGE_MARGIN_MM, foot + 2.0),
            2.8,
            HAlign::Right,
            false,
            format!("{number}/{total}"),
        );
        ruler(&mut m, PAGE_MARGIN_MM + 3.0, foot + 11.0);
        render(w, &m, &|p| Vec2::new(p.0 as f32, p.1 as f32), 1.0);
    };
    Page {
        size: (pw, ph),
        draw: Box::new(draw),
    }
}

/// Appends `lines` from `y` down, one row each; returns the next free y.
fn paragraph(m: &mut Marks, x: f64, mut y: f64, width: f64, height: f64, s: &str) -> f64 {
    let per_line = (width / (height * 0.5)).floor().max(10.0) as usize;
    let mut cur = String::new();
    let flush = |m: &mut Marks, cur: &mut String, y: &mut f64| {
        if !cur.is_empty() {
            text(m, (x, *y), height, HAlign::Left, false, cur.clone());
            *y += height * 1.45;
            cur.clear();
        }
    };
    for word in s.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > per_line {
            flush(m, &mut cur, &mut y);
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    flush(m, &mut cur, &mut y);
    y
}

fn overview_page<'a>(sheet: &'a TemplateSheet, plan: &'a TilePlan, project: &'a str) -> Page<'a> {
    let (pw, ph) = plan.page_mm();
    let draw = move |w: &mut PageWriter<'_>| {
        let mut m = Marks::default();
        let x0 = PAGE_MARGIN_MM;
        let width = pw - 2.0 * PAGE_MARGIN_MM;
        let mut y = PAGE_MARGIN_MM;
        text(
            &mut m,
            (x0, y),
            6.0,
            HAlign::Left,
            true,
            format!(
                "Drilling template 1:1 - {} - {}",
                sheet.title,
                sheet.surface.name()
            ),
        );
        y += 9.0;
        let sheets = if plan.count() == 1 {
            "1 sheet".to_owned()
        } else {
            format!("{} sheets ({} x {})", plan.count(), plan.cols, plan.rows)
        };
        text(
            &mut m,
            (x0, y),
            3.2,
            HAlign::Left,
            false,
            format!(
                "{project}   {} x {} mm   {sheets} {} {}",
                mm(sheet.width),
                mm(sheet.height),
                plan.paper.name(),
                if plan.landscape {
                    "landscape"
                } else {
                    "portrait"
                },
            ),
        );
        y += 7.0;

        // Map of the tiles, not to scale of the paper (the sheet is smaller).
        let map_h = (ph * 0.34).min(130.0);
        let k = ((width - 10.0) / sheet.width).min(map_h / sheet.height);
        let (mx, my) = (x0 + (width - sheet.width * k) / 2.0, y + 1.0);
        let at = |p: (f64, f64)| (mx + p.0 * k, my + p.1 * k);
        m.shapes.push((
            Layer::Plate,
            MarkShape::Rect {
                min: at((0.0, 0.0)),
                max: at((sheet.width, sheet.height)),
            },
        ));
        let (cw, ch) = plan.content_mm();
        for r in 0..plan.rows {
            for c in 0..plan.cols {
                let (ox, oy) = plan.origin(c, r);
                let (x1, y1) = ((ox + cw).min(sheet.width), (oy + ch).min(sheet.height));
                m.shapes.push((
                    Layer::Margin,
                    MarkShape::Rect {
                        min: at((ox, oy)),
                        max: at((x1, y1)),
                    },
                ));
                let centre = at(((ox + x1) / 2.0, (oy + y1) / 2.0));
                text(
                    &mut m,
                    (centre.0, centre.1 - 3.0),
                    6.0,
                    HAlign::Center,
                    true,
                    TilePlan::name(c, r),
                );
            }
        }
        for h in &sheet.holes {
            let p = at((h.x, h.y));
            m.shapes.push((
                Layer::Holes,
                MarkShape::Circle {
                    centre: p,
                    radius: (h.dia / 2.0 * k).max(0.4),
                },
            ));
        }
        for c in &sheet.cutouts {
            let (hw, hh) = match c.shape {
                novo_layout::drill::CutoutShape::Rect { w, h } => (w / 2.0, h / 2.0),
                novo_layout::drill::CutoutShape::Round { dia } => (dia / 2.0, dia / 2.0),
            };
            m.shapes.push((
                Layer::Cutouts,
                MarkShape::Rect {
                    min: at((c.x - hw, c.y - hh)),
                    max: at((c.x + hw, c.y + hh)),
                },
            ));
        }
        y = my + sheet.height * k + 6.0;
        text(
            &mut m,
            (x0, y),
            2.5,
            HAlign::Left,
            false,
            "Map of the plate (reduced). The template itself is 1:1 on the following sheets.",
        );
        y += 8.0;

        text(&mut m, (x0, y), 4.0, HAlign::Left, true, "Before you cut");
        y += 6.0;
        let mut steps: Vec<String> = vec![
            "Print at 100% (Actual size). Turn off Fit to page and Shrink to printable area.".into(),
            "Measure the ruler on any sheet, it must be 100 mm. If it is not, fix the print scale first.".into(),
        ];
        if plan.count() > 1 {
            steps.push(format!(
                "Lay the sheets as in the map. Neighbours overlap by {} mm: put the next sheet over the previous one, grey crosses on grey crosses, then tape.",
                mm(OVERLAP_MM)
            ));
        }
        steps.push(
            if sheet.surface == novo_layout::drill::Surface::Plate {
                "Tape the template with its top left corner on the plate corner (x 0, y 0), seen from the side the devices mount on."
            } else {
                "This is not the mounting plate: check the hinge side and the face you drill from against the drill table before you start."
            }
            .into(),
        );
        steps.push(
            "Centre-punch through every cross hair, then drill. Numbers match the lists below."
                .into(),
        );
        for (i, s) in steps.iter().enumerate() {
            text(
                &mut m,
                (x0, y),
                3.0,
                HAlign::Left,
                false,
                format!("{}.", i + 1),
            );
            y = paragraph(&mut m, x0 + 6.0, y, width - 6.0, 3.0, s) + 0.8;
        }
        y += 1.0;
        y = paragraph(
            &mut m,
            x0,
            y,
            width,
            2.7,
            "How exact: positions are exact in the file. On paper they are as good as the print scale and the taping; about 0.5 mm is typical when the ruler measures 100 mm (an estimate, not measured here). Do not rely on a paper template for fits tighter than that.",
        ) + 3.0;
        ruler(&mut m, x0 + 3.0, y + 4.0);
        y += 12.0;

        // Lists.
        let limit = ph - PAGE_MARGIN_MM - 6.0;
        let mut sizes: Vec<(f64, usize)> = Vec::new();
        for h in &sheet.holes {
            match sizes.iter_mut().find(|(d, _)| *d == h.dia) {
                Some(e) => e.1 += 1,
                None => sizes.push((h.dia, 1)),
            }
        }
        sizes.sort_by(|a, b| a.0.total_cmp(&b.0));
        if !sizes.is_empty() {
            text(
                &mut m,
                (x0, y),
                4.0,
                HAlign::Left,
                true,
                "Holes by drill size",
            );
            y += 6.0;
            for (d, n) in sizes {
                if y > limit {
                    break;
                }
                text(
                    &mut m,
                    (x0, y),
                    3.0,
                    HAlign::Left,
                    false,
                    format!("\u{d8}{} mm    {n} \u{d7}", mm(d)),
                );
                y += 4.5;
            }
            y += 2.0;
        }
        if !sheet.cutouts.is_empty() && y < limit {
            text(&mut m, (x0, y), 4.0, HAlign::Left, true, "Cut-outs");
            y += 6.0;
            for c in &sheet.cutouts {
                if y > limit {
                    text(
                        &mut m,
                        (x0, y),
                        3.0,
                        HAlign::Left,
                        false,
                        "... more in the plate drawing's drill table",
                    );
                    break;
                }
                text(
                    &mut m,
                    (x0, y),
                    3.0,
                    HAlign::Left,
                    false,
                    format!(
                        "C{}   {}   centre x {}, y {}   ({})",
                        c.no,
                        c.shape.describe(),
                        mm(c.x),
                        mm(c.y),
                        c.source
                    ),
                );
                y += 4.5;
            }
            y += 2.0;
        }
        let mut notes: Vec<String> = Vec::new();
        if !sheet.without_mounting_data.is_empty() {
            notes.push(format!(
                "No hole data in the part for: {}. Only their outline is drawn; mark and measure those by hand.",
                sheet.without_mounting_data.join(", ")
            ));
        }
        if !sheet.clip_on.is_empty() {
            notes.push(format!(
                "Clip-on devices on a rail, no holes to drill: {}. Their outline shows where they sit.",
                sheet.clip_on.join(", ")
            ));
        }
        for p in &sheet.problems {
            notes.push(format!("Not read: {p}"));
        }
        for n in notes {
            if y > limit {
                break;
            }
            y = paragraph(&mut m, x0, y, width, 2.8, &n) + 1.5;
        }
        m.shapes.push((
            Layer::Table,
            MarkShape::Rect {
                min: (x0 - 3.0, PAGE_MARGIN_MM - 3.0),
                max: (pw - PAGE_MARGIN_MM + 3.0, ph - PAGE_MARGIN_MM + 3.0),
            },
        ));
        render(w, &m, &|p| Vec2::new(p.0 as f32, p.1 as f32), 1.0);
    };
    Page {
        size: (pw, ph),
        draw: Box::new(draw),
    }
}

/// The template as one PDF: for each sheet its overview page, then the
/// tiles row by row. Deterministic.
pub fn export_template_pdf(sheets: &[TemplateSheet], project_name: &str, paper: Paper) -> Vec<u8> {
    let plans: Vec<TilePlan> = sheets.iter().map(|s| tile_plan(s, paper)).collect();
    let mut pages: Vec<Page<'_>> = Vec::new();
    for (sheet, plan) in sheets.iter().zip(&plans) {
        pages.push(overview_page(sheet, plan, project_name));
        let total = plan.count();
        let mut number = 0;
        for row in 0..plan.rows {
            for col in 0..plan.cols {
                number += 1;
                pages.push(tile_page(sheet, plan, (col, row), number, total));
            }
        }
    }
    write_pages(&pages, &format!("{project_name} - drilling template 1:1"))
}

/// Largest page side a PDF reader takes: 14 400 pt, about 5 080 mm.
const MAX_PAGE_MM: f64 = 5080.0;

/// The plate at true size on one page whose size is the plate's: for a print
/// shop or a plotter, where one sheet beats taping tiles. `None` when the
/// plate is smaller than the 100 mm check ruler or larger than a PDF page
/// can be; the tiled template covers those.
pub fn export_plate_pdf(sheet: &TemplateSheet, project_name: &str) -> Option<Vec<u8>> {
    let (w, h) = (sheet.width, sheet.height);
    if !(w.is_finite() && h.is_finite()) || w < 150.0 || h < 40.0 || w.max(h) > MAX_PAGE_MM {
        return None;
    }
    let draw = move |pw: &mut PageWriter<'_>| {
        let map = |p: (f64, f64)| Vec2::new(p.0 as f32, p.1 as f32);
        render(pw, &sheet.marks, &map, 1.0);
        let mut m = Marks::default();
        text(
            &mut m,
            (5.0, h - 21.0),
            3.5,
            HAlign::Left,
            true,
            format!("{} - {}", sheet.title, sheet.surface.name()),
        );
        text(
            &mut m,
            (5.0, h - 16.0),
            2.8,
            HAlign::Left,
            false,
            format!(
                "{} x {} mm, 1:1. Print at 100%, not fit to page. Hole list and notes: the tiled template.",
                mm(w),
                mm(h)
            ),
        );
        ruler(&mut m, 5.0, h - 6.0);
        render(pw, &m, &|p| Vec2::new(p.0 as f32, p.1 as f32), 1.0);
    };
    let page = Page {
        size: (w, h),
        draw: Box::new(draw),
    };
    Some(write_pages(
        &[page],
        &format!("{project_name} - plate {} 1:1, one sheet", sheet.title),
    ))
}

fn write_pages(pages: &[Page<'_>], title: &str) -> Vec<u8> {
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
    let ids: Vec<(Ref, Ref)> = pages.iter().map(|_| (alloc(), alloc())).collect();

    let mut pdf = Pdf::new();
    pdf.catalog(catalog_id).pages(tree_id);
    pdf.pages(tree_id)
        .kids(ids.iter().map(|i| i.0))
        .count(ids.len() as i32);
    pdf.type1_font(font_id)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    let mut info = pdf.document_info(info_id);
    info.title(TextStr(title));
    info.producer(TextStr("KoperCAD"));
    info.finish();

    let mut inter = inter::Inter::new();
    let mut inter_id: Option<Ref> = None;
    let mut bold_id: Option<Ref> = None;
    for (i, page_spec) in pages.iter().enumerate() {
        let (page_id, content_id) = ids[i];
        let (pw, ph) = (page_spec.size.0 as f32, page_spec.size.1 as f32);
        let mut w = PageWriter::new(ph, &mut inter);
        (page_spec.draw)(&mut w);

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
