//! The controller profile: plain data, read from `*.controller.toml`.
//!
//! ```toml
//! [controller]
//! id = "generic-radio-3x2"
//! name = "Radio, 3 motions, 2-step"
//! kind = "radio"
//! supply = { kind = "ac", volts = 48 }
//! common = "COM"
//! gate = ["main=on"]                 # every signal needs this unless it says ungated
//!
//! [[control]]
//! id = "hoist"
//! kind = "rocker2"                   # positions default per kind; rockers name them
//! positions = ["off", "up1", "up2", "down1", "down2"]
//!
//! [[signal]]
//! name = "H_UP_SLOW"
//! control = "hoist"
//! group = "up"                       # a rocker group and the contact it closes:
//! step = 1                           # step 1 = slow, step 2 = fast (and slow, see below)
//! ```

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::validate::{Issue, Severity, validate};

/// Kind of operator control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlKind {
    /// Momentary push button: `off`, `on`.
    Button,
    /// Latching switch or receiver-side latch (main/start): `press` toggles.
    Latch,
    /// Two-step rocker, spring return: positions `<group>1`, `<group>2` around the rest.
    Rocker2,
    /// Three-step rocker, spring return.
    Rocker3,
    /// Joystick axis with detents, spring return to the rest detent by default.
    Axis,
    /// Selector switch, stays where it is.
    Selector,
    /// Latching mushroom: `released`, `pressed`; leaving `pressed` needs a separate reset (twist).
    EStop,
    /// Dead-man bar or enabling switch: `released`, `held`.
    Deadman,
    /// Key switch: `off`, `on`, stays where it is.
    Key,
}

impl ControlKind {
    pub const ALL: [ControlKind; 9] = [
        ControlKind::Button,
        ControlKind::Latch,
        ControlKind::Rocker2,
        ControlKind::Rocker3,
        ControlKind::Axis,
        ControlKind::Selector,
        ControlKind::EStop,
        ControlKind::Deadman,
        ControlKind::Key,
    ];

    /// The name used in profile files.
    pub fn as_str(self) -> &'static str {
        match self {
            ControlKind::Button => "button",
            ControlKind::Latch => "latch",
            ControlKind::Rocker2 => "rocker2",
            ControlKind::Rocker3 => "rocker3",
            ControlKind::Axis => "axis",
            ControlKind::Selector => "selector",
            ControlKind::EStop => "estop",
            ControlKind::Deadman => "deadman",
            ControlKind::Key => "key",
        }
    }

    /// Positions used when the profile gives none (the first one is the rest position).
    pub fn default_positions(self) -> Option<&'static [&'static str]> {
        match self {
            ControlKind::Button | ControlKind::Latch | ControlKind::Key => Some(&["off", "on"]),
            ControlKind::EStop => Some(&["released", "pressed"]),
            ControlKind::Deadman => Some(&["released", "held"]),
            ControlKind::Rocker2
            | ControlKind::Rocker3
            | ControlKind::Axis
            | ControlKind::Selector => None,
        }
    }

    /// Whether the control returns to its rest position when released, unless the profile says.
    pub fn spring_by_default(self) -> bool {
        matches!(
            self,
            ControlKind::Button
                | ControlKind::Rocker2
                | ControlKind::Rocker3
                | ControlKind::Axis
                | ControlKind::Deadman
        )
    }

    /// Number of steps per direction of a rocker.
    pub fn rocker_steps(self) -> Option<u8> {
        match self {
            ControlKind::Rocker2 => Some(2),
            ControlKind::Rocker3 => Some(3),
            _ => None,
        }
    }
}

/// What kind of device the profile stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControllerKind {
    Radio,
    Pendant,
    Joystick,
    PlcPanel,
    #[default]
    Custom,
}

impl ControllerKind {
    pub const ALL: [ControllerKind; 5] = [
        ControllerKind::Radio,
        ControllerKind::Pendant,
        ControllerKind::Joystick,
        ControllerKind::PlcPanel,
        ControllerKind::Custom,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SupplyKind {
    Ac,
    Dc,
}

/// What the receiver or panel switches (informational and for binding checks).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Supply {
    pub kind: SupplyKind,
    pub volts: f64,
}

/// Electrical meaning of a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignalKind {
    /// Dry contact closing the common to an output pin.
    #[default]
    Contact,
    /// The pin carries the supply when high.
    Voltage,
    /// A value (0-10 V, 4-20 mA, ...) taken from `levels` per position.
    Analog,
}

