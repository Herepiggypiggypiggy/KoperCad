//! Reads the parts XML files some makers publish for parts management
//! programs (the "Data Portal" family): one file per part plus companion files
//! for its connection points, its drilling data and its accessories.
//!
//! What the format looks like, as far as this reader relies on it:
//! - the root element is `partsmanagement` and its values are attributes;
//! - a file is one of several kinds, told by its element names or by the
//!   file name `MAKER.ARTICLE.kind.xml`: `part`, `connectionpoints`,
//!   `construction`, `accessorylist`, `accessoryplacementlist`,
//!   `manufacturer`, `propertydefinition`;
//! - text in several languages is `en_US@Contactor;nl_NL@Contactor;`;
//! - electrical values may carry their unit and a decimal comma
//!   (`6,0 mm2`, `41,0 A`), geometry is plain numbers in `length-unit`.
//!
//! The reader was written from those format notes and tested on invented
//! files, not on a maker's file, and attribute names differ between makers
//! and versions. So it never guesses silently: an attribute is matched by
//! what its name says (`wirecount`, `torquemax`, `crosssectionmin`), the
//! ones it cannot place are kept as written (`eplan.<name>`, or the terminal's
//! `extra`) and counted in [`PortalRead::unrecognised`], and everything it
//! assumed is listed in [`PortalRead::assumed`] for the screen to show.
//!
//! Positions of connection points and holes are kept in the maker's own
//! frame as written. The corner they are measured from is not the same in
//! every file, so they are not turned into `position_mm`, which wire routing
//! trusts.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::Value;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use thiserror::Error;

use crate::catalog::{CatalogPart, Dimensions, PartKey};
use crate::describe::keys;
use crate::recipes::{Dim, parse_quantity, recipe_for};
use crate::schema::{
    Accessory, ConnectionKind, DataSource, DrillHole, DrillRepeat, PartRef, PartTerminal,
    Provenance, eng,
};
use crate::search::normalize_code;

/// Source tag in a part's provenance.
pub const PORTAL_SOURCE: &str = "import:partsxml";

const MAX_FILE_BYTES: usize = 32 * 1024 * 1024;
const MAX_DEPTH: usize = 48;
const MAX_ELEMENTS: usize = 400_000;
/// Longest drill repeat taken at face value.
const MAX_REPEAT: u32 = 1000;
/// Notes of one kind listed before the rest are only counted.
const NOTES_PER_KIND: usize = 8;

#[derive(Debug, Error)]
pub enum PortalError {
    #[error("none of the {0} file(s) is a maker parts XML (root element `partsmanagement`)")]
    NoPortalFiles(usize),
    #[error(
        "the files hold no parts: add the part files (`...part.xml`), not only companion files"
    )]
    NoParts,
}

/// One file as the user picked it.
#[derive(Debug, Clone)]
pub struct PortalFile {
    /// The file name, which also names the part a companion file belongs to.
    pub name: String,
    pub text: String,
}

/// How many things were read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortalCounts {
    pub files: usize,
    pub parts: usize,
    pub parts_with_terminals: usize,
    pub terminals: usize,
    /// Terminals that have a wire range.
    pub with_wire_range: usize,
    /// Terminals that have a torque.
    pub with_torque: usize,
    pub parts_with_drilling: usize,
    pub holes: usize,
    pub parts_with_accessories: usize,
    pub accessories: usize,
}

/// What a set of maker parts XML files held.
#[derive(Debug, Clone, Default)]
pub struct PortalRead {
    pub parts: Vec<CatalogPart>,
    pub counts: PortalCounts,
    /// `file (line n): why` for files and records that were left out.
    pub skipped: Vec<String>,
    /// What the reader assumed, for the screen to show next to what it read.
    pub assumed: Vec<String>,
    /// Attributes and elements it kept as written without understanding
    /// them, as `kind/name` with how often they came up.
    pub unrecognised: BTreeMap<String, usize>,
    /// Companion files whose part is not among the files.
    pub unattached: Vec<String>,
}

impl PortalRead {
    /// A text with names only, never values, that says what the files
    /// looked like. It is what to send when a maker's file reads badly.
    pub fn format_report(&self) -> String {
        let mut out = String::from("Maker parts XML read report (names only)\n");
        out.push_str(&format!(
            "files {}, parts {}, terminals {}, holes {}, accessories {}\n",
            self.counts.files,
            self.counts.parts,
            self.counts.terminals,
            self.counts.holes,
            self.counts.accessories
        ));
        out.push_str("Kept as written, not understood:\n");
        if self.unrecognised.is_empty() {
            out.push_str("  (nothing)\n");
        }
        for (name, n) in &self.unrecognised {
            out.push_str(&format!("  {name} x{n}\n"));
        }
        for a in &self.assumed {
            out.push_str(&format!("Assumed: {a}\n"));
        }
        out
    }
}

/// Whether `text` looks like a maker parts XML: the root element is
/// `partsmanagement`.
pub fn looks_like_portal(text: &str) -> bool {
    let head: String = text.chars().take(4096).collect();
    let lower = head.to_ascii_lowercase();
    lower.contains("<partsmanagement")
}

// ---------------------------------------------------------------- XML tree

#[derive(Debug, Default)]
struct Node {
    /// Lower-case local name.
    name: String,
    /// Lower-case local names with their values.
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
    text: String,
    line: usize,
}

impl Node {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Elements below this one (not itself) that have no element children.
    fn leaves<'a>(&'a self, out: &mut Vec<&'a Node>) {
        for c in &self.children {
            if c.children.is_empty() {
                out.push(c);
            } else {
                c.leaves(out);
            }
        }
    }
}

fn local_lower(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    s.rsplit(':').next().unwrap_or("").to_ascii_lowercase()
}

fn start_node(e: &BytesStart, line: usize) -> Node {
    let mut attrs = Vec::new();
    for a in e.attributes().flatten() {
        let key = local_lower(a.key.as_ref());
        if let Ok(v) = quick_xml::escape::unescape(&String::from_utf8_lossy(&a.value)) {
            attrs.push((key, v.trim().to_owned()));
        }
    }
    Node {
        name: local_lower(e.name().as_ref()),
        attrs,
        line,
        ..Node::default()
    }
}

