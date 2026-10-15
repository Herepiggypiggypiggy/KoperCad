//! Typed identifiers.
//!
//! Every object in a project gets a `u64` id from one project-wide counter, so an
//! id is unique across all kinds and is never reused, not even after undo. The
//! typed wrappers keep a `WireId` from being passed where a `DeviceId` is
//! expected, and [`AnyId`] names any object (selections, change sets, errors).

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! define_ids {
    ($($(#[$doc:meta])* $name:ident => $kind:ident, $label:literal;)*) => {
        $(
            $(#[$doc])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
            #[serde(transparent)]
            pub struct $name(pub u64);

            impl $name {
                /// The raw numeric id, for storage and scripting.
                pub fn raw(self) -> u64 {
                    self.0
                }
            }

            impl From<u64> for $name {
                fn from(raw: u64) -> $name {
                    $name(raw)
                }
            }

            impl From<$name> for AnyId {
                fn from(id: $name) -> AnyId {
                    AnyId::$kind(id)
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(f, concat!($label, "#{}"), self.0)
                }
            }
        )*

        /// An id of any kind of project object.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(tag = "kind", content = "id", rename_all = "snake_case")]
        pub enum AnyId {
            $($kind($name),)*
        }

        impl AnyId {
            /// The raw numeric id. Unique across all kinds within one project.
            pub fn raw(self) -> u64 {
                match self {
                    $(AnyId::$kind(id) => id.0,)*
                }
            }

            /// The kind of object this id names.
            pub fn kind(self) -> EntityKind {
                match self {
                    $(AnyId::$kind(_) => EntityKind::$kind,)*
                }
            }
        }

        impl fmt::Display for AnyId {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(AnyId::$kind(id) => id.fmt(f),)*
                }
            }
        }

        /// The kinds of objects a project holds.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum EntityKind {
            $($kind,)*
        }
    };
}

define_ids! {
    /// A page (schematic sheet, overview, generated report page).
    PageId => Page, "Page";
    /// A logical device such as contactor `-K1`, independent of where it is drawn.
    DeviceId => Device, "Device";
    /// A symbol placed on a page.
    SymbolId => Symbol, "Symbol";
    /// A junction (connection dot / T-junction) on a page.
    JunctionId => Junction, "Junction";
    /// A drawn wire between two endpoints.
    WireId => Wire, "Wire";
    /// A named potential such as `L1`, `N`, `PE` or `24V`.
    PotentialId => Potential, "Potential";
    /// A single terminal on a terminal strip.
    TerminalId => Terminal, "Terminal";
    /// A cable with its cores.
    CableId => Cable, "Cable";
    /// A part (manufacturer article) used in this project.
    PartId => Part, "Part";
    /// A text or label on a page.
    TextId => Text, "Text";
    /// A supply system grouping potentials (`400 V 3/N/PE AC`).
    PotentialSystemId => PotentialSystem, "PotentialSystem";
    /// A node of the plant and location structure (`=A1`, `+CAB1`).
    StructureId => Structure, "Structure";
    /// A proposal the user declined, so it does not come back.
    DismissalId => Dismissal, "Dismissal";
    /// A parametric circuit kept in the project, to place as blocks.
    BlockDefinitionId => BlockDefinition, "BlockDefinition";
    /// One placement of a block definition.
    BlockInstanceId => BlockInstance, "BlockInstance";
    /// An issued revision of the drawing set.
    RevisionId => Revision, "Revision";
    /// A DIN rail or wiring duct on the mounting plate of an enclosure.
    RailId => Rail, "Rail";
    /// A device placed on the mounting plate of an enclosure.
    CabinetPlacementId => CabinetPlacement, "CabinetPlacement";
}
