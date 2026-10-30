//! The larger example projects (File > Templates and examples): each is drawn with the
//! editor's own commands over several pages, with a laid-out cabinet, and has
//! to open without a single warning or error in Check.

use novo_core::{ChangeSet, Document, Result as ModelResult};

use crate::actions::Action;

/// One example project in the gallery.
pub(crate) struct Entry {
    /// The command that opens it (File > Templates and examples, command palette).
    pub action: Action,
    /// The project name it opens as.
    pub name: &'static str,
    /// The cabinet location that holds its panel (`+CAB`).
    pub cabinet: &'static str,
    pub build: fn(&mut Document) -> ModelResult<ChangeSet>,
}

/// Every gallery example, in the order of the Start centre.
pub(crate) const GALLERY: &[Entry] = &[
    Entry {
        action: Action::OpenMccExample,
        name: super::example_mcc::MCC_EXAMPLE_NAME,
        cabinet: "+CAB",
        build: super::example_mcc::build_mcc_example,
    },
    Entry {
        action: Action::OpenCraneExample,
        name: super::example_crane::CRANE_EXAMPLE_NAME,
        cabinet: "+CAB",
        build: super::example_crane::build_crane_example,
    },
    Entry {
        action: Action::OpenPlcExample,
        name: super::example_plc::PLC_EXAMPLE_NAME,
        cabinet: "+CAB",
        build: super::example_plc::build_plc_example,
    },
    Entry {
        action: Action::OpenPumpExample,
        name: super::example_pump::PUMP_EXAMPLE_NAME,
        cabinet: "+CAB",
        build: super::example_pump::build_pump_example,
    },
];
