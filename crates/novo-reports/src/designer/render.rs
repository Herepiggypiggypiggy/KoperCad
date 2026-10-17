//! Template + data -> positioned pages.

use novo_core::Document;

use super::fields::{Fields, expand};
use super::table::{TableSpec, build_table};
use super::template::{Element, TableElement, Template};
use super::validate::{DesignProblem, heading_mm, validate};
use crate::page::{Line, PageLayout, TextBox};
use crate::standard_reports;
use crate::table::{Align, Cell, Column, ReportOutput, Section};

/// The pages of a rendered template and what was wrong with it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Rendered {
    pub pages: Vec<PageLayout>,
    pub problems: Vec<DesignProblem>,
}

fn rows_per_page(t: &TableElement) -> usize {
    let room = t.height - heading_mm(&t.spec) - t.spec.header_mm;
    let rows = (room / t.spec.row_mm).floor();
    if rows.is_finite() && rows >= 1.0 {
        rows as usize
    } else {
        1
    }
}

/// Renders `template`: the frame, the title block and the elements on every
/// page, tables paged to their rectangle with the header repeated. Tables
/// are built from `data` (the generated reports, or [`preview_data`]).
/// `fields` supplies the placeholders; `page` and `pages` are set per page.
///
/// The document has as many pages as the longest table needs, and at least
/// one. Elements print on the first page only unless marked `every_page`.
pub fn render(template: &Template, data: &[ReportOutput], fields: &Fields) -> Rendered {
    let mut problems = validate(template, fields);
    let (width, height) = template.size_mm();

    let mut tables: Vec<(usize, &TableElement, Section)> = Vec::new();
    for (i, e) in template.elements.iter().enumerate() {
        if let Element::Table(t) = e {
            let (section, errors) = build_table(&t.spec, data);
            problems.extend(
                errors
                    .into_iter()
                    .map(|error| DesignProblem::Table { index: i, error }),
            );
            tables.push((i, t, section));
        }
    }
    let count = tables
        .iter()
        .map(|(_, t, s)| s.rows.len().div_ceil(rows_per_page(t)))
        .max()
        .unwrap_or(1)
        .max(1);

    let mut pages = Vec::with_capacity(count);
    for page in 0..count {
        let mut f = fields.clone();
        f.set("page", &(page + 1).to_string());
        f.set("pages", &count.to_string());
        let mut layout = PageLayout {
            width,
            height,
            ..PageLayout::default()
        };
        frame(template, &mut layout, &f);
        for (i, e) in template.elements.iter().enumerate() {
            let first = page == 0;
            match e {
                Element::Text {
                    x,
                    y,
                    width,
                    height,
                    text,
                    text_mm,
                    bold,
                    align,
                    every_page,
                    ..
                } if first || *every_page => layout.texts.push(TextBox {
                    x: *x,
                    y: *y,
                    width: *width,
                    height: *height,
                    text: expand(text, &f).0,
                    text_mm: *text_mm,
                    align: *align,
                    bold: *bold,
                }),
                Element::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    heavy,
                    every_page,
                } if first || *every_page => layout.lines.push(Line {
                    from: (*x1, *y1),
                    to: (*x2, *y2),
                    heavy: *heavy,
                }),
                Element::Rect {
                    x,
                    y,
                    width,
                    height,
                    heavy,
                    every_page,
                } if first || *every_page => rect(&mut layout, *x, *y, *width, *height, *heavy),
                Element::Table(_) => {
                    if let Some((_, t, section)) = tables.iter().find(|(ti, _, _)| *ti == i) {
                        table(&mut layout, t, section, page, &f);
                    }
                }
                _ => {}
            }
        }
        pages.push(layout);
    }
    Rendered { pages, problems }
}

fn rect(layout: &mut PageLayout, x: f64, y: f64, w: f64, h: f64, heavy: bool) {
    let edges = [
        ((x, y), (x + w, y)),
        ((x + w, y), (x + w, y + h)),
        ((x + w, y + h), (x, y + h)),
        ((x, y + h), (x, y)),
    ];
    for (from, to) in edges {
        layout.lines.push(Line { from, to, heavy });
    }
}

/// Sheet outline, frame, column ticks and numbers, title block: the same
/// geometry the canvas draws (`novo_canvas::model`).
fn frame(template: &Template, layout: &mut PageLayout, fields: &Fields) {
    let (w, h) = (layout.width, layout.height);
    if let Some(frame) = &template.frame {
        let m = frame.margin_mm;
        rect(layout, 0.0, 0.0, w, h, false);
        rect(layout, m, m, w - 2.0 * m, h - 2.0 * m, true);
        let columns = template.format.columns.max(1);
        let col_w = w / f64::from(columns);
        for c in 0..columns {
            let x0 = f64::from(c) * col_w;
            if c > 0 {
                for (y0, y1) in [(0.0, m), (h - m, h)] {
                    layout.lines.push(Line {
                        from: (x0, y0),
                        to: (x0, y1),
                        heavy: false,
                    });
                }
            }
            if frame.column_numbers {
                for y in [0.0, h - m] {
                    layout.texts.push(TextBox {
                        x: x0,
                        y,
                        width: col_w,
                        height: m,
                        text: (c + 1).to_string(),
                        text_mm: 3.0,
                        align: Align::Center,
                        bold: false,
                    });
                }
            }
        }
    }
    let (Some(block), Some((x, y, bw, bh))) = (&template.title_block, template.title_block_rect())
    else {
        return;
    };
    rect(layout, x, y, bw, bh, false);
    for (r, row) in block.rows.iter().enumerate() {
        let ry = y + block.row_mm * r as f64;
        if r > 0 {
            layout.lines.push(Line {
                from: (x, ry),
                to: (x + bw, ry),
                heavy: false,
            });
        }
        layout.texts.push(TextBox {
            x: x + 2.0,
            y: ry,
            width: (block.caption_mm - 2.0).max(0.0),
            height: block.row_mm,
            text: row.caption.clone(),
            text_mm: 2.0,
            align: Align::Left,
            bold: false,
        });
        let value = expand(&row.value, fields).0;
        if !value.trim().is_empty() {
            layout.texts.push(TextBox {
                x: x + block.caption_mm,
                y: ry,
                width: (bw - block.caption_mm - 1.0).max(0.0),
                height: block.row_mm,
                text: value,
                text_mm: 3.0,
                align: Align::Left,
                bold: false,
            });
        }
    }
}

