//! The objects a project is made of.
//!
//! These are plain data: reading them is free, but they change only through a
//! [`Tx`](crate::Tx) (or a [`Command`](crate::Command)), which validates every
//! edit and records it for undo.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::geom::{Direction, Placement, Point, Rotation};
use crate::id::{
    AnyId, BlockDefinitionId, BlockInstanceId, CabinetPlacementId, CableId, DeviceId, DismissalId,
    JunctionId, PageId, PartId, PotentialId, PotentialSystemId, RailId, RevisionId, StructureId,
    SymbolId, TerminalId, TextId, WireId,
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

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(v) => write!(f, "{v}"),
            Value::Int(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Text(v) => f.write_str(v),
        }
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
    /// Project-wide rules that change how the drawing is interpreted. Every
    /// rule is explicit and off by default.
    #[serde(default, skip_serializing_if = "ProjectRules::is_default")]
    pub rules: ProjectRules,
    #[serde(default)]
    pub properties: Properties,
}

/// Explicit, inspectable project rules. A file without an entry has the
/// rule off, so loading never changes what a project means; new projects
/// start from [`ProjectRules::recommended`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectRules {
    #[serde(default, skip_serializing_if = "TerminalRules::is_off")]
    pub terminals: TerminalRules,
    #[serde(default, skip_serializing_if = "PotentialRules::is_off")]
    pub potentials: PotentialRules,
    /// The generated-page kinds the user turned off, by generator id
    /// (`bom`, `terminals`, `cables`, ...). Empty, the default, wants
    /// every kind. Deleting a generated page ([`Tx::delete_page`]) adds
    /// its kind, so it does not come back on open.
    ///
    /// [`Tx::delete_page`]: crate::Tx::delete_page
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub generated_pages_off: BTreeSet<String>,
    /// The drawing standard, as the id of the symbol pack it picks (`iec`,
    /// `nfpa`); lowercase letters, digits, `_` and `-`. Empty, the default,
    /// means IEC. The model does not interpret it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub standard: String,
}

impl ProjectRules {
    /// The rules a new project starts with: terminals sharing a name are
    /// joined with a connector (WORKFLOW.md J5).
    pub fn recommended() -> Self {
        ProjectRules {
            terminals: TerminalRules {
                same_name: SameName::Join,
                join_link: JoinLink::Connector,
            },
            potentials: PotentialRules {
                from_terminal_names: true,
            },
            generated_pages_off: BTreeSet::new(),
            standard: String::new(),
        }
    }

    /// Whether `id` can name a drawing standard: empty (IEC) or lowercase
    /// letters, digits, `_` and `-`.
    pub fn is_standard_id(id: &str) -> bool {
        id.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    }

    pub fn is_default(&self) -> bool {
        *self == ProjectRules::default()
    }

    /// Whether the project wants generated pages from `generator`.
    pub fn wants_generated(&self, generator: &str) -> bool {
        !self.generated_pages_off.contains(generator)
    }
}

/// What terminals with the same name and level on one strip (two `-X2:1`)
/// mean. See [`Project::terminal_designation_groups`](crate::Project::terminal_designation_groups)
/// for the terminals a rule applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct TerminalRules {
    #[serde(default)]
    pub same_name: SameName,
    /// How the clamps of a joined terminal are linked (bill of materials,
    /// terminal diagrams). Only meaningful with [`SameName::Join`].
    #[serde(default)]
    pub join_link: JoinLink,
}

impl TerminalRules {
    pub fn is_off(&self) -> bool {
        *self == TerminalRules::default()
    }
}

/// Rules for where potentials come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct PotentialRules {
    /// A terminal named like a supply potential (`-X4:PE`, `-X1:N`, `PEN`)
    /// defines that potential on its net.
    #[serde(default)]
    pub from_terminal_names: bool,
}

impl PotentialRules {
    pub fn is_off(&self) -> bool {
        *self == PotentialRules::default()
    }
}

