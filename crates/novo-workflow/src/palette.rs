//! The command field's search (Ctrl+K at the top, I and a tapped Space at
//! the pointer): one engine for commands, pages, devices and symbols. It
//! returns rows; how they are shown (a field at the pointer, a bar at the
//! top) is up to the app.
//!
//! The query is read as a small grammar (docs/design/interaction-model.md):
//!
//! - **A noun selects or jumps.** An exact tag (`K1`, `-K1`, `=A1+CAB-K1`)
//!   puts that device first, then every place it is drawn as `/page.column`
//!   rows; Enter on the device jumps to its main location. A terminal
//!   (`X1:2`) lists where it is drawn, a page number (`12`) opens the page.
//! - **A verb with a noun acts.** `renumber page`, `export pdf`, and a
//!   symbol mnemonic with a device: `no K1` places a contact of `-K1`.
//! - **A bare verb runs** (`problems`, `rules`).
//! - Anything else is a fuzzy search over all of the above.
//!
//! With an empty query the field is the menu: "Repeat last tool" first,
//! then recent picks, then every command by group, each with its key and,
//! when it can't run now, the reason. In place mode (Q) the field searches
//! symbols only and an empty query shows the favourites (1 to 9 first).

use novo_core::{AnyId, DeviceId, Match, PageId, Project, SymbolId};
use novo_symbols::Library;

use crate::commands::{self, COMMANDS, CommandInfo, Context, GROUPS};
use crate::fuzzy;
use crate::insert::{self, InsertSearch};
use crate::keymap::Keymap;
use crate::language;
use crate::locate;

/// What picking a row does.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    /// Run a command by id.
    Command(&'static str),
    /// Run a command with what was typed after its verb (`renumber page`
    /// gives `edit.renumber` with `page`). The command reads the argument.
    Run {
        command: &'static str,
        argument: String,
    },
    /// Open a page.
    Page(PageId),
    /// Select a device and jump to its main location.
    Device(DeviceId),
    /// Jump to one place a device is drawn.
    Location { device: DeviceId, symbol: SymbolId },
    /// Start placing a symbol (by library id).
    Symbol(String),
    /// Start placing a symbol as a function of an existing device.
    Child { symbol: String, device: DeviceId },
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

/// One row.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub target: Target,
    pub kind: Kind,
    pub label: String,
    /// Grey text after the label: a group, a page title, a location.
    pub detail: String,
    /// Heading the row sits under when the field lists by group (empty
    /// query); empty otherwise.
    pub group: &'static str,
    /// Picking this row as a step puts this text in the field instead of
    /// running it (`renumber `), so every typed form can also be built by
    /// picking rows: verb, then object, then parameter. `None` for rows
    /// that run or jump.
    pub completion: Option<String>,
    /// The key, for commands and favourites that have one.
    pub shortcut: Option<String>,
    /// Why the row can't be picked now; `None` when it can.
    pub disabled: Option<&'static str>,
    pub score: i32,
}

/// What the field is open for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Everything (Ctrl+K, Space).
    #[default]
    All,
    /// Symbols to place (Q).
    Place,
}

/// Everything the field searches.
pub struct Sources<'a> {
    pub project: Option<&'a Project>,
    pub library: Option<&'a Library>,
    pub keymap: &'a Keymap,
    pub context: Context,
}

/// The field's memory: recent picks, newest first, and the symbol
/// search's recents and favourites.
#[derive(Debug, Clone, Default)]
pub struct Palette {
    recent: Vec<Target>,
    symbols: InsertSearch,
}

/// How many rows a search returns at most.
pub const MAX_ROWS: usize = 50;
/// How many recent picks are remembered.
pub const RECENT_LIMIT: usize = 30;
/// How many recent picks the empty field lists above the groups.
pub const RECENT_SHOWN: usize = 5;

const EXACT: i32 = 100_000;

/// Group heading of the recent rows of an empty field.
pub const RECENT_GROUP: &str = "Recent";
/// Group heading of the favourites in place mode.
pub const FAVOURITES_GROUP: &str = "Favourites";

impl Palette {
    /// Notes a pick, so it ranks first next time.
    pub fn picked(&mut self, target: Target) {
        match &target {
            Target::Symbol(id) | Target::Child { symbol: id, .. } => self.symbols.used(id),
            _ => {}
        }
        self.recent.retain(|t| *t != target);
        self.recent.insert(0, target);
        self.recent.truncate(RECENT_LIMIT);
    }

