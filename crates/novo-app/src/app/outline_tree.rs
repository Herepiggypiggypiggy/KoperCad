//! The outline's page tree as data: what the Pages section shows, the
//! names it proposes for new groups and cabinets, and the edits its menus
//! make. Pure: no egui here, so the tree, the menus and every edit are
//! tested on a plain [`Document`].
//!
//! A *page group* is the function level of a page's structure (`=G1`), a
//! *cabinet* its location level (`+A1`). The tree shows either one first
//! (the Groups and Cabinets views); pages sit under the pair they are
//! filed in, and groups and cabinets that have no page yet still show, so
//! a new one is never invisible.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    AnyId, Aspect, Command, DeviceTag, MountingPlate, PageId, PageKind, Project, Result, Tx,
    natural_cmp,
};

use super::pages::PageView;
use crate::backend::PageRow;

/// One level name, as `DeviceTag` stores it (`["G1"]` for `=G1`).
pub type Levels = Vec<String>;

/// Property of a cabinet's structure node that names the enclosure picked
/// from the user's list (`enclosures.csv`).
pub const ENCLOSURE_PROPERTY: &str = "enclosure";

/// Plate of a cabinet made without choosing an enclosure, in millimetres:
/// the common 600 x 800 panel.
pub const DEFAULT_PLATE: MountingPlate = MountingPlate {
    width: 600.0,
    height: 800.0,
    margin: 10.0,
};

// ---------------------------------------------------------------------
// Tree

/// What a row of the tree is.
#[derive(Debug, Clone, PartialEq)]
pub enum RowKind {
    /// A page group. `context` is the structure a page made here gets.
    Group {
        function: Levels,
        context: DeviceTag,
        pages: usize,
    },
    /// A cabinet (a location; `enclosure` when it is a box that holds
    /// devices).
    Cabinet {
        location: Levels,
        context: DeviceTag,
        pages: usize,
        enclosure: bool,
    },
    /// The report, contents and other generated pages that belong to no
    /// group or cabinet.
    Generated { pages: usize },
    /// A page, or one sheet of a generated page.
    Page {
        page: PageId,
        sheet: usize,
        label: String,
        title: String,
        number: String,
        drawn: bool,
    },
    /// Placeholder under a group or cabinet that has no page.
    Empty,
}

/// One visible row.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRow {
    pub depth: usize,
    /// Stable id of the row: its path from the root. Collapsing and the
    /// selection are remembered by it.
    pub key: String,
    pub kind: RowKind,
    /// Whether the row has children it can fold away.
    pub foldable: bool,
}

impl TreeRow {
    pub fn page(&self) -> Option<(PageId, usize)> {
        match self.kind {
            RowKind::Page { page, sheet, .. } => Some((page, sheet)),
            _ => None,
        }
    }
}

/// The key of a page row.
pub fn page_key(page: PageId, sheet: usize) -> String {
    format!("p:{}:{sheet}", page.0)
}

fn levels_text(levels: &[String]) -> String {
    levels.join("\u{1f}")
}

fn level_cmp(a: &Levels, b: &Levels) -> std::cmp::Ordering {
    natural_cmp(&a.join("="), &b.join("="))
}

fn group_tag(function: &[String]) -> DeviceTag {
    DeviceTag {
        function: function.to_vec(),
        ..Default::default()
    }
}

fn cabinet_tag(location: &[String]) -> DeviceTag {
    DeviceTag {
        location: location.to_vec(),
        ..Default::default()
    }
}

/// The pages a node of the tree holds, in reading order, with the sheet
/// rows each one has.
type PageSheets = Vec<(PageId, Vec<usize>)>;

