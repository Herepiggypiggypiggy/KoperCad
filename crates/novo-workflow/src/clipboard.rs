//! Copy, cut and paste (Ctrl+C, Ctrl+X, Ctrl+V; docs/design/clipboard.md).
//!
//! [`Clip::capture`] takes a snapshot of a selection: the symbols, junctions,
//! wires between them, texts, and the devices, terminals, parts and
//! potentials they refer to. The snapshot is plain data, so it can be put on
//! the system clipboard as versioned JSON text ([`Clip::to_text`],
//! [`Clip::parse`]) and pasted on another page or in another project.
//! [`paste`] inserts it into the caller's transaction, one undo step.
//!
//! Tags on paste:
//!
//! - [`TagMode::NextFree`] gives every copied device the next free number of
//!   its letter code in the structure of the page it lands on, so a
//!   contactor pasted on a page of another location becomes `=A2+CAB2-K1`
//!   when that is the next free tag there. A device never takes a tag that
//!   is in use.
//! - [`TagMode::KeepIfFree`] keeps the original tag when no device or cable
//!   has it, else falls back to the next free one (Ctrl+Shift+V, and a
//!   plain paste after a cut, which moves the circuit with its tags).
//! - [`TagMode::Keep`] keeps every tag, duplicates included.
//!
//! A device whose main function stays behind (a lone contact copied without
//! its coil) stays a contact of that device when the paste is in the same
//! project and the device is still there; elsewhere it becomes a device of
//! its own, because the coil it belongs to is not in the target.
//!
//! Wires keep their connectivity inside the paste: an end on a copied pin or
//! junction follows the copy. Wire numbers are not pasted (they are
//! numbered again); colours and cross-sections are. Terminals land on the
//! strip of the same tag, which is created when the target has none.
//! Potentials and parts are matched by name (potential) and by manufacturer
//! and order number (part) and added when missing.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    AnyId, Device, DeviceId, DeviceKind, DeviceTag, Endpoint, Junction, ModelError, PageId, Part,
    PartId, Point, Potential, PotentialId, Project, Result, SymbolInstance, SymbolRole, Terminal,
    TerminalId, Text, Tx, Wire, split_code,
};
use serde::{Deserialize, Serialize};

use crate::duplicate::{Duplicated, Selection, TagMode};

/// Names the clipboard text, so other text is never taken for a clip.
pub const FORMAT: &str = "kopercad-clipboard";
/// The newest clip format this build writes and reads.
pub const VERSION: u32 = 1;

/// A device a clip refers to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipDevice {
    pub device: Device,
    /// The device gets a copy (its main function is in the clip). `false`:
    /// the clip only holds contacts of it.
    pub copy: bool,
}

/// A snapshot of a selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub format: String,
    pub version: u32,
    /// Which project it came from, to tell a paste in place from a paste
    /// elsewhere. Chosen by the caller.
    pub source: String,
    /// Cut rather than copied: a paste moves the circuit and keeps its tags.
    #[serde(default)]
    pub cut: bool,
    /// The structure of the page it was taken from.
    #[serde(default)]
    pub structure: DeviceTag,
    /// The top-left corner of the drawn objects: where the pointer holds
    /// the paste.
    pub origin: Point,
    pub devices: Vec<ClipDevice>,
    #[serde(default)]
    pub strips: Vec<Device>,
    #[serde(default)]
    pub terminals: Vec<Terminal>,
    #[serde(default)]
    pub parts: Vec<Part>,
    #[serde(default)]
    pub potentials: Vec<Potential>,
    pub symbols: Vec<SymbolInstance>,
    pub junctions: Vec<Junction>,
    pub wires: Vec<Wire>,
    /// Texts attached to the objects above, and free texts.
    pub texts: Vec<Text>,
}

/// What the text on the system clipboard is.
#[derive(Debug, Clone, PartialEq)]
pub enum Parsed {
    Clip(Box<Clip>),
    /// A clip written by a newer version of the program.
    TooNew(u32),
    /// Some other text.
    NotOurs,
}

/// What a paste did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pasted {
    pub copied: Duplicated,
    /// Devices whose tag changed: original and new.
    pub renamed: Vec<(DeviceTag, DeviceTag)>,
    /// Devices pasted whose original tag was kept.
    pub kept: Vec<DeviceTag>,
    /// Contacts whose device is not in the target and that got a device of
    /// their own, with its new tag.
    pub orphans: Vec<DeviceTag>,
}

