//! The signal monitor's rows and their filter (lane C3): which signals of which controller
//! instances to list in the Controller card's monitor and binding table.
//!
//! With several instances in a project (section 2b of the plan) one list of every signal is too
//! long to read, so the person narrows it: by controller instance, by a text, to the signals that
//! are high right now, or to those that drive nothing yet. The filter is plain data, the rows
//! are worked out here from the [`ControllerSet`] and the [`BindingTable`], so the application
//! only draws them and the rules are tested without a window.

use crate::binding::{BindingTable, SignalKey};
use crate::profile::SignalKind;
use crate::set::ControllerSet;

/// Which instances a monitor lists.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Scope {
    /// The instance the card shows (the default, so a project with one controller reads as before).
    #[default]
    Selected,
    /// Every instance.
    All,
    /// One instance by id.
    Instance(String),
}

/// What the person asked to see. `Default` shows the selected instance's signals unfiltered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignalFilter {
    pub scope: Scope,
    /// Words that must all occur (case does not matter) in the instance label or id, the signal
    /// name or one of its targets.
    pub text: String,
    /// Only signals that are high (or non-zero when analog) now.
    pub only_active: bool,
    /// Only signals that drive no target.
    pub only_unbound: bool,
}

/// One line of the monitor.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorRow {
    pub key: SignalKey,
    /// The label of the signal's instance, for the line when several instances are listed.
    pub instance_label: String,
    pub kind: SignalKind,
    pub level: f64,
    /// What the signal drives, as the binding table writes the targets.
    pub targets: Vec<String>,
}

impl MonitorRow {
    pub fn is_active(&self) -> bool {
        self.level != 0.0
    }

    pub fn is_bound(&self) -> bool {
        !self.targets.is_empty()
    }
}

/// The rows to show, and how many there were before the text, active and unbound filters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Monitor {
    pub rows: Vec<MonitorRow>,
    /// Signals of the chosen instances, before the other filters.
    pub in_scope: usize,
}

impl SignalFilter {
    /// Nothing narrowed beyond the default scope.
    pub fn is_clear(&self) -> bool {
        self.text.trim().is_empty() && !self.only_active && !self.only_unbound
    }

    /// Whether the filter lists more than one instance's signals, so the lines need to say whose
    /// they are.
    pub fn lists_several(&self) -> bool {
        self.scope == Scope::All
    }

    /// Puts the scope back to the selected instance when the instance it named is gone.
    /// Returns whether it had to.
    pub fn heal(&mut self, set: &ControllerSet) -> bool {
        match &self.scope {
            Scope::Instance(id) if set.instance(id).is_none() => {
                self.scope = Scope::Selected;
                true
            }
            _ => false,
        }
    }

    /// Whether `row` passes the text, active and unbound filters (not the scope).
    pub fn passes(&self, row: &MonitorRow) -> bool {
        if self.only_active && !row.is_active() {
            return false;
        }
        if self.only_unbound && row.is_bound() {
            return false;
        }
        let text = self.text.trim();
        if text.is_empty() {
            return true;
        }
        let haystack = format!(
            "{} {} {} {}",
            row.instance_label,
            row.key.controller,
            row.key.signal,
            row.targets.join(" ")
        )
        .to_lowercase();
        text.split_whitespace()
            .all(|word| haystack.contains(&word.to_lowercase()))
    }
}

