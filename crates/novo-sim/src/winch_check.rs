//! Statements about closed-loop winches and winch pairs for walk-throughs (lane L2c).
//!
//! A walk-through plays the same script on a healthy plant and on one with the circuit defects
//! of [`crate::inject`] on it. With closed-loop winches in the project (`cw_*` properties) their
//! loops run on both plants, so an open coil on an STO channel or a lost encoder signal in the
//! script reaches the drive and the script can ask what the winch did: where the hook is, whether
//! the drive tripped and with which code, whether STO is active, whether the run found something.
//!
//! The numbers come from the closed-loop winch engine (`novo-drive`) and the pair mechanics
//! (`novo-machine`): an averaged drive and a spring-damper rope model, not a measured drive.
//! Nothing here is a safety function.

use std::collections::BTreeSet;

use novo_drive::closed_loop::{ClFault, ClState, WinchSnapshot};
use novo_machine::cl_pair_loop::ClPairFindingKind;
use novo_machine::cl_winch_loop::ClFindingKind;
use serde::{Deserialize, Serialize};

use crate::machine::MachineSide;

/// Something a winch run can find, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingName {
    /// The drive tripped (any code).
    Trip,
    /// Both STO channels went low.
    StoRequested,
    /// The encoder signal device lost its supply or its wire.
    EncoderSignalLost,
    /// One encoder channel (the direction) lost its device's supply or wire.
    EncoderChannelLost,
    /// The overload protection inhibited lifting (single winch with a rated load).
    OverloadCutOut,
    /// The load cell signal device lost its supply or its wire (single winch with a rated load).
    LoadCellLost,
    /// The hook moved with the drive off and the brake commanded closed (single winch).
    BrakeSlip,
    /// The hook went against the command after the brake relay closed (single winch).
    StartRollback,
    /// The pair stopped itself (position error or partner fault).
    PairStopped,
    /// A rope carried more than its rating (pair).
    Overload,
    /// A rope went slack (pair), or the drum paid out rope beyond a hook on the ground (single winch).
    Slack,
    /// The slack was taken up and the rope snatched the hook off the ground (single winch).
    Snatch,
}

impl FindingName {
    fn text(self) -> &'static str {
        match self {
            Self::Trip => "a drive trip",
            Self::StoRequested => "safe torque off",
            Self::EncoderSignalLost => "a lost encoder signal",
            Self::EncoderChannelLost => "a lost encoder channel",
            Self::OverloadCutOut => "an overload cut-out",
            Self::LoadCellLost => "a lost load cell signal",
            Self::BrakeSlip => "a brake slip",
            Self::StartRollback => "a start rollback",
            Self::PairStopped => "the pair stopping itself",
            Self::Overload => "a rope above its rating",
            Self::Slack => "a slack rope",
            Self::Snatch => "a snatch on pick-up",
        }
    }
}

/// A statement about a closed-loop winch (a single one, or one winch of a pair).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WinchIs {
    /// The hook is at least this high, metres above the start.
    Above { m: f64 },
    /// The hook is at most this high.
    Below { m: f64 },
    /// Not moving (under 1 mm/s).
    Standing,
    /// Moving.
    Moving,
    /// The drive is tripped (any code).
    Tripped,
    /// The drive is tripped with this code (`STD`, `ENC`).
    Fault { code: String },
    /// The drive is not tripped.
    Clear,
    /// Safe torque off is active.
    StoActive,
    /// The run found this.
    Found { finding: FindingName },
}

/// What one winch reads, from a single winch or from one side of a pair.
struct Reading<'a> {
    snapshot: &'a WinchSnapshot,
    height_m: f64,
}

fn reading<'a>(side: &'a MachineSide, id: &str) -> Option<Reading<'a>> {
    if let Some(s) = side.cw.module(id).and_then(|m| m.snapshot()) {
        return Some(Reading {
            snapshot: s,
            height_m: s.hook_height_m,
        });
    }
    for m in &side.cp.modules {
        if let (Some(i), Some(s)) = (m.names.iter().position(|n| n == id), m.snapshot()) {
            return Some(Reading {
                snapshot: &s.winches[i],
                height_m: s.true_position_m[i],
            });
        }
    }
    None
}