/// The tree for `view`. `collapsed` holds the keys of rows folded shut.
pub fn build(
    project: &Project,
    rows: &[PageRow],
    view: PageView,
    collapsed: &BTreeSet<String>,
) -> Vec<TreeRow> {
    // Page rows grouped per page, in order.
    let mut pages: PageSheets = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        match pages.last_mut() {
            Some((p, sheets)) if *p == row.page => sheets.push(i),
            _ => pages.push((row.page, vec![i])),
        }
    }
    let function_first = view == PageView::Function;
    // (primary, secondary) of a structure: group then cabinet in the
    // Groups view, the other way round in the Cabinets view.
    let pair = |tag: &DeviceTag| -> (Levels, Levels) {
        if function_first {
            (tag.function.clone(), tag.location.clone())
        } else {
            (tag.location.clone(), tag.function.clone())
        }
    };

    let mut loose: PageSheets = Vec::new();
    let mut generated: PageSheets = Vec::new();
    let mut nodes: BTreeMap<(Levels, Levels), PageSheets> = BTreeMap::new();
    for (page, sheets) in &pages {
        let Some(p) = project.page(*page) else {
            continue;
        };
        let is_generated = matches!(p.kind, PageKind::Generated { .. });
        let key = pair(&p.structure);
        if key.0.is_empty() && key.1.is_empty() {
            if is_generated {
                generated.push((*page, sheets.clone()));
            } else {
                loose.push((*page, sheets.clone()));
            }
        } else {
            nodes.entry(key).or_default().push((*page, sheets.clone()));
        }
    }
    // Groups and cabinets that exist as nodes but hold no page yet.
    for s in project.structures() {
        let key = pair(&s.tag);
        if !(key.0.is_empty() && key.1.is_empty()) {
            nodes.entry(key).or_default();
        }
    }
    let enclosure = |location: &Levels| {
        project
            .structures()
            .any(|s| s.enclosure && s.tag.location == *location)
    };

    let mut out = Vec::new();
    let page_rows = |out: &mut Vec<TreeRow>, list: &PageSheets, depth: usize, parent: &str| {
        for (page, sheets) in list {
            for &i in sheets {
                let row = &rows[i];
                let Some(p) = project.page(*page) else {
                    continue;
                };
                out.push(TreeRow {
                    depth,
                    key: format!("{parent}{}", page_key(*page, row.sheet)),
                    kind: RowKind::Page {
                        page: *page,
                        sheet: row.sheet,
                        label: row.label.clone(),
                        title: p.title.clone(),
                        number: p.name.clone(),
                        drawn: !matches!(p.kind, PageKind::Generated { .. }),
                    },
                    foldable: false,
                });
            }
        }
    };
    let count = |list: &PageSheets| list.len();

    page_rows(&mut out, &loose, 0, "");

    // One node: `own` levels, inside `parent` when it has one. The first
    // level is a group in the Groups view, a cabinet in the Cabinets view;
    // the second is the other.
    let make = |own: &Levels,
                parent: Option<&Levels>,
                pages: usize,
                parent_key: &str,
                depth: usize|
     -> TreeRow {
        let is_group = function_first == parent.is_none();
        let other = parent.cloned().unwrap_or_default();
        let key = format!(
            "{parent_key}{}:{}",
            if is_group { "g" } else { "c" },
            levels_text(own)
        );
        let kind = if is_group {
            RowKind::Group {
                function: own.clone(),
                context: DeviceTag {
                    function: own.clone(),
                    location: other,
                    ..Default::default()
                },
                pages,
            }
        } else {
            RowKind::Cabinet {
                location: own.clone(),
                context: DeviceTag {
                    function: other,
                    location: own.clone(),
                    ..Default::default()
                },
                pages,
                enclosure: enclosure(own),
            }
        };
        TreeRow {
            depth,
            key,
            kind,
            foldable: true,
        }
    };

    // Top-level nodes: every distinct primary name, then names that exist
    // only as the secondary level (a cabinet with no group in the Groups
    // view).
    let mut primaries: Vec<Levels> = nodes
        .keys()
        .filter(|(p, _)| !p.is_empty())
        .map(|(p, _)| p.clone())
        .collect();
    primaries.sort_by(level_cmp);
    primaries.dedup();
    for primary in &primaries {
        let own: PageSheets = nodes
            .get(&(primary.clone(), Vec::new()))
            .cloned()
            .unwrap_or_default();
        let mut seconds: Vec<Levels> = nodes
            .keys()
            .filter(|(p, s)| p == primary && !s.is_empty())
            .map(|(_, s)| s.clone())
            .collect();
        seconds.sort_by(level_cmp);
        let total: usize = count(&own)
            + seconds
                .iter()
                .map(|s| count(&nodes[&(primary.clone(), s.clone())]))
                .sum::<usize>();
        let head = make(primary, None, total, "", 0);
        let head_key = format!("{}/", head.key);
        let shut = collapsed.contains(&head.key);
        out.push(head);
        if shut {
            continue;
        }
        if total == 0 && seconds.is_empty() {
            out.push(TreeRow {
                depth: 1,
                key: format!("{head_key}empty"),
                kind: RowKind::Empty,
                foldable: false,
            });
            continue;
        }
        page_rows(&mut out, &own, 1, &head_key);
        for second in &seconds {
            let list = &nodes[&(primary.clone(), second.clone())];
            let row = make(second, Some(primary), count(list), &head_key, 1);
            let sub_key = format!("{}/", row.key);
            let shut = collapsed.contains(&row.key);
            out.push(row);
            if shut {
                continue;
            }
            if list.is_empty() {
                out.push(TreeRow {
                    depth: 2,
                    key: format!("{sub_key}empty"),
                    kind: RowKind::Empty,
                    foldable: false,
                });
            }
            page_rows(&mut out, list, 2, &sub_key);
        }
    }
    let mut loose_nodes: Vec<Levels> = nodes
        .keys()
        .filter(|(p, s)| p.is_empty() && !s.is_empty())
        .map(|(_, s)| s.clone())
        .collect();
    // A node made only to hold a cabinet's plate (or a group's name) does
    // not show again at the top when pages file it under another level.
    loose_nodes.retain(|second| {
        !nodes[&(Vec::new(), second.clone())].is_empty()
            || !nodes.keys().any(|(p, s)| !p.is_empty() && s == second)
    });
    loose_nodes.sort_by(level_cmp);
    for second in &loose_nodes {
        let list = &nodes[&(Vec::new(), second.clone())];
        // Reads as a top-level node of the other kind: a cabinet in the
        // Groups view, a group in the Cabinets view.
        let row = loose_row(second, function_first, count(list), enclosure(second));
        let sub_key = format!("{}/", row.key);
        let shut = collapsed.contains(&row.key);
        out.push(row);
        if shut {
            continue;
        }
        if list.is_empty() {
            out.push(TreeRow {
                depth: 1,
                key: format!("{sub_key}empty"),
                kind: RowKind::Empty,
                foldable: false,
            });
        }
        page_rows(&mut out, list, 1, &sub_key);
    }
    if !generated.is_empty() {
        let key = "generated".to_owned();
        let shut = collapsed.contains(&key);
        out.push(TreeRow {
            depth: 0,
            key: key.clone(),
            kind: RowKind::Generated {
                pages: generated.len(),
            },
            foldable: true,
        });
        if !shut {
            page_rows(&mut out, &generated, 1, "generated/");
        }
    }
    out
}

/// A top-level node that has no parent of the other kind.
fn loose_row(levels: &Levels, cabinet: bool, pages: usize, enclosure: bool) -> TreeRow {
    if cabinet {
        TreeRow {
            depth: 0,
            key: format!("loose/c:{}", levels_text(levels)),
            kind: RowKind::Cabinet {
                location: levels.clone(),
                context: cabinet_tag(levels),
                pages,
                enclosure,
            },
            foldable: true,
        }
    } else {
        TreeRow {
            depth: 0,
            key: format!("loose/g:{}", levels_text(levels)),
            kind: RowKind::Group {
                function: levels.clone(),
                context: group_tag(levels),
                pages,
            },
            foldable: true,
        }
    }
}

/// The keys of every row that can fold, for "Collapse all".
pub fn foldable_keys(rows: &[TreeRow]) -> Vec<String> {
    rows.iter()
        .filter(|r| r.foldable)
        .map(|r| r.key.clone())
        .collect()
}

/// Rows of the flat result list while the filter box has text: pages whose
/// label matches, each with the group and cabinet it is in.
pub fn filtered(project: &Project, rows: &[PageRow], filter: &str) -> Vec<(PageRow, String)> {
    let filter = filter.to_lowercase();
    rows.iter()
        .filter(|r| r.label.to_lowercase().contains(&filter))
        .map(|r| {
            let place = project
                .page(r.page)
                .map(|p| p.structure.to_string())
                .unwrap_or_default();
            (r.clone(), place)
        })
        .collect()
}

// ---------------------------------------------------------------------
// Names

/// What the person likes new groups and cabinets to be called: the start of
/// the name, for projects that have no name to continue yet. Kept per user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Naming {
    pub group_prefix: String,
    pub cabinet_prefix: String,
}

impl Default for Naming {
    fn default() -> Self {
        Self {
            group_prefix: "G".into(),
            cabinet_prefix: "A".into(),
        }
    }
}

/// `(prefix, number, width)` of a name: `BAY1G2` is `("BAY1G", 2, 1)`, a
/// name without digits counts as number 1 (`CAB` is the first `CAB`).
fn split_number(name: &str) -> (String, u64, usize) {
    let digits = name.chars().rev().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 12 {
        return (name.to_owned(), 1, 0);
    }
    let (prefix, tail) = name.split_at(name.len() - digits);
    (prefix.to_owned(), tail.parse().unwrap_or(1), digits)
}

