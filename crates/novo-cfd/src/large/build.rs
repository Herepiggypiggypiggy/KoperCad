//! Lumping, snapping and smearing: the reduced scenario.

use novo_layout::Rect;
use novo_thermal::{Device, Obstacle, ObstacleKind, Scenario};

use super::report::LargeReport;
use crate::mesh::{MeshSettings, plan_grid_report};

/// Choices of the reduction; the defaults are planning values.
#[derive(Debug, Clone, PartialEq)]
pub struct ReduceSettings {
    /// A device is cold when its loss is at most this share of the total.
    pub cold_share: f64,
    /// ... but at least this many watts counts as cold regardless.
    pub cold_min_w: f64,
    /// Largest gap between two devices of one block, mm.
    pub max_gap_mm: f64,
    /// Longest block, mm. Keeps vertical gaps in a long row.
    pub max_block_mm: f64,
    /// Mesh rules used to size the grid.
    pub mesh: MeshSettings,
}

impl ReduceSettings {
    pub fn for_cells(target_cells: usize) -> ReduceSettings {
        ReduceSettings {
            cold_share: 0.03,
            cold_min_w: 0.5,
            max_gap_mm: 40.0,
            max_block_mm: 400.0,
            mesh: MeshSettings {
                target_cells,
                ..MeshSettings::default()
            },
        }
    }
}

/// A group of original devices solved as one body.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// The id in the reduced scenario (the member's own id for a block of
    /// one).
    pub id: u64,
    /// Indices into the original `devices`.
    pub members: Vec<usize>,
}

/// The reduced scenario and the way back.
#[derive(Debug, Clone, PartialEq)]
pub struct Reduction {
    pub scenario: Scenario,
    pub blocks: Vec<Block>,
    pub report: LargeReport,
}

/// Most the solid volume of the devices may grow by snapping (ratio).
const MAX_VOLUME_RATIO: f64 = 1.6;
const SYNTHETIC_BASE: u64 = u64::MAX / 2;
/// Pitches tried, mm; 0 is lumping alone.
const PITCHES: [f64; 14] = [
    0.0, 5.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0, 96.0, 128.0, 192.0, 256.0, 384.0,
];

fn overlap(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.1.min(b.1) - a.0.max(b.0)).max(0.0)
}

fn rects_touch(a: &Rect, b: &Rect) -> bool {
    overlap((a.x, a.right()), (b.x, b.right())) > 0.0
        && overlap((a.y, a.bottom()), (b.y, b.bottom())) > 0.0
}

fn bounds(rects: impl Iterator<Item = Rect>) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for r in rects {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.right());
        y1 = y1.max(r.bottom());
    }
    Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

/// Splits the devices into hot ones (kept) and row blocks of cold ones.
fn plan_blocks(scn: &Scenario, st: &ReduceSettings) -> (usize, Vec<Vec<usize>>) {
    let total: f64 = scn.devices.iter().map(|d| d.loss_w).sum();
    let cold_limit = (st.cold_share * total).max(st.cold_min_w);
    let hot: Vec<&Device> = scn
        .devices
        .iter()
        .filter(|d| d.loss_w > cold_limit)
        .collect();
    let mut cold: Vec<usize> = (0..scn.devices.len())
        .filter(|&i| scn.devices[i].loss_w <= cold_limit)
        .collect();
    cold.sort_by(|&a, &b| {
        scn.devices[a]
            .footprint
            .x
            .total_cmp(&scn.devices[b].footprint.x)
            .then(scn.devices[a].id.cmp(&scn.devices[b].id))
    });
    let mut blocks: Vec<(Rect, Vec<usize>)> = Vec::new();
    for i in cold {
        let f = scn.devices[i].footprint;
        // the open block on the same row that this device continues
        let host = blocks.iter().position(|(r, _)| {
            let row = overlap((r.y, r.bottom()), (f.y, f.bottom())) >= 0.5 * r.h.min(f.h)
                && f.x - r.right() <= st.max_gap_mm
                && f.right() - r.x <= st.max_block_mm;
            if !row {
                return false;
            }
            let joined = bounds([*r, f].into_iter());
            !hot.iter().any(|h| rects_touch(&joined, &h.footprint))
        });
        match host {
            Some(b) => {
                let (r, m) = &mut blocks[b];
                m.push(i);
                *r = bounds(m.iter().map(|&k| scn.devices[k].footprint));
            }
            None => blocks.push((f, vec![i])),
        }
    }
    // hot devices first (exact), then the blocks in a stable order
    let mut out: Vec<Vec<usize>> = (0..scn.devices.len())
        .filter(|&i| scn.devices[i].loss_w > cold_limit)
        .map(|i| vec![i])
        .collect();
    let hot_n = out.len();
    out.extend(blocks.into_iter().map(|b| b.1));
    (hot_n, out)
}

