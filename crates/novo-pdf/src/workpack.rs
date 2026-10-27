//! The workshop pack as a PDF: a cover with the tool list, a tick-off
//! overview of the build order, then the work cards, two per A4 page (one
//! step per half page, cut or fold at the line between them).
//!
//! A card carries a hand template drawn at true size where it fits its area
//! and at a stated smaller scale where it does not (then it says so in
//! words and the distance list is the thing to measure from). True size
//! depends on the print dialog: every card carries a 100 mm ruler. The cards
//! come from `novo_layout::workpack`. Output is deterministic.

use novo_canvas::Vec2;
use novo_layout::assembly::Phase;
use novo_layout::drill::{CutoutShape, mm};
use novo_layout::plate_drawing::{HAlign, Layer, Marks, Shape as MarkShape};
use novo_layout::workpack::{Card, CardKind, Changes, Pack, Template};

use crate::draw::PageWriter;
use crate::plate::render;
use crate::template::{Page, line, ruler, text, write_pages};

const A4: (f64, f64) = (210.0, 297.0);
const CARD_X: f64 = 10.0;
const CARD_W: f64 = 190.0;
/// The strip card's three tick boxes: first box x and step from card left.
const STRIP_BOX_X: f64 = 168.5;
const STRIP_BOX_STEP: f64 = 6.5;
const CARD_H: f64 = 138.0;
/// Top of the two cards on a page.
const CARD_Y: [f64; 2] = [6.0, 153.0];
/// Where the bottom zone (ticks, note, ruler) of a card starts.
const FOOT_Y: f64 = CARD_H - 17.0;
/// Left text column of a card with a block template beside it.
const LEFT_W: f64 = 104.0;
/// Room for a block template: its left edge, width and height.
const BLOCK_X: f64 = 118.0;
const BLOCK_W: f64 = CARD_W - BLOCK_X - 4.0;
const BLOCK_H: f64 = 94.0;
const HEAD_H: f64 = 11.0;
const BODY_TEXT_MM: f64 = 3.4;
const SMALL_TEXT_MM: f64 = 2.9;
const LINE_MM: f64 = 4.1;
const BOX_MM: f64 = 6.0;
/// Rows of the overview on one page.
const OVERVIEW_ROWS: usize = 38;
const OVERVIEW_ROW_MM: f64 = 6.2;
/// Scales a template is drawn at when it does not fit at 1:1.
const SCALES: [f64; 6] = [2.0, 2.5, 5.0, 10.0, 20.0, 50.0];

/// The scale a `width` x `height` template is drawn at inside an area: `1.0`
/// when it fits, else the next 1:n of [`SCALES`] that does (the last one if
/// none does).
pub fn template_scale(width: f64, height: f64, area_w: f64, area_h: f64) -> f64 {
    if width <= area_w && height <= area_h {
        return 1.0;
    }
    SCALES
        .iter()
        .map(|n| 1.0 / n)
        .find(|s| width * s <= area_w && height * s <= area_h)
        .unwrap_or(1.0 / SCALES[SCALES.len() - 1])
}

fn scale_text(s: f64) -> String {
    if s >= 0.9999 {
        "1:1 true size, if printed at 100 %".to_owned()
    } else {
        format!("scale 1:{}, NOT true size", mm(1.0 / s))
    }
}

/// The second line under a scaled template's heading.
const SCALE_HINT: &str = "Measure from the list; the ruler below is true size";