/// How terminals that share a name are treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SameName {
    /// No rule: separate terminals that happen to share a name, listed as a
    /// design check finding. What files without a rule load as.
    #[default]
    Off,
    /// One electrical point built from several clamps (1-in/1-out clamps
    /// linked by [`JoinLink`]): the netlist joins them like a jumper.
    Join,
    /// A second terminal with a name already on the strip is refused.
    Refuse,
}

/// What links the clamps of a joined terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinLink {
    #[default]
    Connector,
    Jumper,
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
    /// The structure the page belongs to: function (`=A2`) and location
    /// (`+CAB1`) levels only, never a product level. Pages sort by structure
    /// first, then by name, and cross references to a page with another
    /// structure carry it as a prefix (`=A2/3.4`). Empty for a page outside
    /// any structure.
    #[serde(default, skip_serializing_if = "DeviceTag::is_empty")]
    pub structure: DeviceTag,
    /// Set when the page was issued (a revision or an export): references
    /// to it keep the page and column printed then, so a printed set never
    /// changes under its readers (pinned flow, DESIGN.md section 8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen: Option<PageFreeze>,
    #[serde(default)]
    pub properties: Properties,
}

/// What a page looked like when it was issued.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageFreeze {
    /// The revision or export it was issued with, e.g. `B` or `2026-10-14`.
    pub revision: String,
    /// The page's name and structure at that point.
    pub name: String,
    #[serde(default, skip_serializing_if = "DeviceTag::is_empty")]
    pub structure: DeviceTag,
    /// The grid column of each symbol on the page at that point, sorted by
    /// symbol (`[symbol, column]` pairs in JSON). Symbols added later are
    /// referenced where they are.
    #[serde(default)]
    pub columns: Vec<(SymbolId, u16)>,
}

impl PageFreeze {
    /// The column a symbol was printed in, if it was on the page then.
    pub fn column_of(&self, symbol: SymbolId) -> Option<u16> {
        self.columns
            .binary_search_by_key(&symbol, |&(s, _)| s)
            .ok()
            .map(|i| self.columns[i].1)
    }
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
    /// A spare point that may stay unconnected (a second PE clamp, an
    /// unused auxiliary contact), copied from the symbol library. Design
    /// checks do not report it as open.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

impl Pin {
    pub fn new(name: impl Into<String>, x: f64, y: f64) -> Self {
        Pin {
            name: name.into(),
            offset: Point::new(x, y),
            direction: None,
            optional: false,
        }
    }

    /// The same pin marked optional, builder style.
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
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
    /// Pins that define a potential on their net, sorted by pin.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feeds: Vec<PinFeed>,
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Endpoint {
    Pin {
        symbol: SymbolId,
        pin: u16,
    },
    Junction {
        junction: JunctionId,
    },
    /// An open end: a free position on the wire's page, connected to
    /// nothing (DESIGN §4 "Open ends"). The wire's other end still
    /// connects; an open end joins no net and makes no connection, and
    /// checks report each one as `wire.open`. Moving it is an ordinary
    /// `update` of the wire; connecting it replaces it with a pin or
    /// junction endpoint.
    Free {
        at: Point,
    },
}

impl Eq for Endpoint {}

impl PartialOrd for Endpoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Pins first, then junctions, then open ends by position.
impl Ord for Endpoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use Endpoint::*;
        let rank = |e: &Endpoint| match e {
            Pin { .. } => 0,
            Junction { .. } => 1,
            Free { .. } => 2,
        };
        match (self, other) {
            (Pin { symbol: a, pin: p }, Pin { symbol: b, pin: q }) => (a, p).cmp(&(b, q)),
            (Junction { junction: a }, Junction { junction: b }) => a.cmp(b),
            (Free { at: a }, Free { at: b }) => {
                a.x.total_cmp(&b.x).then_with(|| a.y.total_cmp(&b.y))
            }
            _ => rank(self).cmp(&rank(other)),
        }
    }
}