impl SignalKind {
    pub const ALL: [SignalKind; 3] = [SignalKind::Contact, SignalKind::Voltage, SignalKind::Analog];
}

/// How several signals bound to one target combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Combine {
    /// Any high signal drives the target (largest magnitude wins). Right for motion commands.
    #[default]
    Or,
    /// The target is only high while every signal bound to it is (smallest magnitude wins).
    /// Right for a normally-closed chain such as the E-stop, where one pressed button must open it.
    And,
}

/// Effects the engine can inject. All off by default; the UI shows them when on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effects {
    /// Receiver self-test at power-up: every signal stays low this long (0 = no delay).
    #[serde(default)]
    pub self_test_ms: u64,
}

/// Header of a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllerMeta {
    /// Neutral id such as `generic-radio-6d`; never a vendor part name unless the user types it.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: ControllerKind,
    #[serde(default)]
    pub supply: Option<Supply>,
    /// Shared pin of the dry contacts.
    #[serde(default)]
    pub common: Option<String>,
    /// A higher step keeps the lower contacts closed (a real two-step switch). `false` for
    /// pendants that drop the slow contact when the fast one closes.
    #[serde(default = "yes")]
    pub step2_includes_step1: bool,
    /// This controller may drive a target that another controller also drives (service pendant).
    #[serde(default)]
    pub shared: bool,
    /// A generic stand-in, not a specific product (the UI says so).
    #[serde(default)]
    pub generic: bool,
    /// Conditions every signal needs (for example `main=on`), unless the signal is `ungated`.
    #[serde(default)]
    pub gate: Vec<String>,
}

fn yes() -> bool {
    true
}

/// One operator control.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlDef {
    pub id: String,
    #[serde(default)]
    pub label: String,
    pub kind: ControlKind,
    /// Positions, rest first (or see `rest`). Optional for kinds with a default.
    #[serde(default)]
    pub positions: Vec<String>,
    /// Rest position when it is not the first one in `positions`.
    #[serde(default)]
    pub rest: Option<String>,
    /// Returns to rest on release. Default depends on the kind.
    #[serde(default)]
    pub spring: Option<bool>,
    /// Receiver-side control: drops to rest when the radio link is lost or power is off, and
    /// cannot be set while it is down (main/start).
    #[serde(default)]
    pub needs_link: bool,
    /// Conditions needed to leave rest; when one stops holding the control drops to rest.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Conditions that send the control to rest and keep it there (a stop button for main).
    #[serde(default)]
    pub drops_when: Vec<String>,
    /// Joystick this axis belongs to. Axes that name the same stick share one pad: the first is
    /// left-right, the second up-down. Without it a joystick pairs its axes in order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stick: Option<String>,
}

impl ControlDef {
    /// The positions in use, including the kind's default when none are given.
    pub fn position_list(&self) -> Vec<String> {
        if self.positions.is_empty() {
            self.kind
                .default_positions()
                .map(|p| p.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default()
        } else {
            self.positions.clone()
        }
    }

    pub fn rest_position(&self) -> String {
        match &self.rest {
            Some(r) => r.clone(),
            None => self.position_list().into_iter().next().unwrap_or_default(),
        }
    }

    pub fn is_spring(&self) -> bool {
        self.spring.unwrap_or_else(|| self.kind.spring_by_default())
    }

    /// First position that is not the rest one (what `press` selects).
    pub fn first_active_position(&self) -> Option<String> {
        let rest = self.rest_position();
        self.position_list().into_iter().find(|p| *p != rest)
    }
}

/// How an interlock treats two controls held at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterlockMode {
    /// The control pressed second is refused while the first is down.
    #[default]
    FirstWins,
    /// Both held: both act as released (like contacts wired against each other).
    BothOff,
}

/// Mutual exclusion between separate controls (two push buttons for forward and reverse).
/// A single rocker cannot do both by construction, so it needs none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterlockDef {
    pub between: Vec<String>,
    #[serde(default)]
    pub mode: InterlockMode,
}

