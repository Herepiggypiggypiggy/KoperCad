//! Vent and fan position sweep: tries where openings go and how big they are,
//! ranks the tries by the predicted device temperatures and keeps the best few
//! with before and after numbers.
//!
//! Three stages, all in the 2D model of this crate (a model with planning
//! values, not a measurement):
//!
//! 1. **Positions.** Four kinds of pair (fan in and fan out, vent in and vent
//!    out, vent in and fan out, fan in and vent out) at a nominal size, over a
//!    grid of places on the walls: air in low, out high, in the door, the roof
//!    or a side wall, and the x of the device that is worst off. Coarse grid.
//! 2. **Sizes.** The best places of each kind with other fan flows and vent
//!    areas.
//! 3. **Finalists.** The best of each kind, run again on a finer grid next to
//!    a baseline on the same grid, so before and after are fair. These carry
//!    the numbers that are shown, with the flow each opening really passes.
//!
//! Gains under [`SIGNIFICANT_K`] are flagged as not clear: the 2D solver
//! wobbles by about that much between layouts (see `Result::stalled`).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use novo_layout::sizing::FAN_FACTOR;

use crate::model::*;
use crate::report;
use crate::result::*;
use crate::setup::Error;
use crate::thermal::State;

/// Cell size of the screening runs, mm.
pub const COARSE_CELL_MM: f64 = 40.0;
/// Cell size of the finalist runs, mm.
pub const FINE_CELL_MM: f64 = 25.0;
/// A gain smaller than this, K, is within what the model wobbles by.
pub const SIGNIFICANT_K: f64 = 1.0;
/// A margin above this, K, is enough: options that reach it tie and the
/// cheaper one wins.
const COMFORT_K: f64 = 2.0;
/// A device with less margin than this, K, counts as one to help.
const TROUBLE_MARGIN_K: f64 = 5.0;
/// Fan flows a maker typically lists, m3/h.
pub const FAN_FLOWS_M3H: [f64; 7] = [50.0, 100.0, 150.0, 250.0, 400.0, 600.0, 800.0];
/// Vent areas tried, cm2 of opening.
pub const VENT_AREAS_CM2: [f64; 5] = [100.0, 200.0, 300.0, 500.0, 800.0];
/// Planning value for a fan's shut-off pressure, Pa.
const FAN_PRESSURE_PA: f64 = 60.0;
/// Free share of a louvred vent.
const FREE_FRACTION: f64 = 0.6;
/// Space kept between an opening and the corner of its wall, mm.
const EDGE_MM: f64 = 40.0;

/// What kind of pair of openings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pairing {
    /// Filter fan in, exhaust fan out.
    FanPair,
    /// Vent in, vent out: no power, the warm air rises out by itself.
    VentPair,
    /// Vent in, exhaust fan out.
    VentInFanOut,
    /// Filter fan in, vent out.
    FanInVentOut,
}

impl Pairing {
    pub const ALL: [Pairing; 4] = [
        Pairing::FanPair,
        Pairing::VentPair,
        Pairing::VentInFanOut,
        Pairing::FanInVentOut,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Pairing::FanPair => "Filter fan in, exhaust fan out",
            Pairing::VentPair => "Vent in, vent out (no power)",
            Pairing::VentInFanOut => "Vent in, exhaust fan out",
            Pairing::FanInVentOut => "Filter fan in, vent out",
        }
    }

    fn inlet_is_fan(self) -> bool {
        matches!(self, Pairing::FanPair | Pairing::FanInVentOut)
    }

    fn outlet_is_fan(self) -> bool {
        matches!(self, Pairing::FanPair | Pairing::VentInFanOut)
    }

    /// Number of fans in the pair.
    pub fn fans(self) -> usize {
        usize::from(self.inlet_is_fan()) + usize::from(self.outlet_is_fan())
    }
}

/// One opening of a candidate: a fan (`fan_flow_m3h` set) or a vent.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub opening: Opening,
    /// Free-air flow of the fan, m3/h; `None` for a vent.
    pub fan_flow_m3h: Option<f64>,
}

impl Piece {
    /// Opening area in cm2.
    pub fn area_cm2(&self) -> f64 {
        self.opening.area_m2() * 1.0e4
    }

    fn vent(&self) -> Option<Vent> {
        self.fan_flow_m3h.is_none().then_some(Vent {
            opening: self.opening,
            free_area_fraction: FREE_FRACTION,
        })
    }

    fn fan(&self, mode: FanMode) -> Option<Fan> {
        self.fan_flow_m3h.map(|flow| Fan {
            opening: self.opening,
            mode,
            free_flow_m3h: flow,
            max_pressure_pa: FAN_PRESSURE_PA,
        })
    }
}

