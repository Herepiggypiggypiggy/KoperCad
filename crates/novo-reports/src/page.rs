//! Paging a report onto sheets and laying a sheet out as positioned text.
//!
//! The output is renderer-neutral: text boxes and lines in millimetres on
//! the sheet, origin at the top-left corner, y pointing down (the same
//! convention as schematic pages). The canvas (WS-03) and the PDF export
//! (WS-09) draw a [`PageLayout`] inside their usual page frame and title
//! block; this crate draws nothing itself. [`PageSetup`] says how much of
//! the sheet the frame and title block take, so set it from the frame they
//! draw.

use std::ops::Range;

use novo_core::PageFormat;

use crate::table::{Align, ReportOutput};

/// Margins of the usable area, in mm from the sheet edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// How report pages are laid out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSetup {
    pub format: PageFormat,
    /// The page frame: the binding edge on the left is wider.
    pub margins: Margins,
    /// Height kept free above the bottom margin for the title block.
    pub title_block_mm: f64,
    /// Height of the band with the report and section title.
    pub heading_mm: f64,
    /// Height of the repeated column header row.
    pub header_row_mm: f64,
    pub row_mm: f64,
    /// Character height of table text; headings are 1.4 times this.
    pub text_mm: f64,
    /// Space between a cell's border and its text.
    pub padding_mm: f64,
}

impl Default for PageSetup {
    /// A3 landscape, like the schematic pages.
    fn default() -> Self {
        PageSetup::for_format(PageFormat::default())
    }
}

impl PageSetup {
    /// Default layout on a given sheet, inside the page frame the canvas and
    /// PDF draw on every page (`novo_canvas::model`: frame line 10 mm in
    /// from each edge, title block 36 mm high in the bottom-right corner):
    /// 5 mm clear of the frame, and the table stops 4 mm above the title
    /// block.
    pub fn for_format(format: PageFormat) -> Self {
        PageSetup {
            format,
            margins: Margins {
                left: 15.0,
                top: 15.0,
                right: 15.0,
                bottom: 10.0,
            },
            title_block_mm: 40.0,
            heading_mm: 12.0,
            header_row_mm: 8.0,
            row_mm: 6.0,
            text_mm: 2.5,
            padding_mm: 1.5,
        }
    }

    /// The table area: `(x, y, width, height)` in mm, below the heading
    /// band and above the title block.
    pub fn table_area(&self) -> (f64, f64, f64, f64) {
        let (width, height) = self.format.size_mm();
        let m = self.margins;
        let x = m.left;
        let y = m.top + self.heading_mm;
        let w = (width - m.left - m.right).max(0.0);
        let h = (height - y - m.bottom - self.title_block_mm).max(0.0);
        (x, y, w, h)
    }

    /// Data rows that fit on one page below the header row (at least one).
    pub fn rows_per_page(&self) -> usize {
        let (_, _, _, h) = self.table_area();
        let rows = ((h - self.header_row_mm) / self.row_mm).floor();
        if rows.is_finite() && rows >= 1.0 {
            rows as usize
        } else {
            1
        }
    }
}

/// One sheet of a paged report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportPage {
    /// 1-based page number within the report.
    pub number: usize,
    /// Pages of the whole report.
    pub count: usize,
    /// Index into [`ReportOutput::sections`].
    pub section: usize,
    /// The section's rows on this page.
    pub rows: Range<usize>,
    /// Whether the section started on an earlier page.
    pub continued: bool,
}

/// Splits a report into pages: every section starts on a new page, the
/// column header repeats on every page, an empty section still gets one
/// page. Pure and cheap (no text measuring).
pub fn paginate(report: &ReportOutput, setup: &PageSetup) -> Vec<ReportPage> {
    let per_page = setup.rows_per_page();
    let mut pages = Vec::new();
    for (section, s) in report.sections.iter().enumerate() {
        let mut start = 0;
        loop {
            let end = (start + per_page).min(s.rows.len());
            pages.push(ReportPage {
                number: pages.len() + 1,
                count: 0,
                section,
                rows: start..end,
                continued: start > 0,
            });
            start = end;
            if start >= s.rows.len() {
                break;
            }
        }
    }
    if pages.is_empty() {
        pages.push(ReportPage {
            number: 1,
            count: 0,
            section: 0,
            rows: 0..0,
            continued: false,
        });
    }
    let count = pages.len();
    for page in &mut pages {
        page.count = count;
    }
    pages
}

