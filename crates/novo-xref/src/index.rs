//! The cross-reference index: every coil, contact, terminal and interruption
//! point, where it is drawn and what it refers to.

use std::collections::HashMap;

use novo_core::{
    AnyId, DeviceId, Location, Match, Page, PageId, Project, SymbolId, SymbolRole, Value,
    natural_cmp,
};

use crate::function::{ContactKind, Functions};

/// What a group of cross-referenced symbols has in common.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Owner {
    /// The symbols drawing one device: a coil and its contacts, or the
    /// terminals of a terminal strip.
    Device(DeviceId),
    /// Interruption points sharing a name.
    Interruption(String),
}

/// One cross-referenced symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    pub symbol: SymbolId,
    pub owner: Owner,
    /// The device's main function (a contactor's coil).
    pub main: bool,
    pub kind: ContactKind,
    /// Connection designations, `13-14`, or `1-2 3-4 5-6` for main contacts.
    /// Empty for symbols that are neither a main function nor a contact.
    pub pins: String,
    /// Page and column, from [`Project::location_of`].
    pub location: Location,
    /// Structure of the page it is drawn on (`=A2+CAB1`), empty when the
    /// page has none; see [`page_structure`].
    pub structure: String,
    /// Position in the project's reading order (pages in natural order,
    /// then left to right, then top to bottom), as in
    /// [`Project::cross_references`].
    pub order: usize,
}

impl Reference {
    /// The device this symbol draws, if any.
    pub fn device(&self) -> Option<DeviceId> {
        match self.owner {
            Owner::Device(d) => Some(d),
            Owner::Interruption(_) => None,
        }
    }

    /// The location as reference text: `3.4`.
    pub fn target(&self) -> String {
        self.location.to_string()
    }

    /// The location as read from a page with structure `reader`: `3.4`,
    /// or `=A2/3.4` when this symbol's page has a different structure.
    pub fn target_from(&self, reader: &str) -> String {
        if self.structure.is_empty() || self.structure == reader {
            self.target()
        } else {
            format!("{}/{}", self.structure, self.location)
        }
    }

    /// One row of a contact image or reference list: `NO 13-14 3.4`.
    pub fn row(&self) -> String {
        self.row_from("")
    }

    /// [`Reference::row`] as read from a page with structure `reader`.
    pub fn row_from(&self, reader: &str) -> String {
        [self.kind.label(), &self.pins, &self.target_from(reader)]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// What to highlight when a symbol or device is selected: related symbols
/// on the same page, and the rest for the status bar.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Highlight {
    pub on_page: Vec<SymbolId>,
    pub off_page: Vec<Reference>,
    /// Device tag or interruption name, for the status bar.
    pub name: String,
    /// Structure of the page being read, for the reference format.
    pub reader: String,
}

impl Highlight {
    pub fn is_empty(&self) -> bool {
        self.on_page.is_empty() && self.off_page.is_empty()
    }

    /// Status bar text: `-K1 elsewhere: NO 13-14 3.4, NC 21-22 5.2`.
    /// Empty when nothing is off the page.
    pub fn status_text(&self) -> String {
        if self.off_page.is_empty() {
            return String::new();
        }
        let rows: Vec<String> = self
            .off_page
            .iter()
            .map(|r| r.row_from(&self.reader))
            .collect();
        format!("{} elsewhere: {}", self.name, rows.join(", "))
    }
}

/// Where Ctrl+K with an exact tag goes; see [`XrefIndex::go_to_tag`].
#[derive(Debug, Clone, PartialEq)]
pub struct TagJump<'i> {
    pub device: DeviceId,
    /// Where to jump: the main function, else the first occurrence.
    pub main: &'i Reference,
    /// Every other place the device occurs, in reading order.
    pub others: Vec<&'i Reference>,
}

/// Cross-reference text to draw next to a symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub symbol: SymbolId,
    /// One line per row; a coil's contact image has one row per contact.
    pub lines: Vec<String>,
    /// The device has more contacts of some kind drawn than it has
    /// (see [`Issue::TooManyContacts`]); draw the image as a warning.
    pub warning: bool,
}

