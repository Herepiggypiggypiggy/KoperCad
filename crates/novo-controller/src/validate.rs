//! The profile validator: returns every problem at once, errors and warnings, each with the
//! place in the profile it concerns.

use std::collections::BTreeSet;
use std::fmt;

use crate::profile::{
    Condition, ControlDef, ControlKind, ControllerKind, Profile, SignalKind, split_group_step,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    /// Where: `controller`, `control[hoist]`, `signal[H_UP_SLOW]`, `interlock[0]`.
    pub path: String,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Warning => "warning",
            Severity::Error => "error",
        };
        write!(f, "{level} at {}: {}", self.path, self.message)
    }
}

struct Sink(Vec<Issue>);

impl Sink {
    fn error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.0.push(Issue {
            severity: Severity::Error,
            path: path.into(),
            message: message.into(),
        });
    }
    fn warn(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.0.push(Issue {
            severity: Severity::Warning,
            path: path.into(),
            message: message.into(),
        });
    }
}

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Check a profile. An empty result means nothing to report.
pub fn validate(profile: &Profile) -> Vec<Issue> {
    let mut out = Sink(Vec::new());
    let meta = &profile.controller;

    if !valid_name(&meta.id) {
        out.error(
            "controller",
            format!(
                "id `{}` must be non-empty letters, digits, `_`, `-` or `.`",
                meta.id
            ),
        );
    }
    if meta.name.trim().is_empty() {
        out.error("controller", "name is empty");
    }
    if let Some(s) = meta.supply
        && !(s.volts.is_finite() && s.volts > 0.0)
    {
        out.error("controller", "supply volts must be a positive number");
    }
    for c in &meta.gate {
        check_condition(profile, &mut out, "controller", c);
    }

    // controls
    let mut seen = BTreeSet::new();
    for control in &profile.controls {
        let path = format!("control[{}]", control.id);
        if !valid_name(&control.id) {
            out.error(
                &path,
                "id must be non-empty letters, digits, `_`, `-` or `.`",
            );
        }
        if !seen.insert(control.id.as_str()) {
            out.error(&path, "duplicate control id");
        }
        check_control(profile, control, &path, &mut out);
    }
    if profile.controls.is_empty() {
        out.error("controller", "no controls");
    }

    // signals
    let mut seen = BTreeSet::new();
    for signal in &profile.signals {
        let path = format!("signal[{}]", signal.name);
        if !valid_name(&signal.name) {
            out.error(
                &path,
                "name must be non-empty letters, digits, `_`, `-` or `.`",
            );
        }
        if !seen.insert(signal.name.as_str()) {
            out.error(&path, "duplicate signal name");
        }
        for c in &signal.requires {
            check_condition(profile, &mut out, &path, c);
        }
        let Some(control) = profile.control(&signal.control) else {
            out.error(
                &path,
                format!("control `{}` does not exist", signal.control),
            );
            continue;
        };
        let positions = control.position_list();
        let has_pos = |p: &str| positions.iter().any(|q| q == p);
        match signal.kind {
            SignalKind::Analog => {
                if signal.levels.is_empty() {
                    out.error(&path, "an analog signal needs `levels`");
                }
                if !signal.when.is_empty() || signal.group.is_some() || signal.step.is_some() {
                    out.error(
                        &path,
                        "an analog signal takes `levels`, not `when` or `group`",
                    );
                }
                for (p, v) in &signal.levels {
                    if !has_pos(p) {
                        out.error(
                            &path,
                            format!(
                                "level for unknown position `{p}` of control `{}`",
                                control.id
                            ),
                        );
                    }
                    if !v.is_finite() {
                        out.error(&path, format!("level for `{p}` is not a finite number"));
                    }
                }
            }
            SignalKind::Contact | SignalKind::Voltage => {
                if !signal.levels.is_empty() {
                    out.error(&path, "`levels` is only for analog signals");
                }
                let by_group = signal.group.is_some() || signal.step.is_some();
                if by_group == !signal.when.is_empty() {
                    out.error(
                        &path,
                        "give either `when` or `group` with `step`, not both and not neither",
                    );
                }
                for p in &signal.when {
                    if !has_pos(p) {
                        out.error(
                            &path,
                            format!(
                                "`when` names unknown position `{p}` of control `{}`",
                                control.id
                            ),
                        );
                    }
                }
                if by_group {
                    check_group_step(
                        control,
                        signal.group.as_deref(),
                        signal.step,
                        &positions,
                        &path,
                        &mut out,
                    );
                }
            }
        }
        if signal.kind == SignalKind::Contact && meta.common.is_none() {
            out.warn(
                &path,
                "contact signal but the controller has no `common` pin",
            );
        }
    }
    if profile.signals.is_empty() {
        out.warn("controller", "no signals: the controls drive nothing");
    }

    // interlocks
    for (i, il) in profile.interlocks.iter().enumerate() {
        let path = format!("interlock[{i}]");
        if il.between.len() < 2 {
            out.error(&path, "an interlock needs at least two controls");
        }
        let distinct: BTreeSet<&String> = il.between.iter().collect();
        if distinct.len() != il.between.len() {
            out.error(&path, "a control is listed twice");
        }
        for id in &il.between {
            if profile.control(id).is_none() {
                out.error(&path, format!("control `{id}` does not exist"));
            }
        }
    }

    check_requires_cycles(profile, &mut out);

    if matches!(meta.kind, ControllerKind::Radio | ControllerKind::Pendant)
        && !profile
            .controls
            .iter()
            .any(|c| c.kind == ControlKind::EStop)
    {
        out.warn(
            "controller",
            "no emergency stop control: radios and pendants for cranes normally have one",
        );
    }

    out.0
}