/// A text box on a laid-out page.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBox {
    /// Top-left corner of the box, mm.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text: String,
    /// Character height, mm.
    pub text_mm: f64,
    /// Horizontal alignment inside the box; vertically the text is centred.
    /// Text wider than the box is clipped to it by the renderer.
    pub align: Align,
    pub bold: bool,
}

/// A straight line on a laid-out page (table rules).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub from: (f64, f64),
    pub to: (f64, f64),
    /// Thick for the outline and header rule, thin between rows.
    pub heavy: bool,
}

/// A report page as positioned text and rules, ready to draw.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageLayout {
    /// Sheet size, mm.
    pub width: f64,
    pub height: f64,
    pub texts: Vec<TextBox>,
    pub lines: Vec<Line>,
}

/// Lays out one page of a report: a heading (`Bill of materials · Devices
/// without parts`, `continued` on follow-on pages, `page n / count` on the
/// right), the column header row and the page's rows, each cell a
/// [`TextBox`] inside a ruled grid. Column widths split the table width in
/// proportion to [`Column::width`](crate::Column::width).
pub fn layout_page(report: &ReportOutput, page: &ReportPage, setup: &PageSetup) -> PageLayout {
    let (width, height) = setup.format.size_mm();
    let mut layout = PageLayout {
        width,
        height,
        ..PageLayout::default()
    };
    let (x0, y0, table_w, _) = setup.table_area();
    let heading_text = setup.text_mm * 1.4;
    let heading_y = setup.margins.top;

    // A report with no sections (no strips, no cables) still prints its
    // heading on one page.
    let section = report.sections.get(page.section);
    let mut heading = report.title.clone();
    if let Some(title) = section.map(|s| &s.title).filter(|t| !t.is_empty()) {
        heading.push_str(" · ");
        heading.push_str(title);
    }
    if page.continued {
        heading.push_str(" (continued)");
    }
    layout.texts.push(TextBox {
        x: x0,
        y: heading_y,
        width: table_w * 0.75,
        height: setup.heading_mm,
        text: heading,
        text_mm: heading_text,
        align: Align::Left,
        bold: true,
    });
    layout.texts.push(TextBox {
        x: x0 + table_w * 0.75,
        y: heading_y,
        width: table_w * 0.25,
        height: setup.heading_mm,
        text: format!("page {} / {}", page.number, page.count),
        text_mm: setup.text_mm,
        align: Align::Right,
        bold: false,
    });

    let Some(section) = section else {
        return layout;
    };

    // Column edges.
    let total: f64 = section.columns.iter().map(|c| c.width.max(0.0)).sum();
    let mut edges = vec![x0];
    for column in &section.columns {
        let share = if total > 0.0 {
            column.width.max(0.0) / total
        } else {
            1.0 / section.columns.len() as f64
        };
        edges.push(edges.last().copied().unwrap_or(x0) + share * table_w);
    }

    let pad = setup.padding_mm;
    let cell = |x0: f64, x1: f64, y: f64, h: f64, text: String, align, bold| TextBox {
        x: x0 + pad,
        y,
        width: (x1 - x0 - 2.0 * pad).max(0.0),
        height: h,
        text,
        text_mm: setup.text_mm,
        align,
        bold,
    };
    for (i, column) in section.columns.iter().enumerate() {
        layout.texts.push(cell(
            edges[i],
            edges[i + 1],
            y0,
            setup.header_row_mm,
            column.header.clone(),
            column.align,
            true,
        ));
    }
    let mut y = y0 + setup.header_row_mm;
    let mut rules = vec![(y0, true), (y, true)];
    for row in &section.rows[page.rows.clone()] {
        for (i, column) in section.columns.iter().enumerate() {
            let text = row
                .cells
                .get(i)
                .map(ToString::to_string)
                .unwrap_or_default();
            if !text.is_empty() {
                layout.texts.push(cell(
                    edges[i],
                    edges[i + 1],
                    y,
                    setup.row_mm,
                    text,
                    column.align,
                    false,
                ));
            }
        }
        y += setup.row_mm;
        rules.push((y, false));
    }
    if let Some(last) = rules.last_mut() {
        last.1 = true;
    }
    let x1 = edges.last().copied().unwrap_or(x0);
    for (ry, heavy) in rules {
        layout.lines.push(Line {
            from: (x0, ry),
            to: (x1, ry),
            heavy,
        });
    }
    for (i, &ex) in edges.iter().enumerate() {
        layout.lines.push(Line {
            from: (ex, y0),
            to: (ex, y),
            heavy: i == 0 || i == edges.len() - 1,
        });
    }
    layout
}
