//! Three-dimensional finite-volume CFD for enclosures (design
//! `docs/design/thermal-cfd.md`).
//!
//! Pure engine: no UI dependencies. The result contract lives in [`result`],
//! the [`Solver`] trait in [`solver`]. The instant 2D estimate of
//! `novo-thermal` stays the preview; solvers here refine it and publish a
//! [`CfdSnapshot`] each time a level improves.
//!
//! Where the user finds it: Cabinet view, Heat layer, "Simulate"
//! (`cabinet.thermal.simulate`). The app side is lane CFD-F.

pub mod flow;
pub mod mesh;
pub mod result;
pub mod sched;
pub mod solver;
mod stub;

pub use result::*;
pub use solver::{CfdError, Control, Solver};
pub use stub::StubSolver;