fn parse_tree(text: &str) -> Result<Node, String> {
    if text.len() > MAX_FILE_BYTES {
        return Err("the file is larger than 32 MB".into());
    }
    let text = text.trim_start_matches('\u{feff}');
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = false;
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let mut elements = 0usize;
    let line_at = |pos: usize| {
        text.as_bytes()[..pos.min(text.len())]
            .iter()
            .filter(|b| **b == b'\n')
            .count()
            + 1
    };
    loop {
        let pos = usize::try_from(reader.buffer_position()).unwrap_or(0);
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                elements += 1;
                if elements > MAX_ELEMENTS {
                    return Err("the file has more than 400000 elements".into());
                }
                if stack.len() >= MAX_DEPTH {
                    return Err(format!("the XML nests deeper than {MAX_DEPTH} levels"));
                }
                stack.push(start_node(&e, line_at(pos)));
            }
            Event::Empty(e) => {
                elements += 1;
                if elements > MAX_ELEMENTS {
                    return Err("the file has more than 400000 elements".into());
                }
                let node = start_node(&e, line_at(pos));
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None if root.is_none() => root = Some(node),
                    None => {}
                }
            }
            Event::End(_) => {
                let Some(node) = stack.pop() else {
                    return Err("an end tag without a start tag".into());
                };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::Text(t) => {
                if let (Some(top), Ok(s)) = (stack.last_mut(), t.decode()) {
                    let s = s.trim();
                    if !s.is_empty() {
                        if !top.text.is_empty() {
                            top.text.push(' ');
                        }
                        top.text.push_str(s);
                    }
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text
                        .push_str(String::from_utf8_lossy(&t.into_inner()).trim());
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err("the XML is cut off".into());
    }
    root.ok_or_else(|| "the file has no XML element".into())
}

// ----------------------------------------------------------------- helpers

/// An attribute name reduced to what it says: lower case letters and digits,
/// without the program prefix some makers put in front.
fn norm(key: &str) -> String {
    let k: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    for prefix in [
        "particleterminal",
        "particleconnection",
        "particleconstruction",
        "particle",
    ] {
        if let Some(rest) = k.strip_prefix(prefix)
            && !rest.is_empty()
        {
            return rest.to_owned();
        }
    }
    k
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bound {
    Min,
    Max,
}

/// Whether a name marks the lower or the upper end of a range.
fn bound(k: &str) -> Option<Bound> {
    if k.contains("min") || k.contains("from") || k.contains("lower") {
        Some(Bound::Min)
    } else if k.contains("max") || k.contains("upper") || (k.ends_with("to") && k.len() > 2) {
        Some(Bound::Max)
    } else {
        None
    }
}

/// Splits `en_US@Text;nl_NL@Tekst;` into language and text. Text without a
/// language prefix comes back under the language `""`.
pub(crate) fn multilingual(value: &str) -> Vec<(String, String)> {
    fn lang_prefix(piece: &str) -> Option<(&str, &str)> {
        let (lang, text) = piece.split_once('@')?;
        let ok = (2..=8).contains(&lang.len())
            && lang
                .chars()
                .all(|c| c.is_ascii_alphabetic() || c == '_' || c == '-')
            && lang.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
        ok.then_some((lang, text))
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for piece in value.split(';') {
        match lang_prefix(piece) {
            Some((lang, text)) => out.push((lang.to_owned(), text.trim().to_owned())),
            None => match out.last_mut() {
                // A `;` inside the text: the piece belongs to the one before.
                Some((_, text)) => {
                    text.push(';');
                    text.push_str(piece);
                }
                None => out.push((String::new(), piece.to_owned())),
            },
        }
    }
    for (_, t) in &mut out {
        *t = t.trim().trim_end_matches(';').trim().to_owned();
    }
    out.retain(|(_, t)| !t.is_empty());
    out
}

/// The text to show: English, then Dutch, German, French, else the first.
fn preferred(texts: &[(String, String)]) -> Option<&(String, String)> {
    for want in ["en", "nl", "de", "fr", ""] {
        if let Some(t) = texts.iter().find(|(l, _)| {
            l.split(['_', '-'])
                .next()
                .unwrap_or("")
                .eq_ignore_ascii_case(want)
        }) {
            return Some(t);
        }
    }
    texts.first()
}

/// A length in mm. A number without a unit is in the file's length unit.
fn length_mm(value: &str, file_unit: f64) -> Option<f64> {
    let q = parse_quantity(value)?;
    let f = if q.unit.is_empty() {
        file_unit
    } else {
        Dim::Length.factor(&q.unit)?
    };
    let v = q.low * f;
    v.is_finite().then_some(v)
}

fn number(value: &str) -> Option<f64> {
    parse_quantity(value)
        .map(|q| q.low)
        .filter(|v| v.is_finite())
}

fn count(value: &str) -> Option<u32> {
    let v = number(value)?;
    (v >= 0.0 && v.fract() == 0.0 && v < 1e6).then_some(v as u32)
}

/// A range written in one value or the single value of one end.
fn quantity_in(value: &str, dim: Dim) -> Option<(f64, f64)> {
    let q = parse_quantity(value)?;
    let f = dim.factor(&q.unit)?;
    Some((q.low * f, q.high * f))
}

/// `MAKER.ARTICLE` into the maker code and the article, when the first part
/// looks like a maker code (2 to 6 letters).
fn split_id(id: &str) -> (Option<&str>, &str) {
    if let Some((prefix, rest)) = id.split_once('.')
        && (2..=6).contains(&prefix.len())
        && prefix.chars().all(|c| c.is_ascii_alphabetic())
        && !rest.is_empty()
    {
        return (Some(prefix), rest);
    }
    (None, id)
}

/// Maker codes in file names and part numbers, and the maker they stand for.
const MAKER_CODES: &[(&str, &str)] = &[
    ("SE", "Schneider Electric"),
    ("SCHNEI", "Schneider Electric"),
    ("SIE", "Siemens"),
    ("SIEMEN", "Siemens"),
    ("PXC", "Phoenix Contact"),
    ("WER", "Weidmüller"),
    ("WIS", "Wieland"),
    ("RIT", "Rittal"),
    ("RITTAL", "Rittal"),
    ("ETN", "Eaton"),
    ("WAGO", "WAGO"),
    ("LAPP", "LAPP"),
    ("ILME", "ILME"),
];

fn maker_for_code(code: &str) -> Option<String> {
    let c = code.to_ascii_uppercase();
    MAKER_CODES
        .iter()
        .find(|(k, _)| *k == c)
        .map(|(_, v)| (*v).to_owned())
}

/// The maker's usual name when a recipe knows it (`Siemens AG` to `Siemens`).
fn canonical_maker(name: &str) -> String {
    recipe_for(name)
        .map(|r| r.manufacturer.to_owned())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| name.trim().to_owned())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Part,
    ConnectionPoints,
    Construction,
    AccessoryList,
    AccessoryPlacement,
    Manufacturer,
    PropertyDefinition,
}

impl Kind {
    fn of_element(name: &str) -> Option<Kind> {
        Some(match name {
            "part" | "article" | "product" => Kind::Part,
            "connectionpoints" | "connectionpoint" | "connections" => Kind::ConnectionPoints,
            "construction" | "drilling" | "drillings" | "mounting" => Kind::Construction,
            "accessorylist" | "accessories" => Kind::AccessoryList,
            "accessoryplacementlist" | "accessoryplacement" => Kind::AccessoryPlacement,
            "manufacturer" | "manufacturers" => Kind::Manufacturer,
            "propertydefinition" | "propertydefinitions" => Kind::PropertyDefinition,
            _ => return None,
        })
    }

    fn of_file_name(name: &str) -> Option<Kind> {
        let lower = name.to_ascii_lowercase();
        let stem = lower.strip_suffix(".xml").unwrap_or(&lower);
        let last = stem.rsplit('.').next()?;
        Kind::of_element(last)
    }

    fn label(self) -> &'static str {
        match self {
            Kind::Part => "part",
            Kind::ConnectionPoints => "connection points",
            Kind::Construction => "construction",
            Kind::AccessoryList => "accessory list",
            Kind::AccessoryPlacement => "accessory placement",
            Kind::Manufacturer => "manufacturer",
            Kind::PropertyDefinition => "property definition",
        }
    }
}

/// The part a companion file is about, from its name: `WAGO.2002-1201`
/// out of `WAGO.2002-1201.connectionpoints.xml`.
fn stem_of(file: &str) -> String {
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let lower = name.to_ascii_lowercase();
    let mut end = name.len();
    if lower.ends_with(".xml") {
        end -= 4;
    }
    if let Some(dot) = name[..end].rfind('.')
        && Kind::of_element(&lower[dot + 1..end]).is_some()
    {
        end = dot;
    }
    name[..end].to_owned()
}

fn note_once(list: &mut Vec<String>, s: impl Into<String>) {
    let s = s.into();
    if !list.contains(&s) {
        list.push(s);
    }
}

// --------------------------------------------------------------- the reader

/// One part being put together from its files.
struct Draft {
    part: CatalogPart,
    /// Accessory list names the part file refers to, normalized.
    list_refs: BTreeSet<String>,
    file: String,
}

#[derive(Default)]
struct Build {
    drafts: Vec<Draft>,
    /// Normalized id to draft index; `None` when two parts share it.
    by_id: HashMap<String, Option<usize>>,
    /// Maker code to long name from manufacturer files.
    makers: HashMap<String, String>,
    /// Property id to display name from property definitions.
    prop_names: HashMap<String, String>,
    out: PortalRead,
    today: String,
}

/// Reads a set of maker parts XML files into catalog parts. A companion file
/// (connection points, drilling, accessories) is joined to its part by the
/// part number inside it, else by its file name. `default_maker` is used
/// when neither the file nor the part number names one.
pub fn read_portal(
    files: &[PortalFile],
    default_maker: &str,
    today: &str,
) -> Result<PortalRead, PortalError> {
    let mut me = Build {
        today: today.to_owned(),
        ..Build::default()
    };
    // (file name, kind, element) for the companion kinds, read after parts.
    let mut trees: Vec<(String, Node)> = Vec::new();
    for f in files {
        if !looks_like_portal(&f.text) {
            me.out.skipped.push(format!(
                "{}: not a maker parts XML (no `partsmanagement` root)",
                f.name
            ));
            continue;
        }
        match parse_tree(&f.text) {
            Ok(root) if root.name == "partsmanagement" => trees.push((f.name.clone(), root)),
            Ok(_) => me.out.skipped.push(format!(
                "{}: not a maker parts XML (the root element is not `partsmanagement`)",
                f.name
            )),
            Err(e) => me
                .out
                .skipped
                .push(format!("{}: the XML is not valid ({e})", f.name)),
        }
    }
    if trees.is_empty() {
        return Err(PortalError::NoPortalFiles(files.len()));
    }
    me.out.counts.files = trees.len();

    // Pass 1: manufacturers and property names, which the parts use.
    for (file, root) in &trees {
        for el in kinds(root, file) {
            match el.0 {
                Kind::Manufacturer => me.manufacturer(el.1),
                Kind::PropertyDefinition => me.property_definition(el.1),
                _ => {}
            }
        }
    }
    // Pass 2: parts.
    for (file, root) in &trees {
        let units = Units::of(root, &mut me.out.assumed);
        for el in kinds(root, file) {
            if el.0 == Kind::Part {
                me.part(file, el.1, &units, default_maker);
            }
        }
    }
    if me.drafts.is_empty() {
        return Err(PortalError::NoParts);
    }
    // Pass 3: companion files.
    for (file, root) in &trees {
        let units = Units::of(root, &mut Vec::new());
        for el in kinds(root, file) {
            match el.0 {
                Kind::ConnectionPoints => me.connection_points(file, el.1, &units),
                Kind::Construction => me.construction(file, el.1, &units),
                Kind::AccessoryList => me.accessory_list(file, el.1),
                Kind::AccessoryPlacement => me.placement(file, el.1, &units),
                _ => {}
            }
        }
    }
    Ok(me.finish())
}

/// The kind of every top-level record of a file: its children by element
/// name, else the file name's kind for the children it cannot place.
fn kinds<'a>(root: &'a Node, file: &str) -> Vec<(Kind, &'a Node)> {
    let by_name = Kind::of_file_name(file);
    let mut out = Vec::new();
    for c in &root.children {
        if let Some(k) = Kind::of_element(&c.name).or(by_name) {
            out.push((k, c));
        }
    }
    out
}