/// Page property naming the page's function aspect (`A2` for `=A2`), until
/// the model gives pages a structure of their own.
pub const PAGE_FUNCTION_PROPERTY: &str = "function";
/// Page property naming the page's location aspect (`CAB1` for `+CAB1`).
pub const PAGE_LOCATION_PROPERTY: &str = "location";

/// The structure prefix of a page, `=A2+CAB1`, from its `function` and
/// `location` properties; empty when it has neither.
pub fn page_structure(page: &Page) -> String {
    let mut out = String::new();
    for (prefix, key) in [('=', PAGE_FUNCTION_PROPERTY), ('+', PAGE_LOCATION_PROPERTY)] {
        if let Some(Value::Text(v)) = page.properties.get(key)
            && !v.trim().is_empty()
        {
            out.push(prefix);
            out.push_str(v.trim().trim_start_matches(prefix));
        }
    }
    out
}

/// Device property giving how many contacts of a kind the device has,
/// until parts (WS-07) supply it: `contacts.main`, `contacts.no`,
/// `contacts.nc`, `contacts.co` (integers).
pub fn contact_limit_property(kind: ContactKind) -> Option<&'static str> {
    match kind {
        ContactKind::Power => Some("contacts.main"),
        ContactKind::NormallyOpen => Some("contacts.no"),
        ContactKind::NormallyClosed => Some("contacts.nc"),
        ContactKind::Changeover => Some("contacts.co"),
        ContactKind::Other => None,
    }
}

/// Something the cross references cannot resolve, for the design checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    /// Contacts of a device that has no main function (no coil).
    NoMain {
        device: DeviceId,
        symbols: Vec<SymbolId>,
    },
    /// A device drawn with more than one main function.
    SeveralMains {
        device: DeviceId,
        symbols: Vec<SymbolId>,
    },
    /// More contacts of one kind drawn than the device has.
    TooManyContacts {
        device: DeviceId,
        kind: ContactKind,
        drawn: usize,
        available: usize,
    },
    /// An interruption point with no partner of the same name.
    LoneInterruption { symbol: SymbolId, name: String },
}

#[derive(Debug, Clone, Default)]
struct Group {
    name: String,
    /// Main functions first, then reading order.
    symbols: Vec<SymbolId>,
    /// Contact kinds over their limit: (kind, drawn, available).
    over: Vec<(ContactKind, usize, usize)>,
}

/// Cross references of a whole project, built from one revision of it.
///
/// Building is a single pass over the symbols, so the app rebuilds it after
/// every change (see [`LiveXref`]) and references are never stale: moving,
/// renaming or deleting anything shows in the next frame. Locations and
/// order are the model's ([`Project::location_of`],
/// [`Project::cross_references`]); this adds what the drawing and the
/// navigation need on top.
#[derive(Debug, Clone, Default)]
pub struct XrefIndex {
    revision: u64,
    refs: HashMap<SymbolId, Reference>,
    groups: HashMap<Owner, Group>,
    by_page: HashMap<PageId, Vec<SymbolId>>,
    structures: HashMap<PageId, String>,
}

const LIMITED_KINDS: [ContactKind; 4] = [
    ContactKind::Power,
    ContactKind::NormallyOpen,
    ContactKind::NormallyClosed,
    ContactKind::Changeover,
];

