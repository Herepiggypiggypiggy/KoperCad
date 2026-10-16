//! A report as a stand-alone PDF: ruled tables on plain pages, without the
//! schematic frame and title block. Used for documents that leave the
//! project, such as the commissioning report with its sign-off block.
//!
//! Pages come from [`novo_reports::paginate`] and [`novo_reports::layout_page`],
//! the same layout the generated pages use, so a report reads the same on
//! paper as in the app. Each page has a footer with the document title and
//! the page number of the whole document; every section gets a bookmark.

use std::collections::BTreeMap;

use novo_canvas::{Anchor, Color, Label, Primitive, Shape, Stroke, Vec2};
use novo_core::{PageFormat, PaperSize};
use novo_reports::{
    Align, CsvOptions, ExportError, PageLayout, PageSetup, ReportOutput, i18n::Translator,
    i18n::translate_layout, i18n::translate_output, layout_page, paginate, to_csv, xlsx_bytes,
};
use pdf_writer::types::PageMode;
use pdf_writer::{Finish, Name, Pdf, Rect, Ref, TextStr};

use crate::draw::{self, PT_PER_MM};
use crate::{font, inter};

/// How to write a report PDF.
#[derive(Clone, Debug, PartialEq)]
pub struct ReportPdfOptions {
    /// Document title: the PDF's title and the left side of every footer.
    pub title: String,
    /// Printed on the right of every footer (a date, a revision); may be
    /// empty. Nothing is read from the clock, so output is reproducible.
    pub footer_right: String,
    /// Paper. A4 landscape by default, which fits the wide check tables.
    pub format: PageFormat,
    /// Row height in mm for sections that need room to write in, by section
    /// key. The sign-off block gets tall rows so it can be signed by hand.
    pub row_mm: BTreeMap<String, f64>,
    /// The language of the report text: titles, headers, fixed section
    /// titles and the pager words. English prints the text as generated.
    pub language: Translator,
}

impl Default for ReportPdfOptions {
    fn default() -> Self {
        Self {
            title: String::new(),
            footer_right: String::new(),
            format: PageFormat {
                size: PaperSize::A4,
                landscape: true,
                ..PageFormat::default()
            },
            row_mm: BTreeMap::from([("signoff".to_owned(), 16.0)]),
            language: Translator::english(),
        }
    }
}

/// Height kept free under the table for the footer.
const FOOTER_MM: f64 = 12.0;

fn setup_for(options: &ReportPdfOptions, section_key: &str) -> PageSetup {
    let mut setup = PageSetup::for_format(options.format);
    setup.title_block_mm = FOOTER_MM;
    setup.margins.left = 15.0;
    setup.margins.bottom = 8.0;
    if let Some(row) = options.row_mm.get(section_key) {
        setup.row_mm = *row;
    }
    setup
}

/// Width of a line in mm at `size`.
fn width_mm(text: &str, size: f32, bold: bool) -> f32 {
    if font::is_winansi(text) {
        let bytes = font::encode(text);
        size * if bold {
            font::bold_text_width(&bytes)
        } else {
            font::text_width(&bytes)
        }
    } else {
        // Outside WinAnsi the line is set in Inter; estimate generously.
        size * 0.62 * text.chars().count() as f32
    }
}

/// `text` cut with an ellipsis to fit `width` mm.
fn fit(text: &str, width: f32, size: f32, bold: bool) -> String {
    let line = text.lines().next().unwrap_or_default();
    if width_mm(line, size, bold) <= width {
        return line.to_owned();
    }
    let mut chars: Vec<char> = line.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cut: String = chars.iter().collect::<String>() + "...";
        if width_mm(&cut, size, bold) <= width {
            return cut;
        }
    }
    String::new()
}

fn draw_layout(w: &mut draw::PageWriter, layout: &PageLayout) {
    for t in &layout.texts {
        if t.text == novo_reports::TICK_BOX {
            // The base fonts have no ballot box: draw a square to tick.
            let side = (t.height * 0.6).min(t.width * 0.8).min(4.0) as f32;
            let (cx, cy) = ((t.x + t.width / 2.0) as f32, (t.y + t.height / 2.0) as f32);
            w.primitive(&Primitive::stroked(
                Shape::Rect {
                    min: Vec2::new(cx - side / 2.0, cy - side / 2.0),
                    max: Vec2::new(cx + side / 2.0, cy + side / 2.0),
                },
                Stroke::world(0.25, Color::BLACK),
            ));
            continue;
        }
        let size = t.text_mm as f32;
        let text = fit(&t.text, t.width as f32, size, t.bold);
        if text.is_empty() {
            continue;
        }
        let (x, anchor) = match t.align {
            Align::Left => (t.x, Anchor::CenterLeft),
            Align::Center => (t.x + t.width / 2.0, Anchor::Center),
            Align::Right => (t.x + t.width, Anchor::CenterRight),
        };
        let mut label = Label::new(
            Vec2::new(x as f32, (t.y + t.height / 2.0) as f32),
            text,
            size,
        );
        label.anchor = anchor;
        label.bold = t.bold;
        w.label(&label);
    }
    for l in &layout.lines {
        let width = if l.heavy { 0.35 } else { 0.15 };
        w.primitive(&Primitive::stroked(
            Shape::Line {
                a: Vec2::new(l.from.0 as f32, l.from.1 as f32),
                b: Vec2::new(l.to.0 as f32, l.to.1 as f32),
            },
            Stroke::world(width, Color::BLACK),
        ));
    }
}

