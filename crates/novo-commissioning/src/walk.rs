//! The guided walk: going wire by wire at the cabinet.
//!
//! [`WirePlan::of`] reads what a person standing at the machine needs for
//! each drawn wire: where it starts and ends, the ferrule text on each end,
//! its length, and whether it is a plain wire or a core of a cable. The
//! lengths and ferrules are the wire production figures
//! (`novo_reports::wire_cut_jobs`), so the walk shows what the bench made.
//! [`next_open`] picks the wire (or terminal, or device) to look at next.
//!
//! Computing a plan routes every wire, so callers keep it until the project
//! changes (`Project::revision`).

use std::collections::{BTreeMap, HashMap};

use novo_core::{Endpoint, PageId, Project, WireId};
use novo_reports::wire_cut_jobs;

use crate::check::{CheckState, Target};
use crate::progress::{TargetInfo, targets};
use crate::store::CommissioningStore;

/// What a drawn wire is made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireKind {
    /// A single conductor drawn between two pins.
    Plain,
    /// A core of a cable: `cable` is the tag (`-W1`), `core` its designation.
    Core { cable: String, core: String },
}

impl WireKind {
    /// Short text for a list row: `wire` or `core -W1:2`.
    pub fn short(&self) -> String {
        match self {
            WireKind::Plain => "wire".to_owned(),
            WireKind::Core { cable, core } => format!("core {cable}:{core}"),
        }
    }
}

/// One drawn wire as it is checked at the machine.
#[derive(Debug, Clone, PartialEq)]
pub struct WireStep {
    pub wire: WireId,
    /// The wire number, empty when it has none.
    pub number: String,
    pub kind: WireKind,
    /// `=A1+CAB-K1:13`, or empty when the end is open.
    pub from: String,
    pub to: String,
    /// Ferrule text for each end (`A1.5-10`), `None` when marked without one.
    pub ferrule_from: Option<String>,
    pub ferrule_to: Option<String>,
    /// The routed length with service loops, in millimetres.
    pub length_mm: Option<f64>,
    pub cross_section_mm2: Option<f64>,
    pub colour: String,
    /// Other drawn wires carrying the same number, as `kind` text with the
    /// page: the hint that a number seen twice is not a mistake.
    pub same_number: Vec<String>,
}

impl WireStep {
    /// The one-line hint for a number that appears more than once, empty
    /// when it is unique.
    pub fn same_number_hint(&self) -> String {
        if self.same_number.is_empty() {
            String::new()
        } else {
            format!("Same number also on {}", self.same_number.join(", "))
        }
    }
}

/// Every drawn wire's step, by wire id.
#[derive(Debug, Clone, Default)]
pub struct WirePlan {
    steps: BTreeMap<WireId, WireStep>,
}

impl WirePlan {
    pub fn of(project: &Project) -> WirePlan {
        // A production job covers the drawn wires of one connection.
        let mut job_of: HashMap<WireId, usize> = HashMap::new();
        let jobs = wire_cut_jobs(project);
        for (i, job) in jobs.iter().enumerate() {
            for &w in &job.wires {
                job_of.insert(w, i);
            }
        }
        let pin_text = |e: &Endpoint| {
            e.as_pin()
                .and_then(|p| project.pin_designation(p))
                .unwrap_or_default()
        };
        let mut steps = BTreeMap::new();
        for wire in project.wires() {
            let kind = match project.core_of_wire(wire.id) {
                Some((cable, core)) => project
                    .cable(cable)
                    .and_then(|c| c.cores.get(core).map(|k| (c, k)))
                    .map_or(WireKind::Plain, |(c, k)| WireKind::Core {
                        cable: c.tag.to_string(),
                        core: k.name.clone(),
                    }),
                None => WireKind::Plain,
            };
            let step = match job_of.get(&wire.id).map(|&i| &jobs[i]) {
                Some(job) => WireStep {
                    wire: wire.id,
                    number: wire.number.clone().unwrap_or_default(),
                    kind,
                    from: job.end_a.designation.clone(),
                    to: job.end_b.designation.clone(),
                    ferrule_from: job.end_a.ferrule.clone(),
                    ferrule_to: job.end_b.ferrule.clone(),
                    length_mm: Some(job.length_mm),
                    cross_section_mm2: job.cross_section_mm2,
                    colour: job.colour.clone(),
                    same_number: Vec::new(),
                },
                None => WireStep {
                    wire: wire.id,
                    number: wire.number.clone().unwrap_or_default(),
                    kind,
                    from: pin_text(&wire.from),
                    to: pin_text(&wire.to),
                    ferrule_from: None,
                    ferrule_to: None,
                    length_mm: None,
                    cross_section_mm2: wire.cross_section_mm2,
                    colour: wire.color.clone().unwrap_or_default(),
                    same_number: Vec::new(),
                },
            };
            steps.insert(wire.id, step);
        }
        // A number drawn on several wires: say what the others are.
        let mut by_number: HashMap<String, Vec<WireId>> = HashMap::new();
        for step in steps.values().filter(|s| !s.number.is_empty()) {
            by_number
                .entry(step.number.clone())
                .or_default()
                .push(step.wire);
        }
        for group in by_number.values().filter(|g| g.len() > 1) {
            for &id in group {
                let others: Vec<String> = group
                    .iter()
                    .filter(|&&o| o != id)
                    .filter_map(|o| steps.get(o).map(|s| (o, s)))
                    .map(|(o, s)| {
                        let page = project
                            .wire(*o)
                            .and_then(|w| project.page(w.page))
                            .map(|p| format!(" on {}", p.name))
                            .unwrap_or_default();
                        format!("{}{page}", s.kind.short())
                    })
                    .collect();
                if let Some(step) = steps.get_mut(&id) {
                    step.same_number = others;
                }
            }
        }
        WirePlan { steps }
    }

