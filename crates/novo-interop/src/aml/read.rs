//! AutomationML to a neutral [`Plant`]: what a CAEX file says about devices,
//! locations, terminals and cables, from KoperCAD's own export or from any
//! other tool that writes AutomationML.
//!
//! The reader is forgiving about vocabulary (the tag may be in an attribute
//! called `Tag`, `TagName`, `Designation`, or just the element name) and
//! strict about nothing it cannot use: such elements are counted in the
//! [`Report`], never guessed.

use std::collections::BTreeMap;

use novo_core::{DeviceKind, DeviceTag, Value};
use thiserror::Error;

use crate::eplan::xml::{Node, XmlError, parse};
use crate::report::Report;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AmlError {
    #[error("not readable XML: {0}")]
    Xml(#[from] XmlError),
    #[error("this is not an AutomationML file (the root element is <{0}>, not <CAEXFile>)")]
    NotCaex(String),
    #[error("the file has no instance hierarchy, so there is no plant in it")]
    NoHierarchy,
}

/// A plant section or location.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantStructure {
    pub tag: DeviceTag,
    pub description: String,
    pub enclosure: bool,
    /// Mounting plate width and height in millimetres, when the file has them.
    pub plate: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlantTerminal {
    pub name: String,
    pub level: u8,
    pub jumper: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlantDevice {
    pub tag: DeviceTag,
    pub description: String,
    pub kind: DeviceKind,
    pub manufacturer: String,
    pub order_number: String,
    pub type_number: String,
    pub terminals: Vec<PlantTerminal>,
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlantCore {
    pub name: String,
    pub cross_section_mm2: Option<f64>,
    pub shield: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlantCable {
    pub tag: DeviceTag,
    pub description: String,
    pub length_m: Option<f64>,
    pub cores: Vec<PlantCore>,
}

/// A drawn page of the file's drawing profile (role `Page`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlantPage {
    /// The page's name, usually its label (`=A1+CAB1/01`).
    pub name: String,
    pub title: String,
}

/// A free text of a page (role `Text`): a cross section, a note. Device tags
/// are not among them.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantText {
    /// Name of the page it sits on.
    pub page: String,
    pub value: String,
    /// Millimetres on the page, origin bottom left, y up.
    pub at: (f64, f64),
    /// Character height in millimetres when the file says; else the default.
    pub height: Option<f64>,
    /// Turn in degrees counter-clockwise, 0 when the file has none.
    pub rotation: f64,
}

/// A line or rectangle drawn on a page (roles `Line` and `Rectangle`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlantShape {
    /// Name of the page it sits on.
    pub page: String,
    /// A rectangle between the two corners; else a line from one to the other.
    pub rect: bool,
    pub a: (f64, f64),
    pub b: (f64, f64),
}

/// A connection point of a drawn symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantPin {
    pub name: String,
    /// Offset from the symbol's position in millimetres, y up.
    pub offset: (f64, f64),
    /// The sides a wire may leave from: `up`, `down`, `left`, `right`.
    pub faces: Vec<String>,
}

/// One drawn symbol of a device or terminal strip (role `Symbol`): which
/// page, where, and its pins. A device may have many (a coil and its
/// contacts on other pages).
#[derive(Debug, Clone, PartialEq)]
pub struct PlantSymbol {
    /// Index into [`Plant::devices`] of the device or strip it belongs to;
    /// `None` for a symbol that stands for no device (an earth symbol),
    /// which the file lists under its page.
    pub device: Option<usize>,
    /// The terminal this symbol draws, for a strip.
    pub terminal: Option<String>,
    /// Name of the page it sits on (matches a [`PlantPage::name`]).
    pub page: String,
    /// Millimetres on the page, origin bottom left, y up.
    pub at: (f64, f64),
    /// What the symbol is: `coil`, `contact`, `terminal`, `lamp`, `motor`...
    pub kind: String,
    pub pins: Vec<PlantPin>,
}

/// A net: the symbol pins that are one conductor, as `(symbol, pin)` indexes
/// into [`Plant::symbols`] and that symbol's pins.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantNet {
    pub name: String,
    pub pins: Vec<(usize, usize)>,
}

/// Everything a file holds that KoperCAD can create.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plant {
    pub name: String,
    pub structures: Vec<PlantStructure>,
    pub devices: Vec<PlantDevice>,
    pub cables: Vec<PlantCable>,
    pub pages: Vec<PlantPage>,
    pub symbols: Vec<PlantSymbol>,
    pub nets: Vec<PlantNet>,
    pub texts: Vec<PlantText>,
    pub shapes: Vec<PlantShape>,
    pub report: Report,
}

