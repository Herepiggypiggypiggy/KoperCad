//! Analog circuit solver: modified nodal analysis (MNA) for DC, AC steady
//! state (RMS phasors) and transient (trapezoidal integration).
//!
//! The unknowns are the node voltages (ground is node 0) and one branch
//! current for every element that cannot be written as a conductance:
//! voltage sources, inductors and ideal transformers. Every element reports
//! the current through it from its first terminal to its second.
//!
//! Honest limits: elements are linear and lumped. Saturation, diodes and
//! other non-linear parts are not here; the motor model lives in
//! [`motor`]. Each node gets 1 pS to ground so a floating sub-circuit
//! solves to 0 V instead of failing.

// Dense linear algebra reads best with index loops.
#![allow(clippy::needless_range_loop)]

mod circuit;
mod complex;
mod matrix;
pub mod motor;
mod solve;

pub use circuit::{Circuit, Element, G_OFF, GROUND, Node, R_ON, Wave};
pub use complex::Complex;
pub use matrix::{Lu, Matrix};
pub use solve::{AcSolution, Error, Event, Method, Trace, solve_ac, solve_dc, transient};
