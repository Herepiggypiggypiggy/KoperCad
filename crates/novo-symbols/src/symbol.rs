//! The symbol definition format (format version 1).
//!
//! One symbol is one TOML file. The full field reference, with the drawing
//! conventions, is in `crates/novo-symbols/FORMAT.md`.

use serde::{Deserialize, Serialize};

/// The format version this crate reads and writes.
pub const FORMAT_VERSION: u32 = 1;

/// The connection grid in millimetres. Every connection point lies on it, so
/// symbols placed on the sheet grid connect to orthogonal wires exactly.
pub const GRID: f64 = 4.0;

/// Behavior kinds whose `poles` always conduct, so they are bridges.
pub const ALWAYS_CLOSED: &[&str] = &["conductor", "overload"];

/// Behavior kinds whose `poles` are switched paths that carry a potential
/// through when closed.
pub const PASS_THROUGH: &[&str] = &["contact", "breaker", "fuse"];

/// Groups of connection indices a potential passes through, from
/// [`Symbol::pass_through`]. Deliberately not a plain `Vec` so it can't be
/// stored as netlist bridges by accident. Each group is a path; propagate a
/// potential outward from the side it arrives on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PassThrough(Vec<Vec<usize>>);

impl PassThrough {
    /// The paths as connection-index groups.
    pub fn paths(&self) -> &[Vec<usize>] {
        &self.0
    }
}

/// A point in symbol coordinates: millimetres, `[x, y]`, x to the right and
/// y downwards, relative to the symbol origin (its insertion point).
pub type Point = [f64; 2];

/// One schematic symbol: artwork, connection points, text anchors, the
/// function it represents and the behavior slot used by simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Symbol {
    /// Format version, currently always [`FORMAT_VERSION`].
    pub format: u32,
    /// Stable library id, equal to the file path below `symbols/` without
    /// the `.toml` extension, for example `iec/contacts/no`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// One or two sentences on what the symbol is and when to use it.
    #[serde(default)]
    pub description: String,
    /// Standard the artwork follows, for example `IEC 60617`.
    pub standard: String,
    /// Library category; equal to the directory the file sits in.
    pub category: String,
    /// Function type: what the device does, independent of artwork, for
    /// example `contact.no` or `coil.relay`. Used for cross references,
    /// design checks and to pick a simulation model.
    pub function: String,
    /// Search words for the symbol picker and command palette.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Short names the command palette matches exactly, such as `no`, `coil`
    /// or `mcb3`. Unique within a library.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Offered in the palette before anything is typed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub favorite: bool,
    /// Electrical connection points, in their default order.
    pub connections: Vec<Connection>,
    /// Where texts such as the device tag are placed.
    #[serde(default)]
    pub anchors: Vec<Anchor>,
    /// Groups of connection designations that are always joined inside the
    /// symbol whatever its behavior, such as the PE in and out of a drive.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bridges: Vec<Vec<String>>,
    /// The artwork, drawn in order.
    pub graphics: Vec<Graphic>,
    /// The behavior slot read by the simulation engine.
    pub behavior: Behavior,
}

/// An electrical connection point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    /// Default connection designation printed next to the point, for
    /// example `13`, `A1` or `U1` (IEC 61666 / EN 50005 numbering).
    /// Unique within the symbol.
    pub id: String,
    /// Position on the connection grid.
    pub at: Point,
    /// Direction a wire leaves the point.
    pub dir: Direction,
    /// Optional special role of the point, such as `pe` or `n`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Direction a wire leaves a connection point (y is downwards).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    /// Unit vector of the direction in symbol coordinates.
    pub fn vector(self) -> Point {
        match self {
            Direction::Up => [0.0, -1.0],
            Direction::Down => [0.0, 1.0],
            Direction::Left => [-1.0, 0.0],
            Direction::Right => [1.0, 0.0],
        }
    }
}

