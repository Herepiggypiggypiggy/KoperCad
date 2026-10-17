//! Design checks (owned by WS-14).
//!
//! Problems are derived from the model, never stored: [`check`] reads a
//! [`Project`] through its queries and returns every [`Problem`] it finds, in
//! reading order, so the Issues list and F8 / Shift+F8 can walk them and the
//! canvas can mark each one in place (docs/DESIGN.md §2, §3, §5). Checks
//! never edit the project and never block anything: export, save and
//! editing go on whatever they report.
//!
//! What is checked, by [`ProblemKind`]:
//!
//! | Kind | Severity | Finds |
//! | --- | --- | --- |
//! | [`DuplicateTag`](ProblemKind::DuplicateTag) | error | One tag on several devices or cables |
//! | [`TooManyContacts`](ProblemKind::TooManyContacts) | error | More contacts of a kind drawn than the device has (`contacts.main/no/nc/co` device properties) |
//! | [`PotentialConflict`](ProblemKind::PotentialConflict) | error | Two potentials defined on one net (a short) |
//! | [`UnconnectedPin`](ProblemKind::UnconnectedPin) | warning | Connection points no wire ends on |
//! | [`LoneInterruption`](ProblemKind::LoneInterruption) | warning | An interruption point without a partner of the same name |
//! | [`NoMainFunction`](ProblemKind::NoMainFunction) | warning | A device drawn only through contacts, without its coil |
//! | [`SeveralMainFunctions`](ProblemKind::SeveralMainFunctions) | warning | A device drawn with more than one main function |
//! | [`DuplicateTerminal`](ProblemKind::DuplicateTerminal) | warning | Two terminals with one name on a strip while the same-name rule is off |
//! | [`ZeroLengthWire`](ProblemKind::ZeroLengthWire) | warning | A wire of zero length that is not two touching pins |
//! | [`WrongTerminalBlock`](ProblemKind::WrongTerminalBlock) | warning | A PE terminal built with a block the catalog does not mark PE, or another terminal built with a block it does |
//! | [`UndersizedWire`](ProblemKind::UndersizedWire) | warning | A wire on an overcurrent protective device that carries less than the device's rated current |
//! | [`CableCore`](ProblemKind::CableCore) | warning, error for green-yellow misuse | A cable core used against its marking or the cable's limits (novo-cables' checks) |
//! | [`TerminalUse`](ProblemKind::TerminalUse) | warning, error for a wrong potential or a one-sided fuse terminal | A terminal used against its kind (novo-terminals' checks) |
//! | [`Plc`](ProblemKind::Plc) | warning, error where the I/O list would be wrong | A PLC card or channel that cannot be addressed or wired as drawn (novo-plc's checks) |
//! | [`WireOverlap`](ProblemKind::WireOverlap) | warning | Two wires on a page sharing a stretch of line |
//! | [`OpenWire`](ProblemKind::OpenWire) | warning | A wire with an open end that is connected to nothing |
//! | [`MissingPart`](ProblemKind::MissingPart) | info | A device or cable without a part |
//! | [`VoltageDrop`](ProblemKind::VoltageDrop) | warning, error above twice the limit | A wire whose voltage drop exceeds the limit (calc::drop) |
//! | [`ShortCircuit`](ProblemKind::ShortCircuit) | error | A device that cannot break the supply's short-circuit current |
//! | [`DisconnectionTime`](ProblemKind::DisconnectionTime) | error | A wire so long that an earth fault cannot trip its protective device in time |
//! | [`ThermalWithstand`](ProblemKind::ThermalWithstand) | error | A wire that cannot take the let-through energy of its protective device |
//! | [`CableRating`](ProblemKind::CableRating) | error | A wire whose cable carries less than its protective device allows, or the load draws more than the device is rated for |
//! | [`MotorStarter`](ProblemKind::MotorStarter) | warning, error where it cannot work | A motor starter whose relay, contactor or fuse does not suit the motor |
//! | [`ProtectiveConductor`](ProblemKind::ProtectiveConductor) | warning | A protective conductor thinner than its phase conductors need |
//! | [`UnplacedDevice`](ProblemKind::UnplacedDevice) | info | A device not yet placed on a mounting plate |
//! | [`WireWithoutDuct`](ProblemKind::WireWithoutDuct) | warning | A wire between placed devices that no duct serves |
//! | [`OverfullDuct`](ProblemKind::OverfullDuct) | warning | A duct filled beyond its limit |
//! | [`UnsizedDuct`](ProblemKind::UnsizedDuct) | info | A duct with wires through it but no cross-section, so its fill cannot be checked |
//! | [`GuessedWire`](ProblemKind::GuessedWire) | info | Wires with no cross-section, routed with a default size (one problem for all of them) |
//! | [`AmbiguousInterruption`](ProblemKind::AmbiguousInterruption) | warning | A name used by more than two interruption points |
//! | [`WireNumber`](ProblemKind::WireNumber) | warning | A wire number used on two nets |
//! | [`UntaggedDevice`](ProblemKind::UntaggedDevice) | warning | A device without a product level in its tag |
//! | [`UnnamedTerminal`](ProblemKind::UnnamedTerminal) | warning | A terminal without a number or name |
//! | [`SameShortTag`](ProblemKind::SameShortTag) | info | One short tag under two function or location levels |
//! | [`DuplicatePage`](ProblemKind::DuplicatePage) | warning | Two pages with one name in one structure |
//! | [`DanglingJunction`](ProblemKind::DanglingJunction) | warning | A junction no wire ends on |
//! | [`UnwiredTerminal`](ProblemKind::UnwiredTerminal) | warning | A terminal or plug pin with no wire on any side |
//! | [`StrayWireEnd`](ProblemKind::StrayWireEnd) | warning | An open wire end that lies on a pin, junction or wire without joining it |
//! | [`ContactType`](ProblemKind::ContactType) | warning | A contact whose pin numbers do not match NO, NC or changeover |
//! | [`ContactPosition`](ProblemKind::ContactPosition) | warning | A contact position beyond the contacts the device has |
//! | [`ContactDrawnTwice`](ProblemKind::ContactDrawnTwice) | warning | One physical contact drawn more than once |
//! | [`VoltageClassMix`](ProblemKind::VoltageClassMix) | warning | Extra-low and low voltage under one function code |
//! | [`ConductorContinuity`](ProblemKind::ConductorContinuity) | warning | A PE or N terminal that does not reach its conductor |
//! | [`MotorPhases`](ProblemKind::MotorPhases) | warning | A motor whose windings cannot get three different phases |
//! | [`CoilSupply`](ProblemKind::CoilSupply) | warning | A coil not fed between a control voltage and its return |
//! | [`PlugPinOneSided`](ProblemKind::PlugPinOneSided) | warning | A plug pin wired on one half only |
//! | [`RelaySize`](ProblemKind::RelaySize) | warning | A relay larger than the contacts drawn need, when its sizes are declared |
//! | [`CableText`](ProblemKind::CableText) | warning | A cable whose cores, cross-section or protective core disagree with its `4G1.5` text |
//! | [`NeutralUse`](ProblemKind::NeutralUse) | warning | A device on a neutral net that is neither supply nor external (opt-in) |
//! | [`PlugPins`](ProblemKind::PlugPins) | info | Overview of the pins of a plug, wired and free |
//! | [`TagRule`](ProblemKind::TagRule) | warning | A device name that breaks the page and column rule (opt-in) |
//! | [`DisplayedTag`](ProblemKind::DisplayedTag) | warning | A tag text on a symbol that is not the device's tag |
//! | [`StructureCode`](ProblemKind::StructureCode) | warning | A function or location code not in a structure list declared complete |
//! | [`Standard`](ProblemKind::Standard) | per rule | A rule of a selected standard (IEC 60204-1, NFPA 79, UL 508A) is not met; see [`standards`] |
//! | [`DeviceCollision`](ProblemKind::DeviceCollision) | error | Two devices overlapping on a mounting plate |
//! | [`DeviceOutsidePlate`](ProblemKind::DeviceOutsidePlate) | error | A device beyond the usable area of its plate |
//! | [`DevicesTooClose`](ProblemKind::DevicesTooClose) | warning | Two devices closer than their clearance |
//! | [`RailOverflow`](ProblemKind::RailOverflow) | warning | Devices needing more room than their DIN rail has |
//! | [`DuctOverlap`](ProblemKind::DuctOverlap) | error | A wiring duct lying over a DIN rail or a device on the plate |
//! | [`DeviceTooDeep`](ProblemKind::DeviceTooDeep) | error | A device deeper than the room between plate and door |
//! | [`DoorGapTight`](ProblemKind::DoorGapTight) | warning | A device that leaves less than the minimum gap to the door |
//! | [`DepthUnknown`](ProblemKind::DepthUnknown) | info | A placed device without a depth, so the door was not checked |
//! | [`PlateTooBig`](ProblemKind::PlateTooBig) | error | A plate larger than the inside of its enclosure |
//! | [`SafetyLevelNotMet`](ProblemKind::SafetyLevelNotMet) | error | A safety function whose achieved performance level is below the required one (ISO 13849-1 calculation aid) |
//! | [`SafetyBinding`](ProblemKind::SafetyBinding) | warning | A safety function block bound to a device that is missing or states no reliability data |
//! | [`DoorSwing`](ProblemKind::DoorSwing) | warning | An open door that needs more room outside than the site allows, or a device on the door that touches a wall or a plate device as the door opens |
//!
//! A problem can be waived with a reason ([`waive`]); [`check_open`] leaves
//! waived problems out and [`waived_problems`] lists them for the report.
//!
//! Not checked here:
//! - Wires beyond the next device on a protective device's load side (past
//!   a contactor, say): the undersized-wire check follows the load-side net
//!   through terminals and cable cores, not through other devices' poles.
//! - A wire on two cable cores: novo-core refuses it on every edit and load
//!   (`ModelError::WireAlreadyOnCore`), so it cannot occur.
//!
//! ```
//! use novo_checks::{Severity, Summary, check};
//! use novo_core::Project;
//!
//! let project = Project::new("Empty");
//! let problems = check(&project);
//! assert!(problems.is_empty());
//! assert_eq!(Summary::of(&problems).pill_text(), "No issues");
//! ```

