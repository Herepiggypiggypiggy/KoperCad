//! Printable label sheets: the labels of [`Labels`](crate::Labels) laid out
//! on A4 sticker sheets (any grid) or on tape, one label per page, for a
//! label printer that prints a PDF at its tape width.
//!
//! This crate only lays out: [`layout_labels`] gives [`PageLayout`]s in mm
//! that the PDF writer draws at 1:1. Print the PDF at 100% (no "fit to
//! page"), or labels drift off their stickers. The sheet presets give the
//! pitch of well-known sticker sheets from their published data and were
//! not checked on a printer here; a test print on plain paper held against
//! the sheet, and the offset fields, correct any drift.

use novo_core::{Project, Value};
use serde::{Deserialize, Serialize};

use crate::page::{Line, PageLayout, TextBox};
use crate::table::{Align, ReportOutput};

/// Project property that holds the label settings (JSON text).
pub const SETTINGS_KEY: &str = "bench.labels";

/// Text smaller than this (mm, character height) is hard to read on paper.
pub const MIN_TEXT_MM: f64 = 2.0;

/// A grid of labels on a flat sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetGrid {
    pub name: String,
    pub page_w: f64,
    pub page_h: f64,
    pub cols: usize,
    pub rows: usize,
    pub label_w: f64,
    pub label_h: f64,
    /// Left and top margin to the first label's corner, mm.
    pub left: f64,
    pub top: f64,
    /// Distance from one label's corner to the next, mm (label size plus gap).
    pub pitch_x: f64,
    pub pitch_y: f64,
}

/// Continuous tape: one label per page, the page as wide as the tape is
/// high.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TapeStock {
    /// Tape width, mm: the page height.
    pub tape_mm: f64,
    /// Label length, mm; `None` sizes each label to its text.
    pub length_mm: Option<f64>,
}

/// What the labels are printed on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LabelStock {
    Sheet(SheetGrid),
    Tape(TapeStock),
}

impl LabelStock {
    pub fn name(&self) -> String {
        match self {
            LabelStock::Sheet(g) => g.name.clone(),
            LabelStock::Tape(t) => format!("Tape {} mm", crate::bench::number_text(t.tape_mm)),
        }
    }