impl std::hash::Hash for Endpoint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Endpoint::Pin { symbol, pin } => (symbol, pin).hash(state),
            Endpoint::Junction { junction } => junction.hash(state),
            // `0.0 == -0.0`, so hash them alike.
            Endpoint::Free { at } => {
                (at.x + 0.0).to_bits().hash(state);
                (at.y + 0.0).to_bits().hash(state);
            }
        }
    }
}

impl Endpoint {
    pub fn pin(symbol: SymbolId, pin: u16) -> Self {
        Endpoint::Pin { symbol, pin }
    }

    pub fn junction(junction: JunctionId) -> Self {
        Endpoint::Junction { junction }
    }

    pub fn free(at: Point) -> Self {
        Endpoint::Free { at }
    }

    pub fn as_pin(self) -> Option<PinRef> {
        match self {
            Endpoint::Pin { symbol, pin } => Some(PinRef { symbol, pin }),
            Endpoint::Junction { .. } | Endpoint::Free { .. } => None,
        }
    }

    /// Whether this is an open end.
    pub fn is_free(self) -> bool {
        matches!(self, Endpoint::Free { .. })
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

/// A text on a page: free text, or a label that shows a value of the object
/// it is attached to (a symbol's device tag, a wire's number) and moves with
/// it. Labels are records of their own so they can be moved and rotated
/// independently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub id: TextId,
    pub page: PageId,
    /// The symbol, junction or wire this text belongs to, on the same page.
    /// The text moves with it and is deleted with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_to: Option<AnyId>,
    pub content: TextContent,
    #[serde(default)]
    pub placement: TextPlacement,
    /// Character height in mm.
    #[serde(default = "default_text_height")]
    pub height_mm: f64,
    #[serde(default)]
    pub properties: Properties,
}

fn default_text_height() -> f64 {
    Text::DEFAULT_HEIGHT_MM
}

impl Text {
    pub const DEFAULT_HEIGHT_MM: f64 = 2.5;
}

/// What a text shows. Everything but `Literal` is read from the object the
/// text is attached to, so it never goes stale; see
/// [`Project::text_string`](crate::Project::text_string).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TextContent {
    Literal {
        text: String,
    },
    /// The device tag of the attached symbol's device (`-K1`), or the
    /// designation of its terminal (`-X1:3`).
    DeviceTag,
    /// The attached wire's number.
    WireNumber,
    /// The name of the potential the attached wire defines.
    Potential,
    /// A property of the attached object, or of its device for a symbol.
    Property {
        key: String,
    },
}

/// Where a text sits: an offset from its attached object's origin (or from
/// the page origin when free), a rotation, and which point of the text box
/// the offset names. The offset does not turn with a rotated symbol, so
/// labels stay readable.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct TextPlacement {
    pub offset: Point,
    #[serde(default)]
    pub rotation: Rotation,
    #[serde(default)]
    pub anchor: TextAnchor,
}

/// The point of a text box that sits at the text's position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAnchor {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    #[default]
    BottomLeft,
    BottomCenter,
    BottomRight,
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
    /// Whether the user placed the bend points (Lines mode, or Corners
    /// with clicked corners) rather than the router. `tidy` keeps a
    /// hand-drawn route unless it is selected, and moving a symbol moves
    /// only its end segments (DESIGN §4 "Wiring modes"). The model only
    /// stores the mark; the editor sets and clears it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hand_drawn: bool,
    /// Wire number, when numbered.
    #[serde(default)]
    pub number: Option<String>,
    /// Colour code, e.g. `BK`, `BU`, `GNYE`.
    #[serde(default)]
    pub color: Option<String>,
    /// Cross-section in mm².
    #[serde(default)]
    pub cross_section_mm2: Option<f64>,
    /// A potential this wire defines on its net, like a potential definition
    /// point: select three wires and assign L1, L2, L3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potential: Option<PotentialId>,
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
            hand_drawn: false,
            number: None,
            color: None,
            cross_section_mm2: None,
            potential: None,
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
    /// Combined protective earth and neutral (TN-C).
    Pen,
    DcPositive,
    DcNegative,
    #[default]
    Signal,
}