/// The planes the reduced bodies may sit on, per axis: the domain walls, the
/// plate edges, the edges of the exact (hot) devices and the openings, and a
/// lattice of pitch `p` thinned so that no lattice plane lies within 0.4 p
/// of another anchor. Snapping to these never makes a sliver next to an
/// exact body.
struct Lattice {
    /// `None` for pitch 0 (nothing is moved).
    axes: Option<[Vec<f64>; 3]>,
}

/// Planes within this share of the pitch of each other become one plane.
const CLUSTER: f64 = 0.3;

impl Lattice {
    fn new(scn: &Scenario, hot: &[usize], p: f64) -> Lattice {
        if p <= 0.0 {
            return Lattice { axes: None };
        }
        let dom = crate::mesh::domain_mm(scn);
        let t = scn.params.plate_thickness_mm.clamp(0.0, dom[2]);
        let pl = scn.enclosure.plate;
        // planes that never move: the walls, the plate edges, the openings
        let mut fixed: [Vec<f64>; 3] = [
            vec![0.0, dom[0], pl.x, pl.right()],
            vec![0.0, dom[1], pl.y, pl.bottom()],
            vec![0.0, dom[2], t],
        ];
        for o in scn
            .vents
            .iter()
            .map(|v| &v.opening)
            .chain(scn.fans.iter().map(|f| &f.opening))
        {
            for (a, sp) in crate::mesh::opening_spans(o).into_iter().enumerate() {
                if let Some((lo, hi)) = sp {
                    fixed[a].extend([lo, hi]);
                }
            }
        }
        // planes of the devices that make heat: they join a fixed plane or
        // each other when closer than CLUSTER x pitch
        let mut movable: [Vec<f64>; 3] = Default::default();
        for &i in hot {
            let d = &scn.devices[i];
            let f = &d.footprint;
            movable[0].extend([f.x, f.right()]);
            movable[1].extend([f.y, f.bottom()]);
            movable[2].push(t + d.height());
        }
        let axes = std::array::from_fn(|a| {
            // heights keep their volume, so they snap to a finer lattice
            let pitch = if a == 2 {
                (p / 8.0).clamp(4.0, 24.0)
            } else {
                p
            };
            let tol = CLUSTER * pitch;
            let inside = |v: &f64| (0.0..=dom[a]).contains(v);
            let mut pts: Vec<f64> = fixed[a].iter().copied().filter(inside).collect();
            pts.sort_by(f64::total_cmp);
            pts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
            let mut free: Vec<f64> = movable[a].iter().copied().filter(inside).collect();
            free.sort_by(f64::total_cmp);
            let mut cluster: Vec<f64> = Vec::new();
            let flush = |cluster: &mut Vec<f64>, pts: &mut Vec<f64>| {
                if cluster.is_empty() {
                    return;
                }
                let mean = cluster.iter().sum::<f64>() / cluster.len() as f64;
                // joins a fixed plane when one is close
                if !pts.iter().any(|f| (f - mean).abs() <= tol) {
                    pts.push(mean);
                }
                cluster.clear();
            };
            for v in free {
                if cluster.last().is_some_and(|l| v - l > tol) {
                    flush(&mut cluster, &mut pts);
                }
                cluster.push(v);
            }
            flush(&mut cluster, &mut pts);
            let anchors = pts.clone();
            let mut v = pitch;
            while v < dom[a] {
                if anchors.iter().all(|f| (f - v).abs() >= 0.4 * pitch) {
                    pts.push(v);
                }
                v += pitch;
            }
            pts.sort_by(f64::total_cmp);
            pts
        });
        Lattice { axes: Some(axes) }
    }

    fn nearest(&self, a: usize, v: f64) -> f64 {
        let Some(ax) = &self.axes else {
            return v;
        };
        ax[a]
            .iter()
            .min_by(|x, y| (*x - v).abs().total_cmp(&(*y - v).abs()))
            .copied()
            .unwrap_or(v)
    }

    /// The first anchor strictly above `v` (or `v` itself without a lattice).
    fn above(&self, a: usize, v: f64) -> f64 {
        let Some(ax) = &self.axes else {
            return v;
        };
        ax[a].iter().copied().find(|x| *x > v + 1e-9).unwrap_or(v)
    }

