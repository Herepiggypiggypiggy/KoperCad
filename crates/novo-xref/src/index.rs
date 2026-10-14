//! The cross-reference index: every coil, contact and interruption point,
//! where it is drawn and what it refers to.

use std::collections::HashMap;

use novo_core::{DeviceId, DeviceTag, PageId, Project, SymbolId, SymbolRole, natural_cmp};

use crate::function::{ContactKind, Functions};
use crate::grid::{Location, PageGrid};

/// What a group of cross-referenced symbols has in common.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Owner {
    /// The symbols drawing one device (a coil and its contacts).
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
    pub pins: String,
    pub location: Location,
}

impl Reference {
    /// The device this symbol draws, if any.
    pub fn device(&self) -> Option<DeviceId> {
        match self.owner {
            Owner::Device(d) => Some(d),
            Owner::Interruption(_) => None,
        }
    }

    /// One row of a contact image or reference list: `NO 13-14 /3.4`.
    pub fn row(&self) -> String {
        [self.kind.label(), &self.pins, &self.location.reference()]
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
}

impl Highlight {
    pub fn is_empty(&self) -> bool {
        self.on_page.is_empty() && self.off_page.is_empty()
    }

    /// Status bar text: `-K1 elsewhere: NO 13-14 /3.4, NC 21-22 /5.2`.
    /// Empty when nothing is off the page.
    pub fn status_text(&self) -> String {
        if self.off_page.is_empty() {
            return String::new();
        }
        let rows: Vec<String> = self.off_page.iter().map(Reference::row).collect();
        format!("{} elsewhere: {}", self.name, rows.join(", "))
    }
}

/// Cross-reference text to draw next to a symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub symbol: SymbolId,
    /// One line per row; a coil's contact image has one row per contact.
    pub lines: Vec<String>,
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
    /// An interruption point with no partner of the same name.
    LoneInterruption { symbol: SymbolId, name: String },
}

#[derive(Debug, Clone, Default)]
struct Group {
    name: String,
    /// Main functions first, then reading order.
    symbols: Vec<SymbolId>,
}

/// Cross references of a whole project, built from one revision of it.
///
/// Building is a single pass over the symbols, so the app rebuilds it after
/// every change (see [`LiveXref`]) and references are never stale: moving,
/// renaming or deleting anything shows in the next frame.
#[derive(Debug, Clone, Default)]
pub struct XrefIndex {
    revision: u64,
    refs: HashMap<SymbolId, Reference>,
    groups: HashMap<Owner, Group>,
    by_page: HashMap<PageId, Vec<SymbolId>>,
    tags: Vec<(DeviceTag, DeviceId)>,
}

impl XrefIndex {
    pub fn build(project: &Project, functions: &dyn Functions) -> Self {
        let mut index = XrefIndex {
            revision: project.revision(),
            refs: HashMap::with_capacity(project.symbols().size_hint().0),
            ..XrefIndex::default()
        };
        let pages: HashMap<PageId, (usize, PageGrid, &str)> = project
            .pages_in_order()
            .into_iter()
            .enumerate()
            .map(|(rank, p)| (p.id, (rank, PageGrid::of(p), p.name.as_str())))
            .collect();

        for sym in project.symbols() {
            let (owner, main) = match &sym.role {
                SymbolRole::Device { device, main } => (Owner::Device(*device), *main),
                SymbolRole::Interruption { name } => (Owner::Interruption(name.clone()), false),
                _ => continue,
            };
            let Some(&(page_rank, grid, page_name)) = pages.get(&sym.page) else {
                continue;
            };
            let kind = ContactKind::classify(functions.function(&sym.symbol), &sym.symbol);
            let mut names: Vec<&str> = sym.pins.iter().map(|p| p.name.as_str()).collect();
            names.sort_by(|a, b| natural_cmp(a, b));
            let pins =
                if kind == ContactKind::Power && names.len().is_multiple_of(2) && names.len() > 2 {
                    names
                        .chunks(2)
                        .map(|pair| pair.join("-"))
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    names.join("-")
                };
            let location = Location {
                page: sym.page,
                page_name: page_name.to_owned(),
                column: grid.column_at(sym.placement.origin.x),
                page_rank,
            };
            index.by_page.entry(sym.page).or_default().push(sym.id);
            let group = index.groups.entry(owner.clone()).or_default();
            group.symbols.push(sym.id);
            index.refs.insert(
                sym.id,
                Reference {
                    symbol: sym.id,
                    owner,
                    main,
                    kind,
                    pins,
                    location,
                },
            );
        }

        for (owner, group) in &mut index.groups {
            group.name = match owner {
                Owner::Device(d) => project
                    .device(*d)
                    .map(|d| d.tag.to_string())
                    .unwrap_or_default(),
                Owner::Interruption(name) => name.clone(),
            };
            let refs = &index.refs;
            group.symbols.sort_by(|a, b| {
                let (ra, rb) = (&refs[a], &refs[b]);
                rb.main
                    .cmp(&ra.main)
                    .then_with(|| ra.location.reading_cmp(&rb.location))
                    .then(a.cmp(b))
            });
        }
        for list in index.by_page.values_mut() {
            list.sort();
        }
        index.tags = project.devices().map(|d| (d.tag.clone(), d.id)).collect();
        index
    }

