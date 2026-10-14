//! Errors returned by model edits and loading.

use thiserror::Error;

use crate::id::{AnyId, CableId, DeviceId, PageId, SymbolId, TerminalId, TextId, WireId};
use crate::tag::TagError;

/// Why an edit or a load was refused. A refused edit leaves the project
/// exactly as it was.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ModelError {
    #[error("{0} does not exist")]
    NotFound(AnyId),
    #[error("{0} already exists")]
    AlreadyExists(AnyId),
    #[error("id {0} is reserved and cannot be used")]
    InvalidId(u64),
    #[error("the recorded project info no longer matches the project")]
    InfoConflict,
    #[error("{id} has a value in `{field}` that is not a finite number")]
    NonFinite { id: AnyId, field: &'static str },
    #[error("the project info has a property that is not a finite number")]
    NonFiniteInfo,
    #[error("{0} cannot be split at one of its ends")]
    SplitAtEnd(WireId),
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
    #[error("{id}: `{field}` is out of range")]
    InvalidValue { id: AnyId, field: &'static str },
    #[error("{text} cannot be attached to {target}; only symbols, junctions and wires carry texts")]
    InvalidAttachment { text: TextId, target: AnyId },
    #[error(
        "{0} has the same name as another terminal on its strip, which the project rule refuses"
    )]
    DuplicateTerminal(TerminalId),
    #[error("{0} connects an endpoint to itself")]
    SelfConnection(WireId),
    #[error("{wires} wire(s) but only {potentials} potential(s) to assign")]
    TooFewPotentials { wires: usize, potentials: usize },
    #[error(
        "{symbol} lists {terminals} terminal(s) but has {bridges} bridge group(s); a multi-level terminal needs one group per distinct terminal"
    )]
    TerminalLevels {
        symbol: SymbolId,
        terminals: usize,
        bridges: usize,
    },
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
