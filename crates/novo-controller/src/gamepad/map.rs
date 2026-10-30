//! The map from a pad to a controller: which axis or button drives which control, with deadzone,
//! calibration, invert and hysteresis, the dead-man input and what a vanished device does.
//! Plain data (TOML), stored with the controller instance in the project.
//!
//! ```toml
//! on_disconnect = "drop_link"
//!
//! [deadman]
//! input = "left_trigger"          # a button, or a trigger beyond half travel
//! control = "deadman"             # optional: the profile's dead-man control held with it
//!
//! [[axis]]
//! input = "left_y"
//! control = "hoist"               # a rocker, axis or selector control
//! deadzone = 0.15
//! centre = 0.01                   # calibration: raw value at rest, and at both ends
//! low = -0.98
//! high = 0.97
//!
//! [[button]]
//! input = "east"
//! control = "estop"
//! action = "estop"
//! ```

use serde::{Deserialize, Serialize};

use crate::panel::stick_axis;
use crate::profile::{ControlKind, Profile};
use crate::validate::{Issue, Severity};

use super::input::{is_axis_name, is_button_name};

/// Default deadzone: stick values up to this are rest.
pub const DEFAULT_DEADZONE: f64 = 0.15;
/// Default hysteresis: a stick must come back this far below a threshold to leave a step.
pub const DEFAULT_HYSTERESIS: f64 = 0.05;

fn d_deadzone() -> f64 {
    DEFAULT_DEADZONE
}
fn d_hysteresis() -> f64 {
    DEFAULT_HYSTERESIS
}
fn d_low() -> f64 {
    -1.0
}
fn d_high() -> f64 {
    1.0
}
fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}
fn is_false(b: &bool) -> bool {
    !*b
}

/// One axis driving one control. For a rocker or an axis control the stick position picks the
/// step (the detent) in each direction; for a selector it picks the position by equal bands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxisMap {
    /// Device axis, for example `left_y`.
    pub input: String,
    pub control: String,
    /// Share of the throw that counts as rest (0 to 0.9).
    #[serde(default = "d_deadzone")]
    pub deadzone: f64,
    /// Calibration: raw value with the stick at rest, and at full deflection each way.
    #[serde(default)]
    pub centre: f64,
    #[serde(default = "d_low")]
    pub low: f64,
    #[serde(default = "d_high")]
    pub high: f64,
    /// Swap the two directions.
    #[serde(default, skip_serializing_if = "is_false")]
    pub invert: bool,
    /// How far a value must fall back below a threshold before the step is left (0 to 0.5).
    #[serde(default = "d_hysteresis")]
    pub hysteresis: f64,
    /// Held back by the dead-man input (default). A selector is never held back.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub gated: bool,
}

impl AxisMap {
    pub fn new(input: &str, control: &str) -> Self {
        AxisMap {
            input: input.to_owned(),
            control: control.to_owned(),
            deadzone: DEFAULT_DEADZONE,
            centre: 0.0,
            low: -1.0,
            high: 1.0,
            invert: false,
            hysteresis: DEFAULT_HYSTERESIS,
            gated: true,
        }
    }

    /// The raw value as a calibrated, signed fraction of the throw, -1 to +1.
    pub fn value(&self, raw: f64) -> f64 {
        normalise(raw, self.centre, self.low, self.high, self.invert)
    }
}

/// Raw axis value to a signed fraction of the throw: `centre` is 0, `high` is +1, `low` is -1,
/// each side scaled on its own because sticks are rarely symmetric. A range that is not valid
/// (`high <= centre` or `low >= centre`) reads as if the side had the nominal throw of 1.
/// Never outside -1..1, never not-a-number.
pub fn normalise(raw: f64, centre: f64, low: f64, high: f64, invert: bool) -> f64 {
    if !raw.is_finite() {
        return 0.0;
    }
    let up = if high.is_finite() && high - centre > 1e-6 {
        high - centre
    } else {
        1.0
    };
    let down = if low.is_finite() && centre - low > 1e-6 {
        centre - low
    } else {
        1.0
    };
    let d = raw - centre;
    let v = if d >= 0.0 { d / up } else { d / down };
    let v = v.clamp(-1.0, 1.0);
    if invert { -v } else { v }
}

/// What a button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonAction {
    /// Hold the control at a position while the button is down (a push button, the dead-man
    /// control, one direction of a rocker). `position` defaults to the first active one.
    Hold,
    /// Press edge toggles a latch (main, start).
    Toggle,
    /// Press edge presses the E-stop. Never held back by the dead-man input.
    Estop,
    /// Press edge resets the control (twist-release of the E-stop).
    Reset,
    /// Press edge puts the control at `position` (a key switch, a selector).
    Set,
    /// Press edge moves a selector to the next position (stops at the end).
    Next,
    /// Press edge moves a selector to the previous position.
    Prev,
}

