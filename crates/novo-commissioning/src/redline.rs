//! As-built redlines: what the machine turned out to be, noted at the machine.
//!
//! A redline is about one target. It is a free note, or a correction that can
//! be applied to the drawing: a new wire number or a new device tag.
//! Commissioning never edits the drawing itself; the redlines wait here until
//! the person is back in the editor, where [`apply_commands`] turns the open
//! ones into ordinary model commands (undoable, one transaction) and
//! [`revision_description`] writes what changed for the revision they are
//! issued under.

use novo_core::{Command, DeviceTag, Project};
use serde::{Deserialize, Serialize};

use crate::check::Target;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Change {
    /// A remark only; nothing to apply.
    Note,
    /// The wire carries this number on the machine.
    WireNumber(String),
    /// The device carries this tag on the machine.
    DeviceTag(String),
}

impl Change {
    /// Whether this change can be made on that kind of target.
    pub fn fits(&self, target: Target) -> bool {
        matches!(
            (self, target),
            (Change::Note, _)
                | (Change::WireNumber(_), Target::Wire(_))
                | (Change::DeviceTag(_), Target::Device(_))
        )
    }

    pub fn is_edit(&self) -> bool {
        !matches!(self, Change::Note)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedlineState {
    #[default]
    Open,
    /// Made in the drawing.
    Applied,
    Rejected,
}

impl RedlineState {
    pub fn label(self) -> &'static str {
        match self {
            RedlineState::Open => "Open",
            RedlineState::Applied => "Applied",
            RedlineState::Rejected => "Rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Redline {
    pub id: u64,
    pub target: Target,
    /// What the target was called when the redline was drawn, for the
    /// revision text and for a file read in another project.
    pub label: String,
    pub change: Change,
    #[serde(default)]
    pub note: String,
    pub by: String,
    /// ISO 8601 text.
    pub at: String,
    #[serde(default)]
    pub state: RedlineState,
}

/// The commands that make the open edits of `redlines` in the drawing, with
/// the id of the redline each came from. A redline that cannot be applied
/// (the object is gone, the tag does not parse) comes back as `Err` with the
/// reason, so the person sees it instead of a silent skip.
pub fn apply_commands<'a>(
    redlines: impl IntoIterator<Item = &'a Redline>,
    project: &Project,
) -> Vec<(u64, Result<Command, String>)> {
    redlines
        .into_iter()
        .filter(|r| r.state == RedlineState::Open && r.change.is_edit())
        .map(|r| (r.id, command_of(r, project)))
        .collect()
}

fn command_of(r: &Redline, project: &Project) -> Result<Command, String> {
    match (&r.change, r.target) {
        (Change::WireNumber(number), Target::Wire(wire)) => {
            if project.wire(wire).is_none() {
                return Err(format!("{} is no longer in the drawing", r.label));
            }
            let number = number.trim();
            Ok(Command::SetWireNumber {
                wire,
                number: (!number.is_empty()).then(|| number.to_owned()),
            })
        }
        (Change::DeviceTag(tag), Target::Device(device)) => {
            if project.device(device).is_none() {
                return Err(format!("{} is no longer in the drawing", r.label));
            }
            let tag: DeviceTag = tag
                .trim()
                .parse()
                .map_err(|e| format!("{} cannot be renamed to {tag}: {e}", r.label))?;
            Ok(Command::SetTag { device, tag })
        }
        _ => Err(format!("{}: the change does not fit the object", r.label)),
    }
}

/// One line per redline, for the description of the revision that issues them.
pub fn revision_description<'a>(redlines: impl IntoIterator<Item = &'a Redline>) -> String {
    let lines: Vec<String> = redlines
        .into_iter()
        .map(|r| {
            let what = match &r.change {
                Change::Note => format!("{}: {}", r.label, r.note.trim()),
                Change::WireNumber(n) => format!("Wire {} renumbered {}", r.label, n.trim()),
                Change::DeviceTag(t) => format!("Device {} renamed {}", r.label, t.trim()),
            };
            format!("As built: {what} ({})", r.by)
        })
        .collect();
    lines.join("\n")
}
