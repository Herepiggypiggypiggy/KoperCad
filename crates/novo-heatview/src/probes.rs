//! Numbers from a solved cabinet: a plot along a line and volume figures for
//! the air. Pure functions on a [`CfdSnapshot`], so the Heat strip, the
//! reports and the tests share them.
//!
//! Frame: millimetres, `y` down, as in [`crate::cfd`]. Air means every cell
//! that is not a solid; a slotted wire duct counts as air, like in the
//! slices. These are readings of a model, not measurements.

use novo_cfd::CfdSnapshot;

use crate::cfd;

/// One sample along a line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Distance from the start of the line, mm.
    pub along_mm: f64,
    pub temperature_c: f64,
    /// Air speed, m/s.
    pub speed: f64,
    /// The sample lies in a solid (device body or wall), where the value is
    /// the solid's own temperature.
    pub solid: bool,
}

/// The values along a line between two points.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub from: [f64; 3],
    pub to: [f64; 3],
    pub samples: Vec<Sample>,
}

/// What a line plot says in numbers, over its air samples only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProfileStats {
    pub min_c: f64,
    pub max_c: f64,
    pub mean_c: f64,
    pub max_speed: f64,
    /// Share of the line that runs through solids, 0..1.
    pub solid_share: f64,
}

/// Fewest and most samples on a line.
pub const SAMPLES: std::ops::RangeInclusive<usize> = 2..=2000;