/// A pair of openings to add to the cabinet.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub pairing: Pairing,
    pub inlet: Piece,
    pub outlet: Piece,
}

impl Candidate {
    /// The scenario with this pair added to what it already has.
    pub fn apply(&self, scn: &Scenario) -> Scenario {
        let mut out = scn.clone();
        for (piece, mode) in [
            (&self.inlet, FanMode::Intake),
            (&self.outlet, FanMode::Exhaust),
        ] {
            out.vents.extend(piece.vent());
            out.fans.extend(piece.fan(mode));
        }
        out
    }

    /// Rough price of the pair in arbitrary units, to break ties: a fan costs
    /// more than a vent and grows with its flow.
    fn cost(&self) -> f64 {
        [&self.inlet, &self.outlet]
            .iter()
            .map(|p| match p.fan_flow_m3h {
                Some(f) => 3.0 + f / 100.0,
                None => p.area_cm2() / 200.0,
            })
            .sum()
    }

    /// Where the two openings are, in words.
    pub fn describe(&self, e: &Enclosure) -> String {
        format!(
            "{} {}; {} {}",
            kind_word(&self.inlet, "in"),
            where_text(&self.inlet.opening, e),
            kind_word(&self.outlet, "out"),
            where_text(&self.outlet.opening, e),
        )
    }

    fn same_places(&self, other: &Candidate) -> bool {
        self.pairing == other.pairing
            && self.inlet.opening.placement == other.inlet.opening.placement
            && self.outlet.opening.placement == other.outlet.opening.placement
    }
}

fn kind_word(p: &Piece, way: &str) -> String {
    match p.fan_flow_m3h {
        Some(f) => format!("fan {way} ({f:.0} m³/h)"),
        None => format!(
            "vent {way} ({:.0} cm² opening, {:.0} % free)",
            p.area_cm2(),
            100.0 * FREE_FRACTION
        ),
    }
}

/// "door, 30 % from the left, 150 mm above the floor".
pub fn where_text(o: &Opening, e: &Enclosure) -> String {
    let (w, h) = (e.interior_width_mm(), e.interior_height_mm());
    let pct = |v: f64, len: f64| (100.0 * v / len).round() as i64;
    match o.placement {
        Placement::Top { x_mm } => format!("roof, {} % from the left", pct(x_mm, w)),
        Placement::Bottom { x_mm } => format!("floor, {} % from the left", pct(x_mm, w)),
        Placement::Left { y_mm } => {
            format!("left wall, {:.0} mm above the floor", (h - y_mm).max(0.0))
        }
        Placement::Right { y_mm } => {
            format!("right wall, {:.0} mm above the floor", (h - y_mm).max(0.0))
        }
        Placement::Front { x_mm, y_mm } => format!(
            "door, {} % from the left, {:.0} mm above the floor",
            pct(x_mm, w),
            (h - y_mm).max(0.0)
        ),
    }
}

/// The numbers of one run that the sweep compares.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// Smallest margin to a limit over all devices, K; `None` when no device
    /// has a limit.
    pub worst_margin_k: Option<f64>,
    /// Devices over a limit.
    pub over: usize,
    pub max_air_c: f64,
    pub mean_air_c: f64,
    /// Margin per device in scenario order.
    pub margins_k: Vec<Option<f64>>,
    /// Housing surface temperature per device, degrees C.
    pub surface_c: Vec<f64>,
    /// The solver did not settle fully: the numbers may move by about 1 K.
    pub unsettled: bool,
}

impl Reading {
    fn of(r: &ThermalResult) -> Reading {
        let margins_k: Vec<Option<f64>> = r.devices.iter().map(|d| d.margin_k).collect();
        Reading {
            worst_margin_k: margins_k
                .iter()
                .flatten()
                .copied()
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v)))),
            over: margins_k.iter().flatten().filter(|m| **m < 0.0).count(),
            max_air_c: r.balance.max_air_c,
            mean_air_c: r.balance.mean_air_c,
            surface_c: r.devices.iter().map(|d| d.surface_c).collect(),
            margins_k,
            unsettled: !r.converged || r.stalled,
        }
    }

    /// How much better `self` is than `before`: the tightest margin gained,
    /// or the hottest air lost when no device has a limit. K.
    pub fn gain_over(&self, before: &Reading) -> f64 {
        match (before.worst_margin_k, self.worst_margin_k) {
            (Some(b), Some(a)) => a - b,
            _ => before.max_air_c - self.max_air_c,
        }
    }
}