/// The units the root element declares.
struct Units {
    /// mm per file length unit.
    length: f64,
    /// kg per file weight unit.
    weight: f64,
}

impl Units {
    fn of(root: &Node, assumed: &mut Vec<String>) -> Units {
        let length = match root.attr("length-unit") {
            Some(u) => match Dim::Length.factor(&u.to_ascii_lowercase()) {
                Some(f) => {
                    if f != 1.0 {
                        note_once(
                            assumed,
                            format!(
                                "Lengths are in {u} as the file's length-unit says; converted to mm."
                            ),
                        );
                    }
                    f
                }
                None => {
                    note_once(
                        assumed,
                        format!(
                            "The file's length-unit \"{u}\" is not known; lengths were read as mm."
                        ),
                    );
                    1.0
                }
            },
            None => {
                note_once(
                    assumed,
                    "A file without length-unit: lengths were read as mm.",
                );
                1.0
            }
        };
        let weight = match root
            .attr("weight-unit")
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("g") => 0.001,
            Some("t") => 1000.0,
            Some("lb" | "lbs") => 0.453_592,
            _ => 1.0,
        };
        Units { length, weight }
    }
}

impl Build {
    fn unknown(&mut self, kind: &str, name: &str) {
        *self
            .out
            .unrecognised
            .entry(format!("{kind}/{name}"))
            .or_default() += 1;
    }

    fn manufacturer(&mut self, el: &Node) {
        let mut records: Vec<&Node> = vec![el];
        if !el.children.is_empty() {
            records = el.children.iter().collect();
        }
        for r in records {
            let mut short = None;
            let mut long = None;
            for (k, v) in &r.attrs {
                let k = norm(k);
                if k.contains("short") || k == "id" || k == "code" {
                    short = Some(v.clone());
                } else if k.contains("long") || k == "name" || k == "manufacturername" {
                    long = Some(v.clone());
                }
            }
            if let (Some(s), Some(l)) = (short, long)
                && !s.is_empty()
                && !l.is_empty()
            {
                self.makers.insert(s.to_ascii_uppercase(), l);
            }
        }
    }

    fn property_definition(&mut self, el: &Node) {
        let mut records: Vec<&Node> = vec![el];
        if !el.children.is_empty() {
            records = el.children.iter().collect();
        }
        for r in records {
            let id = r.attr("id").or_else(|| r.attr("propertyid"));
            let name = r
                .attr("name")
                .or_else(|| r.attr("displayname"))
                .or_else(|| r.attr("text"));
            if let (Some(i), Some(n)) = (id, name) {
                self.prop_names.insert(i.to_owned(), n.to_owned());
            }
        }
    }