impl ButtonAction {
    pub const ALL: [ButtonAction; 7] = [
        ButtonAction::Hold,
        ButtonAction::Toggle,
        ButtonAction::Estop,
        ButtonAction::Reset,
        ButtonAction::Set,
        ButtonAction::Next,
        ButtonAction::Prev,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ButtonAction::Hold => "hold",
            ButtonAction::Toggle => "toggle latch",
            ButtonAction::Estop => "E-stop",
            ButtonAction::Reset => "reset",
            ButtonAction::Set => "set position",
            ButtonAction::Next => "next position",
            ButtonAction::Prev => "previous position",
        }
    }
}

/// One button (or trigger used as a button) driving one control.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ButtonMap {
    pub input: String,
    pub control: String,
    pub action: ButtonAction,
    /// For `hold` and `set`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    /// A `hold` is held back by the dead-man input (default).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub gated: bool,
}

impl ButtonMap {
    pub fn new(input: &str, control: &str, action: ButtonAction) -> Self {
        ButtonMap {
            input: input.to_owned(),
            control: control.to_owned(),
            action,
            position: None,
            gated: true,
        }
    }
}

/// The dead-man input of the pad.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeadmanMap {
    /// A button, or a trigger (pressed beyond half travel).
    pub input: String,
    /// The profile's dead-man control, held while the input is. Optional: the gate works without.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
}

/// What losing the device does, after every control has been let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkLoss {
    /// Release everything; nothing else.
    #[default]
    Release,
    /// Also drop the radio link, like a transmitter that lost power; the link comes back when
    /// the device does (receiver-side latches such as main stay dropped).
    DropLink,
    /// Also press the E-stop. It is not reset when the device returns.
    Estop,
}

impl LinkLoss {
    pub const ALL: [LinkLoss; 3] = [LinkLoss::Release, LinkLoss::DropLink, LinkLoss::Estop];

    pub fn label(self) -> &'static str {
        match self {
            LinkLoss::Release => "release all controls",
            LinkLoss::DropLink => "release and drop the radio link",
            LinkLoss::Estop => "release and press the E-stop",
        }
    }
}

/// The whole map of one controller instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GamepadMap {
    /// The device the map was made with (informational: the name the system reported).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub device: String,
    /// What happens when the device disappears.
    #[serde(default)]
    pub on_disconnect: LinkLoss,
    /// After a connect, a closed dead-man gate or a reconnect, a deflected stick (or held hold
    /// button) acts only after it has been let go once. Default on.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub neutral_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadman: Option<DeadmanMap>,
    #[serde(default, rename = "axis", skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<AxisMap>,
    #[serde(default, rename = "button", skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<ButtonMap>,
}

impl Default for GamepadMap {
    fn default() -> Self {
        GamepadMap {
            device: String::new(),
            on_disconnect: LinkLoss::Release,
            neutral_required: true,
            deadman: None,
            axes: Vec::new(),
            buttons: Vec::new(),
        }
    }
}

impl GamepadMap {
    pub fn parse(text: &str) -> Result<GamepadMap, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// True when the map drives nothing.
    pub fn is_empty(&self) -> bool {
        self.axes.is_empty() && self.buttons.is_empty() && self.deadman.is_none()
    }

    /// A first map for a profile, the usual layout: sticks drive the rockers and axes in the
    /// order of the profile (left stick up-down, left left-right, right up-down, right
    /// left-right), the left trigger is the dead-man when the profile has one, the east button
    /// presses the E-stop and north resets it, start toggles the first latch, other buttons hold
    /// the push buttons, the d-pad steps the first selector. A starting point to edit, not a
    /// recommendation for any real crane.
    pub fn suggest(profile: &Profile) -> GamepadMap {
        let mut map = GamepadMap::default();
        let sticks = ["left_y", "left_x", "right_y", "right_x"];
        let mut faces = [
            "south",
            "west",
            "right_bumper",
            "left_bumper",
            "left_thumb",
            "right_thumb",
            "select",
        ]
        .into_iter();
        let (mut stick, mut latch, mut estop, mut deadman, mut selector) =
            (0, false, false, false, false);
        for def in &profile.controls {
            match def.kind {
                ControlKind::Rocker2 | ControlKind::Rocker3 | ControlKind::Axis => {
                    if let Some(input) = sticks.get(stick) {
                        stick += 1;
                        map.axes.push(AxisMap::new(input, &def.id));
                    }
                }
                ControlKind::Latch if !latch => {
                    latch = true;
                    map.buttons
                        .push(ButtonMap::new("start", &def.id, ButtonAction::Toggle));
                }
                ControlKind::EStop if !estop => {
                    estop = true;
                    map.buttons
                        .push(ButtonMap::new("east", &def.id, ButtonAction::Estop));
                    map.buttons
                        .push(ButtonMap::new("north", &def.id, ButtonAction::Reset));
                }
                ControlKind::Deadman if !deadman => {
                    deadman = true;
                    map.deadman = Some(DeadmanMap {
                        input: "left_trigger".to_owned(),
                        control: Some(def.id.clone()),
                    });
                }
                ControlKind::Button => {
                    if let Some(input) = faces.next() {
                        map.buttons
                            .push(ButtonMap::new(input, &def.id, ButtonAction::Hold));
                    }
                }
                ControlKind::Selector if !selector => {
                    selector = true;
                    map.buttons
                        .push(ButtonMap::new("dpad_right", &def.id, ButtonAction::Next));
                    map.buttons
                        .push(ButtonMap::new("dpad_left", &def.id, ButtonAction::Prev));
                }
                _ => {}
            }
        }
        if profile.controller.kind == crate::profile::ControllerKind::Radio {
            map.on_disconnect = LinkLoss::DropLink;
        }
        map
    }

