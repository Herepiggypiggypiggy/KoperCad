//! Pins and devices by their printed names, and the switched poles behind them.
//!
//! Controller bindings (lane C2) and the machine loop's sensor contacts name a drawn contact the
//! way a person reads it: `-K1:13`, a device tag and a pin designation. This module finds the
//! pole of the symbol that pin belongs to, so a caller can force that pole in a [`PoleState`]
//! before the next settle. It adds no state of its own.

use std::fmt;

use novo_core::{DeviceId, DeviceTag, Project, SymbolId};

use crate::PoleState;

/// A switched pole: group `group` of `SymbolInstance::pass_through` of `symbol`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PoleRef {
    pub symbol: SymbolId,
    pub group: usize,
}

/// Why a name did not lead to a pole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinProblem {
    /// The text is not `tag:pin`.
    NotAPin(String),
    /// No device carries this tag in the drawing.
    NoDevice(String),
    /// Several devices carry this tag.
    DuplicateDevice(String),
    /// The device has no pin with this designation.
    NoPin { device: String, pin: String },
    /// The pin exists but belongs to no switched pole (a coil, a lamp, a terminal).
    NotSwitched { device: String, pin: String },
    /// The pin is shared by several poles (a common): bind one of the outputs instead.
    Shared {
        device: String,
        pin: String,
        poles: usize,
    },
    /// The device has no switched pole at all.
    NoPoles(String),
}

impl fmt::Display for PinProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PinProblem::NotAPin(t) => write!(
                f,
                "`{t}` is not a pin (expected a tag and a pin, such as -A1:3)"
            ),
            PinProblem::NoDevice(t) => write!(f, "no device {t} in the drawing"),
            PinProblem::DuplicateDevice(t) => write!(f, "more than one device has the tag {t}"),
            PinProblem::NoPin { device, pin } => write!(f, "{device} has no pin {pin}"),
            PinProblem::NotSwitched { device, pin } => {
                write!(f, "{device}:{pin} is not a switched contact pin")
            }
            PinProblem::Shared { device, pin, poles } => write!(
                f,
                "{device}:{pin} is common to {poles} contacts; bind one of the output pins"
            ),
            PinProblem::NoPoles(t) => write!(f, "{t} has no switched contacts"),
        }
    }
}

impl std::error::Error for PinProblem {}

/// The device with this tag text (`=A1+CAB1-K1`, `-K1`).
pub fn device_by_text(project: &Project, text: &str) -> Result<DeviceId, PinProblem> {
    let tag = DeviceTag::parse(text.trim()).map_err(|_| PinProblem::NotAPin(text.to_owned()))?;
    if tag.is_empty() {
        return Err(PinProblem::NotAPin(text.to_owned()));
    }
    let found = project.devices_by_tag(&tag);
    match found.as_slice() {
        [one] => Ok(*one),
        [] => Err(PinProblem::NoDevice(text.trim().to_owned())),
        _ => Err(PinProblem::DuplicateDevice(text.trim().to_owned())),
    }
}

/// The switched pole whose output side is the pin `designation` (`tag:pin`). A pin that two or
/// more poles share, such as the common of a receiver, is refused ([`PinProblem::Shared`]).
pub fn pole_of_pin(project: &Project, designation: &str) -> Result<PoleRef, PinProblem> {
    let (tag, pin) = designation
        .split_once(':')
        .ok_or_else(|| PinProblem::NotAPin(designation.to_owned()))?;
    let (tag, pin) = (tag.trim(), pin.trim());
    let device = device_by_text(project, tag)?;
    let mut named = false;
    let mut poles = Vec::new();
    for symbol in project.symbols_of_device(device) {
        for (index, p) in symbol.pins.iter().enumerate() {
            if p.name != pin {
                continue;
            }
            named = true;
            for (group, pins) in symbol.pass_through.iter().enumerate() {
                if pins.iter().any(|&i| usize::from(i) == index) {
                    poles.push(PoleRef {
                        symbol: symbol.id,
                        group,
                    });
                }
            }
        }
    }
    match poles.as_slice() {
        [one] => Ok(*one),
        [] if named => Err(PinProblem::NotSwitched {
            device: tag.to_owned(),
            pin: pin.to_owned(),
        }),
        [] => Err(PinProblem::NoPin {
            device: tag.to_owned(),
            pin: pin.to_owned(),
        }),
        many => Err(PinProblem::Shared {
            device: tag.to_owned(),
            pin: pin.to_owned(),
            poles: many.len(),
        }),
    }
}

/// Every switched pole of the device with this tag text (a pushbutton, a switch, a breaker).
pub fn poles_of_device(project: &Project, text: &str) -> Result<Vec<PoleRef>, PinProblem> {
    let device = device_by_text(project, text)?;
    let poles: Vec<PoleRef> = project
        .symbols_of_device(device)
        .into_iter()
        .flat_map(|s| {
            (0..s.pass_through.len()).map(move |group| PoleRef {
                symbol: s.id,
                group,
            })
        })
        .collect();
    if poles.is_empty() {
        Err(PinProblem::NoPoles(text.trim().to_owned()))
    } else {
        Ok(poles)
    }
}

/// Forces `pole` closed or open in `state`.
pub fn force_pole(state: &mut PoleState, pole: PoleRef, closed: bool) {
    state.set(pole.symbol, pole.group, closed);
}

/// Operates a hand device: its poles go to the opposite of their rest state (`operated`) or back
/// to it. `rest` is [`crate::rest_state`] of the project.
pub fn operate_poles(state: &mut PoleState, rest: &PoleState, poles: &[PoleRef], operated: bool) {
    for p in poles {
        let at_rest = rest.is_closed(p.symbol, p.group);
        state.set(p.symbol, p.group, at_rest != operated);
    }
}