    fn part(&mut self, file: &str, el: &Node, units: &Units, default_maker: &str) {
        // Attributes of the part and its `property` children.
        let mut props: Vec<(String, String)> = el.attrs.clone();
        for c in &el.children {
            if matches!(c.name.as_str(), "property" | "prop" | "attribute") {
                let key = ["id", "propertyid", "name", "key"]
                    .iter()
                    .find_map(|a| c.attr(a));
                let value = c
                    .attr("value")
                    .map(str::to_owned)
                    .or_else(|| (!c.text.is_empty()).then(|| c.text.clone()));
                if let (Some(k), Some(v)) = (key, value) {
                    let k = self
                        .prop_names
                        .get(k)
                        .cloned()
                        .unwrap_or_else(|| k.to_owned());
                    props.push((k, v));
                }
            } else {
                self.unknown("part", &c.name);
            }
        }

        let mut f: BTreeMap<&'static str, String> = BTreeMap::new();
        let mut rest: Vec<(String, String)> = Vec::new();
        for (k, v) in props {
            if v.is_empty() {
                continue;
            }
            match part_field(&norm(&k)) {
                Some(name) => {
                    f.entry(name).or_insert(v);
                }
                None => rest.push((k, v)),
            }
        }

        // Identity.
        let partnr = f
            .get("partnr")
            .cloned()
            .or_else(|| f.get("order").cloned())
            .unwrap_or_else(|| stem_of(file));
        let (prefix, article) = split_id(&partnr);
        let order = f
            .get("order")
            .cloned()
            .unwrap_or_else(|| article.to_owned());
        if order.trim().is_empty() {
            self.out.skipped.push(format!(
                "{file} (line {}): the part has no part number",
                el.line
            ));
            return;
        }
        let maker = f
            .get("manufacturer")
            .map(|m| m.trim().to_owned())
            .filter(|m| !m.is_empty())
            .map(|m| {
                self.makers
                    .get(&m.to_ascii_uppercase())
                    .cloned()
                    .or_else(|| maker_for_code(&m))
                    .unwrap_or(m)
            })
            .or_else(|| prefix.and_then(|p| self.makers.get(&p.to_ascii_uppercase()).cloned()))
            .or_else(|| prefix.and_then(maker_for_code))
            .or_else(|| (!default_maker.trim().is_empty()).then(|| default_maker.trim().to_owned()))
            .or_else(|| prefix.map(str::to_owned));
        let Some(maker) = maker else {
            self.out.skipped.push(format!(
                "{file} (line {}): no manufacturer in the part, its number or the file name; choose one for the import",
                el.line
            ));
            return;
        };
        let maker = canonical_maker(&maker);
        if prefix.is_some() && !f.contains_key("manufacturer") && !default_maker.trim().is_empty() {
            note_once(
                &mut self.out.assumed,
                "Parts whose file names a maker code (like WAGO.…) and no manufacturer took the maker chosen for the import.",
            );
        }

        let mut part = CatalogPart::new(maker, order.trim());
        part.type_number = f.get("type").cloned().unwrap_or_default();

        // Text in several languages.
        let mut texts: Vec<(String, String)> = Vec::new();
        for key in ["descr1", "descr2", "descr3"] {
            if let Some(v) = f.get(key) {
                let t = multilingual(v);
                if key == "descr1" {
                    texts = t;
                } else if let Some((_, extra)) = preferred(&t) {
                    part.data
                        .insert(format!("eplan.{key}"), Value::Text(extra.clone()));
                }
            }
        }
        if let Some((chosen, text)) = preferred(&texts) {
            part.description = text.clone();
            for (lang, t) in &texts {
                if lang != chosen && !lang.is_empty() {
                    let short = lang
                        .split(['_', '-'])
                        .next()
                        .unwrap_or(lang)
                        .to_ascii_lowercase();
                    part.data
                        .insert(format!("description_{short}"), Value::Text(t.clone()));
                }
            }
        }

        // Identification.
        if let Some(g) = f.get("gtin") {
            let digits: String = g.chars().filter(char::is_ascii_digit).collect();
            if !digits.is_empty() {
                part.data.insert("gtin".into(), Value::Text(digits));
            }
        }
        if let Some(u) = f.get("datasheet") {
            part.data
                .insert("datasheet_url".into(), Value::Text(u.clone()));
        }
        if let Some(e) = f.get("eclass") {
            part.class.eclass = Some(e.trim().to_owned());
        }
        if let Some(e) = f.get("erp") {
            part.data
                .insert("eplan.erpnr".into(), Value::Text(e.clone()));
        }
        for key in ["picture", "macro", "productgroup"] {
            if let Some(v) = f.get(key) {
                part.data
                    .insert(format!("eplan.{key}"), Value::Text(v.clone()));
            }
        }
        part.data
            .insert("eplan.partnr".into(), Value::Text(partnr.clone()));

        // Size and weight.
        let dim = |k: &str| f.get(k).and_then(|v| length_mm(v, units.length));
        match (dim("width"), dim("height"), dim("depth")) {
            (Some(width_mm), Some(height_mm), Some(depth_mm)) => {
                part.dimensions = Some(Dimensions {
                    width_mm,
                    height_mm,
                    depth_mm,
                });
            }
            (w, h, d) => {
                for (key, v) in [("width_mm", w), ("height_mm", h), ("depth_mm", d)] {
                    if let Some(v) = v {
                        part.data.insert(key.into(), Value::Float(v));
                    }
                }
            }
        }
        if let Some(w) = f.get("weight").and_then(|v| {
            let q = parse_quantity(v)?;
            let f = match q.unit.as_str() {
                "" => units.weight,
                "kg" => 1.0,
                "g" => 0.001,
                _ => return None,
            };
            Some(q.low * f)
        }) {
            part.data.insert("weight_kg".into(), Value::Float(w));
        }

        // Electrical data and everything else.
        for (k, v) in rest {
            if !self.electrical(&mut part, &k, &v) {
                self.unknown("part", &k);
                part.data
                    .insert(format!("eplan.{}", key_text(&k)), Value::Text(v));
            }
        }

        let mut ids = BTreeSet::new();
        ids.insert(normalize_code(&partnr));
        ids.insert(normalize_code(&part.order_number));
        ids.insert(normalize_code(&stem_of(file)));
        let mut list_refs = BTreeSet::new();
        for (k, v) in &el.attrs {
            if norm(k).contains("accessor") {
                for name in v.split([';', ',']) {
                    let n = normalize_code(name.trim());
                    if !n.is_empty() {
                        list_refs.insert(n);
                    }
                }
            }
        }
        part.provenance = Some(Provenance {
            source: PORTAL_SOURCE.into(),
            file: file.to_owned(),
            imported: self.today.clone(),
            licence_note: "Maker data from the user's own file; not for redistribution.".into(),
        });
        let index = self.drafts.len();
        for id in &ids {
            self.by_id
                .entry(id.clone())
                .and_modify(|e| {
                    if *e != Some(index) {
                        *e = None;
                    }
                })
                .or_insert(Some(index));
        }
        self.drafts.push(Draft {
            part,
            list_refs,
            file: file.to_owned(),
        });
    }

