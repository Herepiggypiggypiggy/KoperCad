//! The plain-data form of a library circuit: a TOML file that names the
//! devices, the library symbols with their places, and the wires. The file
//! says nothing the symbol library already knows (pins, bridges, exit
//! directions): [`super::build`] reads those from the library, so a circuit
//! cannot drift from its symbols.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::CircuitError;
use super::mounting::MountKind;

/// One circuit file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CircuitSpec {
    /// The macro name; unique in the library.
    pub name: String,
    /// The chapter slug, one of [`super::CHAPTERS`].
    pub chapter: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// What the circuit assumes and what the user connects, in plain words.
    /// Shown in the macro description after the summary.
    #[serde(default)]
    pub notes: String,
    #[serde(default, rename = "parameter")]
    pub parameters: Vec<ParameterSpec>,
    #[serde(rename = "device")]
    pub devices: Vec<DeviceSpec>,
    #[serde(rename = "symbol")]
    pub symbols: Vec<SymbolSpec>,
    #[serde(default, rename = "junction")]
    pub junctions: Vec<JunctionSpec>,
    #[serde(default, rename = "wire")]
    pub wires: Vec<WireSpec>,
    #[serde(default, rename = "chain")]
    pub chains: Vec<ChainSpec>,
    #[serde(default, rename = "bus")]
    pub buses: Vec<BusSpec>,
    /// The points the user connects after placing, as `symbol.pin` (pins
    /// are listed with commas: `f1s.1,3,5`). The build refuses a circuit
    /// that leaves any other pin unwired, or wires one named here.
    #[serde(default)]
    pub open: Vec<String>,
    /// The reference mounting plate and rails of the mounting view.
    #[serde(default)]
    pub plate: Option<PlateSpec>,
    #[serde(default, rename = "rail")]
    pub rails: Vec<RailSpec>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterSpec {
    pub name: String,
    pub default: ValueSpec,
    #[serde(default)]
    pub description: String,
}

/// A number, a switch or a text, as TOML writes them.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ValueSpec {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSpec {
    /// The name the symbols refer to.
    pub id: String,
    /// IEC 81346 tag, e.g. `-K1`.
    pub tag: String,
    pub description: String,
    /// Device properties; a text may name a parameter as `{name}`.
    #[serde(default)]
    pub props: BTreeMap<String, ValueSpec>,
    /// Where the device is mounted. Omitted: the device has no mounting
    /// entry (a terminal, a symbol-only helper).
    #[serde(default)]
    pub mount: Option<MountSpec>,
}

/// Where a device sits and how big it is, typical values for a generic
/// device of this kind. The real size comes from the part the user assigns.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MountSpec {
    pub kind: MountKind,
    /// Width, height, depth in millimetres. Not for `field`.
    #[serde(default)]
    pub size: Option<[f64; 3]>,
    /// Top left on the reference plate (or door), millimetres. Not for
    /// `field`.
    #[serde(default)]
    pub at: Option<[f64; 2]>,
    /// The rail it clips on, for `din`.
    #[serde(default)]
    pub rail: Option<String>,
    /// Where a field device sits, in words: "on the motor", "on the hook".
    #[serde(default)]
    pub place: Option<String>,
    /// What is mounted with it that is not in the schematic: a rectifier
    /// block, a terminal set, a bracket.
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolSpec {
    pub id: String,
    /// Library key, e.g. `iec/contactors/main_3p`.
    pub key: String,
    /// Origin, millimetres, on the 4 mm grid.
    pub at: [f64; 2],
    /// The device it belongs to; omitted for a terminal or a helper.
    #[serde(default)]
    pub device: Option<String>,
    /// The device's main symbol. Default: the first symbol listed for it.
    #[serde(default)]
    pub main: Option<bool>,
    /// Quarter turns counter-clockwise as degrees: 0, 90, 180 or 270.
    #[serde(default)]
    pub rotate: u16,
    /// Mirrored left to right before it is turned.
    #[serde(default)]
    pub mirror: bool,
    /// New names for the pins in library order (`13`, `14` becomes `23`,
    /// `24` for a second contact).
    #[serde(default)]
    pub numbers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JunctionSpec {
    pub id: String,
    pub at: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireSpec {
    /// `symbol.pin` or a junction id.
    pub from: String,
    pub to: String,
    /// Corner points between the ends, millimetres.
    #[serde(default)]
    pub via: Vec<[f64; 2]>,
}

/// Several wires written as one line: `a.1,3,5 > b.1,3,5 > c.U1,V1,W1`
/// joins the first listed pin of each node to the first of the next, and so
/// on.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainSpec {
    pub path: String,
}

/// A horizontal bus on row `y`: one junction at the x of every tap, in a
/// chain from left to right, joined to the tap pins by straight wires.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusSpec {
    pub y: f64,
    pub taps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlateSpec {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RailSpec {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub length: f64,
}

impl CircuitSpec {
    /// Reads a circuit file. `origin` names the file in error messages.
    pub fn parse(text: &str, origin: &str) -> Result<Self, CircuitError> {
        toml::from_str(text).map_err(|e| CircuitError::Parse {
            origin: origin.into(),
            reason: e.to_string(),
        })
    }
}

/// A pin reference as written in a file: `symbol.pin`, or a bare junction id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Node<'a> {
    Pin { symbol: &'a str, pin: &'a str },
    Junction(&'a str),
}

impl<'a> Node<'a> {
    pub(super) fn parse(text: &'a str) -> Node<'a> {
        let text = text.trim();
        match text.split_once('.') {
            Some((symbol, pin)) => Node::Pin { symbol, pin },
            None => Node::Junction(text),
        }
    }
}

/// One hop of a chain: a symbol with the pins that take the wire from the
/// hop before and the pins that start the wire to the hop after (the same
/// pins unless the hop is written `in/out`), or a junction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Hop<'a> {
    Pins {
        symbol: &'a str,
        incoming: Vec<&'a str>,
        outgoing: Vec<&'a str>,
    },
    Junction(&'a str),
}

fn pin_list(pins: &str) -> Vec<&str> {
    pins.split(',').map(str::trim).collect()
}

pub(super) fn parse_chain(path: &str) -> Vec<Hop<'_>> {
    path.split('>')
        .map(|hop| {
            let hop = hop.trim();
            match hop.split_once('.') {
                Some((symbol, pins)) => match pins.split_once('/') {
                    Some((incoming, outgoing)) => Hop::Pins {
                        symbol,
                        incoming: pin_list(incoming),
                        outgoing: pin_list(outgoing),
                    },
                    None => Hop::Pins {
                        symbol,
                        incoming: pin_list(pins),
                        outgoing: pin_list(pins),
                    },
                },
                None => Hop::Junction(hop),
            }
        })
        .collect()
}