/// Whether this id is a winch of the project at all (before the first tick it has no reading).
pub(crate) fn knows(side: &MachineSide, id: &str) -> bool {
    side.cw.module(id).is_some()
        || side
            .cp
            .modules
            .iter()
            .any(|m| m.names.iter().any(|n| n == id))
}

fn found(side: &MachineSide, id: &str, what: FindingName) -> bool {
    if let Some(m) = side.cw.module(id) {
        return m.findings().iter().any(|f| {
            matches!(
                (&f.kind, what),
                (ClFindingKind::Trip(_), FindingName::Trip)
                    | (ClFindingKind::StoRequested, FindingName::StoRequested)
                    | (
                        ClFindingKind::EncoderSignalLost,
                        FindingName::EncoderSignalLost
                    )
                    | (
                        ClFindingKind::EncoderChannelLost,
                        FindingName::EncoderChannelLost
                    )
                    | (ClFindingKind::SlackRope { .. }, FindingName::Slack)
                    | (ClFindingKind::RopePickedUp { .. }, FindingName::Snatch)
                    | (
                        ClFindingKind::OverloadCutOut { .. },
                        FindingName::OverloadCutOut
                    )
                    | (ClFindingKind::LoadCellLost, FindingName::LoadCellLost)
                    | (ClFindingKind::BrakeSlip { .. }, FindingName::BrakeSlip)
                    | (
                        ClFindingKind::StartRollback { .. },
                        FindingName::StartRollback
                    )
            )
        });
    }
    for m in &side.cp.modules {
        let Some(i) = m.names.iter().position(|n| n == id) else {
            continue;
        };
        return m.findings().iter().any(|f| match (&f.kind, what) {
            (ClPairFindingKind::Trip { winch, .. }, FindingName::Trip)
            | (ClPairFindingKind::StoRequested { winch }, FindingName::StoRequested)
            | (ClPairFindingKind::EncoderSignalLost { winch }, FindingName::EncoderSignalLost)
            | (ClPairFindingKind::EncoderChannelLost { winch }, FindingName::EncoderChannelLost)
            | (ClPairFindingKind::Overload { winch, .. }, FindingName::Overload)
            | (ClPairFindingKind::Slack { winch }, FindingName::Slack) => *winch == i,
            (ClPairFindingKind::Sync(_), FindingName::PairStopped) => true,
            _ => false,
        });
    }
    false
}

/// Checks a winch statement on the faulted side; the text says what was found.
pub(crate) fn check_winch(side: &MachineSide, id: &str, is: &WinchIs) -> (bool, String) {
    if !knows(side, id) {
        return (false, format!("no closed-loop winch {id} in the project"));
    }
    if let WinchIs::Found { finding } = is {
        let yes = found(side, id, *finding);
        return (
            yes,
            format!(
                "{id}: {} {}",
                finding.text(),
                if yes { "was found" } else { "was not found" }
            ),
        );
    }
    let Some(r) = reading(side, id) else {
        return (false, format!("{id} has not run yet"));
    };
    let speed = r.snapshot.hook_speed_m_s;
    let fault: Option<ClFault> = r.snapshot.drive.fault;
    match is {
        WinchIs::Above { m } => (
            r.height_m >= *m,
            format!(
                "hook of {id} is at {:.2} m, expected at least {m:.2} m",
                r.height_m
            ),
        ),
        WinchIs::Below { m } => (
            r.height_m <= *m,
            format!(
                "hook of {id} is at {:.2} m, expected at most {m:.2} m",
                r.height_m
            ),
        ),
        WinchIs::Standing => (
            speed.abs() < 0.001,
            format!("hook of {id} moves at {speed:.3} m/s, expected standing"),
        ),
        WinchIs::Moving => (
            speed.abs() >= 0.001,
            format!("hook of {id} moves at {speed:.3} m/s, expected moving"),
        ),
        WinchIs::Tripped => (fault.is_some(), drive_text(id, fault)),
        WinchIs::Fault { code } => (
            fault.is_some_and(|f| f.code().eq_ignore_ascii_case(code.trim())),
            format!("{}, expected {}", drive_text(id, fault), code.trim()),
        ),
        WinchIs::Clear => (fault.is_none(), drive_text(id, fault)),
        WinchIs::StoActive => (
            r.snapshot.drive.state == ClState::StoActive,
            format!(
                "drive of {id} is {}, expected safe torque off",
                if r.snapshot.drive.state == ClState::StoActive {
                    "in safe torque off"
                } else {
                    "not in safe torque off"
                }
            ),
        ),
        WinchIs::Found { .. } => unreachable!("handled above"),
    }
}

