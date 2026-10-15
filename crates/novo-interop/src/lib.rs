//! Import and export (WS-24). See `docs/design/interop.md`.
//!
//! - [`dxf`]: ASCII DXF reader and writer on a small neutral [`dxf::Drawing`]
//!   (DXF axes: y up, degrees counter-clockwise, millimetres once scaled).
//! - [`eplan`]: EPLAN parts (XML and delimited export) into catalog parts.
//!
//! Everything here is a pure function from text to values and back: no
//! files, no dialogs, nothing that needs elevation. Callers own the disk.

pub mod bridge;
pub mod dxf;
pub mod eplan;
pub mod part3d;
