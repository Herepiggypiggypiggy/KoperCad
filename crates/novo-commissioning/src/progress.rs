use std::collections::{BTreeMap, BTreeSet};

use novo_core::{DeviceKind, PageId, Project};

use crate::check::{CheckState, Target};
use crate::store::CommissioningStore;

/// A target with where it is drawn and which cabinet it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetInfo {
    pub target: Target,
    /// Tag or wire number, for lists and reports.
    pub label: String,
    /// The pages it is drawn on, in id order. Empty when it is not drawn.
    pub pages: Vec<PageId>,
    /// The location tag (`+CAB1`), empty for none.
    pub cabinet: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope<'a> {
    Project,
    Page(PageId),
    Cabinet(&'a str),
}

/// Check counts over a scope. Not-applicable checks count in `total` and as
/// done, like the printed report (`novo_reports::commissioning::Progress`),
/// so the app and the report show the same percentage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub not_applicable: usize,
    pub pending: usize,
}

impl Progress {
    /// Passed or not applicable.
    pub fn done(&self) -> usize {
        self.passed + self.not_applicable
    }

    /// Done over total; 0 for a scope with no checks (nothing is proven yet).
    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            100.0 * self.done() as f64 / self.total as f64
        }
    }
}

fn location(tag: &novo_core::DeviceTag) -> String {
    novo_core::DeviceTag {
        location: tag.location.clone(),
        ..Default::default()
    }
    .to_string()
}

/// Every target of the project, in wire, terminal, device order.
pub fn targets(project: &Project) -> Vec<TargetInfo> {
    let mut out = Vec::new();
    for wire in project.wires() {
        let cabinet = project
            .page(wire.page)
            .map_or_else(String::new, |p| location(&p.structure));
        out.push(TargetInfo {
            target: Target::Wire(wire.id),
            label: wire.number.clone().unwrap_or_else(|| wire.id.to_string()),
            pages: vec![wire.page],
            cabinet,
        });
    }
    for terminal in project.terminals() {
        let strip = project.device(terminal.strip);
        let pages: BTreeSet<PageId> = project
            .symbols_of_terminal(terminal.id)
            .iter()
            .map(|s| s.page)
            .collect();
        out.push(TargetInfo {
            target: Target::Terminal(terminal.id),
            label: strip.map_or_else(
                || terminal.name.clone(),
                |s| format!("{}:{}", s.tag, terminal.name),
            ),
            pages: pages.into_iter().collect(),
            cabinet: strip.map_or_else(String::new, |s| location(&s.tag)),
        });
    }
    for device in project
        .devices()
        .filter(|d| d.kind != DeviceKind::TerminalStrip)
    {
        let pages: BTreeSet<PageId> = project
            .symbols_of_device(device.id)
            .iter()
            .map(|s| s.page)
            .collect();
        out.push(TargetInfo {
            target: Target::Device(device.id),
            label: device.tag.to_string(),
            pages: pages.into_iter().collect(),
            cabinet: location(&device.tag),
        });
    }
    out
}

fn add(progress: &mut Progress, info: &TargetInfo, store: &CommissioningStore) {
    for &check in info.target.checks() {
        match store.state(info.target, check) {
            CheckState::NotApplicable => {
                progress.total += 1;
                progress.not_applicable += 1;
            }
            CheckState::Passed => {
                progress.total += 1;
                progress.passed += 1;
            }
            CheckState::Failed => {
                progress.total += 1;
                progress.failed += 1;
            }
            CheckState::Pending => {
                progress.total += 1;
                progress.pending += 1;
            }
        }
    }
}

fn in_scope(info: &TargetInfo, scope: Scope<'_>) -> bool {
    match scope {
        Scope::Project => true,
        Scope::Page(page) => info.pages.contains(&page),
        Scope::Cabinet(name) => info.cabinet == name,
    }
}

pub fn progress(project: &Project, store: &CommissioningStore, scope: Scope<'_>) -> Progress {
    let mut out = Progress::default();
    for info in targets(project).iter().filter(|i| in_scope(i, scope)) {
        add(&mut out, info, store);
    }
    out
}

/// Progress for every page that has a target, in page order. A target drawn
/// on several pages counts on each.
pub fn progress_by_page(project: &Project, store: &CommissioningStore) -> Vec<(PageId, Progress)> {
    let mut by_page: BTreeMap<PageId, Progress> = BTreeMap::new();
    for info in targets(project) {
        for &page in &info.pages {
            add(by_page.entry(page).or_default(), &info, store);
        }
    }
    let mut out: Vec<_> = by_page.into_iter().collect();
    let order: Vec<PageId> = project.pages_in_order().iter().map(|p| p.id).collect();
    out.sort_by_key(|(id, _)| order.iter().position(|p| p == id));
    out
}

/// Progress for every cabinet (location tag) that has a target, sorted by tag.
pub fn progress_by_cabinet(
    project: &Project,
    store: &CommissioningStore,
) -> Vec<(String, Progress)> {
    let mut by_cabinet: BTreeMap<String, Progress> = BTreeMap::new();
    for info in targets(project) {
        add(
            by_cabinet.entry(info.cabinet.clone()).or_default(),
            &info,
            store,
        );
    }
    by_cabinet.into_iter().collect()
}
