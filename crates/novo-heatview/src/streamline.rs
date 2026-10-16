//! Streamlines through a velocity field, evenly spaced (Jobard and Lefer),
//! and the step of a moving tracer. Both follow the field with a midpoint
//! step, so they stay on curved flow.

use crate::field::Vector;
use std::collections::HashMap;

/// A line with the speed at each point (m/s), for line weight.
#[derive(Debug, Clone, PartialEq)]
pub struct Streamline {
    pub points: Vec<(f64, f64)>,
    pub speeds: Vec<f32>,
}

/// Settings of [`evenly_spaced`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// Distance between neighbouring lines, mm.
    pub separation: f64,
    /// A line stops when it comes closer than this to another, mm
    /// (a fraction of `separation`, usually 0.5).
    pub test: f64,
    /// Integration step, mm.
    pub step: f64,
    /// Speeds below this stop a line, m/s.
    pub min_speed: f32,
    pub max_points: usize,
}

impl Default for Spacing {
    fn default() -> Self {
        Spacing {
            separation: 28.0,
            test: 14.0,
            step: 4.0,
            min_speed: 0.01,
            max_points: 400,
        }
    }
}

fn unit(v: (f32, f32)) -> Option<((f64, f64), f32)> {
    let s = v.0.hypot(v.1);
    (s > 0.0 && !s.is_nan()).then(|| (((v.0 / s) as f64, (v.1 / s) as f64), s))
}

/// One midpoint step along (`dir` 1.0) or against (`dir` -1.0) the flow.
fn step(
    field: &Vector,
    p: (f64, f64),
    h: f64,
    dir: f64,
    min_speed: f32,
) -> Option<((f64, f64), f32)> {
    let (u, s) = unit(field.sample(p.0, p.1))?;
    if s < min_speed {
        return None;
    }
    let mid = (p.0 + u.0 * h * 0.5 * dir, p.1 + u.1 * h * 0.5 * dir);
    let (um, sm) = unit(field.sample(mid.0, mid.1))?;
    if sm < min_speed {
        return None;
    }
    Some(((p.0 + um.0 * h * dir, p.1 + um.1 * h * dir), sm))
}

struct Occupancy {
    cell: f64,
    points: HashMap<(i64, i64), Vec<(f64, f64)>>,
}

impl Occupancy {
    fn key(&self, p: (f64, f64)) -> (i64, i64) {
        (
            (p.0 / self.cell).floor() as i64,
            (p.1 / self.cell).floor() as i64,
        )
    }
    fn add(&mut self, p: (f64, f64)) {
        let k = self.key(p);
        self.points.entry(k).or_default().push(p);
    }
    fn near(&self, p: (f64, f64), d: f64) -> bool {
        let (kx, ky) = self.key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = self.points.get(&(kx + dx, ky + dy))
                    && v.iter().any(|q| (q.0 - p.0).hypot(q.1 - p.1) < d)
                {
                    return true;
                }
            }
        }
        false
    }
}

/// Evenly spaced streamlines. Seeds are tried in the order given (put inlets
/// and spots above hot devices first); a seed too close to an earlier line
/// is skipped. `blocked` says where a line must stop (inside a device body).
/// Lines shorter than 4 points are dropped.
pub fn evenly_spaced(
    field: &Vector,
    seeds: &[(f64, f64)],
    sp: Spacing,
    blocked: &dyn Fn(f64, f64) -> bool,
) -> Vec<Streamline> {
    let (w, h) = (field.grid.width(), field.grid.height());
    let inside =
        |p: (f64, f64)| p.0 >= 0.0 && p.1 >= 0.0 && p.0 <= w && p.1 <= h && !blocked(p.0, p.1);
    let mut occ = Occupancy {
        cell: sp.separation.max(1.0),
        points: HashMap::new(),
    };
    let mut out = Vec::new();
    for &seed in seeds {
        if !inside(seed) || occ.near(seed, sp.separation) {
            continue;
        }
        let trace = |dir: f64, occ: &Occupancy| {
            let mut pts = Vec::new();
            let mut p = seed;
            while pts.len() < sp.max_points {
                let Some((n, s)) = step(field, p, sp.step, dir, sp.min_speed) else {
                    break;
                };
                if !inside(n) || occ.near(n, sp.test) {
                    break;
                }
                pts.push((n, s));
                p = n;
            }
            pts
        };
        let fwd = trace(1.0, &occ);
        let back = trace(-1.0, &occ);
        let seed_speed = field.sample(seed.0, seed.1);
        let mut points: Vec<(f64, f64)> = back.iter().rev().map(|(p, _)| *p).collect();
        let mut speeds: Vec<f32> = back.iter().rev().map(|(_, s)| *s).collect();
        points.push(seed);
        speeds.push(seed_speed.0.hypot(seed_speed.1));
        points.extend(fwd.iter().map(|(p, _)| *p));
        speeds.extend(fwd.iter().map(|(_, s)| *s));
        if points.len() < 4 {
            continue;
        }
        for &p in &points {
            occ.add(p);
        }
        out.push(Streamline { points, speeds });
    }
    out
}

