//! Low-frequency magnetic field estimate: the field of the drawn currents
//! along their routes, the field where a sensitive device sits, and the
//! voltage that field induces in a sensitive run (rules EMC-19 and EMC-20,
//! `docs/EMC.md` section 4, level 2 with a Biot-Savart core).
//!
//! What it is. Every wire with a known current is a chain of straight
//! segments at the wire height above the plate. The field is the sum of the
//! Biot-Savart field of those segments, each with a complex current (RMS
//! amplitude and phase), so a three-phase cable cancels itself and a lone
//! conductor does not. The flux through a victim loop is found without
//! sampling an area: the loop integral of the vector potential of the same
//! segments (Neumann), exact for straight pieces up to the numerical
//! quadrature along the loop. The plate is optionally a perfect conductor
//! (image current of opposite sign below it), which is the right picture for
//! steep edges and a poor one for 50 Hz in a thin plate.
//!
//! What it is not. A near-field model of the differential-mode current you
//! stated: no common-mode current, no capacitive coupling, no eddy currents
//! in a steel plate at mains frequency, no screening of the door or cabinet,
//! no riser wires between plate and device. It is good for the order of
//! magnitude and for the ranking of layouts, never for a compliance
//! statement. Every number carries [`Level::Estimate`].

use std::collections::HashMap;

use crate::class::EmcClass;
use crate::geometry;
use crate::input::{Input, P, Role, Run};
use crate::report::*;

const MU0: f64 = 4.0e-7 * std::f64::consts::PI;
type V3 = [f64; 3];

/// A current in one wire: RMS amplitude and phase at the model frequency.
#[derive(Debug, Clone, PartialEq)]
pub struct Current {
    pub run: u64,
    /// RMS, A.
    pub amps: f64,
    /// Degrees against the first phase; 0 and 180 for go and return,
    /// 0, -120 and 120 for a three-phase set.
    pub phase_deg: f64,
    /// Where the number came from, in a few words.
    pub source: String,
    /// Set by the user (`emc.current_a`), not read from a rating.
    pub declared: bool,
    /// The phase was read from a pin, a potential or the core order. When
    /// false the wire is taken as a lone conductor (worst case).
    pub phase_known: bool,
}

/// The settings of the estimate; every one can be set in the project.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldParams {
    /// Frequency of the currents, Hz (`emc.field_hz`).
    pub hz: f64,
    /// Height above the plate of the field slice, mm (`emc.field_slice_mm`).
    pub slice_mm: f64,
    /// Planning value for the flux density at a sensitive device, microtesla
    /// (`emc.field_limit_ut`).
    pub limit_ut: f64,
    /// Planning value for the voltage induced in a signal loop of the most
    /// sensitive class, millivolt (`emc.induced_limit_mv`).
    pub induced_limit_mv: f64,
    /// How much a twisted pair keeps of the flux, 0 to 1
    /// (`emc.twist_factor`).
    pub twist_factor: f64,
    /// The plate acts as a perfect conductor for the 50 Hz style estimate
    /// (`emc.plate_image`). `None`: on from 1 kHz up.
    pub plate_image: Option<bool>,
    /// Names of the fields the project set.
    pub user_set: Vec<&'static str>,
}

impl Default for FieldParams {
    fn default() -> FieldParams {
        FieldParams {
            hz: 50.0,
            slice_mm: 30.0,
            // 10 A/m, the middle level of the power-frequency field test.
            limit_ut: 12.6,
            induced_limit_mv: 10.0,
            twist_factor: 0.1,
            plate_image: None,
            user_set: Vec::new(),
        }
    }
}

impl FieldParams {
    /// Whether the plate counts as a conducting plane at the model frequency.
    pub fn image(&self) -> bool {
        self.plate_image.unwrap_or(self.hz >= 1000.0)
    }
}

/// Currents and settings the field rules read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FieldSetup {
    pub currents: Vec<Current>,
    pub params: FieldParams,
}

// ---- Biot-Savart -----------------------------------------------------------

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// A straight piece of conductor with a complex current, metres and amperes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    pub a: V3,
    pub b: V3,
    pub re: f64,
    pub im: f64,
}

