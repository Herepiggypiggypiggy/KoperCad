//! EPLAN project exchange (PXF): the `.epj` project export and the `.emp`
//! page macro, read into a [`PxfFile`].
//!
//! PXF is plain XML (`<EplanPxfRoot ...>`) with numbered elements: `O4` is a
//! page, `O59` a cable core, `O70` an interruption point, `P150` a device
//! tag. The reader streams: it never builds the whole tree, so a project of
//! 200 MB reads in constant memory.
//!
//! What is exact and what is approximated:
//! - Exact: the element and attribute numbers named in the field table of
//!   `docs/design/eplan-pxf.md`, taken from projects exported by one EPLAN
//!   release (PxfVersion 1.23). Page numbers, tag parts, part numbers, terminal
//!   and cable texts, core numbers, positions and ids are read as written.
//! - Approximated: which object types are devices (any object with a tag),
//!   which property holds a free text (the first multi-language value), and
//!   which page an object sits on (the enclosing page, else the last one
//!   seen). Nothing of the drawing (lines, arcs, symbol graphics) is carried
//!   yet; that is counted in [`report`].
//!
//! No EPLAN content is in this crate: the tests read fixtures written for
//! them.

mod dialect;
mod draw;
mod graphics;
mod layout;
mod layout_apply;
mod library;
mod model;
mod plan;
mod read;
mod stream;
mod survey;
mod text;
mod write;

use std::io::BufRead;

use thiserror::Error;

use crate::report::Report;

pub use dialect::{Dialect, EPLAN_2024};
pub use graphics::{GraphicsStats, PxfShape, ShapeKind, background_properties};
pub use layout::{LayoutDevice, LayoutDuct, LayoutRail, LayoutSheet};
pub use library::{DOWN, LEFT, LibFunction, LibPort, LibSymbol, Match, PxfLibrary, RIGHT, UP};
pub use model::{
    DeviceRole, ObjId, PageClass, PxfCore, PxfDevice, PxfFile, PxfHeader, PxfInterruption, PxfKind,
    PxfNode, PxfPage, PxfProjectData, PxfStructure, PxfTag, PxfText, SymbolRef, pair_interruptions,
    split_part_number,
};
pub use plan::{PxfOptions, PxfPlan, plan_pxf};
pub use survey::{PxfSurvey, survey_pxf};
pub use text::MlText;
pub use write::{PxfExport, PxfWriteOptions, WRITER_MARK, is_kopercad_pxf, write_pxf};

/// The PXF version the reader was written against.
pub const TESTED_PXF_VERSION: &str = "1.23";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PxfError {
    #[error("the file is empty")]
    Empty,
    #[error("this is not a PXF file (it starts with <{0}>, not <EplanPxfRoot>)")]
    NotPxf(String),
    #[error("line {0}: the file is cut off")]
    Truncated(usize),
    #[error("line {0}: a tag has no name")]
    BadTag(usize),
    #[error("line {0}: a single tag or text is larger than 64 MB")]
    TooLarge(usize),
    #[error("the file nests deeper than {0} levels")]
    TooDeep(usize),
    #[error("reading the file failed: {0}")]
    Io(String),
}

/// Reads a PXF project or macro from text.
pub fn read_pxf_str(text: &str) -> Result<PxfFile, PxfError> {
    read::read(text.trim_start_matches('\u{feff}').as_bytes())
}

/// Reads a PXF project or macro from a stream (a file, for example). The
/// stream is read once, front to back.
pub fn read_pxf(r: impl BufRead) -> Result<PxfFile, PxfError> {
    read::read(r)
}

/// Why each skipped object type is not carried yet.
fn skipped_name(kind: u32) -> (&'static str, &'static str) {
    match kind {
        6 => (
            "structure lists",
            "structure identifiers are read from the pages",
        ),
        14 => (
            "function definition blocks",
            "function data is not mapped yet",
        ),
        20 => (
            "symbol definitions",
            "EPLAN's symbol library is the user's own; symbols come in with the symbol mapping",
        ),
        26 => ("groups", "grouping is not carried"),
        31 | 32 | 34 | 88 | 89 => (
            "lines",
            "drawing objects with no usable geometry in the file",
        ),
        _ => ("other objects", "object type not known to this reader"),
    }
}

impl PxfFile {
    /// What this file holds and what the reader leaves behind, counted.
    pub fn report(&self) -> Report {
        let mut r = Report::default();
        r.moved("pages", self.pages.len());
        r.moved("devices", self.devices_of(DeviceRole::Device).count());
        r.moved("terminals", self.devices_of(DeviceRole::Terminal).count());
        r.moved("cables", self.devices_of(DeviceRole::Cable).count());
        r.moved("cable cores", self.cores.len());
        r.moved("interruption points", self.interruptions.len());
        r.moved("connection nodes", self.nodes.len());
        r.moved("texts", self.texts.len());
        let mut other = 0;
        let mut by_name: Vec<(&str, usize, &str)> = Vec::new();
        for (kind, n) in &self.skipped {
            let (what, why) = skipped_name(*kind);
            if what == "other objects" {
                other += n;
            } else if let Some(e) = by_name.iter_mut().find(|e| e.0 == what) {
                e.1 += n;
            } else {
                by_name.push((what, *n, why));
            }
        }
        for (what, n, why) in by_name {
            r.lost(what, n, why);
        }
        r.lost(
            "other objects",
            other,
            "object type not known to this reader",
        );
        if !self.library.is_empty() {
            r.note(format!(
                "The file embeds its symbol library ({} symbols, {} function definitions). It is read for pin positions and names and not carried over; {} library drawing objects were skipped.",
                self.library.symbol_count(),
                self.library.function_count(),
                self.library_objects.values().sum::<usize>()
            ));
        }
        let (_, alone) = pair_interruptions(&self.interruptions);
        if !alone.is_empty() {
            r.note(format!(
                "{} interruption points have no partner with the same name (EPLAN leaves an odd one unpaired too).",
                alone.len()
            ));
        }
        if !self.duplicate_ids.is_empty() {
            r.note(format!(
                "{} object ids occur twice in the file.",
                self.duplicate_ids.len()
            ));
        }
        for n in &self.notes {
            r.note(n.clone());
        }
        r
    }
}

#[cfg(test)]
mod tests;
