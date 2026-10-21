//! The hand workshop's two extra 1:1 prints next to the tiled template:
//!
//! * the print-scale check page (`export_scale_check_pdf`): a 100 mm square
//!   with diagonals, a 250 mm ruler and a table from what the square
//!   measures to the print scale that repairs it, and
//! * the cut-out cards (`export_cutout_cards_pdf`): per cut-out shape one
//!   A4 page with the outline at true size, centre lines to tape against,
//!   punch marks on the outline and the distances from the surface edges.
//!
//! Marks come from `novo_layout::cutout_cards`. Output is deterministic.
//! True size depends on the print dialog; every page says so and carries a
//! check square or ruler.

use novo_canvas::Vec2;
use novo_layout::cutout_cards::{
    CARD_ARM_MM, CHECK_SQUARE_MM, CutoutCard, SCALE_TOLERANCE_MM, correction_percent, cutout_cards,
};
use novo_layout::drill::{CutoutShape, mm};
use novo_layout::plate_drawing::{HAlign, Layer, Marks, Shape as MarkShape, Text, VAlign};
use novo_layout::template::{PAGE_MARGIN_MM, TemplateSheet};

use crate::draw::PageWriter;
use crate::plate::render;
use crate::template::{Page, line, paragraph, ruler, text, write_pages};

const A4: (f64, f64) = (210.0, 297.0);
/// Length of the long ruler on the scale-check page.
const LONG_RULER_MM: f64 = 250.0;
/// Side of the small check square at the foot of a card.
const CARD_SQUARE_MM: f64 = 50.0;
/// Top of the card footer (check square and ruler).
const FOOTER_TOP_MM: f64 = A4.1 - PAGE_MARGIN_MM - CARD_SQUARE_MM - 4.0;
const ROW_MM: f64 = 5.0;

fn identity(p: (f64, f64)) -> Vec2 {
    Vec2::new(p.0 as f32, p.1 as f32)
}

fn small_text(m: &mut Marks, pos: (f64, f64), h: HAlign, v: VAlign, s: String) {
    m.texts.push(Text {
        layer: Layer::Text,
        pos,
        height: 2.4,
        rotation_deg: 0.0,
        h,
        v,
        text: s,
        bold: false,
    });
}

/// A square of `side` with both diagonals, ticks every 10 mm on its top and
/// left edges and corner crosses just outside.
fn check_square(m: &mut Marks, x: f64, y: f64, side: f64) {
    m.shapes.push((
        Layer::Cutouts,
        MarkShape::Rect {
            min: (x, y),
            max: (x + side, y + side),
        },
    ));
    line(m, Layer::Table, (x, y), (x + side, y + side));
    line(m, Layer::Table, (x + side, y), (x, y + side));
    let mut t = 10.0;
    while t < side - 1e-9 {
        let long = (t / 10.0) as i64 % 5 == 0;
        let len = if long { 3.0 } else { 1.8 };
        line(m, Layer::Table, (x + t, y), (x + t, y + len));
        line(m, Layer::Table, (x, y + t), (x + len, y + t));
        t += 10.0;
    }
    for (cx, cy) in [(x, y), (x + side, y), (x, y + side), (x + side, y + side)] {
        line(m, Layer::Margin, (cx - 4.0, cy), (cx + 4.0, cy));
        line(m, Layer::Margin, (cx, cy - 4.0), (cx, cy + 4.0));
    }
}

/// A vertical ruler of `len` mm, tick every 10 mm, top at (`x`, `y`).
fn long_ruler(m: &mut Marks, x: f64, y: f64, len: f64) {
    line(m, Layer::Table, (x, y), (x, y + len));
    let n = (len / 10.0).round() as i64;
    for i in 0..=n {
        let ty = y + i as f64 * 10.0;
        let l = if i % 5 == 0 { 3.0 } else { 1.8 };
        line(m, Layer::Table, (x - l, ty), (x, ty));
        if i % 5 == 0 {
            small_text(
                m,
                (x + 1.2, ty),
                HAlign::Left,
                VAlign::Middle,
                format!("{}", i * 10),
            );
        }
    }
}

