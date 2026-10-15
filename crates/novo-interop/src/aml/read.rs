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

/// Everything a file holds that KoperCAD can create.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plant {
    pub name: String,
    pub structures: Vec<PlantStructure>,
    pub devices: Vec<PlantDevice>,
    pub cables: Vec<PlantCable>,
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
    links: usize,
    frames: usize,
    seen: std::collections::BTreeSet<String>,
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
        links: 0,
        frames: 0,
        seen: Default::default(),
    };
    r.plant.name = hierarchies[0].attr("Name").unwrap_or_default().to_string();
    for h in hierarchies {
        for el in h.children.iter().filter(|c| c.name == "InternalElement") {
            r.element(el, &Context::default());
        }
    }
    let mut report = std::mem::take(&mut r.plant.report);
    report.moved("locations and sections", r.plant.structures.len());
    report.moved("devices", r.plant.devices.len());
    report.moved(
        "terminals",
        r.plant.devices.iter().map(|d| d.terminals.len()).sum(),
    );
    report.moved("cables", r.plant.cables.len());
    report.lost(
        "connection links",
        r.links,
        "wires need symbols on a page; the connections are not drawn",
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
        self.links += node
            .children
            .iter()
            .filter(|c| c.name == "InternalLink")
            .count();

        // Nets and wire groups only carry connections.
        if matches!(role.as_str(), "net" | "netgroup" | "wire" | "connection") {
            return;
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
                let plate = first(&attrs, &["PlateWidth"])
                    .zip(first(&attrs, &["PlateHeight"]))
                    .and_then(|(w, h)| Some((w.parse::<f64>().ok()?, h.parse::<f64>().ok()?)));
                self.plant.structures.push(PlantStructure {
                    tag: DeviceTag {
                        product: Vec::new(),
                        ..tag
                    },
                    description: first(&attrs, &["Description"])
                        .unwrap_or_default()
                        .to_string(),
                    enclosure: first(&attrs, &["Enclosure"]).is_some_and(|v| v == "true")
                        || role.contains("enclosure"),
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
                    cross_section_mm2: first(&a, &["CrossSection"]).and_then(|v| v.parse().ok()),
                    shield: first(&a, &["Shield"]).is_some_and(|v| v == "true"),
                }
            })
            .collect();
        self.plant.cables.push(PlantCable {
            tag,
            description: first(attrs, &["Description"])
                .unwrap_or_default()
                .to_string(),
            length_m: first(attrs, &["Length"]).and_then(|v| v.parse().ok()),
            cores,
        });
    }
}