/// What one device wins.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceChange {
    pub id: u64,
    pub surface_before_c: f64,
    pub surface_after_c: f64,
    pub margin_before_k: Option<f64>,
    pub margin_after_k: Option<f64>,
}

/// Flow an opening really passes in the finalist run.
#[derive(Debug, Clone, PartialEq)]
pub struct Delivered {
    pub is_fan: bool,
    /// Into the cabinet is positive, m3/h.
    pub flow_m3h: f64,
    pub speed_ms: f64,
}

/// A candidate with its numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub candidate: Candidate,
    pub reading: Reading,
    /// Gain over the baseline on the same grid, K.
    pub gain_k: f64,
    /// Gain is at least [`SIGNIFICANT_K`].
    pub clear: bool,
    /// The devices that were worst off before, with their change (finalists
    /// only).
    pub devices: Vec<DeviceChange>,
    /// What the inlet and outlet pass (finalists only).
    pub inlet: Option<Delivered>,
    pub outlet: Option<Delivered>,
    /// 1 is the best.
    pub rank: usize,
}

/// The finished sweep.
#[derive(Debug, Clone, PartialEq)]
pub struct Sweep {
    /// The cabinet as it is, on the finalist grid.
    pub before: Reading,
    /// Best first; the best of each kind, at most `Settings::show`.
    pub best: Vec<Outcome>,
    /// Layouts run in all.
    pub tried: usize,
    /// Candidates the model could not build (an opening outside its wall).
    pub skipped: usize,
    /// Flow the sizing rule asks for to carry all the loss, m3/h.
    pub sizing_flow_m3h: f64,
    /// Stopped early: `best` is the best so far.
    pub partial: bool,
    /// Plain-language caveats, always including the model label.
    pub notes: Vec<String>,
}

impl Sweep {
    /// The best outcome, if any gain is clear.
    pub fn winner(&self) -> Option<&Outcome> {
        self.best.first().filter(|o| o.clear)
    }
}

/// What to try.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub fan_flows_m3h: Vec<f64>,
    pub vent_areas_cm2: Vec<f64>,
    /// Places along the width to try, as shares of it. The x of the device
    /// that is worst off is always added.
    pub columns: Vec<f64>,
    /// How many to show.
    pub show: usize,
    /// Sweep even when no device is short of margin.
    pub always: bool,
}

impl Settings {
    /// A small ladder, for tests and a first look: a few seconds.
    pub fn quick() -> Settings {
        Settings {
            fan_flows_m3h: vec![250.0, 600.0],
            vent_areas_cm2: vec![300.0],
            columns: vec![0.5],
            show: 4,
            always: false,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            fan_flows_m3h: FAN_FLOWS_M3H.to_vec(),
            vent_areas_cm2: VENT_AREAS_CM2.to_vec(),
            columns: vec![0.2, 0.5, 0.8],
            show: 4,
            always: false,
        }
    }
}

/// Progress and cancel, shared with the thread that shows the sweep.
#[derive(Debug, Default)]
pub struct Control {
    pub cancel: AtomicBool,
    /// Layouts run so far.
    pub evaluated: AtomicUsize,
    /// Layouts planned so far (grows when a stage is planned).
    pub planned: AtomicUsize,
}

impl Control {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// Sweeps with the default settings and no way to stop it.
pub fn sweep(scn: &Scenario) -> Result<Sweep, Error> {
    sweep_with(scn, &Settings::default(), &Control::default())
}

/// A run on `cell_mm` of the scenario, warm-started from `init`.
fn run(
    scn: &Scenario,
    cell_mm: f64,
    init: Option<&State>,
) -> Result<(Reading, ThermalResult, State), Error> {
    let mut s = scn.clone();
    s.params.cell_mm = s.params.cell_mm.max(cell_mm);
    let (setup, st) = crate::run_steady_from(&s, init)?;
    let r = report::build(&s, &setup, &st, 1.0, None);
    Ok((Reading::of(&r), r, st))
}

/// Runs `f` on every item with a few threads; items not started when the
/// sweep is stopped come back `None`.
fn par<T: Sync, R: Send>(
    items: &[T],
    control: &Control,
    f: impl Fn(&T) -> R + Sync,
) -> Vec<Option<R>> {
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .clamp(1, 8)
        .min(items.len().max(1));
    let mut out: Vec<Option<R>> = Vec::new();
    out.resize_with(items.len(), || None);
    let results = std::sync::Mutex::new(out);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    if control.cancelled() {
                        return;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else {
                        return;
                    };
                    let r = f(item);
                    control.evaluated.fetch_add(1, Ordering::Relaxed);
                    results.lock().expect("results lock")[i] = Some(r);
                }
            });
        }
    });
    results.into_inner().expect("results lock")
}