/// Wraps `s` to `width` mm at text `height` (Helvetica averages about half
/// its height per character).
fn wrap(s: &str, width: f64, height: f64) -> Vec<String> {
    let per_line = (width / (height * 0.5)).floor().max(10.0) as usize;
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > per_line {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub(crate) fn clip(s: &str, width: f64, height: f64) -> String {
    let per_line = (width / (height * 0.5)).floor().max(4.0) as usize;
    if s.chars().count() <= per_line {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(per_line - 3).collect();
        t.push_str("...");
        t
    }
}

pub(crate) fn rect(m: &mut Marks, layer: Layer, min: (f64, f64), max: (f64, f64)) {
    m.shapes.push((layer, MarkShape::Rect { min, max }));
}

pub(crate) fn dashed(m: &mut Marks, layer: Layer, a: (f64, f64), b: (f64, f64)) {
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let n = (len / 4.0).floor().max(1.0) as usize;
    for i in 0..n {
        let (t0, t1) = (i as f64 / n as f64, (i as f64 + 0.55) / n as f64);
        let p = |t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        line(m, layer, p(t0), p(t1));
    }
}

/// Wrapped lines from `y` down to `bottom`; returns the next y and how many
/// lines did not fit.
fn lines_down(
    m: &mut Marks,
    x: f64,
    mut y: f64,
    width: f64,
    bottom: f64,
    h: f64,
    items: &[String],
) -> (f64, usize) {
    let mut left = 0;
    for item in items {
        for l in wrap(item, width, h) {
            if y + h > bottom {
                left += 1;
            } else {
                text(m, (x, y), h, HAlign::Left, false, l);
                y += h * 1.4;
            }
        }
    }
    (y, left)
}

fn tick_row(m: &mut Marks, x: f64, y: f64, labels: &[String]) {
    let mut x = x;
    for l in labels {
        rect(m, Layer::Table, (x, y), (x + BOX_MM, y + BOX_MM));
        text(
            m,
            (x + BOX_MM + 2.0, y + 1.2),
            3.6,
            HAlign::Left,
            false,
            l.clone(),
        );
        x += BOX_MM + 2.0 + l.chars().count() as f64 * 1.9 + 8.0;
    }
    text(
        m,
        (CARD_X + CARD_W - 4.0, y + 1.2),
        3.0,
        HAlign::Right,
        false,
        "Checked by ________  Date ________",
    );
}

/// Draws `t` with its top left at `(ox, oy)` on the page at scale `s`.
fn draw_template(m: &mut Marks, t: &Template, ox: f64, oy: f64, s: f64) {
    let map = |x: f64, y: f64| (ox + x * s, oy + y * s);
    let (w, h) = (t.width * s, t.height * s);
    rect(
        m,
        if t.rail { Layer::Rails } else { Layer::Devices },
        (ox, oy),
        (ox + w, oy + h),
    );
    if t.rail {
        line(m, Layer::Rails, (ox, oy + h / 2.0), (ox + w, oy + h / 2.0));
        // Each clipped device as a box on the strip with its designation,
        // as wide as the device is along the rail.
        for c in &t.clips {
            // A device that starts before the rail or runs past its end is cut
            // at the strip's ends here; the note lists the raw distances.
            let (x0, x1) = (ox + c.at.max(0.0) * s, ox + (c.at + c.len).min(t.width) * s);
            if x1 <= x0 {
                continue;
            }
            rect(m, Layer::Devices, (x0, oy + h * 0.15), (x1, oy + h * 0.85));
            let label = c.short();
            // Text in the lower half of the box, clear of the hole numbers above the holes. Text the box can hold; a narrow box keeps its outline only.
            if x1 - x0 >= label.chars().count() as f64 * 1.3 + 1.0 {
                text(
                    m,
                    ((x0 + x1) / 2.0, oy + h * 0.85 - 3.0),
                    2.4,
                    HAlign::Center,
                    false,
                    label.to_owned(),
                );
            }
        }
    }
    let tick = |m: &mut Marks, a: (f64, f64), b: (f64, f64)| line(m, Layer::Dimensions, a, b);
    let mut xs: Vec<f64> = t.holes.iter().map(|h| h.x).collect();
    if let Some(c) = &t.cutout {
        xs.push(c.x);
    }
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    // A cut-out template has no holes: its two axis numbers are the cross
    // hair's distances from the frame's corner, so they carry the unit.
    let unit = if t.holes.is_empty() && t.cutout.is_some() && !t.rail {
        " mm"
    } else {
        ""
    };
    let mut last_x = f64::NEG_INFINITY;
    for x in xs {
        let (px, _) = map(x, 0.0);
        tick(m, (px, oy - 1.5), (px, oy));
        if px - last_x >= 9.0 {
            text(
                m,
                (px, oy - 5.0),
                2.4,
                HAlign::Center,
                false,
                format!("{}{unit}", mm(x)),
            );
            last_x = px;
        }
    }
    if !t.rail {
        let mut ys: Vec<f64> = t.holes.iter().map(|h| h.y).collect();
        if let Some(c) = &t.cutout {
            ys.push(c.y);
        }
        ys.sort_by(f64::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        let mut last_y = f64::NEG_INFINITY;
        for y in ys {
            let (_, py) = map(0.0, y);
            tick(m, (ox - 1.5, py), (ox, py));
            if py - last_y >= 4.0 {
                text(
                    m,
                    (ox - 2.2, py - 1.0),
                    2.4,
                    HAlign::Right,
                    false,
                    format!("{}{unit}", mm(y)),
                );
                last_y = py;
            }
        }
    }
    let cross = |m: &mut Marks, p: (f64, f64), (ax, ay): (f64, f64)| {
        line(m, Layer::Holes, (p.0 - ax, p.1), (p.0 + ax, p.1));
        line(m, Layer::Holes, (p.0, p.1 - ay), (p.0, p.1 + ay));
    };
    for hole in &t.holes {
        let p = map(hole.x, hole.y);
        let r = hole.dia / 2.0 * s;
        m.shapes.push((
            Layer::Holes,
            MarkShape::Circle {
                centre: p,
                radius: r,
            },
        ));
        cross(m, p, (r + 3.0, r + 3.0));
        if hole.no > 0 {
            text(
                m,
                (p.0 + r + 1.5, p.1 - r - 3.6),
                2.6,
                HAlign::Left,
                true,
                format!("{}", hole.no),
            );
        }
    }
    if let Some(c) = &t.cutout {
        let p = map(c.x, c.y);
        match c.shape {
            CutoutShape::Rect { w, h } => rect(
                m,
                Layer::Cutouts,
                (p.0 - w * s / 2.0, p.1 - h * s / 2.0),
                (p.0 + w * s / 2.0, p.1 + h * s / 2.0),
            ),
            CutoutShape::Round { dia } => m.shapes.push((
                Layer::Cutouts,
                MarkShape::Circle {
                    centre: p,
                    radius: dia * s / 2.0,
                },
            )),
        }
        // Each arm reaches 3 mm past the outline on its own axis, so a wide
        // slot does not throw a vertical line far out of its frame.
        let arm = match c.shape {
            CutoutShape::Rect { w, h } => (w * s / 2.0 + 3.0, h * s / 2.0 + 3.0),
            CutoutShape::Round { dia } => (dia * s / 2.0 + 3.0, dia * s / 2.0 + 3.0),
        };
        cross(m, p, arm);
    }
    text(
        m,
        (ox, oy + h + 1.5),
        2.6,
        HAlign::Left,
        false,
        t.caption.clone(),
    );
}

/// Tallest the rail strip may be on a card, all windows together.
const RAIL_STRIP_MAX_MM: f64 = 62.0;
/// Scales a rail strip may be drawn at, largest first.
const RAIL_SCALES: [f64; 6] = [1.0, 0.5, 0.4, 0.2, 0.1, 0.05];
/// Room above a window for the hole positions and below it for the pitch.
const RAIL_LABEL_ABOVE_MM: f64 = 8.0;
const RAIL_LABEL_BELOW_MM: f64 = 9.0;
const RAIL_WINDOW_GAP_MM: f64 = 4.0;

/// How a rail strip is laid out on a card: the scale, the window length in
/// rail millimetres, and how many windows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailStripPlan {
    pub scale: f64,
    pub window_mm: f64,
    pub windows: usize,
    /// Height of all windows with their labels.
    pub height_mm: f64,
}

/// The largest scale at which a `length` x `band` mm rail fits `width` x
/// `max_h` mm of card, cut into windows of whole 10 mm when it is longer
/// than one line.
pub fn rail_strip_plan(length: f64, band: f64, width: f64, max_h: f64) -> RailStripPlan {
    let each = |s: f64| band * s + RAIL_LABEL_ABOVE_MM + RAIL_LABEL_BELOW_MM;
    let make = |s: f64| {
        let window = ((width / s / 10.0).floor() * 10.0).max(10.0);
        let windows = ((length / window).ceil() as usize).max(1);
        RailStripPlan {
            scale: s,
            window_mm: window,
            windows,
            height_mm: windows as f64 * each(s) + (windows - 1) as f64 * RAIL_WINDOW_GAP_MM,
        }
    };
    RAIL_SCALES
        .iter()
        .map(|&s| make(s))
        .find(|p| p.height_mm <= max_h)
        .unwrap_or_else(|| make(RAIL_SCALES[RAIL_SCALES.len() - 1]))
}

struct RailStrip {
    /// Bottom of the last window with its labels.
    bottom: f64,
}

/// Draws the rail of `t` with its left at `x` and its top at `y`: windows of
/// the rail at one scale, each with the hole positions from the rail start
/// above it, the devices clipped on it, and the pitch between holes under it.
fn rail_strip(m: &mut Marks, t: &Template, x: f64, y: f64, aw: f64) -> RailStrip {
    let plan = rail_strip_plan(t.width, t.height, aw, RAIL_STRIP_MAX_MM);
    let s = plan.scale;
    let h = t.height * s;
    text(
        m,
        (x - 8.0, y),
        2.8,
        HAlign::Left,
        true,
        if s >= 0.9999 {
            format!("Rail strip, {}", scale_text(s))
        } else {
            format!("Rail strip, {}. {SCALE_HINT}", scale_text(s))
        },
    );
    let mut top = y + 4.0;
    let mut holes: Vec<&novo_layout::workpack::TemplateHole> = t.holes.iter().collect();
    holes.sort_by(|a, b| a.x.total_cmp(&b.x));
    for k in 0..plan.windows {
        let a = k as f64 * plan.window_mm;
        let b = (a + plan.window_mm).min(t.width);
        let oy = top + RAIL_LABEL_ABOVE_MM;
        let px = |along: f64| x + (along - a) * s;
        rect(m, Layer::Rails, (px(a), oy), (px(b), oy + h));
        line(
            m,
            Layer::Rails,
            (px(a), oy + h / 2.0),
            (px(b), oy + h / 2.0),
        );
        if plan.windows > 1 {
            text(
                m,
                (px(a), oy + h + 0.8),
                2.2,
                HAlign::Left,
                false,
                format!("rail {} to {} mm", mm(a), mm(b)),
            );
        }
        // Devices clipped here, cut at the window edges.
        for c in &t.clips {
            let (c0, c1) = (c.at.max(a), (c.at + c.len).min(b));
            if c1 <= c0 {
                continue;
            }
            rect(
                m,
                Layer::Devices,
                (px(c0), oy + h * 0.15),
                (px(c1), oy + h * 0.85),
            );
            let label = c.short();
            if (c1 - c0) * s >= label.chars().count() as f64 * 1.3 + 1.0 {
                text(
                    m,
                    (px((c0 + c1) / 2.0), oy + h * 0.85 - 3.0),
                    2.4,
                    HAlign::Center,
                    false,
                    label.to_owned(),
                );
            }
        }
        // Holes of this window: cross, circle, number, position above.
        let mut last_label = f64::NEG_INFINITY;
        let mut prev: Option<f64> = None;
        for hole in &holes {
            if hole.x < a - 1e-9 || hole.x > b + 1e-9 || (hole.x >= b - 1e-9 && b < t.width) {
                continue;
            }
            let (cx, cy) = (px(hole.x), oy + hole.y * s);
            let r = hole.dia / 2.0 * s;
            m.shapes.push((
                Layer::Holes,
                MarkShape::Circle {
                    centre: (cx, cy),
                    radius: r,
                },
            ));
            line(m, Layer::Holes, (cx - r - 3.0, cy), (cx + r + 3.0, cy));
            line(m, Layer::Holes, (cx, cy - r - 3.0), (cx, cy + r + 3.0));
            line(m, Layer::Dimensions, (cx, oy - 1.5), (cx, oy));
            if cx - last_label >= 9.0 {
                text(m, (cx, oy - 5.0), 2.4, HAlign::Center, false, mm(hole.x));
                last_label = cx;
            }
            if hole.no > 0 && s >= 0.3 {
                text(
                    m,
                    (cx + r + 1.5, cy - r - 3.6),
                    2.6,
                    HAlign::Left,
                    true,
                    format!("{}", hole.no),
                );
            }
            // Pitch chain under the strip: a dimension line from the hole
            // before it, its value between them when it fits.
            let dim_y = oy + h + 5.0;
            if let Some(p) = prev {
                let (xa, xb) = (px(p), cx);
                line(m, Layer::Dimensions, (xa, dim_y), (xb, dim_y));
                line(m, Layer::Dimensions, (xa, dim_y - 1.2), (xa, dim_y + 1.2));
                line(m, Layer::Dimensions, (xb, dim_y - 1.2), (xb, dim_y + 1.2));
                let value = mm(hole.x - p);
                if xb - xa >= value.chars().count() as f64 * 1.3 + 2.0 {
                    text(
                        m,
                        ((xa + xb) / 2.0, dim_y + 1.4),
                        2.4,
                        HAlign::Center,
                        false,
                        value,
                    );
                }
            }
            prev = Some(hole.x);
        }
        top = oy + h + RAIL_LABEL_BELOW_MM + RAIL_WINDOW_GAP_MM;
    }
    RailStrip {
        bottom: top - RAIL_WINDOW_GAP_MM,
    }
}

/// The marks of one card with its top left at `(CARD_X, y0)`.
fn card_marks(card: &Card, cabinet: &str, y0: f64) -> Marks {
    let mut m = Marks::default();
    let x0 = CARD_X;
    let w = CARD_W;
    rect(&mut m, Layer::Plate, (x0, y0), (x0 + w, y0 + CARD_H));
    line(
        &mut m,
        Layer::Table,
        (x0, y0 + HEAD_H),
        (x0 + w, y0 + HEAD_H),
    );
    text(
        &mut m,
        (x0 + 3.0, y0 + 2.2),
        5.6,
        HAlign::Left,
        true,
        if card.kind == CardKind::Opening {
            "CUT-OUT".to_owned()
        } else if matches!(card.kind, CardKind::Outside | CardKind::Strips) {
            "NOT A STEP".to_owned()
        } else {
            format!(
                "STEP{} {} of {}",
                if card.steps.0 == card.steps.1 {
                    ""
                } else {
                    "S"
                },
                card.steps_text(),
                card.total
            )
        },
    );
    text(
        &mut m,
        (x0 + 60.0, y0 + 3.0),
        4.4,
        HAlign::Left,
        true,
        clip(
            &if card.mark == card.phase.name()
                || matches!(
                    card.kind,
                    CardKind::Opening | CardKind::Outside | CardKind::Strips
                )
            {
                card.mark.clone()
            } else {
                format!("{}  {}", card.phase.name(), card.mark)
            },
            85.0,
            4.4,
        ),
    );
    text(
        &mut m,
        (x0 + w - 3.0, y0 + 3.4),
        3.4,
        HAlign::Right,
        false,
        cabinet.to_owned(),
    );

    // What to do.
    let mut y = y0 + HEAD_H + 2.5;
    let block = is_block(card);
    let text_lines: Vec<String> = wrap(
        &card.text,
        if block { LEFT_W - 4.0 } else { w - 8.0 },
        BODY_TEXT_MM,
    )
    .into_iter()
    .take(if block { 6 } else { 3 })
    .collect();
    for l in &text_lines {
        text(
            &mut m,
            (x0 + 4.0, y),
            BODY_TEXT_MM,
            HAlign::Left,
            false,
            l.clone(),
        );
        y += BODY_TEXT_MM * 1.45;
    }
    y += 1.5;
    let bottom = y0 + FOOT_Y - 1.0;

    match card.kind {
        CardKind::Wires | CardKind::Outside => wire_table(&mut m, card, x0, y, bottom),
        CardKind::Strips => strip_table(&mut m, card, x0, y, bottom),
        CardKind::Labels => label_table(&mut m, card, x0, y, bottom),
        _ => body(&mut m, card, (x0, y0), y, bottom),
    }

    // Foot: ticks, honesty note, ruler.
    let fy = y0 + FOOT_Y;
    line(&mut m, Layer::Table, (x0, fy), (x0 + w, fy));
    tick_row(&mut m, x0 + 4.0, fy + 2.5, &card.ticks);
    ruler(&mut m, x0 + 4.0, y0 + CARD_H - 5.5);
    m
}

fn is_block(card: &Card) -> bool {
    card.template
        .as_ref()
        .is_some_and(|t| t.width <= t.height * 3.0)
}

const HONESTY_OPENING: &str = "Positions come from the cabinet model (Heat panel or opening settings), not from your cabinet: check the first cut-out against the real wall, door or plate.";
const HONESTY: &str = "Positions come from the cabinet layout, not from your plate: check the first part against the real plate.";

fn body(m: &mut Marks, card: &Card, (x0, y0): (f64, f64), y: f64, bottom: f64) {
    let honesty = if card.kind == CardKind::Opening {
        HONESTY_OPENING
    } else {
        HONESTY
    };
    let mut tools: Vec<String> = card.tools.iter().map(|t| format!("- {t}")).collect();
    tools.extend(card.notes.iter().map(|n| format!("Note: {n}")));
    let dims: Vec<String> = card.dims.iter().map(|d| format!("- {d}")).collect();
    let wires: Vec<String> = card
        .wires
        .iter()
        .map(|w| {
            format!(
                "- {}  {} to {}  {}",
                w.number,
                w.from,
                w.to,
                novo_layout::workpack::run_or_cut(w)
            )
        })
        .collect();

    // Two columns of lists: (x, width, top) of the tools and of the dims.
    let (tools_at, dims_at);
    let mut bottom = bottom;
    match &card.template {
        Some(t) if is_block(card) => {
            // Template on the right, true size where it fits; the lists in
            // the left column under the instruction.
            let s = template_scale(t.width, t.height, BLOCK_W - 8.0, BLOCK_H - 3.0);
            let ox = x0 + BLOCK_X + 8.0;
            let top = y0 + HEAD_H + 2.5;
            // A scaled template carries a second heading line: start it lower so
            // the dimension labels above the frame stay clear of the hint.
            draw_template(m, t, ox, top + if s < 0.9999 { 11.0 } else { 8.0 }, s);
            text(
                m,
                (x0 + BLOCK_X, top),
                2.6,
                HAlign::Left,
                true,
                clip(&format!("Template, {}", scale_text(s)), BLOCK_W, 2.6),
            );
            if s < 0.9999 {
                text(
                    m,
                    (x0 + BLOCK_X, top + 3.4),
                    2.4,
                    HAlign::Left,
                    false,
                    clip(SCALE_HINT, BLOCK_W, 2.4),
                );
            }
            tools.push(format!("- {}", t.origin));
            tools_at = (x0 + 4.0, LEFT_W - 4.0, y);
            dims_at = None;
        }
        Some(t) if t.rail => {
            // A rail strip across the card, as large as the card holds: a
            // long rail is drawn in windows, one under the other.
            let aw = CARD_W - 24.0;
            let strip = rail_strip(m, t, x0 + 12.0, y, aw);
            let below = strip.bottom + 2.0;
            text(
                m,
                (x0 + 4.0, below),
                2.6,
                HAlign::Left,
                false,
                t.origin.clone(),
            );
            // The tools and notes list the clipped devices: give them the wider
            // column so no position is cut off.
            tools_at = (x0 + 4.0, 96.0, below + 5.0);
            dims_at = Some((x0 + 104.0, CARD_W - 108.0, below + 5.0));
        }
        Some(t) => {
            // A duct strip across the card, lists below it.
            let aw = CARD_W - 24.0;
            let s = template_scale(t.width, t.height, aw, t.height);
            let oy = y + 11.0;
            text(
                m,
                (x0 + 4.0, y),
                2.8,
                HAlign::Left,
                true,
                if s < 0.9999 {
                    format!("Template, {}. {SCALE_HINT}", scale_text(s))
                } else {
                    format!("Template, {}", scale_text(s))
                },
            );
            draw_template(m, t, x0 + 12.0, oy, s);
            let below = oy + t.height * s + 6.0;
            text(
                m,
                (x0 + 4.0, below),
                2.6,
                HAlign::Left,
                false,
                t.origin.clone(),
            );
            tools_at = (x0 + 4.0, 78.0, below + 5.0);
            dims_at = Some((x0 + 86.0, CARD_W - 90.0, below + 5.0));
        }
        None => {
            tools_at = (x0 + 4.0, 82.0, y);
            dims_at = Some((x0 + 94.0, CARD_W - 98.0, y));
        }
    }
    let (tx, tw, ttop) = tools_at;
    // Block cards keep the honesty note at the foot of the left column.
    if is_block(card) {
        let lines = wrap(honesty, LEFT_W - 4.0, 2.5);
        let h = lines.len() as f64 * 3.5;
        for (i, l) in lines.into_iter().enumerate() {
            text(
                m,
                (tx, bottom - h + i as f64 * 3.5),
                2.5,
                HAlign::Left,
                false,
                l,
            );
        }
        bottom -= h + 1.5;
    } else {
        text(
            m,
            (x0 + 4.0, bottom - 2.6),
            2.5,
            HAlign::Left,
            false,
            honesty,
        );
        bottom -= 5.0;
    }

    text(m, (tx, ttop), SMALL_TEXT_MM, HAlign::Left, true, "Tools");
    let (mut ty, _) = lines_down(m, tx, ttop + LINE_MM, tw, bottom, SMALL_TEXT_MM, &tools);
    let (dx, dw, mut dy) = dims_at.unwrap_or((tx, tw, ty + 1.5));
    if !dims.is_empty() {
        text(
            m,
            (dx, dy),
            SMALL_TEXT_MM,
            HAlign::Left,
            true,
            "Mark and drill (nearest edges)",
        );
        let (ny, left) = lines_down(m, dx, dy + LINE_MM, dw, bottom, SMALL_TEXT_MM, &dims);
        dy = ny;
        if left > 0 {
            text(
                m,
                (dx, dy),
                SMALL_TEXT_MM,
                HAlign::Left,
                false,
                format!("... and {left} more lines: see the drilling template"),
            );
            dy += LINE_MM;
        }
    }
    if !wires.is_empty() {
        let same_column = dims_at.is_none();
        let (wx, ww, wy) = if same_column {
            (dx, dw, dy + 1.5)
        } else {
            ty += 1.5;
            (tx, tw, ty)
        };
        text(
            m,
            (wx, wy),
            SMALL_TEXT_MM,
            HAlign::Left,
            true,
            "Wires that end here (cut later)",
        );
        let (ny, left) = lines_down(m, wx, wy + LINE_MM, ww, bottom, SMALL_TEXT_MM, &wires);
        if card.wires_more + left > 0 {
            text(
                m,
                (wx, ny),
                SMALL_TEXT_MM,
                HAlign::Left,
                false,
                format!("... and {} more on the wire cards", card.wires_more + left),
            );
        }
    }
}

/// Room for the strip text between the size column and the cut length.
const STRIP_MAX_MM: f64 = 21.0;

fn wire_table(m: &mut Marks, card: &Card, x0: f64, y: f64, bottom: f64) {
    let cols = [4.0, 16.0, 44.0, 92.0, 146.0, 166.0];
    // Each end's allowance sits in the Size column's right part.
    const ENDS_X: f64 = 126.0;
    for (c, h) in ["Wire", "From", "To", "Size, colour", "Cut mm", "Cut"]
        .iter()
        .enumerate()
    {
        let (x, align) = if c == 4 {
            (x0 + cols[c] + 12.0, HAlign::Right)
        } else {
            (x0 + cols[c], HAlign::Left)
        };
        text(m, (x, y), SMALL_TEXT_MM, align, true, *h);
    }
    text(
        m,
        (x0 + cols[5] + 10.0, y),
        SMALL_TEXT_MM,
        HAlign::Left,
        true,
        "Run",
    );
    text(
        m,
        (x0 + ENDS_X, y),
        SMALL_TEXT_MM,
        HAlign::Left,
        true,
        "Per end",
    );
    line(m, Layer::Table, (x0, y + 4.2), (x0 + CARD_W, y + 4.2));
    let row_h = ((bottom - y - 6.0) / 9.0).clamp(6.0, 9.0);
    for (i, w) in card.wires.iter().enumerate() {
        let ry = y + 6.0 + i as f64 * row_h;
        let ty = ry + (row_h - 3.4) / 2.0 - 0.6;
        text(
            m,
            (x0 + cols[0], ty),
            3.2,
            HAlign::Left,
            true,
            clip(&w.number, 11.0, 3.2),
        );
        text(
            m,
            (x0 + cols[0], ty + 3.4),
            2.4,
            HAlign::Left,
            false,
            clip(&w.part_text(), 11.0, 2.4),
        );
        text(
            m,
            (x0 + cols[1], ty),
            3.0,
            HAlign::Left,
            false,
            clip(&w.from, 27.0, 3.0),
        );
        text(
            m,
            (x0 + cols[2], ty),
            3.0,
            HAlign::Left,
            false,
            clip(&w.to, 46.0, 3.0),
        );
        text(
            m,
            (x0 + cols[3], ty),
            3.0,
            HAlign::Left,
            false,
            clip(&w.kind, ENDS_X - cols[3] - 1.0, 3.0),
        );
        text(
            m,
            (x0 + ENDS_X, ty),
            3.0,
            HAlign::Left,
            false,
            clip(&w.end_text(), 20.0, 3.0),
        );
        // A longer text (two strip lengths) gets a smaller font, so the source
        // word stays inside the room between the size and the cut length.
        let strip = w.strip_text();
        let size = if strip.chars().count() > 16 { 2.1 } else { 2.6 };
        text(
            m,
            (x0 + ENDS_X, ty + 3.1),
            size,
            HAlign::Left,
            false,
            clip(&strip, STRIP_MAX_MM, size),
        );
        text(
            m,
            (x0 + cols[4] + 12.0, ty),
            3.4,
            HAlign::Right,
            true,
            format!("{}{}", if w.estimated { "~" } else { "" }, mm(w.cut_mm)),
        );
        for k in 0..2 {
            let bx = x0 + cols[5] + k as f64 * 10.0;
            rect(m, Layer::Table, (bx, ry + 0.6), (bx + 6.0, ry + 6.6));
        }
        line(
            m,
            Layer::Margin,
            (x0 + 3.0, ry + row_h),
            (x0 + CARD_W - 3.0, ry + row_h),
        );
    }
}

/// The cut-and-label strip: per row the length to cut (large, the thing the
/// hand needs), the wire, its ends, the ferrule to pick, and a marker flag to
/// cut out and wrap (the number twice either side of a fold line, so it reads
/// from both sides of a wrapped wire). Three tick boxes close the row.
fn strip_table(m: &mut Marks, card: &Card, x0: f64, y: f64, bottom: f64) {
    // Cores of a cable are run, not cut.
    let cores = card.ticks.first().is_some_and(|t| t == "Cable run");
    for (x, align, h) in [
        (
            4.0 + 20.0,
            HAlign::Right,
            if cores { "Cable mm" } else { "Cut mm" },
        ),
        (28.0, HAlign::Left, "Wire"),
        (46.0, HAlign::Left, "From / to"),
        (88.0, HAlign::Left, "Size, ferrule"),
        (128.0, HAlign::Left, "Marker (cut out, wrap)"),
    ] {
        text(m, (x0 + x, y), SMALL_TEXT_MM, align, true, h);
    }
    // One short header over each of the three tick boxes, inside the card.
    for (k, h) in [if cores { "Run" } else { "Cut" }, "Fer", "Mrk"]
        .iter()
        .enumerate()
    {
        let bx = x0 + STRIP_BOX_X + k as f64 * STRIP_BOX_STEP;
        text(m, (bx - 0.4, y), SMALL_TEXT_MM, HAlign::Left, true, *h);
    }
    line(m, Layer::Table, (x0, y + 4.2), (x0 + CARD_W, y + 4.2));
    let row_h = ((bottom - y - 6.0) / 9.0).clamp(6.0, 9.0);
    for (i, w) in card.wires.iter().enumerate() {
        let ry = y + 6.0 + i as f64 * row_h;
        let ty = ry + (row_h - 3.4) / 2.0 - 0.6;
        text(
            m,
            (x0 + 24.0, ty),
            4.6,
            HAlign::Right,
            true,
            format!("{}{}", if w.estimated { "~" } else { "" }, mm(w.cut_mm)),
        );
        text(
            m,
            (x0 + 28.0, ty),
            3.2,
            HAlign::Left,
            true,
            clip(&w.number, 16.0, 3.2),
        );
        text(
            m,
            (x0 + 28.0, ty + 3.4),
            2.4,
            HAlign::Left,
            false,
            clip(&w.part_text(), 16.0, 2.4),
        );
        text(
            m,
            (x0 + 46.0, ty),
            3.0,
            HAlign::Left,
            false,
            clip(&w.from, 40.0, 3.0),
        );
        text(
            m,
            (x0 + 46.0, ty + 3.2),
            3.0,
            HAlign::Left,
            false,
            clip(&format!("to {}", w.to), 40.0, 3.0),
        );
        text(
            m,
            (x0 + 88.0, ty),
            3.0,
            HAlign::Left,
            false,
            clip(&w.kind, 38.0, 3.0),
        );
        let ferrule = if w.ferrule.is_empty() {
            "ferrule: check size".to_owned()
        } else {
            w.ferrule.clone()
        };
        text(
            m,
            (x0 + 88.0, ty + 3.2),
            2.8,
            HAlign::Left,
            true,
            clip(&ferrule, 38.0, 2.8),
        );
        // The marker flag: two halves either side of a fold line.
        let (fx0, fx1) = (x0 + 128.0, x0 + 166.0);
        let (fy0, fy1) = (ry + 0.8, ry + row_h - 0.8);
        let marker = if w.number.is_empty() {
            "-".to_owned()
        } else {
            w.number.clone()
        };
        dashed(m, Layer::Table, (fx0, fy0), (fx1, fy0));
        dashed(m, Layer::Table, (fx0, fy1), (fx1, fy1));
        dashed(m, Layer::Table, (fx0, fy0), (fx0, fy1));
        dashed(m, Layer::Table, (fx1, fy0), (fx1, fy1));
        line(
            m,
            Layer::Margin,
            ((fx0 + fx1) / 2.0, fy0),
            ((fx0 + fx1) / 2.0, fy1),
        );
        for cx in [fx0 + 9.5, fx1 - 9.5] {
            text(
                m,
                (cx, (fy0 + fy1) / 2.0 - 1.6),
                3.4,
                HAlign::Center,
                true,
                clip(&marker, 17.0, 3.4),
            );
        }
        for k in 0..3 {
            let bx = x0 + STRIP_BOX_X + k as f64 * STRIP_BOX_STEP;
            rect(m, Layer::Table, (bx, ry + 0.8), (bx + 4.6, ry + 5.4));
        }
        line(
            m,
            Layer::Margin,
            (x0 + 3.0, ry + row_h),
            (x0 + CARD_W - 3.0, ry + row_h),
        );
    }
}

fn label_table(m: &mut Marks, card: &Card, x0: f64, y: f64, bottom: f64) {
    for (i, l) in card.labels.iter().enumerate() {
        let (col, row) = (i / 6, i % 6);
        let x = x0 + 4.0 + col as f64 * 92.0;
        let ry = y + row as f64 * 11.0;
        rect(m, Layer::Table, (x, ry), (x + BOX_MM, ry + BOX_MM));
        text(
            m,
            (x + BOX_MM + 3.0, ry + 0.8),
            4.2,
            HAlign::Left,
            true,
            clip(l, 70.0, 4.2),
        );
    }
    // The label stock and what the label says, under the rows.
    let mut ty = (y + 6.0 * 11.0 + 1.0).min(bottom - 10.0);
    for line in card.tools.iter().chain(&card.notes) {
        text(
            m,
            (x0 + 4.0, ty),
            2.6,
            HAlign::Left,
            false,
            clip(line, CARD_W - 8.0, 2.6),
        );
        ty += 3.6;
    }
}

/// The cover and tick-off sheet line about the cut-out cards, which sit
/// before the step cards but are done before the plate goes in.
fn cut_out_note(n: usize) -> String {
    format!(
        "{n} cut-out card{} at the front of the cards ({} for grilles, fans and wiring openings): do {} FIRST, before the plate goes in. They are not numbered steps.",
        if n == 1 { "" } else { "s" },
        "CUT-OUT",
        if n == 1 { "it" } else { "them" }
    )
}

/// The cover line about the wire cards for wires with no end on the plate.
fn outside_note(n: usize) -> String {
    format!(
        "{n} wire card{} \"Wires outside the plate\" at the end, not numbered (NOT A STEP): supply and field wires that have no end on this plate. Cut them with the cabinet wires and run them last.",
        if n == 1 { "" } else { "s" }
    )
}

/// The cover line about the cut-and-label strip cards.
fn strip_note(n: usize) -> String {
    format!(
        "{n} cut strip card{} at the end (NOT A STEP): the wires of each group longest first, with the ferrule to pick and a marker to cut out and wrap. Use {} to cut and mark the wires before the wire steps.",
        if n == 1 { "" } else { "s" },
        if n == 1 { "it" } else { "them" }
    )
}

/// What a print says about itself: the date, and, when the pack was printed
/// before, which cards differ from that print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    /// `YYYY-MM-DD` of this print.
    pub printed: String,
    /// Date of the earlier print the changes are measured against.
    pub since: Option<String>,
    pub changes: Changes,
}

impl Stamp {
    fn is_changed(&self, cabinet: &str, mark: &str) -> bool {
        self.since.is_some()
            && self
                .changes
                .changed
                .iter()
                .any(|(c, m)| c == cabinet && m == mark)
    }

    fn changed_in(&self, cabinet: &str) -> usize {
        self.changes
            .changed
            .iter()
            .filter(|(c, _)| c == cabinet)
            .count()
    }
}

/// The revision and print date in the card header, and a CHANGED banner when
/// the card differs from the earlier print.
fn stamp_card(m: &mut Marks, card: &Card, pack: &Pack, y0: f64, stamp: &Stamp) {
    let x1 = CARD_X + CARD_W - 3.0;
    text(
        m,
        (x1, y0 + 7.6),
        2.4,
        HAlign::Right,
        false,
        format!("Rev {}  printed {}", pack.revision(), stamp.printed),
    );
    if let (true, Some(since)) = (stamp.is_changed(&pack.cabinet, &card.mark), &stamp.since) {
        rect(
            m,
            Layer::Plate,
            (CARD_X + 60.0, y0 + 7.0),
            (CARD_X + 60.0 + 76.0, y0 + 10.4),
        );
        text(
            m,
            (CARD_X + 62.0, y0 + 7.5),
            2.6,
            HAlign::Left,
            true,
            format!("CHANGED since the print of {since}: check this card"),
        );
    }
}

fn cover_marks(pack: &Pack, project: &str, stamp: Option<&Stamp>) -> Marks {
    let mut m = Marks::default();
    if let Some(st) = stamp {
        let mut line = format!("Revision {}   printed {}", pack.revision(), st.printed);
        if let Some(since) = &st.since {
            let n = st.changed_in(&pack.cabinet);
            if n == 0 && st.changes.removed == 0 {
                line.push_str(&format!("   unchanged since the print of {since}"));
            } else {
                line.push_str(&format!(
                    "   {n} card{} CHANGED since the print of {since}{}",
                    if n == 1 { "" } else { "s" },
                    if st.changes.removed > 0 {
                        format!(", {} removed", st.changes.removed)
                    } else {
                        String::new()
                    }
                ));
            }
        }
        text(&mut m, (15.0, 31.2), 3.0, HAlign::Left, true, line);
    }
    let x0 = 15.0;
    let w = A4.0 - 30.0;
    text(
        &mut m,
        (x0, 15.0),
        8.0,
        HAlign::Left,
        true,
        format!("Workshop pack {}", pack.cabinet),
    );
    text(
        &mut m,
        (x0, 26.0),
        3.4,
        HAlign::Left,
        false,
        format!(
            "{project}   plate {} x {} mm   {} steps on {} cards",
            mm(pack.plate.0),
            mm(pack.plate.1),
            pack.overview.len(),
            pack.cards.len()
        ),
    );
    let mut y = 36.0;
    let how = [
        "How to use: print at 100 % (actual size), no fit-to-page. Check the 100 mm ruler on any card with a tape measure before you trust a template. Cut the sheets in half or fold them at the dashed line: each half is one step. Work in the order of the step numbers; tick the boxes as you go.",
        "How exact: the sizes and positions come from the cabinet layout and the part data. The template is as exact as your printer scale and your taping, about 0.5 mm if the ruler reads 100 mm (an estimate, not measured here). Wire lengths come from the cut list: routed lengths follow the duct model, estimates are marked as such. \"Per end\" is what each end adds to the routed length (strip plus ferrule collar or lug barrel, the project's wire rules), already in the cut length; the model has the same allowance at both ends. \"strip\" is the length to strip, the same as the cut list and the bench output use: a part's or the wire's stated length, else the typical length for the size (typ., a planning value, the datasheet of the clamp or ferrule wins). Two numbers are the from end and the to end. Beside a lug there is none: its barrel length counts. Check the first part against your real plate before drilling the rest.",
    ];
    let cut_outs = pack
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Opening)
        .count();
    let mut how: Vec<String> = how.iter().map(|p| (*p).to_owned()).collect();
    if cut_outs > 0 {
        how.insert(1, cut_out_note(cut_outs));
    }
    let outside = pack
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Outside)
        .count();
    if outside > 0 {
        how.insert(1, outside_note(outside));
    }
    let strips = pack
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Strips)
        .count();
    if strips > 0 {
        how.insert(1, strip_note(strips));
    }
    for p in how {
        let (ny, _) = lines_down(&mut m, x0, y, w, 120.0, 3.4, &[p]);
        y = ny + 3.0;
    }
    text(
        &mut m,
        (x0, y),
        5.0,
        HAlign::Left,
        true,
        "Tools and materials",
    );
    y += 8.0;
    text(&mut m, (x0, y), 3.0, HAlign::Left, true, "Tool");
    text(&mut m, (x0 + 80.0, y), 3.0, HAlign::Left, true, "For");
    line(&mut m, Layer::Table, (x0, y + 4.2), (x0 + w, y + 4.2));
    y += 6.0;
    for t in &pack.tools {
        if y > A4.1 - 30.0 {
            text(
                &mut m,
                (x0, y),
                3.0,
                HAlign::Left,
                false,
                "... more tools on the cards",
            );
            break;
        }
        text(
            &mut m,
            (x0, y),
            3.2,
            HAlign::Left,
            false,
            clip(&t.tool, 78.0, 3.2),
        );
        text(
            &mut m,
            (x0 + 80.0, y),
            3.2,
            HAlign::Left,
            false,
            clip(&t.for_what, w - 80.0, 3.2),
        );
        y += 5.2;
    }
    if !pack.problems.is_empty() {
        y += 3.0;
        text(
            &mut m,
            (x0, y),
            3.6,
            HAlign::Left,
            true,
            "Left out or not read:",
        );
        y += 5.0;
        let items: Vec<String> = pack.problems.iter().map(|p| format!("- {p}")).collect();
        lines_down(&mut m, x0, y, w, A4.1 - 15.0, 3.0, &items);
    }
    ruler(&mut m, x0, A4.1 - 12.0);
    m
}

