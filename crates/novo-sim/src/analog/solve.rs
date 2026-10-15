//! MNA assembly and the three analyses.

use std::f64::consts::PI;

use super::circuit::{Circuit, Element, G_OFF, R_ON};
use super::{Complex, Lu, Matrix};

/// Why a circuit could not be solved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Error {
    /// The equations have no unique answer: a loop made only of voltage
    /// sources and inductors, or sources that contradict each other.
    Singular,
    /// An element has a value that cannot be simulated (index of it).
    BadValue(usize),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Singular => write!(
                f,
                "the circuit has no unique solution (a loop of voltage sources or inductors?)"
            ),
            Error::BadValue(i) => write!(f, "element {i} has a value that cannot be simulated"),
        }
    }
}

impl std::error::Error for Error {}

/// Node voltages and element currents of a DC or AC solve. AC values are
/// RMS phasors, so `v * i.conj()` is the complex power. Currents run from
/// an element's first terminal to its second.
#[derive(Debug, Clone)]
pub struct AcSolution {
    pub voltages: Vec<Complex>,
    pub currents: Vec<Complex>,
}

impl AcSolution {
    pub fn voltage(&self, node: usize) -> Complex {
        self.voltages[node]
    }

    pub fn current(&self, element: usize) -> Complex {
        self.currents[element]
    }

    /// Voltage of `a` relative to `b`.
    pub fn between(&self, a: usize, b: usize) -> Complex {
        self.voltages[a] - self.voltages[b]
    }
}

struct Layout {
    nodes: usize,
    /// Unknown index of each element's branch current.
    branch: Vec<Option<usize>>,
    /// Per inductor: the inductors it is coupled to, with the mutual
    /// inductance.
    mutual: Vec<Vec<(usize, f64)>>,
    size: usize,
}

impl Layout {
    fn new(c: &Circuit) -> Result<Layout, Error> {
        let nodes = c.node_count();
        let mut size = nodes - 1;
        let mut branch = vec![None; c.elements().len()];
        let mut mutual = vec![Vec::new(); c.elements().len()];
        for (i, e) in c.elements().iter().enumerate() {
            let ok = match *e {
                Element::Resistor { ohms, .. } => ohms.is_finite() && ohms > 0.0,
                Element::Capacitor { farads, .. } => farads.is_finite() && farads >= 0.0,
                Element::Inductor { henries, .. } => henries.is_finite() && henries > 0.0,
                Element::IdealTransformer { ratio, .. } => ratio.is_finite() && ratio != 0.0,
                Element::Coupling { l1, l2, k } => {
                    let l = |x: usize| match c.elements().get(x) {
                        Some(Element::Inductor { henries, .. }) => Some(*henries),
                        _ => None,
                    };
                    match (l(l1), l(l2)) {
                        (Some(a), Some(b)) if l1 != l2 && (0.0..1.0).contains(&k) => {
                            let m = k * (a * b).sqrt();
                            mutual[l1].push((l2, m));
                            mutual[l2].push((l1, m));
                            true
                        }
                        _ => false,
                    }
                }
                _ => true,
            };
            if !ok {
                return Err(Error::BadValue(i));
            }
            if matches!(
                e,
                Element::Voltage { .. }
                    | Element::Inductor { .. }
                    | Element::IdealTransformer { .. }
            ) {
                branch[i] = Some(size);
                size += 1;
            }
        }
        Ok(Layout {
            nodes,
            branch,
            mutual,
            size,
        })
    }
}

/// What the analysis is.
#[derive(Clone, Copy)]
enum Mode<'a> {
    Dc,
    Ac(f64),
    /// Step `h` ending at `t`; `k` is 2 for trapezoidal and 1 for backward
    /// Euler; `prev` is the state at the start of the step.
    Tran {
        h: f64,
        k: f64,
        t: f64,
        prev: &'a State,
    },
}

/// Current and voltage (first to second terminal) of every element.
#[derive(Debug, Clone)]
struct State {
    cur: Vec<f64>,
    volt: Vec<f64>,
}

