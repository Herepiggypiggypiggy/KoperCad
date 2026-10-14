//! NovoElectric data model (owned by WS-02).
//!
//! Every view in the program (schematic pages, terminal and cable screens,
//! cabinet layout, reports, simulation) is a projection of the model defined
//! here. Other crates read it through queries and change it only through
//! commands, so undo/redo, scripting and AI edits all go through one door.
//!
//! # Shape of the API
//!
//! - [`Project`] holds every object in typed tables keyed by typed ids
//!   ([`PageId`], [`DeviceId`], ...). Reads are plain method calls.
//! - Edits run inside [`Project::transact`] or, with undo, [`Document::transact`].
//!   The closure gets a [`Tx`]; each step is checked (no dangling references,
//!   pins exist, wires stay on one page) and a failed edit leaves nothing behind.
//! - [`Command`] is the same set of edits as serializable data, for macros,
//!   Python (WS-28) and the AI assistant (WS-23).
//! - [`Netlist`] is the connectivity graph, derived on demand.
//!
//! # JSON wire format
//!
//! Storage (WS-04), scripting (WS-28) and the AI assistant exchange model
//! values as JSON, so these tag keys are stable API:
//!
//! - [`Record`]: `{"record": "device", "id": 5, ...}`
//! - [`Op`]: `{"op": "insert" | "remove" | "update" | "set_info", ...}`
//! - [`Command`]: `{"command": "add_device", ...}`
//! - [`AnyId`]: `{"kind": "device", "id": 5}`
//! - [`Endpoint`], [`SymbolRole`] and [`PageKind`]: `{"type": ..., ...}`
//!
//! Typed ids serialize as bare numbers and [`DeviceTag`] as its text form
//! (`"=A1+CAB1-K1"`).
//!
//! A saved project (`Project::to_json`) carries `"format":`
//! [`JSON_FORMAT_VERSION`]. `tests/fixtures/format_v1.json` pins that format:
//! a build must load every earlier fixture, and a change to what is written
//! fails `tests/format.rs` until the fixture is updated on purpose. New
//! optional fields with serde defaults keep the version; anything an older
//! build could not read bumps it.
//!
//! ```
//! use novo_core::{Document, DeviceTag, Endpoint, Pin, Placement, Point, SymbolRole};
//!
//! let mut doc = Document::default();
//! let (k1_a1, _) = doc.transact("Place coil", |tx| {
//!     let page = tx.add_page("1")?;
//!     let k1 = tx.add_device(DeviceTag::parse("-K1")?, "Main contactor")?;
//!     let coil = tx.place_symbol(
//!         page,
//!         "iec/coil",
//!         Placement::at(Point::new(100.0, 50.0)),
//!         vec![Pin::new("A1", 0.0, -5.0), Pin::new("A2", 0.0, 5.0)],
//!         SymbolRole::Device { device: k1, main: true },
//!     )?;
//!     Ok(Endpoint::pin(coil, 0))
//! })?;
//! assert_eq!(doc.project().endpoint_position(k1_a1), Some(Point::new(100.0, 45.0)));
//! doc.undo()?;
//! assert_eq!(doc.project().devices().count(), 0);
//! # Ok::<(), novo_core::ModelError>(())
//! ```

mod command;
mod document;
mod entity;
mod error;
mod geom;
mod graph;
mod id;
mod json;
mod project;
mod record;
mod search;
mod tag;
mod text;
mod tx;
mod xref;

pub use command::Command;
pub use document::{DEFAULT_HISTORY_LIMIT, Document, Outcome};
pub use entity::{
    Cable, CableCore, Device, DeviceKind, Dismissal, Endpoint, JoinLink, Junction, Page,
    PageFormat, PageFreeze, PageKind, PaperSize, Part, Pin, PinFeed, PinRef, Potential,
    PotentialKind, PotentialRules, PotentialSet, PotentialSpec, PotentialSystem, ProjectInfo,
    ProjectRules, Properties, SameName, Structure, SymbolInstance, SymbolKey, SymbolRole, Terminal,
    TerminalKind, TerminalRules, Text, TextAnchor, TextContent, TextPlacement, Value, Wire,
};
pub use error::{ModelError, Result};
pub use geom::{Direction, Placement, Point, Rotation};
pub use graph::{Net, NetId, Netlist, PotentialSource};
pub use id::{
    AnyId, CableId, DeviceId, DismissalId, EntityKind, JunctionId, PageId, PartId, PotentialId,
    PotentialSystemId, StructureId, SymbolId, TerminalId, TextId, WireId,
};
pub use json::JSON_FORMAT_VERSION;
pub use project::{PageContents, Project, RATED_CURRENT_A, natural_cmp};
pub use record::{ChangeSet, Entity, Op, Record, Transaction};
pub use search::{Match, SearchHit};
pub use tag::{Aspect, DeviceTag, TagError, split_code};
pub use tx::Tx;
pub use xref::{CrossReference, Location};

/// Version of the public model API. Bumped when a frozen API changes.
pub const MODEL_VERSION: u32 = 0;