    /// Maps an electrical value to a catalog key. Returns false when the
    /// attribute is not one it knows.
    fn electrical(&self, part: &mut CatalogPart, key: &str, value: &str) -> bool {
        let k = norm(key);
        let put = |part: &mut CatalogPart, name: &str, dim: Dim| -> bool {
            match quantity_in(value, dim) {
                Some((low, _)) => {
                    part.data.insert(name.into(), Value::Float(low));
                    part.sources.insert(name.into(), DataSource::MakerDatasheet);
                    true
                }
                None => false,
            }
        };
        if k.contains("coil") {
            let name = if k.contains("freq") || k.ends_with("hz") {
                // `50`, `50/60` or `50 Hz`: kept as the maker wrote it when
                // it is not one number.
                let t = value
                    .trim()
                    .trim_end_matches("Hz")
                    .trim_end_matches("hz")
                    .trim();
                if !t.chars().any(|c| c.is_ascii_digit()) {
                    return false;
                }
                let v = number(t)
                    .filter(|_| !t.contains('/'))
                    .map_or_else(|| Value::Text(t.to_owned()), Value::Float);
                part.data.insert(keys::COIL_HZ.into(), v);
                part.sources
                    .insert(keys::COIL_HZ.into(), DataSource::MakerDatasheet);
                return true;
            } else if k.contains("voltage") {
                eng::COIL_VOLTAGE_V
            } else if k.ends_with("type") || k.ends_with("kind") {
                // Only a current type (AC, DC, AC/DC, UC); `coilcurrent="20 mA"`
                // is a current, not a type.
                let t = value.trim();
                let up = t.to_ascii_uppercase();
                if !matches!(up.as_str(), "AC" | "DC" | "AC/DC" | "DC/AC" | "UC") {
                    return false;
                }
                part.data
                    .insert(keys::COIL_KIND.into(), Value::Text(t.to_owned()));
                return true;
            } else {
                return false;
            };
            return put(part, name, Dim::Voltage);
        }
        if matches!(
            k.as_str(),
            "poles" | "numberofpoles" | "polecount" | "polenumber"
        ) {
            return match count(value) {
                Some(n) => {
                    part.data
                        .insert(eng::POLES.into(), Value::Int(i64::from(n)));
                    part.sources
                        .insert(eng::POLES.into(), DataSource::MakerDatasheet);
                    true
                }
                None => false,
            };
        }
        if k.contains("contact") {
            let aux = k.contains("aux");
            // Ratings such as `auxcontactsbreakingcapacity="10 A"` are not counts.
            let rating = ["capacity", "current", "rating", "voltage"]
                .iter()
                .any(|w| k.contains(w));
            let nc = !rating && (k.ends_with("nc") || k.ends_with("normallyclosed"));
            let no = !rating && (k.ends_with("no") || k.ends_with("normallyopen"));
            let name = match (aux, no, nc) {
                (true, true, false) => Some(keys::AUX_NO),
                (true, false, true) => Some(keys::AUX_NC),
                (false, true, false) if k.contains("main") || !k.contains("aux") => {
                    Some(keys::MAIN_NO)
                }
                _ => None,
            };
            // A count carries no unit.
            let plain = value.trim().chars().all(|c| c.is_ascii_digit());
            return match (name, count(value).filter(|_| plain)) {
                (Some(name), Some(n)) => {
                    part.data.insert(name.into(), Value::Int(i64::from(n)));
                    part.sources.insert(name.into(), DataSource::MakerDatasheet);
                    true
                }
                // `1 NO + 1 NC` in one value: the description reads that text.
                (None, _) if aux && !rating && !value.trim().is_empty() => {
                    part.data.insert(
                        "auxiliary_contacts".into(),
                        Value::Text(value.trim().to_owned()),
                    );
                    true
                }
                _ => false,
            };
        }
        if k.contains("powerdissipation") || k.contains("powerloss") {
            return put(part, eng::POWER_LOSS_W, Dim::PowerW);
        }
        if k.contains("current") && (k.contains("iec") || k.contains("rated")) {
            return put(part, eng::RATED_CURRENT_A, Dim::Current);
        }
        if k.contains("voltage") && (k.contains("rated") || k.contains("iec")) {
            return put(part, "rated_voltage_v", Dim::Voltage);
        }
        if (k.contains("crosssection") || k.contains("conductorsection")) && !k.contains("awg") {
            return match bound(&k) {
                Some(Bound::Min) => put(part, "section_min_mm2", Dim::Area),
                Some(Bound::Max) => put(part, "section_max_mm2", Dim::Area),
                None => false,
            };
        }
        false
    }

    /// The draft a companion file belongs to.
    fn owner(&self, ids: &[String]) -> Option<usize> {
        for id in ids {
            let n = normalize_code(id);
            if let Some(Some(i)) = self.by_id.get(&n) {
                return Some(*i);
            }
            let (_, rest) = split_id(id);
            if let Some(Some(i)) = self.by_id.get(&normalize_code(rest)) {
                return Some(*i);
            }
        }
        None
    }

    fn owner_ids(file: &str, header: &Node) -> Vec<String> {
        let mut ids: Vec<String> = header
            .attrs
            .iter()
            .filter(|(k, _)| {
                matches!(
                    norm(k).as_str(),
                    "partnr"
                        | "partnumber"
                        | "part"
                        | "partid"
                        | "partname"
                        | "article"
                        | "ordernr"
                        | "reference"
                )
            })
            .map(|(_, v)| v.clone())
            .collect();
        ids.push(stem_of(file));
        ids
    }

    fn connection_points(&mut self, file: &str, el: &Node, units: &Units) {
        let ids = Self::owner_ids(file, el);
        let mut records: Vec<&Node> = Vec::new();
        if el.children.is_empty() {
            records.push(el);
        } else {
            el.leaves(&mut records);
        }
        let mut terminals = Vec::new();
        for r in records {
            if let Some(t) = self.terminal(r, units) {
                terminals.push(t);
            }
        }
        let Some(i) = self.owner(&ids) else {
            self.unattached(file, Kind::ConnectionPoints, &ids);
            return;
        };
        let draft = &mut self.drafts[i];
        // A second file for the same part replaces nothing: names that are
        // already there are skipped.
        for t in terminals {
            if draft.part.terminals.iter().any(|x| x.name == t.name) {
                continue;
            }
            draft.part.terminals.push(t);
        }
        // Estimated, not the maker's, until the reader has met a real maker
        // file: the wire checks then only warn about a too-large wire.
        if draft.part.terminals.iter().any(|t| t.wire_mm2.is_some()) {
            draft
                .part
                .sources
                .insert("connection_data".into(), DataSource::Estimated);
        }
    }