/// Flux density per ampere of a straight segment at `p`, tesla per ampere.
/// Exact for a finite segment; `r_min` (m) keeps a point on the wire finite.
pub fn b_segment(a: V3, b: V3, p: V3, r_min: f64) -> V3 {
    let ab = sub(b, a);
    let len = norm(ab);
    if len < 1e-9 {
        return [0.0; 3];
    }
    let l = [ab[0] / len, ab[1] / len, ab[2] / len];
    let r = sub(p, a);
    let s = dot(r, l);
    let rho = [r[0] - s * l[0], r[1] - s * l[1], r[2] - s * l[2]];
    let d = norm(rho).max(r_min);
    // Direction of the field: l x rho_hat; for a point on the axis any
    // perpendicular will do, the magnitude is clamped anyway.
    let rho_hat = if norm(rho) < 1e-12 {
        let pick = if l[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let c = cross(l, pick);
        let n = norm(c);
        [c[0] / n, c[1] / n, c[2] / n]
    } else {
        [rho[0] / norm(rho), rho[1] / norm(rho), rho[2] / norm(rho)]
    };
    let dir = cross(l, rho_hat);
    let k = MU0 / (4.0 * std::f64::consts::PI * d)
        * ((len - s) / ((len - s).powi(2) + d * d).sqrt() + s / (s * s + d * d).sqrt());
    [dir[0] * k, dir[1] * k, dir[2] * k]
}

/// Vector potential per ampere of a straight segment at `p` along the
/// segment's own direction, weber per metre per ampere, and that direction.
fn a_segment(a: V3, b: V3, p: V3, r_min: f64) -> (f64, V3) {
    let ab = sub(b, a);
    let len = norm(ab);
    if len < 1e-9 {
        return (0.0, [0.0; 3]);
    }
    let l = [ab[0] / len, ab[1] / len, ab[2] / len];
    let r = sub(p, a);
    let s = dot(r, l);
    let rho2 = dot(r, r) - s * s;
    let d = rho2.max(0.0).sqrt().max(r_min);
    let k = MU0 / (4.0 * std::f64::consts::PI) * (((len - s) / d).asinh() + (s / d).asinh());
    (k, l)
}

/// The field phasor at `p`: real and imaginary parts as vectors, tesla.
pub fn b_at(p: V3, sources: &[Source], r_min: f64) -> (V3, V3) {
    let (mut re, mut im) = ([0.0; 3], [0.0; 3]);
    for s in sources {
        let g = b_segment(s.a, s.b, p, r_min);
        for k in 0..3 {
            re[k] += s.re * g[k];
            im[k] += s.im * g[k];
        }
    }
    (re, im)
}

/// RMS flux density at `p`, tesla.
pub fn b_rms(p: V3, sources: &[Source], r_min: f64) -> f64 {
    let (re, im) = b_at(p, sources, r_min);
    (dot(re, re) + dot(im, im)).sqrt()
}

const GL_X: [f64; 4] = [
    0.183_434_642_495_649_8,
    0.525_532_409_916_329,
    0.796_666_477_413_626_7,
    0.960_289_856_497_536_3,
];
const GL_W: [f64; 4] = [
    0.362_683_783_378_362,
    0.313_706_645_877_887_3,
    0.222_381_034_453_374_5,
    0.101_228_536_290_376_3,
];

/// Flux through the closed polyline `ring` (metres, closed implicitly):
/// the loop integral of the vector potential of `sources`, weber phasor.
pub fn flux(ring: &[V3], sources: &[Source], r_min: f64) -> (f64, f64) {
    let (mut re, mut im) = (0.0, 0.0);
    let n = ring.len();
    for i in 0..n {
        let (p, q) = (ring[i], ring[(i + 1) % n]);
        let d = sub(q, p);
        let len = norm(d);
        if len < 1e-9 {
            continue;
        }
        let pieces = (len / 0.025).ceil().max(1.0) as usize;
        for k in 0..pieces {
            let t0 = k as f64 / pieces as f64;
            let t1 = (k + 1) as f64 / pieces as f64;
            for (x, w) in GL_X.iter().zip(GL_W) {
                for sign in [-1.0, 1.0] {
                    let t = 0.5 * (t0 + t1) + 0.5 * (t1 - t0) * sign * x;
                    let at = [p[0] + t * d[0], p[1] + t * d[1], p[2] + t * d[2]];
                    let wt = 0.5 * (t1 - t0) * w * len;
                    for s in sources {
                        let (a, l) = a_segment(s.a, s.b, at, r_min);
                        let along = dot(l, d) / len;
                        re += s.re * a * along * wt;
                        im += s.im * a * along * wt;
                    }
                }
            }
        }
    }
    (re, im)
}

/// Induced RMS voltage in volt for an RMS flux in weber at `hz`: Faraday,
/// V = 2 pi f Phi.
pub fn induced_v(flux_wb: f64, hz: f64) -> f64 {
    2.0 * std::f64::consts::PI * hz * flux_wb.abs()
}

// ---- sources from runs -----------------------------------------------------

fn mm_to_m(v: f64) -> f64 {
    v / 1000.0
}

/// The routes with wires that share a line laid side by side. The router
/// gives every wire of a duct the same centre line; wires cannot occupy one
/// line, and balanced phases on one line would cancel exactly. Runs on the
/// same line of a plate are spread across it, one wire diameter apart, in the
/// order of their ids, centred on the line. Straight stretches that do not
/// overlap are moved too, which is a small error for a model of this size.
pub fn laid_paths(input: &Input) -> HashMap<u64, Vec<P>> {
    // (plate, horizontal, coordinate in 0.5 mm steps) -> runs on it.
    let mut lines: HashMap<(u64, bool, i64), Vec<(u64, f64)>> = HashMap::new();
    let key = |plate: u64, horiz: bool, c: f64| (plate, horiz, (c * 2.0).round() as i64);
    for r in &input.runs {
        let Some(plate) = r.plate else { continue };
        let mut seen: Vec<(bool, i64)> = Vec::new();
        for w in r.path.windows(2) {
            let horiz = (w[0].1 - w[1].1).abs() < 1e-6;
            let vert = (w[0].0 - w[1].0).abs() < 1e-6;
            if !horiz && !vert {
                continue;
            }
            let k = key(plate, horiz, if horiz { w[0].1 } else { w[0].0 });
            if !seen.contains(&(k.1, k.2)) {
                seen.push((k.1, k.2));
                lines.entry(k).or_default().push((r.id, r.diameter_mm));
            }
        }
    }
    let offset = |plate: u64, horiz: bool, c: f64, id: u64| -> f64 {
        let Some(group) = lines.get(&key(plate, horiz, c)) else {
            return 0.0;
        };
        let mut ids: Vec<u64> = group.iter().map(|g| g.0).collect();
        ids.sort_unstable();
        let rank = ids.iter().position(|x| *x == id).unwrap_or(0) as f64;
        let pitch = group.iter().map(|g| g.1).fold(1.0, f64::max);
        (rank - (ids.len() as f64 - 1.0) / 2.0) * pitch
    };
    let mut out = HashMap::new();
    for r in &input.runs {
        let Some(plate) = r.plate else { continue };
        if r.path.len() < 2 {
            continue;
        }
        let n = r.path.len();
        let moved: Vec<P> = (0..n)
            .map(|j| {
                let (mut dx, mut dy) = (Vec::new(), Vec::new());
                for k in [j.checked_sub(1), (j + 1 < n).then_some(j)]
                    .into_iter()
                    .flatten()
                {
                    let (a, b) = (r.path[k], r.path[k + 1]);
                    if (a.1 - b.1).abs() < 1e-6 {
                        dy.push(offset(plate, true, a.1, r.id));
                    } else if (a.0 - b.0).abs() < 1e-6 {
                        dx.push(offset(plate, false, a.0, r.id));
                    }
                }
                let mean = |v: Vec<f64>| {
                    if v.is_empty() {
                        0.0
                    } else {
                        v.iter().sum::<f64>() / v.len() as f64
                    }
                };
                (r.path[j].0 + mean(dx), r.path[j].1 + mean(dy))
            })
            .collect();
        out.insert(r.id, moved);
    }
    out
}

/// The segments of the runs that carry a current, at `height_mm` above the
/// plate; with `image` also the mirrored set below it with the opposite sign.
/// `keep` chooses the runs.
pub fn sources(
    input: &Input,
    currents: &[Current],
    height_mm: f64,
    image: bool,
    keep: impl Fn(&Run) -> bool,
) -> Vec<Source> {
    let by_run: HashMap<u64, &Run> = input.runs.iter().map(|r| (r.id, r)).collect();
    let laid = laid_paths(input);
    let mut out = Vec::new();
    let z = mm_to_m(height_mm);
    for c in currents {
        let Some(run) = by_run.get(&c.run) else {
            continue;
        };
        if !keep(run) || c.amps <= 0.0 {
            continue;
        }
        let path = laid.get(&run.id).unwrap_or(&run.path);
        let (re, im) = (
            c.amps * c.phase_deg.to_radians().cos(),
            c.amps * c.phase_deg.to_radians().sin(),
        );
        for w in path.windows(2) {
            let a = [mm_to_m(w[0].0), mm_to_m(w[0].1), z];
            let b = [mm_to_m(w[1].0), mm_to_m(w[1].1), z];
            out.push(Source { a, b, re, im });
            if image {
                out.push(Source {
                    a: [a[0], a[1], -z],
                    b: [b[0], b[1], -z],
                    re: -re,
                    im: -im,
                });
            }
        }
    }
    out
}

// ---- the map ---------------------------------------------------------------

/// The flux density over one plate at the slice height.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldMap {
    /// Plate millimetres of the first cell's corner.
    pub origin: (f64, f64),
    pub cell_mm: f64,
    pub nx: usize,
    pub ny: usize,
    /// RMS, microtesla, row by row from the top left.
    pub b_ut: Vec<f32>,
    pub peak_ut: f32,
    pub peak_at: (f64, f64),
}