/// A try with its screening numbers.
struct Tried {
    candidate: Candidate,
    reading: Reading,
}

/// Better first: fewer devices over, then the tightest margin (in half-kelvin
/// steps, so a wobble does not decide), then the cheaper pair, then the
/// cooler air.
fn order(a: &(Reading, f64), b: &(Reading, f64)) -> std::cmp::Ordering {
    let step = |r: &Reading| match r.worst_margin_k {
        Some(m) => -((m.min(COMFORT_K) * 2.0).round() as i64),
        None => ((r.max_air_c * 2.0).round()) as i64,
    };
    a.0.over
        .cmp(&b.0.over)
        .then(step(&a.0).cmp(&step(&b.0)))
        .then(a.1.total_cmp(&b.1))
        .then(a.0.max_air_c.total_cmp(&b.0.max_air_c))
}

fn order_tried(a: &Tried, b: &Tried) -> std::cmp::Ordering {
    order(
        &(a.reading.clone(), a.candidate.cost()),
        &(b.reading.clone(), b.candidate.cost()),
    )
}

/// Where an opening could go.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Spot {
    DoorLow(f64),
    DoorHigh(f64),
    Floor(f64),
    Roof(f64),
    LeftLow,
    RightLow,
    LeftHigh,
    RightHigh,
}

/// Fan cut-out side for a flow, mm (as the cooling advice uses).
fn fan_side_mm(flow: f64) -> f64 {
    match flow as u32 {
        0..=100 => 92.0,
        101..=250 => 120.0,
        251..=400 => 150.0,
        _ => 200.0,
    }
}

/// Width and height of a vent of `area_cm2` at `spot`, kept inside its wall.
fn vent_size_mm(area_cm2: f64, spot: Spot, e: &Enclosure) -> (f64, f64) {
    let area = area_cm2 * 100.0;
    let (w, h, d) = (
        e.interior_width_mm(),
        e.interior_height_mm(),
        e.interior_depth_mm(),
    );
    // (length along the wall, other side limit) per wall.
    let (along_max, other_max) = match spot {
        Spot::DoorLow(_) | Spot::DoorHigh(_) => (0.8 * w, 0.4 * h),
        Spot::Floor(_) | Spot::Roof(_) => (0.8 * w, 0.6 * d),
        _ => (0.8 * h, 0.6 * d),
    };
    let mut wide = (3.0 * area).sqrt().min(along_max);
    let mut high = (area / wide).min(other_max);
    wide = (area / high).min(along_max);
    high = (area / wide).min(other_max);
    (wide.max(20.0), high.max(20.0))
}

/// The opening for a size at a spot.
fn opening_at(spot: Spot, wide: f64, high: f64, e: &Enclosure) -> Opening {
    let (w, h) = (e.interior_width_mm(), e.interior_height_mm());
    let x = |c: f64| {
        c.clamp(
            0.5 * wide + EDGE_MM,
            (w - 0.5 * wide - EDGE_MM).max(0.5 * w),
        )
    };
    // Vertical extent of a side wall opening is its width_mm.
    let low_y = |extent: f64| h - 0.5 * extent - EDGE_MM;
    let high_y = |extent: f64| 0.5 * extent + EDGE_MM;
    let (placement, width_mm, height_mm) = match spot {
        Spot::DoorLow(c) => (
            Placement::Front {
                x_mm: x(c),
                y_mm: low_y(high),
            },
            wide,
            high,
        ),
        Spot::DoorHigh(c) => (
            Placement::Front {
                x_mm: x(c),
                y_mm: high_y(high),
            },
            wide,
            high,
        ),
        Spot::Floor(c) => (Placement::Bottom { x_mm: x(c) }, wide, high),
        Spot::Roof(c) => (Placement::Top { x_mm: x(c) }, wide, high),
        Spot::LeftLow => (Placement::Left { y_mm: low_y(wide) }, wide, high),
        Spot::RightLow => (Placement::Right { y_mm: low_y(wide) }, wide, high),
        Spot::LeftHigh => (Placement::Left { y_mm: high_y(wide) }, wide, high),
        Spot::RightHigh => (Placement::Right { y_mm: high_y(wide) }, wide, high),
    };
    Opening {
        placement,
        width_mm,
        height_mm,
    }
}