/// The monitor for `filter`. `selected` is the instance the card shows; it decides what
/// [`Scope::Selected`] means. Rows follow the instance order, then the order of each profile.
pub fn monitor(
    set: &ControllerSet,
    table: &BindingTable,
    selected: Option<&str>,
    filter: &SignalFilter,
) -> Monitor {
    let mut out = Monitor::default();
    for inst in set.instances() {
        let listed = match &filter.scope {
            Scope::All => true,
            Scope::Selected => selected == Some(inst.id.as_str()),
            Scope::Instance(id) => *id == inst.id,
        };
        if !listed {
            continue;
        }
        for s in inst.controller.signals() {
            let key = SignalKey::new(&inst.id, &s.name);
            let targets = table.targets_of(&key).map(|t| t.to_string()).collect();
            let row = MonitorRow {
                key,
                instance_label: inst.label.clone(),
                kind: s.kind,
                level: s.level,
                targets,
            };
            out.in_scope += 1;
            if filter.passes(&row) {
                out.rows.push(row);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Action;
    use crate::binding::Target;
    use crate::profile::Profile;

    const PROFILE: &str = r#"
[controller]
id = "radio"
name = "Radio"
kind = "radio"

[[control]]
id = "estop"
kind = "estop"
[[control]]
id = "start"
kind = "latch"
[[control]]
id = "hoist"
kind = "rocker2"
positions = ["off", "up1", "up2", "down1", "down2"]

[[signal]]
name = "UP_SLOW"
control = "hoist"
group = "up"
step = 1
ungated = true
[[signal]]
name = "DOWN_SLOW"
control = "hoist"
group = "down"
step = 1
ungated = true
"#;

    fn two() -> (ControllerSet, BindingTable) {
        let profile = Profile::parse(PROFILE).expect("profile parses");
        let mut set = ControllerSet::new();
        set.add_instance("crane1", "Crane 1", &profile)
            .expect("first");
        set.add_instance("crane2", "Crane 2", &profile)
            .expect("second");
        let mut table = BindingTable::new();
        table.bind(
            SignalKey::new("crane1", "UP_SLOW"),
            Target::Pin("K1:A1".into()),
        );
        (set, table)
    }

    fn names(m: &Monitor) -> Vec<String> {
        m.rows.iter().map(|r| r.key.to_string()).collect()
    }

    #[test]
    fn the_default_lists_the_selected_instance_only() {
        let (set, table) = two();
        let f = SignalFilter::default();
        let m = monitor(&set, &table, Some("crane2"), &f);
        assert_eq!(names(&m), vec!["crane2/UP_SLOW", "crane2/DOWN_SLOW"]);
        assert_eq!(m.in_scope, 2);
        // No instance selected, nothing listed.
        assert!(monitor(&set, &table, None, &f).rows.is_empty());
    }

    #[test]
    fn all_lists_every_instance_in_instance_order() {
        let (set, table) = two();
        let f = SignalFilter {
            scope: Scope::All,
            ..SignalFilter::default()
        };
        let m = monitor(&set, &table, Some("crane2"), &f);
        assert_eq!(m.rows.len(), 4);
        assert_eq!(m.rows[0].key.controller, "crane1");
        assert_eq!(m.rows[3].key.controller, "crane2");
        assert!(f.lists_several());
        assert_eq!(m.rows[0].instance_label, "Crane 1");
    }

    #[test]
    fn one_named_instance_ignores_the_selection() {
        let (set, table) = two();
        let f = SignalFilter {
            scope: Scope::Instance("crane1".into()),
            ..SignalFilter::default()
        };
        let m = monitor(&set, &table, Some("crane2"), &f);
        assert_eq!(names(&m), vec!["crane1/UP_SLOW", "crane1/DOWN_SLOW"]);
    }

    #[test]
    fn only_unbound_and_only_active() {
        let (mut set, table) = two();
        let all = SignalFilter {
            scope: Scope::All,
            ..SignalFilter::default()
        };
        let unbound = SignalFilter {
            only_unbound: true,
            ..all.clone()
        };
        let m = monitor(&set, &table, None, &unbound);
        assert_eq!(m.rows.len(), 3, "crane1/UP_SLOW is bound");
        assert_eq!(m.in_scope, 4, "the count before the other filters");
        assert!(m.rows.iter().all(|r| !r.is_bound()));

        // Nothing is high until the latch closes and the rocker moves.
        let active = SignalFilter {
            only_active: true,
            ..all
        };
        assert!(monitor(&set, &table, None, &active).rows.is_empty());
        set.act(
            "crane2",
            &Action::Press {
                control: "start".into(),
            },
        )
        .unwrap();
        set.act(
            "crane2",
            &Action::SetPosition {
                control: "hoist".into(),
                position: "up1".into(),
            },
        )
        .unwrap();
        let m = monitor(&set, &table, None, &active);
        assert_eq!(names(&m), vec!["crane2/UP_SLOW"]);
        // Both at once: active and unbound.
        let both = SignalFilter {
            only_unbound: true,
            ..active
        };
        assert_eq!(
            names(&monitor(&set, &table, None, &both)),
            vec!["crane2/UP_SLOW"]
        );
    }

    #[test]
    fn text_matches_words_anywhere_case_blind() {
        let (set, table) = two();
        let all = SignalFilter {
            scope: Scope::All,
            ..SignalFilter::default()
        };
        let by = |text: &str| {
            let f = SignalFilter {
                text: text.into(),
                ..all.clone()
            };
            names(&monitor(&set, &table, None, &f))
        };
        assert_eq!(by("up_slow"), vec!["crane1/UP_SLOW", "crane2/UP_SLOW"]);
        assert_eq!(
            by("crane 2 DOWN"),
            vec!["crane2/DOWN_SLOW"],
            "all words, any order of fields"
        );
        assert_eq!(by("k1:a1"), vec!["crane1/UP_SLOW"], "a target");
        assert_eq!(by("  "), by(""), "blank is no filter");
        assert!(by("nothing like this").is_empty());
    }

    #[test]
    fn a_removed_instance_heals_to_the_selected_scope() {
        let (mut set, _) = two();
        let mut f = SignalFilter {
            scope: Scope::Instance("crane1".into()),
            ..SignalFilter::default()
        };
        assert!(!f.heal(&set));
        set.remove_instance("crane1").unwrap();
        assert!(f.heal(&set));
        assert_eq!(f.scope, Scope::Selected);
        assert!(!f.heal(&set));
        // An instance that never existed lists nothing before it is healed.
        let g = SignalFilter {
            scope: Scope::Instance("ghost".into()),
            ..SignalFilter::default()
        };
        assert!(
            monitor(&set, &BindingTable::new(), None, &g)
                .rows
                .is_empty()
        );
    }

    #[test]
    fn is_clear_ignores_the_scope() {
        let mut f = SignalFilter::default();
        assert!(f.is_clear());
        f.scope = Scope::All;
        assert!(f.is_clear());
        f.only_active = true;
        assert!(!f.is_clear());
    }
}
