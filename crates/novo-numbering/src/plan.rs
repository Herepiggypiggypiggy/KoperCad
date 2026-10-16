//! Planning and applying numbers.
//!
//! A [`Plan`] is computed from the project without changing it, shown as a
//! preview, and applied as one undo step.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::{ModelError, PageId, Project, Result, Tx, Wire, WireId};

use crate::check::{Conflict, conflicts_of};
use crate::group::{Group, Target, count_after, fits, groups};
use crate::scheme::Scheme;

/// What to do with existing numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Number what has no number and settle what is unsettled. Every number
    /// that is already there and unique stays, whatever the scheme says, so
    /// a small edit never moves the rest. This is what placing a macro and
    /// "Number new wires" run.
    #[default]
    Fill,
    /// Like [`Mode::Fill`], but a number that does not look like the scheme
    /// would make it is replaced too.
    FillStrict,
    /// Number everything in reading order from the start of each series.
    /// Hand-typed (locked) numbers stay.
    Renumber,
}

/// Which wires a plan may change.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Scope {
    #[default]
    Project,
    Pages(Vec<PageId>),
    /// The groups these wires belong to (a net is numbered as a whole).
    Wires(Vec<WireId>),
}

/// Why a group has the number it gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Already numbered and untouched.
    Kept,
    /// Hand-typed, never changed by the engine.
    Locked,
    /// Was not numbered.
    New,
    /// Some wires of the net differed or had none; they follow the rest.
    Healed,
    /// Replaced: the old number clashed or is off the scheme.
    Replaced,
    /// Part of a full renumber.
    Renumbered,
    /// Could not be given a number (the one it needs is taken).
    Blocked,
}

/// One group of wires in a plan.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupPlan {
    pub wires: Vec<WireId>,
    /// `page.column`, for the preview.
    pub at: String,
    /// What the potential is called, when the net carries one.
    pub potential: Option<String>,
    pub old: Vec<String>,
    pub new: Option<String>,
    pub reason: Reason,
}

/// One wire whose number changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub wire: WireId,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// The result of planning.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub scheme: Scheme,
    pub mode: Mode,
    /// Every group in reading order, changed or not.
    pub groups: Vec<GroupPlan>,
    pub changes: Vec<Change>,
    /// What is wrong after the plan is applied.
    pub conflicts: Vec<Conflict>,
    revision: u64,
}

impl Plan {
    /// Nothing would change.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Groups whose number changes.
    pub fn changed_groups(&self) -> impl Iterator<Item = &GroupPlan> {
        self.groups
            .iter()
            .filter(|g| !matches!(g.reason, Reason::Kept | Reason::Locked))
    }

    /// One line: "12 wires numbered, 3 settled, 40 unchanged".
    pub fn summary(&self) -> String {
        let count = |r: Reason| self.groups.iter().filter(|g| g.reason == r).count();
        let (n, h, r) = (
            count(Reason::New),
            count(Reason::Healed),
            count(Reason::Replaced) + count(Reason::Renumbered),
        );
        let kept = count(Reason::Kept) + count(Reason::Locked);
        let mut parts = Vec::new();
        if n > 0 {
            parts.push(format!("{n} numbered"));
        }
        if r > 0 {
            parts.push(format!("{r} renumbered"));
        }
        if h > 0 {
            parts.push(format!("{h} settled"));
        }
        let blocked = count(Reason::Blocked);
        if blocked > 0 {
            parts.push(format!("{blocked} blocked"));
        }
        if parts.is_empty() {
            return format!("Nothing to change, {kept} unchanged");
        }
        format!("{}, {kept} unchanged", parts.join(", "))
    }

    /// Writes the numbers as the caller's undo step. Refused when the
    /// project changed since the plan was made.
    pub fn apply(&self, tx: &mut Tx<'_>) -> Result<usize> {
        if tx.project().revision() != self.revision {
            return Err(ModelError::Refused {
                what: "Wire numbers".into(),
                reason: "the drawing changed since the preview".into(),
            });
        }
        for c in &self.changes {
            tx.update::<Wire>(c.wire, |w| w.number = c.new.clone())?;
        }
        Ok(self.changes.len())
    }
}

/// Counter state of one series (one stem).
#[derive(Default)]
struct Series {
    used: BTreeSet<u32>,
}

struct State<'a> {
    scheme: &'a Scheme,
    /// Reserved numbers, with the group that holds each.
    taken: HashMap<String, usize>,
    series: BTreeMap<String, Series>,
}