fn piece(spot: Spot, fan_flow_m3h: Option<f64>, area_cm2: f64, e: &Enclosure) -> Piece {
    let (wide, high) = match fan_flow_m3h {
        Some(f) => {
            let side = fan_side_mm(f).min(0.8 * e.interior_depth_mm());
            (side, side)
        }
        None => vent_size_mm(area_cm2, spot, e),
    };
    Piece {
        opening: opening_at(spot, wide, high, e),
        fan_flow_m3h,
    }
}

/// x positions to try along the width: spread, and over the device that is
/// worst off.
fn columns(e: &Enclosure, shares: &[f64], hot_x: Option<f64>) -> Vec<f64> {
    let w = e.interior_width_mm();
    let mut xs: Vec<f64> = shares.iter().map(|s| s * w).collect();
    if xs.is_empty() {
        xs.push(0.5 * w);
    }
    if let Some(h) = hot_x
        && xs.iter().all(|x| (x - h).abs() > 0.08 * w)
    {
        xs.push(h);
    }
    xs
}

/// The spots for air coming in (low) and going out (high).
fn spots(e: &Enclosure, shares: &[f64], hot_x: Option<f64>) -> (Vec<Spot>, Vec<Spot>) {
    let xs = columns(e, shares, hot_x);
    let mut ins: Vec<Spot> = xs.iter().map(|x| Spot::DoorLow(*x)).collect();
    let w = e.interior_width_mm();
    ins.extend(
        [0.3 * w, 0.7 * w]
            .into_iter()
            .chain(hot_x)
            .filter(|_| xs.len() > 1)
            .map(Spot::Floor),
    );
    if xs.len() == 1 {
        ins.push(Spot::Floor(xs[0]));
    }
    ins.extend([Spot::LeftLow, Spot::RightLow]);
    let mut outs: Vec<Spot> = xs.iter().map(|x| Spot::Roof(*x)).collect();
    outs.extend(
        [0.5 * e.interior_width_mm()]
            .into_iter()
            .chain(hot_x)
            .map(Spot::DoorHigh),
    );
    outs.extend([Spot::LeftHigh, Spot::RightHigh]);
    (ins, outs)
}

fn make(
    pairing: Pairing,
    i: Spot,
    o: Spot,
    fan_flow: f64,
    area_cm2: f64,
    e: &Enclosure,
) -> Candidate {
    Candidate {
        pairing,
        inlet: piece(i, pairing.inlet_is_fan().then_some(fan_flow), area_cm2, e),
        outlet: piece(o, pairing.outlet_is_fan().then_some(fan_flow), area_cm2, e),
    }
}

/// The device to line the openings up with: the one with the least margin,
/// else the one that loses most. Centre x in the interior frame.
fn hot_column(scn: &Scenario, base: &Reading) -> Option<f64> {
    let by_margin = base
        .margins_k
        .iter()
        .enumerate()
        .filter_map(|(i, m)| m.map(|m| (i, m)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i);
    let index = by_margin.or_else(|| {
        scn.devices
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.loss_w.total_cmp(&b.1.loss_w))
            .map(|(i, _)| i)
    })?;
    let f = scn.devices[index].footprint;
    Some(f.x + 0.5 * f.w)
}

/// Flow the sizing rule asks for, m3/h.
fn sizing_flow(scn: &Scenario, base: &Reading) -> f64 {
    let total: f64 = scn.devices.iter().map(|d| d.loss_w).sum::<f64>()
        + scn.obstacles.iter().map(|o| o.loss_w).sum::<f64>();
    let limit = scn
        .devices
        .iter()
        .zip(&base.margins_k)
        .filter(|(_, m)| m.is_some_and(|m| m < TROUBLE_MARGIN_K))
        .filter_map(|(d, _)| d.rated_max_ambient_c)
        .fold(f64::INFINITY, f64::min);
    let allowed = if limit.is_finite() {
        (limit - scn.ambient_c - 2.0).max(5.0)
    } else {
        10.0
    };
    FAN_FACTOR * total / allowed
}

