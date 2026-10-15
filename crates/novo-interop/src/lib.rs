//! Import and export (WS-24). See `docs/design/interop.md`.
//!
//! - [`dxf`]: ASCII DXF reader and writer on a small neutral [`dxf::Drawing`]
//!   (DXF axes: y up, degrees counter-clockwise, millimetres once scaled).
//! - [`eplan`]: EPLAN parts (XML and delimited export) into catalog parts.
//! - [`step`]: the cabinet as a STEP assembly (AP214) for mechanical CAD.
//! - [`report`]: what an import or export carried and what it left out.
//! - [`sheets`]: the spreadsheet round trip (devices, terminals, wires and
//!   cables to `.xlsx` or CSV, edited in bulk, previewed and imported back).
//!
//! Everything here is a pure function from text to values and back: no
//! files, no dialogs, nothing that needs elevation. Callers own the disk.

pub mod aml;
pub mod bridge;
pub mod dwg;
pub mod dxf;
pub mod eplan;
pub mod part3d;
pub mod plate;
pub mod report;
pub mod sheets;
pub mod step;
pub mod underlay;
pub mod wiring;