impl State<'_> {
    fn reserve(&mut self, group: usize, number: &str, target: &Target) {
        self.taken.insert(number.to_owned(), group);
        if let Target::Counter { stem } = target
            && let Some(n) = count_after(number, stem)
        {
            self.series.entry(stem.clone()).or_default().used.insert(n);
        }
    }

    fn free(&self, number: &str) -> bool {
        !self.taken.contains_key(number)
    }

    /// The next number of a series, reserved.
    fn allocate(&mut self, group: usize, target: &Target, lowest: bool) -> Option<String> {
        match target {
            Target::Fixed { text, shared } => {
                if *shared || self.free(text) {
                    self.taken.insert(text.clone(), group);
                    Some(text.clone())
                } else {
                    None
                }
            }
            Target::Counter { stem } => {
                let (start, step) = (self.scheme.start, self.scheme.step.max(1));
                let series = self.series.entry(stem.clone()).or_default();
                let mut n = if lowest || self.scheme.reuse_gaps {
                    start
                } else {
                    series
                        .used
                        .last()
                        .map_or(start, |m| m.saturating_add(step).max(start))
                };
                loop {
                    let text = format!("{stem}{}", self.scheme.count_text(n));
                    if !series.used.contains(&n) && !self.taken.contains_key(&text) {
                        series.used.insert(n);
                        self.taken.insert(text.clone(), group);
                        return Some(text);
                    }
                    n = n.checked_add(step)?;
                }
            }
        }
    }
}

/// Plans numbers for a project. Does not change it.
pub fn plan(project: &Project, scheme: &Scheme, mode: Mode, scope: &Scope) -> Plan {
    let netlist = project.netlist();
    let all = groups(project, &netlist);
    let in_scope: Vec<bool> = all.iter().map(|g| selected(g, scope)).collect();
    let targets: Vec<Target> = all.iter().map(|g| g.target(scheme)).collect();
    let mut st = State {
        scheme,
        taken: HashMap::new(),
        series: BTreeMap::new(),
    };
    let mut result: Vec<Option<(Option<String>, Reason)>> = vec![None; all.len()];

    // Pass 1: whatever stays is reserved before anything is handed out;
    // locked and out-of-scope numbers first, so nothing in scope takes them.
    for firm in [true, false] {
        for (i, g) in all.iter().enumerate() {
            let Some(cur) = g.current.as_deref() else {
                continue;
            };
            let keep_always = !in_scope[i] || g.locked;
            if keep_always != firm {
                continue;
            }
            let keep = keep_always
                || match mode {
                    Mode::Renumber => false,
                    Mode::Fill => true,
                    Mode::FillStrict => fits(&targets[i], cur),
                };
            if !keep {
                continue;
            }
            if st.free(cur) || g.shares(cur, scheme) {
                st.reserve(i, cur, &targets[i]);
                let reason = if g.locked {
                    Reason::Locked
                } else if !in_scope[i] || (g.unnumbered == 0 && g.numbers.len() == 1) {
                    Reason::Kept
                } else {
                    Reason::Healed
                };
                result[i] = Some((Some(cur.to_owned()), reason));
            }
            // A taken number leaves the group undecided: it is replaced
            // below (in scope) or reported as a conflict.
        }
    }

    // Pass 2: everything undecided in reading order.
    let lowest = mode == Mode::Renumber;
    for (i, g) in all.iter().enumerate() {
        if result[i].is_some() {
            continue;
        }
        if !in_scope[i] || g.locked {
            result[i] = Some((g.current.clone(), Reason::Kept));
            continue;
        }
        let reason = match (&g.current, mode) {
            (_, Mode::Renumber) if g.current.is_some() => Reason::Renumbered,
            (Some(_), _) => Reason::Replaced,
            _ => Reason::New,
        };
        result[i] = Some(match st.allocate(i, &targets[i], lowest) {
            Some(n) => (Some(n), reason),
            None => (g.current.clone(), Reason::Blocked),
        });
    }

    let mut plan_groups = Vec::new();
    let mut changes = Vec::new();
    let mut after: BTreeMap<WireId, Option<String>> = BTreeMap::new();
    for (g, r) in all.iter().zip(result) {
        let (new, reason) = r.unwrap_or((None, Reason::Kept));
        for &id in &g.wires {
            let old = project.wire(id).and_then(|w| w.number.clone());
            let old = old.filter(|n| !n.is_empty());
            let changed = new.is_some() && old != new && reason != Reason::Blocked;
            if changed {
                changes.push(Change {
                    wire: id,
                    old: old.clone(),
                    new: new.clone(),
                });
            }
            after.insert(id, if changed { new.clone() } else { old });
        }
        plan_groups.push(GroupPlan {
            wires: g.wires.clone(),
            at: format!("{}.{}", g.page_name, g.column),
            potential: g.potential.as_ref().map(|p| p.0.clone()),
            old: g.numbers.iter().map(|n| n.0.clone()).collect(),
            new,
            reason,
        });
    }
    let conflicts = conflicts_of(project, &all, scheme, Some(&after));
    Plan {
        scheme: scheme.clone(),
        mode,
        groups: plan_groups,
        changes,
        conflicts,
        revision: project.revision(),
    }
}

fn selected(g: &Group, scope: &Scope) -> bool {
    match scope {
        Scope::Project => true,
        Scope::Pages(pages) => pages.contains(&g.page),
        Scope::Wires(wires) => g.wires.iter().any(|w| wires.contains(w)),
    }
}

/// Plans and applies in one go: the project's scheme, `Fill`, whole project.
/// Returns the number of wires changed.
pub fn number_new_wires(tx: &mut Tx<'_>) -> Result<usize> {
    let scheme = Scheme::load(tx.project());
    plan(tx.project(), &scheme, Mode::Fill, &Scope::Project).apply(tx)
}
