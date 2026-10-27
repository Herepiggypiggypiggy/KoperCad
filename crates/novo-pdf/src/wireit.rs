//! The Wire-it list as a PDF: every wire in the bench sequence on landscape
//! A4, one dense row per wire with a box to tick, the cut, strip, ferrule,
//! labels and the duct fill after the wire. Print it, hang it on the cabinet
//! and tick as you go; the app keeps the same ticks in the project.
//!
//! The sequence comes from `novo_reports::wireit`. Output is deterministic.

use novo_layout::plate_drawing::{HAlign, Layer, Marks};
use novo_reports::{WireClass, WireItPlan, WireItStep};

use crate::template::{Page, line, text, write_pages};
use crate::workpack::{clip, page, rect};

const SIZE: (f64, f64) = (297.0, 210.0);
const X0: f64 = 10.0;
const WIDTH: f64 = SIZE.0 - 2.0 * X0;
const TOP: f64 = 30.0;
const BOTTOM: f64 = SIZE.1 - 14.0;
const ROW_MM: f64 = 9.0;
const HEAD_MM: f64 = 7.0;
/// Column left edges (the cut column is right aligned at `CUT_RIGHT`).
const COL_NUM: f64 = X0 + 13.0;
const COL_WIRE: f64 = X0 + 17.0;
const COL_ENDS: f64 = X0 + 42.0;
const CUT_RIGHT: f64 = X0 + 112.0;
const COL_FIT: f64 = X0 + 116.0;
const COL_LABEL: f64 = X0 + 154.0;
const COL_ROUTE: f64 = X0 + 196.0;

/// One printed line of the list.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    /// A pass heading: `Power, 14 wires`.
    Heading(String),
    /// The step at this index of the plan.
    Step(usize),
}

/// The lines of each page, headings included. A heading is never the last
/// line of a page (it would be alone), and a pass that continues on the next
/// page is headed again there.
pub fn layout(plan: &WireItPlan) -> Vec<Vec<Line>> {
    let per_page = ((BOTTOM - TOP - HEAD_MM) / ROW_MM).floor().max(2.0) as usize;
    let mut pages: Vec<Vec<Line>> = vec![Vec::new()];
    let mut class: Option<WireClass> = None;
    let mut shown_on_page: Option<WireClass> = None;
    for (i, s) in plan.steps.iter().enumerate() {
        let need_heading = class != Some(s.class) || shown_on_page != Some(s.class);
        let room = per_page - pages.last().map_or(0, Vec::len);
        // A heading needs a step under it.
        if room < if need_heading { 2 } else { 1 } {
            pages.push(Vec::new());
            shown_on_page = None;
        }
        if class != Some(s.class) || shown_on_page != Some(s.class) {
            let n = plan.of_class(s.class).count();
            let first = class != Some(s.class);
            pages.last_mut().unwrap().push(Line::Heading(if first {
                format!(
                    "{}, {} wire{}",
                    s.class.name(),
                    n,
                    if n == 1 { "" } else { "s" }
                )
            } else {
                format!("{} (continued)", s.class.name())
            }));
            class = Some(s.class);
            shown_on_page = Some(s.class);
        }
        pages.last_mut().unwrap().push(Line::Step(i));
    }
    pages
}

/// Number of A4 pages the list makes.
pub fn page_count(plan: &WireItPlan) -> usize {
    if plan.steps.is_empty() {
        0
    } else {
        layout(plan).len()
    }
}

fn head(m: &mut Marks, project: &str, plan: &WireItPlan, done: &[bool], page_no: (usize, usize)) {
    text(
        m,
        (X0, 8.0),
        6.0,
        HAlign::Left,
        true,
        format!("Wire-it: {}", clip(project, 150.0, 6.0)),
    );
    text(
        m,
        (SIZE.0 - X0, 9.0),
        3.4,
        HAlign::Right,
        false,
        format!("page {} of {}", page_no.0, page_no.1),
    );
    let ticked = done.iter().filter(|d| **d).count();
    let left_m: f64 = plan
        .steps
        .iter()
        .zip(done)
        .filter(|(_, d)| !**d)
        .map(|(s, _)| s.cut_to_do_mm())
        .sum::<f64>()
        / 1000.0;
    text(
        m,
        (X0, 16.5),
        3.4,
        HAlign::Left,
        false,
        format!(
            "{} wires, {} run, {:.1} m of wire still to cut. Order: earth, power, 24 V, signals; \
             one duct at a time, short before long.",
            plan.steps.len(),
            ticked,
            left_m
        ),
    );
    text(
        m,
        (X0, 21.0),
        3.0,
        HAlign::Left,
        false,
        "Cut includes the end allowance; a cable core is run with its cable, not cut, and is left out of the total. Duct fill is the routing's model (wire area against the \
         duct's allowed fill), not a measurement: check the first wire against the real cabinet.",
    );
    // Column headings.
    let y = TOP - 1.0;
    for (x, align, h) in [
        (X0 + 1.0, HAlign::Left, "Done"),
        (COL_NUM, HAlign::Right, "No."),
        (COL_WIRE, HAlign::Left, "Wire, size"),
        (COL_ENDS, HAlign::Left, "From / to"),
        (CUT_RIGHT, HAlign::Right, "Cut / run mm"),
        (COL_FIT, HAlign::Left, "Strip, ferrule"),
        (COL_LABEL, HAlign::Left, "Label at both ends"),
        (COL_ROUTE, HAlign::Left, "Route, duct fill after the wire"),
    ] {
        text(m, (x, y), 2.8, align, true, h);
    }
    line(m, Layer::Table, (X0, TOP + 3.2), (X0 + WIDTH, TOP + 3.2));
}

