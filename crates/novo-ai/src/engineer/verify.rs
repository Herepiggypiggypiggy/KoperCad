//! What the program itself says about a project: the design checks, the
//! standards report and a diff against where the change set started. These
//! are computed here, by the program, not by the model, so the card the
//! person reads is not Claude's opinion of its own work.

use novo_checks::Severity;
use novo_core::Project;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// One open design-check problem, reduced to what is compared and shown.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Line {
    pub severity: &'static str,
    pub kind: String,
    pub message: String,
    pub page: Option<String>,
}

fn severity_name(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// The open problems of the Issues list (waived ones excluded).
pub fn problems(project: &Project) -> Vec<Line> {
    let mut out: Vec<Line> = novo_reports::issues::open(project)
        .iter()
        .map(|p| Line {
            severity: severity_name(p.severity),
            kind: format!("{:?}", p.kind),
            message: friendly_message(&p.message),
            page: p.page.and_then(|g| project.page(g)).map(|g| g.name.clone()),
        })
        .collect();
    out.sort();
    out
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}

impl Tally {
    pub fn of(lines: &[Line]) -> Self {
        let mut t = Tally::default();
        for l in lines {
            match l.severity {
                "error" => t.errors += 1,
                "warning" => t.warnings += 1,
                _ => t.infos += 1,
            }
        }
        t
    }
    pub fn text(&self) -> String {
        let n =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        format!(
            "{}, {}, {}",
            n(self.errors, "error", "errors"),
            n(self.warnings, "warning", "warnings"),
            n(self.infos, "note", "notes")
        )
    }
}

/// Problems before and after, with what appeared and what went away.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckDelta {
    pub before: Tally,
    pub after: Tally,
    pub new: Vec<Line>,
    pub resolved: Vec<Line>,
}

pub fn delta(before: &[Line], after: &[Line]) -> CheckDelta {
    fn minus(a: &[Line], b: &[Line]) -> Vec<Line> {
        let mut left: BTreeMap<&Line, usize> = BTreeMap::new();
        for l in b {
            *left.entry(l).or_default() += 1;
        }
        a.iter()
            .filter(|l| match left.get_mut(l) {
                Some(n) if *n > 0 => {
                    *n -= 1;
                    false
                }
                _ => true,
            })
            .cloned()
            .collect()
    }
    CheckDelta {
        before: Tally::of(before),
        after: Tally::of(after),
        new: minus(after, before),
        resolved: minus(before, after),
    }
}

impl CheckDelta {
    pub fn is_unchanged(&self) -> bool {
        self.new.is_empty() && self.resolved.is_empty()
    }
}

fn line_json(l: &Line) -> Value {
    json!({"severity": l.severity, "kind": l.kind, "message": l.message, "page": l.page})
}

/// The `run_checks` answer: all open problems and what this draft changed.
pub fn checks_json(before: &[Line], now: &[Line]) -> Value {
    let d = delta(before, now);
    json!({
        "open_now": d.after.text(),
        "open_when_the_change_set_started": d.before.text(),
        "new_since_start": d.new.iter().map(line_json).collect::<Vec<_>>(),
        "resolved_since_start": d.resolved.iter().map(line_json).collect::<Vec<_>>(),
        "all_open": now.iter().take(80).map(line_json).collect::<Vec<_>>(),
        "note": "These are the program's design checks. Zero problems means the checks found \
                 nothing, not that the design is correct or compliant.",
    })
}

/// The standards report in short: per selected standard, the counts and
/// every rule that does not pass, with its clause pointer. Always carries
/// the program's own disclaimer.
pub fn standards_json(project: &Project) -> Value {
    use novo_checks::standards::{self, RuleStatus};
    let report = standards::compliance(project);
    if report.sets.is_empty() {
        return json!({
            "selected_standards": [],
            "note": "No standard is selected for this project (Check > Standards). Without \
                     one the program evaluates no standards rules. Say so; do not claim \
                     compliance with anything.",
        });
    }
    let sets: Vec<Value> = report
        .sets
        .iter()
        .map(|s| {
            let t = report.tally(Some(*s));
            json!({
                "standard": s.title(), "edition": s.edition(),
                "pass": t.pass, "fail": t.fail, "partial": t.partial,
                "not_evaluated": t.not_evaluated, "waived": t.waived,
                "not_applicable": t.not_applicable,
            })
        })
        .collect();
    let open: Vec<Value> = report
        .results
        .iter()
        .filter(|r| {
            matches!(
                r.status,
                RuleStatus::Fail | RuleStatus::Partial | RuleStatus::NotEvaluated
            )
        })
        .take(60)
        .map(|r| {
            json!({
                "rule": r.rule.id, "clause": r.rule.clause, "title": r.rule.title,
                "status": format!("{:?}", r.status),
                "reads": r.rule.inputs,
                "findings": r.open.iter().take(5).map(|p| p.message.clone()).collect::<Vec<_>>(),
                "verify_in_licensed_copy": r.rule.verify,
            })
        })
        .collect();
    json!({
        "selected_standards": sets,
        "rules_not_passing": open,
        "note": standards::NOTICE,
        "how_to_speak": "A PASS means the program's own short rule found nothing wrong. It is \
                         not certification. NOT EVALUATED means the program could not tell: \
                         say what data is missing. Never write that the project meets a \
                         standard; say which rules the program checked and which it did not.",
    })
}

