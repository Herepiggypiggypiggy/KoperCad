//! A quick thermal picture from the part losses alone: devices that heat the
//! air around them, a plume above each, an inlet and an outlet. It drives the
//! pictures before the solver can, in the demos and in every test. A
//! calibrated estimate of the device and air temperatures (see
//! [`SynthScene::temperature`]); the airflow is only an indication of where
//! warm air rises.

use crate::field::{Grid, Scalar, Vector};

/// Air of the cabinet: kelvin per watt of total loss, on the lesson cabinet
/// (600 x 800 mm plate, 34.5 W gives about 6 K over the room).
const AIR_K_PER_W: f64 = 0.17;
/// Share of the rises of the other devices that adds to the strongest one at a point.
const NEIGHBOUR_SHARE: f64 = 0.2;
/// Surface conductance of a device to its air and the plate, W/K: a fixed
/// part (the connections and the plate) and a part per square metre of the
/// footprint. Fitted to the 2D model: 20 W on 90 x 110 mm runs about 38 K
/// over its air, 1.5 W on 18 x 90 mm about 10 K.
const SURFACE_G0_W_PER_K: f64 = 0.07;
const SURFACE_G_W_PER_K_M2: f64 = 47.0;

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

    /// Temperature at a point, degrees C: the air of the cabinet plus, around
    /// every device, the rise its own loss makes there.
    ///
    /// - Air: `AIR_K_PER_W` kelvin for every watt of loss in the cabinet,
    ///   a little cooler low on the plate and warmer high up (heat rises).
    /// - Device: its loss over its own surface conductance, `loss / (G0 + G_A
    ///   area)`, flat over the device and fading with the distance from it,
    ///   further up than down. Where devices stand close the strongest rise
    ///   counts in full and the others at a fifth.
    ///
    /// Everything scales with the loss alone, so a 20 W device is never
    /// cooler than a 3 W device of the same size, and several small devices
    /// side by side add up. The constants are fitted to the 2D thermal model
    /// of `novo-thermal` on the lesson cabinet (the test
    /// `heat_preview_tests` in `novo-app` keeps them honest); they give a
    /// ranking and a rough size, not an exact temperature.
    pub fn temperature(&self, x: f64, y: f64) -> f64 {
        let up = 1.0 - y / self.height; // 1 at the top
        let total: f64 = self.devices.iter().map(|d| d.loss_w.max(0.0)).sum();
        let (mut hottest, mut sum) = (0.0_f64, 0.0_f64);
        for d in &self.devices {
            let r = self.device_rise(d) * Self::reach(d, x, y);
            hottest = hottest.max(r);
            sum += r;
        }
        // A neighbour warms the air around a device, not the device as much as its own loss
        // does: the strongest rise counts in full, the others a fifth.
        self.ambient
            + AIR_K_PER_W * total * (0.8 + 0.4 * up)
            + hottest
            + NEIGHBOUR_SHARE * (sum - hottest)
    }

    /// Kelvin a device runs above the air around it, steady state.
    pub fn device_rise(&self, d: &SynthDevice) -> f64 {
        let area_m2 = d.w * d.h / 1.0e6;
        d.loss_w.max(0.0) / (SURFACE_G0_W_PER_K + SURFACE_G_W_PER_K_M2 * area_m2)
    }

    /// How much of a device's rise reaches a point: 1 on the device, fading
    /// with the distance, reaching further above it than below.
    fn reach(d: &SynthDevice, x: f64, y: f64) -> f64 {
        let l = 25.0 + 0.2 * d.w.max(d.h);
        let dx = (d.x - x).max(0.0).max(x - (d.x + d.w));
        let above = (d.y - y).max(0.0);
        let below = (y - (d.y + d.h)).max(0.0);
        let u2 = (dx / l).powi(2) + (above / (1.6 * l)).powi(2) + (below / (0.7 * l)).powi(2);
        (-u2).exp()
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
        assert!(lo > 25.0 && hi < 140.0, "{lo} {hi}");
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

    fn one(loss_w: f64, w: f64, h: f64, x: f64, y: f64) -> SynthDevice {
        SynthDevice {
            id: 0,
            x,
            y,
            w,
            h,
            loss_w,
        }
    }

    fn hottest(devices: Vec<SynthDevice>) -> f64 {
        let s = SynthScene {
            width: 600.0,
            height: 800.0,
            ambient: 35.0,
            devices,
        };
        f64::from(s.temperature_field(10.0).extent().unwrap().1)
    }

    #[test]
    fn more_loss_is_never_cooler_on_the_same_footprint() {
        let at = |w| hottest(vec![one(w, 90.0, 110.0, 200.0, 300.0)]);
        assert!(at(1.5) < at(3.0) && at(3.0) < at(6.0) && at(6.0) < at(20.0));
    }

    #[test]
    fn one_twenty_watt_device_beats_small_devices_standing_close() {
        // The case behind the lesson: 20 W on a transformer against 6 + 4 + 1.5 W of small
        // devices side by side, the small ones high on the plate.
        let transformer = hottest(vec![one(20.0, 90.0, 110.0, 55.0, 550.0)]);
        let small = hottest(vec![
            one(6.0, 54.0, 90.0, 55.0, 10.0),
            one(4.0, 45.0, 75.0, 119.0, 18.0),
            one(1.5, 18.0, 90.0, 174.0, 10.0),
        ]);
        assert!(transformer > small + 10.0, "{transformer} against {small}");
    }

    #[test]
    fn neighbours_add_up() {
        let alone = hottest(vec![one(6.0, 54.0, 90.0, 100.0, 100.0)]);
        let pair = hottest(vec![
            one(6.0, 54.0, 90.0, 100.0, 100.0),
            one(6.0, 54.0, 90.0, 160.0, 100.0),
        ]);
        assert!(pair > alone + 1.0, "{pair} against {alone}");
    }

    #[test]
    fn a_device_does_not_cool_the_cabinet_by_standing_low() {
        // Where a lone device stands changes its own temperature by a few kelvin, never by more
        // than the part it adds itself (the old picture cooled a device by 9 K at the inlet).
        let low = hottest(vec![one(20.0, 90.0, 110.0, 50.0, 650.0)]);
        let high = hottest(vec![one(20.0, 90.0, 110.0, 50.0, 20.0)]);
        assert!((high - low).abs() < 6.0, "{low} {high}");
        assert!(high > low, "heat rises: high {high}, low {low}");
    }

    #[test]
    fn a_cabinet_without_loss_is_at_ambient() {
        let s = SynthScene {
            width: 600.0,
            height: 800.0,
            ambient: 35.0,
            devices: vec![one(0.0, 90.0, 110.0, 50.0, 50.0)],
        };
        let (lo, hi) = s.temperature_field(10.0).extent().unwrap();
        assert!(
            (lo - 35.0).abs() < 1e-3 && (hi - 35.0).abs() < 1e-3,
            "{lo} {hi}"
        );
    }

    #[test]
    fn air_rises_above_the_drive() {
        let s = demo_scene();
        let (_, vy) = s.velocity(315.0, 10.0);
        assert!(vy < 0.0);
    }
}