/// One sheet to write: its layout, an optional bookmark, its height in mm.
struct Sheet {
    layout: PageLayout,
    bookmark: Option<String>,
    height: f32,
}

/// The footer line of a report PDF: document title on the left, `right` and
/// the page number on the right.
struct Footer<'a> {
    title: &'a str,
    right: &'a str,
}

/// Writes reports as one PDF, each section starting on a new page, with a
/// footer and a bookmark per section. An empty list gives one blank page,
/// so the result always opens in a reader. Output is deterministic.
pub fn export_report_pdf(reports: &[&ReportOutput], options: &ReportPdfOptions) -> Vec<u8> {
    let mut sheets: Vec<Sheet> = Vec::new();
    let translated: Vec<ReportOutput> = reports
        .iter()
        .map(|r| translate_output(r, &options.language))
        .collect();
    for report in &translated {
        // Paging is per section: rows per page depends on the section's
        // row height.
        for section in &report.sections {
            let setup = setup_for(options, &section.key);
            let single = ReportOutput {
                sections: vec![section.clone()],
                ..(*report).clone()
            };
            let pages = paginate(&single, &setup);
            for (n, page) in pages.iter().enumerate() {
                let mut layout = layout_page(&single, page, &setup);
                // layout_page prints "page n / m" within the one-section
                // report; the footer carries the document number instead.
                // Drop exactly that heading box, never a table cell.
                let heading_number = format!("page {} / {}", page.number, page.count);
                let heading_y = setup.margins.top;
                layout
                    .texts
                    .retain(|t| !(t.text == heading_number && t.y == heading_y && !t.bold));
                let layout = translate_layout(&layout, &options.language);
                let title = if section.title.is_empty() {
                    report.title.clone()
                } else {
                    format!("{} · {}", report.title, section.title)
                };
                sheets.push(Sheet {
                    height: layout.height as f32,
                    layout,
                    bookmark: (n == 0).then_some(title),
                });
            }
        }
    }
    let footer = Footer {
        title: &options.title,
        right: &options.footer_right,
    };
    write_sheets(sheets, options.format, &options.title, Some(&footer))
}

/// Writes pages laid out by the documentation designer
/// (`novo_reports::designer::render`) as one PDF. The template draws its own
/// frame and title block, so there is no footer; `bookmarks` name pages
/// (0-based index, title). Output is deterministic.
pub fn export_layouts_pdf(
    layouts: &[PageLayout],
    title: &str,
    bookmarks: &[(usize, String)],
) -> Vec<u8> {
    let sheets = layouts
        .iter()
        .enumerate()
        .map(|(i, layout)| Sheet {
            height: layout.height as f32,
            layout: layout.clone(),
            bookmark: bookmarks
                .iter()
                .find(|(p, _)| *p == i)
                .map(|(_, t)| t.clone()),
        })
        .collect();
    let format = layouts
        .first()
        .map_or_else(PageFormat::default, |l| PageFormat {
            size: PaperSize::A4,
            landscape: l.width > l.height,
            ..PageFormat::default()
        });
    write_sheets(sheets, format, title, None)
}