fn assemble(c: &Circuit, l: &Layout, mode: Mode) -> (Matrix, Vec<Complex>) {
    let mut m = Matrix::new(l.size);
    let mut rhs = vec![Complex::ZERO; l.size];
    let ix = |n: usize| n.checked_sub(1);
    let y_stamp = |m: &mut Matrix, a: usize, b: usize, y: Complex| {
        if let Some(a) = ix(a) {
            m.add(a, a, y);
        }
        if let Some(b) = ix(b) {
            m.add(b, b, y);
        }
        if let (Some(a), Some(b)) = (ix(a), ix(b)) {
            m.add(a, b, -y);
            m.add(b, a, -y);
        }
    };
    // KCL contribution of a branch current j flowing a -> b.
    let branch_kcl = |m: &mut Matrix, a: usize, b: usize, j: usize, scale: f64| {
        if let Some(a) = ix(a) {
            m.add(a, j, Complex::real(scale));
        }
        if let Some(b) = ix(b) {
            m.add(b, j, Complex::real(-scale));
        }
    };
    let volt_row = |m: &mut Matrix, row: usize, a: usize, b: usize, scale: f64| {
        if let Some(a) = ix(a) {
            m.add(row, a, Complex::real(scale));
        }
        if let Some(b) = ix(b) {
            m.add(row, b, Complex::real(-scale));
        }
    };
    for n in 0..l.nodes - 1 {
        m.add(n, n, Complex::real(G_OFF));
    }
    for (i, e) in c.elements().iter().enumerate() {
        match *e {
            Element::Resistor { a, b, ohms } => y_stamp(&mut m, a, b, Complex::real(1.0 / ohms)),
            Element::Switch { a, b, closed } => {
                let g = if closed { 1.0 / R_ON } else { G_OFF };
                y_stamp(&mut m, a, b, Complex::real(g));
            }
            Element::Capacitor { a, b, farads } => match mode {
                Mode::Dc => {}
                Mode::Ac(hz) => y_stamp(&mut m, a, b, Complex::new(0.0, 2.0 * PI * hz * farads)),
                Mode::Tran { h, k, prev, .. } => {
                    let g = k * farads / h;
                    y_stamp(&mut m, a, b, Complex::real(g));
                    let ihist = -(g * prev.volt[i] + (k - 1.0) * prev.cur[i]);
                    if let Some(a) = ix(a) {
                        rhs[a] = rhs[a] - Complex::real(ihist);
                    }
                    if let Some(b) = ix(b) {
                        rhs[b] += Complex::real(ihist);
                    }
                }
            },
            Element::Current { from, to, wave } => {
                let value = match mode {
                    Mode::Dc => Complex::real(wave.dc()),
                    Mode::Ac(hz) => wave.phasor(hz),
                    Mode::Tran { t, .. } => Complex::real(wave.at(t)),
                };
                if let Some(a) = ix(from) {
                    rhs[a] = rhs[a] - value;
                }
                if let Some(b) = ix(to) {
                    rhs[b] += value;
                }
            }
            Element::Voltage { p, n, wave } => {
                let j = l.branch[i].expect("voltage source has a branch");
                branch_kcl(&mut m, p, n, j, 1.0);
                volt_row(&mut m, j, p, n, 1.0);
                rhs[j] = match mode {
                    Mode::Dc => Complex::real(wave.dc()),
                    Mode::Ac(hz) => wave.phasor(hz),
                    Mode::Tran { t, .. } => Complex::real(wave.at(t)),
                };
            }
            Element::IdealTransformer {
                p1,
                p2,
                s1,
                s2,
                ratio,
            } => {
                let j = l.branch[i].expect("transformer has a branch");
                branch_kcl(&mut m, p1, p2, j, 1.0);
                branch_kcl(&mut m, s1, s2, j, -ratio);
                volt_row(&mut m, j, p1, p2, 1.0);
                volt_row(&mut m, j, s1, s2, -ratio);
            }
            Element::Inductor { a, b, henries } => {
                let j = l.branch[i].expect("inductor has a branch");
                branch_kcl(&mut m, a, b, j, 1.0);
                volt_row(&mut m, j, a, b, 1.0);
                // v - z*i - sum zm*i_other = history.
                let scale = match mode {
                    Mode::Dc => 0.0,
                    Mode::Ac(hz) => 2.0 * PI * hz,
                    Mode::Tran { h, k, .. } => k / h,
                };
                let z = |x: f64| match mode {
                    Mode::Ac(_) => Complex::new(0.0, scale * x),
                    _ => Complex::real(scale * x),
                };
                m.add(j, j, -z(henries));
                let mut hist = Complex::ZERO;
                if let Mode::Tran { k, prev, .. } = mode {
                    hist =
                        hist - z(henries) * prev.cur[i] - Complex::real((k - 1.0) * prev.volt[i]);
                }
                for &(other, mut_l) in &l.mutual[i] {
                    let jo = l.branch[other].expect("coupled inductor has a branch");
                    m.add(j, jo, -z(mut_l));
                    if let Mode::Tran { prev, .. } = mode {
                        hist = hist - z(mut_l) * prev.cur[other];
                    }
                }
                rhs[j] = hist;
            }
            Element::Coupling { .. } => {}
        }
    }
    (m, rhs)
}

