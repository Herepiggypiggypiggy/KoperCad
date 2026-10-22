//! CFD for large cabinets (lane CFD-LARGE).
//!
//! A real machine cabinet (the CV-200: 115 devices, 9 rails, 11 ducts on a
//! 1.8 x 1.9 m plate) gives a rectilinear grid of 1.5 million cells at the
//! Coarse level: every face of every small body (breakers, relays, terminals)
//! is a plane that runs through the whole domain, so the cell count is set
//! by the number of bodies, not by the physics. That grid does not converge
//! in any useful time.
//!
//! This module builds a *reduced scenario* that the unchanged solver can run
//! on a budget of cells, and maps the result back to the real devices:
//!
//! 1. **Hot devices stay exact.** Everything above the cold limit (3 % of the
//!    cabinet's loss, at least 0.5 W) keeps its own box.
//! 2. **Cold devices are lumped** into row blocks (neighbours on the same rail
//!    row, small gaps, no hot device between). A block keeps the summed loss
//!    and the summed solid volume (its height is set so the volume matches).
//! 3. **Plane snapping.** The edges of the lumped blocks and the ducts and
//!    rails are rounded to a pitch, so planes of neighbouring bodies coincide
//!    and the grid keeps few planes. The pitch is the smallest that fits the
//!    cell budget.
//! 4. **Porous smearing.** A duct or rail thinner than the pitch is widened
//!    to the pitch and made proportionally lower, which keeps the displaced
//!    volume and the heat released in it (wires in a duct).
//! 5. **Results come back per original device**: a lumped member gets the air
//!    temperature of its block and a housing temperature from its own loss and
//!    the block's surface film. These are estimates and are labelled as such.
//!
//! What this is not: a validated model. The cabinet result stays "not
//! validated" ([`crate::cabinet::ENCLOSURE_VALIDATED`] is false); the reduced
//! model adds its own errors (mixing of cold neighbours, shifted edges) and
//! says so in the notes and the accuracy limits.
//!
//! Where the user finds it: Cabinet view, Heat, Simulate. A cabinet whose
//! full grid would be far over the cell budget runs reduced automatically and
//! the result says "Reduced model" with the numbers of [`LargeReport`].

mod build;
mod expand;
mod report;

pub use build::{ReduceSettings, Reduction, reduce};
pub use expand::expand;
pub use report::{
    Convergence, LargeReport, REDUCED_SUMMARY_PREFIX, Verdict, convergence, reduced_summary,
};

/// When the cabinet solver uses the reduced model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LargeMode {
    /// Only when the full grid would be more than 1.5 times the cell budget
    /// of the level; a small cabinet is solved as it is.
    #[default]
    Auto,
    /// Never: the full scenario (a grid far over budget, slow).
    Off,
    /// Always, also for a small cabinet (tests, comparison with the full run).
    Force,
}

/// The full grid is this many times over the budget before `Auto` reduces.
pub const AUTO_RATIO: f64 = 1.5;