/// Samples the fields at `count` evenly spaced points from `from` to `to`
/// (both included). `None` for an inconsistent result, a line with no
/// length, a count outside [`SAMPLES`] or a line that leaves the cabinet.
pub fn line_profile(
    snap: &CfdSnapshot,
    from: [f64; 3],
    to: [f64; 3],
    count: usize,
) -> Option<Profile> {
    if !snap.is_consistent() || !SAMPLES.contains(&count) {
        return None;
    }
    if !cfd::inside(snap, from) || !cfd::inside(snap, to) {
        return None;
    }
    let len = (0..3)
        .map(|i| (to[i] - from[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    if len < 1e-9 {
        return None;
    }
    let samples = (0..count)
        .map(|n| {
            let t = n as f64 / (count - 1) as f64;
            let p = [
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
                from[2] + (to[2] - from[2]) * t,
            ];
            let v = cfd::velocity_at(snap, p);
            Sample {
                along_mm: len * t,
                temperature_c: f64::from(cfd::temperature_at(snap, p)),
                speed: v.iter().map(|c| f64::from(*c).powi(2)).sum::<f64>().sqrt(),
                solid: cfd::solid_at(snap, p),
            }
        })
        .collect();
    Some(Profile { from, to, samples })
}

impl Profile {
    /// Length of the line, mm.
    pub fn length_mm(&self) -> f64 {
        self.samples.last().map_or(0.0, |s| s.along_mm)
    }

    /// Minimum, maximum and mean of the air samples; `None` when the whole
    /// line is inside solids.
    pub fn stats(&self) -> Option<ProfileStats> {
        let air: Vec<&Sample> = self.samples.iter().filter(|s| !s.solid).collect();
        if air.is_empty() {
            return None;
        }
        let n = air.len() as f64;
        Some(ProfileStats {
            min_c: air.iter().map(|s| s.temperature_c).fold(f64::MAX, f64::min),
            max_c: air.iter().map(|s| s.temperature_c).fold(f64::MIN, f64::max),
            mean_c: air.iter().map(|s| s.temperature_c).sum::<f64>() / n,
            max_speed: air.iter().map(|s| s.speed).fold(0.0, f64::max),
            solid_share: 1.0 - n / self.samples.len() as f64,
        })
    }

    /// Comma separated text for a spreadsheet: distance, temperature, speed,
    /// and whether the point is inside a solid.
    pub fn csv(&self) -> String {
        let mut out = String::from("distance_mm,temperature_c,speed_m_s,solid\n");
        for s in &self.samples {
            out.push_str(&format!(
                "{:.2},{:.3},{:.4},{}\n",
                s.along_mm,
                s.temperature_c,
                s.speed,
                u8::from(s.solid)
            ));
        }
        out
    }
}

/// A box in the solver frame, mm, to count the air of a part of the cabinet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub lo: [f64; 3],
    pub hi: [f64; 3],
}

/// Figures for the air of a result, or of a region of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirNumbers {
    /// Air in the region, litres.
    pub air_l: f64,
    /// Air hotter than the chosen level, litres.
    pub above_l: f64,
    /// Mean air temperature weighted by volume, degrees C.
    pub mean_c: f64,
    pub min_c: f64,
    pub max_c: f64,
}

impl AirNumbers {
    /// Share of the air above the level, 0..1.
    pub fn above_share(&self) -> f64 {
        if self.air_l > 0.0 {
            self.above_l / self.air_l
        } else {
            0.0
        }
    }
}

fn is_air(snap: &CfdSnapshot, id: u16) -> bool {
    id == 0
        || snap
            .materials
            .iter()
            .any(|m| m.id == id && m.kind == novo_cfd::MaterialKind::Duct)
}

/// Overlap of the cell `[a, b]` with `[lo, hi]`, as a fraction of the cell.
fn share(a: f64, b: f64, lo: f64, hi: f64) -> f64 {
    ((b.min(hi) - a.max(lo)).max(0.0)) / (b - a)
}

/// Volume of air hotter than `level_c` and the other figures, over the whole
/// result or over `region`. A cell cut by the region counts by its overlap.
/// `None` for an inconsistent result or when the region holds no air.
pub fn air_numbers(snap: &CfdSnapshot, level_c: f64, region: Option<Region>) -> Option<AirNumbers> {
    if !snap.is_consistent() {
        return None;
    }
    let g = &snap.grid;
    let (mut air, mut above, mut weighted) = (0.0, 0.0, 0.0);
    let (mut min_c, mut max_c) = (f64::MAX, f64::MIN);
    for k in 0..g.nz {
        let fz = region.map_or(1.0, |r| {
            share(g.z_edges_mm[k], g.z_edges_mm[k + 1], r.lo[2], r.hi[2])
        });
        if fz <= 0.0 {
            continue;
        }
        for j in 0..g.ny {
            let fy = region.map_or(1.0, |r| {
                share(g.y_edges_mm[j], g.y_edges_mm[j + 1], r.lo[1], r.hi[1])
            });
            if fy <= 0.0 {
                continue;
            }
            for i in 0..g.nx {
                let fx = region.map_or(1.0, |r| {
                    share(g.x_edges_mm[i], g.x_edges_mm[i + 1], r.lo[0], r.hi[0])
                });
                let idx = g.index(i, j, k);
                if fx <= 0.0 || !is_air(snap, snap.material[idx]) {
                    continue;
                }
                // Cubic metres to litres.
                let v = g.volume_m3(i, j, k) * 1000.0 * fx * fy * fz;
                let t = f64::from(snap.temperature_c[idx]);
                air += v;
                weighted += v * t;
                if t > level_c {
                    above += v;
                }
                min_c = min_c.min(t);
                max_c = max_c.max(t);
            }
        }
    }
    (air > 0.0).then(|| AirNumbers {
        air_l: air,
        above_l: above,
        mean_c: weighted / air,
        min_c,
        max_c,
    })
}

/// How the air volume is spread over temperature: `bins` equal steps from the
/// coldest to the hottest air, litres in each. Empty for no air.
pub fn air_histogram(snap: &CfdSnapshot, bins: usize) -> Vec<(f64, f64, f64)> {
    let Some(n) = air_numbers(snap, f64::MAX, None) else {
        return Vec::new();
    };
    if bins == 0 {
        return Vec::new();
    }
    let step = ((n.max_c - n.min_c) / bins as f64).max(1e-9);
    let mut out: Vec<(f64, f64, f64)> = (0..bins)
        .map(|b| {
            let lo = n.min_c + step * b as f64;
            (lo, lo + step, 0.0)
        })
        .collect();
    let g = &snap.grid;
    for k in 0..g.nz {
        for j in 0..g.ny {
            for i in 0..g.nx {
                let idx = g.index(i, j, k);
                if !is_air(snap, snap.material[idx]) {
                    continue;
                }
                let t = f64::from(snap.temperature_c[idx]);
                let b = (((t - n.min_c) / step) as usize).min(bins - 1);
                out[b].2 += g.volume_m3(i, j, k) * 1000.0;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_cfd::{
        AccuracyStatement, CfdSnapshot, Fidelity, Grid3, MaterialInfo, MaterialKind, Progress,
        Residuals,
    };

    /// 100 x 80 x 60 mm, `T = 20 + x / 10`, wind 1 m/s along +x, a solid
    /// cube of 20 mm at the origin.
    fn snapshot() -> CfdSnapshot {
        let grid = Grid3::uniform([20, 16, 12], [100.0, 80.0, 60.0]);
        let n = grid.cells();
        let (mut temp, mut mat) = (Vec::new(), Vec::new());
        for k in 0..grid.nz {
            for j in 0..grid.ny {
                for i in 0..grid.nx {
                    let c = grid.centre_mm(i, j, k);
                    temp.push((c[0] / 10.0 + 20.0) as f32);
                    mat.push(u16::from(c[0] < 20.0 && c[1] < 20.0 && c[2] < 20.0));
                }
            }
        }
        CfdSnapshot {
            grid,
            temperature_c: temp,
            velocity: vec![[1.0, 0.0, 0.0]; n],
            pressure_pa: vec![0.0; n],
            turb_visc_ratio: vec![0.0; n],
            material: mat,
            materials: vec![
                MaterialInfo {
                    id: 0,
                    kind: MaterialKind::Air,
                    name: "Air".into(),
                    conductivity_w_mk: 0.026,
                },
                MaterialInfo {
                    id: 1,
                    kind: MaterialKind::Device(7),
                    name: "-U1".into(),
                    conductivity_w_mk: 20.0,
                },
            ],
            wall_flux_w_m2: vec![0.0; n],
            plate_rear_c: None,
            devices: Vec::new(),
            hot_spots: Vec::new(),
            progress: Progress {
                fidelity: Fidelity::Coarse,
                iteration: 1,
                residuals: Residuals::default(),
                converged: true,
                elapsed_s: 0.0,
                energy_balance_rel: 0.0,
                monitor_drift_k: 0.0,
            },
            notes: Vec::new(),
            accuracy: AccuracyStatement::default(),
            heat_paths: Vec::new(),
            radiation_on: false,
            opening_flows: Vec::new(),
        }
    }

    #[test]
    fn a_line_along_a_linear_field_is_a_straight_plot() {
        let snap = snapshot();
        let p = line_profile(&snap, [30.0, 50.0, 40.0], [90.0, 50.0, 40.0], 7).unwrap();
        assert_eq!(p.samples.len(), 7);
        assert!((p.length_mm() - 60.0).abs() < 1e-9);
        for s in &p.samples {
            let x = 30.0 + s.along_mm;
            assert!((s.temperature_c - (20.0 + x / 10.0)).abs() < 0.01, "{s:?}");
            assert!((s.speed - 1.0).abs() < 1e-6 && !s.solid);
        }
        let st = p.stats().unwrap();
        assert!((st.min_c - 23.0).abs() < 0.01 && (st.max_c - 29.0).abs() < 0.01);
        assert!((st.mean_c - 26.0).abs() < 0.01);
        assert_eq!(st.solid_share, 0.0);
    }

    #[test]
    fn a_line_through_a_device_marks_the_solid_samples() {
        let snap = snapshot();
        let p = line_profile(&snap, [5.0, 10.0, 10.0], [95.0, 10.0, 10.0], 10).unwrap();
        assert!(p.samples[0].solid && !p.samples[9].solid);
        let st = p.stats().unwrap();
        assert!(st.solid_share > 0.1 && st.solid_share < 0.4, "{st:?}");
    }

    #[test]
    fn a_bad_line_gives_nothing() {
        let snap = snapshot();
        let a = [10.0, 10.0, 10.0];
        assert!(line_profile(&snap, a, a, 5).is_none(), "no length");
        assert!(line_profile(&snap, a, [10.0, 10.0, 500.0], 5).is_none());
        assert!(line_profile(&snap, a, [50.0, 10.0, 10.0], 1).is_none());
        // All inside the device: no air to report.
        let p = line_profile(&snap, [2.0, 2.0, 2.0], [15.0, 15.0, 15.0], 5).unwrap();
        assert!(p.stats().is_none());
    }

    #[test]
    fn the_csv_has_a_header_and_a_row_per_sample() {
        let snap = snapshot();
        let p = line_profile(&snap, [30.0, 50.0, 40.0], [90.0, 50.0, 40.0], 4).unwrap();
        let csv = p.csv();
        assert!(csv.starts_with("distance_mm,temperature_c,speed_m_s,solid\n"));
        assert_eq!(csv.lines().count(), 5);
        assert!(csv.contains("0.00,23.000,1.0000,0"));
    }

    #[test]
    fn the_air_volume_above_a_level_follows_the_field() {
        let snap = snapshot();
        // Air = whole box 480 000 mm3 minus the 20 mm cube = 472 000 mm3.
        let all = air_numbers(&snap, f64::MAX, None).unwrap();
        assert!((all.air_l - 0.472).abs() < 1e-9, "{all:?}");
        assert_eq!(all.above_l, 0.0);
        // T > 25 C is the half x > 50 mm: 50 * 80 * 60 = 240 000 mm3.
        let hot = air_numbers(&snap, 25.0, None).unwrap();
        assert!((hot.above_l - 0.240).abs() < 1e-6, "{hot:?}");
        assert!((hot.above_share() - 0.240 / 0.472).abs() < 1e-6);
        // The cold cube is missing from the air, so the volume mean lies a
        // little above the 25 C of the plain box.
        assert!(hot.mean_c > 25.0 && hot.mean_c < 25.2, "{hot:?}");
        assert!((hot.max_c - 29.75).abs() < 0.01 && (hot.min_c - 20.25).abs() < 0.01);
    }

    #[test]
    fn a_region_counts_the_overlap_of_cut_cells() {
        let snap = snapshot();
        // x 50..100 of the box; cells are 5 mm wide so the cut is clean.
        let r = Region {
            lo: [50.0, 0.0, 0.0],
            hi: [100.0, 80.0, 60.0],
        };
        let n = air_numbers(&snap, 0.0, Some(r)).unwrap();
        assert!((n.air_l - 0.240).abs() < 1e-9 && (n.above_share() - 1.0).abs() < 1e-9);
        // A cut through a cell (x 52.5): half of the 50..55 cell counts.
        let cut = Region {
            lo: [52.5, 0.0, 0.0],
            hi: [100.0, 80.0, 60.0],
        };
        let c = air_numbers(&snap, 0.0, Some(cut)).unwrap();
        assert!((c.air_l - 0.228).abs() < 1e-9, "{c:?}");
        // A region inside the device holds no air.
        let dev = Region {
            lo: [0.0, 0.0, 0.0],
            hi: [15.0, 15.0, 15.0],
        };
        assert!(air_numbers(&snap, 0.0, Some(dev)).is_none());
    }

    #[test]
    fn the_histogram_adds_up_to_the_air_volume() {
        let snap = snapshot();
        let h = air_histogram(&snap, 5);
        assert_eq!(h.len(), 5);
        let sum: f64 = h.iter().map(|b| b.2).sum();
        assert!((sum - 0.472).abs() < 1e-9, "{sum}");
        assert!(h.windows(2).all(|w| (w[0].1 - w[1].0).abs() < 1e-9));
        assert!(air_histogram(&snap, 0).is_empty());
    }
}
