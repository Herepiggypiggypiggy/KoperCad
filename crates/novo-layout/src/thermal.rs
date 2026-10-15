//! Heat model and cooling (R3): total loss, effective cooling surface,
//! temperature rise, required cooling power and a loss-density map of the
//! mounting plate. See `docs/design/thermal.md` for the formulas and the
//! standard they follow (IEC 60890 effective-area method, steady-state heat
//! balance). A planning value, never an authority.
//!
//! Everything here is a pure function of its inputs; [`model`] reads the
//! inputs from a project.

use crate::{Placement, Plate, Rect};

/// Air heat capacity, J/(kg K).
const AIR_CP: f64 = 1005.0;
/// Air density at 35 degrees C and sea level, kg/m3.
const AIR_RHO_SEA: f64 = 1.146;
/// Scale height of the atmosphere used to thin the air with altitude, m.
const SCALE_HEIGHT_M: f64 = 8434.0;

pub const DEFAULT_AMBIENT_C: f64 = 35.0;
pub const DEFAULT_MAX_INTERNAL_C: f64 = 40.0;
/// Default heat map cell size and blur radius, mm.
pub const DEFAULT_CELL_MM: f64 = 10.0;
pub const DEFAULT_SPREAD_MM: f64 = 40.0;
/// Upper limits of the first four heat bands, W/dm2; the fifth is above.
pub const DEFAULT_BAND_LIMITS: [f64; 4] = [0.5, 2.0, 5.0, 10.0];

/// What the enclosure wall is made of: the heat transfer coefficient `k`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Material {
    #[default]
    SheetSteel,
    StainlessSteel,
    Aluminium,
    Plastic,
}

impl Material {
    /// Heat transfer coefficient in W/(m2 K), surface emission included.
    pub fn k(self) -> f64 {
        match self {
            Material::SheetSteel => 5.5,
            Material::StainlessSteel => 4.5,
            Material::Aluminium => 12.0,
            Material::Plastic => 3.5,
        }
    }

    pub fn parse(text: &str) -> Option<Material> {
        match text.trim().to_ascii_lowercase().as_str() {
            "sheet_steel" | "steel" => Some(Material::SheetSteel),
            "stainless_steel" | "stainless" => Some(Material::StainlessSteel),
            "aluminium" | "aluminum" => Some(Material::Aluminium),
            "plastic" => Some(Material::Plastic),
            _ => None,
        }
    }
}

/// How the enclosure stands: which faces are covered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Installation {
    #[default]
    FreeStanding,
    Wall,
    RowEnd,
    RowEndWall,
    RowMiddle,
    RowMiddleWall,
}

impl Installation {
    pub fn parse(text: &str) -> Option<Installation> {
        match text.trim().to_ascii_lowercase().as_str() {
            "free_standing" => Some(Installation::FreeStanding),
            "wall" => Some(Installation::Wall),
            "row_end" => Some(Installation::RowEnd),
            "row_end_wall" => Some(Installation::RowEndWall),
            "row_middle" => Some(Installation::RowMiddle),
            "row_middle_wall" => Some(Installation::RowMiddleWall),
            _ => None,
        }
    }

    fn rear_covered(self) -> bool {
        matches!(
            self,
            Installation::Wall | Installation::RowEndWall | Installation::RowMiddleWall
        )
    }

    fn covered_sides(self) -> u8 {
        match self {
            Installation::RowEnd | Installation::RowEndWall => 1,
            Installation::RowMiddle | Installation::RowMiddleWall => 2,
            _ => 0,
        }
    }
}

/// The enclosure as the heat balance sees it. Exterior size in mm.
#[derive(Debug, Clone, PartialEq)]
pub struct Enclosure {
    pub width_mm: f64,
    pub height_mm: f64,
    pub depth_mm: f64,
    pub installation: Installation,
    pub top_covered: bool,
    pub material: Material,
    pub ambient_c: f64,
    pub max_internal_c: f64,
    pub altitude_m: f64,
    /// The size is a guess (not entered by the user): results are flagged.
    pub guessed: bool,
}

impl Enclosure {
    /// An enclosure of the given exterior size with the default conditions.
    pub fn new(width_mm: f64, height_mm: f64, depth_mm: f64) -> Enclosure {
        Enclosure {
            width_mm,
            height_mm,
            depth_mm,
            installation: Installation::default(),
            top_covered: false,
            material: Material::default(),
            ambient_c: DEFAULT_AMBIENT_C,
            max_internal_c: DEFAULT_MAX_INTERNAL_C,
            altitude_m: 0.0,
            guessed: false,
        }
    }