/// A placement point for a text that belongs to the symbol instance rather
/// than to the artwork, such as the device tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    /// What is placed here: `tag`, `function_text`, `address` or `part`.
    pub role: String,
    /// Anchor point; the text is vertically centred on it.
    pub at: Point,
    /// Horizontal alignment of the text relative to the point.
    #[serde(default)]
    pub align: Align,
}

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Line style of a stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stroke {
    #[default]
    Solid,
    /// Mechanical links and other dashed lines.
    Dashed,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

fn default_text_size() -> f64 {
    2.5
}

fn default_text_align() -> Align {
    Align::Center
}

/// One drawing primitive. Stroke width is the renderer's choice, so the
/// artwork carries geometry only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Graphic {
    /// A straight line.
    Line {
        from: Point,
        to: Point,
        #[serde(default, skip_serializing_if = "is_default")]
        stroke: Stroke,
    },
    /// Connected straight segments, optionally closed and filled.
    Polyline {
        points: Vec<Point>,
        #[serde(default, skip_serializing_if = "is_default")]
        closed: bool,
        #[serde(default, skip_serializing_if = "is_default")]
        fill: bool,
        #[serde(default, skip_serializing_if = "is_default")]
        stroke: Stroke,
    },
    /// An axis-aligned rectangle; `at` is the top-left corner.
    Rect {
        at: Point,
        size: [f64; 2],
        #[serde(default, skip_serializing_if = "is_default")]
        fill: bool,
        #[serde(default, skip_serializing_if = "is_default")]
        stroke: Stroke,
    },
    /// A circle.
    Circle {
        center: Point,
        r: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        fill: bool,
        #[serde(default, skip_serializing_if = "is_default")]
        stroke: Stroke,
    },
    /// An axis-aligned ellipse.
    Ellipse {
        center: Point,
        rx: f64,
        ry: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        fill: bool,
        #[serde(default, skip_serializing_if = "is_default")]
        stroke: Stroke,
    },
    /// A circular arc from `start` to `end` degrees, with `end > start`.
    /// A point at angle `a` is `center + r * (cos a, sin a)`; because y is
    /// downwards, increasing angles run clockwise on screen.
    Arc {
        center: Point,
        r: f64,
        start: f64,
        end: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        stroke: Stroke,
    },
    /// Fixed text that is part of the artwork, such as `M` in a motor.
    Text {
        at: Point,
        text: String,
        /// Character height in millimetres.
        #[serde(default = "default_text_size")]
        size: f64,
        #[serde(default = "default_text_align")]
        align: Align,
    },
}

/// The simulation behavior slot. `kind` selects the model; the remaining
/// keys are model parameters, kept as data so the simulation workstream can
/// grow them without a format change. See `FORMAT.md` for the vocabulary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Behavior {
    pub kind: String,
    #[serde(flatten)]
    pub params: toml::Table,
}

impl Behavior {
    /// The `poles` parameter: conducting paths between connections, as
    /// designations. Empty when absent or malformed (the validator reports
    /// malformed values).
    pub fn poles(&self) -> Vec<Vec<&str>> {
        self.groups("poles")
    }

    /// The `ports` parameter: connections that together form one winding
    /// or port of a motor, transformer or converter.
    pub fn ports(&self) -> Vec<Vec<&str>> {
        self.groups("ports")
    }

