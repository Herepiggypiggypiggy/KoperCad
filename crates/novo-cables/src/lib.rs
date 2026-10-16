//! Cables (WS-12): cable types, cable proposals at location boundaries,
//! core assignment, shields, checks and the data behind cable connection
//! screens.
//!
//! - [`kind`]: cable types (`4G1.5`, `7x1`, shielded variants), standard
//!   colour codes and the catalog parts that match them.
//! - [`propose`]: wires leaving one location for another (`+CAB` to
//!   `+FLD`) between the same two devices become one proposed cable;
//!   accepting puts every wire on its core (L1, L2, L3, N, PE) in one undo
//!   step; declined proposals are remembered.
//! - [`edit`]: assign, unassign and swap cores, spare cores, shields.
//! - [`check`]: spare cores in use, too few cores, PE off the green-yellow
//!   core, shields not connected.
//! - [`screen`]: each cable with both ends of every core, and the overview.
//!
//! Nothing here touches the model directly: reads go through
//! [`novo_core::Project`] queries and every change through a
//! [`novo_core::Tx`], so it undoes like any other edit.

pub mod check;
pub mod edit;
pub mod ends;
pub mod kind;
pub mod propose;
pub mod screen;
pub mod strings;

pub use check::{Finding, check, check_with};
pub use edit::{
    ShieldConnection, add_shield, assign_core, connect_shield, delete_cable, is_spare,
    rename_cable, retype_cable, set_spare, shield_connection, spare_cores, swap_cores,
    unassign_core, unassign_wire,
};
pub use kind::{CableType, default_types, type_label};
pub use propose::{
    CableProposal, CableSide, Conductor, accept_proposals, declined_wires, dismiss_proposal,
    proposal_for_wires, propose_cables, restore_proposal, summarize_proposals,
};
pub use screen::{
    CableScreen, CoreEnd, CoreRow, OverviewRow, cable_screen, cable_screens, overview,
};