    /// A guessed enclosure around a mounting plate: 100 mm more each way,
    /// 250 mm deep. Flagged `guessed`.
    pub fn around_plate(plate: &Plate) -> Enclosure {
        Enclosure {
            guessed: true,
            ..Enclosure::new(plate.width + 100.0, plate.height + 100.0, 250.0)
        }
    }

    /// Effective cooling surface in m2 (IEC 60890 surface factors).
    pub fn effective_area(&self) -> f64 {
        let (w, h, d) = (
            self.width_mm / 1000.0,
            self.height_mm / 1000.0,
            self.depth_mm / 1000.0,
        );
        let top = if self.top_covered { 0.7 } else { 1.4 } * w * d;
        let front = 0.9 * w * h;
        let rear = if self.installation.rear_covered() {
            0.5
        } else {
            0.9
        } * w
            * h;
        let covered = self.installation.covered_sides();
        let side = |covered: bool| if covered { 0.5 } else { 0.9 } * d * h;
        let sides = (0..2).map(|i| side(i < covered)).sum::<f64>();
        top + front + rear + sides
    }
}

/// How sure the result is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The walls shed the heat at the permitted temperature.
    Ok,
    /// Cooling is needed.
    NeedsCooling,
    /// Loss data or size is missing: the numbers are "at least" values.
    Incomplete,
}

/// The planning note shown next to every result.
pub const NOTE: &str = "Planning value. Check with the enclosure or cooling unit maker.";

/// The result of the heat balance.
#[derive(Debug, Clone, PartialEq)]
pub struct Thermal {
    /// Total effective loss of the devices with known loss, W.
    pub loss_w: f64,
    /// Effective cooling surface, m2.
    pub effective_area_m2: f64,
    /// Heat the walls shed at the permitted interior temperature, W.
    pub wall_dissipation_w: f64,
    /// Mean interior temperature rise without cooling, K.
    pub rise_k: f64,
    /// Mean interior temperature without cooling, degrees C.
    pub internal_c: f64,
    /// Cooling power needed to hold the permitted temperature, W.
    pub cooling_w: f64,
    /// Filter-fan airflow for that power, m3/h; `None` when a fan cannot do
    /// it (permitted temperature not above ambient) or no cooling is needed.
    pub airflow_m3h: Option<f64>,
    /// Devices counted and devices without loss data.
    pub known: usize,
    pub missing: usize,
    pub verdict: Verdict,
}