    /// `[lo, hi]` moved onto anchors, never empty.
    fn span(&self, a: usize, lo: f64, hi: f64) -> (f64, f64) {
        if self.axes.is_none() {
            return (lo, hi);
        }
        let l = self.nearest(a, lo);
        let mut h = self.nearest(a, hi);
        if h <= l + 1e-9 {
            h = self.above(a, l);
        }
        (l, h)
    }
}

/// `carry` is the solid volume (mm3) the heights rounded away so far; the next
/// block takes it up, so the total stays within one rounding step.
fn block_device(
    scn: &Scenario,
    members: &[usize],
    id: u64,
    exact: bool,
    lat: &Lattice,
    t: f64,
    carry: &mut f64,
) -> Device {
    let ds: Vec<&Device> = members.iter().map(|&i| &scn.devices[i]).collect();
    if exact && ds.len() == 1 {
        // a device that makes heat: its planes may join a neighbour's (see
        // `Lattice::new`), its size and loss stay
        let mut d = ds[0].clone();
        let f = d.footprint;
        let (x0, x1) = lat.span(0, f.x, f.right());
        let (y0, y1) = lat.span(1, f.y, f.bottom());
        let (_, top) = lat.span(2, t, t + d.height());
        d.footprint = Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        };
        d.height_mm = Some(top - t);
        return d;
    }
    let bb = bounds(ds.iter().map(|d| d.footprint));
    let volume: f64 = ds
        .iter()
        .map(|d| d.footprint.w * d.footprint.h * d.height())
        .sum();
    let (x0, x1) = lat.span(0, bb.x, bb.right());
    let (y0, y1) = lat.span(1, bb.y, bb.bottom());
    let area = (x1 - x0) * (y1 - y0);
    let want = volume + *carry;
    let (_, top) = lat.span(2, t, t + want / area.max(1.0));
    let h = top - t;
    *carry = want - h * area;
    let cap = |f: fn(&Device) -> Option<f64>| -> Option<f64> {
        ds.iter().filter_map(|d| f(d)).reduce(f64::min)
    };
    let heat_cap = if ds.iter().any(|d| d.heat_capacity_j_per_k.is_some()) {
        Some(
            ds.iter()
                .map(|d| {
                    d.heat_capacity_j_per_k
                        .unwrap_or(500.0 * d.footprint.w * d.footprint.h * 1e-4)
                })
                .sum(),
        )
    } else {
        None
    };
    Device {
        id,
        footprint: Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        },
        height_mm: Some(h),
        loss_w: ds.iter().map(|d| d.loss_w).sum(),
        rated_max_ambient_c: cap(|d| d.rated_max_ambient_c),
        rated_max_surface_c: cap(|d| d.rated_max_surface_c),
        heat_capacity_j_per_k: heat_cap,
    }
}

fn smear(o: &Obstacle, depth_mm: f64, t: f64, lat: &Lattice) -> (Obstacle, bool) {
    if lat.axes.is_none() || o.kind == ObstacleKind::Solid {
        return (o.clone(), false);
    }
    let r = o.rect;
    let (x0, x1) = lat.span(0, r.x, r.right());
    let (y0, y1) = lat.span(1, r.y, r.bottom());
    let thin_before = r.w.min(r.h);
    let thin_after = (x1 - x0).min(y1 - y0);
    // volume kept: the area grew, so the strip gets lower
    let scale = (r.w * r.h / ((x1 - x0) * (y1 - y0)).max(1.0)).min(1.0);
    let h = o.blockage() * depth_mm * scale;
    let (_, top) = lat.span(2, t, t + h);
    let blockage = ((top - t) / depth_mm).clamp(0.01, 1.0);
    (
        Obstacle {
            kind: o.kind,
            rect: Rect {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            },
            blockage: Some(blockage),
            loss_w: o.loss_w,
        },
        thin_after > thin_before * 1.05,
    )
}

