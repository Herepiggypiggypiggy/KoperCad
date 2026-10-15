//! Design checks (owned by WS-14).
//!
//! Problems are derived from the model, never stored: [`check`] reads a
//! [`Project`] through its queries and returns every [`Problem`] it finds, in
//! reading order, so the Issues list and F8 / Shift+F8 can walk them and the
//! canvas can mark each one in place (docs/DESIGN.md §2, §3, §5). Checks
//! never edit the project and never block anything: export, save and
//! editing go on whatever they report.
//!
//! What is checked, by [`ProblemKind`]:
//!
//! | Kind | Severity | Finds |
//! | --- | --- | --- |
//! | [`DuplicateTag`](ProblemKind::DuplicateTag) | error | One tag on several devices or cables |
//! | [`TooManyContacts`](ProblemKind::TooManyContacts) | error | More contacts of a kind drawn than the device has (`contacts.main/no/nc/co` device properties) |
//! | [`PotentialConflict`](ProblemKind::PotentialConflict) | error | Two potentials defined on one net (a short) |
//! | [`UnconnectedPin`](ProblemKind::UnconnectedPin) | warning | Connection points no wire ends on |
//! | [`LoneInterruption`](ProblemKind::LoneInterruption) | warning | An interruption point without a partner of the same name |
//! | [`NoMainFunction`](ProblemKind::NoMainFunction) | warning | A device drawn only through contacts, without its coil |
//! | [`SeveralMainFunctions`](ProblemKind::SeveralMainFunctions) | warning | A device drawn with more than one main function |
//! | [`DuplicateTerminal`](ProblemKind::DuplicateTerminal) | warning | Two terminals with one name on a strip while the same-name rule is off |
//! | [`ZeroLengthWire`](ProblemKind::ZeroLengthWire) | warning | A wire of zero length that is not two touching pins |
//! | [`WrongTerminalBlock`](ProblemKind::WrongTerminalBlock) | warning | A PE terminal built with a block the catalog does not mark PE, or another terminal built with a block it does |
//! | [`UndersizedWire`](ProblemKind::UndersizedWire) | warning | A wire on an overcurrent protective device that carries less than the device's rated current |
//! | [`CableCore`](ProblemKind::CableCore) | warning, error for green-yellow misuse | A cable core used against its marking or the cable's limits (novo-cables' checks) |
//! | [`TerminalUse`](ProblemKind::TerminalUse) | warning, error for a wrong potential or a one-sided fuse terminal | A terminal used against its kind (novo-terminals' checks) |
//! | [`Plc`](ProblemKind::Plc) | warning, error where the I/O list would be wrong | A PLC card or channel that cannot be addressed or wired as drawn (novo-plc's checks) |
//! | [`WireOverlap`](ProblemKind::WireOverlap) | warning | Two wires on a page sharing a stretch of line |
//! | [`MissingPart`](ProblemKind::MissingPart) | info | A device or cable without a part |
//!
//! Not checked here:
//! - Wires beyond the next device on a protective device's load side (past
//!   a contactor, say): the undersized-wire check follows the load-side net
//!   through terminals and cable cores, not through other devices' poles.
//! - A wire on two cable cores: novo-core refuses it on every edit and load
//!   (`ModelError::WireAlreadyOnCore`), so it cannot occur.
//!
//! ```
//! use novo_checks::{Severity, Summary, check};
//! use novo_core::Project;
//!
//! let project = Project::new("Empty");
//! let problems = check(&project);
//! assert!(problems.is_empty());
//! assert_eq!(Summary::of(&problems).pill_text(), "No issues");
//! ```

mod checks;
mod contacts;

use std::cmp::Ordering;
use std::collections::HashMap;

use novo_core::{
    AnyId, DeviceId, Endpoint, Netlist, PageId, Point, Project, SymbolInstance, SymbolRole,
    TerminalId,
};

pub use contacts::{ContactKind, contact_limit_property};

/// How bad a problem is. Orders from most to least severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// Wrong: the drawing says something that cannot be built as drawn.
    Error,
    /// Probably wrong or unfinished.
    Warning,
    /// Worth knowing, nothing wrong yet (a device still waiting for a part).
    Info,
}