/// Runs the heat balance for the total effective loss `loss_w` of `known`
/// devices; `missing` devices have no loss data.
pub fn balance(enclosure: &Enclosure, loss_w: f64, known: usize, missing: usize) -> Thermal {
    let area = enclosure.effective_area();
    let k = enclosure.material.k();
    let delta = enclosure.max_internal_c - enclosure.ambient_c;
    let wall = k * area * delta;
    let rise = if area > 0.0 { loss_w / (k * area) } else { 0.0 };
    let cooling = (loss_w - wall).max(0.0);
    let airflow = (cooling > 0.0 && delta > 0.0).then(|| {
        let rho = AIR_RHO_SEA * (-enclosure.altitude_m / SCALE_HEIGHT_M).exp();
        3600.0 * cooling / (rho * AIR_CP * delta)
    });
    let verdict = if missing > 0 || enclosure.guessed {
        Verdict::Incomplete
    } else if cooling > 0.0 {
        Verdict::NeedsCooling
    } else {
        Verdict::Ok
    };
    Thermal {
        loss_w,
        effective_area_m2: area,
        wall_dissipation_w: wall,
        rise_k: rise,
        internal_c: enclosure.ambient_c + rise,
        cooling_w: cooling,
        airflow_m3h: airflow,
        known,
        missing,
        verdict,
    }
}

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
/// device's effective loss by id, `None` when unknown.
pub fn sources(placements: &[Placement], loss: impl Fn(u64) -> Option<f64>) -> Vec<HeatSource> {
    placements
        .iter()
        .filter_map(|p| {
            Some(HeatSource {
                id: p.id,
                footprint: p.footprint(),
                loss_w: loss(p.id)?,
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

    /// Total heat in the map, W (conserved by the blur).
    pub fn total_w(&self) -> f64 {
        let dm2 = (self.cell_mm / 100.0).powi(2);
        self.density.iter().sum::<f64>() * dm2
    }
}

/// Builds the heat map: each source spreads its loss over its footprint, then
/// a box blur of `spread_mm` radius shares it with the neighbourhood.
pub fn heat_map(plate: &Plate, sources: &[HeatSource], cell_mm: f64, spread_mm: f64) -> HeatMap {
    let cell = cell_mm.max(1.0);
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
    //! Reads the heat inputs from a project: enclosure properties, part
    //! loss data and the devices at the enclosure's location.

    use novo_core::{Device, Part, Project, Structure, StructureId, Value};

    use super::{Enclosure, Installation, Material};
    use crate::Plate;

    /// Part property keys, the same names as the Parts library uses.
    pub const POWER_LOSS_KEY: &str = "power_loss_w";
    pub const LOAD_FACTOR_KEY: &str = "load_factor";
    /// Enclosure (`Structure`) property keys.
    pub const WIDTH_KEY: &str = "width_mm";
    pub const HEIGHT_KEY: &str = "height_mm";
    pub const DEPTH_KEY: &str = "depth_mm";
    pub const INSTALLATION_KEY: &str = "installation";
    pub const TOP_COVERED_KEY: &str = "top_covered";
    pub const MATERIAL_KEY: &str = "material";
    pub const AMBIENT_KEY: &str = "ambient_c";
    pub const MAX_INTERNAL_KEY: &str = "max_internal_c";
    pub const ALTITUDE_KEY: &str = "altitude_m";

    fn number(value: Option<&Value>) -> Option<f64> {
        match value? {
            Value::Float(v) => Some(*v),
            Value::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// The enclosure with its stored properties; a missing size is guessed
    /// from the plate (and flagged), `None` when there is neither.
    pub fn enclosure(project: &Project, id: StructureId) -> Option<Enclosure> {
        let s = project.get::<Structure>(id)?;
        let p = &s.properties;
        let size = (
            number(p.get(WIDTH_KEY)),
            number(p.get(HEIGHT_KEY)),
            number(p.get(DEPTH_KEY)),
        );
        let mut e = match size {
            (Some(w), Some(h), Some(d)) if w > 0.0 && h > 0.0 && d > 0.0 => Enclosure::new(w, h, d),
            _ => {
                let plate = s.plate?;
                Enclosure::around_plate(&Plate {
                    width: plate.width,
                    height: plate.height,
                    margin: plate.margin,
                })
            }
        };
        let text = |key: &str| match p.get(key) {
            Some(Value::Text(t)) => Some(t.as_str()),
            _ => None,
        };
        if let Some(v) = text(INSTALLATION_KEY).and_then(Installation::parse) {
            e.installation = v;
        }
        if let Some(v) = text(MATERIAL_KEY).and_then(Material::parse) {
            e.material = v;
        }
        if let Some(Value::Bool(v)) = p.get(TOP_COVERED_KEY) {
            e.top_covered = *v;
        }
        if let Some(v) = number(p.get(AMBIENT_KEY)) {
            e.ambient_c = v;
        }
        if let Some(v) = number(p.get(MAX_INTERNAL_KEY)) {
            e.max_internal_c = v;
        }
        if let Some(v) = number(p.get(ALTITUDE_KEY)) {
            e.altitude_m = v;
        }
        Some(e)
    }

    /// The effective loss of a device in W: the first of its parts with a
    /// `power_loss_w`, times that part's `load_factor` (default 1, clamped to
    /// 0..=1). `None` when no part gives a loss.
    pub fn device_loss(project: &Project, device: &Device) -> Option<f64> {
        device.parts.iter().find_map(|part| {
            let part = project.get::<Part>(*part)?;
            let loss = number(part.properties.get(POWER_LOSS_KEY))?;
            let factor = number(part.properties.get(LOAD_FACTOR_KEY)).unwrap_or(1.0);
            Some(loss.max(0.0) * factor.clamp(0.0, 1.0))
        })
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

    /// The heat balance of an enclosure from its project data.
    pub fn estimate(project: &Project, id: StructureId) -> Option<super::Thermal> {
        let enclosure = enclosure(project, id)?;
        let losses = losses(project, id);
        let known = losses.iter().filter(|l| l.1.is_some()).count();
        let total = losses.iter().filter_map(|l| l.1).sum();
        Some(super::balance(
            &enclosure,
            total,
            known,
            losses.len() - known,
        ))
    }
}