    fn terminal(&mut self, r: &Node, units: &Units) -> Option<PartTerminal> {
        let mut t = PartTerminal::default();
        let mut pos: [Option<f64>; 3] = [None; 3];
        let mut dir: [Option<f64>; 3] = [None; 3];
        let (mut s_lo, mut s_hi): (Option<f64>, Option<f64>) = (None, None);
        let (mut a_lo, mut a_hi): (Option<f64>, Option<f64>) = (None, None);
        let (mut t_lo, mut t_hi): (Option<f64>, Option<f64>) = (None, None);
        for (key, v) in &r.attrs {
            if v.is_empty() {
                continue;
            }
            let k = norm(key);
            let axis = |c: char| match c {
                'x' => 0,
                'y' => 1,
                _ => 2,
            };
            match terminal_field(&k) {
                Some(TField::Name) => t.name = v.clone(),
                Some(TField::Pos(c)) => pos[axis(c)] = length_mm(v, units.length),
                Some(TField::Dir(c)) => dir[axis(c)] = number(v),
                Some(TField::Strip) => t.strip_mm = length_mm(v, units.length),
                Some(TField::Torque) => {
                    if let Some((lo, hi)) = torque_nm(v) {
                        match bound(&k) {
                            Some(Bound::Min) => t_lo = Some(lo),
                            Some(Bound::Max) => t_hi = Some(hi),
                            None => {
                                t_lo = Some(lo);
                                t_hi = Some(hi);
                            }
                        }
                    }
                }
                Some(TField::Awg) => {
                    if let Some(q) = parse_quantity(v) {
                        match bound(&k) {
                            Some(Bound::Min) => a_lo = Some(q.low),
                            Some(Bound::Max) => a_hi = Some(q.high),
                            None => {
                                a_lo = Some(q.low);
                                a_hi = Some(q.high);
                            }
                        }
                    }
                }
                Some(TField::Section) => {
                    if let Some((lo, hi)) = quantity_in(v, Dim::Area) {
                        match bound(&k) {
                            Some(Bound::Min) => s_lo = Some(lo),
                            Some(Bound::Max) => s_hi = Some(hi),
                            None => {
                                s_lo = Some(lo);
                                s_hi = Some(hi);
                            }
                        }
                    }
                }
                Some(TField::WireCount) => {
                    t.max_wires = count(v).and_then(|n| u8::try_from(n).ok())
                }
                Some(TField::Kind) => t.kind = connection_kind(v),
                None => {
                    self.unknown("connection point", key);
                    t.extra.insert(key.clone(), v.clone());
                }
            }
        }
        if t.name.is_empty() {
            return None;
        }
        if let [Some(x), Some(y), Some(z)] = pos {
            t.maker_position_mm = Some([x, y, z]);
        } else if let [Some(x), Some(y), None] = pos {
            t.maker_position_mm = Some([x, y, 0.0]);
        }
        if let [Some(x), Some(y), Some(z)] = dir {
            t.direction = Some([x, y, z]);
        }
        t.wire_mm2 = match (s_lo, s_hi) {
            (Some(a), Some(b)) => Some((a.min(b), a.max(b))),
            (Some(a), None) | (None, Some(a)) => Some((a, a)),
            _ => None,
        };
        t.awg = match (a_lo, a_hi) {
            (Some(a), Some(b)) => Some((a, b)),
            (Some(a), None) | (None, Some(a)) => Some((a, a)),
            _ => None,
        };
        t.torque_nm = match (t_lo, t_hi) {
            (Some(a), Some(b)) => Some((a.min(b), a.max(b))),
            (Some(a), None) | (None, Some(a)) => Some((a, a)),
            _ => None,
        };
        Some(t)
    }

    fn construction(&mut self, file: &str, el: &Node, units: &Units) {
        let ids = Self::owner_ids(file, el);
        let mut records: Vec<&Node> = Vec::new();
        if el.children.is_empty() {
            records.push(el);
        } else {
            el.leaves(&mut records);
        }
        let mut holes = Vec::new();
        for r in records {
            match self.hole(r, units) {
                Some(h) => holes.push(h),
                None => {
                    for (k, _) in &r.attrs {
                        self.unknown("construction", k);
                    }
                }
            }
        }
        let Some(i) = self.owner(&ids) else {
            self.unattached(file, Kind::Construction, &ids);
            return;
        };
        let draft = &mut self.drafts[i];
        if draft.part.drilling.is_empty() && !holes.is_empty() {
            draft.part.drilling = holes;
            // The hand drilling plan labels these rows as the maker's data.
            draft
                .part
                .data
                .insert("drill_basis".into(), Value::Text("maker".into()));
        }
    }

    fn hole(&mut self, r: &Node, units: &Units) -> Option<DrillHole> {
        let mut h = DrillHole::default();
        let (mut x, mut y) = (None, None);
        let (mut count_n, mut dx, mut dy) = (None, None, None);
        let mut any_spacing = false;
        for (key, v) in &r.attrs {
            if v.is_empty() {
                continue;
            }
            let k = norm(key);
            if matches!(k.as_str(), "x" | "posx" | "xpos" | "positionx") {
                x = length_mm(v, units.length);
            } else if matches!(k.as_str(), "y" | "posy" | "ypos" | "positiony") {
                y = length_mm(v, units.length);
            } else if k.contains("diameter") || matches!(k.as_str(), "d" | "dia" | "drill") {
                h.diameter_mm = length_mm(v, units.length);
            } else if matches!(
                k.as_str(),
                "type" | "kind" | "holetype" | "drilltype" | "name"
            ) {
                h.kind = v.clone();
            } else if (k.contains("repeat")
                || k.contains("distance")
                || k.contains("spacing")
                || k.contains("pitch")
                || k.contains("step"))
                && (k.ends_with('x') || k.contains("x"))
                && !k.contains("count")
                && !k.contains("number")
            {
                dx = length_mm(v, units.length);
                any_spacing = true;
            } else if (k.contains("repeat")
                || k.contains("distance")
                || k.contains("spacing")
                || k.contains("pitch")
                || k.contains("step"))
                && k.contains('y')
                && !k.contains("count")
                && !k.contains("number")
            {
                dy = length_mm(v, units.length);
                any_spacing = true;
            } else if k.contains("repeat")
                || k.contains("count")
                || k.contains("number")
                || k.contains("quantity")
            {
                count_n = count(v);
            } else {
                self.unknown("construction", key);
            }
        }
        let (x, y) = (x?, y?);
        h.x_mm = x;
        h.y_mm = y;
        if let Some(n) = count_n.filter(|n| *n > 1) {
            if n > MAX_REPEAT {
                self.unknown("construction", "repeat count over 1000");
            } else {
                h.repeat = Some(DrillRepeat {
                    count: n,
                    dx_mm: dx.unwrap_or(0.0),
                    dy_mm: dy.unwrap_or(0.0),
                });
            }
        } else if any_spacing && count_n.is_none() {
            self.unknown("construction", "repeat spacing without a count");
        }
        Some(h)
    }

    fn accessory_list(&mut self, file: &str, el: &Node) {
        let ids = Self::owner_ids(file, el);
        let mut records: Vec<&Node> = Vec::new();
        if el.children.is_empty() {
            records.push(el);
        } else {
            el.leaves(&mut records);
        }
        let mut found: Vec<Accessory> = Vec::new();
        for r in records {
            if let Some(a) = self.accessory(r, &ids) {
                found.push(a);
            }
        }
        // Lists the parts name, then the list that carries the part's own name.
        let list_names: Vec<String> = ids.iter().map(|i| normalize_code(i)).collect();
        let mut owners: Vec<usize> = self
            .drafts
            .iter()
            .enumerate()
            .filter(|(_, d)| list_names.iter().any(|n| d.list_refs.contains(n)))
            .map(|(i, _)| i)
            .collect();
        if owners.is_empty()
            && let Some(i) = self.owner(&ids)
        {
            owners.push(i);
        }
        if owners.is_empty() {
            self.unattached(file, Kind::AccessoryList, &ids);
            return;
        }
        for i in owners {
            let draft = &mut self.drafts[i];
            for a in &found {
                if !draft.part.accessories.iter().any(|x| x.part == a.part) {
                    draft.part.accessories.push(a.clone());
                }
            }
        }
    }

