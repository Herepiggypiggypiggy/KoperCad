//! The Ctrl+K search (J8): commands, pages, devices and symbols in one list.
//!
//! - Each row says what it is and shows the command's shortcut, so the
//!   palette teaches the keys.
//! - An exact tag (`-K1`, `K1`, `=A1+CAB1-K1`) puts that device first, then
//!   every place it is drawn (page and column) as rows to jump to. This is
//!   the "search can't find my component" fix: it works every time.
//! - A page name or number (`3`) puts that page first.
//! - Recently picked rows rank first for an empty or short query, so the
//!   last command used is the first row when the palette opens.
//! - Unavailable commands stay in the list, greyed, with the reason.
//!
//! The UI (WS-01) draws [`Row`]s and runs the picked [`Target`].

use novo_core::{AnyId, DeviceId, Match, PageId, Project, SymbolId};
use novo_symbols::Library;

use crate::commands::{self, COMMANDS, Context};
use crate::fuzzy;
use crate::insert::InsertSearch;
use crate::keymap::Keymap;
use crate::locate;

/// What picking a row does.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    /// Run a command by id.
    Command(&'static str),
    /// Open a page.
    Page(PageId),
    /// Select a device and jump to its main symbol.
    Device(DeviceId),
    /// Jump to one place a device is drawn.
    Location { device: DeviceId, symbol: SymbolId },
    /// Start placing a symbol (by library id).
    Symbol(String),
}

/// What kind of thing a row is, shown as a label in front of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Command,
    Page,
    Device,
    Location,
    Symbol,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Command => "Command",
            Kind::Page => "Page",
            Kind::Device => "Device",
            Kind::Location => "Location",
            Kind::Symbol => "Symbol",
        }
    }
}

/// One palette row.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub target: Target,
    pub kind: Kind,
    pub label: String,
    /// Grey text after the label: a menu, a page title, a location.
    pub detail: String,
    /// The shortcut, for commands that have one.
    pub shortcut: Option<String>,
    /// Why the row can't be picked now; `None` when it can.
    pub disabled: Option<&'static str>,
    pub score: i32,
}

/// Everything the palette searches.
pub struct Sources<'a> {
    pub project: Option<&'a Project>,
    pub library: Option<&'a Library>,
    pub keymap: &'a Keymap,
    pub context: Context,
}

/// The palette's memory: recent picks, newest first.
#[derive(Debug, Clone, Default)]
pub struct Palette {
    recent: Vec<Target>,
    symbols: InsertSearch,
}

/// How many rows a search returns at most.
pub const MAX_ROWS: usize = 50;
/// How many recent picks are remembered.
pub const RECENT_LIMIT: usize = 30;

const EXACT: i32 = 100_000;

impl Palette {
    /// Notes a pick, so it ranks first next time.
    pub fn picked(&mut self, target: Target) {
        if let Target::Symbol(id) = &target {
            self.symbols.used(id);
        }
        self.recent.retain(|t| *t != target);
        self.recent.insert(0, target);
        self.recent.truncate(RECENT_LIMIT);
    }

    pub fn recent(&self) -> &[Target] {
        &self.recent
    }

    /// Restores recent picks saved by the app, newest first. Command ids
    /// that no longer exist are dropped.
    pub fn restore_recent_commands<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        for id in ids {
            if let Some(info) = commands::get(id) {
                let target = Target::Command(info.id);
                if !self.recent.contains(&target) {
                    self.recent.push(target);
                }
            }
        }
        self.recent.truncate(RECENT_LIMIT);
    }

    /// Rows for a query, best first.
    pub fn search(&self, query: &str, sources: &Sources<'_>) -> Vec<Row> {
        let query = query.trim();
        let mut rows: Vec<Row> = Vec::new();

        if let Some(project) = sources.project {
            project_rows(project, query, &mut rows);
        }
        command_rows(query, sources, &mut rows);
        if let (Some(library), false) = (sources.library, query.is_empty()) {
            for hit in self.symbols.search(library, query).into_iter().take(10) {
                rows.push(Row {
                    target: Target::Symbol(hit.symbol.id.clone()),
                    kind: Kind::Symbol,
                    label: hit.symbol.name.clone(),
                    detail: hit
                        .mnemonic
                        .map_or_else(|| hit.symbol.category.clone(), |m| format!("`{m}`")),
                    shortcut: None,
                    disabled: (!sources.context.has_page).then_some("No page is open"),
                    // Symbols come after commands and devices of equal match.
                    score: hit.score.min(EXACT / 2) - 50,
                });
            }
        }

        // Recent picks first for an empty or short query.
        if query.chars().count() <= 3 {
            for row in &mut rows {
                if let Some(rank) = self.recent.iter().position(|t| *t == row.target) {
                    row.score += 5_000 - i32::try_from(rank).unwrap_or(0) * 100;
                }
            }
        }

        // One row per target, keeping the best.
        rows.sort_by_key(|r| std::cmp::Reverse(r.score));
        let mut seen = Vec::new();
        rows.retain(|r| {
            if seen.contains(&r.target) {
                false
            } else {
                seen.push(r.target.clone());
                true
            }
        });
        rows.truncate(MAX_ROWS);
        rows
    }
}