fn check_condition(profile: &Profile, out: &mut Sink, path: &str, text: &str) {
    match text.parse::<Condition>() {
        Err(e) => out.error(path, e),
        Ok(c) => match profile.control(&c.control) {
            None => out.error(
                path,
                format!("condition `{text}`: control `{}` does not exist", c.control),
            ),
            Some(def) => {
                if !def.position_list().contains(&c.position) {
                    out.error(
                        path,
                        format!(
                            "condition `{text}`: control `{}` has no position `{}`",
                            c.control, c.position
                        ),
                    );
                }
            }
        },
    }
}

fn check_group_step(
    control: &ControlDef,
    group: Option<&str>,
    step: Option<u8>,
    positions: &[String],
    path: &str,
    out: &mut Sink,
) {
    let (Some(group), Some(step)) = (group, step) else {
        out.error(path, "`group` and `step` go together");
        return;
    };
    let Some(max) = control.kind.rocker_steps() else {
        out.error(
            path,
            format!(
                "`group`/`step` only works on rocker controls, `{}` is {}",
                control.id,
                control.kind.as_str()
            ),
        );
        return;
    };
    if step == 0 || step > max {
        out.error(
            path,
            format!(
                "step {step} is outside 1..={max} for a {}",
                control.kind.as_str()
            ),
        );
    }
    if !positions
        .iter()
        .any(|p| split_group_step(p).is_some_and(|(g, _)| g == group))
    {
        out.error(
            path,
            format!("control `{}` has no group `{group}`", control.id),
        );
    }
}

fn check_control(profile: &Profile, control: &ControlDef, path: &str, out: &mut Sink) {
    let positions = control.position_list();
    if positions.len() < 2 {
        out.error(
            path,
            if control.positions.is_empty() {
                format!("kind {} needs explicit `positions`", control.kind.as_str())
            } else {
                "needs at least two positions".to_string()
            },
        );
        return;
    }
    let mut seen = BTreeSet::new();
    for p in &positions {
        if p.is_empty() || p.contains(['=', '!']) {
            out.error(
                path,
                format!("position `{p}` must be non-empty and contain no `=` or `!`"),
            );
        }
        if !seen.insert(p.as_str()) {
            out.error(path, format!("duplicate position `{p}`"));
        }
    }
    if let Some(rest) = &control.rest
        && !positions.contains(rest)
    {
        out.error(path, format!("rest `{rest}` is not one of the positions"));
    }
    if let Some(max) = control.kind.rocker_steps() {
        let rest = control.rest_position();
        for p in positions.iter().filter(|p| **p != rest) {
            match split_group_step(p) {
                Some((_, n)) if (1..=max).contains(&n) => {}
                _ => out.error(path, format!("rocker position `{p}` must be a group name and a step 1..={max}, like `up1`")),
            }
        }
    }
    if control.kind == ControlKind::EStop && control.is_spring() {
        out.error(path, "an E-stop latches; `spring = true` is not allowed");
    }
    for c in control.requires.iter().chain(&control.drops_when) {
        check_condition(profile, out, path, c);
        if c.parse::<Condition>()
            .is_ok_and(|c| c.control == control.id)
        {
            out.error(
                path,
                format!("condition `{c}` refers to the control itself"),
            );
        }
    }
    let receiver_side = control.kind == ControlKind::Latch;
    if !receiver_side
        && (!control.requires.is_empty() || !control.drops_when.is_empty() || control.needs_link)
    {
        out.warn(path, "`requires`, `drops_when` and `needs_link` are meant for latches; here the control is forced back to rest");
    }
}

/// Controls whose `requires` or `drops_when` refer to each other in a ring: whichever is set
/// first decides, which is rarely what the profile means.
fn check_requires_cycles(profile: &Profile, out: &mut Sink) {
    let edges = |c: &ControlDef| -> Vec<String> {
        c.requires
            .iter()
            .chain(&c.drops_when)
            .filter_map(|t| t.parse::<Condition>().ok().map(|c| c.control))
            .collect()
    };
    for start in &profile.controls {
        let mut seen = BTreeSet::new();
        let mut stack = edges(start);
        while let Some(id) = stack.pop() {
            if id == start.id {
                out.warn(
                    format!("control[{}]", start.id),
                    "its conditions lead back to itself through other controls",
                );
                break;
            }
            if seen.insert(id.clone())
                && let Some(c) = profile.control(&id)
            {
                stack.extend(edges(c));
            }
        }
    }
}