fn scale_check_page() -> Page<'static> {
    let draw = |w: &mut PageWriter<'_>| {
        let mut m = Marks::default();
        let x0 = PAGE_MARGIN_MM;
        let mut y = PAGE_MARGIN_MM;
        text(
            &mut m,
            (x0, y),
            6.0,
            HAlign::Left,
            true,
            "Print scale check - 1:1",
        );
        y += 9.0;
        y = paragraph(
            &mut m,
            x0,
            y,
            A4.0 - 2.0 * PAGE_MARGIN_MM - 40.0,
            3.0,
            "Print this page first, with the same program, printer and settings you will use for the templates: 100% or Actual size, Fit to page and Shrink to printable area off. Measure the square with a steel rule. A template on paper is only as true as this page.",
        ) + 2.0;
        let sq_y = y + 6.0;
        check_square(&mut m, x0 + 6.0, sq_y, CHECK_SQUARE_MM);
        text(
            &mut m,
            (x0 + 6.0, sq_y - 7.5),
            2.8,
            HAlign::Left,
            false,
            "Square: both sides must measure 100 mm",
        );
        // The long ruler checks the paper feed over the length of a door.
        let rx = A4.0 - PAGE_MARGIN_MM - 14.0;
        long_ruler(&mut m, rx, PAGE_MARGIN_MM + 10.0, LONG_RULER_MM);
        small_text(
            &mut m,
            (rx, PAGE_MARGIN_MM + 5.0),
            HAlign::Center,
            VAlign::Top,
            "250 mm".into(),
        );
        // Write-in boxes.
        let by = sq_y + CHECK_SQUARE_MM + 10.0;
        text(&mut m, (x0, by), 3.2, HAlign::Left, true, "Measured");
        text(
            &mut m,
            (x0, by + 7.0),
            3.0,
            HAlign::Left,
            false,
            "Printed at ..........  %      Width ..........  mm      Height ..........  mm      250 mm ruler ..........  mm",
        );
        // Scale table.
        let ty = by + 18.0;
        text(
            &mut m,
            (x0, ty),
            3.2,
            HAlign::Left,
            true,
            "If the square measures ... after a print at 100 %, print again at ...",
        );
        let mut row = ty + 7.0;
        let mut col_x = x0;
        for (i, tenth) in (-40..=40).step_by(5).enumerate() {
            let measured = CHECK_SQUARE_MM + f64::from(tenth) / 10.0;
            if (measured - CHECK_SQUARE_MM).abs() < 1e-9 {
                continue;
            }
            if i % 9 == 0 && i > 0 {
                col_x += 62.0;
                row = ty + 7.0;
            }
            text(
                &mut m,
                (col_x, row),
                2.8,
                HAlign::Left,
                false,
                format!(
                    "{:6.1} mm   ->   {:6.2} %",
                    measured,
                    correction_percent(measured)
                ),
            );
            row += 4.6;
        }
        let ny = row.max(ty + 7.0 + 9.0 * 4.6) + 6.0;
        let ny = paragraph(
            &mut m,
            x0,
            ny,
            A4.0 - 2.0 * PAGE_MARGIN_MM - 24.0,
            2.8,
            &format!(
                "Good enough: both sides within {} mm of 100 mm (0.2 %: about 1.2 mm over a 600 mm door). Set the percentage in the print dialog as a custom scale, print again and measure again. The table is for a first print at 100 %; after a reprint use new scale = scale printed at x 100 / measured (the app does this for you in Bench output). The 250 mm ruler shows the paper feed: if it is off by more than 0.5 mm, lengths along the page are off by as much. If a side is outside that and width and height differ by more than 0.3 mm the printer stretches one way: print from another program or printer, a percentage cannot repair that.",
                mm(SCALE_TOLERANCE_MM)
            ),
        ) + 2.0;
        paragraph(
            &mut m,
            x0,
            ny,
            A4.0 - 2.0 * PAGE_MARGIN_MM - 24.0,
            2.8,
            "How exact: the sizes in the file are exact; on paper they are as good as this check and the taping, about 0.5 mm when the square measures 100 mm (an estimate, not measured here). Do not trust a paper template for a fit tighter than that.",
        );
        ruler(&mut m, x0 + 6.0, A4.1 - PAGE_MARGIN_MM - 6.0);
        render(w, &m, &identity, 1.0);
    };
    Page {
        size: A4,
        draw: Box::new(draw),
    }
}

/// The scale-check page as a one-page PDF (A4).
pub fn export_scale_check_pdf(project_name: &str) -> Vec<u8> {
    write_pages(
        &[scale_check_page()],
        &format!("{project_name} - print scale check"),
    )
}

