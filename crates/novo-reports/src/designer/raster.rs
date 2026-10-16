//! A tiny rasteriser for [`PageLayout`]s, for pixel-diff tests of frames
//! and templates. Lines are drawn exactly (Bresenham, heavy lines two
//! pixels wide); a text box is drawn as the solid block its characters
//! would fill, so the diff catches moved, resized or missing text without
//! depending on a font.

use crate::page::PageLayout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    pub width: usize,
    pub height: usize,
    bits: Vec<bool>,
}

impl Raster {
    pub fn blank(width: usize, height: usize) -> Self {
        Raster {
            width,
            height,
            bits: vec![false; width * height],
        }
    }

    /// From rows of text: `#` is ink, anything else paper. For golden
    /// pictures written into tests.
    pub fn from_rows(rows: &[&str]) -> Self {
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        let mut r = Raster::blank(width, rows.len());
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                r.set(x as i64, y as i64, c == '#');
            }
        }
        r
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height && self.bits[y * self.width + x]
    }

    fn set(&mut self, x: i64, y: i64, on: bool) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.bits[y as usize * self.width + x as usize] = on;
        }
    }

    pub fn ink(&self) -> usize {
        self.bits.iter().filter(|b| **b).count()
    }

    /// The picture as `#` and `.` rows.
    pub fn to_ascii(&self) -> String {
        (0..self.height)
            .map(|y| {
                (0..self.width)
                    .map(|x| if self.get(x, y) { '#' } else { '.' })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Draws a layout at `px_per_mm`.
    pub fn from_layout(layout: &PageLayout, px_per_mm: f64) -> Self {
        let mut r = Raster::blank(
            (layout.width * px_per_mm).round() as usize,
            (layout.height * px_per_mm).round() as usize,
        );
        let px = |v: f64| (v * px_per_mm).round() as i64;
        for l in &layout.lines {
            let (x0, y0, x1, y1) = (px(l.from.0), px(l.from.1), px(l.to.0), px(l.to.1));
            r.line(x0, y0, x1, y1);
            if l.heavy {
                // One more pixel across the line.
                if (x1 - x0).abs() >= (y1 - y0).abs() {
                    r.line(x0, y0 + 1, x1, y1 + 1);
                } else {
                    r.line(x0 + 1, y0, x1 + 1, y1);
                }
            }
        }
        for t in &layout.texts {
            let run = (t.text.chars().count() as f64 * t.text_mm * 0.5).min(t.width);
            let x = match t.align {
                crate::Align::Left => t.x,
                crate::Align::Center => t.x + (t.width - run) / 2.0,
                crate::Align::Right => t.x + t.width - run,
            };
            let y = t.y + (t.height - t.text_mm) / 2.0;
            for yy in px(y)..px(y + t.text_mm) {
                for xx in px(x)..px(x + run) {
                    r.set(xx, yy, true);
                }
            }
        }
        r
    }

    fn line(&mut self, x0: i64, y0: i64, x1: i64, y1: i64) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.set(x, y, true);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Pixels that differ between two pictures of the same size; `None` if
    /// the sizes differ.
    pub fn diff(&self, other: &Raster) -> Option<usize> {
        (self.width == other.width && self.height == other.height).then(|| {
            self.bits
                .iter()
                .zip(&other.bits)
                .filter(|(a, b)| a != b)
                .count()
        })
    }
}