/// Runs the sweep. `control` can stop it (the best so far is returned) and
/// shows progress.
pub fn sweep_with(scn: &Scenario, settings: &Settings, control: &Control) -> Result<Sweep, Error> {
    let e = &scn.enclosure;
    let (base_reading, _, base_state) = run(scn, COARSE_CELL_MM, None)?;
    let mut notes = vec![
        "Model estimate: the 2D heat model with planning values, the losses you entered and the wire heat in the ducts. It has not been checked against measurements, and the 2D slice does not see the depth of the cabinet.".to_string(),
        "This is a screening: outlet and inlet places are searched one after the other, so it finds a good option, not necessarily the best possible one.".to_string(),
        "Fans run on a straight-line curve from the free-air flow to a planning shut-off pressure of 60 Pa; use a maker's curve before you buy. The mat of a filter fan is not modelled (a mat typically takes about 30 % off the flow), so size a filter fan up.".to_string(),
    ];
    let sizing_flow_m3h = sizing_flow(scn, &base_reading);
    let trouble = base_reading
        .worst_margin_k
        .is_none_or(|m| m < TROUBLE_MARGIN_K);
    if !trouble && !settings.always {
        notes.push(format!(
            "Every device is at least {TROUBLE_MARGIN_K:.0} K inside its limit, so there is nothing to fix. Nothing was swept."
        ));
        return Ok(Sweep {
            before: base_reading,
            best: Vec::new(),
            tried: 0,
            skipped: 0,
            sizing_flow_m3h,
            partial: false,
            notes,
        });
    }

    let hot_x = hot_column(scn, &base_reading);
    let (ins, outs) = spots(e, &settings.columns, hot_x);
    let nominal_flow = settings
        .fan_flows_m3h
        .iter()
        .copied()
        .find(|f| *f >= sizing_flow_m3h)
        .or_else(|| settings.fan_flows_m3h.last().copied())
        .unwrap_or(250.0);
    let nominal_area = settings
        .vent_areas_cm2
        .iter()
        .copied()
        .find(|a| *a >= 300.0)
        .or_else(|| settings.vent_areas_cm2.last().copied())
        .unwrap_or(300.0);

    // Stage 1: places at a nominal size. Outlet and inlet are searched one
    // after the other (every outlet with a reference inlet, then every inlet
    // with the best outlet), then the best few of each are tried together.
    let mut skipped = 0;
    let screen = |cands: Vec<Candidate>, skipped: &mut usize| -> Vec<Tried> {
        control.planned.fetch_add(cands.len(), Ordering::Relaxed);
        let runs = par(&cands, control, |c| {
            run(&c.apply(scn), COARSE_CELL_MM, Some(&base_state)).map(|r| r.0)
        });
        let mut out = Vec::new();
        for (c, r) in cands.into_iter().zip(runs) {
            match r {
                Some(Ok(reading)) => out.push(Tried {
                    candidate: c,
                    reading,
                }),
                Some(Err(_)) => *skipped += 1,
                None => {}
            }
        }
        out
    };
    let reference_in = ins
        .first()
        .copied()
        .unwrap_or(Spot::DoorLow(0.5 * e.interior_width_mm()));
    let mut all: Vec<Tried> = Vec::new();
    let mut outs_tried = screen(
        Pairing::ALL
            .iter()
            .flat_map(|p| {
                outs.iter()
                    .map(|o| make(*p, reference_in, *o, nominal_flow, nominal_area, e))
            })
            .collect(),
        &mut skipped,
    );
    let best_outs: Vec<Vec<Spot>> = Pairing::ALL
        .iter()
        .map(|p| top_spots(&outs_tried, *p, 2, |c| &c.outlet.opening, e))
        .collect();
    let mut ins_tried = screen(
        Pairing::ALL
            .iter()
            .zip(&best_outs)
            .flat_map(|(p, bo)| {
                let o = bo.first().copied().unwrap_or(outs[0]);
                ins.iter().map(move |i| (*p, *i, o)).collect::<Vec<_>>()
            })
            .map(|(p, i, o)| make(p, i, o, nominal_flow, nominal_area, e))
            .collect(),
        &mut skipped,
    );
    let mut both: Vec<Candidate> = Vec::new();
    for (p, bo) in Pairing::ALL.iter().zip(&best_outs) {
        for i in top_spots(&ins_tried, *p, 2, |c| &c.inlet.opening, e) {
            for o in bo {
                both.push(make(*p, i, *o, nominal_flow, nominal_area, e));
            }
        }
    }
    all.append(&mut outs_tried);
    all.append(&mut ins_tried);
    both.retain(|c| {
        !all.iter()
            .any(|t| t.candidate.same_places(c) && same_size(&t.candidate, c))
    });
    dedupe(&mut both);
    all.extend(screen(both, &mut skipped));

    // Stage 2: sizes at the best places of each kind.
    let seeds = best_per_pairing(&all, 2);
    let mut stage2: Vec<Candidate> = Vec::new();
    for seed in &seeds {
        let (i, o) = (
            spot_of(&seed.candidate.inlet.opening, e),
            spot_of(&seed.candidate.outlet.opening, e),
        );
        let p = seed.candidate.pairing;
        let has_fan = p.fans() > 0;
        let has_vent = p.fans() < 2;
        if has_fan {
            for f in &settings.fan_flows_m3h {
                stage2.push(make(
                    p,
                    i,
                    o,
                    *f,
                    vent_area_of(&seed.candidate, nominal_area),
                    e,
                ));
            }
        }
        if has_vent {
            for a in &settings.vent_areas_cm2 {
                stage2.push(make(
                    p,
                    i,
                    o,
                    fan_flow_of(&seed.candidate, nominal_flow),
                    *a,
                    e,
                ));
            }
        }
    }
    stage2.retain(|c| {
        !all.iter()
            .any(|t| t.candidate.same_places(c) && same_size(&t.candidate, c))
    });
    dedupe(&mut stage2);
    control.planned.fetch_add(stage2.len(), Ordering::Relaxed);
    all.extend(screen(stage2, &mut skipped));

    // Stage 3: the best of each kind on the finer grid, next to a baseline.
    all.sort_by(order_tried);
    let mut finalists: Vec<Candidate> = best_per_pairing(&all, 1)
        .into_iter()
        .map(|t| t.candidate.clone())
        .collect();
    for t in &all {
        if finalists.len() >= settings.show + 2 {
            break;
        }
        if !finalists.contains(&t.candidate) {
            finalists.push(t.candidate.clone());
        }
    }
    control
        .planned
        .fetch_add(finalists.len() + 1, Ordering::Relaxed);
    let (fine_base, fine_base_result, fine_state) = run(scn, FINE_CELL_MM, None)?;
    control.evaluated.fetch_add(1, Ordering::Relaxed);
    let runs = par(&finalists, control, |c| {
        run(&c.apply(scn), FINE_CELL_MM, Some(&fine_state)).map(|(r, full, _)| (r, full))
    });
    let mut outcomes: Vec<Outcome> = Vec::new();
    for (c, r) in finalists.into_iter().zip(runs) {
        let Some(Ok((reading, full))) = r else {
            if matches!(r, Some(Err(_))) {
                skipped += 1;
            }
            continue;
        };
        let gain_k = reading.gain_over(&fine_base);
        let delivered = |index: usize, is_fan: bool| {
            full.openings
                .iter()
                .find(|f| f.is_fan == is_fan && f.index == index)
                .map(|f| Delivered {
                    is_fan,
                    flow_m3h: f.flow_m3h,
                    speed_ms: f.speed_ms,
                })
        };
        // `apply` appends the inlet's opening before the outlet's.
        let (inlet, outlet) = {
            let (v0, f0) = (scn.vents.len(), scn.fans.len());
            match c.pairing {
                Pairing::FanPair => (delivered(f0, true), delivered(f0 + 1, true)),
                Pairing::VentPair => (delivered(v0, false), delivered(v0 + 1, false)),
                Pairing::VentInFanOut => (delivered(v0, false), delivered(f0, true)),
                Pairing::FanInVentOut => (delivered(f0, true), delivered(v0, false)),
            }
        };
        outcomes.push(Outcome {
            devices: device_changes(&fine_base_result, &full, &fine_base, &reading),
            candidate: c,
            clear: gain_k >= SIGNIFICANT_K,
            reading,
            gain_k,
            inlet,
            outlet,
            rank: 0,
        });
    }
    outcomes.sort_by(|a, b| {
        order(
            &(a.reading.clone(), a.candidate.cost()),
            &(b.reading.clone(), b.candidate.cost()),
        )
    });
    // The best of each kind first, so passive and powered options both show.
    let mut best: Vec<Outcome> = Vec::new();
    let mut rest: Vec<Outcome> = Vec::new();
    for o in outcomes {
        if best
            .iter()
            .any(|b| b.candidate.pairing == o.candidate.pairing)
        {
            rest.push(o);
        } else {
            best.push(o);
        }
    }
    best.extend(rest);
    best.truncate(settings.show);
    for (i, o) in best.iter_mut().enumerate() {
        o.rank = i + 1;
    }

    if best.iter().all(|o| !o.clear) {
        notes.push(format!(
            "No position or size of a vent or fan pair wins {SIGNIFICANT_K:.0} K or more in this model. Under that the 2D model cannot tell layouts apart. The heat probably needs a bigger cabinet, a cooling unit or devices with less loss."
        ));
    }
    if fine_base.unsettled || best.iter().any(|o| o.reading.unsettled) {
        notes.push(
            "At least one run did not settle fully; its numbers may move by about 1 K.".into(),
        );
    }
    let partial = control.cancelled();
    if partial {
        notes.push("The sweep was stopped; this is the best found so far.".into());
    }
    Ok(Sweep {
        before: fine_base,
        best,
        tried: control.evaluated.load(Ordering::Relaxed),
        skipped,
        sizing_flow_m3h,
        partial,
        notes,
    })
}