/// What a problem is about. Each kind has one fixed [`Severity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProblemKind {
    /// A connection point of a placed symbol that no wire ends on. One
    /// problem per symbol, naming each of its free points.
    UnconnectedPin,
    /// One tag used by several devices or cables.
    DuplicateTag,
    /// Terminals of one strip sharing a name and level while the project's
    /// same-name rule is off, so they are separate points that read alike.
    DuplicateTerminal,
    /// A device drawn only through auxiliary functions (contacts), with no
    /// main function (coil).
    NoMainFunction,
    /// A device drawn with more than one main function.
    SeveralMainFunctions,
    /// More contacts of one kind drawn than the device has.
    TooManyContacts,
    /// An interruption point with no other interruption point of its name.
    LoneInterruption,
    /// Two or more potentials defined on one net: a short between them.
    PotentialConflict,
    /// A wire whose ends coincide, other than two touching pins.
    ZeroLengthWire,
    /// A device or cable without a part.
    MissingPart,
    /// A wire ending on an overcurrent protective device (fuse, breaker,
    /// overload relay) whose cross-section carries less current than the
    /// device is rated for.
    UndersizedWire,
    /// A terminal whose article does not suit its kind: a PE terminal built
    /// with an ordinary (grey) block, which leaves the protective conductor
    /// unbonded, or another terminal built with a PE block, which bonds a
    /// live conductor to the rail.
    WrongTerminalBlock,
    /// A cable core used against its marking or the cable's limits
    /// (novo-cables): a live conductor on the green-yellow core (error), PE
    /// on another core (error), a spare core in use, more wires than free
    /// cores, more cores than the cable's part, a floating shield, or a
    /// core thinner than its wire.
    CableCore,
    /// A terminal used against its kind (novo-terminals): a name that
    /// defines another kind (`PE` on a feed-through terminal), a wire whose
    /// potential does not fit the kind (error), a fuse or disconnect
    /// terminal wired on one side (error), or, under the refuse rule, a
    /// terminal drawn twice (error).
    TerminalUse,
    /// A PLC card or channel that cannot be addressed or wired as drawn
    /// (novo-plc, plc.md §7): overlapping addresses, a channel drawn twice
    /// or that the card does not have, a channel without a number or of
    /// the wrong type, a card without a layout or start address, or two
    /// cards in one slot. Errors where the I/O list would be wrong.
    Plc,
    /// Two wires on a page sharing a stretch of line (DESIGN.md §4: wires
    /// never overlap). The editor cannot create one; this finds those that
    /// come from imports, scripts and undo or redo. Wires leaving one pin
    /// together may share the stretch from that pin.
    WireOverlap,
}

impl ProblemKind {
    /// Every kind, in the order the table in the crate docs lists them.
    pub const ALL: [ProblemKind; 16] = [
        ProblemKind::DuplicateTag,
        ProblemKind::TooManyContacts,
        ProblemKind::PotentialConflict,
        ProblemKind::UnconnectedPin,
        ProblemKind::LoneInterruption,
        ProblemKind::NoMainFunction,
        ProblemKind::SeveralMainFunctions,
        ProblemKind::DuplicateTerminal,
        ProblemKind::ZeroLengthWire,
        ProblemKind::CableCore,
        ProblemKind::TerminalUse,
        ProblemKind::Plc,
        ProblemKind::WrongTerminalBlock,
        ProblemKind::UndersizedWire,
        ProblemKind::MissingPart,
        ProblemKind::WireOverlap,
    ];

    pub fn severity(self) -> Severity {
        match self {
            ProblemKind::DuplicateTag
            | ProblemKind::TooManyContacts
            | ProblemKind::PotentialConflict => Severity::Error,
            ProblemKind::UnconnectedPin
            | ProblemKind::LoneInterruption
            | ProblemKind::NoMainFunction
            | ProblemKind::SeveralMainFunctions
            | ProblemKind::DuplicateTerminal
            | ProblemKind::ZeroLengthWire
            | ProblemKind::UndersizedWire
            | ProblemKind::WrongTerminalBlock
            | ProblemKind::CableCore
            | ProblemKind::TerminalUse
            | ProblemKind::WireOverlap
            | ProblemKind::Plc => Severity::Warning,
            ProblemKind::MissingPart => Severity::Info,
        }
    }