    /// Write calibration results into every axis map that reads those inputs.
    pub fn apply_calibration(&mut self, cal: &[AxisCal]) {
        for c in cal {
            for a in self.axes.iter_mut().filter(|a| a.input == c.input) {
                a.centre = c.centre;
                a.low = c.low;
                a.high = c.high;
            }
        }
    }

    /// Every problem at once, with the place in the map. Errors make the mapper skip the entry;
    /// warnings are things that work but are probably not meant.
    pub fn validate(&self, profile: &Profile) -> Vec<Issue> {
        let mut out = Vec::new();
        let mut issue = |severity, path: String, message: String| {
            out.push(Issue {
                severity,
                path,
                message,
            })
        };
        let err = Severity::Error;
        let warn = Severity::Warning;

        if let Some(d) = &self.deadman {
            let path = "deadman".to_owned();
            if !(is_button_name(&d.input) || is_axis_name(&d.input)) {
                issue(
                    err,
                    path.clone(),
                    format!("`{}` is not a pad input", d.input),
                );
            }
            if let Some(c) = &d.control {
                match profile.control(c) {
                    None => issue(err, path.clone(), format!("control `{c}` does not exist")),
                    Some(def) if def.kind != ControlKind::Deadman => issue(
                        err,
                        path.clone(),
                        format!("control `{c}` is a {}, not a dead-man", def.kind.as_str()),
                    ),
                    Some(_) => {}
                }
            }
        } else if profile
            .controls
            .iter()
            .any(|c| c.kind == ControlKind::Deadman)
        {
            issue(
                warn,
                "deadman".to_owned(),
                "the profile has a dead-man control but no pad input holds it".to_owned(),
            );
        }

        for (i, a) in self.axes.iter().enumerate() {
            let path = format!("axis[{i}] {}->{}", a.input, a.control);
            if !is_axis_name(&a.input) {
                issue(
                    err,
                    path.clone(),
                    format!("`{}` is not a pad axis", a.input),
                );
            }
            let def = profile.control(&a.control);
            match def {
                None => issue(
                    err,
                    path.clone(),
                    format!("control `{}` does not exist", a.control),
                ),
                Some(def) => {
                    let ok = matches!(
                        def.kind,
                        ControlKind::Rocker2
                            | ControlKind::Rocker3
                            | ControlKind::Axis
                            | ControlKind::Selector
                    );
                    if !ok {
                        issue(
                            err,
                            path.clone(),
                            format!(
                                "an axis cannot drive a {}; use a rocker, axis or selector",
                                def.kind.as_str()
                            ),
                        );
                    } else if def.kind != ControlKind::Selector {
                        let s = stick_axis(def);
                        if s.detents.len() < 2 {
                            issue(
                                warn,
                                path.clone(),
                                format!("control `{}` has no position besides rest", a.control),
                            );
                        } else if s.rest == 0 || s.rest + 1 == s.detents.len() {
                            issue(
                                warn,
                                path.clone(),
                                format!(
                                    "control `{}` moves in one direction only; the other half of the axis does nothing",
                                    a.control
                                ),
                            );
                        }
                    }
                }
            }
            if !(a.deadzone.is_finite() && (0.0..=0.9).contains(&a.deadzone)) {
                issue(
                    err,
                    path.clone(),
                    "deadzone must be between 0 and 0.9".to_owned(),
                );
            }
            if !(a.hysteresis.is_finite() && (0.0..=0.5).contains(&a.hysteresis)) {
                issue(
                    err,
                    path.clone(),
                    "hysteresis must be between 0 and 0.5".to_owned(),
                );
            }
            let cal_ok = a.centre.is_finite()
                && a.low.is_finite()
                && a.high.is_finite()
                && a.low < a.centre
                && a.centre < a.high;
            if !cal_ok {
                issue(
                    err,
                    path.clone(),
                    "calibration needs low < centre < high".to_owned(),
                );
            } else if a.high - a.centre < 0.3 || a.centre - a.low < 0.3 {
                issue(
                    warn,
                    path.clone(),
                    "calibrated throw is under 0.3 on one side; calibrate again".to_owned(),
                );
            }
            if a.deadzone.is_finite() && a.deadzone > 0.5 {
                issue(
                    warn,
                    path.clone(),
                    "a deadzone over 0.5 leaves little throw for the steps".to_owned(),
                );
            }
            if self.axes[..i]
                .iter()
                .any(|b| b.input == a.input && b.control == a.control)
            {
                issue(warn, path.clone(), "this axis is mapped twice".to_owned());
            } else if self.axes[..i].iter().any(|b| b.control == a.control) {
                issue(
                    warn,
                    path.clone(),
                    format!("control `{}` has two axes; the first one wins", a.control),
                );
            }
        }

        for (i, b) in self.buttons.iter().enumerate() {
            let path = format!("button[{i}] {}->{}", b.input, b.control);
            if !(is_button_name(&b.input) || is_axis_name(&b.input)) {
                issue(
                    err,
                    path.clone(),
                    format!("`{}` is not a pad button", b.input),
                );
            }
            let Some(def) = profile.control(&b.control) else {
                issue(
                    err,
                    path.clone(),
                    format!("control `{}` does not exist", b.control),
                );
                continue;
            };
            let positions = def.position_list();
            let has = |p: &str| positions.iter().any(|q| q == p);
            let kind_ok = match b.action {
                ButtonAction::Hold => matches!(
                    def.kind,
                    ControlKind::Button
                        | ControlKind::Deadman
                        | ControlKind::Rocker2
                        | ControlKind::Rocker3
                        | ControlKind::Axis
                ),
                ButtonAction::Toggle => def.kind == ControlKind::Latch,
                ButtonAction::Estop => def.kind == ControlKind::EStop,
                ButtonAction::Reset => true,
                ButtonAction::Set => def.kind != ControlKind::EStop,
                ButtonAction::Next | ButtonAction::Prev => def.kind == ControlKind::Selector,
            };
            if !kind_ok {
                issue(
                    err,
                    path.clone(),
                    format!(
                        "action `{}` does not fit a {} control",
                        b.action.label(),
                        def.kind.as_str()
                    ),
                );
            }
            match (b.action, &b.position) {
                (ButtonAction::Hold | ButtonAction::Set, Some(p)) if !has(p) => issue(
                    err,
                    path.clone(),
                    format!("control `{}` has no position `{p}`", b.control),
                ),
                (ButtonAction::Set, None) => {
                    issue(err, path.clone(), "`set position` needs a position".into())
                }
                (ButtonAction::Hold, None) => {
                    if def.first_active_position().is_none() {
                        issue(
                            err,
                            path.clone(),
                            format!("control `{}` has no active position to hold", b.control),
                        );
                    } else if matches!(def.kind, ControlKind::Rocker2 | ControlKind::Rocker3) {
                        issue(
                            warn,
                            path.clone(),
                            "no position given: holds the first step of the first direction".into(),
                        );
                    }
                }
                _ => {}
            }
            if b.action == ButtonAction::Estop && !b.gated {
                // Harmless but misleading: the E-stop is never gated.
                issue(
                    warn,
                    path.clone(),
                    "the E-stop button is never held back by the dead-man input".into(),
                );
            }
        }

        // A pad that cannot stop the machine is worth a warning when the profile has an E-stop.
        for def in profile
            .controls
            .iter()
            .filter(|c| c.kind == ControlKind::EStop)
        {
            let pressed = self
                .buttons
                .iter()
                .any(|b| b.control == def.id && b.action == ButtonAction::Estop);
            if !pressed {
                issue(
                    warn,
                    format!("estop {}", def.id),
                    format!("no pad button presses the E-stop `{}`", def.id),
                );
            }
        }
        if self.on_disconnect == LinkLoss::Estop
            && !profile
                .controls
                .iter()
                .any(|c| c.kind == ControlKind::EStop)
        {
            issue(
                err,
                "on_disconnect".to_owned(),
                "the profile has no E-stop to press when the device is lost".to_owned(),
            );
        }
        out
    }

    /// Whether `validate` found an error.
    pub fn has_errors(&self, profile: &Profile) -> bool {
        self.validate(profile)
            .iter()
            .any(|i| i.severity == Severity::Error)
    }
}

use super::calibrate::AxisCal;
