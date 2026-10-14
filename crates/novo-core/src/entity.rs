//! The objects a project is made of.
//!
//! These are plain data: reading them is free, but they change only through a
//! [`Tx`](crate::Tx) (or a [`Command`](crate::Command)), which validates every
//! edit and records it for undo.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geom::{Direction, Placement, Point};
use crate::id::{
    CableId, DeviceId, JunctionId, PageId, PartId, PotentialId, SymbolId, TerminalId, WireId,
};
use crate::tag::DeviceTag;

/// A free-form property value. Properties let later workstreams and user
/// scripts attach data without changing the model's types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_owned())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

/// Named properties, ordered by name so files stay diffable.
pub type Properties = BTreeMap<String, Value>;

/// Project-wide information shown in title blocks and reports.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub properties: Properties,
}

/// Standard paper sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PaperSize {
    A4,
    #[default]
    A3,
    A2,
    A1,
    A0,
}

impl PaperSize {
    /// Width and height in millimetres, portrait.
    pub fn portrait_mm(self) -> (f64, f64) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A2 => (420.0, 594.0),
            PaperSize::A1 => (594.0, 841.0),
            PaperSize::A0 => (841.0, 1189.0),
        }
    }
}

/// Paper size and orientation of a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageFormat {
    pub size: PaperSize,
    pub landscape: bool,
    /// Number of grid columns across the page, used for cross-references
    /// such as `3.4` (page 3, column 4). 0 counts as 1.
    #[serde(default = "default_columns")]
    pub columns: u16,
}

fn default_columns() -> u16 {
    PageFormat::DEFAULT_COLUMNS
}

impl Default for PageFormat {
    fn default() -> Self {
        PageFormat {
            size: PaperSize::A3,
            landscape: true,
            columns: PageFormat::DEFAULT_COLUMNS,
        }
    }
}

impl PageFormat {
    pub const DEFAULT_COLUMNS: u16 = 10;

    /// The grid column (1-based) at a horizontal position. Positions off the
    /// page fall into the first or last column.
    pub fn column_at(self, x: f64) -> u16 {
        let columns = self.columns.max(1);
        let (width, _) = self.size_mm();
        let column = (x / width * f64::from(columns)).floor();
        // NaN and negatives clamp to 0, so the result is 1..=columns.
        (column.clamp(0.0, f64::from(columns - 1)) as u16) + 1
    }

    /// Width and height of the page in millimetres.
    pub fn size_mm(self) -> (f64, f64) {
        let (w, h) = self.size.portrait_mm();
        if self.landscape { (h, w) } else { (w, h) }
    }
}

/// What a page is for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PageKind {
    /// A hand-drawn schematic page.
    #[default]
    Schematic,
    /// An overview or single-line page.
    Overview,
    /// A page generated from the model (terminal screen, cable screen,
    /// report); `generator` names the report type that owns it.
    Generated { generator: String },
}

/// A page of the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: PageId,
    /// The page name shown in cross references, e.g. `1`, `12` or `3.1`.
    /// Pages sort by this name in natural order.
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub kind: PageKind,
    #[serde(default)]
    pub format: PageFormat,
    #[serde(default)]
    pub properties: Properties,
}

/// What kind of thing a device is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// Any ordinary device: contactor, motor, push button, breaker.
    #[default]
    General,
    /// A terminal strip (`-X1`); its terminals are [`Terminal`] records.
    TerminalStrip,
    /// A plug or socket.
    Connector,
    /// A PLC rack, CPU or I/O card.
    Plc,
}

/// A logical device, identified by its tag. A device can be drawn several
/// times (a contactor's coil and its contacts on different pages); each
/// drawing is a [`SymbolInstance`] pointing back here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    pub tag: DeviceTag,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub kind: DeviceKind,
    /// Parts assigned to this device (a contactor plus its auxiliary block).
    #[serde(default)]
    pub parts: Vec<PartId>,
    #[serde(default)]
    pub properties: Properties,
}

/// Names a symbol definition in a symbol library (WS-05), e.g.
/// `iec/contacts/no`. The model does not interpret it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SymbolKey(pub String);

impl From<&str> for SymbolKey {
    fn from(key: &str) -> Self {
        SymbolKey(key.to_owned())
    }
}

/// A connection point of a placed symbol, copied from the symbol definition
/// when the symbol is placed so the model stays self-contained.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pin {
    /// The connection point designation printed on the device, e.g. `13`, `A1`.
    pub name: String,
    /// Position relative to the symbol origin, in symbol coordinates.
    pub offset: Point,
    /// The direction a wire leaves the point, in symbol coordinates. Wire
    /// routing uses it; it turns and mirrors with the symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<Direction>,
}