/// Writes laid-out sheets as one PDF with the shared page machinery: fonts,
/// bookmarks, deterministic output.
fn write_sheets(
    sheets: Vec<Sheet>,
    format: PageFormat,
    title: &str,
    footer: Option<&Footer<'_>>,
) -> Vec<u8> {
    let blank = format.size_mm();
    let count = sheets.len();

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
    let outline_id = alloc();
    let ids: Vec<(Ref, Ref)> = (0..count.max(1)).map(|_| (alloc(), alloc())).collect();

    let mut pdf = Pdf::new();
    let mut catalog = pdf.catalog(catalog_id);
    catalog.pages(tree_id);
    let has_marks = sheets.iter().any(|s| s.bookmark.is_some());
    if has_marks {
        catalog.outlines(outline_id);
        catalog.page_mode(PageMode::UseOutlines);
    }
    catalog.finish();
    pdf.pages(tree_id)
        .kids(ids.iter().map(|p| p.0))
        .count(ids.len() as i32);
    pdf.type1_font(font_id)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    let mut info = pdf.document_info(info_id);
    if !title.is_empty() {
        info.title(TextStr(title));
    }
    info.producer(TextStr("KoperCAD"));
    info.finish();

    let mut inter = inter::Inter::new();
    let mut inter_id: Option<Ref> = None;
    let mut bold_id: Option<Ref> = None;
    let (page_w, page_h) = (blank.0 as f32, blank.1 as f32);
    for (i, (page_ref, content_ref)) in ids.iter().enumerate() {
        let h = sheets.get(i).map_or(page_h, |s| s.height);
        let mut writer = draw::PageWriter::new(h, &mut inter);
        if let Some(sheet) = sheets.get(i) {
            draw_layout(&mut writer, &sheet.layout);
            if let Some(footer) = footer {
                let y = h - 6.0;
                let line = |x: f32, text: String, anchor: Anchor| {
                    let mut l = Label::new(Vec2::new(x, y), text, 2.5);
                    l.anchor = anchor;
                    l
                };
                let right = sheet.layout.width as f32 - 15.0;
                let half = (sheet.layout.width as f32 - 30.0) / 2.0;
                writer.label(&line(
                    15.0,
                    fit(footer.title, half, 2.5, false),
                    Anchor::CenterLeft,
                ));
                writer.label(&line(
                    right,
                    format!(
                        "{}{}page {} / {}",
                        fit(footer.right, half - 30.0, 2.5, false),
                        if footer.right.is_empty() { "" } else { "   " },
                        i + 1,
                        count
                    ),
                    Anchor::CenterRight,
                ));
            }
        }
        let mut page = pdf.page(*page_ref);
        page.parent(tree_id)
            .media_box(Rect::new(
                0.0,
                0.0,
                sheets.get(i).map_or(page_w, |s| s.layout.width as f32) * PT_PER_MM,
                h * PT_PER_MM,
            ))
            .contents(*content_ref);
        let mut resources = page.resources();
        let mut fonts = resources.fonts();
        fonts.pair(Name(font::FONT_NAME), font_id);
        if writer.uses_inter() {
            let id = *inter_id.get_or_insert_with(&mut alloc);
            fonts.pair(Name(inter::FONT_NAME), id);
        }
        if writer.uses_bold() {
            let id = *bold_id.get_or_insert_with(&mut alloc);
            fonts.pair(Name(font::BOLD_NAME), id);
        }
        fonts.finish();
        resources.finish();
        page.finish();
        let data = writer.finish();
        pdf.stream(*content_ref, &data);
    }
    if let Some(id) = inter_id {
        inter.write(&mut pdf, id, &mut alloc);
    }
    if let Some(id) = bold_id {
        pdf.type1_font(id)
            .base_font(Name(b"Helvetica-Bold"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
    }

    if has_marks {
        let marks: Vec<(usize, &str)> = sheets
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.bookmark.as_deref().map(|b| (i, b)))
            .collect();
        let items: Vec<Ref> = marks.iter().map(|_| alloc()).collect();
        pdf.outline(outline_id)
            .first(items[0])
            .last(items[items.len() - 1])
            .count(items.len() as i32);
        for (k, (page, title)) in marks.iter().enumerate() {
            let mut item = pdf.outline_item(items[k]);
            item.parent(outline_id).title(TextStr(title));
            if k > 0 {
                item.prev(items[k - 1]);
            }
            if k + 1 < items.len() {
                item.next(items[k + 1]);
            }
            item.dest().page(ids[*page].0).fit();
        }
    }
    pdf.finish()
}

/// A file format a single report can be written in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReportFileFormat {
    Pdf,
    Csv(CsvOptions),
    Xlsx,
}

impl ReportFileFormat {
    /// The format a file name asks for (`.pdf`, `.csv`, `.xlsx`); `None` for
    /// anything else.
    pub fn from_path(path: &std::path::Path) -> Option<ReportFileFormat> {
        let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
        match ext.as_str() {
            "pdf" => Some(ReportFileFormat::Pdf),
            "csv" => Some(ReportFileFormat::Csv(CsvOptions::default())),
            "xlsx" => Some(ReportFileFormat::Xlsx),
            _ => None,
        }
    }
}

/// One report as the bytes of a file in `format`. PDF uses `options`; CSV
/// and Excel carry the same tables without the page layout.
pub fn report_bytes(
    report: &ReportOutput,
    format: ReportFileFormat,
    options: &ReportPdfOptions,
) -> Result<Vec<u8>, ExportError> {
    match format {
        ReportFileFormat::Pdf => Ok(export_report_pdf(&[report], options)),
        ReportFileFormat::Csv(csv) => {
            Ok(to_csv(&translate_output(report, &options.language), &csv).into_bytes())
        }
        ReportFileFormat::Xlsx => xlsx_bytes(&[&translate_output(report, &options.language)]),
    }
}