/// An attribute with its nested attributes, by dotted name.
fn attributes(node: &Node) -> BTreeMap<String, (String, String)> {
    fn walk(node: &Node, prefix: &str, out: &mut BTreeMap<String, (String, String)>) {
        for a in node.children.iter().filter(|c| c.name == "Attribute") {
            let name = a.attr("Name").unwrap_or_default();
            let key = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}.{name}")
            };
            let value = a
                .children
                .iter()
                .find(|c| c.name == "Value")
                .map(|v| v.text.clone());
            if let Some(v) = value {
                out.insert(
                    key.clone(),
                    (
                        v,
                        a.attr("AttributeDataType").unwrap_or_default().to_string(),
                    ),
                );
            }
            walk(a, &key, out);
        }
    }
    let mut out = BTreeMap::new();
    walk(node, "", &mut out);
    out
}

fn first<'a>(attrs: &'a BTreeMap<String, (String, String)>, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|n| {
        attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(n))
            .map(|(_, (v, _))| v.trim())
            .filter(|v| !v.is_empty())
    })
}

/// The last segment of the element's role path, lower case (`terminalstrip`).
fn role_of(node: &Node) -> String {
    let path = node
        .children
        .iter()
        .find_map(|c| match c.name.as_str() {
            "RoleRequirements" => c.attr("RefBaseRoleClassPath"),
            "SupportedRoleClass" => c.attr("RefRoleClassPath"),
            _ => None,
        })
        .unwrap_or_default();
    path.rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Upper bounds for sizes read from a file; anything above is treated as a typo.
const MAX_PLATE_MM: f64 = 10_000.0;
const MAX_CROSS_SECTION_MM2: f64 = 2_000.0;
const MAX_LENGTH_M: f64 = 100_000.0;
/// Page coordinates and pin offsets beyond this are a typo (a page is a few hundred millimetres).
const MAX_PAGE_MM: f64 = 5_000.0;

/// A size from a file: finite, above zero and no larger than `max`.
fn dimension(text: &str, max: f64) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0 && *v <= max)
}

fn value_of(text: &str, data_type: &str) -> Value {
    let t = data_type.to_ascii_lowercase();
    if t.ends_with("boolean") {
        return Value::Bool(text.eq_ignore_ascii_case("true") || text == "1");
    }
    if (t.ends_with("int") || t.ends_with("long") || t.ends_with("integer"))
        && let Ok(i) = text.parse::<i64>()
    {
        return Value::Int(i);
    }
    if (t.ends_with("double") || t.ends_with("float") || t.ends_with("decimal"))
        && let Ok(f) = text.parse::<f64>()
        && f.is_finite()
    {
        return Value::Float(f);
    }
    Value::Text(text.to_string())
}

/// Aspects handed down from the sections and locations above an element.
#[derive(Clone, Default)]
struct Context {
    function: Vec<String>,
    location: Vec<String>,
}

impl Context {
    fn complete(&self, mut tag: DeviceTag) -> DeviceTag {
        if tag.function.is_empty() {
            tag.function = self.function.clone();
        }
        if tag.location.is_empty() {
            tag.location = self.location.clone();
        }
        tag
    }
}

struct Reader {
    plant: Plant,
    ignored_elements: usize,
    duplicate_tags: usize,
    bad_numbers: usize,
    links: usize,
    frames: usize,
    seen: std::collections::BTreeSet<String>,
    /// Interface id of a symbol pin to `(symbol, pin)`.
    pin_ids: BTreeMap<String, (usize, usize)>,
    /// Both ends of every internal link, by the ids they name.
    link_ends: Vec<(String, String)>,
    /// Nets as read: name and the ids of their net points.
    net_points: Vec<(String, Vec<String>)>,
    /// Symbols of devices that were skipped (a repeated tag).
    lost_symbols: usize,
}

const TAG_NAMES: &[&str] = &[
    "Tag",
    "TagName",
    "DeviceTag",
    "Designation",
    "ReferenceDesignation",
    "BMK",
    "Kennzeichen",
];