fn overview_marks(pack: &Pack, rows: std::ops::Range<usize>, number: (usize, usize)) -> Marks {
    let mut m = Marks::default();
    let x0 = 15.0;
    let w = A4.0 - 30.0;
    text(
        &mut m,
        (x0, 12.0),
        5.5,
        HAlign::Left,
        true,
        format!("Tick-off sheet {}", pack.cabinet),
    );
    text(
        &mut m,
        (x0 + w, 14.0),
        3.2,
        HAlign::Right,
        false,
        format!("Page {} of {}", number.0, number.1),
    );
    let cut_outs = pack
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Opening)
        .count();
    if cut_outs > 0 {
        text(
            &mut m,
            (x0, 19.0),
            2.8,
            HAlign::Left,
            true,
            format!(
                "{cut_outs} CUT-OUT card{} at the front, not numbered: do {} FIRST, before the plate goes in.",
                if cut_outs == 1 { "" } else { "s" },
                if cut_outs == 1 { "it" } else { "them" }
            ),
        );
    }
    let outside = pack
        .cards
        .iter()
        .filter(|c| c.kind == CardKind::Outside)
        .count();
    if outside > 0 {
        text(
            &mut m,
            (x0, 21.4),
            2.6,
            HAlign::Left,
            true,
            format!(
                "{outside} wire card{} \"Wires outside the plate\" at the end, not numbered: supply and field wires.",
                if outside == 1 { "" } else { "s" }
            ),
        );
    }
    let top = 24.0;
    for (x, h) in [
        (0.0, "Step"),
        (11.0, "Phase"),
        (30.0, "Mark"),
        (56.0, "What to do"),
        (w - 10.0, "Done"),
    ] {
        text(&mut m, (x0 + x, top), 3.0, HAlign::Left, true, h);
    }
    line(&mut m, Layer::Table, (x0, top + 4.4), (x0 + w, top + 4.4));
    for (i, r) in pack.overview[rows].iter().enumerate() {
        let y = top + 6.5 + i as f64 * OVERVIEW_ROW_MM;
        text(&mut m, (x0, y), 3.2, HAlign::Left, true, r.step.to_string());
        text(
            &mut m,
            (x0 + 11.0, y),
            3.0,
            HAlign::Left,
            false,
            r.phase.name(),
        );
        text(
            &mut m,
            (x0 + 30.0, y),
            3.0,
            HAlign::Left,
            false,
            clip(&r.mark, 26.0, 3.0),
        );
        // The position is the point of the row: wrap to two lines, never cut it.
        let lines = wrap(&r.text, w - 56.0 - 12.0, 3.0);
        if lines.len() <= 1 {
            text(
                &mut m,
                (x0 + 56.0, y),
                3.0,
                HAlign::Left,
                false,
                r.text.clone(),
            );
        } else {
            let lines = wrap(&r.text, w - 56.0 - 12.0, 2.7);
            for (k, l) in lines.iter().take(2).enumerate() {
                text(
                    &mut m,
                    (x0 + 56.0, y - 1.0 + k as f64 * 3.1),
                    2.7,
                    HAlign::Left,
                    false,
                    l.clone(),
                );
            }
        }
        rect(
            &mut m,
            Layer::Table,
            (x0 + w - 8.0, y - 0.6),
            (x0 + w - 2.0, y + 5.0),
        );
        line(&mut m, Layer::Margin, (x0, y + 5.4), (x0 + w, y + 5.4));
    }
    m
}