/// Centre lines, outline and punch marks of `card` with its centre at
/// (`cx`, `cy`) on the page.
fn card_art(m: &mut Marks, shape: CutoutShape, cx: f64, cy: f64) {
    let (hw, hh) = match shape {
        CutoutShape::Rect { w, h } => (w / 2.0, h / 2.0),
        CutoutShape::Round { dia } => (dia / 2.0, dia / 2.0),
    };
    // Centre lines run through the outline to the arms' ends, so they can be
    // lined up with pencil marks on the door.
    line(
        m,
        Layer::Holes,
        (cx - hw - CARD_ARM_MM, cy),
        (cx + hw + CARD_ARM_MM, cy),
    );
    line(
        m,
        Layer::Holes,
        (cx, cy - hh - CARD_ARM_MM),
        (cx, cy + hh + CARD_ARM_MM),
    );
    let punch = |m: &mut Marks, p: (f64, f64)| {
        line(m, Layer::Holes, (p.0 - 2.0, p.1), (p.0 + 2.0, p.1));
        line(m, Layer::Holes, (p.0, p.1 - 2.0), (p.0, p.1 + 2.0));
    };
    match shape {
        CutoutShape::Rect { w, h } => {
            m.shapes.push((
                Layer::Cutouts,
                MarkShape::Rect {
                    min: (cx - w / 2.0, cy - h / 2.0),
                    max: (cx + w / 2.0, cy + h / 2.0),
                },
            ));
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                punch(m, (cx + sx * w / 2.0, cy + sy * h / 2.0));
            }
        }
        CutoutShape::Round { dia } => {
            m.shapes.push((
                Layer::Cutouts,
                MarkShape::Circle {
                    centre: (cx, cy),
                    radius: dia / 2.0,
                },
            ));
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                punch(m, (cx + dx * dia / 2.0, cy + dy * dia / 2.0));
            }
        }
    }
    // The centre itself: the cross hair to centre-punch.
    punch(m, (cx, cy));
}

/// Table rows that fit under the art on the first page.
fn first_page_rows(table_top: f64) -> usize {
    let room = FOOTER_TOP_MM - 4.0 - table_top - ROW_MM;
    if room < 3.0 * ROW_MM {
        0
    } else {
        (room / ROW_MM).floor() as usize
    }
}

fn table(m: &mut Marks, x0: f64, y: f64, card: &CutoutCard, rows: &[usize]) {
    let cols = [0.0, 16.0, 50.0, 84.0, 118.0, 152.0];
    let heads = ["No", "From left", "From right", "From top", "From bottom"];
    for (c, h) in heads.iter().enumerate() {
        text(m, (x0 + cols[c], y), 2.8, HAlign::Left, true, *h);
    }
    line(
        m,
        Layer::Table,
        (x0, y + 4.0),
        (A4.0 - PAGE_MARGIN_MM, y + 4.0),
    );
    text(m, (x0 + cols[5], y), 2.8, HAlign::Left, true, "Device");
    for (i, &p) in rows.iter().enumerate() {
        let pos = &card.positions[p];
        let ry = y + 6.0 + i as f64 * ROW_MM;
        let cells = [
            format!("C{}", pos.no),
            format!("{} mm", mm(pos.from_left)),
            format!("{} mm", mm(pos.from_right)),
            format!("{} mm", mm(pos.from_top)),
            format!("{} mm", mm(pos.from_bottom)),
            pos.source.clone(),
        ];
        for (c, s) in cells.into_iter().enumerate() {
            text(m, (x0 + cols[c], ry), 2.8, HAlign::Left, false, s);
        }
    }
}

