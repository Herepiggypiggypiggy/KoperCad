//! Heat on the mounting plate (R3): per-device heat sources, a loss-density
//! map with a legend, hot-device ranking and the share of heat high on the
//! plate. The enclosure-level heat balance (effective surface, rise, cooling
//! power) is `crate::sizing::thermal`; see `docs/design/thermal.md`.
//! A planning value, never an authority.
//!
//! Everything here is a pure function of its inputs; [`model`] reads the
//! devices' losses from a project.

use crate::{Placement, Plate, Rect};

/// Note shown next to every heat result.
pub const NOTE: &str = "Planning value. Check with the enclosure or cooling unit maker.";

/// Default heat map cell size, mm.
pub const DEFAULT_CELL_MM: f64 = 10.0;
/// Default blur radius, mm.
pub const DEFAULT_SPREAD_MM: f64 = 40.0;
/// Smallest cell size and most cells [`heat_map`] will use: a finer request
/// is coarsened so the blur stays fast on a large plate.
pub const MIN_CELL_MM: f64 = 5.0;
pub const MAX_CELLS: usize = 40_000;
/// Upper limits of the first four heat bands, W/dm2; the fifth is above.
pub const DEFAULT_BAND_LIMITS: [f64; 4] = [0.5, 2.0, 5.0, 10.0];

/// One device's heat for the plate map; `loss_w` is already the effective
/// loss (rated loss times load factor).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatSource {
    pub id: u64,
    pub footprint: Rect,
    pub loss_w: f64,
}

impl HeatSource {
    /// Loss per footprint area, W/dm2.
    pub fn density(&self) -> f64 {
        let dm2 = self.footprint.area() / 10_000.0;
        if dm2 > 0.0 { self.loss_w / dm2 } else { 0.0 }
    }
}

/// Sources for the placed devices whose loss is known: `loss` gives a
/// device's effective loss by id, `None` when unknown. A negative loss is
/// treated as 0.
pub fn sources(placements: &[Placement], loss: impl Fn(u64) -> Option<f64>) -> Vec<HeatSource> {
    placements
        .iter()
        .filter_map(|p| {
            Some(HeatSource {
                id: p.id,
                footprint: p.footprint(),
                loss_w: loss(p.id)?.max(0.0),
            })
        })
        .collect()
}

/// Loss density over the plate, W/dm2 per cell, row by row from the top
/// left.
#[derive(Debug, Clone, PartialEq)]
pub struct HeatMap {
    pub cols: usize,
    pub rows: usize,
    pub cell_mm: f64,
    pub density: Vec<f64>,
}

impl HeatMap {
    pub fn at(&self, col: usize, row: usize) -> f64 {
        self.density[row * self.cols + col]
    }

    pub fn max(&self) -> f64 {
        self.density.iter().copied().fold(0.0, f64::max)
    }

    /// The band of every cell (0 cool to 4 very hot) for the canvas overlay.
    pub fn bands(&self, limits: &[f64; 4]) -> Vec<u8> {
        self.density
            .iter()
            .map(|d| band_of(*d, limits) as u8)
            .collect()
    }

    /// Total heat in the map, W (conserved by the blur).
    pub fn total_w(&self) -> f64 {
        let dm2 = (self.cell_mm / 100.0).powi(2);
        self.density.iter().sum::<f64>() * dm2
    }
}

/// Builds the heat map: each source spreads its loss over its footprint, then
/// a box blur of `spread_mm` radius shares it with the neighbourhood. The
/// cell is at least [`MIN_CELL_MM`] and grows until there are at most
/// [`MAX_CELLS`] cells. Heat of a footprint that reaches past the plate is
/// not mapped, so the sum is conserved only for devices inside the plate.
pub fn heat_map(plate: &Plate, sources: &[HeatSource], cell_mm: f64, spread_mm: f64) -> HeatMap {
    let mut cell = cell_mm.max(MIN_CELL_MM);
    while (plate.width / cell).ceil() * (plate.height / cell).ceil() > MAX_CELLS as f64 {
        cell *= 1.5;
    }
    let cols = ((plate.width / cell).ceil() as usize).max(1);
    let rows = ((plate.height / cell).ceil() as usize).max(1);
    let mut raw = vec![0.0; cols * rows];
    for s in sources {
        let f = s.footprint;
        let d = s.density();
        if d <= 0.0 {
            continue;
        }
        let c0 = ((f.x / cell).floor().max(0.0)) as usize;
        let c1 = (((f.right() / cell).ceil()) as usize).min(cols);
        let r0 = ((f.y / cell).floor().max(0.0)) as usize;
        let r1 = (((f.bottom() / cell).ceil()) as usize).min(rows);
        for r in r0..r1 {
            for c in c0..c1 {
                let cell_rect = Rect {
                    x: c as f64 * cell,
                    y: r as f64 * cell,
                    w: cell,
                    h: cell,
                };
                let ox = (cell_rect.right().min(f.right()) - cell_rect.x.max(f.x)).max(0.0);
                let oy = (cell_rect.bottom().min(f.bottom()) - cell_rect.y.max(f.y)).max(0.0);
                raw[r * cols + c] += d * ox * oy / (cell * cell);
            }
        }
    }
    let radius = (spread_mm / cell).round() as usize;
    let density = if radius == 0 {
        raw
    } else {
        blur(&raw, cols, rows, radius)
    };
    HeatMap {
        cols,
        rows,
        cell_mm: cell,
        density,
    }
}