/// Most cells along one side, so a big plate stays cheap.
const MAX_SIDE: usize = 160;

/// Where a value sits on the map's colour ramp, 0 to 1: logarithmic, the
/// planning value at the middle, a tenth of it at the bottom and ten times
/// it at the top.
pub fn ramp_value(b_ut: f64, limit_ut: f64) -> f32 {
    let lo = limit_ut / 10.0;
    if b_ut <= lo {
        return 0.0;
    }
    ((b_ut / lo).log10() / 2.0).clamp(0.0, 1.0) as f32
}

/// The map of a plate of `size` mm, from the currents on that plate. `None`
/// when no wire on it carries a current.
pub fn map(input: &Input, setup: &FieldSetup, plate: u64, size: (f64, f64)) -> Option<FieldMap> {
    let on_plate = |r: &Run| r.plate == Some(plate) && r.path.len() >= 2;
    let srcs = sources(
        input,
        &setup.currents,
        input.params.wire_height_mm,
        setup.params.image(),
        on_plate,
    );
    if srcs.is_empty() || size.0 <= 0.0 || size.1 <= 0.0 {
        return None;
    }
    let cell = 10.0f64
        .max(size.0 / MAX_SIDE as f64)
        .max(size.1 / MAX_SIDE as f64);
    let nx = ((size.0 / cell).ceil() as usize).max(1);
    let ny = ((size.1 / cell).ceil() as usize).max(1);
    let z = mm_to_m(setup.params.slice_mm);
    let r_min = 0.001;
    let mut b_ut = Vec::with_capacity(nx * ny);
    let (mut peak, mut peak_at) = (0.0f32, (0.0, 0.0));
    for j in 0..ny {
        for i in 0..nx {
            let (x, y) = ((i as f64 + 0.5) * cell, (j as f64 + 0.5) * cell);
            let v = (b_rms([mm_to_m(x), mm_to_m(y), z], &srcs, r_min) * 1.0e6) as f32;
            if v > peak {
                peak = v;
                peak_at = (x, y);
            }
            b_ut.push(v);
        }
    }
    Some(FieldMap {
        origin: (0.0, 0.0),
        cell_mm: cell,
        nx,
        ny,
        b_ut,
        peak_ut: peak,
        peak_at,
    })
}

// ---- results ---------------------------------------------------------------

/// One line of the table of currents in the report.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentRow {
    pub run: u64,
    pub name: String,
    pub amps: f64,
    pub phase_deg: f64,
    pub source: String,
    pub declared: bool,
    pub phase_known: bool,
    /// The wire has a route on a plate, so it is in the field.
    pub routed: bool,
}

/// The field at a sensitive device.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceField {
    pub device: u64,
    pub tag: String,
    pub plate: u64,
    /// Largest RMS flux density found over the device's footprint, µT.
    pub b_ut: f64,
    pub at: P,
}