/// Element currents and node voltages from the unknown vector.
fn unpack(c: &Circuit, l: &Layout, mode: Mode, x: &[Complex]) -> AcSolution {
    let mut voltages = vec![Complex::ZERO; l.nodes];
    voltages[1..l.nodes].copy_from_slice(&x[..(l.nodes - 1)]);
    let v = |a: usize, b: usize| voltages[a] - voltages[b];
    let currents = c
        .elements()
        .iter()
        .enumerate()
        .map(|(i, e)| match *e {
            Element::Resistor { a, b, ohms } => v(a, b) * (1.0 / ohms),
            Element::Switch { a, b, closed } => v(a, b) * if closed { 1.0 / R_ON } else { G_OFF },
            Element::Capacitor { a, b, farads } => match mode {
                Mode::Dc => Complex::ZERO,
                Mode::Ac(hz) => v(a, b) * Complex::new(0.0, 2.0 * PI * hz * farads),
                Mode::Tran { h, k, prev, .. } => {
                    let g = k * farads / h;
                    Complex::real(g * (v(a, b).re - prev.volt[i]) - (k - 1.0) * prev.cur[i])
                }
            },
            Element::Current { wave, .. } => match mode {
                Mode::Dc => Complex::real(wave.dc()),
                Mode::Ac(hz) => wave.phasor(hz),
                Mode::Tran { t, .. } => Complex::real(wave.at(t)),
            },
            Element::Coupling { .. } => Complex::ZERO,
            Element::Voltage { .. }
            | Element::Inductor { .. }
            | Element::IdealTransformer { .. } => x[l.branch[i].expect("branch")],
        })
        .collect();
    AcSolution { voltages, currents }
}

fn factor(c: &Circuit, l: &Layout, mode: Mode) -> Result<(Lu, Vec<Complex>), Error> {
    let (m, rhs) = assemble(c, l, mode);
    Ok((m.factor().ok_or(Error::Singular)?, rhs))
}

/// DC operating point: capacitors open, inductors short, sources at their
/// DC value (a sine source contributes its offset).
pub fn solve_dc(c: &Circuit) -> Result<AcSolution, Error> {
    let l = Layout::new(c)?;
    let (lu, rhs) = factor(c, &l, Mode::Dc)?;
    Ok(unpack(c, &l, Mode::Dc, &lu.solve(&rhs)))
}

/// AC steady state at `hz`. Sources of other frequencies and DC sources are
/// zero here (superposition); call again for them.
pub fn solve_ac(c: &Circuit, hz: f64) -> Result<AcSolution, Error> {
    let l = Layout::new(c)?;
    let (lu, rhs) = factor(c, &l, Mode::Ac(hz))?;
    Ok(unpack(c, &l, Mode::Ac(hz), &lu.solve(&rhs)))
}

/// The integration rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Method {
    /// Second order, no damping. The first step and the step after every
    /// switch event use backward Euler to avoid numerical ringing.
    #[default]
    Trapezoidal,
    /// First order, strongly damped.
    BackwardEuler,
}