/// A named output with an electrical meaning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalDef {
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: SignalKind,
    pub control: String,
    /// High in these positions (contact, voltage). Alternative: `group` plus `step`.
    #[serde(default)]
    pub when: Vec<String>,
    /// Rocker group (`up` for positions `up1`, `up2`) and the contact number it closes.
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub step: Option<u8>,
    /// Analog value per position; positions not listed give 0.
    #[serde(default)]
    pub levels: std::collections::BTreeMap<String, f64>,
    #[serde(default)]
    pub unit: Option<String>,
    /// Extra conditions (`control=position`, `control!=position`).
    #[serde(default)]
    pub requires: Vec<String>,
    /// Not subject to the controller gate (the main contact and the E-stop chain itself).
    #[serde(default)]
    pub ungated: bool,
    /// Low while the link is lost. Default: true for a radio, false otherwise.
    #[serde(default)]
    pub needs_link: Option<bool>,
    /// How this signal combines with others on the same target: `or` (default) takes the largest
    /// magnitude, `and` the smallest among the `and` signals only; the two results then combine as
    /// the larger magnitude.
    #[serde(default)]
    pub combine: Combine,
    /// Terminal label the real device wires this signal to (for auto-binding by name).
    #[serde(default)]
    pub pin: Option<String>,
}

/// A whole controller profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub controller: ControllerMeta,
    #[serde(default)]
    pub effects: Effects,
    #[serde(default, rename = "control")]
    pub controls: Vec<ControlDef>,
    #[serde(default, rename = "signal")]
    pub signals: Vec<SignalDef>,
    #[serde(default, rename = "interlock")]
    pub interlocks: Vec<InterlockDef>,
}

/// Why a profile did not load.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileError {
    /// Not valid TOML or does not match the format (unknown field, wrong type).
    Syntax(String),
    /// Parsed, but the validator found errors.
    Invalid(Vec<Issue>),
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::Syntax(s) => write!(f, "profile syntax: {s}"),
            ProfileError::Invalid(issues) => {
                write!(f, "profile invalid: ")?;
                for (i, issue) in issues.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "{issue}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ProfileError {}

impl Profile {
    /// Parse without validating (the profile editor wants to show issues next to the data).
    pub fn parse(text: &str) -> Result<Profile, ProfileError> {
        toml::from_str(text).map_err(|e| ProfileError::Syntax(e.to_string()))
    }

    /// Parse and require that the validator finds no errors (warnings are allowed).
    pub fn load(text: &str) -> Result<Profile, ProfileError> {
        let profile = Self::parse(text)?;
        let errors: Vec<Issue> = validate(&profile)
            .into_iter()
            .filter(|i| i.severity == Severity::Error)
            .collect();
        if errors.is_empty() {
            Ok(profile)
        } else {
            Err(ProfileError::Invalid(errors))
        }
    }

    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn control(&self, id: &str) -> Option<&ControlDef> {
        self.controls.iter().find(|c| c.id == id)
    }

    pub fn signal(&self, name: &str) -> Option<&SignalDef> {
        self.signals.iter().find(|s| s.name == name)
    }
}

/// `control=position` or `control!=position`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    pub control: String,
    pub position: String,
    pub negate: bool,
}

impl FromStr for Condition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let (control, position, negate) = if let Some((c, p)) = s.split_once("!=") {
            (c, p, true)
        } else if let Some((c, p)) = s.split_once('=') {
            (c, p, false)
        } else {
            return Err(format!(
                "condition `{s}` is not control=position or control!=position"
            ));
        };
        let (control, position) = (control.trim(), position.trim());
        if control.is_empty() || position.is_empty() {
            return Err(format!("condition `{s}` has an empty side"));
        }
        Ok(Condition {
            control: control.to_string(),
            position: position.to_string(),
            negate,
        })
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.control,
            if self.negate { "!=" } else { "=" },
            self.position
        )
    }
}

/// Split `up2` into `("up", 2)`; `None` when the position has no trailing step number.
pub fn split_group_step(position: &str) -> Option<(&str, u8)> {
    let digits = position.len()
        - position
            .trim_end_matches(|c: char| c.is_ascii_digit())
            .len();
    if digits == 0 || digits == position.len() {
        return None;
    }
    let (group, n) = position.split_at(position.len() - digits);
    n.parse().ok().map(|n| (group, n))
}