pub mod calc;
mod calc_checks;
pub mod calc_view;
mod checks;
mod connection_rules;
mod contacts;
mod door;
mod house_rules;
mod more_rules;
mod physical;
pub mod protection;
pub mod safety;
pub mod standards;
mod waiver;

use std::cmp::Ordering;
use std::collections::HashMap;

use novo_core::{
    AnyId, DeviceId, Endpoint, Netlist, PageId, Point, Project, SymbolInstance, SymbolRole,
    TerminalId,
};

pub use contacts::{ContactKind, contact_limit_property};
pub use waiver::{Waived, check_open, report_lines, waive, waived_problems, waiver_of};

/// How bad a problem is. Orders from most to least severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// Wrong: the drawing says something that cannot be built as drawn.
    Error,
    /// Probably wrong or unfinished.
    Warning,
    /// Worth knowing, nothing wrong yet (a device still waiting for a part).
    Info,
}

/// What a problem is about. Each kind has one fixed [`Severity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProblemKind {
    /// A connection point of a placed symbol that no wire ends on. One
    /// problem per symbol, naming each of its free points.
    UnconnectedPin,
    /// One tag used by several devices or cables.
    DuplicateTag,
    /// Terminals of one strip sharing a name and level while the project's
    /// same-name rule is off, so they are separate points that read alike.
    DuplicateTerminal,
    /// A device drawn only through auxiliary functions (contacts), with no
    /// main function (coil).
    NoMainFunction,
    /// A device drawn with more than one main function.
    SeveralMainFunctions,
    /// More contacts of one kind drawn than the device has.
    TooManyContacts,
    /// An interruption point with no other interruption point of its name.
    LoneInterruption,
    /// Two or more potentials defined on one net: a short between them.
    PotentialConflict,
    /// A wire whose ends coincide, other than two touching pins.
    ZeroLengthWire,
    /// A device or cable without a part.
    MissingPart,
    /// A wire ending on an overcurrent protective device (fuse, breaker,
    /// overload relay) whose cross-section carries less current than the
    /// device is rated for.
    UndersizedWire,
    /// A terminal whose article does not suit its kind: a PE terminal built
    /// with an ordinary (grey) block, which leaves the protective conductor
    /// unbonded, or another terminal built with a PE block, which bonds a
    /// live conductor to the rail.
    WrongTerminalBlock,
    /// A cable core used against its marking or the cable's limits
    /// (novo-cables): a live conductor on the green-yellow core (error), PE
    /// on another core (error), a spare core in use, more wires than free
    /// cores, more cores than the cable's part, a floating shield, or a
    /// core thinner than its wire.
    CableCore,
    /// A terminal used against its kind (novo-terminals): a name that
    /// defines another kind (`PE` on a feed-through terminal), a wire whose
    /// potential does not fit the kind (error), a fuse or disconnect
    /// terminal wired on one side (error), or, under the refuse rule, a
    /// terminal drawn twice (error).
    TerminalUse,
    /// A PLC card or channel that cannot be addressed or wired as drawn
    /// (novo-plc, plc.md §7): overlapping addresses, a channel drawn twice
    /// or that the card does not have, a channel without a number or of
    /// the wrong type, a card without a layout or start address, or two
    /// cards in one slot. Errors where the I/O list would be wrong.
    Plc,
    /// Two wires on a page sharing a stretch of line (DESIGN.md §4: wires
    /// never overlap). The editor cannot create one; this finds those that
    /// come from imports, scripts and undo or redo. Wires leaving one pin
    /// together may share the stretch from that pin.
    WireOverlap,
    /// A wire with an open end (`wire.open`, DESIGN.md §4): an end connected
    /// to nothing. Open ends are allowed while sketching; they join no net.
    OpenWire,
    /// A wire that drops too much of the supply voltage. Filled by the
    /// electrical calculations (novo-checks::calc), which read the wire
    /// property `length_m` (or a cable's length), `load_current_a` and the
    /// project property `check.voltage_drop_pct`.
    VoltageDrop,
    /// A protective device that cannot break the prospective short-circuit
    /// current. Filled by the electrical calculations, which read the device
    /// or part property `breaking_capacity_ka` and the project property
    /// `supply.short_circuit_ka`.
    ShortCircuit,
    /// A wire whose smallest earth-fault current is too low for its
    /// protective device to trip at once (`calc::short_circuit`).
    DisconnectionTime,
    /// A wire thinner than its protective device's let-through energy
    /// allows (`k²S²`).
    ThermalWithstand,
    /// A motor starter whose overload relay, contactor or short-circuit
    /// device does not suit the motor or each other (`calc::motor`).
    MotorStarter,
    /// A protective conductor thinner than the rule for its phase
    /// conductors (`calc::pe`).
    ProtectiveConductor,
    /// A name used by more than two interruption points. EPLAN pairs them
    /// two by two in page order, KoperCAD joins all of one name, so an
    /// imported project and this one read it differently.
    AmbiguousInterruption,
    /// A wire number used on two nets.
    WireNumber,
    /// A device without a product level in its tag.
    UntaggedDevice,
    /// A terminal without a number or name.
    UnnamedTerminal,
    /// One short tag under two function or location levels.
    SameShortTag,
    /// Two pages with one name in the same structure.
    DuplicatePage,
    /// A junction no wire ends on.
    DanglingJunction,
    /// A terminal or plug pin no wire reaches (findings C02).
    UnwiredTerminal,
    /// An open wire end lying on a pin, junction or wire without being connected (C03).
    StrayWireEnd,
    /// A contact whose pin numbers do not suit its type: NO ends in 3, 4, 7 or 8, NC in 1, 2, 5 or 6 (C05).
    ContactType,
    /// A contact position beyond the auxiliary contacts the device declares (C06).
    ContactPosition,
    /// One physical contact of a device drawn more than once (C07).
    ContactDrawnTwice,
    /// Extra-low and low voltage under one function code (C10).
    VoltageClassMix,
    /// A PE or N terminal not connected to its conductor (C11).
    ConductorContinuity,
    /// A motor whose windings cannot receive three different phases (C14).
    MotorPhases,
    /// A coil without control voltage on one side and its return on the other (C15).
    CoilSupply,
    /// A plug pin wired on one half only (C17).
    PlugPinOneSided,
    /// A relay larger than the auxiliary contacts drawn need (C08).
    RelaySize,
    /// A cable whose cores disagree with its text such as `4G1.5` (C12).
    CableText,
    /// A device on a neutral net that is neither supply nor external, when selected (C16).
    NeutralUse,
    /// An overview of the pins of a plug (C18).
    PlugPins,
    /// A device name that breaks the project's page-and-column rule, when selected (F01).
    TagRule,
    /// A tag text on a symbol that differs from the device's tag (F04).
    DisplayedTag,
    /// A function or location code the structure list does not know (H01).
    StructureCode,
    /// A device drawn in the schematic but not placed on any mounting plate,
    /// in a project where the cabinet layout has been started.
    UnplacedDevice,
    /// A wire between two placed devices that no duct path serves (novo-routing).
    WireWithoutDuct,
    /// A duct carrying more wires than its fill limit (novo-routing).
    OverfullDuct,
    /// A duct that wires run through but that has no width and height, so
    /// its fill cannot be checked (novo-routing).
    UnsizedDuct,
    /// Wires routed with a default size because they have no cross-section;
    /// the fill figures are only as good as that guess (novo-routing). One
    /// problem lists all of them.
    GuessedWire,
    /// A rule of a selected standard is not met (IEC 60204-1, NFPA 79,
    /// UL 508A; [`standards`]). Only produced for projects that selected a
    /// standard. Each problem carries its rule id in [`Problem::rule`] and
    /// its own severity, set by the rule.
    Standard,
    /// A wire whose cable cannot carry what its protective device allows
    /// (`Ib ≤ In ≤ Iz`, with the installation method, ambient temperature
    /// and grouping of IEC 60364-5-52), read from the planning tables
    /// unless the user loaded a licensed copy (`calc::tables`).
    CableRating,
    /// Two devices whose footprints overlap on a mounting plate
    /// (novo-layout).
    DeviceCollision,
    /// A device that leaves the usable area of its mounting plate.
    DeviceOutsidePlate,
    /// Two devices closer than the clearance they need.
    DevicesTooClose,
    /// Devices on a DIN rail that need more room than the rail has.
    RailOverflow,
    /// A wiring duct that lies over a DIN rail or a device on the plate.
    DuctOverlap,
    /// A device deeper than the room between the plate and the door
    /// (novo-layout's door report); only for enclosures with an interior.
    DeviceTooDeep,
    /// A device that fits but leaves less than the minimum gap to the door.
    DoorGapTight,
    /// A placed device whose depth is not known (no `depth_mm` on the
    /// device or its parts), so the door could not be checked for it.
    DepthUnknown,
    /// A mounting plate wider or higher than the inside of its enclosure.
    PlateTooBig,
    /// A safety function achieving a lower PL than it requires (SAFE-2).
    SafetyLevelNotMet,
    /// A safety function part bound to a missing device or without reliability data (SAFE-2).
    SafetyBinding,
    /// The open door needs more room in front or beside the hinge side than
    /// the enclosure's stated free room.
    DoorSwing,
}