    /// A short plain name, for filter chips in the Issues list.
    pub fn label(self) -> &'static str {
        match self {
            ProblemKind::UnconnectedPin => "Unconnected point",
            ProblemKind::DuplicateTag => "Duplicate tag",
            ProblemKind::DuplicateTerminal => "Duplicate terminal",
            ProblemKind::NoMainFunction => "No main function",
            ProblemKind::SeveralMainFunctions => "Several main functions",
            ProblemKind::TooManyContacts => "Too many contacts",
            ProblemKind::LoneInterruption => "Interruption point without partner",
            ProblemKind::PotentialConflict => "Potential conflict",
            ProblemKind::ZeroLengthWire => "Zero-length wire",
            ProblemKind::MissingPart => "No part",
            ProblemKind::UndersizedWire => "Undersized wire",
            ProblemKind::WrongTerminalBlock => "Wrong terminal block",
            ProblemKind::CableCore => "Cable core",
            ProblemKind::TerminalUse => "Terminal use",
            ProblemKind::Plc => "PLC",
            ProblemKind::WireOverlap => "Wires overlap",
        }
    }
}

/// One finding of the design checks.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub kind: ProblemKind,
    pub severity: Severity,
    /// The objects to mark, drawn objects (symbols, wires) first where the
    /// problem has any. A device, terminal or cable that is not drawn
    /// anywhere is named itself, so the Issues list can still select it.
    pub at: Vec<AnyId>,
    /// The page of the first marked object in reading order, where F8 goes.
    /// `None` when nothing of the problem is drawn.
    pub page: Option<PageId>,
    /// Where on [`page`](Problem::page) that object sits, in mm.
    pub position: Option<Point>,
    /// One plain sentence, shown on hover and in the Issues list.
    pub message: String,
}

impl Problem {
    pub(crate) fn new(kind: ProblemKind, at: Vec<AnyId>, message: String) -> Problem {
        Problem {
            kind,
            severity: kind.severity(),
            at,
            page: None,
            position: None,
            message,
        }
    }
}

/// Problem counts by severity, for the issue pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Summary {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}

impl Summary {
    pub fn of(problems: &[Problem]) -> Summary {
        let mut summary = Summary::default();
        for problem in problems {
            match problem.severity {
                Severity::Error => summary.errors += 1,
                Severity::Warning => summary.warnings += 1,
                Severity::Info => summary.infos += 1,
            }
        }
        summary
    }

    /// No errors and no warnings. Infos do not count as issues.
    pub fn is_clean(&self) -> bool {
        self.errors == 0 && self.warnings == 0
    }

    /// The issue pill's text: `No issues`, `1 error`, `5 warnings` or
    /// `2 errors · 5 warnings`. Infos are listed in Issues but not counted
    /// on the pill, which counts things to fix (DESIGN.md §3).
    pub fn pill_text(&self) -> String {
        let count =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        match (self.errors, self.warnings) {
            (0, 0) => "No issues".to_owned(),
            (e, 0) => count(e, "error", "errors"),
            (0, w) => count(w, "warning", "warnings"),
            (e, w) => format!(
                "{} · {}",
                count(e, "error", "errors"),
                count(w, "warning", "warnings")
            ),
        }
    }
}

/// Runs every check on the project. Builds a netlist for the potential
/// check; an app that already keeps a current netlist calls
/// [`check_with`] instead.
pub fn check(project: &Project) -> Vec<Problem> {
    check_with(project, &project.netlist())
}

/// Runs every check, reading connectivity from `netlist`, which must be
/// current for `project` ([`Netlist::is_current`]).
pub fn check_with(project: &Project, netlist: &Netlist) -> Vec<Problem> {
    debug_assert!(netlist.is_current(project), "netlist is stale");
    let cx = Context::new(project);
    let mut problems = Vec::new();
    checks::unconnected_pins(&cx, &mut problems);
    checks::duplicate_tags(&cx, &mut problems);
    checks::duplicate_terminals(&cx, &mut problems);
    checks::device_functions(&cx, &mut problems);
    checks::lone_interruptions(&cx, &mut problems);
    checks::potential_conflicts(&cx, netlist, &mut problems);
    checks::zero_length_wires(&cx, &mut problems);
    checks::missing_parts(&cx, &mut problems);
    checks::undersized_wires(&cx, netlist, &mut problems);
    checks::wrong_terminal_blocks(&cx, &mut problems);
    checks::cable_cores(&cx, netlist, &mut problems);
    checks::terminal_use(&cx, netlist, &mut problems);
    checks::plc(&cx, &mut problems);
    checks::wire_overlaps(&cx, &mut problems);
    cx.sort(&mut problems);
    problems
}