fn cross(m: &mut Marks, x: f64, y: f64, s: f64) {
    line(m, Layer::Table, (x, y), (x + s, y + s));
    line(m, Layer::Table, (x, y + s), (x + s, y));
}

fn row(m: &mut Marks, s: &WireItStep, y: f64, done: bool) {
    let ty = y + 0.8;
    rect(m, Layer::Table, (X0 + 1.0, y + 1.6), (X0 + 6.0, y + 6.6));
    if done {
        cross(m, X0 + 1.0, y + 1.6, 5.0);
    }
    text(
        m,
        (COL_NUM, ty),
        3.4,
        HAlign::Right,
        false,
        s.number.to_string(),
    );
    let name = if s.wire_number.is_empty() {
        "(no number)".to_owned()
    } else {
        s.wire_number.clone()
    };
    text(
        m,
        (COL_WIRE, ty),
        3.4,
        HAlign::Left,
        true,
        clip(&name, 24.0, 3.4),
    );
    text(
        m,
        (COL_WIRE, ty + 4.0),
        2.8,
        HAlign::Left,
        false,
        clip(&s.kind_text(), 24.0, 2.8),
    );
    text(
        m,
        (COL_ENDS, ty),
        3.0,
        HAlign::Left,
        false,
        clip(&s.from, 68.0, 3.0),
    );
    text(
        m,
        (COL_ENDS, ty + 4.0),
        3.0,
        HAlign::Left,
        false,
        clip(&format!("to {}", s.to), 68.0, 3.0),
    );
    text(
        m,
        (CUT_RIGHT, ty),
        4.4,
        HAlign::Right,
        true,
        format!(
            "{}{}",
            if s.estimated() { "~" } else { "" },
            novo_layout::drill::mm(s.cut_mm)
        ),
    );
    text(
        m,
        (CUT_RIGHT, ty + 4.6),
        2.4,
        HAlign::Right,
        false,
        if s.cable_core {
            clip(&format!("run cable {}", s.cable_tag), 30.0, 2.4)
        } else {
            format!("+{} each end", novo_layout::drill::mm(s.allowance_mm))
        },
    );
    let strip = s.strip_text();
    text(
        m,
        (COL_FIT, ty),
        3.0,
        HAlign::Left,
        false,
        clip(
            if strip.is_empty() {
                "strip: size not set"
            } else {
                &strip
            },
            36.0,
            3.0,
        ),
    );
    text(
        m,
        (COL_FIT, ty + 4.0),
        2.8,
        HAlign::Left,
        true,
        clip(&s.fitting_text(), 36.0, 2.8),
    );
    text(
        m,
        (COL_LABEL, ty),
        2.8,
        HAlign::Left,
        false,
        clip(&s.label_from, 40.0, 2.8),
    );
    text(
        m,
        (COL_LABEL, ty + 4.0),
        2.8,
        HAlign::Left,
        false,
        clip(
            if s.label_to == s.label_from {
                "same at the other end"
            } else {
                &s.label_to
            },
            40.0,
            2.8,
        ),
    );
    text(
        m,
        (COL_ROUTE, ty),
        3.0,
        HAlign::Left,
        false,
        clip(&s.track, 88.0, 3.0),
    );
    text(
        m,
        (COL_ROUTE, ty + 4.0),
        2.8,
        HAlign::Left,
        s.over_filled(),
        clip(
            &if s.over_filled() {
                format!("OVER LIMIT: {}", s.fill_text())
            } else {
                s.fill_text()
            },
            88.0,
            2.8,
        ),
    );
    line(m, Layer::Margin, (X0, y + ROW_MM), (X0 + WIDTH, y + ROW_MM));
}

fn sheet(
    plan: &WireItPlan,
    done: &[bool],
    project: &str,
    lines: &[Line],
    no: (usize, usize),
) -> Marks {
    let mut m = Marks::default();
    head(&mut m, project, plan, done, no);
    let mut y = TOP + 4.5;
    for l in lines {
        match l {
            Line::Heading(h) => {
                text(
                    &mut m,
                    (X0 + 1.0, y + 1.6),
                    3.8,
                    HAlign::Left,
                    true,
                    h.clone(),
                );
                line(
                    &mut m,
                    Layer::Table,
                    (X0, y + HEAD_MM * 0.9),
                    (X0 + WIDTH, y + HEAD_MM * 0.9),
                );
                y += ROW_MM * 0.8;
            }
            Line::Step(i) => {
                row(
                    &mut m,
                    &plan.steps[*i],
                    y,
                    done.get(*i).copied().unwrap_or(false),
                );
                y += ROW_MM;
            }
        }
    }
    text(
        &mut m,
        (X0, SIZE.1 - 8.0),
        3.0,
        HAlign::Left,
        false,
        "Wired by ________  Date ________    Checked by ________  Date ________",
    );
    m
}

/// The list as a PDF. `done[i]` says whether step `i` is already run (a
/// ticked box is printed crossed). `None` when there is no wire.
pub fn export_wireit_pdf(plan: &WireItPlan, done: &[bool], project_name: &str) -> Option<Vec<u8>> {
    if plan.steps.is_empty() {
        return None;
    }
    let pages = layout(plan);
    let n = pages.len();
    let all: Vec<Page<'static>> = pages
        .iter()
        .enumerate()
        .map(|(i, lines)| {
            let mut p = page(sheet(plan, done, project_name, lines, (i + 1, n)));
            p.size = SIZE;
            p
        })
        .collect();
    Some(write_pages(&all, &format!("{project_name} - Wire-it")))
}