impl ProblemKind {
    /// Every kind, in the order the table in the crate docs lists them.
    pub const ALL: [ProblemKind; 66] = [
        ProblemKind::DuplicateTag,
        ProblemKind::TooManyContacts,
        ProblemKind::PotentialConflict,
        ProblemKind::UnconnectedPin,
        ProblemKind::LoneInterruption,
        ProblemKind::NoMainFunction,
        ProblemKind::SeveralMainFunctions,
        ProblemKind::DuplicateTerminal,
        ProblemKind::ZeroLengthWire,
        ProblemKind::CableCore,
        ProblemKind::TerminalUse,
        ProblemKind::Plc,
        ProblemKind::WrongTerminalBlock,
        ProblemKind::UndersizedWire,
        ProblemKind::MissingPart,
        ProblemKind::WireOverlap,
        ProblemKind::OpenWire,
        ProblemKind::VoltageDrop,
        ProblemKind::ShortCircuit,
        ProblemKind::DisconnectionTime,
        ProblemKind::ThermalWithstand,
        ProblemKind::MotorStarter,
        ProblemKind::ProtectiveConductor,
        ProblemKind::UnplacedDevice,
        ProblemKind::WireWithoutDuct,
        ProblemKind::OverfullDuct,
        ProblemKind::UnsizedDuct,
        ProblemKind::GuessedWire,
        ProblemKind::Standard,
        ProblemKind::CableRating,
        ProblemKind::DeviceCollision,
        ProblemKind::DeviceOutsidePlate,
        ProblemKind::DevicesTooClose,
        ProblemKind::RailOverflow,
        ProblemKind::DuctOverlap,
        ProblemKind::DeviceTooDeep,
        ProblemKind::DoorGapTight,
        ProblemKind::DepthUnknown,
        ProblemKind::PlateTooBig,
        ProblemKind::SafetyLevelNotMet,
        ProblemKind::SafetyBinding,
        ProblemKind::DoorSwing,
        ProblemKind::AmbiguousInterruption,
        ProblemKind::WireNumber,
        ProblemKind::UntaggedDevice,
        ProblemKind::UnnamedTerminal,
        ProblemKind::SameShortTag,
        ProblemKind::DuplicatePage,
        ProblemKind::DanglingJunction,
        ProblemKind::UnwiredTerminal,
        ProblemKind::StrayWireEnd,
        ProblemKind::ContactType,
        ProblemKind::ContactPosition,
        ProblemKind::ContactDrawnTwice,
        ProblemKind::VoltageClassMix,
        ProblemKind::ConductorContinuity,
        ProblemKind::MotorPhases,
        ProblemKind::CoilSupply,
        ProblemKind::PlugPinOneSided,
        ProblemKind::RelaySize,
        ProblemKind::CableText,
        ProblemKind::NeutralUse,
        ProblemKind::PlugPins,
        ProblemKind::TagRule,
        ProblemKind::DisplayedTag,
        ProblemKind::StructureCode,
    ];