fn assemble(
    scn: &Scenario,
    groups: &[Vec<usize>],
    hot_n: usize,
    p: f64,
) -> (Scenario, Vec<Block>, usize) {
    let t = scn.params.plate_thickness_mm;
    let dom_d = scn.enclosure.interior_depth_mm() - t;
    let hot: Vec<usize> = groups.iter().take(hot_n).map(|g| g[0]).collect();
    let lat = Lattice::new(scn, &hot, p);
    let mut blocks = Vec::new();
    let mut devices = Vec::new();
    let mut carry = 0.0;
    for (n, g) in groups.iter().enumerate() {
        let id = if g.len() == 1 {
            scn.devices[g[0]].id
        } else {
            SYNTHETIC_BASE + n as u64
        };
        devices.push(block_device(scn, g, id, n < hot_n, &lat, t, &mut carry));
        blocks.push(Block {
            id,
            members: g.clone(),
        });
    }
    let mut smeared = 0;
    let obstacles = scn
        .obstacles
        .iter()
        .map(|o| {
            let (o2, grew) = smear(o, dom_d, t, &lat);
            smeared += usize::from(grew);
            o2
        })
        .collect();
    let mut s = scn.clone();
    s.devices = devices;
    s.obstacles = obstacles;
    (s, blocks, smeared)
}

/// Reduces `scn` to about `settings.mesh.target_cells` cells.
pub fn reduce(scn: &Scenario, settings: &ReduceSettings) -> Reduction {
    let (hot_n, groups) = plan_blocks(scn, settings);
    let total_heat: f64 = scn.devices.iter().map(|d| d.loss_w).sum();
    let target = settings.mesh.target_cells as f64;
    let full_cells = plan_grid_report(scn, &settings.mesh).map_or(0, |p| p.grid.cells());

    let vol = |s: &Scenario| -> f64 {
        s.devices
            .iter()
            .map(|d| d.footprint.w * d.footprint.h * d.height())
            .sum()
    };
    let vol0 = vol(scn).max(1.0);
    // (scenario, blocks, smeared, pitch, cells, over budget, volume ratio)
    type Candidate = (Scenario, Vec<Block>, usize, f64, usize, bool, f64);
    let mut best: Option<Candidate> = None;
    for &p in &PITCHES {
        let (s, blocks, smeared) = assemble(scn, &groups, hot_n, p);
        let Ok(plan) = plan_grid_report(&s, &settings.mesh) else {
            continue;
        };
        let cells = plan.grid.cells();
        let ratio = vol(&s) / vol0;
        if std::env::var("LARGE_DEBUG").is_ok() {
            eprintln!(
                "pitch {p}: {}x{}x{} = {cells} across {} over {} volume x{ratio:.2}",
                plan.grid.nx, plan.grid.ny, plan.grid.nz, plan.cells_across, plan.over_budget
            );
        }
        // A pitch that makes the bodies much bigger than they are would
        // answer another question: it is only taken when nothing else fits.
        let faithful = ratio <= MAX_VOLUME_RATIO;
        let fits = cells as f64 <= 1.25 * target && !plan.over_budget && faithful;
        let better = best.as_ref().is_none_or(|b| {
            let b_faithful = b.6 <= MAX_VOLUME_RATIO;
            match (faithful, b_faithful) {
                (true, false) => true,
                (false, true) => false,
                (true, true) => cells < b.4,
                (false, false) => ratio < b.6,
            }
        });
        if better {
            best = Some((s, blocks, smeared, p, cells, plan.over_budget, ratio));
        }
        if fits {
            break;
        }
    }
    let (scenario, blocks, smeared, pitch, cells, over, _) =
        best.unwrap_or_else(|| (scn.clone(), Vec::new(), 0, 0.0, full_cells, true, 1.0));
    let lumped: Vec<&Block> = blocks.iter().filter(|b| b.members.len() > 1).collect();
    let report = LargeReport {
        full_cells,
        reduced_cells: cells,
        target_cells: settings.mesh.target_cells,
        pitch_mm: pitch,
        devices: scn.devices.len(),
        exact_devices: hot_n,
        lumped_devices: lumped.iter().map(|b| b.members.len()).sum(),
        blocks: lumped.len(),
        lumped_heat_w: lumped
            .iter()
            .flat_map(|b| b.members.iter())
            .map(|&i| scn.devices[i].loss_w)
            .sum(),
        total_heat_w: total_heat,
        smeared_obstacles: smeared,
        over_budget: over,
        hot_shift_mm: scenario
            .devices
            .iter()
            .take(hot_n)
            .zip(blocks.iter().take(hot_n))
            .map(|(d, b)| {
                let o = &scn.devices[b.members[0]];
                let (a, c) = (o.footprint, d.footprint);
                [
                    a.x - c.x,
                    a.y - c.y,
                    a.right() - c.right(),
                    a.bottom() - c.bottom(),
                ]
                .iter()
                .fold(0.0, |m: f64, v| m.max(v.abs()))
            })
            .fold(0.0, f64::max),
    };
    Reduction {
        scenario,
        blocks,
        report,
    }
}