/// A switch element changes state at `time` (applied at the first step
/// boundary at or after it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    pub time: f64,
    pub element: usize,
    pub closed: bool,
}

/// The result of a transient run, one sample per recorded step.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    pub times: Vec<f64>,
    /// `voltages[sample][node]`.
    pub voltages: Vec<Vec<f64>>,
    /// `currents[sample][element]`.
    pub currents: Vec<Vec<f64>>,
}

impl Trace {
    pub fn voltage(&self, node: usize) -> Vec<f64> {
        self.voltages.iter().map(|s| s[node]).collect()
    }

    pub fn current(&self, element: usize) -> Vec<f64> {
        self.currents.iter().map(|s| s[element]).collect()
    }

    /// Index of the sample nearest `t`.
    pub fn index_at(&self, t: f64) -> usize {
        self.times
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 - t).abs().total_cmp(&(b.1 - t).abs()))
            .map_or(0, |(i, _)| i)
    }
}

/// Transient analysis from rest (every capacitor discharged, every inductor
/// carrying no current) with fixed step `h`, up to `t_end`.
pub fn transient(
    circuit: &Circuit,
    h: f64,
    t_end: f64,
    method: Method,
    events: &[Event],
) -> Result<Trace, Error> {
    if !(h.is_finite() && h > 0.0 && t_end.is_finite()) {
        return Err(Error::BadValue(0));
    }
    let mut c = circuit.clone();
    let l = Layout::new(&c)?;
    let n_el = c.elements().len();
    let mut state = State {
        cur: vec![0.0; n_el],
        volt: vec![0.0; n_el],
    };
    let mut trace = Trace::default();
    trace.times.push(0.0);
    trace.voltages.push(vec![0.0; l.nodes]);
    trace.currents.push(vec![0.0; n_el]);
    let mut pending: Vec<Event> = events.to_vec();
    pending.sort_by(|a, b| a.time.total_cmp(&b.time));
    let mut pending = pending.into_iter().peekable();
    let steps = (t_end / h).round() as usize;
    let mut cached: Option<(f64, Lu)> = None;
    let mut damp = true;
    for step in 1..=steps {
        let start = (step - 1) as f64 * h;
        let t = step as f64 * h;
        while pending.peek().is_some_and(|e| e.time <= start + h * 1e-9) {
            let e = pending.next().expect("peeked");
            c.set_switch(e.element, e.closed);
            cached = None;
            damp = true;
        }
        let k = if method == Method::BackwardEuler || damp {
            1.0
        } else {
            2.0
        };
        damp = false;
        let mode = Mode::Tran {
            h,
            k,
            t,
            prev: &state,
        };
        if cached.as_ref().is_none_or(|(ck, _)| *ck != k) {
            let (lu, _) = factor(&c, &l, mode)?;
            cached = Some((k, lu));
        }
        let (_, rhs) = assemble(&c, &l, mode);
        let x = cached.as_ref().expect("factored").1.solve(&rhs);
        let sol = unpack(&c, &l, mode, &x);
        for (i, e) in c.elements().iter().enumerate() {
            let (a, b) = terminals(e);
            state.cur[i] = sol.currents[i].re;
            state.volt[i] = sol.voltages[a].re - sol.voltages[b].re;
        }
        trace.times.push(t);
        trace
            .voltages
            .push(sol.voltages.iter().map(|v| v.re).collect());
        trace
            .currents
            .push(sol.currents.iter().map(|v| v.re).collect());
    }
    Ok(trace)
}

fn terminals(e: &Element) -> (usize, usize) {
    match *e {
        Element::Resistor { a, b, .. }
        | Element::Capacitor { a, b, .. }
        | Element::Inductor { a, b, .. }
        | Element::Switch { a, b, .. } => (a, b),
        Element::Voltage { p, n, .. } => (p, n),
        Element::Current { from, to, .. } => (from, to),
        Element::IdealTransformer { p1, p2, .. } => (p1, p2),
        Element::Coupling { .. } => (0, 0),
    }
}