/// Box blur that conserves the sum: every cell hands its value out evenly
/// to the cells within `radius`, clipped to the plate; heat that would leave
/// the plate stays in the cells at the edge.
fn blur(raw: &[f64], cols: usize, rows: usize, radius: usize) -> Vec<f64> {
    let mut out = vec![0.0; raw.len()];
    for r in 0..rows {
        for c in 0..cols {
            let v = raw[r * cols + c];
            if v == 0.0 {
                continue;
            }
            let (c0, c1) = (c.saturating_sub(radius), (c + radius).min(cols - 1));
            let (r0, r1) = (r.saturating_sub(radius), (r + radius).min(rows - 1));
            let n = ((c1 - c0 + 1) * (r1 - r0 + 1)) as f64;
            for rr in r0..=r1 {
                for cc in c0..=c1 {
                    out[rr * cols + cc] += v / n;
                }
            }
        }
    }
    out
}

/// One band of the legend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    /// Lower limit, W/dm2 (inclusive); the band runs up to the next one.
    pub from: f64,
    pub label: &'static str,
}

const BAND_LABELS: [&str; 5] = ["cool", "mild", "warm", "hot", "very hot"];

/// The legend: five bands from the limits.
pub fn legend(limits: &[f64; 4]) -> [Band; 5] {
    let mut from = [0.0, limits[0], limits[1], limits[2], limits[3]].into_iter();
    std::array::from_fn(|i| Band {
        from: from.next().unwrap_or(0.0),
        label: BAND_LABELS[i],
    })
}

/// The band index (0 cool to 4 very hot) of a density.
pub fn band_of(density: f64, limits: &[f64; 4]) -> usize {
    limits.iter().filter(|l| density >= **l).count()
}

/// Heat sources ranked by share of the total loss, biggest first.
pub fn hot_devices(sources: &[HeatSource]) -> Vec<(u64, f64)> {
    let total: f64 = sources.iter().map(|s| s.loss_w).sum();
    let mut out: Vec<(u64, f64)> = sources
        .iter()
        .map(|s| (s.id, if total > 0.0 { s.loss_w / total } else { 0.0 }))
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

/// Share of the placed loss in the upper third of the plate, by footprint
/// centre. Hot air collects at the top, so a high share is a layout problem.
pub fn top_third_share(plate: &Plate, sources: &[HeatSource]) -> f64 {
    let total: f64 = sources.iter().map(|s| s.loss_w).sum();
    if total <= 0.0 {
        return 0.0;
    }
    let limit = plate.height / 3.0;
    let top: f64 = sources
        .iter()
        .filter(|s| s.footprint.y + s.footprint.h / 2.0 < limit)
        .map(|s| s.loss_w)
        .sum();
    top / total
}

pub mod model {
    //! Reads heat inputs from a project: part loss data for the devices at an
    //! enclosure's location.

    use novo_core::{Device, Part, Project, Structure, StructureId, Value};

    /// Part property keys, the same names as the Parts library uses.
    pub const POWER_LOSS_KEY: &str = "power_loss_w";
    pub const LOAD_FACTOR_KEY: &str = "load_factor";

    fn number(value: Option<&Value>) -> Option<f64> {
        match value? {
            Value::Float(v) => Some(*v),
            Value::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// A property of the device, else of the first of its parts that has it
    /// (the device overrides the part, as in the Parts library). Negative or
    /// non-numeric values count as missing.
    fn property(project: &Project, device: &Device, key: &str) -> Option<f64> {
        let valid = |v: Option<f64>| v.filter(|v| *v >= 0.0);
        valid(number(device.properties.get(key))).or_else(|| {
            device
                .parts
                .iter()
                .find_map(|part| valid(number(project.get::<Part>(*part)?.properties.get(key))))
        })
    }

    /// The effective loss of a device in W: `power_loss_w` times
    /// `load_factor` (default 1, clamped to 0..=1). `None` when no loss is
    /// given; never 0 by omission.
    pub fn device_loss(project: &Project, device: &Device) -> Option<f64> {
        let loss = property(project, device, POWER_LOSS_KEY)?;
        let factor = property(project, device, LOAD_FACTOR_KEY).unwrap_or(1.0);
        Some(loss * factor.clamp(0.0, 1.0))
    }

    /// Every device at the enclosure's location with its effective loss, in
    /// tag order. A device without a part or loss data has `None`.
    pub fn losses(project: &Project, id: StructureId) -> Vec<(u64, Option<f64>)> {
        let Some(s) = project.get::<Structure>(id) else {
            return Vec::new();
        };
        if s.tag.location.is_empty() {
            return Vec::new();
        }
        let mut devices: Vec<&Device> = project
            .all::<Device>()
            .filter(|d| d.tag.location == s.tag.location)
            .collect();
        devices.sort_by_key(|d| d.tag.to_string());
        devices
            .into_iter()
            .map(|d| (d.id.raw(), device_loss(project, d)))
            .collect()
    }
}
