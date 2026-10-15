//! The drawn terminal strip diagram: one block per terminal down the middle,
//! the cabinet side (wire number and destination) on the left, the field
//! side on the right, jumper bars beside the blocks and end plates at both
//! ends of the strip.
//!
//! The result is a [`PageLayout`] (text boxes and rules in mm), so the
//! canvas and the PDF export draw it exactly like a report table. It reads
//! the same [`novo_terminals::screen`] rows as the terminal diagram report,
//! so the drawing and the table never disagree.

use novo_core::{DeviceId, Project};
use novo_terminals::{Screen, ScreenRow};

use crate::page::{Line, PageLayout, PageSetup, TextBox};
use crate::table::Align;

/// Height of one terminal block, mm.
const ROW_MM: f64 = 6.0;
/// Block width, mm.
const BLOCK_MM: f64 = 14.0;
/// Space between two jumper bars, mm.
const LANE_MM: f64 = 2.5;
/// Thickness of an end plate, mm.
const PLATE_MM: f64 = 1.2;
/// Gap between the header rule and the first end plate, mm.
const GAP_MM: f64 = 0.8;

/// Terminals per sheet for `setup`: the same as the table's rows per page
/// (a block is as high as a table row; the end plates sit in the table's
/// bottom gap), so the drawn sheets and the paginated report agree.
pub fn terminals_per_sheet(setup: &PageSetup) -> usize {
    setup.rows_per_page()
}

/// Number of sheets the strip needs.
pub fn sheet_count(screen: &Screen, setup: &PageSetup) -> usize {
    screen
        .rows
        .len()
        .div_ceil(terminals_per_sheet(setup))
        .max(1)
}

/// Draws a strip: one layout per sheet.
pub fn strip_diagram(project: &Project, strip: DeviceId, setup: &PageSetup) -> Vec<PageLayout> {
    let screen = novo_terminals::screen(project, strip);
    let per_sheet = terminals_per_sheet(setup);
    let count = sheet_count(&screen, setup);
    (0..count)
        .map(|n| {
            let rows = screen.rows.chunks(per_sheet).nth(n).unwrap_or(&[]);
            sheet(&screen, rows, (n, count), setup)
        })
        .collect()
}

/// Jumper groups on a sheet, in order of first appearance, mapped to lanes.
fn lanes(rows: &[ScreenRow]) -> Vec<u32> {
    let mut groups = Vec::new();
    for row in rows {
        if let Some(group) = row.jumper
            && !groups.contains(&group)
        {
            groups.push(group);
        }
    }
    groups
}

/// One sheet of a strip: the terminals `rows` of the strip, as the
/// `number`th (0-based) of `number.1` sheets. This is what a generated
/// sheet of the terminal diagram prints.
pub fn strip_sheet(
    project: &Project,
    strip: DeviceId,
    rows: std::ops::Range<usize>,
    number: (usize, usize),
    setup: &PageSetup,
) -> PageLayout {
    let screen = novo_terminals::screen(project, strip);
    let end = rows.end.min(screen.rows.len());
    let start = rows.start.min(end);
    sheet(&screen, &screen.rows[start..end], number, setup)
}