/// A short, readable account of what differs between two projects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    pub pages_added: Vec<String>,
    pub devices_added: Vec<String>,
    pub devices_removed: Vec<String>,
    pub devices_renamed: Vec<(String, String)>,
    pub properties_changed: usize,
    pub parts_assigned: usize,
    pub symbols_added: usize,
    pub symbols_removed: usize,
    pub wires_added: usize,
    pub wires_removed: usize,
    pub wire_numbers_set: usize,
}

pub fn diff(before: &Project, after: &Project) -> Diff {
    let mut d = Diff::default();
    for p in after.pages() {
        if before.page(p.id).is_none() {
            d.pages_added.push(p.name.clone());
        }
    }
    for dev in after.devices() {
        match before.device(dev.id) {
            None => d.devices_added.push(dev.tag.to_string()),
            Some(old) => {
                if old.tag != dev.tag {
                    d.devices_renamed
                        .push((old.tag.to_string(), dev.tag.to_string()));
                }
                if old.properties != dev.properties {
                    d.properties_changed += 1;
                }
                if old.parts != dev.parts {
                    d.parts_assigned += 1;
                }
            }
        }
    }
    for dev in before.devices() {
        if after.device(dev.id).is_none() {
            d.devices_removed.push(dev.tag.to_string());
        }
    }
    d.devices_added.sort();
    d.symbols_added = after
        .symbols()
        .filter(|s| before.symbol(s.id).is_none())
        .count();
    d.symbols_removed = before
        .symbols()
        .filter(|s| after.symbol(s.id).is_none())
        .count();
    d.wires_added = after
        .wires()
        .filter(|w| before.wire(w.id).is_none())
        .count();
    d.wires_removed = before
        .wires()
        .filter(|w| after.wire(w.id).is_none())
        .count();
    d.wire_numbers_set = after
        .wires()
        .filter(|w| before.wire(w.id).is_some_and(|o| o.number != w.number))
        .count()
        + after
            .wires()
            .filter(|w| before.wire(w.id).is_none() && w.number.is_some())
            .count();
    d
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        *self == Diff::default()
    }

    /// Lines for the card: one fact each.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        let list = |label: &str, items: &[String]| {
            (!items.is_empty()).then(|| format!("{label}: {}", items.join(", ")))
        };
        out.extend(list("New pages", &self.pages_added));
        out.extend(list("New devices", &self.devices_added));
        out.extend(list("Removed devices", &self.devices_removed));
        if !self.devices_renamed.is_empty() {
            let r: Vec<String> = self
                .devices_renamed
                .iter()
                .map(|(a, b)| format!("{a} to {b}"))
                .collect();
            out.push(format!("Renamed: {}", r.join(", ")));
        }
        let count =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let both = |added: usize, removed: usize, one: &str, many: &str| {
            let mut parts = Vec::new();
            if added > 0 {
                parts.push(format!("{} added", count(added, one, many)));
            }
            if removed > 0 {
                parts.push(format!("{} removed", count(removed, one, many)));
            }
            parts.join(", ")
        };
        if self.symbols_added + self.symbols_removed > 0 {
            out.push(both(
                self.symbols_added,
                self.symbols_removed,
                "symbol",
                "symbols",
            ));
        }
        if self.wires_added + self.wires_removed > 0 {
            out.push(both(self.wires_added, self.wires_removed, "wire", "wires"));
        }
        if self.wire_numbers_set > 0 {
            out.push(format!(
                "{} set or changed",
                count(self.wire_numbers_set, "wire number", "wire numbers")
            ));
        }
        if self.properties_changed > 0 {
            out.push(format!(
                "Devices with changed properties: {}",
                self.properties_changed
            ));
        }
        if self.parts_assigned > 0 {
            out.push(format!(
                "Devices with changed parts: {}",
                self.parts_assigned
            ));
        }
        out
    }
}

/// A check message that starts with a library symbol id (a symbol without a device,
/// such as a PE terminal) names it in words instead.
fn friendly_message(message: &str) -> String {
    match message.split_once(": ") {
        Some((head, rest)) if head.contains('/') && !head.contains(' ') => {
            let name = super::resolve::friendly_symbol(head);
            let mut c = name.chars();
            let name = c
                .next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or(name);
            format!("{name}: {rest}")
        }
        _ => message.to_string(),
    }
}

#[cfg(test)]
mod wording_tests {
    use super::*;

    #[test]
    fn plurals_are_real_and_library_ids_are_named_in_words() {
        let t = Tally {
            errors: 0,
            warnings: 1,
            infos: 7,
        };
        assert_eq!(t.text(), "0 errors, 1 warning, 7 notes");
        assert_eq!(
            friendly_message("iec/terminals/terminal_pe: 1 not connected"),
            "Protective earth terminal: 1 not connected"
        );
        assert_eq!(
            friendly_message("-K1: A2 not connected"),
            "-K1: A2 not connected"
        );
    }
}