impl Pasted {
    pub fn new_ids(&self) -> Vec<AnyId> {
        self.copied.new_ids()
    }
}

impl Clip {
    /// Snapshots `items` of `project`. `None` when nothing in the selection
    /// can be copied.
    pub fn capture(project: &Project, items: &[AnyId], source: &str, cut: bool) -> Option<Clip> {
        let selection = Selection::gather(project, items);
        let symbols = selection.symbols(project);
        let junctions = selection.junctions(project);
        let wires = selection.wires(project);
        let free_texts = selection.free_texts(project);
        if symbols.is_empty() && junctions.is_empty() && wires.is_empty() && free_texts.is_empty() {
            return None;
        }
        let to_copy = selection.devices_to_copy(project);

        let mut devices: BTreeMap<DeviceId, ClipDevice> = BTreeMap::new();
        let mut terminal_ids: BTreeSet<TerminalId> = BTreeSet::new();
        let mut potentials: BTreeSet<PotentialId> = BTreeSet::new();
        for s in &symbols {
            match &s.role {
                SymbolRole::Device { device, .. } => {
                    if let Some(d) = project.device(*device) {
                        devices.entry(d.id).or_insert_with(|| ClipDevice {
                            device: d.clone(),
                            copy: to_copy.contains(&d.id),
                        });
                    }
                }
                SymbolRole::Terminal { terminals } => terminal_ids.extend(terminals),
                SymbolRole::PotentialFeed { potential } => {
                    potentials.insert(*potential);
                }
                _ => {}
            }
            potentials.extend(s.feeds.iter().map(|f| f.potential));
        }
        potentials.extend(wires.iter().filter_map(|w| w.potential));
        let terminals: Vec<Terminal> = terminal_ids
            .iter()
            .filter_map(|&t| project.terminal(t).cloned())
            .collect();
        potentials.extend(terminals.iter().filter_map(|t| t.feed));
        let mut strips: BTreeMap<DeviceId, Device> = BTreeMap::new();
        for t in &terminals {
            if let Some(d) = project.device(t.strip) {
                strips.insert(d.id, d.clone());
            }
        }
        let mut part_ids: BTreeSet<PartId> = BTreeSet::new();
        for d in devices.values().map(|d| &d.device).chain(strips.values()) {
            part_ids.extend(&d.parts);
        }
        part_ids.extend(terminals.iter().filter_map(|t| t.part));

        let mut texts: Vec<Text> = Vec::new();
        let attached: Vec<AnyId> = symbols
            .iter()
            .map(|s| AnyId::from(s.id))
            .chain(junctions.iter().map(|j| AnyId::from(j.id)))
            .chain(wires.iter().map(|w| AnyId::from(w.id)))
            .collect();
        for owner in attached {
            texts.extend(
                project
                    .texts_of(owner)
                    .into_iter()
                    .filter_map(|id| project.text(id).cloned()),
            );
        }
        texts.extend(free_texts);

        let page = symbols
            .first()
            .map(|s| s.page)
            .or_else(|| junctions.first().map(|j| j.page))
            .or_else(|| wires.first().map(|w| w.page))
            .or_else(|| texts.first().map(|t| t.page));
        let structure = page
            .and_then(|p| project.page(p))
            .map(|p| p.structure.clone())
            .unwrap_or_default();

        let mut clip = Clip {
            format: FORMAT.to_owned(),
            version: VERSION,
            source: source.to_owned(),
            cut,
            structure,
            origin: Point::ZERO,
            devices: devices.into_values().collect(),
            strips: strips.into_values().collect(),
            terminals,
            parts: part_ids
                .iter()
                .filter_map(|&p| project.part(p).cloned())
                .collect(),
            potentials: potentials
                .iter()
                .filter_map(|&p| project.potential(p).cloned())
                .collect(),
            symbols,
            junctions,
            wires,
            texts,
        };
        clip.origin = clip.corner();
        Some(clip)
    }