    pub fn severity(self) -> Severity {
        match self {
            ProblemKind::DuplicateTag
            | ProblemKind::TooManyContacts
            | ProblemKind::PotentialConflict
            | ProblemKind::ShortCircuit
            | ProblemKind::DisconnectionTime
            | ProblemKind::ThermalWithstand
            | ProblemKind::CableRating
            | ProblemKind::DeviceCollision
            | ProblemKind::DeviceOutsidePlate
            | ProblemKind::DuctOverlap
            | ProblemKind::DeviceTooDeep
            | ProblemKind::SafetyLevelNotMet
            | ProblemKind::PlateTooBig => Severity::Error,
            ProblemKind::UnconnectedPin
            | ProblemKind::LoneInterruption
            | ProblemKind::NoMainFunction
            | ProblemKind::SeveralMainFunctions
            | ProblemKind::DuplicateTerminal
            | ProblemKind::ZeroLengthWire
            | ProblemKind::UndersizedWire
            | ProblemKind::WrongTerminalBlock
            | ProblemKind::CableCore
            | ProblemKind::TerminalUse
            | ProblemKind::WireOverlap
            | ProblemKind::OpenWire
            | ProblemKind::VoltageDrop
            | ProblemKind::MotorStarter
            | ProblemKind::ProtectiveConductor
            | ProblemKind::WireWithoutDuct
            | ProblemKind::OverfullDuct
            | ProblemKind::Standard
            | ProblemKind::DevicesTooClose
            | ProblemKind::RailOverflow
            | ProblemKind::DoorGapTight
            | ProblemKind::DoorSwing
            | ProblemKind::AmbiguousInterruption
            | ProblemKind::WireNumber
            | ProblemKind::UntaggedDevice
            | ProblemKind::UnnamedTerminal
            | ProblemKind::DuplicatePage
            | ProblemKind::DanglingJunction
            | ProblemKind::UnwiredTerminal
            | ProblemKind::StrayWireEnd
            | ProblemKind::ContactType
            | ProblemKind::ContactPosition
            | ProblemKind::ContactDrawnTwice
            | ProblemKind::VoltageClassMix
            | ProblemKind::ConductorContinuity
            | ProblemKind::MotorPhases
            | ProblemKind::CoilSupply
            | ProblemKind::PlugPinOneSided
            | ProblemKind::RelaySize
            | ProblemKind::CableText
            | ProblemKind::NeutralUse
            | ProblemKind::TagRule
            | ProblemKind::DisplayedTag
            | ProblemKind::StructureCode
            | ProblemKind::SafetyBinding
            | ProblemKind::Plc => Severity::Warning,
            ProblemKind::MissingPart
            | ProblemKind::UnplacedDevice
            | ProblemKind::UnsizedDuct
            | ProblemKind::GuessedWire
            | ProblemKind::DepthUnknown
            | ProblemKind::PlugPins
            | ProblemKind::SameShortTag => Severity::Info,
        }
    }