impl XrefIndex {
    pub fn build(project: &Project, functions: &dyn Functions) -> Self {
        let mut index = XrefIndex {
            revision: project.revision(),
            ..XrefIndex::default()
        };
        let pages = project.pages_in_order();
        let page_rank: HashMap<PageId, usize> = pages
            .iter()
            .enumerate()
            .map(|(rank, p)| (p.id, rank))
            .collect();
        index.structures = pages
            .iter()
            .map(|p| (p.id, page_structure(p)))
            .filter(|(_, s)| !s.is_empty())
            .collect();

        let mut found = Vec::new();
        for sym in project.symbols() {
            let (owner, main) = match &sym.role {
                SymbolRole::Device { device, main } => (Owner::Device(*device), *main),
                SymbolRole::Interruption { name } => (Owner::Interruption(name.clone()), false),
                SymbolRole::Terminal { terminals } => {
                    let Some(strip) = terminals
                        .first()
                        .and_then(|t| project.terminal(*t))
                        .map(|t| t.strip)
                    else {
                        continue;
                    };
                    (Owner::Device(strip), false)
                }
                _ => continue,
            };
            let (Some(location), Some(&rank)) =
                (project.location_of(sym.id), page_rank.get(&sym.page))
            else {
                continue;
            };
            let kind = match sym.role {
                SymbolRole::Device { .. } => {
                    ContactKind::classify(functions.function(&sym.symbol), &sym.symbol)
                }
                _ => ContactKind::Other,
            };
            found.push((rank, sym, owner, main, kind, location));
        }
        // The model's reading order: page, then x, then y, then id.
        found.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.placement.origin.x.total_cmp(&b.1.placement.origin.x))
                .then(a.1.placement.origin.y.total_cmp(&b.1.placement.origin.y))
                .then(a.1.id.cmp(&b.1.id))
        });

        index.refs.reserve(found.len());
        for (order, (_, sym, owner, main, kind, location)) in found.into_iter().enumerate() {
            let mut names: Vec<&str> = sym.pins.iter().map(|p| p.name.as_str()).collect();
            names.sort_by(|a, b| natural_cmp(a, b));
            let pins =
                if kind == ContactKind::Power && names.len().is_multiple_of(2) && names.len() > 2 {
                    names
                        .chunks(2)
                        .map(|pair| pair.join("-"))
                        .collect::<Vec<_>>()
                        .join(" ")
                } else if kind.is_contact() || main {
                    names.join("-")
                } else {
                    String::new()
                };
            index.by_page.entry(sym.page).or_default().push(sym.id);
            index
                .groups
                .entry(owner.clone())
                .or_default()
                .symbols
                .push(sym.id);
            index.refs.insert(
                sym.id,
                Reference {
                    symbol: sym.id,
                    owner,
                    main,
                    kind,
                    pins,
                    structure: index.structures.get(&sym.page).cloned().unwrap_or_default(),
                    location,
                    order,
                },
            );
        }

        for (owner, group) in &mut index.groups {
            let refs = &index.refs;
            // Symbols went in in reading order; a stable sort puts mains first.
            group.symbols.sort_by_key(|s| !refs[s].main);
            match owner {
                Owner::Device(d) => {
                    let Some(device) = project.device(*d) else {
                        continue;
                    };
                    group.name = device.tag.to_string();
                    for kind in LIMITED_KINDS {
                        let Some(Value::Int(limit)) =
                            contact_limit_property(kind).and_then(|key| device.properties.get(key))
                        else {
                            continue;
                        };
                        let available = usize::try_from(*limit).unwrap_or(0);
                        let drawn = group
                            .symbols
                            .iter()
                            .filter(|s| !refs[*s].main && refs[*s].kind == kind)
                            .count();
                        if drawn > available {
                            group.over.push((kind, drawn, available));
                        }
                    }
                }
                Owner::Interruption(name) => group.name = name.clone(),
            }
        }
        index
    }

    /// The project revision this index was built from.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The reference of one symbol, if it draws a device or terminal or is
    /// an interruption point.
    pub fn reference(&self, symbol: SymbolId) -> Option<&Reference> {
        self.refs.get(&symbol)
    }

    fn group_refs(&self, owner: &Owner) -> Vec<&Reference> {
        self.groups
            .get(owner)
            .map(|g| g.symbols.iter().map(|s| &self.refs[s]).collect())
            .unwrap_or_default()
    }

    /// Every symbol drawing a device, main function first, then in reading
    /// order: the Shift+G list for a selected device.
    pub fn device_references(&self, device: DeviceId) -> Vec<&Reference> {
        self.group_refs(&Owner::Device(device))
    }

    /// Every place the selected symbol's device (or interruption name)
    /// occurs, the symbol itself included: the Shift+G list.
    pub fn all_references(&self, symbol: SymbolId) -> Vec<&Reference> {
        self.refs
            .get(&symbol)
            .map(|r| self.group_refs(&r.owner))
            .unwrap_or_default()
    }

    /// Where G jumps from a symbol: from a coil to its contacts in reading
    /// order, from a contact to its coil, from an interruption point to the
    /// others of the same name, from a terminal to the strip's other
    /// terminals.
    pub fn partners(&self, symbol: SymbolId) -> Vec<&Reference> {
        let Some(me) = self.refs.get(&symbol) else {
            return Vec::new();
        };
        let others = self
            .group_refs(&me.owner)
            .into_iter()
            .filter(|r| r.symbol != symbol);
        match me.owner {
            Owner::Device(_) if !me.main && me.kind.is_contact() => {
                others.filter(|r| r.main).collect()
            }
            _ => others.collect(),
        }
    }

    /// The main function (coil) of a device, if drawn.
    pub fn main_of(&self, device: DeviceId) -> Option<&Reference> {
        self.device_references(device).into_iter().find(|r| r.main)
    }

    /// The contacts of a device, main contacts first, then in reading order:
    /// the rows of the contact image under its coil.
    pub fn contact_image(&self, device: DeviceId) -> Vec<&Reference> {
        let mut rows: Vec<&Reference> = self
            .device_references(device)
            .into_iter()
            .filter(|r| !r.main && r.kind.is_contact())
            .collect();
        rows.sort_by_key(|r| (r.kind != ContactKind::Power, r.order));
        rows
    }

    /// The cross-reference text drawn next to a symbol: a coil's contact
    /// image, the coil location next to a contact (`3.4`), the partner
    /// locations next to an interruption point. `None` when there is
    /// nothing to show.
    pub fn label(&self, symbol: SymbolId) -> Option<Label> {
        let me = self.refs.get(&symbol)?;
        let mut warning = false;
        let lines: Vec<String> = match &me.owner {
            Owner::Device(device) if me.main => {
                let mut lines: Vec<String> = self
                    .contact_image(*device)
                    .into_iter()
                    .map(|r| r.row_from(&me.structure))
                    .collect();
                for &(kind, drawn, available) in &self.groups[&me.owner].over {
                    warning = true;
                    lines.push(format!(
                        "! {drawn} {} drawn, {available} available",
                        kind.label()
                    ));
                }
                lines
            }
            Owner::Device(_) if !me.kind.is_contact() => Vec::new(),
            _ => {
                let targets: Vec<String> = self
                    .partners(symbol)
                    .iter()
                    .map(|r| r.target_from(&me.structure))
                    .collect();
                if targets.is_empty() {
                    Vec::new()
                } else {
                    vec![targets.join(" ")]
                }
            }
        };
        (!lines.is_empty()).then_some(Label {
            symbol,
            lines,
            warning,
        })
    }

    /// Labels of every cross-referenced symbol on a page.
    pub fn labels_on_page(&self, page: PageId) -> Vec<Label> {
        self.symbols_on_page(page)
            .iter()
            .filter_map(|&s| self.label(s))
            .collect()
    }

    /// Cross-referenced symbols drawn on a page, in reading order.
    pub fn symbols_on_page(&self, page: PageId) -> &[SymbolId] {
        self.by_page.get(&page).map_or(&[], Vec::as_slice)
    }

    /// What selecting a symbol highlights: the other symbols of its device
    /// (or interruption name) on its own page, the rest listed off-page.
    /// Selecting a coil lights its contacts, selecting a contact its coil.
    pub fn highlight(&self, symbol: SymbolId) -> Highlight {
        let Some(me) = self.refs.get(&symbol) else {
            return Highlight::default();
        };
        let mut h = self.split(&me.owner, me.location.page);
        h.on_page.retain(|&s| s != symbol);
        h.off_page.retain(|r| r.symbol != symbol);
        h
    }

    /// What selecting a device (in a list or search) highlights while
    /// `page` is shown.
    pub fn highlight_device(&self, device: DeviceId, page: PageId) -> Highlight {
        self.split(&Owner::Device(device), page)
    }

    fn split(&self, owner: &Owner, page: PageId) -> Highlight {
        let mut h = Highlight {
            name: self
                .groups
                .get(owner)
                .map(|g| g.name.clone())
                .unwrap_or_default(),
            reader: self.structures.get(&page).cloned().unwrap_or_default(),
            ..Highlight::default()
        };
        for r in self.group_refs(owner) {
            if r.location.page == page {
                h.on_page.push(r.symbol);
            } else {
                h.off_page.push(r.clone());
            }
        }
        h
    }

    /// Devices matching a typed tag ([`Project::search`], best match
    /// first), each with every place it occurs. `K1` and `-K1` find
    /// `=A1+CAB1-K1`.
    pub fn find<'i>(
        &'i self,
        project: &Project,
        query: &str,
    ) -> Vec<(DeviceId, Vec<&'i Reference>)> {
        project
            .search(query)
            .into_iter()
            .filter_map(|hit| match hit.id {
                AnyId::Device(d) => Some((d, self.device_references(d))),
                _ => None,
            })
            .collect()
    }

    /// Ctrl+K with an exact tag, then Enter: the device's main location to
    /// jump to (its coil, else its first occurrence) and every other
    /// location to list. `None` unless the best search hit is a device
    /// matching exactly and drawn somewhere.
    pub fn go_to_tag<'i>(&'i self, project: &Project, query: &str) -> Option<TagJump<'i>> {
        let hit = project.search(query).into_iter().next()?;
        let AnyId::Device(device) = hit.id else {
            return None;
        };
        if hit.quality != Match::Exact {
            return None;
        }
        let mut all = self.device_references(device);
        if all.is_empty() {
            return None;
        }
        let main = all.remove(0);
        Some(TagJump {
            device,
            main,
            others: all,
        })
    }

    /// Unresolved references, for the design checks (WS-14), in reading
    /// order.
    pub fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (owner, group) in &self.groups {
            let first = group
                .symbols
                .first()
                .map_or(usize::MAX, |s| self.refs[s].order);
            match owner {
                Owner::Device(device) => {
                    let (mains, contacts): (Vec<SymbolId>, Vec<SymbolId>) = group
                        .symbols
                        .iter()
                        .filter(|s| self.refs[*s].main || self.refs[*s].kind.is_contact())
                        .partition(|s| self.refs[*s].main);
                    if mains.is_empty() && !contacts.is_empty() {
                        issues.push((
                            first,
                            Issue::NoMain {
                                device: *device,
                                symbols: contacts,
                            },
                        ));
                    } else if mains.len() > 1 {
                        issues.push((
                            first,
                            Issue::SeveralMains {
                                device: *device,
                                symbols: mains,
                            },
                        ));
                    }
                    for &(kind, drawn, available) in &group.over {
                        issues.push((
                            first,
                            Issue::TooManyContacts {
                                device: *device,
                                kind,
                                drawn,
                                available,
                            },
                        ));
                    }
                }
                Owner::Interruption(name) => {
                    if let [single] = group.symbols.as_slice() {
                        issues.push((
                            first,
                            Issue::LoneInterruption {
                                symbol: *single,
                                name: name.clone(),
                            },
                        ));
                    }
                }
            }
        }
        issues.sort_by_key(|(order, _)| *order);
        issues.into_iter().map(|(_, issue)| issue).collect()
    }
}

/// Keeps an [`XrefIndex`] current: [`LiveXref::get`] rebuilds it when the
/// project's revision changed (any edit, undo or redo).
#[derive(Debug, Clone, Default)]
pub struct LiveXref {
    index: Option<XrefIndex>,
}

impl LiveXref {
    pub fn get(&mut self, project: &Project, functions: &dyn Functions) -> &XrefIndex {
        let stale = self
            .index
            .as_ref()
            .is_none_or(|i| i.revision != project.revision());
        if stale {
            self.index = Some(XrefIndex::build(project, functions));
        }
        self.index.get_or_insert_with(XrefIndex::default)
    }

    /// Forces a rebuild on the next [`LiveXref::get`], for when the symbol
    /// library changed or another project was opened.
    pub fn invalidate(&mut self) {
        self.index = None;
    }
}
