//! DXF reader and writer.
//!
//! The model is DXF-native: y up, angles in degrees counter-clockwise.
//! Flipping to KoperCAD's y-down page is the bridge's job, not this
//! module's.

mod model;
mod read;
mod write;

pub use model::{Drawing, Entity, EntityKind, HAlign, Layer, Point, TextAlign, VAlign};
pub use read::{DxfError, DxfRead, read_dxf};
pub use write::write_dxf;
