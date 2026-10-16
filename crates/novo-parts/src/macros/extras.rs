//! Macro library extras: the `parts` field of a macro file, the picture a
//! browser shows for a macro, and the update check against a library.
//!
//! All of it is pure: it reads macro files and block definitions and never
//! touches a project or the screen.

use std::collections::BTreeMap;

use novo_core::{BlockDefinition, BlockSource, Endpoint, Placement, Point, Record, SymbolId};
use serde_json::{Value, json};

use super::{MACRO_BASE_VERSION, MacroFile, MacroLibrary};

/// The macro file's field that carries parts.
const PARTS_KEY: &str = "parts";
/// The format version that introduced [`PARTS_KEY`].
const PARTS_VERSION: u32 = 2;

/// One part a macro carries: the part assigned to a device of the circuit.
/// Placing the macro assigns it again, adding the part to the project when
/// it is not there yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroPart {
    /// The device in the macro's definition (its template id).
    pub device: u64,
    pub manufacturer: String,
    pub order_number: String,
    pub type_number: String,
    pub description: String,
}

impl MacroFile {
    /// The parts the file carries, in file order. Entries that are not
    /// understood (no device or no order number) are skipped, so a later
    /// version's extra fields never break reading.
    pub fn parts(&self) -> Vec<MacroPart> {
        let text = |e: &Value, key: &str| {
            e.get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        self.extra
            .get(PARTS_KEY)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let device = e.get("device")?.as_u64()?;
                let order_number = text(e, "order_number");
                (!order_number.is_empty()).then(|| MacroPart {
                    device,
                    manufacturer: text(e, "manufacturer"),
                    order_number,
                    type_number: text(e, "type_number"),
                    description: text(e, "description"),
                })
            })
            .collect()
    }

    /// Replaces the carried parts. A file with parts is written as format
    /// version 2; clearing them again returns a file to version 1 so older
    /// builds can read it.
    pub fn set_parts(&mut self, parts: &[MacroPart]) {
        if parts.is_empty() {
            self.extra.remove(PARTS_KEY);
            self.version = MACRO_BASE_VERSION;
            return;
        }
        let list: Vec<Value> = parts
            .iter()
            .map(|p| {
                json!({
                    "device": p.device,
                    "manufacturer": p.manufacturer,
                    "order_number": p.order_number,
                    "type_number": p.type_number,
                    "description": p.description,
                })
            })
            .collect();
        self.extra.insert(PARTS_KEY.into(), Value::Array(list));
        self.version = self.version.max(PARTS_VERSION);
    }
}

/// One symbol of a [`Thumbnail`]: which library symbol, and where it sits.
#[derive(Debug, Clone, PartialEq)]
pub struct ThumbSymbol {
    /// The symbol key, e.g. `contactor.coil`; the caller draws it from its
    /// symbol library, or as a box when the library does not know it.
    pub key: String,
    pub placement: Placement,
    /// Pin positions on the thumbnail's plane (the fallback box is drawn
    /// around them).
    pub pins: Vec<Point>,
}

/// Wire, junction and symbol geometry of a macro's members on one plane
/// (millimetres, block origin at 0,0), ready to scale into a preview.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Thumbnail {
    pub symbols: Vec<ThumbSymbol>,
    pub wires: Vec<Vec<Point>>,
    pub junctions: Vec<Point>,
    /// The box around everything, `(min, max)`, symbols padded by
    /// [`SYMBOL_PAD`] so artwork is not clipped. `None` for an empty macro.
    pub bounds: Option<(Point, Point)>,
}

/// Padding around a symbol's origin and pins in [`Thumbnail::bounds`], mm.
pub const SYMBOL_PAD: f64 = 6.0;

/// Builds the picture data from a macro's members. Wire ends that name a
/// symbol or junction the macro does not contain are skipped.
pub fn thumbnail(def: &BlockDefinition) -> Thumbnail {
    let mut out = Thumbnail::default();
    let mut symbols: BTreeMap<SymbolId, usize> = BTreeMap::new();
    let mut junctions = BTreeMap::new();
    for member in &def.members {
        match member {
            Record::Symbol(s) => {
                symbols.insert(s.id, out.symbols.len());
                out.symbols.push(ThumbSymbol {
                    key: s.symbol.0.clone(),
                    placement: s.placement,
                    pins: (0..s.pins.len())
                        .filter_map(|i| s.pin_position(i as u16))
                        .collect(),
                });
            }
            Record::Junction(j) => {
                junctions.insert(j.id, j.position);
                out.junctions.push(j.position);
            }
            _ => {}
        }
    }
    let end = |e: &Endpoint| -> Option<Point> {
        match e {
            Endpoint::Pin { symbol, pin } => {
                let sym = def.members.iter().find_map(|m| match m {
                    Record::Symbol(s) if s.id == *symbol => Some(s),
                    _ => None,
                })?;
                sym.pin_position(*pin)
            }
            Endpoint::Junction { junction } => junctions.get(junction).copied(),
            Endpoint::Free { at } => Some(*at),
        }
    };
    for member in &def.members {
        if let Record::Wire(w) = member
            && let (Some(a), Some(b)) = (end(&w.from), end(&w.to))
        {
            let mut line = vec![a];
            line.extend(w.route.iter().copied());
            line.push(b);
            out.wires.push(line);
        }
    }
    let mut min = Point::new(f64::MAX, f64::MAX);
    let mut max = Point::new(f64::MIN, f64::MIN);
    let mut any = false;
    let mut add = |p: Point, pad: f64| {
        any = true;
        min = Point::new(min.x.min(p.x - pad), min.y.min(p.y - pad));
        max = Point::new(max.x.max(p.x + pad), max.y.max(p.y + pad));
    };
    for s in &out.symbols {
        add(s.placement.origin, SYMBOL_PAD);
        s.pins.iter().for_each(|p| add(*p, SYMBOL_PAD));
    }
    out.wires.iter().flatten().for_each(|p| add(*p, 0.0));
    out.junctions.iter().for_each(|p| add(*p, 0.0));
    out.bounds = any.then_some((min, max));
    out
}

/// A macro of the library that is newer than the copy in a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUpdate {
    /// The project's block definition name.
    pub definition: String,
    /// The library path the definition came from.
    pub path: String,
    /// The revision the project holds.
    pub project_revision: u32,
    /// The revision in the library.
    pub library_revision: u32,
}

/// Compares the sources of a project's block definitions (name and
/// [`BlockSource`]) with `library`: the ones whose file now has a higher
/// revision. A source naming another library, or a file that is gone or
/// unreadable, is skipped; an offer is only ever made for a file that can be
/// read now. Sorted by definition name.
pub fn macro_updates<'a>(
    library: &MacroLibrary,
    sources: impl IntoIterator<Item = (&'a str, &'a BlockSource)>,
) -> Vec<MacroUpdate> {
    let prefix = format!("{}/", library.label());
    let mut out: Vec<MacroUpdate> = sources
        .into_iter()
        .filter_map(|(name, source)| {
            let path = source.library.strip_prefix(&prefix)?;
            let file = library.load(path).ok()?;
            (file.revision > source.version).then(|| MacroUpdate {
                definition: name.to_owned(),
                path: path.to_owned(),
                project_revision: source.version,
                library_revision: file.revision,
            })
        })
        .collect();
    out.sort_by(|a, b| a.definition.cmp(&b.definition));
    out
}