    pub fn recent(&self) -> &[Target] {
        &self.recent
    }

    /// The symbol search, for its favourites (1 to 9) and recents.
    pub fn symbols(&self) -> &InsertSearch {
        &self.symbols
    }

    pub fn symbols_mut(&mut self) -> &mut InsertSearch {
        &mut self.symbols
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

    /// Rows for a query in the everything mode (Ctrl+K), best first.
    pub fn search(&self, query: &str, sources: &Sources<'_>) -> Vec<Row> {
        self.search_in(Mode::All, query, sources)
    }

    /// Rows for a query, best first.
    pub fn search_in(&self, mode: Mode, raw: &str, sources: &Sources<'_>) -> Vec<Row> {
        // A trailing space after a verb asks for its objects or chips.
        let wants_next = raw.ends_with(char::is_whitespace);
        let query = raw.trim();
        if query.is_empty() {
            return match mode {
                Mode::All => self.menu(sources),
                Mode::Place => self.favourites(sources),
            };
        }
        let mut rows: Vec<Row> = Vec::new();
        if let Some(project) = sources.project {
            child_rows(project, sources, query, wants_next, &mut rows);
        }
        if mode == Mode::All {
            if let Some(project) = sources.project {
                project_rows(project, query, &mut rows);
                // `page 2`, `pagina 2`: the page named after the word.
                if let Some(name) = page_query(query) {
                    let mut pages = Vec::new();
                    project_rows(project, name, &mut pages);
                    rows.extend(pages.into_iter().filter(|r| r.kind == Kind::Page));
                }
            }
            command_rows(query, wants_next, sources, &mut rows);
        }
        if let Some(library) = sources.library {
            for hit in self.symbols.search(library, query).into_iter().take(10) {
                rows.push(Row {
                    target: Target::Symbol(hit.symbol.id.clone()),
                    kind: Kind::Symbol,
                    label: hit.symbol.name.clone(),
                    detail: hit
                        .mnemonic
                        .map_or_else(|| hit.symbol.category.clone(), |m| format!("`{m}`")),
                    group: "",
                    completion: None,
                    shortcut: None,
                    disabled: (!sources.context.has_page).then_some("No page is open"),
                    // Symbols come after commands and devices of equal match.
                    score: hit.score.min(EXACT / 2) - 50,
                });
                // A contact can belong to a device: offer "of a device..."
                // as a step, which completes to `no ` and lists devices.
                if insert::is_child_function(hit.symbol)
                    && let Some(alias) = hit.symbol.aliases.first()
                    && sources.project.is_some()
                {
                    rows.push(Row {
                        target: Target::Run {
                            command: "tool.place",
                            argument: format!("{alias} "),
                        },
                        kind: Kind::Symbol,
                        label: format!("{} of a device…", hit.symbol.name),
                        detail: format!("`{alias} K1`"),
                        group: "",
                        completion: Some(format!("{alias} ")),
                        shortcut: None,
                        disabled: (!sources.context.has_page).then_some("No page is open"),
                        score: hit.score.min(EXACT / 2) - 60,
                    });
                }
            }
        }

        // Recent picks first for a short query.
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

    /// The empty field: repeat, recents, then every command by group.
    fn menu(&self, sources: &Sources<'_>) -> Vec<Row> {
        let mut rows = Vec::new();
        if let Some(info) = commands::get("tool.repeat") {
            rows.push(command_row(info, sources, "", 0));
        }
        for target in self.recent.iter().take(RECENT_SHOWN) {
            if let Target::Command(id) = target
                && let Some(info) = commands::get(id)
            {
                rows.push(command_row(info, sources, RECENT_GROUP, 0));
            }
        }
        for group in GROUPS {
            for info in COMMANDS.iter().filter(|c| c.group == group) {
                if info.id != "tool.repeat" {
                    rows.push(command_row(info, sources, group, 0));
                }
            }
        }
        rows
    }

    /// The empty place field: favourites 1 to 9, then recent symbols.
    fn favourites(&self, sources: &Sources<'_>) -> Vec<Row> {
        let Some(library) = sources.library else {
            return Vec::new();
        };
        let disabled = (!sources.context.has_page).then_some("No page is open");
        let mut rows = Vec::new();
        for slot in 1..=9 {
            if let Some(symbol) = self.symbols.favourite(library, slot) {
                rows.push(Row {
                    target: Target::Symbol(symbol.id.clone()),
                    kind: Kind::Symbol,
                    label: symbol.name.clone(),
                    detail: symbol.category.clone(),
                    group: FAVOURITES_GROUP,
                    completion: None,
                    shortcut: Some(slot.to_string()),
                    disabled,
                    score: 0,
                });
            }
        }
        for symbol in library.favorites() {
            if !rows
                .iter()
                .any(|r| r.target == Target::Symbol(symbol.id.clone()))
            {
                rows.push(Row {
                    target: Target::Symbol(symbol.id.clone()),
                    kind: Kind::Symbol,
                    label: symbol.name.clone(),
                    detail: symbol.category.clone(),
                    group: FAVOURITES_GROUP,
                    completion: None,
                    shortcut: None,
                    disabled,
                    score: 0,
                });
            }
        }
        for id in self.symbols.recent() {
            if rows.iter().any(|r| r.target == Target::Symbol(id.clone())) {
                continue;
            }
            if let Some(symbol) = library.get(id) {
                rows.push(Row {
                    target: Target::Symbol(symbol.id.clone()),
                    kind: Kind::Symbol,
                    label: symbol.name.clone(),
                    detail: symbol.category.clone(),
                    group: RECENT_GROUP,
                    completion: None,
                    shortcut: None,
                    disabled,
                    score: 0,
                });
            }
        }
        rows
    }
}

/// `no K1`: a symbol alias and an existing device place a child symbol.
/// `no ` (an alias and a space) lists the devices it can belong to, so the
/// object can be picked instead of typed.
fn child_rows(
    project: &Project,
    sources: &Sources<'_>,
    query: &str,
    wants_next: bool,
    rows: &mut Vec<Row>,
) {
    let Some(library) = sources.library else {
        return;
    };
    let disabled = (!sources.context.has_page).then_some("No page is open");
    let mut push = |symbol: &novo_symbols::Symbol, device: DeviceId, score: i32| {
        let Some(d) = project.device(device) else {
            return;
        };
        rows.push(Row {
            target: Target::Child {
                symbol: symbol.id.clone(),
                device,
            },
            kind: Kind::Symbol,
            label: format!("{} of {}", symbol.name, d.tag),
            detail: d.description.clone(),
            group: "",
            completion: None,
            shortcut: None,
            disabled,
            score,
        });
    };
    if wants_next && let Some(symbol) = insert::mnemonic(library, query) {
        // Devices that already have a main function (a coil) first, in
        // natural tag order (-K2 before -K10): those are the ones contacts
        // belong to.
        let mut devices: Vec<_> = project
            .devices()
            .filter(|d| d.kind == novo_core::DeviceKind::General)
            .map(|d| {
                let has_main = project.cross_references(d.id).iter().any(|r| r.main);
                (!has_main, d.tag.to_string(), d.id)
            })
            .collect();
        devices.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| novo_core::natural_cmp(&a.1, &b.1))
                .then(a.2.cmp(&b.2))
        });
        for (n, (_, _, id)) in devices.into_iter().enumerate() {
            push(symbol, id, EXACT + 400 - i32::try_from(n).unwrap_or(0));
        }
        return;
    }
    for child in insert::child_queries(query) {
        let Some(symbol) = insert::mnemonic(library, child.text) else {
            continue;
        };
        let devices: Vec<DeviceId> = project
            .search(child.parent)
            .into_iter()
            .filter(|h| h.quality != Match::Contains)
            .filter_map(|h| match h.id {
                AnyId::Device(d) => Some((h.quality, d)),
                _ => None,
            })
            .map(|(_, d)| d)
            .collect();
        for (n, device) in devices.into_iter().enumerate() {
            push(symbol, device, EXACT + 500 - i32::try_from(n).unwrap_or(0));
        }
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
                    group: "",
                    completion: None,
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
                        group: "",
                        completion: None,
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
                    group: "",
                    completion: None,
                    shortcut: None,
                    disabled: None,
                    score: base,
                });
            }
            _ => {}
        }
    }
}