impl PotentialKind {
    /// Supply potentials: everything but `Signal`. A terminal named like a
    /// supply potential (`-X4:PE`, `-X1:N`) defines it on its net.
    pub fn is_supply(self) -> bool {
        self != PotentialKind::Signal
    }
}

/// A potential by name, kind and voltage, for assigning potentials without
/// looking up ids first: an existing potential with the same name is
/// reused, otherwise one is created.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PotentialSpec {
    pub name: String,
    #[serde(default)]
    pub kind: PotentialKind,
    #[serde(default)]
    pub voltage: Option<f64>,
}

impl PotentialSpec {
    pub fn new(name: impl Into<String>, kind: PotentialKind, voltage: Option<f64>) -> Self {
        PotentialSpec {
            name: name.into(),
            kind,
            voltage,
        }
    }
}

/// Common sets of supply potentials, in the order wires are usually drawn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PotentialSet {
    /// L1, L2, L3, then N and PE as asked (TN-S: both; TN-C: use `pen`).
    ThreePhase {
        /// Line-to-line voltage, e.g. 400.
        voltage: f64,
        #[serde(default)]
        neutral: bool,
        #[serde(default)]
        pe: bool,
        /// A combined PEN conductor instead of separate N and PE.
        #[serde(default)]
        pen: bool,
    },
    /// L, N, PE.
    SinglePhase { voltage: f64 },
    /// L+ and L- (e.g. 24 V control voltage).
    Dc { voltage: f64 },
}

impl PotentialSet {
    /// The set's voltage: line-to-line for three-phase.
    pub fn voltage(self) -> f64 {
        match self {
            PotentialSet::ThreePhase { voltage, .. }
            | PotentialSet::SinglePhase { voltage }
            | PotentialSet::Dc { voltage } => voltage,
        }
    }

    pub fn specs(self) -> Vec<PotentialSpec> {
        use PotentialKind::*;
        let spec = PotentialSpec::new;
        match self {
            PotentialSet::ThreePhase {
                voltage,
                neutral,
                pe,
                pen,
            } => {
                let mut specs: Vec<PotentialSpec> = ["L1", "L2", "L3"]
                    .into_iter()
                    .map(|name| spec(name, Line, Some(voltage)))
                    .collect();
                if pen {
                    specs.push(spec("PEN", Pen, None));
                } else {
                    if neutral {
                        specs.push(spec("N", Neutral, None));
                    }
                    if pe {
                        specs.push(spec("PE", ProtectiveEarth, None));
                    }
                }
                specs
            }
            PotentialSet::SinglePhase { voltage } => vec![
                spec("L", Line, Some(voltage)),
                spec("N", Neutral, None),
                spec("PE", ProtectiveEarth, None),
            ],
            PotentialSet::Dc { voltage } => vec![
                spec("L+", DcPositive, Some(voltage)),
                spec("L-", DcNegative, None),
            ],
        }
    }
}

/// A supply system: the potentials that belong together (L1, L2, L3, N,
/// PE) with the system's voltage, declared once at its source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PotentialSystem {
    pub id: PotentialSystemId,
    /// E.g. `400 V 3/N/PE AC`.
    pub name: String,
    /// Line-to-line voltage in volts (DC: the supply voltage).
    #[serde(default)]
    pub voltage: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_hz: Option<f64>,
    /// The system's potentials in conductor order (L1, L2, L3, N, PE).
    #[serde(default)]
    pub members: Vec<PotentialId>,
    #[serde(default)]
    pub properties: Properties,
}

/// A node of the project structure: a plant section (`=A1`) or a location
/// (`+CAB1`), with what it is. Devices and pages refer to it by tag, so a
/// structure node is optional description, never required to use a tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Structure {
    pub id: StructureId,
    /// Function and location aspects only, e.g. `=A1` or `+CAB1`. Unique.
    pub tag: DeviceTag,
    #[serde(default)]
    pub description: String,
    /// The location is an enclosure (cabinet, junction box): the internal
    /// side of a terminal strip faces it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub enclosure: bool,
    /// The mounting plate of an enclosure, which devices are placed on
    /// (cabinet layout). `None` until its size is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plate: Option<MountingPlate>,
    #[serde(default)]
    pub properties: Properties,
}