/// Where something sits in reading order: page rank, then x, then y, as
/// `Project::cross_references` orders symbols.
#[derive(Debug, Clone, Copy)]
struct Spot {
    rank: usize,
    page: PageId,
    at: Point,
}

impl Spot {
    fn cmp(&self, other: &Spot) -> Ordering {
        self.rank
            .cmp(&other.rank)
            .then(self.page.cmp(&other.page))
            .then(self.at.x.total_cmp(&other.at.x))
            .then(self.at.y.total_cmp(&other.at.y))
    }
}

fn symbol_spot(page_rank: &HashMap<PageId, usize>, symbol: &SymbolInstance) -> Spot {
    Spot {
        rank: page_rank.get(&symbol.page).copied().unwrap_or(usize::MAX),
        page: symbol.page,
        at: symbol.placement.origin,
    }
}

/// Lookups shared by the checks, built in one pass.
pub(crate) struct Context<'a> {
    pub(crate) project: &'a Project,
    page_rank: HashMap<PageId, usize>,
    /// The symbols of each device: main functions first, then reading order.
    pub(crate) by_device: HashMap<DeviceId, Vec<&'a SymbolInstance>>,
    /// The symbols drawing each terminal, in reading order.
    pub(crate) by_terminal: HashMap<TerminalId, Vec<&'a SymbolInstance>>,
}