fn identity(p: (f64, f64)) -> Vec2 {
    Vec2::new(p.0 as f32, p.1 as f32)
}

pub(crate) fn page(marks: Marks) -> Page<'static> {
    Page {
        size: A4,
        draw: Box::new(move |w: &mut PageWriter<'_>| render(w, &marks, &identity, 1.0)),
    }
}

fn pages(pack: &Pack, project: &str, stamp: Option<&Stamp>) -> Vec<Page<'static>> {
    let mut out = vec![page(cover_marks(pack, project, stamp))];
    let sheets = pack.overview.len().div_ceil(OVERVIEW_ROWS);
    for i in 0..sheets {
        let a = i * OVERVIEW_ROWS;
        let b = (a + OVERVIEW_ROWS).min(pack.overview.len());
        out.push(page(overview_marks(pack, a..b, (i + 1, sheets))));
    }
    for pair in pack.cards.chunks(2) {
        let mut m = Marks::default();
        for (card, y0) in pair.iter().zip(CARD_Y) {
            let mut c = card_marks(card, &pack.cabinet, y0);
            if let Some(st) = stamp {
                stamp_card(&mut c, card, pack, y0, st);
            }
            m.shapes.extend(c.shapes);
            m.texts.extend(c.texts);
        }
        dashed(&mut m, Layer::Margin, (3.0, 148.5), (A4.0 - 3.0, 148.5));
        text(
            &mut m,
            (A4.0 - 4.0, 146.0),
            2.2,
            HAlign::Right,
            false,
            "cut or fold here",
        );
        out.push(page(m));
    }
    out
}