/// Reads an AutomationML file.
pub fn read_aml(text: &str) -> Result<Plant, AmlError> {
    let root = parse(text)?;
    if root.name != "CAEXFile" {
        return Err(AmlError::NotCaex(root.name));
    }
    let hierarchies: Vec<&Node> = root
        .children
        .iter()
        .filter(|c| c.name == "InstanceHierarchy")
        .collect();
    if hierarchies.is_empty() {
        return Err(AmlError::NoHierarchy);
    }
    let mut r = Reader {
        plant: Plant::default(),
        ignored_elements: 0,
        duplicate_tags: 0,
        bad_numbers: 0,
        links: 0,
        frames: 0,
        seen: Default::default(),
        pin_ids: BTreeMap::new(),
        link_ends: Vec::new(),
        net_points: Vec::new(),
        lost_symbols: 0,
    };
    r.plant.name = hierarchies[0].attr("Name").unwrap_or_default().to_string();
    for h in hierarchies {
        for el in h.children.iter().filter(|c| c.name == "InternalElement") {
            r.element(el, &Context::default());
        }
    }
    let resolved = r.resolve_nets();
    let mut report = std::mem::take(&mut r.plant.report);
    report.moved("locations and sections", r.plant.structures.len());
    report.moved("devices", r.plant.devices.len());
    report.moved(
        "terminals",
        r.plant.devices.iter().map(|d| d.terminals.len()).sum(),
    );
    report.moved("cables", r.plant.cables.len());
    report.moved("pages", r.plant.pages.len());
    report.moved("drawn symbols", r.plant.symbols.len());
    report.moved("nets", r.plant.nets.len());
    report.lost(
        "connection links",
        r.links.saturating_sub(resolved),
        "the file has no drawn symbols for them, so there is nothing to wire on a page",
    );
    report.lost(
        "drawn symbols of skipped devices",
        r.lost_symbols,
        "a repeated tag is used once, with the symbols of its first element",
    );
    report.lost(
        "elements without a usable tag",
        r.ignored_elements,
        "no attribute or name that reads as a device tag",
    );
    report.lost(
        "repeated tags",
        r.duplicate_tags,
        "a tag already read from this file is used once",
    );
    report.lost(
        "sizes that are not a finite positive number in range",
        r.bad_numbers,
        "plate sizes, cable lengths and cross sections outside the accepted range are dropped",
    );
    report.lost(
        "positions",
        r.frames,
        "cabinet placement is set in the layout, not imported",
    );
    r.plant.report = report;
    Ok(r.plant)
}