    /// The project revision this index was built from.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The reference of one symbol, if it is a device function or an
    /// interruption point.
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
    /// order. This is the Shift+G list for a selected device.
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
    /// others of the same name.
    pub fn partners(&self, symbol: SymbolId) -> Vec<&Reference> {
        let Some(me) = self.refs.get(&symbol) else {
            return Vec::new();
        };
        let others = self
            .group_refs(&me.owner)
            .into_iter()
            .filter(|r| r.symbol != symbol);
        match me.owner {
            Owner::Device(_) if !me.main => others.filter(|r| r.main).collect(),
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
        rows.sort_by(|a, b| {
            (a.kind != ContactKind::Power)
                .cmp(&(b.kind != ContactKind::Power))
                .then_with(|| a.location.reading_cmp(&b.location))
                .then(a.symbol.cmp(&b.symbol))
        });
        rows
    }

    /// The cross-reference text drawn next to a symbol: a coil's contact
    /// image, the coil location next to a contact (`/3.4`), the partner
    /// locations next to an interruption point. `None` when there is
    /// nothing to show.
    pub fn label(&self, symbol: SymbolId) -> Option<Label> {
        let me = self.refs.get(&symbol)?;
        let lines: Vec<String> = match &me.owner {
            Owner::Device(device) if me.main => self
                .contact_image(*device)
                .into_iter()
                .map(Reference::row)
                .collect(),
            Owner::Device(_) => {
                let mains: Vec<String> = self
                    .partners(symbol)
                    .iter()
                    .map(|r| r.location.reference())
                    .collect();
                if mains.is_empty() {
                    Vec::new()
                } else {
                    vec![mains.join(" ")]
                }
            }
            Owner::Interruption(_) => {
                let others: Vec<String> = self
                    .partners(symbol)
                    .iter()
                    .map(|r| r.location.reference())
                    .collect();
                if others.is_empty() {
                    Vec::new()
                } else {
                    vec![others.join(" ")]
                }
            }
        };
        (!lines.is_empty()).then_some(Label { symbol, lines })
    }

    /// Labels of every cross-referenced symbol on a page.
    pub fn labels_on_page(&self, page: PageId) -> Vec<Label> {
        self.by_page
            .get(&page)
            .into_iter()
            .flatten()
            .filter_map(|&s| self.label(s))
            .collect()
    }

    /// Cross-referenced symbols drawn on a page.
    pub fn symbols_on_page(&self, page: PageId) -> &[SymbolId] {
        self.by_page.get(&page).map_or(&[], Vec::as_slice)
    }

    /// What selecting a symbol highlights: the other symbols of its device
    /// (or interruption name) on its own page, the rest listed off-page.
    pub fn highlight(&self, symbol: SymbolId) -> Highlight {
        let Some(me) = self.refs.get(&symbol) else {
            return Highlight::default();
        };
        let mut h = self.split(&me.owner, me.location.page);
        h.on_page.retain(|&s| s != symbol);
        h
    }

    /// What selecting a device highlights while `page` is shown.
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

    /// Devices whose tag matches a typed tag, then every place each occurs.
    /// Aspects left out of the query match anything, so `-K1` finds
    /// `=A1+CAB1-K1`; an exact full tag finds only that device.
    pub fn find_tag(&self, query: &str) -> Vec<(DeviceId, Vec<&Reference>)> {
        let Ok(query) = DeviceTag::parse(query) else {
            return Vec::new();
        };
        if query.is_empty() {
            return Vec::new();
        }
        let fits = |q: &[String], t: &[String]| {
            q.is_empty()
                || q.iter()
                    .map(|s| s.to_lowercase())
                    .eq(t.iter().map(|s| s.to_lowercase()))
        };
        let mut hits: Vec<(bool, String, DeviceId)> = self
            .tags
            .iter()
            .filter(|(tag, _)| {
                fits(&query.function, &tag.function)
                    && fits(&query.location, &tag.location)
                    && fits(&query.product, &tag.product)
            })
            .map(|(tag, device)| (*tag != query, tag.to_string(), *device))
            .collect();
        hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| natural_cmp(&a.1, &b.1)));
        hits.into_iter()
            .map(|(_, _, device)| (device, self.device_references(device)))
            .collect()
    }

    /// Unresolved references, for the design checks (WS-14).
    pub fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        for (owner, group) in &self.groups {
            match owner {
                Owner::Device(device) => {
                    let mains: Vec<SymbolId> = group
                        .symbols
                        .iter()
                        .copied()
                        .filter(|s| self.refs[s].main)
                        .collect();
                    let contacts: Vec<SymbolId> = group
                        .symbols
                        .iter()
                        .copied()
                        .filter(|s| !self.refs[s].main && self.refs[s].kind.is_contact())
                        .collect();
                    if mains.is_empty() && !contacts.is_empty() {
                        issues.push(Issue::NoMain {
                            device: *device,
                            symbols: contacts,
                        });
                    } else if mains.len() > 1 {
                        issues.push(Issue::SeveralMains {
                            device: *device,
                            symbols: mains,
                        });
                    }
                }
                Owner::Interruption(name) => {
                    if let [single] = group.symbols.as_slice() {
                        issues.push(Issue::LoneInterruption {
                            symbol: *single,
                            name: name.clone(),
                        });
                    }
                }
            }
        }
        issues.sort_by_key(|i| match i {
            Issue::NoMain { symbols, .. } | Issue::SeveralMains { symbols, .. } => symbols[0],
            Issue::LoneInterruption { symbol, .. } => *symbol,
        });
        issues
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