    fn accessory(&mut self, r: &Node, owner_ids: &[String]) -> Option<Accessory> {
        let mut number = None;
        let mut maker = None;
        let mut quantity = None;
        for (key, v) in &r.attrs {
            let k = norm(key);
            if matches!(
                k.as_str(),
                "partnr"
                    | "partnumber"
                    | "part"
                    | "partid"
                    | "accessory"
                    | "accessorypartnr"
                    | "article"
                    | "ordernr"
            ) {
                number = Some(v.clone());
            } else if k.contains("manufacturer") {
                maker = Some(v.clone());
            } else if matches!(k.as_str(), "quantity" | "count" | "number" | "amount") {
                quantity = count(v);
            } else {
                self.unknown("accessory", key);
            }
        }
        let number = number.filter(|n| !n.is_empty())?;
        let (prefix, article) = split_id(&number);
        let own_prefix = owner_ids
            .iter()
            .find_map(|o| split_id(o).0.map(str::to_owned));
        let manufacturer = maker
            .or_else(|| prefix.and_then(maker_for_code))
            .or_else(|| prefix.map(str::to_owned))
            .or_else(|| own_prefix.and_then(|p| maker_for_code(&p).or(Some(p))))
            .map(|m| canonical_maker(&m))
            .unwrap_or_default();
        Some(Accessory {
            part: PartRef {
                manufacturer,
                order_number: article.to_owned(),
            },
            quantity,
            offset_mm: None,
        })
    }

    fn placement(&mut self, file: &str, el: &Node, units: &Units) {
        let ids = Self::owner_ids(file, el);
        let mut records: Vec<&Node> = Vec::new();
        if el.children.is_empty() {
            records.push(el);
        } else {
            el.leaves(&mut records);
        }
        let Some(i) = self.owner(&ids) else {
            self.unattached(file, Kind::AccessoryPlacement, &ids);
            return;
        };
        let mut offsets: Vec<(String, [f64; 3])> = Vec::new();
        for r in records {
            let mut number = None;
            let mut off = [0.0f64; 3];
            let mut any = false;
            for (key, v) in &r.attrs {
                let k = norm(key);
                match k.as_str() {
                    "partnr" | "partnumber" | "part" | "accessory" | "accessorypartnr"
                    | "article" => {
                        number = Some(v.clone());
                    }
                    "x" | "dx" | "spacingx" | "distancex" => {
                        off[0] = length_mm(v, units.length).unwrap_or(0.0);
                        any = true;
                    }
                    "y" | "dy" | "spacingy" | "distancey" => {
                        off[1] = length_mm(v, units.length).unwrap_or(0.0);
                        any = true;
                    }
                    "z" | "dz" | "spacingz" | "distancez" => {
                        off[2] = length_mm(v, units.length).unwrap_or(0.0);
                        any = true;
                    }
                    _ => self.unknown("accessory placement", key),
                }
            }
            if let (Some(n), true) = (number, any) {
                offsets.push((n, off));
            }
        }
        let draft = &mut self.drafts[i];
        for (n, off) in offsets {
            let (_, article) = split_id(&n);
            let want = normalize_code(article);
            if let Some(a) = draft
                .part
                .accessories
                .iter_mut()
                .find(|a| normalize_code(&a.part.order_number) == want)
            {
                a.offset_mm = Some(off);
            }
        }
    }

    fn unattached(&mut self, file: &str, kind: Kind, ids: &[String]) {
        let who = ids.last().cloned().unwrap_or_default();
        self.out.unattached.push(format!(
            "{file}: {} for \"{who}\", but no part with that number is among the files (pick its part file too)",
            kind.label()
        ));
    }

    fn finish(mut self) -> PortalRead {
        let mut out = std::mem::take(&mut self.out);
        for d in &self.drafts {
            let p = &d.part;
            out.counts.parts += 1;
            if !p.terminals.is_empty() {
                out.counts.parts_with_terminals += 1;
                out.counts.terminals += p.terminals.len();
                out.counts.with_wire_range +=
                    p.terminals.iter().filter(|t| t.wire_mm2.is_some()).count();
                out.counts.with_torque +=
                    p.terminals.iter().filter(|t| t.torque_nm.is_some()).count();
            }
            if !p.drilling.is_empty() {
                out.counts.parts_with_drilling += 1;
                out.counts.holes += p
                    .drilling
                    .iter()
                    .map(|h| h.positions().len())
                    .sum::<usize>();
            }
            if !p.accessories.is_empty() {
                out.counts.parts_with_accessories += 1;
                out.counts.accessories += p.accessories.len();
            }
        }
        if out.counts.terminals > 0 || out.counts.holes > 0 {
            note_once(
                &mut out.assumed,
                "Connection point and hole positions are kept in the maker's own frame, as written. The corner they are measured from is not known, so wire routing does not use them yet.",
            );
        }
        if out.counts.terminals > 0 {
            note_once(
                &mut out.assumed,
                "The side a terminal faces is not worked out from its direction; the direction is kept as written.",
            );
        }
        // Parts with the same key from two files: the later file wins.
        let mut seen: HashMap<PartKey, usize> = HashMap::new();
        let mut parts: Vec<CatalogPart> = Vec::new();
        let mut dupes: Vec<String> = Vec::new();
        for d in self.drafts {
            let key = d.part.key();
            if let Some(&at) = seen.get(&key) {
                dupes.push(format!(
                    "{} {}: in {} and in {}; the later file was used",
                    d.part.manufacturer,
                    d.part.order_number,
                    parts[at]
                        .provenance
                        .as_ref()
                        .map_or("", |p| p.file.as_str()),
                    d.file
                ));
                parts[at] = d.part;
            } else {
                seen.insert(key, parts.len());
                parts.push(d.part);
            }
        }
        for d in dupes.iter().take(NOTES_PER_KIND) {
            out.skipped.push(d.clone());
        }
        if dupes.len() > NOTES_PER_KIND {
            out.skipped.push(format!(
                "and {} more parts in more than one file",
                dupes.len() - NOTES_PER_KIND
            ));
        }
        out.counts.parts = parts.len();
        out.parts = parts;
        out
    }
}

/// An attribute name as a catalog key.
fn key_text(k: &str) -> String {
    k.trim_start_matches("P_ARTICLE_")
        .trim_start_matches("p_article_")
        .to_lowercase()
}

