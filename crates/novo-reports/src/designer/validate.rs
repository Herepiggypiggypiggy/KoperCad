//! Checks on a template that need no report data.

use super::fields::{Fields, expand};
use super::table::TableError;
use super::template::{Element, Template};

/// A problem the designer lists next to the template. None of them stops
/// rendering: the page is drawn from what is valid.
#[derive(Debug, Clone, PartialEq)]
pub enum DesignProblem {
    EmptyName,
    /// `{field}` in `place` has no value; it prints as typed.
    UnknownField {
        place: String,
        field: String,
    },
    /// Element `index` reaches outside the sheet or the frame.
    OutsideSheet {
        index: usize,
    },
    /// Element `index` overlaps the title block.
    OverlapsTitleBlock {
        index: usize,
    },
    /// Tables `a` and `b` overlap.
    TablesOverlap {
        a: usize,
        b: usize,
    },
    /// Table `index` is too small for its header and one row.
    TableTooSmall {
        index: usize,
    },
    /// A table could not be fully resolved against the data.
    Table {
        index: usize,
        error: TableError,
    },
}

fn overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// Geometry and field problems of `template`, judged with `fields` as the
/// known names.
pub fn validate(template: &Template, fields: &Fields) -> Vec<DesignProblem> {
    let mut out = Vec::new();
    // `page` and `pages` are set by the renderer for every page.
    let mut fields = fields.clone();
    fields.set("page", "1");
    fields.set("pages", "1");
    let fields = &fields;
    if template.name.trim().is_empty() {
        out.push(DesignProblem::EmptyName);
    }
    let mut check = |place: String, text: &str| {
        for field in expand(text, fields).1 {
            let p = DesignProblem::UnknownField {
                place: place.clone(),
                field,
            };
            if !out.contains(&p) {
                out.push(p);
            }
        }
    };
    if let Some(block) = &template.title_block {
        for row in &block.rows {
            check(format!("title block, {}", row.caption), &row.value);
        }
    }
    for (i, e) in template.elements.iter().enumerate() {
        match e {
            Element::Text { text, .. } => check(format!("element {}", i + 1), text),
            Element::Table(t) => check(format!("element {}, heading", i + 1), &t.spec.heading),
            _ => {}
        }
    }

    let (w, h) = template.size_mm();
    let (ix, iy, iw, ih) = template.inner_rect();
    let block = template.title_block_rect();
    for (i, e) in template.elements.iter().enumerate() {
        let b = e.bounds();
        let eps = 1e-6;
        let sheet_ok = b.0 >= -eps && b.1 >= -eps && b.0 + b.2 <= w + eps && b.1 + b.3 <= h + eps;
        let frame_ok = template.frame.is_none()
            || (b.0 >= ix - eps
                && b.1 >= iy - eps
                && b.0 + b.2 <= ix + iw + eps
                && b.1 + b.3 <= iy + ih + eps);
        if !(sheet_ok && frame_ok) {
            out.push(DesignProblem::OutsideSheet { index: i });
        }
        if let Some(block) = block
            && overlap(b, block)
        {
            out.push(DesignProblem::OverlapsTitleBlock { index: i });
        }
        if let Element::Table(t) = e
            && t.height < t.spec.header_mm + t.spec.row_mm + heading_mm(&t.spec)
        {
            out.push(DesignProblem::TableTooSmall { index: i });
        }
    }
    let tables: Vec<(usize, (f64, f64, f64, f64))> = template
        .elements
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Element::Table(_)))
        .map(|(i, e)| (i, e.bounds()))
        .collect();
    for (n, (a, ba)) in tables.iter().enumerate() {
        for (b, bb) in &tables[n + 1..] {
            if overlap(*ba, *bb) {
                out.push(DesignProblem::TablesOverlap { a: *a, b: *b });
            }
        }
    }
    out
}

/// Height a table's heading takes (none when there is no heading text).
pub(super) fn heading_mm(spec: &super::table::TableSpec) -> f64 {
    if spec.heading.is_empty() {
        0.0
    } else {
        spec.header_mm
    }
}