fn card_pages(card: &CutoutCard, project: &str, pages: &mut Vec<Page<'static>>) {
    let card = card.clone();
    let project = project.to_owned();
    let art_h = match card.shape {
        CutoutShape::Rect { h, .. } => h,
        CutoutShape::Round { dia } => dia,
    } + 2.0 * CARD_ARM_MM;
    let cx = A4.0 / 2.0;
    let art_top = 46.0;
    let cy = art_top + art_h / 2.0;
    let table_top = art_top + art_h + 6.0;
    let first = first_page_rows(table_top).min(card.positions.len());
    let rest: Vec<usize> = (first..card.positions.len()).collect();
    let per_page = ((A4.1 - 2.0 * PAGE_MARGIN_MM - 24.0) / ROW_MM).floor() as usize;
    let continuation: Vec<Vec<usize>> = rest.chunks(per_page).map(<[usize]>::to_vec).collect();
    let total = 1 + continuation.len();

    let first_card = card.clone();
    let first_project = project.clone();
    let first_draw = move |w: &mut PageWriter<'_>| {
        let card = &first_card;
        let mut m = Marks::default();
        let x0 = PAGE_MARGIN_MM;
        let mut y = PAGE_MARGIN_MM;
        text(
            &mut m,
            (x0, y),
            5.5,
            HAlign::Left,
            true,
            format!(
                "Cut-out {} - {}",
                card.shape.describe(),
                card.surface.name()
            ),
        );
        y += 8.0;
        text(
            &mut m,
            (x0, y),
            3.0,
            HAlign::Left,
            false,
            format!(
                "{} - {} off - 1:1, page {} of {}",
                first_project,
                card.positions.len(),
                1,
                total
            ),
        );
        y += 6.0;
        paragraph(
            &mut m,
            x0,
            y,
            A4.0 - 2.0 * PAGE_MARGIN_MM,
            2.7,
            "Print at 100% and check the square (50 mm) and the ruler (100 mm) first. Mark the centre on the surface with the distances below, tape the card with its cross on the mark and its lines parallel to the edges, centre-punch the cross and the four outline marks, then drill and cut to the outline. The edge distances are the drill table's x and y from the top left of the surface; check which face the overview sheet shows.",
        );
        card_art(&mut m, card.shape, cx, cy);
        small_text(
            &mut m,
            (cx + 3.0, cy + art_h / 2.0 - 2.0),
            HAlign::Left,
            VAlign::Bottom,
            card.shape.describe(),
        );
        let rows: Vec<usize> = (0..first).collect();
        if first > 0 {
            table(&mut m, x0, table_top, card, &rows);
            small_text(
                &mut m,
                (A4.0 - PAGE_MARGIN_MM, table_top - 3.0),
                HAlign::Right,
                VAlign::Bottom,
                "The same centre, measured from each edge".into(),
            );
        }
        if first < card.positions.len() {
            small_text(
                &mut m,
                (
                    x0,
                    table_top
                        + if first > 0 {
                            6.0 + first as f64 * ROW_MM
                        } else {
                            0.0
                        },
                ),
                HAlign::Left,
                VAlign::Top,
                format!(
                    "{} more position{}: next page",
                    card.positions.len() - first,
                    if card.positions.len() - first == 1 {
                        ""
                    } else {
                        "s"
                    }
                ),
            );
        }
        check_square(&mut m, x0, FOOTER_TOP_MM, CARD_SQUARE_MM);
        small_text(
            &mut m,
            (x0 + CARD_SQUARE_MM + 6.0, FOOTER_TOP_MM + 2.0),
            HAlign::Left,
            VAlign::Top,
            "Print check: the square must measure 50 mm. See the scale-check page.".into(),
        );
        ruler(
            &mut m,
            x0 + CARD_SQUARE_MM + 8.0,
            FOOTER_TOP_MM + CARD_SQUARE_MM - 8.0,
        );
        render(w, &m, &identity, 1.0);
    };
    pages.push(Page {
        size: A4,
        draw: Box::new(first_draw),
    });
    for (n, rows) in continuation.into_iter().enumerate() {
        let card = card.clone();
        let project = project.clone();
        let draw = move |w: &mut PageWriter<'_>| {
            let mut m = Marks::default();
            let x0 = PAGE_MARGIN_MM;
            text(
                &mut m,
                (x0, PAGE_MARGIN_MM),
                5.0,
                HAlign::Left,
                true,
                format!(
                    "Cut-out {} - {} (positions)",
                    card.shape.describe(),
                    card.surface.name()
                ),
            );
            text(
                &mut m,
                (x0, PAGE_MARGIN_MM + 8.0),
                3.0,
                HAlign::Left,
                false,
                format!("{project} - page {} of {total}", n + 2),
            );
            table(&mut m, x0, PAGE_MARGIN_MM + 18.0, &card, &rows);
            render(w, &m, &identity, 1.0);
        };
        pages.push(Page {
            size: A4,
            draw: Box::new(draw),
        });
    }
}

/// What `export_cutout_cards_pdf` made.
pub struct CutoutCards {
    pub pdf: Vec<u8>,
    /// Pages in the PDF.
    pub pages: usize,
    /// Cut-outs on cards.
    pub cutouts: usize,
    /// Shapes too large for an A4 card (`describe()` text); the tiled
    /// template carries them.
    pub too_large: Vec<String>,
}

/// One card per cut-out shape of the surfaces other than the mounting
/// plate's own hole pattern, A4. `None` when no surface has a cut-out that
/// fits a card. Covers every surface, the plate included, since a plate
/// can carry cut-outs too.
pub fn export_cutout_cards_pdf(
    sheets: &[TemplateSheet],
    project_name: &str,
) -> Option<CutoutCards> {
    let mut pages: Vec<Page<'static>> = Vec::new();
    let mut cutouts = 0;
    let mut too_large = Vec::new();
    for sheet in sheets {
        for card in cutout_cards(sheet) {
            if !card.fits_card() {
                too_large.push(format!(
                    "{} on {}",
                    card.shape.describe(),
                    card.surface.name()
                ));
                continue;
            }
            cutouts += card.positions.len();
            card_pages(&card, project_name, &mut pages);
        }
    }
    if pages.is_empty() {
        return None;
    }
    let count = pages.len();
    Some(CutoutCards {
        pdf: write_pages(&pages, &format!("{project_name} - cut-out cards 1:1")),
        pages: count,
        cutouts,
        too_large,
    })
}