fn part_field(k: &str) -> Option<&'static str> {
    Some(match k {
        "partnr" | "partnumber" | "partno" | "id" => "partnr",
        "ordernr" | "ordernumber" | "orderno" | "bestellnummer" => "order",
        "typenr" | "typenumber" | "type" | "typecode" | "typenummer" => "type",
        "erpnr" | "erpnumber" => "erp",
        "manufacturer" | "manufacturername" | "hersteller" => "manufacturer",
        "descr1" | "description1" | "description" | "designation" => "descr1",
        "descr2" | "description2" => "descr2",
        "descr3" | "description3" => "descr3",
        "ean" | "eannr" | "gtin" | "eancode" => "gtin",
        "width" => "width",
        "height" => "height",
        "depth" => "depth",
        "weight" => "weight",
        "datasheet" | "datasheeturl" | "datasheetlink" | "hyperlink" | "link" => "datasheet",
        "picturefile" | "picture" | "pictureshortname" => "picture",
        "macrofile" | "macro" | "macroname" => "macro",
        "producttopgroup" | "productgroup" | "productsubgroup" => "productgroup",
        _ if k.contains("eclass") || k.contains("eclss") => "eclass",
        _ => return None,
    })
}

#[derive(Clone, Copy)]
enum TField {
    Name,
    Pos(char),
    Dir(char),
    Strip,
    Torque,
    Awg,
    Section,
    WireCount,
    Kind,
}

fn terminal_field(k: &str) -> Option<TField> {
    let pos = |c: char| {
        (k == c.to_string()
            || k == format!("pos{c}")
            || k == format!("{c}pos")
            || k == format!("position{c}"))
        .then_some(TField::Pos(c))
    };
    let dir = |c: char| {
        (k == format!("dir{c}")
            || k == format!("{c}dir")
            || k == format!("direction{c}")
            || k == format!("{c}direction"))
        .then_some(TField::Dir(c))
    };
    for c in ['x', 'y', 'z'] {
        if let Some(t) = pos(c).or_else(|| dir(c)) {
            return Some(t);
        }
    }
    if matches!(
        k,
        "name"
            | "designation"
            | "connectionpointdesignation"
            | "connectiondesignation"
            | "pin"
            | "pinnumber"
            | "marking"
            | "label"
            | "terminalname"
    ) {
        return Some(TField::Name);
    }
    if k.contains("strip") {
        return Some(TField::Strip);
    }
    if k.contains("torque") {
        return Some(TField::Torque);
    }
    if k.contains("awg") {
        return Some(TField::Awg);
    }
    if k.contains("crosssection") || k.contains("wirerange") || k.contains("conductorsection") {
        return Some(TField::Section);
    }
    if (k.contains("wire") || k.contains("conductor"))
        && (k.contains("count") || k.contains("number"))
    {
        return Some(TField::WireCount);
    }
    if matches!(
        k,
        "typeofterminal"
            | "terminaltype"
            | "connectiontype"
            | "clamptype"
            | "connectiontechnology"
            | "type"
    ) {
        return Some(TField::Kind);
    }
    None
}

/// Torque in N·m. A value without a unit is N·m.
fn torque_nm(value: &str) -> Option<(f64, f64)> {
    let q = parse_quantity(value)?;
    let f = match q.unit.as_str() {
        "" | "nm" => 1.0,
        "ncm" => 0.01,
        "nmm" => 0.001,
        "lbin" | "lb.in" | "lbfin" | "in-lb" | "inlb" => 0.112_985,
        _ => return None,
    };
    Some((q.low * f, q.high * f))
}

fn connection_kind(v: &str) -> ConnectionKind {
    let t = v.to_ascii_lowercase();
    if t.contains("push") {
        ConnectionKind::PushIn
    } else if t.contains("spring")
        || t.contains("cage")
        || t.contains("clamp") && t.contains("tension")
    {
        ConnectionKind::Spring
    } else if t.contains("screwless") {
        ConnectionKind::Screwless
    } else if t.contains("screw") {
        ConnectionKind::Screw
    } else if t.contains("faston") || t.contains("flat") {
        ConnectionKind::Faston
    } else if t.contains("solder") {
        ConnectionKind::SolderPin
    } else if t.contains("lug") || t.contains("ring") {
        ConnectionKind::Lug
    } else if t.contains("pin") {
        ConnectionKind::Pin
    } else {
        ConnectionKind::Unknown
    }
}

// Lets a part found by a `PartKey` pick up what the import adds without
// losing what is already in the catalog.
impl CatalogPart {
    /// This part with what `incoming` (maker parts XML) knows laid over it:
    /// text, size, terminals, drilling and accessories from the file win; the
    /// price, lifecycle, classification, files and alternatives already here
    /// stay.
    pub fn with_maker_xml(&self, incoming: &CatalogPart) -> CatalogPart {
        let mut out = self.clone();
        if !incoming.type_number.is_empty() {
            out.type_number = incoming.type_number.clone();
        }
        if !incoming.description.is_empty() {
            out.description = incoming.description.clone();
        }
        if incoming.dimensions.is_some() {
            out.dimensions = incoming.dimensions;
        }
        for (k, v) in &incoming.data {
            out.data.insert(k.clone(), v.clone());
        }
        for (k, v) in &incoming.sources {
            out.sources.insert(k.clone(), *v);
        }
        if incoming.class.eclass.is_some() {
            out.class.eclass = incoming.class.eclass.clone();
        }
        if !incoming.terminals.is_empty() {
            out.terminals = incoming.terminals.clone();
        }
        if !incoming.drilling.is_empty() {
            out.drilling = incoming.drilling.clone();
        }
        if !incoming.accessories.is_empty() {
            out.accessories = incoming.accessories.clone();
        }
        if incoming.provenance.is_some() {
            out.provenance = incoming.provenance.clone();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_and_semicolons_in_text() {
        let t = multilingual("de_DE@Schütz;en_US@Contactor; 3-pole;nl_NL@Contactor");
        assert_eq!(t.len(), 3);
        assert_eq!(t[1], ("en_US".into(), "Contactor; 3-pole".into()));
        assert_eq!(preferred(&t).unwrap().0, "en_US");
        let plain = multilingual("Just text");
        assert_eq!(plain, vec![(String::new(), "Just text".into())]);
    }

    #[test]
    fn ids_and_stems() {
        assert_eq!(split_id("WAGO.2002-1201"), (Some("WAGO"), "2002-1201"));
        assert_eq!(split_id("RIT.SK3305.500"), (Some("RIT"), "SK3305.500"));
        assert_eq!(split_id("SK3305.500"), (None, "SK3305.500"));
        assert_eq!(
            stem_of("WAGO.2002-1201.connectionpoints.xml"),
            "WAGO.2002-1201"
        );
        assert_eq!(stem_of("C:\\x\\PXC.3044076.part.xml"), "PXC.3044076");
        assert_eq!(stem_of("plain.xml"), "plain");
    }

    #[test]
    fn range_words() {
        assert_eq!(bound("crosssectionmin"), Some(Bound::Min));
        assert_eq!(bound("torqueto"), Some(Bound::Max));
        assert_eq!(bound("torque"), None);
    }

    #[test]
    fn torque_units() {
        assert_eq!(torque_nm("0,5 Nm"), Some((0.5, 0.5)));
        assert_eq!(torque_nm("50 Ncm"), Some((0.5, 0.5)));
        assert_eq!(torque_nm("2 foo"), None);
    }
}