impl Reader {
    fn element(&mut self, node: &Node, ctx: &Context) {
        let name = node.attr("Name").unwrap_or_default().trim().to_string();
        let attrs = attributes(node);
        let role = role_of(node);
        for l in node.children.iter().filter(|c| c.name == "InternalLink") {
            self.links += 1;
            if let (Some(a), Some(b)) = (l.attr("RefPartnerSideA"), l.attr("RefPartnerSideB")) {
                self.link_ends.push((a.to_string(), b.to_string()));
            }
        }

        // Pages, symbols and nets belong to the drawing profile: pages are
        // read here, symbols with their device, nets after everything else.
        match role.as_str() {
            "pagegroup" => {
                for c in node.children.iter().filter(|c| c.name == "InternalElement") {
                    self.element(c, ctx);
                }
                return;
            }
            "page" => {
                self.page(node, &name, &attrs);
                self.texts_of(node, &name);
                self.shapes_of(node, &name);
                self.symbols_of(node, None);
                return;
            }
            "symbol" => return,
            "netgroup" => {
                for c in node.children.iter().filter(|c| c.name == "InternalElement") {
                    self.element(c, ctx);
                }
                return;
            }
            "net" => {
                self.net(node, &name);
                return;
            }
            "wire" | "connection" => return,
            _ => {}
        }

        // A plant section or location: only aspects, no product level.
        let structural = !name.is_empty()
            && (name.starts_with('=') || name.starts_with('+'))
            && DeviceTag::parse(&name).is_ok_and(|t| t.product.is_empty());
        if structural
            || role.contains("plantsection")
            || role.contains("location")
            || role.contains("enclosure")
        {
            let text = first(&attrs, TAG_NAMES).map_or(name.clone(), str::to_string);
            if let Ok(tag) = DeviceTag::parse(&text)
                && tag.product.is_empty()
                && !tag.is_empty()
            {
                let tag = ctx.complete(tag);
                let mut inner = ctx.clone();
                if !tag.function.is_empty() {
                    inner.function = tag.function.clone();
                }
                if !tag.location.is_empty() {
                    inner.location = tag.location.clone();
                }
                if !self.seen.insert(tag.to_string()) {
                    self.duplicate_tags += 1;
                    return;
                }
                let plate = match (
                    first(&attrs, &["PlateWidth"]),
                    first(&attrs, &["PlateHeight"]),
                ) {
                    (Some(w), Some(h)) => {
                        let plate = dimension(w, MAX_PLATE_MM).zip(dimension(h, MAX_PLATE_MM));
                        self.bad_numbers += usize::from(plate.is_none());
                        plate
                    }
                    _ => None,
                };
                // A location is a panel unless the file says it is not.
                let enclosure = first(&attrs, &["Enclosure"]).map_or(
                    role.contains("enclosure")
                        || (!tag.location.is_empty() && tag.function.is_empty()),
                    |v| v == "true",
                );
                self.plant.structures.push(PlantStructure {
                    tag: DeviceTag {
                        product: Vec::new(),
                        ..tag
                    },
                    description: first(&attrs, &["Description"])
                        .unwrap_or_default()
                        .to_string(),
                    enclosure,
                    plate,
                });
                self.children(node, &inner);
                return;
            }
        }

        // A cable.
        if role.contains("cable") && !role.contains("core") {
            self.cable(node, &name, &attrs, ctx);
            return;
        }

        // A device, if its tag reads.
        let tag_text = first(&attrs, TAG_NAMES).map_or(name.clone(), str::to_string);
        let parsed = DeviceTag::parse(&tag_text)
            .ok()
            .filter(|t| !t.product.is_empty());
        let Some(tag) = parsed else {
            // A grouping element (a folder in the other tool): look inside.
            if node.children.iter().any(|c| c.name == "InternalElement") {
                self.children(node, ctx);
            } else if !name.is_empty() || !attrs.is_empty() {
                self.ignored_elements += 1;
            }
            return;
        };
        let tag = ctx.complete(tag);
        if !self.seen.insert(tag.to_string()) {
            self.duplicate_tags += 1;
            self.lost_symbols += node
                .children
                .iter()
                .filter(|c| c.name == "InternalElement" && role_of(c) == "symbol")
                .count();
            return;
        }
        if attrs.contains_key("Frame.x") {
            self.frames += 1;
        }
        let code = tag.letter_code().to_string();
        let kind = if role.contains("terminalstrip") || role.contains("terminal") || code == "X" {
            DeviceKind::TerminalStrip
        } else if role.contains("plc") {
            DeviceKind::Plc
        } else if role.contains("connector") {
            DeviceKind::Connector
        } else {
            DeviceKind::General
        };
        let mut terminals = Vec::new();
        if kind == DeviceKind::TerminalStrip {
            for i in node
                .children
                .iter()
                .filter(|c| c.name == "ExternalInterface")
            {
                let Some(n) = i.attr("Name").map(str::trim).filter(|n| !n.is_empty()) else {
                    continue;
                };
                let a = attributes(i);
                terminals.push(PlantTerminal {
                    name: n.to_string(),
                    level: first(&a, &["Level"])
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1),
                    jumper: first(&a, &["Jumper"]).and_then(|v| v.parse().ok()),
                });
            }
        }
        let mut properties = BTreeMap::new();
        for (k, (v, ty)) in &attrs {
            if let Some(key) = k.strip_prefix("Properties.") {
                properties.insert(key.to_string(), value_of(v, ty));
            }
        }
        // Unknown flat attributes survive as properties under `aml.`.
        const KNOWN: &[&str] = &[
            "Tag",
            "TagName",
            "DeviceTag",
            "Designation",
            "ReferenceDesignation",
            "BMK",
            "Kennzeichen",
            "Description",
            "Manufacturer",
            "OrderNumber",
            "TypeNumber",
            "ArticleDescription",
        ];
        for (k, (v, ty)) in &attrs {
            let flat = !k.contains('.');
            let known = KNOWN.iter().any(|n| n.eq_ignore_ascii_case(k));
            if flat
                && !known
                && k != "Properties"
                && k != "Frame"
                && k != "AdditionalArticles"
                && !v.is_empty()
            {
                properties.insert(format!("aml.{k}"), value_of(v, ty));
            }
        }
        self.plant.devices.push(PlantDevice {
            tag,
            description: first(&attrs, &["Description"])
                .unwrap_or_default()
                .to_string(),
            kind,
            manufacturer: first(&attrs, &["Manufacturer", "Hersteller", "Vendor"])
                .unwrap_or_default()
                .to_string(),
            order_number: first(
                &attrs,
                &[
                    "OrderNumber",
                    "ArticleNumber",
                    "Bestellnummer",
                    "PartNumber",
                ],
            )
            .unwrap_or_default()
            .to_string(),
            type_number: first(&attrs, &["TypeNumber", "TypeDesignation", "Typ"])
                .unwrap_or_default()
                .to_string(),
            terminals,
            properties,
        });
        let device_index = self.plant.devices.len() - 1;
        self.symbols_of(node, Some(device_index));
        // Sub-devices (a card in a rack) are devices too.
        let ctx = Context {
            function: self
                .plant
                .devices
                .last()
                .map_or(ctx.function.clone(), |d| d.tag.function.clone()),
            location: self
                .plant
                .devices
                .last()
                .map_or(ctx.location.clone(), |d| d.tag.location.clone()),
        };
        self.children(node, &ctx);
    }

    fn page(&mut self, _node: &Node, name: &str, attrs: &BTreeMap<String, (String, String)>) {
        if name.is_empty() {
            return;
        }
        self.plant.pages.push(PlantPage {
            name: name.to_string(),
            title: first(attrs, &["Title", "Description"])
                .unwrap_or_default()
                .to_string(),
        });
    }

    /// The lines and rectangles on a page.
    fn shapes_of(&mut self, node: &Node, page: &str) {
        for c in node.children.iter().filter(|c| c.name == "InternalElement") {
            let rect = match role_of(c).as_str() {
                "line" => false,
                "rectangle" => true,
                _ => continue,
            };
            let a = attributes(c);
            let coord = |k: &str| {
                first(&a, &[k])
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite() && v.abs() <= MAX_PAGE_MM)
            };
            if let (Some(x1), Some(y1), Some(x2), Some(y2)) =
                (coord("X1"), coord("Y1"), coord("X2"), coord("Y2"))
            {
                self.plant.shapes.push(PlantShape {
                    page: page.to_string(),
                    rect,
                    a: (x1, y1),
                    b: (x2, y2),
                });
            }
        }
    }

    /// The free texts on a page: `Text` children with a value and a place.
    fn texts_of(&mut self, node: &Node, page: &str) {
        for t in node
            .children
            .iter()
            .filter(|c| c.name == "InternalElement" && role_of(c) == "text")
        {
            let a = attributes(t);
            let coord = |k: &str| {
                first(&a, &[k])
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite() && v.abs() <= MAX_PAGE_MM)
            };
            let value = first(&a, &["Value"]).unwrap_or_default().trim().to_string();
            let (Some(x), Some(y)) = (coord("Frame.x"), coord("Frame.y")) else {
                continue;
            };
            if !value.is_empty() {
                self.plant.texts.push(PlantText {
                    page: page.to_string(),
                    value,
                    at: (x, y),
                    height: coord("Height").filter(|h| *h > 0.0 && *h <= 50.0),
                    rotation: first(&a, &["Rotation", "Frame.rz"])
                        .and_then(|v| v.parse::<f64>().ok())
                        .filter(|v| v.is_finite())
                        .unwrap_or(0.0),
                });
            }
        }
    }

    /// The `Symbol` children of a device or strip, or of a page (symbols of no device).
    fn symbols_of(&mut self, node: &Node, device: Option<usize>) {
        for sym in node
            .children
            .iter()
            .filter(|c| c.name == "InternalElement" && role_of(c) == "symbol")
        {
            let a = attributes(sym);
            let Some(page) = first(&a, &["Page"]) else {
                self.lost_symbols += 1;
                continue;
            };
            let coord = |k: &str| {
                first(&a, &[k])
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite() && v.abs() <= MAX_PAGE_MM)
            };
            let (Some(x), Some(y)) = (coord("Frame.x"), coord("Frame.y")) else {
                self.lost_symbols += 1;
                continue;
            };
            let index = self.plant.symbols.len();
            let mut pins = Vec::new();
            for i in sym
                .children
                .iter()
                .filter(|c| c.name == "ExternalInterface")
            {
                let pa = attributes(i);
                let offset = |k: &str| {
                    first(&pa, &[k])
                        .and_then(|v| v.parse::<f64>().ok())
                        .filter(|v| v.is_finite() && v.abs() <= MAX_PAGE_MM)
                        .unwrap_or(0.0)
                };
                if let Some(id) = i.attr("ID") {
                    self.pin_ids.insert(id.to_string(), (index, pins.len()));
                }
                pins.push(PlantPin {
                    name: i.attr("Name").unwrap_or_default().trim().to_string(),
                    offset: (offset("X"), offset("Y")),
                    faces: first(&pa, &["Faces"])
                        .unwrap_or_default()
                        .split(',')
                        .map(|f| f.trim().to_ascii_lowercase())
                        .filter(|f| matches!(f.as_str(), "up" | "down" | "left" | "right"))
                        .collect(),
                });
            }
            self.plant.symbols.push(PlantSymbol {
                device,
                terminal: first(&a, &["Terminal"]).map(str::to_string),
                page: page.to_string(),
                at: (x, y),
                kind: first(&a, &["Kind"]).unwrap_or("generic").to_string(),
                pins,
            });
        }
    }

    /// A net element: its name and the ids of its net points.
    fn net(&mut self, node: &Node, name: &str) {
        let points: Vec<String> = node
            .children
            .iter()
            .filter(|c| c.name == "ExternalInterface")
            .filter_map(|c| c.attr("ID").map(str::to_string))
            .collect();
        if points.len() >= 2 {
            // An unnamed net still needs a name to pair its interruption points by.
            let name = if name.trim().is_empty() {
                format!("N{}", self.net_points.len() + 1)
            } else {
                name.to_string()
            };
            self.net_points.push((name, points));
        }
    }

    /// Joins the net points to the symbol pins through the links; returns
    /// how many links ended on a drawn pin.
    fn resolve_nets(&mut self) -> usize {
        let mut partner: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (a, b) in &self.link_ends {
            partner.entry(a.as_str()).or_default().push(b.as_str());
            partner.entry(b.as_str()).or_default().push(a.as_str());
        }
        let mut resolved = 0usize;
        let mut nets = Vec::new();
        for (name, points) in &self.net_points {
            let mut pins: Vec<(usize, usize)> = Vec::new();
            for p in points {
                for other in partner.get(p.as_str()).into_iter().flatten() {
                    if let Some(&pin) = self.pin_ids.get(*other) {
                        resolved += 1;
                        if !pins.contains(&pin) {
                            pins.push(pin);
                        }
                    }
                }
            }
            if pins.len() >= 2 {
                nets.push(PlantNet {
                    name: name.clone(),
                    pins,
                });
            }
        }
        self.plant.nets = nets;
        resolved
    }

    fn children(&mut self, node: &Node, ctx: &Context) {
        for c in node.children.iter().filter(|c| c.name == "InternalElement") {
            self.element(c, ctx);
        }
    }

    fn cable(
        &mut self,
        node: &Node,
        name: &str,
        attrs: &BTreeMap<String, (String, String)>,
        ctx: &Context,
    ) {
        let tag_text = first(attrs, TAG_NAMES).map_or(name.to_string(), str::to_string);
        let Some(tag) = DeviceTag::parse(&tag_text)
            .ok()
            .filter(|t| !t.product.is_empty())
        else {
            self.ignored_elements += 1;
            return;
        };
        let tag = ctx.complete(tag);
        if !self.seen.insert(tag.to_string()) {
            self.duplicate_tags += 1;
            return;
        }
        let cores = node
            .children
            .iter()
            .filter(|c| c.name == "InternalElement")
            .map(|c| {
                let a = attributes(c);
                PlantCore {
                    name: c.attr("Name").unwrap_or_default().to_string(),
                    cross_section_mm2: first(&a, &["CrossSection"]).and_then(|v| {
                        let n = dimension(v, MAX_CROSS_SECTION_MM2);
                        self.bad_numbers += usize::from(n.is_none());
                        n
                    }),
                    shield: first(&a, &["Shield"]).is_some_and(|v| v == "true"),
                }
            })
            .collect();
        self.plant.cables.push(PlantCable {
            tag,
            description: first(attrs, &["Description"])
                .unwrap_or_default()
                .to_string(),
            length_m: first(attrs, &["Length"]).and_then(|v| {
                let n = dimension(v, MAX_LENGTH_M);
                self.bad_numbers += usize::from(n.is_none());
                n
            }),
            cores,
        });
    }
}