/// The next name in the pattern the existing names follow: the prefix most
/// of them share, one past its highest number, written as wide as that
/// number was. `default_prefix` starts a project that has none. The result
/// is never one of `existing`.
pub fn next_name(existing: &[String], default_prefix: &str) -> String {
    let default_prefix = if default_prefix.trim().is_empty() {
        "A"
    } else {
        default_prefix.trim()
    };
    let mut by_prefix: BTreeMap<String, (usize, u64, usize)> = BTreeMap::new();
    for name in existing {
        let (prefix, n, width) = split_number(name);
        let entry = by_prefix.entry(prefix).or_insert((0, 0, 0));
        entry.0 += 1;
        if n >= entry.1 {
            entry.1 = n;
            entry.2 = width;
        }
    }
    let best = by_prefix
        .iter()
        // Most names first, then the highest number, then alphabetical.
        .max_by_key(|(prefix, (count, n, _))| (*count, *n, std::cmp::Reverse((*prefix).clone())));
    let (prefix, mut n, width) = match best {
        Some((prefix, (_, n, width))) => (prefix.clone(), *n + 1, *width),
        None => (default_prefix.to_owned(), 1, 0),
    };
    loop {
        let name = format!("{prefix}{n:0width$}");
        if !existing.contains(&name) {
            return name;
        }
        n += 1;
    }
}

/// Names of the top-level groups of `project`.
pub fn group_names(project: &Project) -> Vec<String> {
    project
        .page_groups()
        .iter()
        .filter_map(|g| g.function.first().cloned())
        .collect()
}

/// Names of the top-level cabinets of `project`.
pub fn cabinet_names(project: &Project) -> Vec<String> {
    project
        .panels()
        .iter()
        .filter_map(|p| p.location.location.first().cloned())
        .collect()
}

impl Naming {
    /// The person's letters, or the defaults for those left empty.
    pub fn from_prefixes(group: &str, cabinet: &str) -> Self {
        let defaults = Self::default();
        let pick = |own: &str, default: String| {
            if own.trim().is_empty() {
                default
            } else {
                own.trim().to_owned()
            }
        };
        Self {
            group_prefix: pick(group, defaults.group_prefix),
            cabinet_prefix: pick(cabinet, defaults.cabinet_prefix),
        }
    }

    /// The name the next new group gets.
    pub fn next_group(&self, project: &Project) -> String {
        next_name(&group_names(project), &self.group_prefix)
    }

    /// The name the next new cabinet gets.
    pub fn next_cabinet(&self, project: &Project) -> String {
        next_name(&cabinet_names(project), &self.cabinet_prefix)
    }
}

// ---------------------------------------------------------------------
// Menus

/// What a menu or key can do. Every row of the outline offers a subset.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// A page in `structure`, after `after` when given (else last).
    NewPage {
        structure: DeviceTag,
        after: Option<PageId>,
    },
    /// A page group with its first page; `in_cabinet` files it under that
    /// cabinet.
    NewGroup {
        in_cabinet: Option<Levels>,
    },
    /// Opens the new cabinet window; `in_group` files its first page there.
    NewCabinet {
        in_group: Option<Levels>,
    },
    RenamePage(PageId),
    RenameGroup(Levels),
    RenameCabinet(Levels),
    Properties(Key),
    DuplicatePage(PageId),
    MoveUp(PageId),
    MoveDown(PageId),
    /// Files the page under another group and/or cabinet.
    MoveTo {
        page: PageId,
        function: Option<Levels>,
        location: Option<Levels>,
    },
    MoveToNew(PageId),
    DeletePage(PageId),
    /// Takes the group or cabinet apart but keeps its pages.
    RemoveLevel(Aspect, Levels),
    /// Deletes the group or cabinet with its pages (asks first).
    DeleteLevelAndPages(Aspect, Levels),
    Renumber(DeviceTag),
    AddContents(Levels),
    OpenCabinet(Levels),
    Fold(String, bool),
    FoldAll(bool),
    /// Puts the pages in this order (a drag onto another page).
    Reorder(Vec<PageId>),
    /// Moves every page of a cabinet, as listed under group `from`, into
    /// group `to` (a cabinet dragged onto a group).
    MoveCabinet {
        location: Levels,
        from: Levels,
        to: Levels,
    },
    /// Files unfiled pages by the tags of their devices.
    AutoFile,
    /// The question was answered yes.
    Confirmed(Box<Act>),
}

/// What the properties window is about.
#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    Page(PageId),
    Group(Levels),
    Cabinet(Levels),
}

/// What a right click landed on.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// Empty space in the Pages section.
    Background,
    Group {
        function: Levels,
        context: DeviceTag,
    },
    Cabinet {
        location: Levels,
        context: DeviceTag,
    },
    Page(PageId),
    GeneratedHeader,
}

/// One entry of a context menu.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Entry {
        label: String,
        act: Act,
        enabled: bool,
        /// The key that does the same, shown at the right.
        keys: &'static str,
        /// Why it is off.
        why: &'static str,
    },
    Submenu {
        label: String,
        items: Vec<Item>,
    },
    Separator,
}

fn entry(label: impl Into<String>, act: Act) -> Item {
    Item::Entry {
        label: label.into(),
        act,
        enabled: true,
        keys: "",
        why: "",
    }
}

fn entry_keys(label: impl Into<String>, act: Act, keys: &'static str) -> Item {
    Item::Entry {
        label: label.into(),
        act,
        enabled: true,
        keys,
        why: "",
    }
}

fn off_unless(item: Item, on: bool, why: &'static str) -> Item {
    match item {
        Item::Entry {
            label,
            act,
            keys,
            enabled,
            ..
        } => Item::Entry {
            label,
            act,
            enabled: enabled && on,
            keys,
            why,
        },
        other => other,
    }
}