/// What a signal run picks up.
#[derive(Debug, Clone, PartialEq)]
pub struct VictimRow {
    pub run: u64,
    pub name: String,
    pub class: EmcClass,
    /// RMS at the model frequency, mV.
    pub v_line_mv: f64,
    /// Peak on a current step of the stated rate, mV.
    pub v_edge_mv: f64,
    pub limit_mv: f64,
    /// The run whose field contributes most.
    pub from: Option<u64>,
    /// The loop was go and return of a pair (true) or the wire against the plate.
    pub paired: bool,
}

/// The tables behind the findings, shown whether or not a rule fired.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FieldSummary {
    pub hz: f64,
    pub limit_ut: f64,
    pub image: bool,
    pub currents: Vec<CurrentRow>,
    pub devices: Vec<DeviceField>,
    pub victims: Vec<VictimRow>,
}

/// What the field rules hand back to the engine.
pub struct Outcome {
    pub findings: Vec<Finding>,
    pub status: Vec<RuleStatus>,
    pub summary: FieldSummary,
}

fn name_of(input: &Input, id: u64) -> String {
    input
        .runs
        .iter()
        .find(|r| r.id == id)
        .map_or_else(|| format!("wire {id}"), |r| r.name.clone())
}

fn dist_pt_seg(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
    };
    ((p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)).max(0.0)
}

/// Distance in mm between two routes and the point of the first nearest to
/// the second, sampled every 10 mm.
fn closest(a: &[P], b: &[P]) -> Option<(f64, P)> {
    let mut best: Option<(f64, P)> = None;
    for w in a.windows(2) {
        let len = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
        let n = (len / 10.0).ceil().max(1.0) as usize;
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let p = (
                w[0].0 + t * (w[1].0 - w[0].0),
                w[0].1 + t * (w[1].1 - w[0].1),
            );
            for s in b.windows(2) {
                let d = dist_pt_seg(p, s[0], s[1]);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, p));
                }
            }
        }
    }
    best
}

fn pt3(p: P, z_mm: f64) -> V3 {
    [mm_to_m(p.0), mm_to_m(p.1), mm_to_m(z_mm)]
}

/// The ring of a victim run: go and return of a pair (the return drawn),
/// or the wire above its return through the plate.
fn ring(run: &Run, partner: Option<&Run>, laid: &HashMap<u64, Vec<P>>, height_mm: f64) -> Vec<V3> {
    let path = |r: &Run| laid.get(&r.id).cloned().unwrap_or_else(|| r.path.clone());
    let go = path(run);
    let mut ring: Vec<V3> = go.iter().map(|p| pt3(*p, height_mm)).collect();
    match partner {
        Some(back) => {
            let back = path(back);
            let d = |p: P, q: P| (p.0 - q.0).hypot(p.1 - q.1);
            let (g0, g1) = (go[0], go[go.len() - 1]);
            let (b0, b1) = (back[0], back[back.len() - 1]);
            let flip = d(g0, b0) + d(g1, b1) > d(g0, b1) + d(g1, b0);
            if flip {
                ring.extend(back.iter().map(|p| pt3(*p, height_mm)));
            } else {
                ring.extend(back.iter().rev().map(|p| pt3(*p, height_mm)));
            }
        }
        None => {
            ring.extend(go.iter().rev().map(|p| pt3(*p, 0.0)));
        }
    }
    ring
}

fn fmt_ut(v: f64) -> String {
    if v >= 100.0 {
        format!("{v:.0} \u{b5}T")
    } else if v >= 10.0 {
        format!("{v:.1} \u{b5}T")
    } else {
        format!("{v:.2} \u{b5}T")
    }
}

fn fmt_mv(v: f64) -> String {
    if v >= 100.0 {
        format!("{v:.0} mV")
    } else if v >= 1.0 {
        format!("{v:.1} mV")
    } else if v >= 0.001 {
        format!("{v:.3} mV")
    } else {
        "under 1 \u{b5}V".to_owned()
    }
}

fn points_for(ratio: f64) -> u32 {
    (20.0 + 20.0 * ratio.max(1.0).log2())
        .round()
        .clamp(10.0, 60.0) as u32
}