    /// The top-left corner of what is drawn.
    fn corner(&self) -> Point {
        let mut points: Vec<Point> = self
            .symbols
            .iter()
            .map(|s| s.placement.origin)
            .chain(self.junctions.iter().map(|j| j.position))
            .collect();
        if points.is_empty() {
            for w in &self.wires {
                points.extend(w.route.iter().copied());
                for end in w.ends() {
                    if let Endpoint::Free { at } = end {
                        points.push(at);
                    }
                }
            }
        }
        if points.is_empty() {
            points.extend(self.texts.iter().map(|t| t.placement.offset));
        }
        let x = points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let y = points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
        if x.is_finite() && y.is_finite() {
            Point::new(x, y)
        } else {
            Point::ZERO
        }
    }

    /// How many objects the clip holds, for the status line.
    pub fn len(&self) -> usize {
        self.symbols.len()
            + self.junctions.len()
            + self.wires.len()
            + self
                .texts
                .iter()
                .filter(|t| t.attached_to.is_none())
                .count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The symbols as drawn at their source, and one polyline per wire, for
    /// the ghost that follows the pointer. Add `pointer - origin` to place.
    pub fn outline(&self) -> (Vec<SymbolInstance>, Vec<Vec<Point>>) {
        let position = |end: Endpoint| match end {
            Endpoint::Free { at } => Some(at),
            Endpoint::Pin { symbol, pin } => self
                .symbols
                .iter()
                .find(|s| s.id == symbol)
                .and_then(|s| s.pin_position(pin)),
            Endpoint::Junction { junction } => self
                .junctions
                .iter()
                .find(|j| j.id == junction)
                .map(|j| j.position),
        };
        let paths = self
            .wires
            .iter()
            .filter_map(|w| {
                let mut path = vec![position(w.from)?];
                path.extend(w.route.iter().copied());
                path.push(position(w.to)?);
                Some(path)
            })
            .collect();
        (self.symbols.clone(), paths)
    }

    /// The clipboard text: the clip as JSON.
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Reads clipboard text. Anything that is not a clip is `NotOurs`.
    pub fn parse(text: &str) -> Parsed {
        let text = text.trim();
        if !text.starts_with('{') || !text.contains(FORMAT) {
            return Parsed::NotOurs;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
            return Parsed::NotOurs;
        };
        if value.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
            return Parsed::NotOurs;
        }
        let version = value
            .get("version")
            .and_then(|v| v.as_u64())
            .unwrap_or(u64::MAX);
        if version > u64::from(VERSION) {
            return Parsed::TooNew(u32::try_from(version).unwrap_or(u32::MAX));
        }
        match serde_json::from_value::<Clip>(value) {
            Ok(clip) => Parsed::Clip(Box::new(clip)),
            Err(_) => Parsed::NotOurs,
        }
    }
}

fn tag_in_use(project: &Project, tag: &DeviceTag) -> bool {
    project.devices().any(|d| &d.tag == tag) || project.cables().any(|c| &c.tag == tag)
}

/// The tag a copied device takes.
fn tag_for(
    project: &Project,
    original: &DeviceTag,
    structure: Option<&DeviceTag>,
    mode: TagMode,
) -> DeviceTag {
    let mut base = original.clone();
    if let Some(structure) = structure {
        base.function = structure.function.clone();
        base.location = structure.location.clone();
    }
    let next = |base: &DeviceTag| {
        let number = project.next_tag_number(base, base.letter_code());
        base.with_number(number)
    };
    match mode {
        TagMode::Keep => original.clone(),
        TagMode::KeepIfFree if !tag_in_use(project, original) => original.clone(),
        TagMode::KeepIfFree | TagMode::NextFree => next(&base),
    }
}

/// A new terminal on `strip`: the next number, the same name when it is
/// free (or not a number), per `mode`.
fn paste_terminal(
    tx: &mut Tx<'_>,
    original: &Terminal,
    strip: DeviceId,
    mode: TagMode,
    part: Option<PartId>,
    feed: Option<PotentialId>,
) -> Result<TerminalId> {
    let siblings = tx.project().terminals_of_strip(strip);
    let numeric = |name: &str| matches!(split_code(name), ("", Some(_), ""));
    let next = || {
        siblings
            .iter()
            .filter(|t| numeric(&t.name))
            .filter_map(|t| split_code(&t.name).1)
            .max()
            .map_or(1, |n| n + 1)
            .to_string()
    };
    let name = match mode {
        TagMode::Keep => original.name.clone(),
        TagMode::NextFree if numeric(&original.name) => next(),
        TagMode::KeepIfFree
            if numeric(&original.name) && siblings.iter().any(|t| t.name == original.name) =>
        {
            next()
        }
        _ => original.name.clone(),
    };
    let order = siblings.iter().map(|t| t.order).max().unwrap_or(0) + 1;
    let original = original.clone();
    tx.insert(|id| Terminal {
        id,
        strip,
        name,
        order,
        jumper: None,
        part,
        feed,
        ..original
    })
}