fn table(layout: &mut PageLayout, t: &TableElement, section: &Section, page: usize, f: &Fields) {
    let per_page = rows_per_page(t);
    let start = page * per_page;
    if page > 0 && start >= section.rows.len() {
        return;
    }
    let end = (start + per_page).min(section.rows.len());
    let spec: &TableSpec = &t.spec;
    let mut y = t.y;
    if !spec.heading.is_empty() {
        layout.texts.push(TextBox {
            x: t.x,
            y,
            width: t.width,
            height: spec.header_mm,
            text: expand(&spec.heading, f).0,
            text_mm: spec.text_mm * 1.4,
            align: Align::Left,
            bold: true,
        });
        y += spec.header_mm;
    }
    let cols: &[Column] = &section.columns;
    if cols.is_empty() {
        return;
    }
    let total: f64 = cols.iter().map(|c| c.width.max(0.0)).sum();
    let mut edges = vec![t.x];
    for c in cols {
        let share = if total > 0.0 {
            c.width.max(0.0) / total
        } else {
            1.0 / cols.len() as f64
        };
        edges.push(edges[edges.len() - 1] + share * t.width);
    }
    let pad = 1.5;
    let cell = |i: usize, y: f64, h: f64, text: String, bold: bool| TextBox {
        x: edges[i] + pad,
        y,
        width: (edges[i + 1] - edges[i] - 2.0 * pad).max(0.0),
        height: h,
        text,
        text_mm: spec.text_mm,
        align: cols[i].align,
        bold,
    };
    let top = y;
    for (i, c) in cols.iter().enumerate() {
        layout
            .texts
            .push(cell(i, y, spec.header_mm, c.header.clone(), true));
    }
    y += spec.header_mm;
    let mut rules = vec![(top, true), (y, true)];
    for row in &section.rows[start..end] {
        for i in 0..cols.len() {
            let text = row
                .cells
                .get(i)
                .map(|c| section.cell_text(c))
                .unwrap_or_default();
            if !text.is_empty() {
                layout.texts.push(cell(i, y, spec.row_mm, text, false));
            }
        }
        y += spec.row_mm;
        rules.push((y, false));
    }
    if let Some(last) = rules.last_mut() {
        last.1 = true;
    }
    let right = edges[edges.len() - 1];
    for (ry, heavy) in rules {
        layout.lines.push(Line {
            from: (t.x, ry),
            to: (right, ry),
            heavy,
        });
    }
    for (i, &ex) in edges.iter().enumerate() {
        layout.lines.push(Line {
            from: (ex, top),
            to: (ex, y),
            heavy: i == 0 || i == edges.len() - 1,
        });
    }
}

/// Columns narrower than this share of the table get a number as sample text.
const NARROW_SAMPLE: f64 = 0.12;

/// Sample data for the live preview: for every table of `template`, a
/// report of that id with `rows` rows of placeholder text in the real
/// report's columns (the spec's own columns if the report is unknown or has
/// no section on an empty project). Filters still apply to it, so a filter
/// that matches nothing previews as an empty table, which is also what the
/// real data would show for a value that never occurs.
pub fn preview_data(template: &Template, rows: usize) -> Vec<ReportOutput> {
    let empty = Document::default();
    let reals: Vec<ReportOutput> = standard_reports()
        .iter()
        .map(|r| r.generate(empty.project()))
        .collect();
    let mut out: Vec<ReportOutput> = Vec::new();
    for e in &template.elements {
        let Element::Table(t) = e else { continue };
        let spec = &t.spec;
        if out.iter().any(|o| o.report == spec.report) {
            continue;
        }
        let real = reals.iter().find(|r| r.report == spec.report);
        let base = real.and_then(|r| r.sections.first());
        let columns = match base {
            Some(s) => s.columns.clone(),
            None => spec
                .columns
                .iter()
                .map(|c| Column::new(&c.key, c.header.as_deref().unwrap_or(&c.key), 10.0))
                .collect(),
        };
        let key = spec
            .section
            .as_deref()
            .or(base.map(|s| s.key.as_str()))
            .unwrap_or("sample");
        let mut section = Section::new(key, "", columns.clone());
        // A narrow column gets just the row number: the key would not fit
        // and the preview would show its clipped end.
        let total: f64 = columns.iter().map(|c| c.width).sum();
        for n in 1..=rows {
            let cells = columns
                .iter()
                .map(|c| {
                    if total > 0.0 && c.width / total < NARROW_SAMPLE {
                        Cell::Text(n.to_string())
                    } else {
                        Cell::Text(format!("{}-{n}", c.key))
                    }
                })
                .collect();
            section.push(cells, Vec::new());
        }
        out.push(ReportOutput {
            report: spec.report.clone(),
            title: real.map_or_else(|| spec.report.clone(), |r| r.title.clone()),
            revision: 0,
            sections: vec![section],
        });
    }
    out
}
