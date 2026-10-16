//! EPLAN interop. See `docs/design/interop.md` §5 (parts) and
//! `docs/design/eplan-nets.md` ([`nets`]: nets rebuilt from drawn geometry).
//!
//! Parts import:
//!
//! Reads the parts management XML export and the delimited text export and
//! produces [`novo_parts::CatalogPart`]s in a [`novo_parts::ImportReport`].
//! Nothing here touches the user's catalog: the caller previews and merges.

pub mod export_script;
pub mod nets;
mod parts;
pub mod pxf;
pub mod symbols;
pub(crate) mod xml;

pub use parts::{EplanError, import_auto, import_delimited, import_xml};
