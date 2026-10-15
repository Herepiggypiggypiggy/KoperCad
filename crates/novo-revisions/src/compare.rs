//! Compare any two states of a project: a kept revision or the project as
//! it is now (DESIGN §7: "any two revisions can be picked"). Read only;
//! it never touches the document, the undo history or autosave.

use std::fmt::Display;

use novo_core::{AnyId, Project, RevisionId};

use crate::list::{ChangeRow, list_of};
use crate::{FieldChange, SheetChange, changed_sheets, diff, field_changes};

/// One side of a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The project as it was issued with this revision (its kept snapshot).
    Revision(RevisionId),
    /// The project as it is now.
    Now,
}

/// Why two states could not be compared.
#[derive(Debug, thiserror::Error)]
pub enum CompareError {
    #[error("no revision {0:?} in this project")]
    UnknownRevision(RevisionId),
    /// The revision is issued but its snapshot is not in the file.
    #[error("revision {index} has no kept copy to compare with")]
    NoSnapshot { index: String },
    /// The snapshot could not be read (corrupt, or a format that no
    /// migration upgraded).
    #[error("revision {index} could not be read: {reason}")]
    Unreadable { index: String, reason: String },
}

/// The result: `from` is the older side as the person picked it, so
/// comparing the other way round turns added into removed.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub from: Side,
    pub to: Side,
    /// Named changes in page order, with field values for changed objects.
    pub rows: Vec<ChangeRow>,
    /// Generated sheets (BOM, wire list, ...) whose content differs.
    pub sheets: Vec<SheetChange>,
    /// Project information that differs (`name`, `description`, `rules`,
    /// `properties`).
    pub info: Vec<FieldChange>,
}

impl Comparison {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.sheets.is_empty() && self.info.is_empty()
    }
}

/// Compares `from` with `to`. `snapshot` reads a kept snapshot (for
/// example `Session::snapshot`); `current` is the project as it is now.
pub fn compare<E: Display>(
    current: &Project,
    mut snapshot: impl FnMut(RevisionId) -> Result<Option<Project>, E>,
    from: Side,
    to: Side,
) -> Result<Comparison, CompareError> {
    let mut load = |side: Side| -> Result<Project, CompareError> {
        match side {
            Side::Now => Ok(current.clone()),
            Side::Revision(id) => {
                let index = current
                    .issued_revision(id)
                    .ok_or(CompareError::UnknownRevision(id))?
                    .index
                    .clone();
                match snapshot(id) {
                    Ok(Some(project)) => Ok(project),
                    Ok(None) => Err(CompareError::NoSnapshot { index }),
                    Err(e) => Err(CompareError::Unreadable {
                        index,
                        reason: e.to_string(),
                    }),
                }
            }
        }
    };
    let (old, new) = (load(from)?, load(to)?);
    let set = diff(&old, &new);
    let mut rows = list_of(&set, &old, &new);
    for row in &mut rows {
        row.fields = field_changes(row, &old, &new);
    }
    let info = crate::fields::info_changes(&old, &new, &set.info);
    Ok(Comparison {
        from,
        to,
        rows,
        sheets: changed_sheets(&old, &new),
        info,
    })
}

/// What kind of thing a row is, for grouping a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Page,
    Device,
    Terminal,
    Cable,
    Wire,
    Text,
    /// Symbols and junctions: the drawing itself.
    Drawing,
    Part,
    /// Everything on no page: potentials, rules, structure, blocks and the
    /// rest of the project's own records.
    Project,
}

/// The group a row belongs to.
pub fn group_of(id: AnyId) -> Group {
    match id {
        AnyId::Page(_) => Group::Page,
        AnyId::Device(_) => Group::Device,
        AnyId::Terminal(_) => Group::Terminal,
        AnyId::Cable(_) => Group::Cable,
        AnyId::Wire(_) => Group::Wire,
        AnyId::Text(_) => Group::Text,
        AnyId::Symbol(_) | AnyId::Junction(_) => Group::Drawing,
        AnyId::Part(_) => Group::Part,
        _ => Group::Project,
    }
}