fn drive_text(id: &str, fault: Option<ClFault>) -> String {
    match fault {
        Some(f) => format!(
            "drive of {id} is tripped ({}: {})",
            f.code(),
            f.description()
        ),
        None => format!("drive of {id} is not tripped"),
    }
}

pub(crate) fn describe_winch(id: &str, is: &WinchIs) -> String {
    match is {
        WinchIs::Above { m } => format!("Expect hook of {id} at least {m:.2} m"),
        WinchIs::Below { m } => format!("Expect hook of {id} at most {m:.2} m"),
        WinchIs::Standing => format!("Expect hook of {id} standing"),
        WinchIs::Moving => format!("Expect hook of {id} moving"),
        WinchIs::Tripped => format!("Expect drive of {id} tripped"),
        WinchIs::Fault { code } => format!("Expect drive of {id} tripped with {}", code.trim()),
        WinchIs::Clear => format!("Expect drive of {id} not tripped"),
        WinchIs::StoActive => format!("Expect drive of {id} in safe torque off"),
        WinchIs::Found { finding } => format!("Expect {id} to find {}", finding.text()),
    }
}

/// How the faulted winches differ from the healthy ones: a clear gap in hook height, a drive
/// that tripped where the healthy one did not, the findings only the faulted run has.
pub(crate) fn differences(clean: &MachineSide, faulty: &MachineSide, out: &mut BTreeSet<String>) {
    let mut ids: Vec<String> = faulty.cw.modules.iter().map(|m| m.id.clone()).collect();
    for m in &faulty.cp.modules {
        ids.extend(m.names.iter().cloned());
    }
    for id in ids {
        if let (Some(a), Some(b)) = (reading(clean, &id), reading(faulty, &id)) {
            let gap = b.height_m - a.height_m;
            if gap.abs() >= 0.05 {
                out.insert(format!(
                    "hook of {id} is {:.2} m {} than a healthy winch's",
                    gap.abs(),
                    if gap > 0.0 { "higher" } else { "lower" }
                ));
            }
            if let (Some(f), None) = (b.snapshot.drive.fault, a.snapshot.drive.fault) {
                out.insert(format!(
                    "{id}: the drive trips ({}: {})",
                    f.code(),
                    f.description()
                ));
            }
        }
    }
    for m in &faulty.cw.modules {
        let had: BTreeSet<String> = clean
            .cw
            .module(&m.id)
            .map(|c| c.findings().iter().map(|f| f.kind.text()).collect())
            .unwrap_or_default();
        for f in m.findings() {
            let text = f.kind.text();
            if !had.contains(&text) && !matches!(f.kind, ClFindingKind::Trip(_)) {
                out.insert(format!("{}: {text}", m.id));
            }
        }
    }
    for m in &faulty.cp.modules {
        let had: BTreeSet<String> = clean
            .cp
            .module(&m.id)
            .map(|c| c.findings().iter().map(|f| f.kind.text()).collect())
            .unwrap_or_default();
        for f in m.findings() {
            let text = f.kind.text();
            if !had.contains(&text) && !matches!(f.kind, ClPairFindingKind::Trip { .. }) {
                out.insert(format!("{} + {}: {text}", m.names[0], m.names[1]));
            }
        }
    }
}