/// Devices, terminals and pages from novo-core's search. Exact tag hits
/// come first, each followed by every place the device is drawn.
fn project_rows(project: &Project, query: &str, rows: &mut Vec<Row>) {
    for (n, hit) in project.search(query).into_iter().enumerate() {
        let order = i32::try_from(n).unwrap_or(i32::MAX / 2);
        let base = match hit.quality {
            Match::Exact => EXACT - order * 1_000,
            Match::Prefix => 2_000 - order,
            Match::Contains => 800 - order,
        };
        match hit.id {
            AnyId::Device(device_id) => {
                let Some(device) = project.device(device_id) else {
                    continue;
                };
                let places = locate::locations(project, device_id);
                let summary = places
                    .iter()
                    .map(locate::reference)
                    .collect::<Vec<_>>()
                    .join("  ");
                rows.push(Row {
                    target: Target::Device(device_id),
                    kind: Kind::Device,
                    label: device.tag.to_string(),
                    detail: join_detail(&device.description, &summary),
                    shortcut: None,
                    disabled: None,
                    score: base,
                });
                if hit.quality == Match::Exact {
                    location_rows(project, device_id, &device.tag.to_string(), base, rows);
                }
            }
            AnyId::Terminal(terminal) => {
                let places: Vec<_> = project
                    .symbols_of_terminal(terminal)
                    .into_iter()
                    .filter_map(|s| Some((s.id, project.location_of(s.id)?)))
                    .collect();
                let Some(strip) = project.terminal(terminal).map(|t| t.strip) else {
                    continue;
                };
                for (i, (symbol, location)) in places.into_iter().enumerate() {
                    rows.push(Row {
                        target: Target::Location {
                            device: strip,
                            symbol,
                        },
                        kind: Kind::Location,
                        label: format!("{} /{location}", hit.text),
                        detail: "terminal".to_owned(),
                        shortcut: None,
                        disabled: None,
                        score: base - i32::try_from(i).unwrap_or(0),
                    });
                }
            }
            AnyId::Page(page_id) => {
                let Some(page) = project.page(page_id) else {
                    continue;
                };
                rows.push(Row {
                    target: Target::Page(page_id),
                    kind: Kind::Page,
                    label: format!("Page {}", page.name),
                    detail: page.title.clone(),
                    shortcut: None,
                    disabled: None,
                    score: base,
                });
            }
            _ => {}
        }
    }
}

fn location_rows(project: &Project, device: DeviceId, tag: &str, base: i32, rows: &mut Vec<Row>) {
    for (i, place) in locate::locations(project, device).iter().enumerate() {
        let what = project
            .symbol(place.symbol)
            .map(|s| {
                s.symbol
                    .0
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .replace('_', " ")
            })
            .unwrap_or_default();
        rows.push(Row {
            target: Target::Location {
                device,
                symbol: place.symbol,
            },
            kind: Kind::Location,
            label: format!("{tag} {}", locate::reference(place)),
            detail: what,
            shortcut: None,
            disabled: None,
            score: base - 1 - i32::try_from(i).unwrap_or(0),
        });
    }
}

fn command_rows(query: &str, sources: &Sources<'_>, rows: &mut Vec<Row>) {
    for (order, info) in COMMANDS.iter().enumerate() {
        let full = format!("{} {}", info.menu, info.label);
        let score = if query.is_empty() {
            // Menu order.
            Some(-i32::try_from(order).unwrap_or(0))
        } else {
            fuzzy::best_score(
                query,
                [info.label, full.as_str()]
                    .into_iter()
                    .chain(info.keywords.iter().copied()),
            )
        };
        let Some(score) = score else {
            continue;
        };
        rows.push(Row {
            target: Target::Command(info.id),
            kind: Kind::Command,
            label: info.label.to_owned(),
            detail: info.menu.to_owned(),
            shortcut: sources.keymap.shortcut_text(info.id),
            disabled: commands::availability(info, &sources.context).err(),
            score,
        });
    }
}

fn join_detail(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_owned(),
        (_, true) => a.to_owned(),
        _ => format!("{a}  {b}"),
    }
}