/// Number of A4 pages the packs make.
pub fn page_count(packs: &[Pack]) -> usize {
    packs
        .iter()
        .map(|p| 1 + p.overview.len().div_ceil(OVERVIEW_ROWS) + p.cards.len().div_ceil(2))
        .sum()
}

/// The workshop pack of every cabinet as one PDF. `None` when no cabinet has
/// a step to build.
pub fn export_workpack_pdf(packs: &[Pack], project_name: &str) -> Option<Vec<u8>> {
    export_workpack_pdf_stamped(packs, project_name, None)
}

/// The same with the revision and print date on the cover and every card,
/// and the cards that changed since an earlier print marked CHANGED.
pub fn export_workpack_pdf_stamped(
    packs: &[Pack],
    project_name: &str,
    stamp: Option<&Stamp>,
) -> Option<Vec<u8>> {
    let all: Vec<Page<'static>> = packs
        .iter()
        .filter(|p| !p.cards.is_empty())
        .flat_map(|p| pages(p, project_name, stamp))
        .collect();
    if all.is_empty() {
        return None;
    }
    Some(write_pages(
        &all,
        &format!("{project_name} - workshop pack"),
    ))
}

/// One card of a pack as a one-page PDF (the card on the top half of A4).
/// `None` when the pack has no card `index`.
pub fn export_card_pdf(pack: &Pack, index: usize, project_name: &str) -> Option<Vec<u8>> {
    export_card_pdf_stamped(pack, index, project_name, None)
}