/// Runs EMC-19 and EMC-20.
pub fn evaluate(input: &Input, setup: &FieldSetup) -> Outcome {
    let fp = &setup.params;
    let by_run: HashMap<u64, &Run> = input.runs.iter().map(|r| (r.id, r)).collect();
    let routed = |id: u64| {
        by_run
            .get(&id)
            .is_some_and(|r| r.path.len() >= 2 && r.plate.is_some())
    };
    let mut summary = FieldSummary {
        hz: fp.hz,
        limit_ut: fp.limit_ut,
        image: fp.image(),
        ..FieldSummary::default()
    };
    for c in &setup.currents {
        summary.currents.push(CurrentRow {
            run: c.run,
            name: name_of(input, c.run),
            amps: c.amps,
            phase_deg: c.phase_deg,
            source: c.source.clone(),
            declared: c.declared,
            phase_known: c.phase_known,
            routed: routed(c.run),
        });
    }
    let live: Vec<&Current> = setup
        .currents
        .iter()
        .filter(|c| c.amps > 0.0 && routed(c.run))
        .collect();
    let sensitive: Vec<_> = input
        .devices
        .iter()
        .filter(|d| d.role == Role::Sensitive && d.plate.is_some() && d.rect.is_some())
        .collect();
    let victims: Vec<&Run> = input
        .runs
        .iter()
        .filter(|r| {
            matches!(
                r.class,
                EmcClass::Analog | EmcClass::Fieldbus | EmcClass::Safety
            ) && r.path.len() >= 2
                && r.plate.is_some()
        })
        .collect();
    let mut findings = Vec::new();
    let height = input.params.wire_height_mm;
    let r_min = 0.001;
    let laid = laid_paths(input);

    // EMC-19: the field where a sensitive device sits.
    for d in &sensitive {
        let (plate, (x, y, w, h)) = (d.plate.expect("filtered"), d.rect.expect("filtered"));
        let Some(probe) = FieldProbe::new(input, setup, plate) else {
            continue;
        };
        let (best, at) = probe.at_rect((x, y, w, h));
        summary.devices.push(DeviceField {
            device: d.id,
            tag: d.tag.clone(),
            plate,
            b_ut: best,
            at,
        });
        let ratio = best / fp.limit_ut;
        if ratio < 0.33 {
            continue;
        }
        let over = ratio >= 1.0;
        // The strongest wire alone, to name it and to say how far to move.
        let top = live
            .iter()
            .filter(|c| by_run.get(&c.run).is_some_and(|r| r.plate == Some(plate)))
            .map(|c| {
                let one = sources(input, &[(*c).clone()], height, fp.image(), |_| true);
                let v = b_rms(
                    [mm_to_m(at.0), mm_to_m(at.1), mm_to_m(fp.slice_mm)],
                    &one,
                    r_min,
                );
                (c.run, v)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let subjects: Vec<Subject> = std::iter::once(Subject::Device(d.id))
            .chain(top.map(|t| Subject::Run(t.0)))
            .collect();
        let mut fixes = Vec::new();
        if over {
            fixes.push(Fix::MoveOutOfField {
                device: d.id,
                below_ut: fp.limit_ut,
            });
        }
        if over && let Some(run) = top.and_then(|t| by_run.get(&t.0)) {
            // Straight wire: the field falls as 1/distance; ask for the
            // distance at which the planning value is reached.
            {
                let centre = (x + w / 2.0, y + h / 2.0);
                let now = run
                    .path
                    .windows(2)
                    .map(|s| dist_pt_seg(centre, s[0], s[1]))
                    .fold(f64::INFINITY, f64::min);
                let to = (now.max(20.0) * ratio).min(1000.0);
                if let Some(from) = run.from.or(run.to) {
                    fixes.push(Fix::MoveDevice {
                        device: d.id,
                        from,
                        to_mm: (to / 10.0).ceil() * 10.0,
                    });
                }
            }
        }
        findings.push(Finding {
            rule: "EMC-19",
            level: Level::Estimate,
            severity: if over { Severity::Warning } else { Severity::Info },
            points: vec![(Subject::Device(d.id), if over { points_for(ratio) } else { 5 })],
            subjects,
            message: format!(
                "{} sits in a field of about {} from the currents drawn ({} at {} Hz); the planning value is {}. A model of the stated currents on the routed wires, not a measurement.",
                d.tag,
                fmt_ut(best),
                top.map_or_else(String::new, |t| format!("mostly {}", name_of(input, t.0))),
                fp.hz,
                fmt_ut(fp.limit_ut),
            ),
            metrics: vec![
                Metric { label: "Field at the device (largest over its footprint)", value: fmt_ut(best), level: Level::Estimate },
                Metric { label: "Planning value", value: fmt_ut(fp.limit_ut), level: Level::Index },
            ],
            fixes,
            at: Some((plate, at)),
            ribbons: Vec::new(),
            loop_polygon: Vec::new(),
            band_hz: Some(BAND_HZ),
        });
    }

    // EMC-20: the voltage induced in a signal run.
    let mut done_pairs: Vec<(u64, u64)> = Vec::new();
    for v in &victims {
        let partner = v.cable.and_then(|c| {
            input.runs.iter().find(|o| {
                o.id != v.id
                    && o.cable == Some(c)
                    && o.class == v.class
                    && o.path.len() >= 2
                    && o.plate == v.plate
            })
        });
        let key = (
            v.id.min(partner.map_or(0, |p| p.id)),
            v.id.max(partner.map_or(0, |p| p.id)),
        );
        if partner.is_some() {
            if done_pairs.contains(&key) {
                continue;
            }
            done_pairs.push(key);
        }
        let plate = v.plate.expect("filtered");
        let not_self =
            |r: &Run| r.id != v.id && Some(r.id) != partner.map(|p| p.id) && r.plate == Some(plate);
        let owned = live_as_owned(&live);
        let line_src = sources(input, &owned, height, fp.image(), not_self);
        let edge_src = sources(input, &owned, height, true, |r| {
            not_self(r) && r.class == EmcClass::Motor
        });
        if line_src.is_empty() {
            continue;
        }
        let ring = ring(v, partner, &laid, height);
        let twist = if v.twisted { fp.twist_factor } else { 1.0 };
        let (fr, fi) = flux(&ring, &line_src, r_min);
        let v_line = induced_v((fr * fr + fi * fi).sqrt() * twist, fp.hz) * 1000.0;
        let i_max = live
            .iter()
            .filter(|c| {
                by_run
                    .get(&c.run)
                    .is_some_and(|r| r.class == EmcClass::Motor && not_self(r))
            })
            .map(|c| c.amps)
            .fold(0.0, f64::max);
        let v_edge = if i_max > 0.0 && !edge_src.is_empty() {
            let (er, ei) = flux(&ring, &edge_src, r_min);
            // Flux per ampere of the largest conductor, times the step rate.
            (er * er + ei * ei).sqrt() / i_max * twist * input.params.didt_a_per_us * 1.0e6 * 1000.0
        } else {
            0.0
        };
        // The biggest single contributor, for the name and the nearest approach.
        let top = live
            .iter()
            .filter(|c| by_run.get(&c.run).is_some_and(|r| not_self(r)))
            .map(|c| {
                let one = sources(input, &[(*c).clone()], height, fp.image(), |_| true);
                let (a, b) = flux(&ring, &one, r_min);
                (c.run, (a * a + b * b).sqrt())
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|t| t.0);
        let limit = fp.induced_limit_mv / v.class.sensitivity().max(0.1);
        summary.victims.push(VictimRow {
            run: v.id,
            name: name_of(input, v.id),
            class: v.class,
            v_line_mv: v_line,
            v_edge_mv: v_edge,
            limit_mv: limit,
            from: top,
            paired: partner.is_some(),
        });
        let worst = (v_line / limit).max(v_edge / limit);
        if worst < 0.33 {
            continue;
        }
        let over = worst >= 1.0;
        let near = top
            .and_then(|t| by_run.get(&t))
            .and_then(|a| closest(&v.path, &a.path));
        let mut fixes = Vec::new();
        if over {
            if !v.twisted {
                fixes.push(Fix::SeparateRuns {
                    a: v.id,
                    b: v.id,
                    to_mm: 0.0,
                });
            }
            if let (Some(t), Some((d, _))) = (top, near) {
                fixes.push(Fix::SeparateRuns {
                    a: v.id,
                    b: t,
                    to_mm: ((d.max(15.0) * worst) / 10.0).ceil().min(100.0) * 10.0,
                });
            }
        }
        let mut subjects = vec![Subject::Run(v.id)];
        if let Some(p) = partner {
            subjects.push(Subject::Run(p.id));
        }
        if let Some(t) = top {
            subjects.push(Subject::Run(t));
        }
        let ribbons = match (top.and_then(|t| by_run.get(&t)), near) {
            (Some(a), Some((d, _))) if d < 300.0 => {
                let pa = geometry::pieces(v, &input.ducts);
                let pb = geometry::pieces(a, &input.ducts);
                geometry::parallel(&pa, &pb, &input.ducts, 200.0)
                    .into_iter()
                    .map(|s| Ribbon {
                        a: s.on_a,
                        b: s.on_b,
                        length_mm: s.length_mm,
                        distance_mm: s.distance_mm,
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        let pts = points_for(worst);
        let mut points = vec![(Subject::Run(v.id), if over { pts } else { 5 })];
        if over && let Some(t) = top {
            points.push((Subject::Run(t), pts / 2));
        }
        findings.push(Finding {
            rule: "EMC-20",
            level: Level::Estimate,
            severity: if over { Severity::Warning } else { Severity::Info },
            subjects,
            points,
            message: format!(
                "{} ({}) picks up about {} at {} Hz and about {} on a {} A/\u{b5}s current step from the wires beside it; planning value {} for this class. {}A model of the stated currents, not a measurement.",
                name_of(input, v.id),
                v.class.label(),
                fmt_mv(v_line),
                fp.hz,
                fmt_mv(v_edge),
                input.params.didt_a_per_us,
                limit_text(limit),
                if partner.is_some() { "Go and return are both drawn. " } else { "The return is taken as the plate. " },
            ),
            metrics: vec![
                Metric { label: "Induced at the line frequency (RMS)", value: fmt_mv(v_line), level: Level::Estimate },
                Metric { label: "Induced on the current step (peak)", value: fmt_mv(v_edge), level: Level::Estimate },
                Metric { label: "Planning value", value: fmt_mv(limit), level: Level::Index },
            ],
            fixes,
            at: near.map(|(_, p)| (plate, p)),
            ribbons,
            loop_polygon: Vec::new(),
            band_hz: Some(BAND_HZ),
        });
    }

    let no_currents = live.is_empty();
    let missing = "No wire has both a current (emc.current_a, or a rated current on a device at an end) and a route on a plate";
    let mk = |rule: &'static str, applies: bool, found: usize, text: &str| RuleStatus {
        rule,
        status: if !applies {
            Status::NotApplicable
        } else if no_currents {
            Status::NotEvaluated
        } else if found > 0 {
            Status::Finding
        } else {
            Status::NoFinding
        },
        findings: found,
        note: if applies && no_currents {
            missing.to_owned()
        } else {
            text.to_owned()
        },
    };
    let n19 = findings.iter().filter(|f| f.rule == "EMC-19").count();
    let n20 = findings.iter().filter(|f| f.rule == "EMC-20").count();
    let status = vec![
        mk(
            "EMC-19",
            !sensitive.is_empty(),
            n19,
            "Field at PLCs and analog modules",
        ),
        mk(
            "EMC-20",
            !victims.is_empty(),
            n20,
            "Voltage induced in analog, fieldbus and safety wires",
        ),
    ];
    Outcome {
        findings,
        status,
        summary,
    }
}

fn limit_text(limit: f64) -> String {
    fmt_mv(limit)
}

fn live_as_owned(live: &[&Current]) -> Vec<Current> {
    live.iter().map(|c| (*c).clone()).collect()
}

/// The field of the stated currents on one plate, ready to be read at any
/// rectangle: the same numbers EMC-19 reports, for a search that moves a
/// device and asks where the field is low enough.
pub struct FieldProbe {
    srcs: Vec<Source>,
    slice_mm: f64,
}

impl FieldProbe {
    /// `None` when no routed wire on the plate carries a current.
    pub fn new(input: &Input, setup: &FieldSetup, plate: u64) -> Option<FieldProbe> {
        let fp = &setup.params;
        let by_run: HashMap<u64, &Run> = input.runs.iter().map(|r| (r.id, r)).collect();
        let live: Vec<Current> = setup
            .currents
            .iter()
            .filter(|c| {
                c.amps > 0.0
                    && by_run
                        .get(&c.run)
                        .is_some_and(|r| r.path.len() >= 2 && r.plate.is_some())
            })
            .cloned()
            .collect();
        let srcs = sources(input, &live, input.params.wire_height_mm, fp.image(), |r| {
            r.plate == Some(plate)
        });
        (!srcs.is_empty()).then_some(FieldProbe {
            srcs,
            slice_mm: fp.slice_mm,
        })
    }

    /// The largest field (µT) over a 5 x 5 sample of `rect` (x, y, w, h) and
    /// where it is.
    pub fn at_rect(&self, rect: (f64, f64, f64, f64)) -> (f64, (f64, f64)) {
        let (x, y, w, h) = rect;
        let (mut best, mut at) = (0.0f64, (x + w / 2.0, y + h / 2.0));
        for i in 0..=4 {
            for j in 0..=4 {
                let p = (x + w * f64::from(i) / 4.0, y + h * f64::from(j) / 4.0);
                let v = b_rms(
                    [mm_to_m(p.0), mm_to_m(p.1), mm_to_m(self.slice_mm)],
                    &self.srcs,
                    0.001,
                ) * 1.0e6;
                if v > best {
                    (best, at) = (v, p);
                }
            }
        }
        (best, at)
    }
}

/// The settings of the estimate, each marked default or set in the project.
pub fn assumptions(input: &Input, setup: &FieldSetup) -> Vec<Assumption> {
    let p = &setup.params;
    let a = |name: &'static str, property: &'static str, value: String| Assumption {
        name,
        value,
        user_set: p.user_set.contains(&property.trim_start_matches("emc.")),
        property,
    };
    vec![
        a(
            "Frequency of the stated currents (magnetic field estimate)",
            "emc.field_hz",
            format!("{} Hz", p.hz),
        ),
        a(
            "Plate treated as a conducting plane (image current) for the line-frequency estimate",
            "emc.plate_image",
            if p.image() {
                "yes".to_owned()
            } else {
                "no: free space (a thin plate hardly screens 50 Hz)".to_owned()
            },
        ),
        a(
            "Height above the plate of the field slice",
            "emc.field_slice_mm",
            format!("{} mm", p.slice_mm),
        ),
        a(
            "Planning value for the field at a sensitive device",
            "emc.field_limit_ut",
            fmt_ut(p.limit_ut),
        ),
        a(
            "Planning value for the voltage induced in the most sensitive signal loop",
            "emc.induced_limit_mv",
            fmt_mv(p.induced_limit_mv),
        ),
        a(
            "Share of the flux a twisted pair keeps",
            "emc.twist_factor",
            format!("{}", p.twist_factor),
        ),
        a(
            "Currents known",
            "",
            format!(
                "{} wires ({} stated by the user)",
                setup.currents.len(),
                setup.currents.iter().filter(|c| c.declared).count()
            ),
        ),
        Assumption {
            name: "Wire height above the plate",
            value: format!("{} mm", input.params.wire_height_mm),
            user_set: input.params.user_set.contains(&"wire_height_mm"),
            property: "emc.wire_height_mm",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) {
        assert!(
            (a - b).abs() <= rel * b.abs().max(1e-30),
            "{a} against {b} (relative {rel})"
        );
    }

    fn one(a: V3, b: V3, amps: f64) -> Vec<Source> {
        vec![Source {
            a,
            b,
            re: amps,
            im: 0.0,
        }]
    }

    #[test]
    fn a_long_straight_wire_gives_mu0_i_over_2_pi_d() {
        // 10 A at 0.1 m: 4e-7 pi * 10 / (2 pi * 0.1) = 2e-5 T = 20 uT.
        let s = one([-100.0, 0.0, 0.0], [100.0, 0.0, 0.0], 10.0);
        close(b_rms([0.0, 0.1, 0.0], &s, 1e-6), 20.0e-6, 1e-4);
        // The field circles the wire: above the wire it points sideways.
        let (re, _) = b_at([0.0, 0.0, 0.1], &s, 1e-6);
        assert!(re[1].abs() > 1.9e-5 && re[2].abs() < 1e-12, "{re:?}");
    }

    #[test]
    fn a_finite_wire_at_its_middle_follows_the_closed_form() {
        // Length 1 m, point 0.1 m off the middle: B = mu0 I /(4 pi d) * 2 sin(theta),
        // sin(theta) = 0.5 / sqrt(0.5^2 + 0.1^2).
        let s = one([-0.5, 0.0, 0.0], [0.5, 0.0, 0.0], 10.0);
        let sin = 0.5 / (0.25f64 + 0.01).sqrt();
        close(
            b_rms([0.0, 0.1, 0.0], &s, 1e-6),
            1.0e-7 * 10.0 / 0.1 * 2.0 * sin,
            1e-9,
        );
    }

    #[test]
    fn a_twin_pair_falls_as_the_square_of_the_distance() {
        // Go and return 10 mm apart, 10 A, long. On the line through both
        // at 0.5 m from the pair's middle: B = mu0 I/(2 pi) (1/(D - s/2) - 1/(D + s/2)).
        let (s, d) = (0.01, 0.5);
        let mut src = one([-100.0, -s / 2.0, 0.0], [100.0, -s / 2.0, 0.0], 10.0);
        src.extend(one([-100.0, s / 2.0, 0.0], [100.0, s / 2.0, 0.0], -10.0));
        let exact = 2.0e-7 * 10.0 * (1.0 / (d - s / 2.0) - 1.0 / (d + s / 2.0));
        close(b_rms([0.0, d, 0.0], &src, 1e-6), exact, 1e-3);
        // Far away it is mu0 I s/(2 pi D^2).
        close(exact, 2.0e-7 * 10.0 * s / (d * d), 1e-3);
        // Doubling the distance quarters it; a lone wire only halves it.
        let ratio = b_rms([0.0, 2.0 * d, 0.0], &src, 1e-6) / b_rms([0.0, d, 0.0], &src, 1e-6);
        assert!((ratio - 0.25).abs() < 0.01, "{ratio}");
    }

    #[test]
    fn a_square_loop_at_its_centre_is_2_root_2_mu0_i_over_pi_a() {
        // Side 0.2 m, 5 A: B = 2 sqrt(2) * 4e-7 pi * 5 / (pi * 0.2) = 2.828e-5 T.
        let a = 0.1;
        let c = [[-a, -a, 0.0], [a, -a, 0.0], [a, a, 0.0], [-a, a, 0.0]];
        let mut src = Vec::new();
        for i in 0..4 {
            src.extend(one(c[i], c[(i + 1) % 4], 5.0));
        }
        let exact = 2.0 * 2.0f64.sqrt() * 4.0e-7 * 5.0 / (0.2);
        close(b_rms([0.0, 0.0, 0.0], &src, 1e-6), exact, 1e-9);
        // On the axis far away it is the dipole field mu0 m / (2 pi z^3), m = I A.
        let z: f64 = 3.0;
        let dipole = MU0 * 5.0 * 0.04 / (2.0 * std::f64::consts::PI * z.powi(3));
        close(b_rms([0.0, 0.0, z], &src, 1e-6), dipole, 5e-3);
    }

    #[test]
    fn three_phases_in_one_bundle_cancel_to_the_square_of_the_distance() {
        // Three wires on a 10 mm circle, 10 A, 0 / -120 / +120 degrees.
        let mut src = Vec::new();
        for k in 0..3 {
            let th = f64::from(k) * 120.0f64.to_radians();
            let (x, y) = (0.005 * th.cos(), 0.005 * th.sin());
            let ph = (-120.0 * f64::from(k)).to_radians();
            src.push(Source {
                a: [-100.0, x, y],
                b: [100.0, x, y],
                re: 10.0 * ph.cos(),
                im: 10.0 * ph.sin(),
            });
        }
        let near = b_rms([0.0, 0.5, 0.0], &src, 1e-6);
        let far = b_rms([0.0, 1.0, 0.0], &src, 1e-6);
        assert!((far / near - 0.25).abs() < 0.02, "{}", far / near);
        // And it is far below a single wire of the same current.
        assert!(near < 0.05 * 2.0e-7 * 10.0 / 0.5, "{near}");
    }

    #[test]
    fn the_plate_doubles_the_tangential_field_on_its_surface() {
        // A wire at 15 mm over a perfect conductor: right under it the image
        // adds the same field again and the normal part vanishes.
        let h = 0.015;
        let mut src = one([-100.0, 0.0, h], [100.0, 0.0, h], 10.0);
        src.extend(one([-100.0, 0.0, -h], [100.0, 0.0, -h], -10.0));
        let (re, _) = b_at([0.0, 0.0, 0.0], &src, 1e-6);
        close(norm(re), 2.0 * 2.0e-7 * 10.0 / h, 1e-6);
        assert!(re[2].abs() < 1e-12);
    }

    #[test]
    fn flux_through_a_loop_beside_a_wire_is_mu0_i_l_ln_ratio_over_2_pi() {
        // Wire along x, a rectangle 0.5 m long between 0.05 and 0.15 m from it,
        // 1 A: Phi = 2e-7 * 0.5 * ln 3.
        let s = one([-100.0, 0.0, 0.0], [100.0, 0.0, 0.0], 1.0);
        let ring = [
            [0.0, 0.05, 0.0],
            [0.5, 0.05, 0.0],
            [0.5, 0.15, 0.0],
            [0.0, 0.15, 0.0],
        ];
        let (re, im) = flux(&ring, &s, 1e-6);
        close(re.abs(), 2.0e-7 * 0.5 * 3.0f64.ln(), 1e-4);
        assert!(im.abs() < 1e-18);
    }

    #[test]
    fn a_wire_over_the_plate_matches_the_mutual_inductance_of_the_level_2_formula() {
        // Aggressor and victim both 15 mm over the plate, 50 mm apart, long;
        // the victim returns through the plate. Per metre the flux per ampere
        // is the existing mutual_per_m(h, d).
        let (h, d) = (0.015, 0.05);
        let mut src = one([-50.0, 0.0, h], [50.0, 0.0, h], 1.0);
        src.extend(one([-50.0, 0.0, -h], [50.0, 0.0, -h], -1.0));
        let l = 1.0;
        let ring = [
            [-l / 2.0, d, h],
            [l / 2.0, d, h],
            [l / 2.0, d, 0.0],
            [-l / 2.0, d, 0.0],
        ];
        let (re, _) = flux(&ring, &src, 1e-6);
        close(re.abs(), geometry::mutual_per_m(15.0, 50.0, 1.0) * l, 2e-3);
    }

    #[test]
    fn faraday_gives_two_pi_f_b_a() {
        // 20 uT through 100 cm2 at 50 Hz: 2 pi 50 * 20e-6 * 0.01 = 62.8 uV.
        close(induced_v(20.0e-6 * 0.01, 50.0), 6.283e-5, 1e-3);
    }

    #[test]
    fn the_ramp_puts_the_planning_value_in_the_middle() {
        assert_eq!(ramp_value(1.0, 12.6), 0.0);
        close(f64::from(ramp_value(12.6, 12.6)), 0.5, 1e-6);
        assert_eq!(ramp_value(1260.0, 12.6), 1.0);
    }
}