    /// The stock is usable: sizes positive and the grid inside the sheet.
    pub fn is_valid(&self) -> bool {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        match self {
            LabelStock::Sheet(g) => {
                g.cols >= 1
                    && g.rows >= 1
                    && [
                        g.page_w, g.page_h, g.label_w, g.label_h, g.pitch_x, g.pitch_y,
                    ]
                    .into_iter()
                    .all(ok)
                    && g.left.is_finite()
                    && g.top.is_finite()
                    && g.left >= 0.0
                    && g.top >= 0.0
                    && g.left + g.pitch_x * (g.cols - 1) as f64 + g.label_w <= g.page_w + 1e-6
                    && g.top + g.pitch_y * (g.rows - 1) as f64 + g.label_h <= g.page_h + 1e-6
                    && g.pitch_x >= g.label_w - 1e-6
                    && g.pitch_y >= g.label_h - 1e-6
            }
            LabelStock::Tape(t) => ok(t.tape_mm) && t.length_mm.is_none_or(ok),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn a4(
    name: &str,
    cols: usize,
    rows: usize,
    w: f64,
    h: f64,
    left: f64,
    top: f64,
    gx: f64,
    gy: f64,
) -> LabelStock {
    LabelStock::Sheet(SheetGrid {
        name: name.to_owned(),
        page_w: 210.0,
        page_h: 297.0,
        cols,
        rows,
        label_w: w,
        label_h: h,
        left,
        top,
        pitch_x: w + gx,
        pitch_y: h + gy,
    })
}

/// The stocks offered: common A4 sticker sheets and tape widths of the usual
/// tape label printers.
pub fn stock_presets() -> Vec<LabelStock> {
    let mut out = vec![
        a4("A4 3×8, 70 × 37 mm", 3, 8, 70.0, 37.0, 0.0, 0.5, 0.0, 0.0),
        a4(
            "A4 3×7, 63.5 × 38.1 mm",
            3,
            7,
            63.5,
            38.1,
            7.2,
            15.15,
            2.5,
            0.0,
        ),
        a4(
            "A4 2×7, 99.1 × 38.1 mm",
            2,
            7,
            99.1,
            38.1,
            4.65,
            15.15,
            2.54,
            0.0,
        ),
        a4(
            "A4 5×13, 38.1 × 21.2 mm",
            5,
            13,
            38.1,
            21.2,
            4.65,
            10.7,
            2.54,
            0.0,
        ),
        a4(
            "A4 7×27, 25.4 × 10 mm",
            7,
            27,
            25.4,
            10.0,
            8.5,
            13.5,
            2.54,
            0.0,
        ),
    ];
    out.extend([6.0, 9.0, 12.0, 18.0, 24.0].map(|tape_mm| {
        LabelStock::Tape(TapeStock {
            tape_mm,
            length_mm: None,
        })
    }));
    out
}

/// How labels are laid out and which are printed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSettings {
    pub stock: LabelStock,
    /// Kinds to print, from [`Labels::KINDS`](crate::Labels::KINDS) (all when
    /// empty).
    pub kinds: Vec<String>,
    /// Copies of each label (a flag on both ends needs two of a kind).
    pub copies: usize,
    /// First free label on the first sheet, 1-based: a partly used sheet
    /// continues where it ended.
    pub start_slot: usize,
    /// Draw a hairline around every label. Off for real stickers; on for a
    /// test print on plain paper.
    pub outlines: bool,
    /// Shift of everything on the sheet, mm: corrects a printer that prints
    /// off the sticker grid.
    pub offset_x: f64,
    pub offset_y: f64,
}

impl Default for LabelSettings {
    fn default() -> Self {
        LabelSettings {
            stock: stock_presets().swap_remove(3),
            kinds: vec!["wires".into()],
            copies: 1,
            start_slot: 1,
            outlines: false,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}

impl LabelSettings {
    /// The settings stored in `project`, else the defaults.
    pub fn of(project: &Project) -> LabelSettings {
        match project.info().properties.get(SETTINGS_KEY) {
            Some(Value::Text(text)) => serde_json::from_str::<LabelSettings>(text)
                .ok()
                .filter(LabelSettings::is_valid)
                .unwrap_or_default(),
            _ => LabelSettings::default(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.stock.is_valid()
            && (1..=99).contains(&self.copies)
            && self.start_slot >= 1
            && self.offset_x.is_finite()
            && self.offset_y.is_finite()
    }

    /// Stores the settings; the defaults remove the property.
    pub fn store(tx: &mut novo_core::Tx<'_>, settings: &LabelSettings) -> novo_core::Result<()> {
        let text = (*settings != LabelSettings::default())
            .then(|| serde_json::to_string(settings).ok())
            .flatten();
        tx.set_info(|info| match text {
            Some(text) => {
                info.properties
                    .insert(SETTINGS_KEY.to_owned(), Value::Text(text));
            }
            None => {
                info.properties.remove(SETTINGS_KEY);
            }
        })
    }
}

/// One label to print: its kind and text lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelItem {
    /// `wires`, `terminals`, `devices` or `cables`.
    pub kind: String,
    /// Up to [`LABEL_LINES`](crate::LABEL_LINES) lines, none empty.
    pub lines: Vec<String>,
}

/// The labels of `output` (the [`Labels`](crate::Labels) report) in the
/// given kinds (all when empty), in report order.
pub fn label_items(output: &ReportOutput, kinds: &[String]) -> Vec<LabelItem> {
    output
        .sections
        .iter()
        .filter(|s| kinds.is_empty() || kinds.contains(&s.key))
        .flat_map(|s| {
            s.rows.iter().filter_map(|row| {
                let lines: Vec<String> = row
                    .cells
                    .iter()
                    .map(ToString::to_string)
                    .filter(|t| !t.trim().is_empty())
                    .collect();
                (!lines.is_empty()).then(|| LabelItem {
                    kind: s.key.clone(),
                    lines,
                })
            })
        })
        .collect()
}

/// The laid-out labels.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LabelRun {
    /// One layout per sheet, or per label on tape.
    pub pages: Vec<PageLayout>,
    /// Labels printed (copies counted).
    pub labels: usize,
    /// Labels per sheet; 1 on tape.
    pub per_page: usize,
    /// Labels whose text had to be set smaller than [`MIN_TEXT_MM`].
    pub too_small: usize,
}

/// Relative height of line `i`: the first line (the wire number, the tag) is
/// the largest.
fn weight(i: usize) -> f64 {
    if i == 0 { 1.0 } else { 0.72 }
}

/// Average advance of a character in Helvetica, in em, used to keep text
/// inside its label. Capitals and digits are wider than lower case, so this
/// leans wide.
const ADVANCE_EM: f64 = 0.62;

/// Text boxes for `lines` centred in the label at `(x, y)`, size `w` × `h`.
/// Returns the boxes and the smallest character height used.
fn label_text(lines: &[String], x: f64, y: f64, w: f64, h: f64) -> (Vec<TextBox>, f64) {
    let pad = (h * 0.08).clamp(0.4, 1.2);
    let inner_w = (w - 2.0 * pad).max(0.1);
    let inner_h = (h - 2.0 * pad).max(0.1);
    // A line of size s takes 1.25 s of height.
    let total: f64 = (0..lines.len()).map(weight).sum();
    let unit = inner_h / (1.25 * total);
    let mut boxes = Vec::new();
    let mut smallest = f64::INFINITY;
    let mut at = y + pad;
    for (i, line) in lines.iter().enumerate() {
        let by_height = unit * weight(i);
        let chars = line.chars().count().max(1) as f64;
        let by_width = inner_w / (ADVANCE_EM * chars);
        let size = by_height.min(by_width).min(6.0);
        smallest = smallest.min(size);
        let line_h = 1.25 * unit * weight(i);
        boxes.push(TextBox {
            x: x + pad,
            y: at,
            width: inner_w,
            height: line_h,
            text: line.clone(),
            text_mm: size,
            align: Align::Center,
            bold: i == 0,
        });
        at += line_h;
    }
    (boxes, smallest)
}

fn outline(x: f64, y: f64, w: f64, h: f64) -> [Line; 4] {
    let l = |from: (f64, f64), to: (f64, f64)| Line {
        from,
        to,
        heavy: false,
    };
    [
        l((x, y), (x + w, y)),
        l((x + w, y), (x + w, y + h)),
        l((x + w, y + h), (x, y + h)),
        l((x, y + h), (x, y)),
    ]
}

/// Lays `items` out on the stock of `settings`, `copies` of each in order.
/// An invalid stock gives an empty run.
pub fn layout_labels(items: &[LabelItem], settings: &LabelSettings) -> LabelRun {
    if !settings.stock.is_valid() || items.is_empty() {
        return LabelRun::default();
    }
    let copies = settings.copies.max(1);
    let all: Vec<&LabelItem> = items
        .iter()
        .flat_map(|i| std::iter::repeat_n(i, copies))
        .collect();
    let mut run = LabelRun {
        labels: all.len(),
        ..LabelRun::default()
    };
    let (dx, dy) = (settings.offset_x, settings.offset_y);
    match &settings.stock {
        LabelStock::Sheet(g) => {
            let per = g.cols * g.rows;
            run.per_page = per;
            let skip = (settings.start_slot - 1).min(per - 1);
            let mut slot = skip;
            let mut page: Option<PageLayout> = None;
            for item in all {
                if page.is_none() {
                    page = Some(PageLayout {
                        width: g.page_w,
                        height: g.page_h,
                        ..PageLayout::default()
                    });
                }
                let p = page.as_mut().expect("page just created");
                let (col, row) = (slot % g.cols, slot / g.cols);
                let x = g.left + col as f64 * g.pitch_x + dx;
                let y = g.top + row as f64 * g.pitch_y + dy;
                let (boxes, smallest) = label_text(&item.lines, x, y, g.label_w, g.label_h);
                if smallest < MIN_TEXT_MM {
                    run.too_small += 1;
                }
                p.texts.extend(boxes);
                if settings.outlines {
                    p.lines.extend(outline(x, y, g.label_w, g.label_h));
                }
                slot += 1;
                if slot == per {
                    run.pages.extend(page.take());
                    slot = 0;
                }
            }
            run.pages.extend(page);
            // The skipped slots count on the first sheet, not as labels.
            if settings.outlines
                && skip > 0
                && let Some(first) = run.pages.first_mut()
            {
                for s in 0..skip {
                    let (col, row) = (s % g.cols, s / g.cols);
                    first.lines.extend(outline(
                        g.left + col as f64 * g.pitch_x + dx,
                        g.top + row as f64 * g.pitch_y + dy,
                        g.label_w,
                        g.label_h,
                    ));
                }
            }
        }
        LabelStock::Tape(t) => {
            run.per_page = 1;
            for item in all {
                let h = t.tape_mm;
                let length = t.length_mm.unwrap_or_else(|| {
                    // Size to the longest line at the size the tape height
                    // gives, rounded up to 5 mm, never under 15 mm.
                    let pad = (h * 0.08).clamp(0.4, 1.2);
                    let total: f64 = (0..item.lines.len()).map(weight).sum();
                    let unit = (h - 2.0 * pad) / (1.25 * total);
                    let need = item
                        .lines
                        .iter()
                        .enumerate()
                        .map(|(i, l)| {
                            ADVANCE_EM * (unit * weight(i)).min(6.0) * l.chars().count() as f64
                        })
                        .fold(0.0, f64::max);
                    (((need + 2.0 * pad + 3.0) / 5.0).ceil() * 5.0).max(15.0)
                });
                let (boxes, smallest) = label_text(&item.lines, 0.0, 0.0, length, h);
                if smallest < MIN_TEXT_MM {
                    run.too_small += 1;
                }
                let mut page = PageLayout {
                    width: length,
                    height: h,
                    texts: boxes,
                    lines: Vec::new(),
                };
                if settings.outlines {
                    page.lines.extend(outline(0.0, 0.0, length, h));
                }
                run.pages.push(page);
            }
        }
    }
    run
}