/// The entries of the menu for `target`, in order.
pub fn menu(project: &Project, target: &Target) -> Vec<Item> {
    let new_things = |group: Option<&Levels>, cabinet: Option<&Levels>| -> Vec<Item> {
        vec![
            entry(
                "New page",
                Act::NewPage {
                    structure: DeviceTag {
                        function: group.cloned().unwrap_or_default(),
                        location: cabinet.cloned().unwrap_or_default(),
                        ..Default::default()
                    },
                    after: None,
                },
            ),
            entry(
                "New page group",
                Act::NewGroup {
                    in_cabinet: cabinet.cloned(),
                },
            ),
            entry(
                "New cabinet…",
                Act::NewCabinet {
                    in_group: group.cloned(),
                },
            ),
        ]
    };
    match target {
        Target::Background => {
            let mut items = new_things(None, None);
            if !suggest_filing(project).is_empty() {
                items.push(Item::Separator);
                items.push(entry("File pages by their device tags", Act::AutoFile));
            }
            items.push(Item::Separator);
            items.push(entry("Expand all", Act::FoldAll(false)));
            items.push(entry("Collapse all", Act::FoldAll(true)));
            items
        }
        Target::GeneratedHeader => vec![
            entry("New page", new_page_here(&DeviceTag::default())),
            Item::Separator,
            entry("Expand all", Act::FoldAll(false)),
            entry("Collapse all", Act::FoldAll(true)),
        ],
        Target::Group { function, context } => {
            let key = Key::Group(function.clone());
            let mut items = vec![entry("New page here", new_page_here(context))];
            // A group in the Cabinets view sits inside a cabinet: another
            // group goes beside it, in the same cabinet.
            items.push(entry(
                "New page group",
                Act::NewGroup {
                    in_cabinet: (!context.location.is_empty()).then(|| context.location.clone()),
                },
            ));
            items.push(entry(
                "New cabinet…",
                Act::NewCabinet {
                    in_group: Some(function.clone()),
                },
            ));
            items.push(Item::Separator);
            items.push(entry_keys(
                "Rename",
                Act::RenameGroup(function.clone()),
                "F2",
            ));
            items.push(entry("Renumber its pages", Act::Renumber(context.clone())));
            items.push(entry("Properties…", Act::Properties(key)));
            items.push(Item::Separator);
            items.push(entry(
                "Remove group, keep pages",
                Act::RemoveLevel(Aspect::Function, function.clone()),
            ));
            items.push(entry_keys(
                "Delete group and its pages…",
                Act::DeleteLevelAndPages(Aspect::Function, function.clone()),
                "Del",
            ));
            items
        }
        Target::Cabinet { location, context } => {
            let key = Key::Cabinet(location.clone());
            let has_contents = super::pages::has_panel_contents(project, &cabinet_tag(location));
            let mut items = vec![entry("New page here", new_page_here(context))];
            items.push(entry(
                "New page group here",
                Act::NewGroup {
                    in_cabinet: Some(location.clone()),
                },
            ));
            items.push(entry(
                "New cabinet…",
                Act::NewCabinet {
                    in_group: (!context.function.is_empty()).then(|| context.function.clone()),
                },
            ));
            items.push(Item::Separator);
            items.push(entry(
                "Open cabinet layout",
                Act::OpenCabinet(location.clone()),
            ));
            items.push(off_unless(
                entry("Add contents page", Act::AddContents(location.clone())),
                !has_contents,
                "This cabinet already has a contents page",
            ));
            items.push(Item::Separator);
            items.push(entry_keys(
                "Rename",
                Act::RenameCabinet(location.clone()),
                "F2",
            ));
            items.push(entry("Renumber its pages", Act::Renumber(context.clone())));
            items.push(entry("Properties…", Act::Properties(key)));
            items.push(Item::Separator);
            items.push(entry(
                "Remove cabinet, keep pages",
                Act::RemoveLevel(Aspect::Location, location.clone()),
            ));
            items.push(entry_keys(
                "Delete cabinet and its pages…",
                Act::DeleteLevelAndPages(Aspect::Location, location.clone()),
                "Del",
            ));
            items
        }
        Target::Page(page) => {
            let Some(p) = project.page(*page) else {
                return Vec::new();
            };
            let drawn = !matches!(p.kind, PageKind::Generated { .. });
            let siblings = project.sibling_pages(*page);
            let at = siblings.iter().position(|s| s == page).unwrap_or(0);
            let mut items = vec![
                entry(
                    "New page after this",
                    Act::NewPage {
                        structure: p.structure.clone(),
                        after: Some(*page),
                    },
                ),
                off_unless(
                    entry("Duplicate page", Act::DuplicatePage(*page)),
                    drawn,
                    "Generated pages are made by the program",
                ),
                Item::Separator,
                off_unless(
                    entry_keys("Rename", Act::RenamePage(*page), "F2"),
                    drawn,
                    "Generated pages are named by the program",
                ),
                entry_keys("Move up", Act::MoveUp(*page), "Ctrl+\u{2191}"),
                entry_keys("Move down", Act::MoveDown(*page), "Ctrl+\u{2193}"),
            ];
            // Disable at the ends.
            items[4] = off_unless(items[4].clone(), at > 0, "Already first");
            items[5] = off_unless(items[5].clone(), at + 1 < siblings.len(), "Already last");
            items.push(Item::Submenu {
                label: "Move to".into(),
                items: move_to_items(project, *page),
            });
            items.push(entry("Properties…", Act::Properties(Key::Page(*page))));
            items.push(Item::Separator);
            items.push(off_unless(
                entry_keys("Delete page", Act::DeletePage(*page), "Del"),
                drawn,
                "Generated pages are turned off in the project rules",
            ));
            items
        }
    }
}

fn new_page_here(context: &DeviceTag) -> Act {
    Act::NewPage {
        structure: context.clone(),
        after: None,
    }
}

/// "Move to" submenu: every group and cabinet, and a new one.
fn move_to_items(project: &Project, page: PageId) -> Vec<Item> {
    let Some(p) = project.page(page) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    let groups = project.page_groups();
    let panels = project.panels();
    for g in &groups {
        let here = p.structure.function == g.function;
        items.push(off_unless(
            entry(
                format!("Group {g}"),
                Act::MoveTo {
                    page,
                    function: Some(g.function.clone()),
                    location: None,
                },
            ),
            !here,
            "Already in this group",
        ));
    }
    if !groups.is_empty() || !p.structure.function.is_empty() {
        items.push(off_unless(
            entry(
                "No group",
                Act::MoveTo {
                    page,
                    function: Some(Vec::new()),
                    location: None,
                },
            ),
            !p.structure.function.is_empty(),
            "Not in a group",
        ));
    }
    if !groups.is_empty() {
        items.push(Item::Separator);
    }
    for c in &panels {
        let here = p.structure.location == c.location.location;
        items.push(off_unless(
            entry(
                format!("Cabinet {}", c.location),
                Act::MoveTo {
                    page,
                    function: None,
                    location: Some(c.location.location.clone()),
                },
            ),
            !here,
            "Already in this cabinet",
        ));
    }
    if !panels.is_empty() || !p.structure.location.is_empty() {
        items.push(off_unless(
            entry(
                "No cabinet",
                Act::MoveTo {
                    page,
                    function: None,
                    location: Some(Vec::new()),
                },
            ),
            !p.structure.location.is_empty(),
            "Not in a cabinet",
        ));
    }
    items.push(Item::Separator);
    items.push(entry("Other group or cabinet…", Act::MoveToNew(page)));
    items
}

// ---------------------------------------------------------------------
// Edits

/// The first free page name in `structure`: one past the highest number.
/// A page that already is in `structure` does not count against itself.
fn free_name(project: &Project, structure: &DeviceTag, name: &str, except: PageId) -> bool {
    !project
        .pages()
        .any(|p| p.id != except && p.structure.same_structure(structure) && p.name == name)
}