/// The mounting plate of an enclosure, in millimetres with the origin at its
/// top left, x to the right and y down.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MountingPlate {
    pub width: f64,
    pub height: f64,
    /// The border where nothing is placed.
    #[serde(default)]
    pub margin: f64,
}

/// What a [`Rail`] is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RailKind {
    #[default]
    DinRail,
    Duct,
}

/// A DIN rail or wiring duct on the mounting plate of an enclosure. It runs
/// from `(x, y)`, its start, for `length` millimetres: to the right, or down
/// when `vertical`. A duct's `width` and `height` are its cross-section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rail {
    pub id: RailId,
    /// The enclosure whose plate it is on.
    pub structure: StructureId,
    pub x: f64,
    pub y: f64,
    pub length: f64,
    #[serde(default)]
    pub kind: RailKind,
    /// Runs down from the start instead of to the right.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vertical: bool,
    /// Cross-section of a duct in millimetres; 0 while unknown.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub width: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub height: f64,
    #[serde(default)]
    pub properties: Properties,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// Where a device sits on the mounting plate of an enclosure. A device has
/// at most one. Deleting the device, the enclosure or the rail deletes
/// the placement, except that deleting the rail leaves the device on the
/// plate, off any rail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CabinetPlacement {
    pub id: CabinetPlacementId,
    pub device: DeviceId,
    pub structure: StructureId,
    /// The top left of the footprint on the plate.
    pub x: f64,
    pub y: f64,
    /// Turned by 90 degrees, so the footprint swaps width and height.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub rotated: bool,
    /// The rail the device is on, and its offset along it; the offset is
    /// only set with a rail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rail: Option<RailId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rail_offset: Option<f64>,
    #[serde(default)]
    pub properties: Properties,
}

/// A proposal the user declined (a terminal suggestion, a connection
/// between facing pins), so it is not proposed again. It goes away with
/// any of its subjects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dismissal {
    pub id: DismissalId,
    /// Which kind of proposal, named by the engine that makes it, e.g.
    /// `terminal.suggest`.
    pub proposal: String,
    /// The objects the proposal was about, sorted.
    pub subjects: Vec<AnyId>,
    #[serde(default)]
    pub properties: Properties,
}

/// A parametric circuit kept in the project (DESIGN §7): devices, symbols,
/// junctions, wires and texts captured from a drawing, placed again as
/// [`BlockInstance`]s. The members are records with the ids they had when
/// captured, local to the definition (never look them up in the project);
/// they sit on no page (page id 0) and their positions are
/// relative to the block origin. Device properties, descriptions and
/// literal texts may name a parameter as `{name}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockDefinition {
    pub id: BlockDefinitionId,
    /// Unique within the project, e.g. `dol`.
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub parameters: Vec<BlockParameter>,
    /// The template records: devices, symbols, junctions, wires and texts.
    pub members: Vec<crate::record::Record>,
    /// Where the definition was copied from, so a newer library version can
    /// be offered as an update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<BlockSource>,
    #[serde(default)]
    pub properties: Properties,
}

/// A value the user sets per placement, e.g. `power` = 4.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockParameter {
    pub name: String,
    /// Used when a placement does not set it.
    pub default: Value,
    #[serde(default)]
    pub description: String,
}

/// The library a definition was copied from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockSource {
    /// The library file or folder, as the user's library list names it.
    pub library: String,
    pub version: u32,
}

