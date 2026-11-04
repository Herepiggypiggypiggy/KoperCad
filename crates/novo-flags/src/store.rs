use std::collections::{BTreeMap, BTreeSet};

use crate::{Anchor, Flag, FlagDraft, FlagId, Severity, Source};

/// Where a flag is mostly seen. Taken from its first anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Area {
    Schematic,
    Cabinet,
    Jobs,
    App,
    Project,
}

impl Area {
    pub fn of(flag: &Flag) -> Area {
        match flag.anchors.first() {
            Some(
                Anchor::Symbol(_)
                | Anchor::Wire(_)
                | Anchor::Pin(..)
                | Anchor::Page(..)
                | Anchor::Terminal(_)
                | Anchor::Cable(_),
            ) => Area::Schematic,
            Some(
                Anchor::Device(_)
                | Anchor::Plate(..)
                | Anchor::Rail(_)
                | Anchor::Duct(_)
                | Anchor::Door(_)
                | Anchor::Opening(_)
                | Anchor::Enclosure(_)
                | Anchor::World(..),
            ) => Area::Cabinet,
            Some(Anchor::Job(_)) => Area::Jobs,
            Some(Anchor::Control(_)) => Area::App,
            Some(Anchor::Project) | None => Area::Project,
        }
    }
}

/// Worst severity and count over a set of anchors (a tab, a rail zone, a panel).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RollUp {
    pub red: usize,
    pub yellow: usize,
}

impl RollUp {
    pub fn worst(&self) -> Option<Severity> {
        if self.red > 0 {
            Some(Severity::Red)
        } else if self.yellow > 0 {
            Some(Severity::Yellow)
        } else {
            None
        }
    }

    pub fn total(&self) -> usize {
        self.red + self.yellow
    }
}

/// The one count: red and yellow, waived flags left out.
pub type Counts = RollUp;

/// All flags of all sources: one list, one count.
#[derive(Debug, Default, Clone)]
pub struct FlagStore {
    flags: BTreeMap<FlagId, Flag>,
    waived_ids: BTreeSet<FlagId>,
    waived_keys: BTreeSet<String>,
    fresh: BTreeSet<FlagId>,
}

