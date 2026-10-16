//! The flow core: incompressible Navier-Stokes with Boussinesq buoyancy and
//! the energy equation on a 3D staggered grid.
//!
//! Method: fractional step. Convection explicit (TVD or QUICK), diffusion
//! implicit, then a pressure projection (multigrid-preconditioned CG) makes
//! the velocity divergence free. The domain is closed unless
//! [`FlowConfig::openings`] opens patches of the walls to the outside air
//! (see [`crate::openings`]).

mod mesh;
mod par;
mod pressure;
mod scheme;
mod sheet;
mod solver;

pub use mesh::{Mesh, other_axes};
pub use pressure::{PressureSolver, Smoother};
pub use scheme::Scheme;
pub use sheet::PlateSheet;
pub use solver::{Face, FaceBc, FlowConfig, FlowSolver, ThermalBc};