/// Adds a drawn page in `structure` under the next free number, after
/// `after` when given. One undo step when run in a transaction.
pub fn add_page(
    tx: &mut Tx<'_>,
    structure: &DeviceTag,
    title: &str,
    after: Option<PageId>,
) -> Result<PageId> {
    let name = tx.project().next_page_name(structure);
    let created = tx.execute(Command::AddPage {
        name,
        title: title.trim().into(),
        kind: PageKind::default(),
        format: Default::default(),
        structure: structure.clone(),
    })?;
    let page = created
        .iter()
        .find_map(|id| match id {
            AnyId::Page(p) => Some(*p),
            _ => None,
        })
        .expect("AddPage creates a page");
    if let Some(after) = after {
        place_after(tx, page, after)?;
    }
    Ok(page)
}

/// Puts `page` right after `after` in their group and cabinet.
fn place_after(tx: &mut Tx<'_>, page: PageId, after: PageId) -> Result<()> {
    let mut order: Vec<PageId> = tx
        .project()
        .sibling_pages(page)
        .into_iter()
        .filter(|p| *p != page)
        .collect();
    if let Some(at) = order.iter().position(|p| *p == after) {
        order.insert(at + 1, page);
        tx.reorder_pages(&order)?;
    }
    Ok(())
}

/// A copy of a drawn page right after it: same group, cabinet, format and
/// title, the next free number, and everything drawn on it with the next
/// free device tags.
pub fn duplicate_page(tx: &mut Tx<'_>, source: PageId) -> Result<PageId> {
    let (structure, title, format) = {
        let p = tx
            .project()
            .page(source)
            .ok_or(novo_core::ModelError::NotFound(source.into()))?;
        (p.structure.clone(), p.title.clone(), p.format)
    };
    let name = tx.project().next_page_name(&structure);
    let created = tx.execute(Command::AddPage {
        name,
        title,
        kind: PageKind::default(),
        format,
        structure,
    })?;
    let page = created
        .iter()
        .find_map(|id| match id {
            AnyId::Page(p) => Some(*p),
            _ => None,
        })
        .expect("AddPage creates a page");
    let contents = tx.project().page_contents(source);
    let mut items: Vec<AnyId> = Vec::new();
    items.extend(contents.symbols.iter().map(|&s| AnyId::from(s)));
    items.extend(contents.junctions.iter().map(|&s| AnyId::from(s)));
    items.extend(contents.wires.iter().map(|&s| AnyId::from(s)));
    items.extend(contents.texts.iter().map(|&s| AnyId::from(s)));
    if !items.is_empty() {
        novo_workflow::duplicate::duplicate(
            tx,
            &items,
            page,
            novo_core::Point::new(0.0, 0.0),
            novo_workflow::duplicate::TagMode::NextFree,
        )?;
    }
    place_after(tx, page, source)?;
    Ok(page)
}

/// Sets a page's title.
pub fn set_title(tx: &mut Tx<'_>, page: PageId, title: &str) -> Result<()> {
    let title = title.trim().to_owned();
    tx.update::<novo_core::Page>(page, |p| p.title = title)
}

/// Numbers the drawn pages of `structure` 1, 2, 3 in reading order. Returns
/// how many changed name. Generated pages keep their names.
pub fn renumber(tx: &mut Tx<'_>, structure: &DeviceTag) -> Result<usize> {
    let pages: Vec<(PageId, String)> = tx
        .project()
        .pages_in_order()
        .into_iter()
        .filter(|p| p.structure.same_structure(structure))
        .filter(|p| !matches!(p.kind, PageKind::Generated { .. }))
        .map(|p| (p.id, p.name.clone()))
        .collect();
    let mut changed = 0;
    for (n, (page, name)) in pages.into_iter().enumerate() {
        let wanted = (n + 1).to_string();
        if name != wanted {
            tx.execute(Command::RenamePage { page, name: wanted })?;
            changed += 1;
        }
    }
    Ok(changed)
}

/// Files `page` under another group and/or cabinet (`None` keeps that
/// level). A page whose number is taken at the new place takes the next
/// free one, so numbering follows the page. Returns the new structure.
pub fn move_page(
    tx: &mut Tx<'_>,
    page: PageId,
    function: Option<Levels>,
    location: Option<Levels>,
) -> Result<DeviceTag> {
    let p = tx
        .project()
        .page(page)
        .ok_or(novo_core::ModelError::NotFound(page.into()))?;
    let mut structure = p.structure.clone();
    let name = p.name.clone();
    let drawn = !matches!(p.kind, PageKind::Generated { .. });
    if let Some(f) = function {
        structure.function = f;
    }
    if let Some(l) = location {
        structure.location = l;
    }
    if structure == p.structure {
        return Ok(structure);
    }
    // Ensure the groups and cabinets named exist as nodes.
    ensure_nodes(tx, &structure)?;
    let taken = !free_name(tx.project(), &structure, &name, page);
    tx.execute(Command::SetPageStructure {
        page,
        structure: structure.clone(),
    })?;
    if taken && drawn {
        let next = tx.project().next_page_name(&structure);
        tx.execute(Command::RenamePage { page, name: next })?;
    }
    Ok(structure)
}

/// Adds the group node and the cabinet node of `structure` that do not
/// exist yet (a cabinet is an enclosure).
pub fn ensure_nodes(tx: &mut Tx<'_>, structure: &DeviceTag) -> Result<()> {
    if !structure.function.is_empty() {
        let tag = group_tag(&structure.function);
        if tx.project().structure_by_tag(&tag).is_none() {
            tx.add_structure(tag, "", false)?;
        }
    }
    if !structure.location.is_empty() {
        let tag = cabinet_tag(&structure.location);
        if tx.project().structure_by_tag(&tag).is_none() {
            tx.add_structure(tag, "", true)?;
        }
    }
    Ok(())
}

/// A new page group with its first page; `in_cabinet` files that page in a
/// cabinet too. Returns the page.
pub fn add_group(tx: &mut Tx<'_>, name: &str, in_cabinet: Option<&Levels>) -> Result<PageId> {
    let structure = DeviceTag {
        function: vec![name.to_owned()],
        location: in_cabinet.cloned().unwrap_or_default(),
        ..Default::default()
    };
    ensure_nodes(tx, &structure)?;
    add_page(tx, &structure, "", None)
}

/// What the new cabinet window collects.
#[derive(Debug, Clone, PartialEq)]
pub struct CabinetSpec {
    pub name: String,
    /// The group the first page goes in (empty for none).
    pub group: Levels,
    /// Mounting plate of the chosen enclosure.
    pub plate: MountingPlate,
    /// A first page, so the cabinet shows in the tree under its group.
    pub first_page: bool,
}