    pub fn step(&self, wire: WireId) -> Option<&WireStep> {
        self.steps.get(&wire)
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

fn is_open(store: &CommissioningStore, info: &TargetInfo) -> bool {
    info.target
        .checks()
        .iter()
        .any(|&c| store.state(info.target, c) == CheckState::Pending)
}

/// Which targets the walk visits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WalkScope {
    /// Wires, terminals and devices.
    #[default]
    All,
    /// Wires only: the wire-by-wire pass.
    Wires,
}

fn rank(target: Target) -> u8 {
    match target {
        Target::Wire(_) => 0,
        Target::Terminal(_) => 1,
        Target::Device(_) => 2,
    }
}

/// The targets in walking order: page by page in page order, then inside a
/// page wires by number, terminals and devices in the project's order. A target
/// drawn on several pages is visited on its first page.
pub fn walk_order(project: &Project, scope: WalkScope) -> Vec<TargetInfo> {
    let all = targets(project);
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for page in project.pages_in_order() {
        let mut on_page: Vec<&TargetInfo> = all
            .iter()
            .filter(|i| i.pages.first() == Some(&page.id))
            .filter(|i| scope == WalkScope::All || matches!(i.target, Target::Wire(_)))
            .collect();
        // Wires in number order, the way a person works through them; the
        // sort is stable, so terminals and devices keep the project's order.
        on_page.sort_by(|a, b| match (a.target, b.target) {
            (Target::Wire(_), Target::Wire(_)) => novo_core::natural_cmp(&a.label, &b.label),
            _ => rank(a.target).cmp(&rank(b.target)),
        });
        for info in on_page {
            if seen.insert(info.target) {
                out.push(info.clone());
            }
        }
    }
    // Targets that are drawn nowhere come last.
    for info in all.iter().filter(|i| i.pages.is_empty()) {
        if (scope == WalkScope::All || matches!(info.target, Target::Wire(_)))
            && seen.insert(info.target)
        {
            out.push(info.clone());
        }
    }
    out
}

/// The next target with an open check after `from` in walking order,
/// wrapping round to the start. With no `from` the walk starts on `page`
/// (the page being looked at), else on the first page. `None` when
/// everything in scope is decided.
pub fn next_open(
    project: &Project,
    store: &CommissioningStore,
    from: Option<Target>,
    page: Option<PageId>,
    scope: WalkScope,
) -> Option<TargetInfo> {
    let order = walk_order(project, scope);
    let start = from
        .and_then(|t| order.iter().position(|i| i.target == t).map(|p| p + 1))
        .or_else(|| page.and_then(|p| order.iter().position(|i| i.pages.first() == Some(&p))))
        .unwrap_or(0);
    order[start.min(order.len())..]
        .iter()
        .chain(order[..start.min(order.len())].iter())
        .find(|i| is_open(store, i) && Some(i.target) != from)
        .or_else(|| order.iter().find(|i| is_open(store, i)))
        .cloned()
}