impl Pin {
    pub fn new(name: impl Into<String>, x: f64, y: f64) -> Self {
        Pin {
            name: name.into(),
            offset: Point::new(x, y),
            direction: None,
        }
    }

    /// The same pin with an exit direction, builder style.
    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = Some(direction);
        self
    }
}

/// What a placed symbol stands for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SymbolRole {
    /// Not assigned to anything yet.
    #[default]
    Unassigned,
    /// A function of a device. `main` marks the main function (the coil of a
    /// contactor) whose cross references list the auxiliary functions.
    Device { device: DeviceId, main: bool },
    /// One or more terminals of a terminal strip. A single-level terminal
    /// has one entry. A multi-level terminal block lists one terminal per
    /// level: `terminals[i]` owns the pins of `bridges[i]`, so the levels stay
    /// separate nets. A single terminal owns every pin and allows at most
    /// one bridge group; more groups need one terminal each.
    Terminal { terminals: Vec<TerminalId> },
    /// An interruption point (off-page connector). All interruption points
    /// with the same name are electrically one node.
    Interruption { name: String },
    /// A potential definition point: labels the net it touches with a
    /// potential, without connecting anything.
    PotentialFeed { potential: PotentialId },
}

/// A symbol placed on a page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolInstance {
    pub id: SymbolId,
    pub page: PageId,
    pub symbol: SymbolKey,
    pub placement: Placement,
    #[serde(default)]
    pub role: SymbolRole,
    /// Connection points. Wires address them by index, so the list may grow
    /// but an existing pin keeps its index while wires use it.
    #[serde(default)]
    pub pins: Vec<Pin>,
    /// Groups of pin indices that are connected inside the symbol (the two
    /// sides of a terminal, a busbar). Contacts are not bridges: whether a
    /// contact conducts is simulation state, not connectivity.
    #[serde(default)]
    pub bridges: Vec<Vec<u16>>,
    /// Groups of pin indices a potential passes through without them being
    /// connected: the switched poles of contacts, breakers and fuses
    /// (novo-symbols' `Symbol::pass_through()` minus its bridges). `L1`
    /// stays `L1` after a contactor, while coils, lamps and motors end it.
    /// One group per contact path: a changeover is `[[common, break],
    /// [common, make]]`, which keeps potentials on its break and make sides
    /// apart.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pass_through: Vec<Vec<u16>>,
    #[serde(default)]
    pub properties: Properties,
}

impl SymbolInstance {
    /// Page position of a pin, or `None` for an index out of range.
    pub fn pin_position(&self, pin: u16) -> Option<Point> {
        self.pins
            .get(usize::from(pin))
            .map(|p| self.placement.apply(p.offset))
    }

    /// Page direction a wire leaves a pin, when the pin defines one.
    pub fn pin_direction(&self, pin: u16) -> Option<Direction> {
        let local = self.pins.get(usize::from(pin))?.direction?;
        Some(self.placement.apply_direction(local))
    }

    /// The terminal a pin belongs to, for terminal symbols.
    pub fn terminal_of_pin(&self, pin: u16) -> Option<TerminalId> {
        let SymbolRole::Terminal { terminals } = &self.role else {
            return None;
        };
        if usize::from(pin) >= self.pins.len() {
            return None;
        }
        match terminals.as_slice() {
            [single] => Some(*single),
            _ => self
                .bridges
                .iter()
                .position(|group| group.contains(&pin))
                .and_then(|level| terminals.get(level).copied()),
        }
    }

    /// Index of the pin with this name.
    pub fn pin_index(&self, name: &str) -> Option<u16> {
        self.pins
            .iter()
            .position(|p| p.name == name)
            .and_then(|i| u16::try_from(i).ok())
    }

    /// The device this symbol draws, if any.
    pub fn device(&self) -> Option<DeviceId> {
        match self.role {
            SymbolRole::Device { device, .. } => Some(device),
            _ => None,
        }
    }
}

/// A connection point on a placed symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PinRef {
    pub symbol: SymbolId,
    pub pin: u16,
}

impl PinRef {
    pub fn new(symbol: SymbolId, pin: u16) -> Self {
        PinRef { symbol, pin }
    }
}

/// Something a wire can end on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Endpoint {
    Pin { symbol: SymbolId, pin: u16 },
    Junction { junction: JunctionId },
}

impl Endpoint {
    pub fn pin(symbol: SymbolId, pin: u16) -> Self {
        Endpoint::Pin { symbol, pin }
    }

    pub fn junction(junction: JunctionId) -> Self {
        Endpoint::Junction { junction }
    }