/// Advance a tracer by `dt` seconds with the field's speed (m/s, the field
/// being in mm: `mm_per_m` converts, 1000 for real units). Returns `None`
/// when it leaves the field, stalls or is blocked, so the caller respawns it.
pub fn advect(
    field: &Vector,
    p: (f64, f64),
    dt: f64,
    speed_scale: f64,
    blocked: &dyn Fn(f64, f64) -> bool,
) -> Option<(f64, f64)> {
    let (vx, vy) = field.sample(p.0, p.1);
    if vx.hypot(vy) < 1e-4 {
        return None;
    }
    let mid = (
        p.0 + vx as f64 * dt * 0.5 * speed_scale,
        p.1 + vy as f64 * dt * 0.5 * speed_scale,
    );
    let (mx, my) = field.sample(mid.0, mid.1);
    let n = (
        p.0 + mx as f64 * dt * speed_scale,
        p.1 + my as f64 * dt * speed_scale,
    );
    let (w, h) = (field.grid.width(), field.grid.height());
    (n.0 >= 0.0 && n.1 >= 0.0 && n.0 <= w && n.1 <= h && !blocked(n.0, n.1)).then_some(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Grid;

    /// Uniform flow to the right.
    fn right(cols: usize, rows: usize) -> Vector {
        let grid = Grid {
            cols,
            rows,
            cell_w: 10.0,
            cell_h: 10.0,
        };
        Vector::new(grid, vec![1.0; cols * rows], vec![0.0; cols * rows])
    }

    /// Solid body rotation about the centre.
    fn swirl() -> Vector {
        let n = 40;
        let grid = Grid {
            cols: n,
            rows: n,
            cell_w: 10.0,
            cell_h: 10.0,
        };
        let mut vx = Vec::new();
        let mut vy = Vec::new();
        for r in 0..n {
            for c in 0..n {
                let (x, y) = grid.centre(c, r);
                let (dx, dy) = (x - 200.0, y - 200.0);
                vx.push((-dy / 100.0) as f32);
                vy.push((dx / 100.0) as f32);
            }
        }
        Vector::new(grid, vx, vy)
    }

    #[test]
    fn lines_follow_uniform_flow_and_keep_apart() {
        let f = right(40, 20);
        let seeds: Vec<_> = (0..20).map(|i| (200.0, 5.0 + i as f64 * 10.0)).collect();
        let lines = evenly_spaced(&f, &seeds, Spacing::default(), &|_, _| false);
        assert!(lines.len() >= 5 && lines.len() <= 8, "{}", lines.len());
        for l in &lines {
            let y0 = l.points[0].1;
            assert!(l.points.iter().all(|p| (p.1 - y0).abs() < 1e-6), "straight");
            assert!(l.points.first().unwrap().0 < 10.0 && l.points.last().unwrap().0 > 390.0);
        }
        let mut ys: Vec<f64> = lines.iter().map(|l| l.points[0].1).collect();
        ys.sort_by(f64::total_cmp);
        assert!(ys.windows(2).all(|w| w[1] - w[0] >= 28.0 - 1e-6));
    }

    #[test]
    fn a_swirl_curves_and_stays_at_its_radius() {
        let f = swirl();
        let lines = evenly_spaced(&f, &[(300.0, 200.0)], Spacing::default(), &|_, _| false);
        assert_eq!(lines.len(), 1);
        for p in &lines[0].points {
            let r = (p.0 - 200.0).hypot(p.1 - 200.0);
            assert!((r - 100.0).abs() < 2.0, "radius drift {r}");
        }
    }

    #[test]
    fn blocked_regions_stop_lines() {
        let f = right(40, 20);
        let lines = evenly_spaced(&f, &[(20.0, 100.0)], Spacing::default(), &|x, _| x > 200.0);
        assert!(lines[0].points.iter().all(|p| p.0 <= 200.0));
    }

    #[test]
    fn still_air_gives_no_lines() {
        let grid = Grid {
            cols: 10,
            rows: 10,
            cell_w: 10.0,
            cell_h: 10.0,
        };
        let f = Vector::new(grid, vec![0.0; 100], vec![0.0; 100]);
        assert!(evenly_spaced(&f, &[(50.0, 50.0)], Spacing::default(), &|_, _| false).is_empty());
    }

    #[test]
    fn tracers_move_and_respawn_at_the_edge() {
        let f = right(10, 10);
        let p = advect(&f, (10.0, 50.0), 1.0, 20.0, &|_, _| false).unwrap();
        assert!((p.0 - 30.0).abs() < 1e-6);
        assert!(advect(&f, (95.0, 50.0), 1.0, 20.0, &|_, _| false).is_none());
    }
}