/// Words that name a page, in English and Dutch.
const PAGE_WORDS: [&str; 4] = ["page", "pagina", "blad", "sheet"];

/// What follows a page word (`page 2` gives `2`), if the query starts with
/// one.
fn page_query(query: &str) -> Option<&str> {
    let (word, rest) = query.split_once(char::is_whitespace)?;
    let rest = rest.trim();
    (PAGE_WORDS.iter().any(|w| w.eq_ignore_ascii_case(word)) && !rest.is_empty()).then_some(rest)
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
            group: "",
            completion: None,
            shortcut: None,
            disabled: None,
            score: base - 1 - i32::try_from(i).unwrap_or(0),
        });
    }
}

fn command_row(
    info: &'static CommandInfo,
    sources: &Sources<'_>,
    group: &'static str,
    score: i32,
) -> Row {
    Row {
        target: Target::Command(info.id),
        kind: Kind::Command,
        label: info.label.to_owned(),
        detail: info.group.to_owned(),
        group,
        completion: None,
        shortcut: sources.keymap.shortcut_text(info.id),
        disabled: commands::availability(info, &sources.context).err(),
        score,
    }
}

/// Every verb phrase of a command, English then Dutch.
fn verbs_of(info: &CommandInfo) -> impl Iterator<Item = &'static str> {
    let dutch = language::dutch(info.id).map_or(&[][..], |a| a.verbs);
    info.verbs.iter().chain(dutch).copied()
}

