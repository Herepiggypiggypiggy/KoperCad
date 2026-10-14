//! Errors returned by model edits and loading.

use thiserror::Error;

use crate::id::{AnyId, CableId, DeviceId, PageId, SymbolId, WireId};
use crate::tag::TagError;

/// Why an edit or a load was refused. A refused edit leaves the project
/// exactly as it was.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ModelError {
    #[error("{0} does not exist")]
    NotFound(AnyId),
    #[error("{0} already exists")]
    AlreadyExists(AnyId),
    #[error("{id} is still used by {} other object(s)", by.len())]
    StillReferenced { id: AnyId, by: Vec<AnyId> },
    #[error("{symbol} has no pin {pin}")]
    InvalidPin { symbol: SymbolId, pin: u16 },
    #[error("{id} is on {found}, expected {expected}")]
    WrongPage {
        id: AnyId,
        expected: PageId,
        found: PageId,
    },
    #[error("{0} connects an endpoint to itself")]
    SelfConnection(WireId),
    #[error("{0} is not a terminal strip")]
    NotATerminalStrip(DeviceId),
    #[error("{cable} has no core {core}")]
    InvalidCore { cable: CableId, core: usize },
    #[error("{wire} is already carried by a core of {cable}")]
    WireAlreadyOnCore { wire: WireId, cable: CableId },
    #[error("the recorded state of {0} no longer matches the project")]
    Conflict(AnyId),
    #[error("{0} must keep its id")]
    IdChanged(AnyId),
    #[error(transparent)]
    Tag(#[from] TagError),
    #[error("unsupported project format version {0}")]
    UnsupportedFormat(u32),
    #[error("invalid project data: {0}")]
    Format(String),
}

/// Result type for model operations.
pub type Result<T, E = ModelError> = std::result::Result<T, E>;