/// One card with the revision, print date and CHANGED banner.
pub fn export_card_pdf_stamped(
    pack: &Pack,
    index: usize,
    project_name: &str,
    stamp: Option<&Stamp>,
) -> Option<Vec<u8>> {
    let card = pack.cards.get(index)?;
    let mut m = card_marks(card, &pack.cabinet, CARD_Y[0]);
    if let Some(st) = stamp {
        stamp_card(&mut m, card, pack, CARD_Y[0], st);
    }
    dashed(&mut m, Layer::Margin, (3.0, 148.5), (A4.0 - 3.0, 148.5));
    Some(write_pages(
        &[page(m)],
        &format!("{project_name} - work card {}", card.mark),
    ))
}

/// Phases that have a hand template, for callers that want to say so.
pub fn has_template(phase: Phase) -> bool {
    matches!(phase, Phase::Rails | Phase::Ducts | Phase::Devices)
}

#[cfg(test)]
mod rail_strip_tests {
    use super::*;

    #[test]
    fn rail_strips_use_the_card_width_and_stay_true_to_their_stated_scale() {
        let (aw, band, max) = (166.0, 35.0, RAIL_STRIP_MAX_MM);
        // Short: true size. 400 mm keeps 1:2.5 on one line.
        assert_eq!(rail_strip_plan(150.0, band, aw, max).scale, 1.0);
        let p = rail_strip_plan(400.0, band, aw, max);
        assert_eq!((p.scale, p.windows), (0.4, 1));
        // A 1 000 mm rail was 1:10; in two windows it is 1:5.
        let p = rail_strip_plan(1000.0, band, aw, max);
        assert_eq!((p.scale, p.windows), (0.2, 2));
        assert!(p.window_mm * p.scale <= aw && p.height_mm <= max);
        // Windows cover the whole rail and every plan fits the card or is
        // the smallest scale.
        for len in [120.0, 333.0, 800.0, 1250.0, 2000.0, 3000.0, 6000.0] {
            let p = rail_strip_plan(len, band, aw, max);
            assert!(p.window_mm * p.windows as f64 >= len, "{len}");
            assert!(p.window_mm * p.scale <= aw + 1e-9, "{len}");
            assert!(p.height_mm <= max || p.scale == RAIL_SCALES[RAIL_SCALES.len() - 1]);
        }
    }

    fn rail_template(length: f64, xs: &[f64]) -> Template {
        Template {
            width: length,
            height: 35.0,
            caption: String::new(),
            origin: String::new(),
            rail: true,
            holes: xs
                .iter()
                .enumerate()
                .map(|(i, x)| novo_layout::workpack::TemplateHole {
                    x: *x,
                    y: 17.5,
                    dia: 5.5,
                    no: i + 1,
                })
                .collect(),
            cutout: None,
            clips: Vec::new(),
        }
    }