/// Pastes `clip` on `page`, moved by `offset`, as part of the caller's
/// transaction. `source` names the project being pasted into (see
/// [`Clip::source`]).
pub fn paste(
    tx: &mut Tx<'_>,
    clip: &Clip,
    page: PageId,
    offset: Point,
    mode: TagMode,
    source: &str,
) -> Result<Pasted> {
    let mut out = Pasted::default();
    let target = tx.project().page(page).map(|p| p.structure.clone());
    // Another structure than the source page's: the tags number in it.
    let structure = target.filter(|t| !t.same_structure(&clip.structure));

    // Potentials and parts the target may lack.
    let mut potentials: BTreeMap<PotentialId, PotentialId> = BTreeMap::new();
    for p in &clip.potentials {
        let found = tx
            .project()
            .potentials()
            .find(|q| q.name == p.name && q.kind == p.kind)
            .map(|q| q.id);
        let id = match found {
            Some(id) => id,
            None => {
                let p = p.clone();
                tx.insert(|id| Potential { id, ..p })?
            }
        };
        potentials.insert(p.id, id);
    }
    let mut parts: BTreeMap<PartId, PartId> = BTreeMap::new();
    for p in &clip.parts {
        let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
        let found = tx
            .project()
            .parts()
            .find(|q| {
                same(&q.manufacturer, &p.manufacturer) && same(&q.order_number, &p.order_number)
            })
            .map(|q| q.id);
        let id = match found {
            Some(id) => id,
            None => {
                let p = p.clone();
                tx.insert(|id| Part { id, ..p })?
            }
        };
        parts.insert(p.id, id);
    }
    let map_parts = |ids: &[PartId]| -> Vec<PartId> {
        ids.iter()
            .map(|p| parts.get(p).copied().unwrap_or(*p))
            .collect()
    };

    // Devices, in natural tag order so numbers follow it.
    let mut devices: BTreeMap<DeviceId, DeviceId> = BTreeMap::new();
    let mut listed: Vec<&ClipDevice> = clip.devices.iter().collect();
    listed.sort_by(|a, b| {
        novo_core::natural_cmp(&a.device.tag.to_string(), &b.device.tag.to_string())
    });
    for cd in listed {
        let still_there = clip.source == source
            && tx
                .project()
                .device(cd.device.id)
                .is_some_and(|d| d.tag == cd.device.tag);
        if !cd.copy && still_there {
            devices.insert(cd.device.id, cd.device.id);
            continue;
        }
        let tag = tag_for(tx.project(), &cd.device.tag, structure.as_ref(), mode);
        if tag != cd.device.tag {
            out.renamed.push((cd.device.tag.clone(), tag.clone()));
        } else {
            out.kept.push(tag.clone());
        }
        if !cd.copy {
            out.orphans.push(tag.clone());
        }
        let device = cd.device.clone();
        let new = tx.insert(|id| Device {
            id,
            tag,
            parts: map_parts(&device.parts),
            ..device
        })?;
        devices.insert(cd.device.id, new);
        if cd.copy {
            out.copied.devices.insert(cd.device.id, new);
        }
    }

    // Terminal strips by tag.
    let mut strips: BTreeMap<DeviceId, DeviceId> = BTreeMap::new();
    for strip in &clip.strips {
        let found = tx
            .project()
            .devices()
            .find(|d| d.kind == DeviceKind::TerminalStrip && d.tag == strip.tag)
            .map(|d| d.id);
        let id = match found {
            Some(id) => id,
            None => {
                let strip = strip.clone();
                tx.insert(|id| Device {
                    id,
                    parts: map_parts(&strip.parts),
                    ..strip
                })?
            }
        };
        strips.insert(strip.id, id);
    }

    for original in &clip.symbols {
        let role = match &original.role {
            SymbolRole::Device { device, main } => SymbolRole::Device {
                device: devices.get(device).copied().unwrap_or(*device),
                main: *main,
            },
            SymbolRole::Terminal { terminals } => {
                let mut copies = Vec::with_capacity(terminals.len());
                for &t in terminals {
                    if let Some(&done) = out.copied.terminals.get(&t) {
                        copies.push(done);
                        continue;
                    }
                    let terminal = clip
                        .terminals
                        .iter()
                        .find(|c| c.id == t)
                        .ok_or_else(|| ModelError::NotFound(t.into()))?;
                    let strip = strips
                        .get(&terminal.strip)
                        .copied()
                        .ok_or_else(|| ModelError::NotFound(terminal.strip.into()))?;
                    let part = terminal.part.map(|p| parts.get(&p).copied().unwrap_or(p));
                    let feed = terminal
                        .feed
                        .map(|f| potentials.get(&f).copied().unwrap_or(f));
                    let copy = paste_terminal(tx, terminal, strip, mode, part, feed)?;
                    out.copied.terminals.insert(t, copy);
                    copies.push(copy);
                }
                SymbolRole::Terminal { terminals: copies }
            }
            SymbolRole::PotentialFeed { potential } => SymbolRole::PotentialFeed {
                potential: potentials.get(potential).copied().unwrap_or(*potential),
            },
            other => other.clone(),
        };
        let mut feeds = original.feeds.clone();
        for f in &mut feeds {
            f.potential = potentials.get(&f.potential).copied().unwrap_or(f.potential);
        }
        let mut placement = original.placement;
        placement.origin = placement.origin + offset;
        let new = tx.insert(|id| SymbolInstance {
            id,
            page,
            placement,
            role,
            feeds,
            ..original.clone()
        })?;
        out.copied.symbols.insert(original.id, new);
    }

    for original in &clip.junctions {
        let position = original.position + offset;
        let new = tx.insert(|id| Junction { id, page, position })?;
        out.copied.junctions.insert(original.id, new);
    }

    for original in &clip.wires {
        let (Some(from), Some(to)) = (
            map_endpoint(&out.copied, original.from, offset),
            map_endpoint(&out.copied, original.to, offset),
        ) else {
            continue;
        };
        let route = original.route.iter().map(|&p| p + offset).collect();
        let potential = original
            .potential
            .map(|p| potentials.get(&p).copied().unwrap_or(p));
        let new = tx.insert(|id| Wire {
            id,
            page,
            from,
            to,
            route,
            number: None,
            potential,
            ..original.clone()
        })?;
        out.copied.wires.insert(original.id, new);
    }

    // Attached texts follow their objects; free texts move by the offset.
    for text in &clip.texts {
        match text.attached_to {
            Some(owner) => {
                let copy = match owner {
                    AnyId::Symbol(s) => out.copied.symbols.get(&s).map(|&c| AnyId::from(c)),
                    AnyId::Junction(j) => out.copied.junctions.get(&j).map(|&c| AnyId::from(c)),
                    AnyId::Wire(w) => out.copied.wires.get(&w).map(|&c| AnyId::from(c)),
                    _ => None,
                };
                let Some(copy) = copy else { continue };
                let new = tx.insert(|id| Text {
                    id,
                    page,
                    attached_to: Some(copy),
                    ..text.clone()
                })?;
                out.copied.texts.insert(text.id, new);
            }
            None => {
                let mut placement = text.placement;
                placement.offset = placement.offset + offset;
                let new = tx.insert(|id| Text {
                    id,
                    page,
                    placement,
                    ..text.clone()
                })?;
                out.copied.texts.insert(text.id, new);
            }
        }
    }
    Ok(out)
}

fn map_endpoint(out: &Duplicated, end: Endpoint, offset: Point) -> Option<Endpoint> {
    match end {
        Endpoint::Free { at } => Some(Endpoint::Free { at: at + offset }),
        Endpoint::Pin { symbol, pin } => out
            .symbols
            .get(&symbol)
            .map(|&s| Endpoint::Pin { symbol: s, pin }),
        Endpoint::Junction { junction } => out
            .junctions
            .get(&junction)
            .map(|&j| Endpoint::Junction { junction: j }),
    }
}