/// One placement of a [`BlockDefinition`]: which drawn object came from
/// which template member, the parameter values, and what the user changed
/// by hand. Deleting a member detaches the instance; the drawing stays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockInstance {
    pub id: BlockInstanceId,
    pub definition: BlockDefinitionId,
    /// The page it was placed on. Members moved later with
    /// [`Command::MoveToPage`](crate::Command::MoveToPage) may be elsewhere.
    pub page: PageId,
    /// Where the definition's origin was placed.
    pub origin: Point,
    /// Parameter values set for this placement, sorted by name; a parameter
    /// not listed takes its default.
    #[serde(default)]
    pub values: Vec<(String, Value)>,
    pub members: Vec<BlockMember>,
    /// Why the instance no longer follows its definition, after a
    /// structural edit (a member deleted); `None` while it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detached: Option<String>,
    #[serde(default)]
    pub properties: Properties,
}

/// An issued revision of the drawing set (DESIGN §7, Revisions): the
/// index printed in title blocks, its date, who issued it and why. Issuing
/// one freezes the schematic pages ([`Tx::issue_revision`](crate::Tx::issue_revision)).
/// The snapshot of the project at that point is kept by storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub id: RevisionId,
    /// Unique, e.g. `A`, `B` or `01`.
    pub index: String,
    /// ISO date, `2026-10-15`.
    pub date: String,
    /// Initials or name of whoever issued it.
    #[serde(default)]
    pub author: String,
    /// One line, e.g. "Motor M3 added".
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub properties: Properties,
}

impl Revision {
    /// The longer note on why it was issued, or an empty string.
    pub fn note(&self) -> &str {
        match self.properties.get("note") {
            Some(Value::Text(note)) => note,
            _ => "",
        }
    }
}

impl BlockInstance {
    /// The value of a parameter for this placement, if set here.
    pub fn value(&self, name: &str) -> Option<&Value> {
        self.values
            .binary_search_by(|(n, _)| n.as_str().cmp(name))
            .ok()
            .map(|i| &self.values[i].1)
    }

    /// The member record for a drawn object.
    pub fn member(&self, actual: AnyId) -> Option<&BlockMember> {
        self.members.iter().find(|m| m.actual == actual)
    }
}

/// A drawn object that came from a template member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockMember {
    /// The member's id inside the definition.
    pub template: AnyId,
    /// The object in the project.
    pub actual: AnyId,
    /// Fields the user changed by hand; a parameter edit or a definition
    /// update keeps them. Sorted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<BlockOverride>,
}

/// A hand edit on a block member that survives updates (DESIGN §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockOverride {
    /// Moved: position relative to the instance origin.
    Position,
    /// A label moved or turned.
    TextPlacement,
    /// The tag changed by hand; renumbering and updates leave it.
    Tag,
    /// A connection re-routed.
    Route,
    /// A property or description edited by hand; parameter edits leave it.
    Value,
}

/// A pin that defines a potential: the output of a supply, transformer or
/// power supply, so a source is not a separate feed symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PinFeed {
    pub pin: u16,
    pub potential: PotentialId,
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

/// Who decides that a terminal is a PE terminal. A terminal becomes one by
/// rule when it is connected to a wire of PE potential or to a ground or PE
/// symbol; a hand toggle is an override that the rule never undoes (DESIGN
/// "PE terminals"). The model only stores the choice: whoever applies the
/// rule (novo-terminals) leaves a terminal alone unless this is `Auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeOverride {
    /// The rule decides.
    #[default]
    Auto,
    /// Set by hand: a PE terminal whatever it is connected to.
    ForcedPe,
    /// Set by hand: not a PE terminal whatever it is connected to.
    ForcedNotPe,
}

impl PeOverride {
    pub fn is_auto(&self) -> bool {
        *self == PeOverride::Auto
    }
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
    /// The terminal's article. Terminals of one type on a strip usually
    /// share one part; the strip device itself may carry the end plates
    /// and accessories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<PartId>,
    /// A potential this terminal feeds into its net (an incoming supply
    /// terminal), declared explicitly rather than read from its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed: Option<PotentialId>,
    /// Whether a hand toggle fixed this as a PE terminal (or not), so the
    /// automatic rule leaves `kind` alone; shown as an override (diamond).
    #[serde(default, skip_serializing_if = "PeOverride::is_auto")]
    pub pe_override: PeOverride,
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