/// Makes a cabinet: its node, flagged as an enclosure with the plate, and
/// its first page. Returns the page when one was made.
pub fn add_cabinet(tx: &mut Tx<'_>, spec: &CabinetSpec) -> Result<Option<PageId>> {
    let tag = cabinet_tag(std::slice::from_ref(&spec.name));
    let id = match tx.project().structure_by_tag(&tag).map(|s| s.id) {
        Some(id) => id,
        None => tx.add_structure(tag, "", true)?,
    };
    tx.set_plate(id, Some(spec.plate))?;
    if !spec.group.is_empty() {
        ensure_nodes(tx, &group_structure(&spec.group))?;
    }
    if spec.first_page {
        let structure = DeviceTag {
            function: spec.group.clone(),
            location: vec![spec.name.clone()],
            ..Default::default()
        };
        return add_page(tx, &structure, "", None).map(Some);
    }
    Ok(None)
}

fn group_structure(function: &[String]) -> DeviceTag {
    group_tag(function)
}

/// Takes a group (`Function`) or cabinet (`Location`) apart: its pages
/// stay but lose that level, and its node goes. Devices and cables keep
/// their tags. Returns the pages that were unfiled.
pub fn remove_level(tx: &mut Tx<'_>, aspect: Aspect, levels: &Levels) -> Result<usize> {
    let pages: Vec<(PageId, DeviceTag)> = tx
        .project()
        .pages()
        .filter(|p| p.structure.levels(aspect).starts_with(levels))
        .map(|p| (p.id, p.structure.clone()))
        .collect();
    let count = pages.len();
    for (page, mut structure) in pages {
        structure.levels_mut(aspect).clear();
        let drawn = tx
            .project()
            .page(page)
            .is_some_and(|p| !matches!(p.kind, PageKind::Generated { .. }));
        let name = tx.project().page(page).map(|p| p.name.clone());
        tx.execute(Command::SetPageStructure {
            page,
            structure: structure.clone(),
        })?;
        if drawn
            && let Some(name) = name
            && !free_name(tx.project(), &structure, &name, page)
        {
            let next = tx.project().next_page_name(&structure);
            tx.execute(Command::RenamePage { page, name: next })?;
        }
    }
    remove_nodes(tx, aspect, levels)?;
    Ok(count)
}

/// Removes the structure nodes that are this group or cabinet (the ones
/// whose tag has no other aspect than this level).
fn remove_nodes(tx: &mut Tx<'_>, aspect: Aspect, levels: &Levels) -> Result<()> {
    let ids: Vec<_> = tx
        .project()
        .structures()
        .filter(|s| s.tag.levels(aspect).starts_with(levels))
        .filter(|s| {
            let other = if aspect == Aspect::Function {
                &s.tag.location
            } else {
                &s.tag.function
            };
            other.is_empty()
        })
        .map(|s| s.id)
        .collect();
    for id in ids {
        tx.execute(Command::Remove { id: id.into() })?;
    }
    Ok(())
}

/// Deletes a group or cabinet with its drawn pages. Generated pages in it
/// are only unfiled, the program makes them. Returns the number of pages
/// deleted.
pub fn delete_level_and_pages(tx: &mut Tx<'_>, aspect: Aspect, levels: &Levels) -> Result<usize> {
    let pages: Vec<PageId> = tx
        .project()
        .pages()
        .filter(|p| p.structure.levels(aspect).starts_with(levels))
        .filter(|p| !matches!(p.kind, PageKind::Generated { .. }))
        .map(|p| p.id)
        .collect();
    let count = pages.len();
    for page in pages {
        tx.delete_page(page)?;
    }
    remove_level(tx, aspect, levels)?;
    Ok(count)
}

/// How much a group or cabinet holds, for the question before it goes.
pub fn level_usage(project: &Project, aspect: Aspect, levels: &Levels) -> (usize, usize, usize) {
    let uses = |t: &DeviceTag| t.levels(aspect).starts_with(levels);
    (
        project
            .pages()
            .filter(|p| !matches!(p.kind, PageKind::Generated { .. }) && uses(&p.structure))
            .count(),
        project.devices().filter(|d| uses(&d.tag)).count(),
        project
            .structures()
            .filter(|s| uses(&s.tag) && s.plate.is_some())
            .count(),
    )
}

/// Files every page of cabinet `location` that is listed under group
/// `from` into group `to`. Returns how many moved.
pub fn move_cabinet(
    tx: &mut Tx<'_>,
    location: &Levels,
    from: &Levels,
    to: &Levels,
) -> Result<usize> {
    let pages: Vec<PageId> = tx
        .project()
        .pages_in_order()
        .into_iter()
        .filter(|p| p.structure.location == *location && p.structure.function == *from)
        .map(|p| p.id)
        .collect();
    let count = pages.len();
    for page in pages {
        move_page(tx, page, Some(to.clone()), None)?;
    }
    Ok(count)
}

/// Where drawn pages that are in no group or cabinet belong by what is drawn
/// on them: the group and cabinet most of their devices are tagged with
/// (`=A1+CAB-K1`). Pages whose devices say nothing are left out.
pub fn suggest_filing(project: &Project) -> Vec<(PageId, DeviceTag)> {
    let mut out = Vec::new();
    for page in project.pages_in_order() {
        if matches!(page.kind, PageKind::Generated { .. }) || !page.structure.is_empty() {
            continue;
        }
        let mut votes: BTreeMap<(Levels, Levels), usize> = BTreeMap::new();
        for symbol in project.page_contents(page.id).symbols {
            let Some(novo_core::SymbolRole::Device { device, .. }) =
                project.symbol(symbol).map(|s| &s.role)
            else {
                continue;
            };
            let Some(device) = project.device(*device) else {
                continue;
            };
            let key = (device.tag.function.clone(), device.tag.location.clone());
            if !(key.0.is_empty() && key.1.is_empty()) {
                *votes.entry(key).or_default() += 1;
            }
        }
        // The most votes; on a tie the one with more levels named.
        let best = votes.into_iter().max_by_key(|((f, l), n)| {
            (*n, usize::from(!f.is_empty()) + usize::from(!l.is_empty()))
        });
        if let Some(((function, location), _)) = best {
            out.push((
                page.id,
                DeviceTag {
                    function,
                    location,
                    ..Default::default()
                },
            ));
        }
    }
    out
}

/// Files the pages as [`suggest_filing`] proposes, one undo step in a
/// transaction. Returns how many pages moved.
pub fn auto_file(tx: &mut Tx<'_>) -> Result<usize> {
    let plan = suggest_filing(tx.project());
    let count = plan.len();
    for (page, structure) in plan {
        move_page(tx, page, Some(structure.function), Some(structure.location))?;
    }
    Ok(count)
}

