//! Thermal and airflow simulation of an enclosure (design
//! `docs/design/thermal-simulation.md`).
//!
//! A reduced-order model, not CFD: a depth-averaged 2D slice of the interior
//! seen from the front. Heat from devices goes into the air and the mounting
//! plate; the plate conducts sideways; air moves by buoyancy, fans and vents
//! through a Darcy–Forchheimer flow field with obstacles; walls lose heat by
//! the IEC 60890 coefficient. Every result is an estimate with a stated
//! band, never an authority.

pub mod model;
pub mod result;

pub use model::*;
pub use result::*;