    /// A short plain name, for filter chips in the Issues list.
    pub fn label(self) -> &'static str {
        match self {
            ProblemKind::UnconnectedPin => "Unconnected point",
            ProblemKind::DuplicateTag => "Duplicate tag",
            ProblemKind::DuplicateTerminal => "Duplicate terminal",
            ProblemKind::NoMainFunction => "No main function",
            ProblemKind::SeveralMainFunctions => "Several main functions",
            ProblemKind::TooManyContacts => "Too many contacts",
            ProblemKind::LoneInterruption => "Interruption point without partner",
            ProblemKind::PotentialConflict => "Potential conflict",
            ProblemKind::ZeroLengthWire => "Zero-length wire",
            ProblemKind::MissingPart => "No part",
            ProblemKind::UndersizedWire => "Undersized wire",
            ProblemKind::WrongTerminalBlock => "Wrong terminal block",
            ProblemKind::CableCore => "Cable core",
            ProblemKind::TerminalUse => "Terminal use",
            ProblemKind::Plc => "PLC",
            ProblemKind::WireOverlap => "Wires overlap",
            ProblemKind::OpenWire => "Open wire end",
            ProblemKind::VoltageDrop => "Voltage drop",
            ProblemKind::ShortCircuit => "Short-circuit rating",
            ProblemKind::DisconnectionTime => "Disconnection time",
            ProblemKind::ThermalWithstand => "Thermal withstand",
            ProblemKind::MotorStarter => "Motor starter",
            ProblemKind::ProtectiveConductor => "Protective conductor",
            ProblemKind::UnplacedDevice => "Unplaced device",
            ProblemKind::WireWithoutDuct => "Wire without duct",
            ProblemKind::OverfullDuct => "Overfull duct",
            ProblemKind::UnsizedDuct => "Duct without size",
            ProblemKind::GuessedWire => "Wire size guessed",
            ProblemKind::Standard => "Standards",
            ProblemKind::CableRating => "Cable rating",
            ProblemKind::DeviceCollision => "Devices overlap",
            ProblemKind::DeviceOutsidePlate => "Device off the plate",
            ProblemKind::DevicesTooClose => "Devices too close",
            ProblemKind::RailOverflow => "Rail overfull",
            ProblemKind::DuctOverlap => "Duct over rail or device",
            ProblemKind::DeviceTooDeep => "Device too deep",
            ProblemKind::DoorGapTight => "Door gap tight",
            ProblemKind::DepthUnknown => "Depth unknown",
            ProblemKind::PlateTooBig => "Plate too big",
            ProblemKind::SafetyLevelNotMet => "Safety level not met",
            ProblemKind::SafetyBinding => "Safety function binding",
            ProblemKind::DoorSwing => "Door swing",
            ProblemKind::AmbiguousInterruption => "Interruption name used often",
            ProblemKind::WireNumber => "Wire number",
            ProblemKind::UntaggedDevice => "Device without tag",
            ProblemKind::UnnamedTerminal => "Unnamed terminal",
            ProblemKind::SameShortTag => "Same short tag",
            ProblemKind::DuplicatePage => "Duplicate page",
            ProblemKind::DanglingJunction => "Dangling junction",
            ProblemKind::UnwiredTerminal => "Unwired terminal",
            ProblemKind::StrayWireEnd => "Wire end on something",
            ProblemKind::ContactType => "Contact numbering",
            ProblemKind::ContactPosition => "Contact beyond relay",
            ProblemKind::ContactDrawnTwice => "Contact drawn twice",
            ProblemKind::VoltageClassMix => "Voltage classes mixed",
            ProblemKind::ConductorContinuity => "Earth or neutral broken",
            ProblemKind::MotorPhases => "Motor phases",
            ProblemKind::CoilSupply => "Coil supply",
            ProblemKind::PlugPinOneSided => "Plug pin on one half",
            ProblemKind::RelaySize => "Relay too large",
            ProblemKind::CableText => "Cable text",
            ProblemKind::NeutralUse => "Neutral use",
            ProblemKind::PlugPins => "Plug pins",
            ProblemKind::TagRule => "Tag rule",
            ProblemKind::DisplayedTag => "Displayed tag",
            ProblemKind::StructureCode => "Unknown structure code",
        }
    }

    /// A stable name for storing a waiver (`check.voltage_drop`).
    pub fn key(self) -> &'static str {
        match self {
            ProblemKind::UnconnectedPin => "check.unconnected_pin",
            ProblemKind::DuplicateTag => "check.duplicate_tag",
            ProblemKind::DuplicateTerminal => "check.duplicate_terminal",
            ProblemKind::NoMainFunction => "check.no_main_function",
            ProblemKind::SeveralMainFunctions => "check.several_main_functions",
            ProblemKind::TooManyContacts => "check.too_many_contacts",
            ProblemKind::LoneInterruption => "check.lone_interruption",
            ProblemKind::PotentialConflict => "check.potential_conflict",
            ProblemKind::ZeroLengthWire => "check.zero_length_wire",
            ProblemKind::MissingPart => "check.missing_part",
            ProblemKind::UndersizedWire => "check.undersized_wire",
            ProblemKind::WrongTerminalBlock => "check.wrong_terminal_block",
            ProblemKind::CableCore => "check.cable_core",
            ProblemKind::TerminalUse => "check.terminal_use",
            ProblemKind::Plc => "check.plc",
            ProblemKind::WireOverlap => "check.wire_overlap",
            ProblemKind::OpenWire => "check.open_wire",
            ProblemKind::VoltageDrop => "check.voltage_drop",
            ProblemKind::ShortCircuit => "check.short_circuit",
            ProblemKind::DisconnectionTime => "check.disconnection_time",
            ProblemKind::ThermalWithstand => "check.thermal_withstand",
            ProblemKind::MotorStarter => "check.motor_starter",
            ProblemKind::ProtectiveConductor => "check.protective_conductor",
            ProblemKind::CableRating => "check.cable_rating",
            ProblemKind::UnplacedDevice => "check.unplaced_device",
            ProblemKind::WireWithoutDuct => "check.wire_without_duct",
            ProblemKind::OverfullDuct => "check.overfull_duct",
            ProblemKind::UnsizedDuct => "check.unsized_duct",
            ProblemKind::GuessedWire => "check.guessed_wire",
            ProblemKind::Standard => "check.standard",
            ProblemKind::DeviceCollision => "check.device_collision",
            ProblemKind::DeviceOutsidePlate => "check.device_outside_plate",
            ProblemKind::DevicesTooClose => "check.devices_too_close",
            ProblemKind::RailOverflow => "check.rail_overflow",
            ProblemKind::DuctOverlap => "check.duct_overlap",
            ProblemKind::DeviceTooDeep => "check.device_too_deep",
            ProblemKind::DoorGapTight => "check.door_gap_tight",
            ProblemKind::DepthUnknown => "check.depth_unknown",
            ProblemKind::PlateTooBig => "check.plate_too_big",
            ProblemKind::SafetyLevelNotMet => "check.safety_level",
            ProblemKind::SafetyBinding => "check.safety_binding",
            ProblemKind::DoorSwing => "check.door_swing",
            ProblemKind::AmbiguousInterruption => "check.ambiguous_interruption",
            ProblemKind::WireNumber => "check.wire_number",
            ProblemKind::UntaggedDevice => "check.untagged_device",
            ProblemKind::UnnamedTerminal => "check.unnamed_terminal",
            ProblemKind::SameShortTag => "check.same_short_tag",
            ProblemKind::DuplicatePage => "check.duplicate_page",
            ProblemKind::DanglingJunction => "check.dangling_junction",
            ProblemKind::UnwiredTerminal => "check.unwired_terminal",
            ProblemKind::StrayWireEnd => "check.stray_wire_end",
            ProblemKind::ContactType => "check.contact_type",
            ProblemKind::ContactPosition => "check.contact_position",
            ProblemKind::ContactDrawnTwice => "check.contact_drawn_twice",
            ProblemKind::VoltageClassMix => "check.voltage_class_mix",
            ProblemKind::ConductorContinuity => "check.conductor_continuity",
            ProblemKind::MotorPhases => "check.motor_phases",
            ProblemKind::CoilSupply => "check.coil_supply",
            ProblemKind::PlugPinOneSided => "check.plug_pin_one_sided",
            ProblemKind::RelaySize => "check.relay_size",
            ProblemKind::CableText => "check.cable_text",
            ProblemKind::NeutralUse => "check.neutral_use",
            ProblemKind::PlugPins => "check.plug_pins",
            ProblemKind::TagRule => "check.tag_rule",
            ProblemKind::DisplayedTag => "check.displayed_tag",
            ProblemKind::StructureCode => "check.structure_code",
        }
    }
}