/// The verb phrase of `command` that starts `query` as whole words, and
/// what follows it. Verbs are ASCII, so folding ASCII case keeps the byte
/// offset valid in the typed text.
fn split_verb<'q>(info: &CommandInfo, query: &'q str) -> Option<(&'static str, &'q str)> {
    verbs_of(info)
        .filter_map(|v| {
            let head = query.get(..v.len())?;
            let rest = &query[v.len()..];
            (head.eq_ignore_ascii_case(v) && rest.chars().next().is_none_or(char::is_whitespace))
                .then_some((v, rest.trim()))
        })
        .max_by_key(|(v, _)| v.len())
}

fn command_rows(query: &str, wants_next: bool, sources: &Sources<'_>, rows: &mut Vec<Row>) {
    // The longest verb phrase typed wins: `accept all` is the accept-all
    // command, not `accept` with the argument `all`.
    let longest = COMMANDS
        .iter()
        .filter_map(|info| split_verb(info, query))
        .map(|(verb, _)| verb.len())
        .max();
    for info in COMMANDS {
        // A verb: bare it runs, with words after it it runs on them.
        if let Some((verb, argument)) = split_verb(info, query)
            && Some(verb.len()) == longest
        {
            // A Dutch argument runs as its English chip (`pagina` is `page`).
            let argument = language::english_argument(argument);
            if argument.is_empty() {
                rows.push(command_row(info, sources, "", EXACT + 200));
            } else {
                rows.push(run_row(info, sources, argument, EXACT + 300));
            }
            // Chips: the arguments the command offers, matching what is typed.
            if argument.is_empty() && !wants_next {
                continue;
            }
            let typed = argument.to_lowercase();
            for (n, chip) in info.arguments.iter().enumerate() {
                let dutch = language::dutch_argument(chip).unwrap_or(chip);
                if (chip.starts_with(&typed) || dutch.starts_with(&typed)) && *chip != argument {
                    let score = EXACT + 250 - i32::try_from(n).unwrap_or(0);
                    rows.push(run_row(info, sources, chip, score));
                }
            }
            continue;
        }
        let full = format!("{} {}", info.group, info.label);
        let dutch = language::dutch(info.id).map_or(&[][..], |a| a.words);
        let words = [info.label]
            .into_iter()
            .chain(info.keywords.iter().copied())
            .chain(verbs_of(info))
            .chain(dutch.iter().copied());
        let score = fuzzy::score(query, &full).max(fuzzy::best_score(query, words));
        if let Some(score) = score {
            let mut row = command_row(info, sources, "", score);
            // A verb that takes chips completes to itself first.
            if !info.arguments.is_empty() {
                row.completion = info.verbs.first().map(|v| format!("{v} "));
            }
            rows.push(row);
        }
    }
}

fn run_row(info: &'static CommandInfo, sources: &Sources<'_>, argument: &str, score: i32) -> Row {
    Row {
        target: Target::Run {
            command: info.id,
            argument: argument.to_owned(),
        },
        label: format!("{}: {argument}", info.label),
        ..command_row(info, sources, "", score)
    }
}

fn join_detail(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_owned(),
        (_, true) => a.to_owned(),
        _ => format!("{a}  {b}"),
    }
}