impl FlagStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces every flag of the engine `source` belongs to (all `Issues(_)`
    /// kinds count as one engine) with `drafts`. Ids are stable, so waivers
    /// survive; ids that were not there before are reported by [`take_fresh`].
    /// A draft whose id repeats one earlier in the batch is dropped.
    ///
    /// [`take_fresh`]: FlagStore::take_fresh
    pub fn replace_source(&mut self, source: &Source, drafts: Vec<FlagDraft>) {
        let before: BTreeSet<FlagId> = self
            .flags
            .iter()
            .filter(|(_, f)| same_engine(&f.source, source))
            .map(|(id, _)| *id)
            .collect();
        for id in &before {
            self.flags.remove(id);
        }
        for draft in drafts {
            if !same_engine(&draft.source, source) {
                continue;
            }
            let flag = Flag::from_draft(draft);
            if self.flags.contains_key(&flag.id) {
                continue;
            }
            if !before.contains(&flag.id) {
                self.fresh.insert(flag.id);
            }
            self.flags.insert(flag.id, flag);
        }
        self.fresh.retain(|id| self.flags.contains_key(id));
    }

    /// Adds or replaces one flag (a job failure). Returns its id.
    pub fn set(&mut self, draft: FlagDraft) -> FlagId {
        let flag = Flag::from_draft(draft);
        let id = flag.id;
        if self.flags.insert(id, flag).is_none() {
            self.fresh.insert(id);
        }
        id
    }

    /// Removes one flag (the same job succeeded, the user dismissed it).
    pub fn clear(&mut self, id: FlagId) -> bool {
        self.fresh.remove(&id);
        self.flags.remove(&id).is_some()
    }

    /// Ids that are new since the last call: the app pulses them once.
    pub fn take_fresh(&mut self) -> Vec<FlagId> {
        std::mem::take(&mut self.fresh).into_iter().collect()
    }

    pub fn get(&self, id: FlagId) -> Option<&Flag> {
        self.flags.get(&id)
    }

    pub fn waive(&mut self, id: FlagId) {
        self.waived_ids.insert(id);
    }

    /// Waivers keyed like the Issues engine, so waivers in existing projects keep working.
    pub fn waive_key(&mut self, key: impl Into<String>) {
        self.waived_keys.insert(key.into());
    }

    pub fn unwaive(&mut self, id: FlagId) {
        self.waived_ids.remove(&id);
        if let Some(key) = self.flags.get(&id).and_then(|f| f.waiver_key.clone()) {
            self.waived_keys.remove(&key);
        }
    }

    pub fn is_waived(&self, flag: &Flag) -> bool {
        self.waived_ids.contains(&flag.id)
            || flag
                .waiver_key
                .as_ref()
                .is_some_and(|k| self.waived_keys.contains(k))
    }

    /// Flags in F8 order: red before yellow, then area, then source, title, id.
    /// Waived flags only when `show_waived`.
    pub fn list(&self, show_waived: bool) -> Vec<&Flag> {
        let mut v: Vec<&Flag> = self
            .flags
            .values()
            .filter(|f| show_waived || !self.is_waived(f))
            .collect();
        v.sort_by_key(|f| order_key(f));
        v
    }

    /// The one count (waived flags left out).
    pub fn counts(&self) -> Counts {
        self.roll(self.flags.values())
    }

    /// Flags with an anchor equal to `anchor`.
    pub fn at(&self, anchor: &Anchor, show_waived: bool) -> Vec<&Flag> {
        self.list(show_waived)
            .into_iter()
            .filter(|f| f.anchors.contains(anchor))
            .collect()
    }

    /// Worst severity and count over a set of anchors; a flag with several
    /// anchors in the set counts once.
    pub fn roll_up(&self, anchors: &[Anchor]) -> RollUp {
        self.roll(
            self.flags
                .values()
                .filter(|f| f.anchors.iter().any(|a| anchors.contains(a))),
        )
    }

    /// Same as [`roll_up`](FlagStore::roll_up) with a predicate on the anchor
    /// (all cabinet anchors for the Cabinet tab).
    pub fn roll_up_where(&self, pred: impl Fn(&Anchor) -> bool) -> RollUp {
        self.roll(self.flags.values().filter(|f| f.anchors.iter().any(&pred)))
    }

    /// The flag F8 goes to after `after` (`forward`) or Shift+F8 before it.
    /// Wraps around. When `after` is not in the list, starts at the first (or last).
    pub fn next(&self, after: Option<FlagId>, forward: bool) -> Option<FlagId> {
        let list = self.list(false);
        if list.is_empty() {
            return None;
        }
        let pos = after.and_then(|id| list.iter().position(|f| f.id == id));
        let n = list.len();
        let idx = match (pos, forward) {
            (Some(p), true) => (p + 1) % n,
            (Some(p), false) => (p + n - 1) % n,
            (None, true) => 0,
            (None, false) => n - 1,
        };
        Some(list[idx].id)
    }

    /// Visible flags grouped by area, in F8 order inside each group.
    pub fn by_area(&self) -> BTreeMap<Area, Vec<&Flag>> {
        let mut map: BTreeMap<Area, Vec<&Flag>> = BTreeMap::new();
        for f in self.list(false) {
            map.entry(Area::of(f)).or_default().push(f);
        }
        map
    }

    fn roll<'a>(&self, flags: impl Iterator<Item = &'a Flag>) -> RollUp {
        let mut r = RollUp::default();
        for f in flags.filter(|f| !self.is_waived(f)) {
            match f.severity {
                Severity::Red => r.red += 1,
                Severity::Yellow => r.yellow += 1,
            }
        }
        r
    }
}

fn same_engine(a: &Source, b: &Source) -> bool {
    match (a, b) {
        (Source::Issues(_), Source::Issues(_)) => true,
        _ => a == b,
    }
}

fn order_key(f: &Flag) -> (u8, Area, String, String, FlagId) {
    (
        f.severity.rank(),
        Area::of(f),
        f.source.key(),
        f.title.clone(),
        f.id,
    )
}