fn fan_flow_of(c: &Candidate, default: f64) -> f64 {
    c.inlet
        .fan_flow_m3h
        .or(c.outlet.fan_flow_m3h)
        .unwrap_or(default)
}

fn vent_area_of(c: &Candidate, default: f64) -> f64 {
    [&c.inlet, &c.outlet]
        .into_iter()
        .find(|p| p.fan_flow_m3h.is_none())
        .map_or(default, Piece::area_cm2)
}

fn same_size(a: &Candidate, b: &Candidate) -> bool {
    let near = |x: f64, y: f64| (x - y).abs() < 1.0;
    let size = |c: &Candidate| (c.inlet.area_cm2(), c.outlet.area_cm2());
    near(size(a).0, size(b).0)
        && near(size(a).1, size(b).1)
        && a.inlet.fan_flow_m3h == b.inlet.fan_flow_m3h
        && a.outlet.fan_flow_m3h == b.outlet.fan_flow_m3h
}

fn dedupe(list: &mut Vec<Candidate>) {
    let mut seen: Vec<Candidate> = Vec::new();
    list.retain(|c| {
        if seen.iter().any(|s| s.same_places(c) && same_size(s, c)) {
            false
        } else {
            seen.push(c.clone());
            true
        }
    });
}

/// The `n` best tries of each kind, best first within a kind, kinds in
/// order.
fn best_per_pairing(all: &[Tried], n: usize) -> Vec<&Tried> {
    let mut out = Vec::new();
    for p in Pairing::ALL {
        let mut of: Vec<&Tried> = all.iter().filter(|t| t.candidate.pairing == p).collect();
        of.sort_by(|a, b| order_tried(a, b));
        out.extend(of.into_iter().take(n));
    }
    out
}

