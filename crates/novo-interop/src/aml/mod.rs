//! AutomationML (IEC 62714, CAEX 2.15): the plant out to any engineering tool
//! that reads it, and a plant from another tool in. See
//! `docs/design/interop-migration.md`.
//!
//! - [`export_project`] writes the structure, devices, terminals, cables and
//!   nets of a project.
//! - [`read_aml`] reads KoperCAD's own file or one from another tool into a
//!   [`Plant`]; [`plan_import`] previews it against the open project and
//!   [`AmlPlan::apply`] adds it as one undo step.
//!
//! AutomationML is an open standard: nothing here uses a vendor SDK or any
//! vendor's data.

mod caex;
mod draw;
mod plan;
mod read;
mod write;

pub use plan::{AmlPlan, plan_import};
pub use read::{
    AmlError, Plant, PlantCable, PlantCore, PlantDevice, PlantNet, PlantPage, PlantPin,
    PlantSegment, PlantStructure, PlantSymbol, PlantTerminal, read_aml,
};
pub use write::{AmlExport, AmlOptions, INTERFACE_LIB, ROLE_LIB, export_project};
