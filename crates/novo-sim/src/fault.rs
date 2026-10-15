//! Short-circuit currents as they flow in time.
//!
//! The solver (or the IEC 60909 calculation in novo-checks) gives the
//! *symmetrical* initial short-circuit current `Ik''` at a node and the ratio
//! `R/X` of the fault loop. This module turns that into what a breaker, a fuse
//! and a current transformer actually see: the asymmetrical current
//!
//! ```text
//! i(t) = √2·Ik''·[ sin(ωt + θ − φ) − sin(θ − φ)·e^(−t/τ) ]
//! ```
//!
//! with the voltage angle `θ` at the moment the fault starts, `φ = atan(X/R)`
//! and the DC time constant `τ = X/(ω·R)`. The worst case, `θ − φ = ±90°`,
//! gives the peak current `ip = κ·√2·Ik''` with `κ = 1.02 + 0.98·e^(−3R/X)`
//! (IEC 60909-0, the formula used for the peak factor of one fed meshed
//! network; the waveform here reproduces it within a few percent).
//!
//! An AC breaker can only interrupt at a natural current zero, so
//! [`FaultCurrent::interrupt_at`] finds the first zero after the opening
//! command: the let-through of a fault is the area under `i²`, up to that
//! zero.

use std::f64::consts::{FRAC_PI_2, PI};

use serde::{Deserialize, Serialize};

use crate::trace::Trace;

/// What kind of fault a scenario starts. Each kind needs a different loop
/// impedance, so the solver is asked per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultKind {
    /// Three phases bolted together: the highest current, rates breakers.
    ThreePhase,
    /// Phase to phase.
    PhasePhase,
    /// Phase to neutral: the lowest current on a TN system, decides whether
    /// the magnetic trip is reached at the end of a long cable.
    PhaseNeutral,
    /// Phase to protective earth: shock protection, disconnection time.
    PhaseEarth,
    /// A phase conductor opens: no short circuit, motors single phase.
    OpenPhase,
}

impl FaultKind {
    pub const ALL: [FaultKind; 5] = [
        FaultKind::ThreePhase,
        FaultKind::PhasePhase,
        FaultKind::PhaseNeutral,
        FaultKind::PhaseEarth,
        FaultKind::OpenPhase,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FaultKind::ThreePhase => "Three-phase short circuit",
            FaultKind::PhasePhase => "Phase to phase",
            FaultKind::PhaseNeutral => "Phase to neutral",
            FaultKind::PhaseEarth => "Phase to earth",
            FaultKind::OpenPhase => "Open phase",
        }
    }
}

/// A short-circuit current at one point, as the solver reports it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FaultCurrent {
    /// Symmetrical initial current Ik'' in A (rms).
    pub ik: f64,
    /// R/X of the fault loop (dimensionless, > 0). Low voltage cables are
    /// around 1 or more; a transformer next to the fault around 0.1.
    pub r_over_x: f64,
    /// Network frequency in Hz.
    pub frequency: f64,
}

impl FaultCurrent {
    pub fn new(ik: f64, r_over_x: f64, frequency: f64) -> Self {
        Self {
            ik,
            r_over_x: r_over_x.max(1e-4),
            frequency,
        }
    }

    pub fn omega(&self) -> f64 {
        2.0 * PI * self.frequency
    }

    /// Phase of the loop impedance.
    pub fn phi(&self) -> f64 {
        (1.0 / self.r_over_x).atan()
    }

    /// DC time constant in seconds.
    pub fn tau(&self) -> f64 {
        1.0 / (self.omega() * self.r_over_x)
    }

    /// Peak factor κ of IEC 60909-0.
    pub fn kappa(&self) -> f64 {
        1.02 + 0.98 * (-3.0 * self.r_over_x).exp()
    }

    /// Peak short-circuit current ip in A.
    pub fn peak(&self) -> f64 {
        self.kappa() * 2f64.sqrt() * self.ik
    }

    /// Inception angle that gives the largest DC offset.
    pub fn worst_case_angle(&self) -> f64 {
        self.phi() - FRAC_PI_2
    }

    /// Instantaneous current `t` seconds after the fault started, for a
    /// fault that begins at voltage angle `theta` (radians).
    pub fn instant(&self, t: f64, theta: f64) -> f64 {
        if t < 0.0 {
            return 0.0;
        }
        let a = theta - self.phi();
        2f64.sqrt() * self.ik * ((self.omega() * t + a).sin() - a.sin() * (-t / self.tau()).exp())
    }

    /// First natural current zero at or after `t_open`, found by stepping a
    /// twentieth of a cycle and refining by bisection. The arc of an AC
    /// breaker is extinguished there; the contacts may open earlier, the
    /// current does not.
    pub fn interrupt_at(&self, t_open: f64, theta: f64) -> f64 {
        let step = 1.0 / (self.frequency * 20.0);
        let mut a = t_open.max(0.0);
        let mut fa = self.instant(a, theta);
        if fa == 0.0 {
            return a;
        }
        // Bounded: with a DC offset that outlasts several cycles the zero can
        // be late, but never later than ~ tau + 2 cycles.
        let limit = a + 6.0 * self.tau() + 10.0 / self.frequency;
        while a < limit {
            let b = a + step;
            let fb = self.instant(b, theta);
            if fa.signum() != fb.signum() {
                let (mut lo, mut hi) = (a, b);
                for _ in 0..60 {
                    let mid = 0.5 * (lo + hi);
                    if self.instant(mid, theta).signum() == fa.signum() {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                return 0.5 * (lo + hi);
            }
            a = b;
            fa = fb;
        }
        limit
    }

    /// The waveform of one phase from the fault start at `t_start` to the
    /// interruption at `t_end`, sampled `per_cycle` times a cycle. Zero
    /// before `t_start` and after `t_end`.
    pub fn waveform(
        &self,
        name: &str,
        theta: f64,
        t_start: f64,
        t_end: f64,
        t_total: f64,
        per_cycle: usize,
    ) -> Trace {
        let dt = 1.0 / (self.frequency * per_cycle.max(8) as f64);
        let mut trace = Trace::new(name, "A");
        trace.push(0.0, 0.0);
        let mut t = t_start;
        trace.push_step(t_start, 0.0);
        while t < t_end {
            trace.push(t, self.instant(t - t_start, theta));
            t += dt;
        }
        trace.push(t_end, self.instant(t_end - t_start, theta));
        if t_total > t_end {
            trace.push_step(t_end, 0.0);
            trace.push(t_total, 0.0);
        }
        trace
    }

    /// Let-through energy ∫i² dt in A²s from the start to `t_end` (after the
    /// start), by Simpson integration over the actual waveform.
    pub fn let_through(&self, theta: f64, t_end: f64) -> f64 {
        let n = ((t_end * self.frequency * 400.0).ceil() as usize).max(2) & !1;
        let h = t_end / n as f64;
        let f = |t: f64| self.instant(t, theta).powi(2);
        let mut s = f(0.0) + f(t_end);
        for k in 1..n {
            s += f(k as f64 * h) * if k % 2 == 1 { 4.0 } else { 2.0 };
        }
        s * h / 3.0
    }
}