/// One finding of the design checks.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub kind: ProblemKind,
    pub severity: Severity,
    /// The objects to mark, drawn objects (symbols, wires) first where the
    /// problem has any. A device, terminal or cable that is not drawn
    /// anywhere is named itself, so the Issues list can still select it.
    pub at: Vec<AnyId>,
    /// The page of the first marked object in reading order, where F8 goes.
    /// `None` when nothing of the problem is drawn.
    pub page: Option<PageId>,
    /// Where on [`page`](Problem::page) that object sits, in mm.
    pub position: Option<Point>,
    /// One plain sentence, shown on hover and in the Issues list.
    pub message: String,
    /// The id of the standards rule this problem comes from
    /// (`iec60204.pe-colour`), for [`ProblemKind::Standard`]; `None` for
    /// every other kind.
    pub rule: Option<&'static str>,
}

impl Problem {
    pub(crate) fn new(kind: ProblemKind, at: Vec<AnyId>, message: String) -> Problem {
        Problem {
            kind,
            severity: kind.severity(),
            at,
            page: None,
            position: None,
            message,
            rule: None,
        }
    }

    /// The name a waiver of this problem is stored under: the kind's key, or
    /// for a standards problem the key of its rule (`check.std.<rule id>`),
    /// so a waiver covers one rule and not every standards problem.
    pub fn waiver_key(&self) -> String {
        match self.rule {
            Some(rule) => standards::waiver_key(rule),
            None => self.kind.key().to_owned(),
        }
    }
}

/// Problem counts by severity, for the issue pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Summary {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}

impl Summary {
    pub fn of(problems: &[Problem]) -> Summary {
        let mut summary = Summary::default();
        for problem in problems {
            match problem.severity {
                Severity::Error => summary.errors += 1,
                Severity::Warning => summary.warnings += 1,
                Severity::Info => summary.infos += 1,
            }
        }
        summary
    }

    /// No errors and no warnings. Infos do not count as issues.
    pub fn is_clean(&self) -> bool {
        self.errors == 0 && self.warnings == 0
    }

