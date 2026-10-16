//! EPLAN parts import. See `docs/design/interop.md` §5.
//!
//! Reads the parts management XML export and the delimited text export and
//! produces [`novo_parts::CatalogPart`]s in a [`novo_parts::ImportReport`].
//! Nothing here touches the user's catalog: the caller previews and merges.

mod parts;
pub(crate) mod xml;

pub use parts::{EplanError, import_auto, import_delimited, import_xml};
