//! The circuit the solver works on: nodes and elements, nothing else. Node 0
//! is ground. A circuit does not know about the project; the extractor
//! builds one from a drawing, tests build them by hand.

use std::f64::consts::{PI, SQRT_2};

use super::Complex;

pub type Node = usize;

/// The ground node.
pub const GROUND: Node = 0;

/// What a source puts out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wave {
    Dc(f64),
    /// `offset + peak * sin(2π f t + phase)`; phase in radians.
    Sine {
        peak: f64,
        hz: f64,
        phase: f64,
        offset: f64,
    },
}

impl Wave {
    /// A sine given by its RMS value and phase in degrees (how supplies are
    /// quoted: 230 V at 50 Hz, L2 at -120°).
    pub fn rms(rms: f64, hz: f64, phase_deg: f64) -> Wave {
        Wave::Sine {
            peak: rms * SQRT_2,
            hz,
            phase: phase_deg.to_radians(),
            offset: 0.0,
        }
    }

    /// The value at time `t` seconds.
    pub fn at(&self, t: f64) -> f64 {
        match *self {
            Wave::Dc(v) => v,
            Wave::Sine {
                peak,
                hz,
                phase,
                offset,
            } => offset + peak * (2.0 * PI * hz * t + phase).sin(),
        }
    }

    /// The RMS phasor at `hz`, if the wave is a sine of that frequency.
    /// Other frequencies and DC contribute nothing to a steady-state AC
    /// solve at `hz` (superposition).
    pub fn phasor(&self, hz: f64) -> Complex {
        match *self {
            Wave::Sine {
                peak, hz: f, phase, ..
            } if (f - hz).abs() <= 1e-9 * hz.abs().max(1.0) => Complex::polar(peak / SQRT_2, phase),
            _ => Complex::ZERO,
        }
    }

    /// The DC component.
    pub fn dc(&self) -> f64 {
        match *self {
            Wave::Dc(v) => v,
            Wave::Sine { offset, .. } => offset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Element {
    Resistor {
        a: Node,
        b: Node,
        ohms: f64,
    },
    Capacitor {
        a: Node,
        b: Node,
        farads: f64,
    },
    Inductor {
        a: Node,
        b: Node,
        henries: f64,
    },
    /// Mutual coupling `k` (0..1) between two inductor elements, by index.
    /// Two coupled inductors are a real transformer with leakage and
    /// magnetising inductance.
    Coupling {
        l1: usize,
        l2: usize,
        k: f64,
    },
    /// Ideal transformer: `v(p) = ratio * v(s)`, no losses, no leakage.
    IdealTransformer {
        p1: Node,
        p2: Node,
        s1: Node,
        s2: Node,
        ratio: f64,
    },
    /// Voltage `v(p) - v(n)`. Its current is the one flowing p to n through
    /// the source, so a source delivering power reports a negative current.
    Voltage {
        p: Node,
        n: Node,
        wave: Wave,
    },
    /// Current flowing from `from` through the source to `to`.
    Current {
        from: Node,
        to: Node,
        wave: Wave,
    },
    /// A contact or breaker: `r_on` closed, practically open otherwise.
    Switch {
        a: Node,
        b: Node,
        closed: bool,
    },
}

/// Resistance of a closed switch and conductance of an open one.
pub const R_ON: f64 = 1e-6;
pub const G_OFF: f64 = 1e-12;

#[derive(Debug, Clone, Default)]
pub struct Circuit {
    nodes: usize,
    elements: Vec<Element>,
}

impl Circuit {
    pub fn new() -> Self {
        Circuit {
            nodes: 1,
            elements: Vec::new(),
        }
    }

    /// A new node.
    pub fn node(&mut self) -> Node {
        self.nodes += 1;
        self.nodes - 1
    }

    /// Nodes including ground.
    pub fn node_count(&self) -> usize {
        self.nodes.max(1)
    }

    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    /// Adds an element and returns its index (results are indexed by it).
    pub fn add(&mut self, e: Element) -> usize {
        let top = match e {
            Element::Resistor { a, b, .. }
            | Element::Capacitor { a, b, .. }
            | Element::Inductor { a, b, .. }
            | Element::Switch { a, b, .. } => a.max(b),
            Element::Voltage { p, n, .. } => p.max(n),
            Element::Current { from, to, .. } => from.max(to),
            Element::IdealTransformer { p1, p2, s1, s2, .. } => p1.max(p2).max(s1).max(s2),
            Element::Coupling { .. } => 0,
        };
        self.nodes = self.nodes.max(top + 1).max(1);
        self.elements.push(e);
        self.elements.len() - 1
    }

    pub fn resistor(&mut self, a: Node, b: Node, ohms: f64) -> usize {
        self.add(Element::Resistor { a, b, ohms })
    }

    pub fn capacitor(&mut self, a: Node, b: Node, farads: f64) -> usize {
        self.add(Element::Capacitor { a, b, farads })
    }

    pub fn inductor(&mut self, a: Node, b: Node, henries: f64) -> usize {
        self.add(Element::Inductor { a, b, henries })
    }

    pub fn voltage(&mut self, p: Node, n: Node, wave: Wave) -> usize {
        self.add(Element::Voltage { p, n, wave })
    }

    pub fn current(&mut self, from: Node, to: Node, wave: Wave) -> usize {
        self.add(Element::Current { from, to, wave })
    }

    pub fn switch(&mut self, a: Node, b: Node, closed: bool) -> usize {
        self.add(Element::Switch { a, b, closed })
    }

    /// Opens or closes a switch element.
    pub fn set_switch(&mut self, element: usize, closed_now: bool) {
        if let Some(Element::Switch { closed, .. }) = self.elements.get_mut(element) {
            *closed = closed_now;
        }
    }
}