    /// The issue pill's text: `No issues`, `1 error`, `5 warnings` or
    /// `2 errors · 5 warnings`. Infos are listed in Issues but not counted
    /// on the pill, which counts things to fix (DESIGN.md §3).
    pub fn pill_text(&self) -> String {
        let count =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        match (self.errors, self.warnings) {
            (0, 0) => "No issues".to_owned(),
            (e, 0) => count(e, "error", "errors"),
            (0, w) => count(w, "warning", "warnings"),
            (e, w) => format!(
                "{} · {}",
                count(e, "error", "errors"),
                count(w, "warning", "warnings")
            ),
        }
    }
}

/// Runs every check on the project. Builds a netlist for the potential
/// check; an app that already keeps a current netlist calls
/// [`check_with`] instead.
pub fn check(project: &Project) -> Vec<Problem> {
    check_with(project, &project.netlist())
}

/// Runs every check, reading connectivity from `netlist`, which must be
/// current for `project` ([`Netlist::is_current`]).
pub fn check_with(project: &Project, netlist: &Netlist) -> Vec<Problem> {
    debug_assert!(netlist.is_current(project), "netlist is stale");
    let cx = Context::new(project);
    let mut problems = Vec::new();
    checks::unconnected_pins(&cx, &mut problems);
    checks::duplicate_tags(&cx, &mut problems);
    checks::duplicate_terminals(&cx, &mut problems);
    checks::device_functions(&cx, &mut problems);
    checks::lone_interruptions(&cx, &mut problems);
    checks::potential_conflicts(&cx, netlist, &mut problems);
    checks::zero_length_wires(&cx, &mut problems);
    checks::missing_parts(&cx, &mut problems);
    checks::undersized_wires(&cx, netlist, &mut problems);
    checks::wrong_terminal_blocks(&cx, &mut problems);
    checks::cable_cores(&cx, netlist, &mut problems);
    checks::terminal_use(&cx, netlist, &mut problems);
    checks::plc(&cx, &mut problems);
    checks::wire_overlaps(&cx, &mut problems);
    checks::open_wires(&cx, &mut problems);
    connection_rules::ambiguous_interruptions(&cx, &mut problems);
    connection_rules::wire_numbers(&cx, netlist, &mut problems);
    connection_rules::untagged_devices(&cx, &mut problems);
    connection_rules::unnamed_terminals(&cx, &mut problems);
    connection_rules::same_short_tags(&cx, &mut problems);
    connection_rules::duplicate_pages(&cx, &mut problems);
    connection_rules::dangling_junctions(&cx, &mut problems);
    house_rules::terminals_and_plugs(&cx, &mut problems);
    house_rules::stray_wire_ends(&cx, &mut problems);
    house_rules::contact_types(&cx, &mut problems);
    house_rules::contact_positions(&cx, &mut problems);
    house_rules::contacts_drawn_twice(&cx, &mut problems);
    house_rules::voltage_classes(&cx, netlist, &mut problems);
    house_rules::conductor_continuity(&cx, netlist, &mut problems);
    house_rules::motor_phases(&cx, netlist, &mut problems);
    house_rules::coil_supply(&cx, netlist, &mut problems);
    more_rules::relay_size(&cx, &mut problems);
    more_rules::cable_text(&cx, &mut problems);
    more_rules::neutral_use(&cx, netlist, &mut problems);
    more_rules::plug_overview(&cx, &mut problems);
    house_rules::tag_rule(&cx, &mut problems);
    house_rules::displayed_tags(&cx, &mut problems);
    house_rules::structure_codes(&cx, &mut problems);
    calc_checks::run(&cx, netlist, &mut problems);
    physical::unplaced_devices(&cx, &mut problems);
    physical::routing(&cx, &mut problems);
    standards::problems(&cx, netlist, &mut problems);
    physical::layout(&cx, &mut problems);
    door::door(&cx, &mut problems);
    safety::run(&cx, &mut problems);
    cx.sort(&mut problems);
    problems
}

/// Where something sits in reading order: page rank, then x, then y, as
/// `Project::cross_references` orders symbols.
#[derive(Debug, Clone, Copy)]
struct Spot {
    rank: usize,
    page: PageId,
    at: Point,
}

impl Spot {
    fn cmp(&self, other: &Spot) -> Ordering {
        self.rank
            .cmp(&other.rank)
            .then(self.page.cmp(&other.page))
            .then(self.at.x.total_cmp(&other.at.x))
            .then(self.at.y.total_cmp(&other.at.y))
    }
}

fn symbol_spot(page_rank: &HashMap<PageId, usize>, symbol: &SymbolInstance) -> Spot {
    Spot {
        rank: page_rank.get(&symbol.page).copied().unwrap_or(usize::MAX),
        page: symbol.page,
        at: symbol.placement.origin,
    }
}

/// Lookups shared by the checks, built in one pass.
pub(crate) struct Context<'a> {
    pub(crate) project: &'a Project,
    page_rank: HashMap<PageId, usize>,
    /// The symbols of each device: main functions first, then reading order.
    pub(crate) by_device: HashMap<DeviceId, Vec<&'a SymbolInstance>>,
    /// The symbols drawing each terminal, in reading order.
    pub(crate) by_terminal: HashMap<TerminalId, Vec<&'a SymbolInstance>>,
}