    #[test]
    fn a_long_rail_strip_labels_hole_positions_and_the_pitch_in_every_window() {
        let t = rail_template(1000.0, &[40.0, 204.0, 368.0, 532.0, 696.0, 860.0]);
        let mut m = Marks::default();
        let strip = rail_strip(&mut m, &t, 22.0, 30.0, 166.0);
        let texts: Vec<&str> = m.texts.iter().map(|t| t.text.as_str()).collect();
        for want in ["40", "204", "368", "532", "696", "860", "164"] {
            assert!(texts.contains(&want), "{want} missing in {texts:?}");
        }
        assert!(texts.iter().any(|t| t.starts_with("rail 0 to ")));
        assert!(texts.iter().any(|t| t.starts_with("rail 830 to ")));
        // The strip stays inside its band of the card.
        assert!(
            strip.bottom <= 30.0 + 4.0 + RAIL_STRIP_MAX_MM + 1.0,
            "{}",
            strip.bottom
        );
        // No window is wider than the card's strip area.
        for (_, shape) in &m.shapes {
            if let MarkShape::Rect { min, max } = shape {
                assert!(max.0 - min.0 <= 166.0 + 1e-6);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::drill::{DeviceFeatures, Hole, Surface};
    use novo_layout::state::CabinetLayout;
    use novo_layout::workpack::{WireCut, WireGroup, build};
    use novo_layout::{Margins, Placement, Plate, Rail, Size};

    pub(super) fn sample_pack() -> Pack {
        pack(3, false)
    }

    pub(super) fn pack(wires: usize, long_text: bool) -> Pack {
        let mut l = CabinetLayout::new(Plate {
            width: 800.0,
            height: 600.0,
            margin: 10.0,
        });
        l.rails = vec![Rail {
            id: 1,
            x: 50.0,
            y: 100.0,
            length: 1900.0,
            vertical: false,
        }];
        let mk = |id: u64, x: f64, rail: Option<u64>| Placement {
            id,
            x,
            y: 100.0,
            rotated: false,
            size: Size {
                width: 45.0,
                height: 90.0,
                guessed: false,
            },
            clearance: None,
            margins: Margins::default(),
            rail,
        };
        l.placements = vec![mk(1, 100.0, Some(1)), mk(2, 300.0, None)];
        let holes = (0..12)
            .map(|i| Hole {
                dx: 3.0 + i as f64,
                dy: 5.0,
                dia: 4.5,
            })
            .collect();
        let f = vec![DeviceFeatures {
            id: 2,
            label: "-Q2".into(),
            surface: Surface::Plate,
            holes,
            cutout: None,
            problems: Vec::new(),
        }];
        let from = if long_text {
            "=A1+CAB1-X1:1234567890123456789012345678901234567890"
        } else {
            "-Q1:1"
        };
        let g = vec![WireGroup {
            title: "Power".into(),
            wires: (0..wires)
                .map(|i| WireCut {
                    number: format!("W{i}"),
                    from: from.into(),
                    to: "-Q2:2".into(),
                    cut_mm: 250.0 + i as f64,
                    kind: "1.5 mm2 black".into(),
                    estimated: false,
                    end_mm: Some(10.0),
                    fitting: "ferrule",
                    part: None,
                    strip_from_mm: Some(10.0),
                    strip_to_mm: Some(10.0),
                    strip_basis: "typical",
                    ferrule: String::new(),
                })
                .collect(),
        }];
        build("+CAB1", &l, &f, &|id| format!("-Q{id}"), &g)
    }

    fn inside(m: &Marks, y0: f64) -> Vec<String> {
        let (x1, y1) = (CARD_X + CARD_W, y0 + CARD_H);
        let mut bad = Vec::new();
        for t in &m.texts {
            let (x, y) = t.pos;
            if x < CARD_X || x > x1 || y < y0 || y > y1 {
                bad.push(format!("text {:?} at {x:.1},{y:.1}", t.text));
            }
        }
        for (_, s) in &m.shapes {
            let pts = match *s {
                MarkShape::Line { a, b } => vec![a, b],
                MarkShape::Rect { min, max } => vec![min, max],
                MarkShape::Circle { centre, .. } => vec![centre],
            };
            for (x, y) in pts {
                if x < CARD_X - 0.01 || x > x1 + 0.01 || y < y0 - 0.01 || y > y1 + 0.01 {
                    bad.push(format!("shape at {x:.1},{y:.1}"));
                }
            }
        }
        bad
    }

    #[test]
    fn every_card_stays_inside_its_half_page() {
        for (wires, long) in [(3, false), (20, true)] {
            let p = pack(wires, long);
            assert!(!p.cards.is_empty());
            for card in &p.cards {
                for y0 in CARD_Y {
                    let m = card_marks(card, &p.cabinet, y0);
                    let bad = inside(&m, y0);
                    assert!(
                        bad.is_empty(),
                        "{} {}: {bad:?}",
                        card.mark,
                        card.steps_text()
                    );
                }
            }
        }
    }

    #[test]
    fn opening_cards_stay_inside_their_half_page() {
        use novo_layout::workpack::{OpeningJob, OpeningWall, add_opening_cards};
        let job = |wall, w: f64, h: f64| OpeningJob {
            name: "x".into(),
            what: "filter fan".into(),
            wall,
            along: 300.0,
            down: Some(200.0),
            width: w,
            height: h,
            size: (600.0, 800.0),
            depth: 250.0,
        };
        let mut p = pack(0, false);
        add_opening_cards(
            &mut p,
            &[
                job(OpeningWall::Door, 120.0, 120.0),
                job(OpeningWall::Door, 400.0, 60.0),
                job(OpeningWall::Left, 500.0, 200.0),
                job(OpeningWall::Top, 250.0, 120.0),
            ],
        );
        let cards: Vec<&Card> = p
            .cards
            .iter()
            .filter(|c| c.kind == CardKind::Opening)
            .collect();
        assert_eq!(cards.len(), 4);
        // The cover and every tick-off page say the cut-outs come first.
        let cover = cover_marks(&p, "Crane", None);
        let words: String = cover.texts.iter().map(|t| format!("{} ", t.text)).collect();
        assert!(words.contains("4 cut-out cards"), "{words}");
        assert!(words.contains("FIRST"), "{words}");
        let sheet = overview_marks(&p, 0..p.overview.len().min(5), (1, 1));
        assert!(
            sheet
                .texts
                .iter()
                .any(|t| t.text.contains("4 CUT-OUT cards"))
        );
        for card in cards {
            for y0 in CARD_Y {
                let m = card_marks(card, &p.cabinet, y0);
                let bad = inside(&m, y0);
                assert!(bad.is_empty(), "{}: {bad:?}", card.mark);
                assert!(m.texts.iter().any(|t| t.text == "CUT-OUT"));
                // Nothing of the card is cut off with dots.
                assert!(
                    !m.texts.iter().any(|t| t.text.ends_with("...")),
                    "{}: {:?}",
                    card.mark,
                    m.texts
                        .iter()
                        .filter(|t| t.text.ends_with("..."))
                        .map(|t| &t.text)
                        .collect::<Vec<_>>()
                );
                assert!(!m.texts.iter().any(|t| t.text.starts_with("STEP")));
            }
        }
    }

    fn rail_pack(rail_len: f64, xs: &[f64]) -> Pack {
        let mut l = CabinetLayout::new(Plate {
            width: 800.0,
            height: 600.0,
            margin: 10.0,
        });
        l.rails = vec![Rail {
            id: 1,
            x: 50.0,
            y: 100.0,
            length: rail_len,
            vertical: false,
        }];
        l.placements = xs
            .iter()
            .enumerate()
            .map(|(i, x)| Placement {
                id: i as u64 + 1,
                x: *x,
                y: 100.0,
                rotated: false,
                size: Size {
                    width: 45.0,
                    height: 90.0,
                    guessed: false,
                },
                clearance: None,
                margins: Margins::default(),
                rail: Some(1),
            })
            .collect();
        build("+CAB1", &l, &[], &|id| format!("=A1+CAB1-K{id}"), &[])
    }

    #[test]
    fn clipped_devices_are_boxes_on_the_rail_strip_as_wide_as_the_device() {
        // 300 mm rail at 1:2 (150 x 17.5 on paper): a 45 mm device is 22.5 mm wide.
        let p = rail_pack(300.0, &[60.0, 200.0]);
        let card = &p.cards[0];
        let t = card.template.as_ref().unwrap();
        let s = template_scale(t.width, t.height, CARD_W - 24.0, t.height);
        let m = card_marks(card, &p.cabinet, CARD_Y[0]);
        let boxes: Vec<f64> = m
            .shapes
            .iter()
            .filter_map(|(layer, sh)| match (layer, sh) {
                (Layer::Devices, MarkShape::Rect { min, max })
                    if (max.1 - min.1) < t.height * s && (max.1 - min.1) > 1.0 =>
                {
                    Some(max.0 - min.0)
                }
                _ => None,
            })
            .collect();
        assert_eq!(boxes.len(), 2, "{boxes:?}");
        assert!(
            boxes.iter().all(|w| (w - 45.0 * s).abs() < 1e-6),
            "{boxes:?}"
        );
        // The designation is on the box, K1 and K2 of =A1+CAB1-Kn.
        for k in ["K1", "K2"] {
            assert!(m.texts.iter().any(|t| t.text == k), "{k}");
        }
        assert!(inside(&m, CARD_Y[0]).is_empty());
    }

    /// `RAIL_DUMP=/tmp/rail.pdf cargo test -p novo-pdf --lib rail_dump` writes a
    /// 1 000 mm rail card to look at.
    #[test]
    fn rail_dump() {
        if let Ok(path) = std::env::var("RAIL_DUMP") {
            let p = rail_pack(1000.0, &[60.0, 200.0, 340.0, 480.0, 620.0, 760.0]);
            std::fs::write(path, export_card_pdf(&p, 0, "Demo").unwrap()).unwrap();
        }
    }

    #[test]
    fn a_rail_card_with_six_clipped_devices_prints_all_six_positions() {
        let xs = [60.0, 200.0, 340.0, 480.0, 620.0, 760.0];
        let p = rail_pack(1000.0, &xs);
        let m = card_marks(&p.cards[0], &p.cabinet, CARD_Y[0]);
        let all: String = m
            .texts
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        // Rail starts at x 50: the devices sit 10, 150, 290, 430, 570 and 710 mm along it.
        for (k, at) in (1..=6).zip([10, 150, 290, 430, 570, 710]) {
            assert!(
                all.contains(&format!("K{k} at {at}")),
                "K{k} at {at} missing in {all}"
            );
        }
        assert!(!all.contains("more lines"), "{all}");
        assert!(inside(&m, CARD_Y[0]).is_empty());
    }

    #[test]
    fn a_box_too_narrow_for_its_label_keeps_only_its_outline() {
        // 6000 mm rail at 1:20: a 45 mm device is 2.25 mm wide.
        let p = rail_pack(6000.0, &[60.0]);
        let m = card_marks(&p.cards[0], &p.cabinet, CARD_Y[0]);
        assert!(!m.texts.iter().any(|t| t.text == "K1"));
        assert!(m.shapes.iter().any(|(l, _)| *l == Layer::Devices));
    }

    #[test]
    fn a_wire_sharing_its_number_shows_which_of_them_it_is_under_the_number() {
        let mut p = pack(9, true);
        let card = p
            .cards
            .iter_mut()
            .find(|c| c.kind == CardKind::Wires)
            .unwrap();
        card.wires[0].part = Some((1, 2));
        card.wires[1].part = Some((2, 2));
        let card = p.cards.iter().find(|c| c.kind == CardKind::Wires).unwrap();
        let m = card_marks(card, &p.cabinet, CARD_Y[0]);
        let parts: Vec<_> = m
            .texts
            .iter()
            .filter(|t| t.text.ends_with(" of 2"))
            .collect();
        assert_eq!(parts.len(), 2);
        let number = m
            .texts
            .iter()
            .find(|t| t.text == card.wires[0].number)
            .unwrap();
        // Under its own wire number, at the same left edge.
        assert_eq!(parts[0].pos.0, number.pos.0);
        assert!(parts[0].pos.1 > number.pos.1);
        assert!(inside(&m, CARD_Y[0]).is_empty());
    }

    #[test]
    fn a_strip_text_for_two_different_ends_keeps_its_source_word() {
        let mut p = pack(9, true);
        let card = p
            .cards
            .iter_mut()
            .find(|c| c.kind == CardKind::Wires)
            .unwrap();
        card.wires[0].strip_from_mm = Some(8.0);
        card.wires[0].strip_to_mm = Some(12.5);
        let card = p.cards.iter().find(|c| c.kind == CardKind::Wires).unwrap();
        let m = card_marks(card, &p.cabinet, CARD_Y[0]);
        assert!(
            m.texts.iter().any(|t| t.text == "strip 8/12.5 mm typ."),
            "{:?}",
            m.texts
                .iter()
                .filter(|t| t.text.starts_with("strip"))
                .collect::<Vec<_>>()
        );
        assert!(inside(&m, CARD_Y[0]).is_empty());
    }

    #[test]
    fn the_note_of_a_two_metre_duct_stays_inside_its_card() {
        let mut p = pack(0, false);
        let mut card = p.cards[0].clone();
        card.kind = CardKind::Duct;
        card.template = None;
        card.notes = vec![
            "The duct is placed from the layout.".into(),
            novo_layout::workpack::duct_fixing_note(2000.0),
        ];
        p.cards = vec![card];
        let m = card_marks(&p.cards[0], &p.cabinet, CARD_Y[0]);
        assert!(
            m.texts
                .iter()
                .any(|t| t.text.contains("Fixing (a planning rule"))
        );
        assert!(
            inside(&m, CARD_Y[0]).is_empty(),
            "{:?}",
            inside(&m, CARD_Y[0])
        );
    }

    #[test]
    fn a_label_card_prints_its_stock_and_note_inside_the_card() {
        let mut p = pack(0, false);
        let mut card = p.cards[0].clone();
        card.kind = CardKind::Labels;
        card.labels = (0..12).map(|i| format!("=A1+CAB-K{i}")).collect();
        card.tools = vec![
            "Labels printed on A4 3x8, 70 x 37 mm (Bench output > Labels tab sets the stock)"
                .into(),
        ];
        card.notes =
            vec!["The label text is the device tag. Print at 100 %, no page scaling.".into()];
        p.cards = vec![card];
        let m = card_marks(&p.cards[0], &p.cabinet, CARD_Y[0]);
        for want in ["Labels printed on A4 3x8", "Print at 100 %"] {
            assert!(m.texts.iter().any(|t| t.text.contains(want)), "{want}");
        }
        assert!(
            inside(&m, CARD_Y[0]).is_empty(),
            "{:?}",
            inside(&m, CARD_Y[0])
        );
    }

    #[test]
    fn outside_the_plate_wires_get_cards_named_on_the_cover_and_the_tick_off_sheet() {
        use novo_layout::workpack::add_outside_wire_cards;
        let mut p = pack(0, false);
        let wires = pack(9, true)
            .cards
            .iter()
            .find(|c| c.kind == CardKind::Wires)
            .unwrap()
            .wires
            .clone();
        add_outside_wire_cards(&mut p, wires.clone());
        let cards: Vec<&Card> = p
            .cards
            .iter()
            .filter(|c| c.kind == CardKind::Outside)
            .collect();
        assert_eq!(
            cards.iter().map(|c| c.wires.len()).sum::<usize>(),
            wires.len()
        );
        let cover = cover_marks(&p, "Crane", None);
        let words: String = cover.texts.iter().map(|t| format!("{} ", t.text)).collect();
        assert!(words.contains("Wires outside the plate"), "{words}");
        let sheet = overview_marks(&p, 0..p.overview.len().min(5), (1, 1));
        assert!(
            sheet
                .texts
                .iter()
                .any(|t| t.text.contains("Wires outside the plate"))
        );
        for card in cards {
            for y0 in CARD_Y {
                let m = card_marks(card, &p.cabinet, y0);
                assert!(inside(&m, y0).is_empty(), "{:?}", inside(&m, y0));
                assert!(m.texts.iter().any(|t| t.text == "NOT A STEP"));
                assert!(m.texts.iter().any(|t| t.text == "Wires outside the plate"));
                assert!(!m.texts.iter().any(|t| t.text.starts_with("STEP")));
                // The same table as a wire card: a row per wire.
                assert!(m.texts.iter().any(|t| t.text == "Per end"));
            }
        }
    }

    #[test]
    fn a_wire_card_shows_each_wires_end_allowance_inside_its_columns() {
        let p = pack(9, true);
        let card = p.cards.iter().find(|c| c.kind == CardKind::Wires).unwrap();
        let m = card_marks(card, &p.cabinet, CARD_Y[0]);
        assert!(m.texts.iter().any(|t| t.text == "Per end"));
        let ends: Vec<_> = m
            .texts
            .iter()
            .filter(|t| t.text == "10 mm ferrule")
            .collect();
        assert_eq!(ends.len(), card.wires.len());
        // The size column ends before the allowance column starts.
        let kinds: Vec<_> = m
            .texts
            .iter()
            .filter(|t| t.text.starts_with("1.5 mm2"))
            .collect();
        assert_eq!(kinds.len(), card.wires.len());
        assert!(kinds[0].pos.0 < ends[0].pos.0);
        assert!(inside(&m, CARD_Y[0]).is_empty());
        // The strip length sits under the allowance, one per wire, inside the card.
        let strips: Vec<_> = m
            .texts
            .iter()
            .filter(|t| t.text == "strip 10 mm typ.")
            .collect();
        assert_eq!(strips.len(), card.wires.len());
        assert!(strips[0].pos.1 > ends[0].pos.1);
        assert_eq!(strips[0].pos.0, ends[0].pos.0);
    }

    #[test]
    fn a_long_wire_size_and_colour_is_not_cut_off_by_the_end_column() {
        let mut p = pack(3, false);
        let card = p
            .cards
            .iter_mut()
            .find(|c| c.kind == CardKind::Wires)
            .unwrap();
        card.wires[0].kind = "2.5 mm2 green/yellow".into();
        card.wires[1].end_mm = None;
        let m = card_marks(card, "+CAB1", CARD_Y[0]);
        assert!(m.texts.iter().any(|t| t.text == "2.5 mm2 green/yellow"));
        // No allowance in the cut list: an empty cell, never "0".
        let ends = m
            .texts
            .iter()
            .filter(|t| t.text.ends_with("ferrule") || t.text == "0 mm")
            .count();
        assert_eq!(ends, card.wires.len() - 1);
    }

    #[test]
    fn a_scaled_templates_hint_stays_clear_of_its_dimension_labels() {
        use novo_layout::workpack::{OpeningJob, OpeningWall, add_opening_cards};
        let mut p = pack(0, false);
        add_opening_cards(
            &mut p,
            &[OpeningJob {
                name: "x".into(),
                what: "grille".into(),
                wall: OpeningWall::Door,
                along: 300.0,
                down: Some(200.0),
                width: 60.0,
                height: 60.0,
                size: (600.0, 800.0),
                depth: 250.0,
            }],
        );
        let card = p.cards.first().unwrap();
        assert_eq!(card.kind, CardKind::Opening);
        let m = card_marks(card, &p.cabinet, CARD_Y[0]);
        let hint = m
            .texts
            .iter()
            .find(|t| t.text.contains("Measure from the list"))
            .expect("hint");
        // Labels above the frame: the numeric ones in the block column.
        let labels: Vec<_> = m
            .texts
            .iter()
            .filter(|t| {
                t.pos.0 > CARD_X + BLOCK_X
                    && t.text.trim_end_matches(" mm").parse::<f64>().is_ok()
                    && t.pos.1 < hint.pos.1 + 10.0
            })
            .collect();
        assert!(!labels.is_empty());
        for l in labels {
            assert!(
                l.pos.1 >= hint.pos.1 + 2.4,
                "{} at {} hint at {}",
                l.text,
                l.pos.1,
                hint.pos.1
            );
        }
    }

    #[test]
    fn wiring_opening_cards_stay_inside_their_half_page() {
        use novo_layout::openings::{Opening, OpeningShape};
        use novo_layout::workpack::add_wiring_opening_cards;
        let mut l = CabinetLayout::new(Plate {
            width: 800.0,
            height: 600.0,
            margin: 10.0,
        });
        l.surface_sizes.insert("door".into(), (600.0, 800.0));
        let mk = |shape, surface: &str| Opening {
            name: "cable entry".into(),
            surface: surface.into(),
            shape,
            x: 300.0,
            y: 200.0,
            keep: 5.0,
            exit: None,
        };
        l.openings = vec![
            mk(OpeningShape::Round { dia: 25.0 }, "plate"),
            mk(OpeningShape::Rect { w: 300.0, h: 40.0 }, "door"),
        ];
        let mut p = pack(0, false);
        add_wiring_opening_cards(&mut p, &l);
        for card in p.cards.iter().filter(|c| c.kind == CardKind::Opening) {
            for y0 in CARD_Y {
                let m = card_marks(card, &p.cabinet, y0);
                let bad = inside(&m, y0);
                assert!(bad.is_empty(), "{}: {bad:?}", card.mark);
                assert!(
                    !m.texts.iter().any(|t| t.text.ends_with("...")),
                    "{}",
                    card.mark
                );
            }
        }
    }

    #[test]
    fn card_text_does_not_run_past_the_right_edge() {
        let p = pack(20, true);
        let right = CARD_X + CARD_W;
        for card in &p.cards {
            let m = card_marks(card, &p.cabinet, CARD_Y[0]);
            for t in m.texts.iter().filter(|t| t.h == HAlign::Left) {
                let w = t.text.chars().count() as f64 * t.height * 0.5;
                assert!(
                    t.pos.0 + w <= right + 0.5,
                    "{} {:?} ends at {:.1}",
                    card.mark,
                    t.text,
                    t.pos.0 + w
                );
            }
        }
    }

    #[test]
    fn scale_is_one_when_it_fits_and_a_stated_fraction_when_not() {
        assert_eq!(template_scale(45.0, 90.0, 94.0, 100.0), 1.0);
        assert_eq!(template_scale(400.0, 35.0, 166.0, 35.0), 1.0 / 2.5);
        assert_eq!(template_scale(1900.0, 35.0, 166.0, 35.0), 1.0 / 20.0);
        // Nothing fits: the smallest stated scale, never an unstated one.
        assert_eq!(template_scale(1.0e6, 35.0, 166.0, 35.0), 1.0 / 50.0);
        assert!(scale_text(1.0).contains("true size"));
        assert!(scale_text(1.0 / 2.5).contains("1:2.5"));
        assert!(scale_text(0.4).contains("NOT true size"));
    }
}

#[cfg(test)]
mod overview_tests {
    use super::*;

    #[test]
    fn a_long_step_text_is_wrapped_and_keeps_its_position() {
        let mut p = tests_pack();
        p.overview[0].text = "Cut a 50 mm wide wiring duct to 530 mm and fit it level at x 55, y 250 on the mounting plate of the cabinet".to_owned();
        let m = overview_marks(&p, 0..1, (1, 1));
        let joined: String = m
            .texts
            .iter()
            .filter(|t| t.pos.0 >= 15.0 + 56.0 - 0.01 && t.pos.1 > 28.0)
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(joined.contains("x 55, y 250"), "{joined}");
        assert!(!joined.contains("..."), "{joined}");
    }

    fn tests_pack() -> Pack {
        tests::sample_pack()
    }

    #[test]
    fn strip_card_prints_length_ferrule_and_a_marker_twice_per_wire() {
        let mut p = tests::pack(3, true);
        let mut g = vec![novo_layout::workpack::WireGroup {
            title: "Power".to_owned(),
            wires: p
                .cards
                .iter()
                .find(|c| c.kind == CardKind::Wires)
                .unwrap()
                .wires
                .clone(),
        }];
        for w in &mut g[0].wires {
            w.ferrule = "1.5 mm2 black, 8 mm".to_owned();
        }
        novo_layout::workpack::add_strip_cards(&mut p, &g);
        let card = p.cards.iter().find(|c| c.kind == CardKind::Strips).unwrap();
        let m = card_marks(card, &p.cabinet, CARD_Y[0]);
        let words: Vec<&str> = m.texts.iter().map(|t| t.text.as_str()).collect();
        assert!(words.contains(&"NOT A STEP"), "{words:?}");
        let n = card.wires.len();
        assert_eq!(
            words
                .iter()
                .filter(|w| **w == "1.5 mm2 black, 8 mm")
                .count(),
            n
        );
        let first = &card.wires[0];
        assert!(words.contains(&mm(first.cut_mm).as_str()));
        // The marker text is printed on both sides of the fold line.
        assert_eq!(
            words.iter().filter(|w| **w == first.number).count(),
            // marker twice, plus the number column
            3
        );
        // Three tick boxes per row, plus the footer's.
        let boxes = m
            .shapes
            .iter()
            .filter(|(l, s)| {
                *l == Layer::Table
                    && matches!(s, MarkShape::Rect { min, max }
                        if (max.0 - min.0 - 4.6).abs() < 1e-6)
            })
            .count();
        assert_eq!(boxes, 3 * n);
        assert!(strip_note(1).contains("NOT A STEP"));
    }

    #[test]
    fn cut_out_template_axes_carry_a_unit() {
        use novo_layout::workpack::{OpeningJob, OpeningWall, add_opening_cards};
        let mut p = tests_pack();
        add_opening_cards(
            &mut p,
            &[OpeningJob {
                name: "x".into(),
                what: "grille".into(),
                wall: OpeningWall::Door,
                along: 300.0,
                down: Some(200.0),
                width: 150.0,
                height: 100.0,
                size: (600.0, 800.0),
                depth: 250.0,
            }],
        );
        let card = &p.cards[0];
        let m = card_marks(card, &p.cabinet, CARD_Y[0]);
        for want in ["85 mm", "60 mm"] {
            assert!(m.texts.iter().any(|t| t.text == want), "{want}");
        }
        assert!(!m.texts.iter().any(|t| t.text == "85" || t.text == "60"));
        let origin = &card.template.as_ref().unwrap().origin;
        assert!(origin.contains("Frame 170 x 120 mm"), "{origin}");
        // Cut-outs are the first cards and the cover says so.
        assert_eq!(p.cards[0].kind, CardKind::Opening);
        let cover = cover_marks(&p, "Crane", None);
        let words: String = cover.texts.iter().map(|t| format!("{} ", t.text)).collect();
        assert!(words.contains("at the front"), "{words}");
    }
}
