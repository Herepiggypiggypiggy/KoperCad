//! A made-up thermal result: devices that heat the air around them, a plume
//! above each, a fan and an outlet. It drives the pictures before the solver
//! can, in the demos and in every test. Not physics, only a believable shape.

use crate::field::{Grid, Scalar, Vector};

#[derive(Debug, Clone, PartialEq)]
pub struct SynthDevice {
    pub id: u64,
    /// Rectangle, mm, `y` down.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub loss_w: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SynthScene {
    pub width: f64,
    pub height: f64,
    pub ambient: f64,
    pub devices: Vec<SynthDevice>,
}

/// A wall-mounted plate with a drive, a supply and a few small devices.
pub fn demo_scene() -> SynthScene {
    let d = |id, x, y, w, h, loss_w| SynthDevice {
        id,
        x,
        y,
        w,
        h,
        loss_w,
    };
    SynthScene {
        width: 760.0,
        height: 520.0,
        ambient: 35.0,
        devices: vec![
            d(1, 30.0, 30.0, 150.0, 110.0, 28.0),
            d(2, 230.0, 30.0, 170.0, 240.0, 120.0),
            d(3, 440.0, 30.0, 45.0, 90.0, 3.0),
            d(4, 500.0, 30.0, 45.0, 90.0, 6.0),
            d(5, 630.0, 30.0, 90.0, 110.0, 18.0),
            d(6, 440.0, 160.0, 160.0, 110.0, 14.0),
        ],
    }
}

impl SynthScene {
    pub fn grid(&self, cell_mm: f64) -> Grid {
        Grid {
            cols: (self.width / cell_mm).round().max(2.0) as usize,
            rows: (self.height / cell_mm).round().max(2.0) as usize,
            cell_w: self.width / (self.width / cell_mm).round().max(2.0),
            cell_h: self.height / (self.height / cell_mm).round().max(2.0),
        }
    }

    /// True inside a device body.
    pub fn solid(&self, x: f64, y: f64) -> bool {
        self.devices
            .iter()
            .any(|d| x > d.x && x < d.x + d.w && y > d.y && y < d.y + d.h)
    }

    /// Air temperature at a point, degrees C. Heat rises (towards smaller
    /// `y`), gathers under the top and is cooled near the inlet.
    pub fn temperature(&self, x: f64, y: f64) -> f64 {
        let up = 1.0 - y / self.height; // 1 at the top
        let mut t = self.ambient + 4.0 + 7.0 * up.powf(1.4);
        for d in &self.devices {
            let (cx, cy) = (d.x + d.w / 2.0, d.y + d.h / 2.0);
            let s = 55.0 + d.w.max(d.h) * 0.35;
            // The effect centre sits above the device: heat is carried up.
            let (dx, dy) = (x - cx, y - (cy - 40.0));
            t += d.loss_w
                * 0.21
                * (-(dx * dx / (2.0 * s * s) + dy * dy / (2.0 * (s * 1.5).powi(2)))).exp();
        }
        let (ix, iy) = (x, y - self.height); // inlet bottom left
        t -= 9.0 * (-((ix * ix + iy * iy) / (2.0 * 150.0 * 150.0))).exp();
        t
    }

    /// Air velocity at a point, m/s (y down: up is negative).
    pub fn velocity(&self, x: f64, y: f64) -> (f64, f64) {
        let (mut vx, mut vy) = (0.0, 0.0);
        // Potential flow from an inlet at the bottom left to an outlet at the top right.
        for (sx, sy, q) in [
            (-40.0, self.height + 20.0, 1.0),
            (self.width + 40.0, -20.0, -1.0),
        ] {
            let (dx, dy) = (x - sx, y - sy);
            let r2 = dx * dx + dy * dy + 900.0;
            vx += q * dx / r2 * 1.0;
            vy += q * dy / r2 * 1.0;
        }
        for d in &self.devices {
            let (cx, top) = (d.x + d.w / 2.0, d.y);
            let (dx, dy) = (x - cx, y - top);
            let above = (-dy).max(0.0); // distance above the device top
            let g =
                (-(dx * dx) / (2.0 * (d.w * 0.6 + 20.0).powi(2))).exp() * (-above / 220.0).exp();
            if dy < 0.0 {
                vy -= d.loss_w * 0.0005 * g;
            }
        }
        (vx, vy)
    }

    /// Temperature on a grid; device bodies are solid (`NaN`).
    pub fn temperature_field(&self, cell_mm: f64) -> Scalar {
        let grid = self.grid(cell_mm);
        let mut values = Vec::with_capacity(grid.cols * grid.rows);
        for r in 0..grid.rows {
            for c in 0..grid.cols {
                let (x, y) = grid.centre(c, r);
                values.push(if self.solid(x, y) {
                    f32::NAN
                } else {
                    self.temperature(x, y) as f32
                });
            }
        }
        Scalar::new(grid, values)
    }

    pub fn velocity_field(&self, cell_mm: f64) -> Vector {
        let grid = self.grid(cell_mm);
        let mut vx = Vec::new();
        let mut vy = Vec::new();
        for r in 0..grid.rows {
            for c in 0..grid.cols {
                let (x, y) = grid.centre(c, r);
                let (u, v) = self.velocity(x, y);
                vx.push(u as f32);
                vy.push(v as f32);
            }
        }
        Vector::new(grid, vx, vy)
    }

    /// Cell sizes of a progressive solve, coarse to fine, mm.
    pub const LEVELS_MM: [f64; 4] = [40.0, 20.0, 10.0, 5.0];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{contour, hotspot, range};

    #[test]
    fn the_drive_is_the_hottest_place() {
        let s = demo_scene();
        let f = s.temperature_field(10.0);
        let p = hotspot::peaks(&f.filled(), 1, 60.0, 2.0);
        let drive = &s.devices[1];
        assert!(
            p[0].x > drive.x - 60.0 && p[0].x < drive.x + drive.w + 60.0,
            "{:?}",
            p[0]
        );
        assert!(
            p[0].y < drive.y + drive.h,
            "heat gathers on or above the device"
        );
    }

    #[test]
    fn every_level_has_the_same_shape_in_a_coarser_grid() {
        let s = demo_scene();
        let fine = s.temperature_field(5.0).extent().unwrap();
        let coarse = s.temperature_field(40.0).extent().unwrap();
        assert!((fine.1 - coarse.1).abs() < 6.0, "{fine:?} {coarse:?}");
        assert!(SynthScene::LEVELS_MM.windows(2).all(|w| w[0] > w[1]));
    }

    #[test]
    fn solids_are_masked_and_air_is_warm_enough() {
        let s = demo_scene();
        let f = s.temperature_field(10.0);
        let (c, r) = f.grid.cell_at(300.0, 150.0);
        assert!(f.at(c, r).is_nan());
        let (lo, hi) = f.extent().unwrap();
        assert!(lo > 25.0 && hi < 100.0, "{lo} {hi}");
    }

    #[test]
    fn contours_exist_at_round_levels() {
        let s = demo_scene();
        let f = s.temperature_field(10.0).filled();
        let (lo, hi) = f.extent().unwrap();
        let levels = range::contour_levels(
            range::Range {
                lo: lo as f64,
                hi: hi as f64,
            },
            8,
        );
        assert!(!contour::contours(&f, &levels.iter().map(|l| l.0).collect::<Vec<_>>()).is_empty());
    }

    #[test]
    fn air_rises_above_the_drive() {
        let s = demo_scene();
        let (_, vy) = s.velocity(315.0, 10.0);
        assert!(vy < 0.0);
    }
}