/// The `n` best distinct spots of one kind, taken from `pick` of each try.
fn top_spots(
    all: &[Tried],
    p: Pairing,
    n: usize,
    pick: impl Fn(&Candidate) -> &Opening,
    e: &Enclosure,
) -> Vec<Spot> {
    let mut of: Vec<&Tried> = all.iter().filter(|t| t.candidate.pairing == p).collect();
    of.sort_by(|a, b| order_tried(a, b));
    let mut out: Vec<Spot> = Vec::new();
    for t in of {
        let spot = spot_of(pick(&t.candidate), e);
        if !out.contains(&spot) {
            out.push(spot);
        }
        if out.len() >= n {
            break;
        }
    }
    out
}

/// The spot an opening was made from (to build the same place again at
/// another size).
fn spot_of(o: &Opening, e: &Enclosure) -> Spot {
    let h = e.interior_height_mm();
    match o.placement {
        Placement::Top { x_mm } => Spot::Roof(x_mm),
        Placement::Bottom { x_mm } => Spot::Floor(x_mm),
        Placement::Left { y_mm } => {
            if y_mm > 0.5 * h {
                Spot::LeftLow
            } else {
                Spot::LeftHigh
            }
        }
        Placement::Right { y_mm } => {
            if y_mm > 0.5 * h {
                Spot::RightLow
            } else {
                Spot::RightHigh
            }
        }
        Placement::Front { x_mm, y_mm } => {
            if y_mm > 0.5 * h {
                Spot::DoorLow(x_mm)
            } else {
                Spot::DoorHigh(x_mm)
            }
        }
    }
}

/// The four devices that were worst off before, and what they gain.
fn device_changes(
    before: &ThermalResult,
    after: &ThermalResult,
    rb: &Reading,
    ra: &Reading,
) -> Vec<DeviceChange> {
    let mut idx: Vec<usize> = (0..before.devices.len()).collect();
    idx.sort_by(|a, b| {
        let m = |i: usize| rb.margins_k[i].unwrap_or(f64::MAX);
        m(*a).total_cmp(&m(*b)).then(
            before.devices[*b]
                .surface_c
                .total_cmp(&before.devices[*a].surface_c),
        )
    });
    idx.into_iter()
        .take(4)
        .map(|i| DeviceChange {
            id: before.devices[i].id,
            surface_before_c: before.devices[i].surface_c,
            surface_after_c: after.devices[i].surface_c,
            margin_before_k: rb.margins_k[i],
            margin_after_k: ra.margins_k[i],
        })
        .collect()
}