fn sheet(
    screen: &Screen,
    rows: &[ScreenRow],
    (index, count): (usize, usize),
    setup: &PageSetup,
) -> PageLayout {
    let (width, height) = setup.format.size_mm();
    let mut layout = PageLayout {
        width,
        height,
        ..PageLayout::default()
    };
    let (x0, y0, w, _) = setup.table_area();
    let text = |x: f64, y: f64, w: f64, h: f64, text: &str, size: f64, align, bold| TextBox {
        x,
        y,
        width: w,
        height: h,
        text: text.to_owned(),
        text_mm: size,
        align,
        bold,
    };
    let rule = |from: (f64, f64), to: (f64, f64), heavy| Line { from, to, heavy };

    layout.texts.push(text(
        x0,
        setup.margins.top,
        w * 0.75,
        setup.heading_mm,
        &format!("Terminal strip · {}", screen.tag),
        setup.text_mm * 1.4,
        Align::Left,
        true,
    ));
    layout.texts.push(text(
        x0 + w * 0.75,
        setup.margins.top,
        w * 0.25,
        setup.heading_mm,
        &format!("page {} / {}", index + 1, count),
        setup.text_mm,
        Align::Right,
        false,
    ));

    // Columns: cabinet destination, wire in, [lanes + block], wire out,
    // field destination, cable, location.
    let wire_w = w * 0.07;
    let dest_w = w * 0.22;
    let cable_w = w * 0.14;
    let place_w = w * 0.08;
    let middle = w - 2.0 * dest_w - 2.0 * wire_w - cable_w - place_w;
    let lanes = lanes(rows);
    let lane_space = (lanes.len() as f64 * LANE_MM)
        .min(middle - BLOCK_MM)
        .max(0.0);
    // Bars are LANE_MM apart, squeezed together when there are too many
    // groups for the room beside the blocks.
    let lane_pitch = if lanes.is_empty() {
        LANE_MM
    } else {
        lane_space / lanes.len() as f64
    };
    let block_x = x0 + dest_w + wire_w + (middle - BLOCK_MM + lane_space) / 2.0;
    let mid_start = x0 + dest_w + wire_w;
    let out_x = mid_start + middle;
    let ext_x = out_x + wire_w;
    let cable_x = ext_x + dest_w;
    let place_x = cable_x + cable_w;

    let header = |x: f64, w: f64, label: &str, align| {
        text(
            x + 1.0,
            y0,
            w - 2.0,
            setup.header_row_mm,
            label,
            setup.text_mm,
            align,
            true,
        )
    };
    layout
        .texts
        .push(header(x0, dest_w, "Internal", Align::Left));
    layout
        .texts
        .push(header(x0 + dest_w, wire_w, "Wire", Align::Left));
    layout
        .texts
        .push(header(mid_start, middle, "Terminal", Align::Center));
    layout
        .texts
        .push(header(out_x, wire_w, "Wire", Align::Left));
    layout
        .texts
        .push(header(ext_x, dest_w, "External", Align::Left));
    layout
        .texts
        .push(header(cable_x, cable_w, "Cable", Align::Left));
    layout
        .texts
        .push(header(place_x, place_w, "Page", Align::Left));
    let top = y0 + setup.header_row_mm;
    layout.lines.push(rule((x0, top), (x0 + w, top), true));

    // End plate above the first block.
    let strip_h = rows.len() as f64 * ROW_MM;
    let plate = |layout: &mut PageLayout, y: f64| {
        let (l, r) = (block_x, block_x + BLOCK_MM);
        layout.lines.extend([
            rule((l, y), (r, y), true),
            rule((l, y + PLATE_MM), (r, y + PLATE_MM), true),
            rule((l, y), (l, y + PLATE_MM), true),
            rule((r, y), (r, y + PLATE_MM), true),
        ]);
    };
    plate(&mut layout, top + GAP_MM);
    let first = top + GAP_MM + PLATE_MM;
    plate(&mut layout, first + strip_h);

    let lane_x = |group: u32| {
        let lane = lanes.iter().position(|&g| g == group).unwrap_or(0);
        block_x - lane_pitch * (lane as f64 + 1.0)
    };
    for (i, row) in rows.iter().enumerate() {
        let y = first + i as f64 * ROW_MM;
        let (l, r) = (block_x, block_x + BLOCK_MM);
        layout.lines.extend([
            rule((l, y), (r, y), false),
            rule((l, y + ROW_MM), (r, y + ROW_MM), false),
            rule((l, y), (l, y + ROW_MM), true),
            rule((r, y), (r, y + ROW_MM), true),
        ]);
        let label = if row.spare {
            format!("{} (spare)", row.terminal)
        } else {
            row.terminal.clone()
        };
        let small = setup.text_mm;
        let mut put = |x: f64, w: f64, s: &str, align, bold| {
            if !s.is_empty() {
                layout
                    .texts
                    .push(text(x + 1.0, y, w - 2.0, ROW_MM, s, small, align, bold));
            }
        };
        put(block_x, BLOCK_MM, &label, Align::Center, true);
        put(x0, dest_w, &row.internal.join(", "), Align::Left, false);
        put(
            x0 + dest_w,
            wire_w,
            &row.wires_internal.join(", "),
            Align::Left,
            false,
        );
        put(
            out_x,
            wire_w,
            &row.wires_external.join(", "),
            Align::Left,
            false,
        );
        put(ext_x, dest_w, &row.external.join(", "), Align::Left, false);
        put(cable_x, cable_w, &row.cables.join(", "), Align::Left, false);
        put(place_x, place_w, &row.pages.join(", "), Align::Left, false);
    }

    // Jumper bars: a vertical bar per group from its first to its last
    // terminal on this sheet, a tick from the bar to each member's block.
    for &group in &lanes {
        let members: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.jumper == Some(group))
            .map(|(i, _)| i)
            .collect();
        let x = lane_x(group);
        let centre = |i: usize| first + (i as f64 + 0.5) * ROW_MM;
        if let (Some(&a), Some(&b)) = (members.first(), members.last()) {
            layout
                .lines
                .push(rule((x, centre(a)), (x, centre(b)), true));
        }
        for &i in &members {
            layout
                .lines
                .push(rule((x, centre(i)), (block_x, centre(i)), true));
        }
    }
    layout
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, jumper: u32) -> ScreenRow {
        ScreenRow {
            internal: Vec::new(),
            terminal: name.into(),
            kind: String::new(),
            external: Vec::new(),
            cables: Vec::new(),
            jumper: Some(jumper),
            bridged_with: Vec::new(),
            wires_internal: Vec::new(),
            wires_external: Vec::new(),
            pages: Vec::new(),
            build: String::new(),
            spare: false,
        }
    }

    #[test]
    fn many_jumper_groups_stay_beside_the_blocks() {
        let rows: Vec<ScreenRow> = (0..30).map(|i| row(&i.to_string(), i)).collect();
        let screen = Screen {
            strip: DeviceId(1),
            tag: "-X1".into(),
            rows: rows.clone(),
        };
        let mut format = novo_core::PageFormat::default();
        format.size = novo_core::PaperSize::A4;
        let setup = PageSetup::for_format(format);
        let layout = sheet(&screen, &rows[..20], (0, 1), &setup);
        let (x0, _, w, _) = setup.table_area();
        // The wire-in column ends where the lanes may start.
        let lanes_start = x0 + w * 0.22 + w * 0.07;
        for line in layout.lines.iter().filter(|l| l.from.0 == l.to.0) {
            assert!(line.from.0 >= lanes_start - 1e-9, "{line:?}");
        }
    }
}
