//! The flow core: incompressible Navier-Stokes with Boussinesq buoyancy and
//! the energy equation on a 3D staggered grid.
//!
//! Method: fractional step. Convection explicit (TVD or QUICK), diffusion
//! implicit, then a pressure projection (multigrid-preconditioned CG) makes
//! the velocity divergence free. Walls only (closed domain); openings come
//! with the solids and openings lane.

mod mesh;
mod pressure;
mod scheme;
mod solver;

pub use mesh::Mesh;
pub use pressure::PressureSolver;
pub use scheme::Scheme;
pub use solver::{Face, FaceBc, FlowConfig, FlowSolver, ThermalBc};