impl<'a> Context<'a> {
    fn new(project: &'a Project) -> Context<'a> {
        let page_rank: HashMap<PageId, usize> = project
            .pages_in_order()
            .iter()
            .enumerate()
            .map(|(rank, page)| (page.id, rank))
            .collect();
        let mut by_device: HashMap<DeviceId, Vec<&SymbolInstance>> = HashMap::new();
        let mut by_terminal: HashMap<TerminalId, Vec<&SymbolInstance>> = HashMap::new();
        for symbol in project.symbols() {
            match &symbol.role {
                SymbolRole::Device { device, .. } => {
                    by_device.entry(*device).or_default().push(symbol)
                }
                SymbolRole::Terminal { terminals } => {
                    for terminal in terminals {
                        by_terminal.entry(*terminal).or_default().push(symbol);
                    }
                }
                _ => {}
            }
        }
        let order = |a: &&SymbolInstance, b: &&SymbolInstance| {
            symbol_spot(&page_rank, a)
                .cmp(&symbol_spot(&page_rank, b))
                .then(a.id.cmp(&b.id))
        };
        let is_aux = |s: &SymbolInstance| !matches!(s.role, SymbolRole::Device { main: true, .. });
        for symbols in by_device.values_mut() {
            symbols.sort_by(|a, b| is_aux(a).cmp(&is_aux(b)).then_with(|| order(a, b)));
        }
        for symbols in by_terminal.values_mut() {
            symbols.sort_by(order);
        }
        Context {
            project,
            page_rank,
            by_device,
            by_terminal,
        }
    }

    fn symbol_spot(&self, symbol: &SymbolInstance) -> Spot {
        symbol_spot(&self.page_rank, symbol)
    }

    /// Where an object is drawn first, in reading order.
    fn spot(&self, id: AnyId) -> Option<Spot> {
        let project = self.project;
        let on = |page: PageId, at: Point| Spot {
            rank: self.page_rank.get(&page).copied().unwrap_or(usize::MAX),
            page,
            at,
        };
        match id {
            AnyId::Symbol(s) => project.symbol(s).map(|s| self.symbol_spot(s)),
            AnyId::Wire(w) => {
                let wire = project.wire(w)?;
                Some(on(wire.page, project.endpoint_position(wire.from)?))
            }
            AnyId::Junction(j) => project.junction(j).map(|j| on(j.page, j.position)),
            AnyId::Device(d) => self.by_device.get(&d)?.first().map(|s| self.symbol_spot(s)),
            AnyId::Terminal(t) => self
                .by_terminal
                .get(&t)?
                .first()
                .map(|s| self.symbol_spot(s)),
            AnyId::Cable(c) => project
                .cable(c)?
                .cores
                .iter()
                .filter_map(|core| self.spot(AnyId::Wire(core.wire?)))
                .min_by(Spot::cmp),
            _ => None,
        }
    }

    /// Fills in a problem's page and position from the first of its objects
    /// in reading order and returns that spot.
    fn locate(&self, problem: &mut Problem) -> Option<Spot> {
        let spot = problem
            .at
            .iter()
            .filter_map(|&id| self.spot(id))
            .min_by(Spot::cmp);
        if let Some(spot) = spot {
            problem.page = Some(spot.page);
            problem.position = Some(spot.at);
        }
        spot
    }

    /// Fills in each problem's page and position and sorts them in reading
    /// order. Problems with nothing drawn come last; ties go by kind, then
    /// message, then the marked ids, so the order is the same every run.
    fn sort(&self, problems: &mut Vec<Problem>) {
        let mut keyed: Vec<(Option<Spot>, Problem)> = problems
            .drain(..)
            .map(|mut problem| (self.locate(&mut problem), problem))
            .collect();
        keyed.sort_unstable_by(|(a, p), (b, q)| {
            let place = match (a, b) {
                (Some(a), Some(b)) => a.cmp(b),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };
            place
                .then(p.kind.cmp(&q.kind))
                .then_with(|| p.message.cmp(&q.message))
                .then_with(|| p.at.cmp(&q.at))
        });
        problems.extend(keyed.into_iter().map(|(_, p)| p));
    }

    /// How a symbol is named in messages: its device tag, terminal
    /// designation, interruption or potential name, or its library key.
    pub(crate) fn symbol_label(&self, symbol: &SymbolInstance) -> String {
        let project = self.project;
        match &symbol.role {
            SymbolRole::Device { device, .. } => project
                .device(*device)
                .map_or_else(|| symbol.symbol.0.clone(), |d| d.tag.to_string()),
            SymbolRole::Terminal { terminals } => terminals
                .first()
                .and_then(|&t| project.terminal_designation(t))
                .unwrap_or_else(|| symbol.symbol.0.clone()),
            SymbolRole::Interruption { name } => format!("Interruption point {name}"),
            SymbolRole::PotentialFeed { potential } => project.potential(*potential).map_or_else(
                || "Potential feed".to_owned(),
                |p| format!("Feed {}", p.name),
            ),
            SymbolRole::Unassigned => symbol.symbol.0.clone(),
        }
    }

    /// How an endpoint is named in messages: `-K1:13` or `a junction`.
    pub(crate) fn endpoint_label(&self, end: Endpoint) -> String {
        match end {
            Endpoint::Pin { symbol, pin } => {
                let Some(s) = self.project.symbol(symbol) else {
                    return "a missing pin".to_owned();
                };
                let name = s
                    .pins
                    .get(usize::from(pin))
                    .map_or("?", |p| p.name.as_str());
                format!("{}:{name}", self.symbol_label(s))
            }
            Endpoint::Junction { .. } => "a junction".to_owned(),
            Endpoint::Free { .. } => "an open end".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pill_text_follows_counts() {
        let s = |errors, warnings, infos| Summary {
            errors,
            warnings,
            infos,
        };
        assert_eq!(s(0, 0, 3).pill_text(), "No issues");
        assert_eq!(s(1, 0, 0).pill_text(), "1 error");
        assert_eq!(s(0, 1, 0).pill_text(), "1 warning");
        assert_eq!(s(0, 5, 0).pill_text(), "5 warnings");
        assert_eq!(s(2, 5, 1).pill_text(), "2 errors · 5 warnings");
        assert_eq!(s(1, 1, 0).pill_text(), "1 error · 1 warning");
        assert!(s(0, 0, 4).is_clean());
        assert!(!s(0, 1, 0).is_clean());
    }

    #[test]
    fn every_kind_is_listed_once() {
        let mut kinds = ProblemKind::ALL.to_vec();
        kinds.sort();
        kinds.dedup();
        assert_eq!(kinds.len(), ProblemKind::ALL.len());
        assert!(ProblemKind::ALL.iter().all(|k| !k.label().is_empty()));
    }
}
