//! NovoElectric data model (owned by WS-02).
//!
//! Every view in the program (schematic pages, terminal and cable screens,
//! cabinet layout, reports, simulation) is a projection of the model defined
//! here. Other crates read it through queries and change it only through
//! commands, so undo/redo, scripting and AI edits all go through one door.

/// Placeholder until WS-02 lands API v0.
pub const MODEL_VERSION: u32 = 0;
