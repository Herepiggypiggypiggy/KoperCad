//! Pages from an I/O list: read a machine's signal list (CSV or Excel),
//! plan the PLC cards it needs and build a page per card with the
//! channels, their field terminals and the wires between them.
//!
//! - [`read_list`]: the list, with the rows it could not use.
//! - [`plan`]: the cards, the channel each signal gets and the notes for
//!   the review. Signals already in the project are left alone, so a
//!   changed list can be imported again.
//! - [`apply`]: builds the plan inside one transaction, one undo step.
//!
//! Addresses stay derived from the cards (plc.md §3). When the list
//! carries addresses, the cards start where those addresses need and
//! unused channels in between stay spare, so the addresses come out as the
//! list wrote them; otherwise the project assigns them in order.

pub mod apply;
pub mod list;
pub mod plan;

pub use apply::{Built, apply};
pub use list::{IoItem, IoList, ListError, Note, read_list};
pub use plan::{Options, Plan, PlannedCard, plan};

/// On a channel symbol: the sensor or actuator the list names (`-B1`).
pub const FIELD: &str = "plc.field";
