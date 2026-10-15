//! The link to the PLC engineering tools (docs/design/plc-link.md).
//!
//! The project already knows every channel's card, address and signal. This
//! module writes that as the files the PLC programmer imports, and reads
//! the programmer's file back so renamed tags and reworded comments come
//! home. Addresses only ever go out: they are derived from the cards.
//!
//! - [`tag_set`]: one [`Tag`] per channel, with a unique valid name.
//! - [`export`]: CSV, PLCopen XML, TIA Portal Excel, Studio 5000 CSV, and
//!   the address plan.
//! - [`read`] and [`plan_import`] / [`apply_import`]: preview and apply.

pub mod addr;
pub mod csvio;
pub mod import;
pub mod read;
pub mod tags;
pub mod write;

pub use import::{ImportPlan, Update, apply_import, plan_import};
pub use read::{FileKind, FileTag, ReadError, TagFile, read};
pub use tags::{TAG, Tag, TagOptions, TagSet, identifier, tag_set};
pub use write::{Export, ExportFile, ExportOptions, LinkFormat, export, hardware};