impl<'a> Context<'a> {
    fn new(project: &'a Project) -> Context<'a> {
        let page_rank: HashMap<PageId, usize> = project
            .pages_in_order()
            .iter()
            .enumerate()
            .map(|(rank, page)| (page.id, rank))
            .collect();
        let mut by_device: HashMap<DeviceId, Vec<&SymbolInstance>> = HashMap::new();
        let mut by_terminal: HashMap<TerminalId, Vec<&SymbolInstance>> = HashMap::new();
        for symbol in project.symbols() {
            match &symbol.role {
                SymbolRole::Device { device, .. } => {
                    by_device.entry(*device).or_default().push(symbol)
                }
                SymbolRole::Terminal { terminals } => {
                    for terminal in terminals {
                        by_terminal.entry(*terminal).or_default().push(symbol);
                    }
                }
                _ => {}
            }
        }
        let order = |a: &&SymbolInstance, b: &&SymbolInstance| {
            symbol_spot(&page_rank, a)
                .cmp(&symbol_spot(&page_rank, b))
                .then(a.id.cmp(&b.id))
        };
        let is_aux = |s: &SymbolInstance| !matches!(s.role, SymbolRole::Device { main: true, .. });
        for symbols in by_device.values_mut() {
            symbols.sort_by(|a, b| is_aux(a).cmp(&is_aux(b)).then_with(|| order(a, b)));
        }
        for symbols in by_terminal.values_mut() {
            symbols.sort_by(order);
        }
        Context {
            project,
            page_rank,
            by_device,
            by_terminal,
        }
    }

    fn symbol_spot(&self, symbol: &SymbolInstance) -> Spot {
        symbol_spot(&self.page_rank, symbol)
    }

    /// Where an object is drawn first, in reading order.
    fn spot(&self, id: AnyId) -> Option<Spot> {
        let project = self.project;
        let on = |page: PageId, at: Point| Spot {
            rank: self.page_rank.get(&page).copied().unwrap_or(usize::MAX),
            page,
            at,
        };
        match id {
            AnyId::Symbol(s) => project.symbol(s).map(|s| self.symbol_spot(s)),
            AnyId::Wire(w) => {
                let wire = project.wire(w)?;
                Some(on(wire.page, project.endpoint_position(wire.from)?))
            }
            AnyId::Junction(j) => project.junction(j).map(|j| on(j.page, j.position)),
            AnyId::Device(d) => self.by_device.get(&d)?.first().map(|s| self.symbol_spot(s)),
            AnyId::Terminal(t) => self
                .by_terminal
                .get(&t)?
                .first()
                .map(|s| self.symbol_spot(s)),
            AnyId::Cable(c) => project
                .cable(c)?
                .cores
                .iter()
                .filter_map(|core| self.spot(AnyId::Wire(core.wire?)))
                .min_by(Spot::cmp),
            _ => None,
        }
    }

    /// Fills in each problem's page and position and sorts them in reading
    /// order. Problems with nothing drawn come last; ties go by kind, then
    /// message, then the marked ids, so the order is the same every run.
    fn sort(&self, problems: &mut Vec<Problem>) {
        let mut keyed: Vec<(Option<Spot>, Problem)> = problems
            .drain(..)
            .map(|mut problem| {
                let spot = problem
                    .at
                    .iter()
                    .filter_map(|&id| self.spot(id))
                    .min_by(Spot::cmp);
                if let Some(spot) = spot {
                    problem.page = Some(spot.page);
                    problem.position = Some(spot.at);
                }
                (spot, problem)
            })
            .collect();
        keyed.sort_unstable_by(|(a, p), (b, q)| {
            let place = match (a, b) {
                (Some(a), Some(b)) => a.cmp(b),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };
            place
                .then(p.kind.cmp(&q.kind))
                .then_with(|| p.message.cmp(&q.message))
                .then_with(|| p.at.cmp(&q.at))
        });
        problems.extend(keyed.into_iter().map(|(_, p)| p));
    }

    /// How a symbol is named in messages: its device tag, terminal
    /// designation, interruption or potential name, or its library key.
    pub(crate) fn symbol_label(&self, symbol: &SymbolInstance) -> String {
        let project = self.project;
        match &symbol.role {
            SymbolRole::Device { device, .. } => project
                .device(*device)
                .map_or_else(|| symbol.symbol.0.clone(), |d| d.tag.to_string()),
            SymbolRole::Terminal { terminals } => terminals
                .first()
                .and_then(|&t| project.terminal_designation(t))
                .unwrap_or_else(|| symbol.symbol.0.clone()),
            SymbolRole::Interruption { name } => format!("Interruption point {name}"),
            SymbolRole::PotentialFeed { potential } => project.potential(*potential).map_or_else(
                || "Potential feed".to_owned(),
                |p| format!("Feed {}", p.name),
            ),
            SymbolRole::Unassigned => symbol.symbol.0.clone(),
        }
    }

    /// How an endpoint is named in messages: `-K1:13` or `a junction`.
    pub(crate) fn endpoint_label(&self, end: Endpoint) -> String {
        match end {
            Endpoint::Pin { symbol, pin } => {
                let Some(s) = self.project.symbol(symbol) else {
                    return "a missing pin".to_owned();
                };
                let name = s
                    .pins
                    .get(usize::from(pin))
                    .map_or("?", |p| p.name.as_str());
                format!("{}:{name}", self.symbol_label(s))
            }
            Endpoint::Junction { .. } => "a junction".to_owned(),
            Endpoint::Free { .. } => "an open end".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pill_text_follows_counts() {
        let s = |errors, warnings, infos| Summary {
            errors,
            warnings,
            infos,
        };
        assert_eq!(s(0, 0, 3).pill_text(), "No issues");
        assert_eq!(s(1, 0, 0).pill_text(), "1 error");
        assert_eq!(s(0, 1, 0).pill_text(), "1 warning");
        assert_eq!(s(0, 5, 0).pill_text(), "5 warnings");
        assert_eq!(s(2, 5, 1).pill_text(), "2 errors · 5 warnings");
        assert_eq!(s(1, 1, 0).pill_text(), "1 error · 1 warning");
        assert!(s(0, 0, 4).is_clean());
        assert!(!s(0, 1, 0).is_clean());
    }

    #[test]
    fn every_kind_is_listed_once() {
        let mut kinds = ProblemKind::ALL.to_vec();
        kinds.sort();
        kinds.dedup();
        assert_eq!(kinds.len(), ProblemKind::ALL.len());
        assert!(ProblemKind::ALL.iter().all(|k| !k.label().is_empty()));
    }
}