/// Does `levels` name an existing group (`Function`) or cabinet?
pub fn level_exists(project: &Project, aspect: Aspect, levels: &[String]) -> bool {
    project
        .pages()
        .any(|p| p.structure.levels(aspect) == levels)
        || project.structures().any(|s| s.tag.levels(aspect) == levels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::Document;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn names_continue_the_pattern_of_the_project() {
        assert_eq!(next_name(&[], "A"), "A1");
        assert_eq!(next_name(&names(&["A1", "A2"]), "G"), "A3");
        assert_eq!(next_name(&names(&["20D41"]), "A"), "20D42");
        assert_eq!(next_name(&names(&["BAY1G1", "BAY1G2"]), "A"), "BAY1G3");
        // A name without a number is the first of its kind.
        assert_eq!(next_name(&names(&["CAB"]), "A"), "CAB2");
        // The widest habit wins: 01, 02 -> 03.
        assert_eq!(next_name(&names(&["A01", "A02"]), "A"), "A03");
        // Most names decide when patterns mix.
        assert_eq!(next_name(&names(&["X1", "A1", "A2"]), "A"), "A3");
        // Never an existing name.
        assert_eq!(next_name(&names(&["A1", "A3"]), "A"), "A4");
        // An empty prefix setting falls back.
        assert_eq!(next_name(&[], "  "), "A1");
    }

    fn doc_with_tree() -> Document {
        let mut doc = Document::new(novo_core::Project::default());
        doc.transact("setup", |tx| {
            let s = DeviceTag::parse("=G1+A1").unwrap();
            ensure_nodes(tx, &s)?;
            add_page(tx, &s, "Motor", None)?;
            add_page(tx, &s, "Control", None)?;
            let t = DeviceTag::parse("=G1").unwrap();
            add_page(tx, &t, "Overview", None)?;
            let u = DeviceTag::parse("=G2+A1").unwrap();
            ensure_nodes(tx, &u)?;
            add_page(tx, &u, "Other", None)?;
            tx.add_structure(cabinet_tag(&["A2".into()]), "", true)?;
            add_page(tx, &DeviceTag::default(), "Loose", None)?;
            Ok(())
        })
        .unwrap();
        doc
    }

    fn rows_of(doc: &Document) -> Vec<PageRow> {
        crate::backend::page_rows(doc.project(), &mut novo_reports::LiveReports::default())
    }

    fn shape(rows: &[TreeRow]) -> Vec<String> {
        rows.iter()
            .map(|r| {
                let text = match &r.kind {
                    RowKind::Group {
                        function, pages, ..
                    } => {
                        format!("group {} ({pages})", function.join("="))
                    }
                    RowKind::Cabinet {
                        location,
                        pages,
                        enclosure,
                        ..
                    } => format!(
                        "cabinet {} ({pages}{})",
                        location.join("+"),
                        if *enclosure { ", box" } else { "" }
                    ),
                    RowKind::Generated { pages } => format!("generated ({pages})"),
                    RowKind::Page { label, .. } => format!("page {label}"),
                    RowKind::Empty => "empty".into(),
                };
                format!("{}{text}", "  ".repeat(r.depth))
            })
            .collect()
    }

    #[test]
    fn the_groups_view_nests_cabinets_in_groups_and_shows_empty_ones() {
        let doc = doc_with_tree();
        let tree = build(
            doc.project(),
            &rows_of(&doc),
            PageView::Function,
            &BTreeSet::new(),
        );
        assert_eq!(
            shape(&tree),
            vec![
                "page 1 Loose",
                "group G1 (3)",
                "  page 1 Overview",
                "  cabinet A1 (2, box)",
                "    page 1 Motor",
                "    page 2 Control",
                "group G2 (1)",
                "  cabinet A1 (1, box)",
                "    page 1 Other",
                "cabinet A2 (0, box)",
                "  empty",
            ]
        );
    }

    #[test]
    fn the_cabinets_view_nests_groups_in_cabinets() {
        let doc = doc_with_tree();
        let tree = build(
            doc.project(),
            &rows_of(&doc),
            PageView::Location,
            &BTreeSet::new(),
        );
        let shape = shape(&tree);
        assert_eq!(shape[1], "cabinet A1 (3, box)");
        assert_eq!(shape[2], "  group G1 (2)");
        assert!(shape.contains(&"cabinet A2 (0, box)".to_owned()));
    }

    #[test]
    fn collapsing_hides_the_children_and_keeps_the_row() {
        let doc = doc_with_tree();
        let rows = rows_of(&doc);
        let open = build(doc.project(), &rows, PageView::Function, &BTreeSet::new());
        let key = open
            .iter()
            .find(|r| matches!(&r.kind, RowKind::Group { function, .. } if function == &["G1"]))
            .unwrap()
            .key
            .clone();
        let shut = build(
            doc.project(),
            &rows,
            PageView::Function,
            &BTreeSet::from([key]),
        );
        assert_eq!(shut.len(), open.len() - 4);
        assert!(shape(&shut).contains(&"group G1 (3)".to_owned()));
        assert_eq!(foldable_keys(&shut).len(), 4);
    }

    #[test]
    fn a_new_page_takes_the_next_number_and_can_go_after_another() {
        let mut doc = doc_with_tree();
        let s = DeviceTag::parse("=G1+A1").unwrap();
        let first = doc.project().sibling_pages(
            doc.project()
                .pages()
                .find(|p| p.title == "Motor")
                .unwrap()
                .id,
        )[0];
        let (new, _) = doc
            .transact("new", |tx| add_page(tx, &s, "Inserted", Some(first)))
            .unwrap();
        let p = doc.project().page(new).unwrap();
        assert_eq!(p.name, "3");
        let order: Vec<&str> = doc
            .project()
            .sibling_pages(new)
            .iter()
            .map(|id| doc.project().page(*id).unwrap().title.as_str())
            .collect();
        assert_eq!(order, ["Motor", "Inserted", "Control"]);
    }

    #[test]
    fn renumber_follows_the_reading_order() {
        let mut doc = doc_with_tree();
        let s = DeviceTag::parse("=G1+A1").unwrap();
        let motor = doc
            .project()
            .pages()
            .find(|p| p.title == "Motor")
            .unwrap()
            .id;
        let control = doc
            .project()
            .pages()
            .find(|p| p.title == "Control")
            .unwrap()
            .id;
        doc.transact("order", |tx| tx.reorder_pages(&[control, motor]))
            .unwrap();
        let (changed, _) = doc.transact("renumber", |tx| renumber(tx, &s)).unwrap();
        assert_eq!(changed, 2);
        assert_eq!(doc.project().page(control).unwrap().name, "1");
        assert_eq!(doc.project().page(motor).unwrap().name, "2");
    }

    #[test]
    fn moving_a_page_renumbers_it_when_the_number_is_taken() {
        let mut doc = doc_with_tree();
        let other = doc
            .project()
            .pages()
            .find(|p| p.title == "Other")
            .unwrap()
            .id;
        // "Other" is page 1 of =G2+A1; =G1+A1 already has pages 1 and 2.
        doc.transact("move", |tx| {
            move_page(tx, other, Some(vec!["G1".into()]), None).map(|_| ())
        })
        .unwrap();
        let p = doc.project().page(other).unwrap();
        assert_eq!(p.structure.to_string(), "=G1+A1");
        assert_eq!(p.name, "3");
    }

    #[test]
    fn a_page_moved_to_a_new_name_creates_the_node() {
        let mut doc = doc_with_tree();
        let loose = doc
            .project()
            .pages()
            .find(|p| p.title == "Loose")
            .unwrap()
            .id;
        doc.transact("move", |tx| {
            move_page(tx, loose, Some(vec!["G9".into()]), Some(vec!["A9".into()])).map(|_| ())
        })
        .unwrap();
        assert!(
            doc.project()
                .structure_by_tag(&DeviceTag::parse("=G9").unwrap())
                .is_some()
        );
        let cab = doc
            .project()
            .structure_by_tag(&DeviceTag::parse("+A9").unwrap())
            .unwrap();
        assert!(cab.enclosure);
    }

    #[test]
    fn a_cabinet_gets_a_box_plate_and_a_first_page_in_its_group() {
        let mut doc = doc_with_tree();
        let spec = CabinetSpec {
            name: "A3".into(),
            group: vec!["G1".into()],
            plate: MountingPlate {
                width: 500.0,
                height: 700.0,
                margin: 10.0,
            },
            first_page: true,
        };
        let (page, _) = doc
            .transact("cabinet", |tx| add_cabinet(tx, &spec))
            .unwrap();
        let page = page.unwrap();
        assert_eq!(
            doc.project().page(page).unwrap().structure.to_string(),
            "=G1+A3"
        );
        let s = doc
            .project()
            .structure_by_tag(&DeviceTag::parse("+A3").unwrap())
            .unwrap();
        assert!(s.enclosure);
        assert_eq!(s.plate.unwrap().width, 500.0);
        // And one undo takes all of it back.
        doc.undo().unwrap();
        assert!(
            doc.project()
                .structure_by_tag(&DeviceTag::parse("+A3").unwrap())
                .is_none()
        );
    }

    #[test]
    fn a_group_comes_with_its_first_page_and_undoes_in_one_step() {
        let mut doc = doc_with_tree();
        let name = Naming::default().next_group(doc.project());
        assert_eq!(name, "G3");
        let (page, _) = doc
            .transact("group", |tx| add_group(tx, &name, None))
            .unwrap();
        assert_eq!(
            doc.project().page(page).unwrap().structure.to_string(),
            "=G3"
        );
        doc.undo().unwrap();
        assert!(
            doc.project()
                .page_groups()
                .iter()
                .all(|g| g.to_string() != "=G3")
        );
    }

    #[test]
    fn removing_a_group_keeps_its_pages_and_deleting_takes_them() {
        let mut doc = doc_with_tree();
        let g1 = vec!["G1".to_owned()];
        let before = doc.project().pages().count();
        let (n, _) = doc
            .transact("remove", |tx| remove_level(tx, Aspect::Function, &g1))
            .unwrap();
        assert_eq!(n, 3);
        assert_eq!(doc.project().pages().count(), before);
        assert!(doc.project().page_groups().iter().all(|g| g.function != g1));
        // The pages are filed one level up; numbers stay unique per place.
        let loose: Vec<String> = doc
            .project()
            .pages()
            .filter(|p| p.structure.to_string().is_empty())
            .map(|p| p.name.clone())
            .collect();
        assert_eq!(loose.len(), 2, "Loose and Overview: {loose:?}");
        assert_ne!(loose[0], loose[1], "Overview took the next free number");
        let a1: Vec<String> = doc
            .project()
            .pages()
            .filter(|p| p.structure.to_string() == "+A1")
            .map(|p| p.name.clone())
            .collect();
        assert_eq!(a1, ["1", "2"]);
    }

    #[test]
    fn deleting_a_group_deletes_its_pages() {
        let mut doc = doc_with_tree();
        let g2 = vec!["G2".to_owned()];
        let (n, _) = doc
            .transact("delete", |tx| {
                delete_level_and_pages(tx, Aspect::Function, &g2)
            })
            .unwrap();
        assert_eq!(n, 1);
        assert!(doc.project().pages().all(|p| p.title != "Other"));
        doc.undo().unwrap();
        assert!(doc.project().pages().any(|p| p.title == "Other"));
    }

    #[test]
    fn a_duplicate_page_keeps_title_place_and_follows_its_source() {
        let mut doc = doc_with_tree();
        let motor = doc
            .project()
            .pages()
            .find(|p| p.title == "Motor")
            .unwrap()
            .id;
        let (copy, _) = doc.transact("dup", |tx| duplicate_page(tx, motor)).unwrap();
        let p = doc.project().page(copy).unwrap();
        assert_eq!(p.title, "Motor");
        assert_eq!(p.name, "3");
        let siblings = doc.project().sibling_pages(motor);
        let at = siblings.iter().position(|s| *s == motor).unwrap();
        assert_eq!(siblings[at + 1], copy);
    }

    #[test]
    fn every_target_has_a_real_menu() {
        let doc = doc_with_tree();
        let project = doc.project();
        let page = project.pages().find(|p| p.title == "Motor").unwrap().id;
        let count = |t: &Target| {
            menu(project, t)
                .iter()
                .filter(|i| !matches!(i, Item::Separator))
                .count()
        };
        assert!(count(&Target::Background) >= 5);
        assert!(count(&Target::GeneratedHeader) >= 3);
        assert!(count(&Target::Page(page)) >= 8);
        let g = Target::Group {
            function: vec!["G1".into()],
            context: DeviceTag::parse("=G1").unwrap(),
        };
        let c = Target::Cabinet {
            location: vec!["A1".into()],
            context: DeviceTag::parse("=G1+A1").unwrap(),
        };
        assert!(count(&g) >= 8);
        assert!(count(&c) >= 10);
    }

    #[test]
    fn a_page_menu_knows_its_ends() {
        let doc = doc_with_tree();
        let project = doc.project();
        let first = project.pages().find(|p| p.title == "Motor").unwrap().id;
        let items = menu(project, &Target::Page(first));
        let enabled = |label: &str| {
            items
                .iter()
                .any(|i| matches!(i, Item::Entry { label: l, enabled: true, .. } if l == label))
        };
        assert!(!enabled("Move up"), "first page cannot move up");
        assert!(enabled("Move down"));
        assert!(enabled("Duplicate page"));
    }
}