    fn groups(&self, key: &str) -> Vec<Vec<&str>> {
        self.params
            .get(key)
            .and_then(|p| p.as_array())
            .map(|groups| {
                groups
                    .iter()
                    .filter_map(|g| g.as_array())
                    .map(|g| g.iter().filter_map(|v| v.as_str()).collect())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Symbol {
    /// Parses one symbol from TOML text.
    pub fn from_toml(text: &str) -> Result<Symbol, toml::de::Error> {
        toml::from_str(text)
    }

    /// Looks up a connection point by its designation.
    pub fn connection(&self, id: &str) -> Option<&Connection> {
        self.connections.iter().find(|c| c.id == id)
    }

    /// Groups of connection indices that are always joined inside the
    /// symbol: the `poles` of kinds that never open ([`ALWAYS_CLOSED`]:
    /// terminals, connectors, the heaters of an overload relay) plus the
    /// top-level `bridges`. Switched paths (contacts, breakers, fuses) are
    /// not bridges because whether they conduct is simulation state. Maps to
    /// `SymbolInstance::bridges` in `novo-core`.
    pub fn bridges(&self) -> Vec<Vec<usize>> {
        let always = if ALWAYS_CLOSED.contains(&self.behavior.kind.as_str()) {
            self.behavior.poles()
        } else {
            Vec::new()
        };
        let declared = self
            .bridges
            .iter()
            .map(|g| g.iter().map(String::as_str).collect::<Vec<_>>());
        self.indices(always.into_iter().chain(declared))
    }

    /// Paths a potential (L1, N, 24V, ...) and its naming pass through:
    /// the [`Symbol::bridges`] plus the `poles` of switching devices
    /// ([`PASS_THROUGH`]: contacts, breakers, fuses), as in a main circuit
    /// where L1 stays L1 through the fuse, breaker, contactor and overload.
    /// Loads (coils, lamps, motors) end a potential and are not included.
    ///
    /// Never store this in `SymbolInstance::bridges`: it only propagates
    /// potential names and must not take part in netlist connectivity or
    /// potential conflict checks, or every open switch would join two nets.
    /// The [`PassThrough`] newtype keeps it from being passed by mistake.
    pub fn pass_through(&self) -> PassThrough {
        let mut groups = self.bridges();
        if PASS_THROUGH.contains(&self.behavior.kind.as_str()) {
            // A changeover [common, break, make] becomes two paths from the
            // common side, so the break and make sides are never joined.
            let paths = self.behavior.poles().into_iter().flat_map(|g| match g[..] {
                [common, brk, make] => vec![vec![common, brk], vec![common, make]],
                _ => vec![g],
            });
            groups.extend(self.indices(paths));
        }
        PassThrough(groups)
    }

    fn indices<'a>(&self, groups: impl Iterator<Item = Vec<&'a str>>) -> Vec<Vec<usize>> {
        groups
            .map(|group| {
                group
                    .iter()
                    .filter_map(|id| self.connections.iter().position(|c| c.id == *id))
                    .collect::<Vec<_>>()
            })
            .filter(|group| group.len() >= 2)
            .collect()
    }

    /// Looks up the first anchor with the given role.
    pub fn anchor(&self, role: &str) -> Option<&Anchor> {
        self.anchors.iter().find(|a| a.role == role)
    }

    /// Bounding box of the artwork and connection points as
    /// `(min, max)`. Text extents are estimated.
    pub fn bounds(&self) -> (Point, Point) {
        let mut b = Bounds::default();
        for c in &self.connections {
            b.add(c.at);
        }
        for g in &self.graphics {
            match g {
                Graphic::Line { from, to, .. } => {
                    b.add(*from);
                    b.add(*to);
                }
                Graphic::Polyline { points, .. } => points.iter().for_each(|p| b.add(*p)),
                Graphic::Rect { at, size, .. } => {
                    b.add(*at);
                    b.add([at[0] + size[0], at[1] + size[1]]);
                }
                Graphic::Circle { center, r, .. } => {
                    b.add([center[0] - r, center[1] - r]);
                    b.add([center[0] + r, center[1] + r]);
                }
                Graphic::Ellipse { center, rx, ry, .. } => {
                    b.add([center[0] - rx, center[1] - ry]);
                    b.add([center[0] + rx, center[1] + ry]);
                }
                Graphic::Arc {
                    center,
                    r,
                    start,
                    end,
                    ..
                } => {
                    const STEPS: usize = 32;
                    for i in 0..=STEPS {
                        let a = (start + (end - start) * i as f64 / STEPS as f64).to_radians();
                        b.add([center[0] + r * a.cos(), center[1] + r * a.sin()]);
                    }
                }
                Graphic::Text {
                    at,
                    text,
                    size,
                    align,
                } => {
                    let w = text.chars().count() as f64 * size * 0.6;
                    let x0 = match align {
                        Align::Left => at[0],
                        Align::Center => at[0] - w / 2.0,
                        Align::Right => at[0] - w,
                    };
                    b.add([x0, at[1] - size / 2.0]);
                    b.add([x0 + w, at[1] + size / 2.0]);
                }
            }
        }
        (b.min, b.max)
    }
}

struct Bounds {
    min: Point,
    max: Point,
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            min: [f64::INFINITY; 2],
            max: [f64::NEG_INFINITY; 2],
        }
    }
}

impl Bounds {
    fn add(&mut self, p: Point) {
        for (i, v) in p.into_iter().enumerate() {
            self.min[i] = self.min[i].min(v);
            self.max[i] = self.max[i].max(v);
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