    pub fn as_pin(self) -> Option<PinRef> {
        match self {
            Endpoint::Pin { symbol, pin } => Some(PinRef { symbol, pin }),
            Endpoint::Junction { .. } => None,
        }
    }
}

impl From<PinRef> for Endpoint {
    fn from(p: PinRef) -> Self {
        Endpoint::Pin {
            symbol: p.symbol,
            pin: p.pin,
        }
    }
}

impl From<JunctionId> for Endpoint {
    fn from(junction: JunctionId) -> Self {
        Endpoint::Junction { junction }
    }
}

/// A connection dot where wires meet (T-junction).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Junction {
    pub id: JunctionId,
    pub page: PageId,
    pub position: Point,
}

/// A wire drawn between two endpoints on the same page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wire {
    pub id: WireId,
    pub page: PageId,
    pub from: Endpoint,
    pub to: Endpoint,
    /// Bend points between the two ends, in page coordinates.
    #[serde(default)]
    pub route: Vec<Point>,
    /// Wire number, when numbered.
    #[serde(default)]
    pub number: Option<String>,
    /// Colour code, e.g. `BK`, `BU`, `GNYE`.
    #[serde(default)]
    pub color: Option<String>,
    /// Cross-section in mm².
    #[serde(default)]
    pub cross_section_mm2: Option<f64>,
    #[serde(default)]
    pub properties: Properties,
}

impl Wire {
    /// A plain wire with no route, number or properties.
    pub fn new(id: WireId, page: PageId, from: Endpoint, to: Endpoint) -> Self {
        Wire {
            id,
            page,
            from,
            to,
            route: Vec::new(),
            number: None,
            color: None,
            cross_section_mm2: None,
            properties: Properties::new(),
        }
    }

    pub fn ends(&self) -> [Endpoint; 2] {
        [self.from, self.to]
    }
}

/// The type of a potential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PotentialKind {
    /// Line conductor (L1, L2, L3).
    Line,
    Neutral,
    /// Protective earth.
    ProtectiveEarth,
    DcPositive,
    DcNegative,
    #[default]
    Signal,
}

/// A named potential (`L1`, `N`, `PE`, `24V`, `0V`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Potential {
    pub id: PotentialId,
    pub name: String,
    #[serde(default)]
    pub kind: PotentialKind,
    /// Nominal voltage in volts.
    #[serde(default)]
    pub voltage: Option<f64>,
    #[serde(default)]
    pub properties: Properties,
}

/// The function of a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalKind {
    #[default]
    Feedthrough,
    ProtectiveEarth,
    Neutral,
    Fuse,
    Disconnect,
}

/// A terminal on a terminal strip device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Terminal {
    pub id: TerminalId,
    /// The terminal strip device (`-X1`) this terminal belongs to.
    pub strip: DeviceId,
    /// Terminal designation within the strip, e.g. `1`, `PE`.
    pub name: String,
    #[serde(default)]
    pub kind: TerminalKind,
    /// Level for multi-level terminals, starting at 1.
    #[serde(default = "default_level")]
    pub level: u8,
    /// Sort position within the strip; ties sort by name.
    #[serde(default)]
    pub order: i64,
    /// Terminals of one strip with the same jumper group are bridged.
    #[serde(default)]
    pub jumper: Option<u32>,
    #[serde(default)]
    pub properties: Properties,
}

fn default_level() -> u8 {
    1
}

/// One core of a cable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CableCore {
    /// Core designation: a number (`1`) or colour (`BK`, `GNYE`).
    pub name: String,
    #[serde(default)]
    pub cross_section_mm2: Option<f64>,
    #[serde(default)]
    pub shield: bool,
    /// The wire this core carries, if assigned.
    #[serde(default)]
    pub wire: Option<WireId>,
}

impl CableCore {
    pub fn new(name: impl Into<String>) -> Self {
        CableCore {
            name: name.into(),
            cross_section_mm2: None,
            shield: false,
            wire: None,
        }
    }
}

/// A cable (`-W1`) and its cores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cable {
    pub id: CableId,
    pub tag: DeviceTag,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub part: Option<PartId>,
    #[serde(default)]
    pub cores: Vec<CableCore>,
    /// Length in metres.
    #[serde(default)]
    pub length_m: Option<f64>,
    #[serde(default)]
    pub properties: Properties,
}

/// A manufacturer article used in this project. The parts database (WS-07)
/// owns the catalog; the project keeps its own copy of each part it uses so
/// a project file is complete on its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: PartId,
    pub manufacturer: String,
    /// Order (article) number, unique per manufacturer.
    pub order_number: String,
    #[serde(default)]
    pub type_number: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub properties: Properties,
}
