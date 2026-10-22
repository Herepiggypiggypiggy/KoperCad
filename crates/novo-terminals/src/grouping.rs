//! Which strip a proposed terminal goes on.
//!
//! A machine's boundary terminals are not one strip: power, control voltage,
//! signals and protective earth are wired, labelled and often delivered on
//! separate strips (IEC 60204-1 keeps protective conductors apart from the
//! rest). The grouping is a saved project setting
//! ([`GROUPING_KEY`] in the project info properties, one undo step to
//! change), so it is the user's to choose and travels with the file.
//!
//! - [`Grouping::ByClass`] (the default): one strip per [`StripClass`] in
//!   each location, named `-X1` power (and wires with no potential yet), `-X2` control, `-X3` signals, `-X4`
//!   protective earth. Only classes that occur get a strip.
//! - [`Grouping::Single`]: everything on one strip per location, the way
//!   earlier versions did.

use novo_core::{Command, Netlist, PotentialKind, Project, Result, Tx, Value, Wire};

/// The project info property that holds the grouping (`class` or `single`).
pub const GROUPING_KEY: &str = "terminals.strip_grouping";

/// Voltage above which a conductor counts as power rather than control
/// (the 50 V AC / 120 V DC extra-low-voltage limit is the usual boundary;
/// 50 V is the conservative one for anything not known to be DC).
const CONTROL_LIMIT_VOLTS: f64 = 50.0;

/// What a terminal carries, for choosing its strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StripClass {
    /// Line and neutral conductors, and anything above extra-low voltage.
    Power,
    /// DC rails and extra-low-voltage control supply.
    Control,
    /// Signals: a signal potential without a voltage.
    Signal,
    /// Wires whose net has no potential yet. They stay on the first strip,
    /// as before, until potentials are assigned and split them.
    Unassigned,
    /// Protective earth and PEN.
    Earth,
}

impl StripClass {
    pub const ALL: [StripClass; 5] = [
        StripClass::Unassigned,
        StripClass::Power,
        StripClass::Control,
        StripClass::Signal,
        StripClass::Earth,
    ];

    /// The strip name this class gets under [`Grouping::ByClass`].
    pub fn strip_name(self) -> &'static str {
        match self {
            StripClass::Power | StripClass::Unassigned => "X1",
            StripClass::Control => "X2",
            StripClass::Signal => "X3",
            StripClass::Earth => "X4",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StripClass::Power => "power",
            StripClass::Control => "control",
            StripClass::Signal => "signals",
            StripClass::Unassigned => "no potential yet",
            StripClass::Earth => "protective earth",
        }
    }
}

/// How proposed terminals are spread over strips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Grouping {
    /// One strip per signal class in each location.
    #[default]
    ByClass,
    /// One strip per location.
    Single,
}

impl Grouping {
    pub fn key(self) -> &'static str {
        match self {
            Grouping::ByClass => "class",
            Grouping::Single => "single",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Grouping::ByClass => "By kind (power, control, signals, PE)",
            Grouping::Single => "One strip per location",
        }
    }
}

/// The grouping saved with the project; [`Grouping::ByClass`] when none is.
pub fn grouping(project: &Project) -> Grouping {
    match project.info().properties.get(GROUPING_KEY) {
        Some(Value::Text(t)) if t.trim().eq_ignore_ascii_case("single") => Grouping::Single,
        _ => Grouping::ByClass,
    }
}

/// Saves the grouping with the project (one undo step).
pub fn set_grouping(tx: &mut Tx<'_>, grouping: Grouping) -> Result<()> {
    if crate::grouping(tx.project()) == grouping {
        return Ok(());
    }
    let mut info = tx.project().info().clone();
    info.properties
        .insert(GROUPING_KEY.into(), Value::Text(grouping.key().into()));
    tx.execute(Command::SetProjectInfo { info }).map(|_| ())
}

/// The class a wire belongs to, from the potential its net carries.
pub(crate) fn class_of_wire(project: &Project, netlist: &Netlist, wire: &Wire) -> StripClass {
    let potential = netlist
        .net_of_wire(wire)
        .and_then(|n| netlist.net(n).potential())
        .and_then(|p| project.potential(p));
    let Some(p) = potential else {
        return StripClass::Unassigned;
    };
    match p.kind {
        PotentialKind::ProtectiveEarth | PotentialKind::Pen => StripClass::Earth,
        PotentialKind::Line | PotentialKind::Neutral => StripClass::Power,
        PotentialKind::DcPositive | PotentialKind::DcNegative => StripClass::Control,
        PotentialKind::Signal => match p.voltage {
            Some(v) if v > CONTROL_LIMIT_VOLTS => StripClass::Power,
            Some(_) => StripClass::Control,
            None => StripClass::Signal,
        },
    }
}
