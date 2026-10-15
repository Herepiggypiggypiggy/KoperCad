//! The lesson lint (design §7): what CI says about every lesson before a
//! person meets it.
//!
//! The lint is static. It reads the lesson files and the sample projects
//! and compares them with what this build has (the registries). What a
//! person would see when the step opens is confirmed by the replay test in
//! the application, so a target that only appears after an earlier step is
//! a warning here, not an error.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::Project;

use crate::catalog::{Catalog, Entry, Files, ProjectRef};
use crate::check::{Check, Context, Event, Facts, page_matches, pins_named, tag_matches};
use crate::lesson::{At, Lesson, SolveOp, Status, Step, Target};
use crate::markup::{self, Span};

/// What this build offers, supplied by the application.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    /// Every command id the command palette knows.
    pub commands: BTreeSet<String>,
    /// Every anchor id the shell registers.
    pub anchors: BTreeSet<String>,
    /// Every signal name the shell raises (`wheel_opened`, ...).
    pub signals: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One thing the lint found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    /// The lesson id, or the file for a lesson that did not parse.
    pub lesson: String,
    pub step: Option<String>,
    pub message: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        match &self.step {
            Some(step) => write!(f, "{sev}: {} / {step}: {}", self.lesson, self.message),
            None => write!(f, "{sev}: {}: {}", self.lesson, self.message),
        }
    }
}

/// The lint's result.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// Steps that have no Dutch text yet, for the translation work. A count,
    /// never an error.
    pub missing_nl: usize,
}

impl Report {
    pub fn errors(&self) -> impl Iterator<Item = &Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
    }
}

/// Lints every lesson. `example` is the built-in motor starter, which
/// lessons with `project = "@example"` start from; without it those lessons
/// skip the checks that read the project.
pub fn lint(
    catalog: &Catalog,
    files: &Files,
    registry: &Registry,
    example: Option<&Project>,
) -> Report {
    let mut report = Report::default();
    let mut push = |severity, lesson: &str, step: Option<&str>, message: String| {
        report.findings.push(Finding {
            severity,
            lesson: lesson.to_owned(),
            step: step.map(str::to_owned),
            message,
        });
    };
    for (path, error) in &catalog.rejected {
        push(Severity::Error, path, None, error.to_string());
    }
    for note in &catalog.notes {
        push(Severity::Error, "catalog", None, note.clone());
    }
    let mut missing_nl = 0;
    let lesson_ids: BTreeSet<&str> = catalog
        .entries
        .iter()
        .map(|e| e.lesson.id.as_str())
        .collect();
    let mut found = Vec::new();
    for entry in &catalog.entries {
        lint_lesson(
            entry,
            catalog,
            files,
            registry,
            example,
            &lesson_ids,
            &mut found,
            &mut missing_nl,
        );
    }
    report.findings.extend(found);
    report.missing_nl = missing_nl;
    report
}

struct Sink<'a> {
    lesson: &'a Lesson,
    out: &'a mut Vec<Finding>,
}

impl Sink<'_> {
    /// An error in a ready lesson; a draft may be ahead of the build.
    fn ahead(&mut self, step: Option<&Step>, message: String) {
        let severity = if self.lesson.status == Status::Ready {
            Severity::Error
        } else {
            Severity::Warning
        };
        self.add(severity, step, message);
    }

    fn error(&mut self, step: Option<&Step>, message: String) {
        self.add(Severity::Error, step, message);
    }

    fn warn(&mut self, step: Option<&Step>, message: String) {
        self.add(Severity::Warning, step, message);
    }

    fn add(&mut self, severity: Severity, step: Option<&Step>, message: String) {
        self.out.push(Finding {
            severity,
            lesson: self.lesson.id.clone(),
            step: step.map(|s| s.id.clone()),
            message,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn lint_lesson(
    entry: &Entry,
    catalog: &Catalog,
    files: &Files,
    registry: &Registry,
    example: Option<&Project>,
    lesson_ids: &BTreeSet<&str>,
    out: &mut Vec<Finding>,
    missing_nl: &mut usize,
) {
    let lesson = &*entry.lesson;
    let mut sink = Sink { lesson, out };
    if !catalog.tracks.iter().any(|t| t.id == lesson.track) {
        sink.error(
            None,
            format!("track `{}` is not in tracks.toml", lesson.track),
        );
    }
    for (what, ids) in [("needs", &lesson.needs), ("next", &lesson.next)] {
        for id in ids {
            if !lesson_ids.contains(id.as_str()) {
                sink.error(None, format!("{what} names `{id}`, which is not a lesson"));
            }
        }
    }
    let project = match &entry.project {
        None => {
            sink.error(
                None,
                format!("project `{}` is not among the files", lesson.project),
            );
            None
        }
        Some(ProjectRef::Example) => example.cloned(),
        Some(ProjectRef::Blank) => Some(Project::new("Blank")),
        Some(ProjectRef::File(path)) => match Project::from_json(&files[path]) {
            Ok(p) => Some(p),
            Err(e) => {
                sink.error(None, format!("{path} does not load: {e}"));
                None
            }
        },
    };
    if let Some(finished) = &lesson.finished {
        match &entry.finished {
            None => sink.error(
                None,
                format!("finished `{finished}` is not among the files"),
            ),
            Some(path) => {
                if let Err(e) = Project::from_json(&files[path]) {
                    sink.error(None, format!("{path} does not load: {e}"));
                }
            }
        }
    }
    let project = project.as_ref();
    if let (Some(page), Some(project)) = (&lesson.page, project)
        && !has_page(project, page)
    {
        sink.warn(None, format!("page `{page}` is not in the sample project"));
    }
    let pages_ok = |name: &str| project.is_none_or(|p| has_page(p, name));

    let facts = Facts {
        tool: "select".into(),
        zoom: 1.0,
        ..Facts::default()
    };
    let netlist = project.map(Project::netlist);
    for (i, step) in lesson.steps.iter().enumerate() {
        if step.text_nl.is_none() || step.title_nl.is_none() {
            *missing_nl += 1;
        }
        let mut texts = vec![step.title.as_str(), step.text.as_str()];
        texts.extend(step.note.iter().map(|n| n.text.as_str()));
        for text in texts {
            let Ok(spans) = markup::parse_markup(text) else {
                continue;
            };
            lint_spans(&spans, catalog, registry, &mut sink, step);
        }
        let mut targets: Vec<&Target> = step.point.iter().collect();
        targets.extend(step.note.iter().map(|n| &n.target));
        for target in targets {
            if let Some(ui) = &target.ui {
                for id in ui.ids() {
                    if !registry.anchors.contains(id) {
                        sink.ahead(Some(step), format!("anchor `{id}` is not in this build"));
                    }
                }
            }
            if let (Some(at), Some(project)) = (&target.at, project) {
                lint_at(at, project, &pages_ok, &mut sink, step);
            }
            if let Some(area) = &target.area
                && !pages_ok(&area.page)
            {
                sink.warn(
                    Some(step),
                    format!("page `{}` is not in the sample project", area.page),
                );
            }
        }
        if let Some(check) = &step.expect {
            let mut commands = Vec::new();
            check.commands(&mut commands);
            for id in commands {
                if !registry.commands.contains(id) {
                    sink.ahead(Some(step), format!("command `{id}` is not in this build"));
                }
            }
            let mut signals = Vec::new();
            check.signals(&mut signals);
            for id in signals {
                if !registry.signals.contains(id) {
                    sink.ahead(Some(step), format!("signal `{id}` is not in this build"));
                }
            }
            let mut anchors = Vec::new();
            check.anchors(&mut anchors);
            for id in anchors {
                if !registry.anchors.contains(id) {
                    sink.ahead(Some(step), format!("anchor `{id}` is not in this build"));
                }
            }
            if let (Some(project), Some(netlist)) = (project, &netlist) {
                lint_check_targets(check, project, &pages_ok, &mut sink, step);
                let events = BTreeSet::<Event>::new();
                if !check.reads_events()
                    && check.eval(&Context {
                        project,
                        netlist,
                        facts: &facts,
                        events: &events,
                    })
                {
                    let message = "the check already holds when the lesson opens, so the step would tick at once".to_owned();
                    if i == 0 {
                        sink.error(Some(step), message);
                    } else {
                        sink.warn(Some(step), message);
                    }
                }
            }
        } else if step.pause {
            sink.warn(Some(step), "`pause` on a reading step does nothing".into());
        }
        for op in &step.solve {
            match op {
                SolveOp::Command(c) if !registry.commands.contains(&c.command) => {
                    sink.ahead(
                        Some(step),
                        format!("command `{}` is not in this build", c.command),
                    );
                }
                SolveOp::Wire(w) => {
                    for pin in &w.wire {
                        if project.is_some_and(|p| pins_named(p, pin).is_empty()) {
                            sink.warn(
                                Some(step),
                                format!("pin `{pin}` is not in the sample project"),
                            );
                        }
                    }
                }
                SolveOp::Select(s) => {
                    if project.is_some_and(|p| !p.devices().any(|d| tag_matches(&d.tag, &s.select)))
                    {
                        sink.warn(
                            Some(step),
                            format!("device `{}` is not in the sample project", s.select),
                        );
                    }
                }
                SolveOp::Page(p) if !pages_ok(&p.page) => {
                    sink.warn(
                        Some(step),
                        format!("page `{}` is not in the sample project", p.page),
                    );
                }
                _ => {}
            }
        }
    }
}

fn lint_spans(
    spans: &[Span],
    catalog: &Catalog,
    registry: &Registry,
    sink: &mut Sink<'_>,
    step: &Step,
) {
    for (kind, value) in markup::references(spans) {
        match kind {
            "key" if !registry.commands.contains(value) => {
                sink.ahead(
                    Some(step),
                    format!("{{key:{value}}}: command is not in this build"),
                );
            }
            "ui" if !registry.anchors.contains(value) => {
                sink.ahead(
                    Some(step),
                    format!("{{ui:{value}}}: anchor is not in this build"),
                );
            }
            "tip" if !catalog.glossary.contains_key(value) => {
                sink.error(
                    Some(step),
                    format!("{{tip:{value}}}: no such term in glossary.toml"),
                );
            }
            _ => {}
        }
    }
}

fn has_page(project: &Project, name: &str) -> bool {
    project.pages().any(|p| page_matches(project, p.id, name))
}

fn lint_at(
    at: &At,
    project: &Project,
    pages_ok: &dyn Fn(&str) -> bool,
    sink: &mut Sink<'_>,
    step: &Step,
) {
    if let Some(device) = &at.device {
        let exists = project.devices().any(|d| tag_matches(&d.tag, device));
        if !exists {
            sink.warn(
                Some(step),
                format!("device `{device}` is not in the sample project at the start"),
            );
        } else if let Some(pin) = &at.pin
            && pins_named(project, &format!("{device}:{pin}")).is_empty()
        {
            sink.warn(
                Some(step),
                format!("pin `{device}:{pin}` is not in the sample project"),
            );
        }
    }
    if let Some(page) = &at.page
        && !pages_ok(page)
    {
        sink.warn(
            Some(step),
            format!("page `{page}` is not in the sample project"),
        );
    }
    if let Some(wire) = &at.wire {
        for pin in wire {
            if pins_named(project, pin).is_empty() {
                sink.warn(
                    Some(step),
                    format!("pin `{pin}` is not in the sample project"),
                );
            }
        }
    }
}

/// Pins and pages a check names must exist at the start; a device or cable
/// the check asks the person to create is of course not there yet.
fn lint_check_targets(
    check: &Check,
    project: &Project,
    pages_ok: &dyn Fn(&str) -> bool,
    sink: &mut Sink<'_>,
    step: &Step,
) {
    match check {
        Check::Wire(w) => {
            for pin in [&w.from, &w.to] {
                if pins_named(project, pin).is_empty() {
                    sink.warn(
                        Some(step),
                        format!("pin `{pin}` is not in the sample project at the start"),
                    );
                }
            }
        }
        Check::Page(name) if !pages_ok(name) => {
            sink.warn(
                Some(step),
                format!("page `{name}` is not in the sample project"),
            );
        }
        Check::All(v) | Check::Any(v) => {
            v.iter()
                .for_each(|c| lint_check_targets(c, project, pages_ok, sink, step));
        }
        Check::Not(c) => lint_check_targets(c, project, pages_ok, sink, step),
        _ => {}
    }
}

/// A one-line summary per severity, for a test's failure message.
pub fn summary(report: &Report) -> String {
    let mut by: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &report.findings {
        *by.entry(match f.severity {
            Severity::Error => "errors",
            Severity::Warning => "warnings",
        })
        .or_default() += 1;
    }
    format!(
        "{} errors, {} warnings, {} steps without Dutch text",
        by.get("errors").unwrap_or(&0),
        by.get("warnings").unwrap_or(&0),
        report.missing_nl
    )
}

#[cfg(test)]
mod tests {
    use novo_core::Document;

    use super::*;

    fn registry() -> Registry {
        Registry {
            commands: ["tool.connect", "edit.duplicate", "wire.mode", "edit.escape"]
                .map(String::from)
                .into(),
            anchors: ["dock.connect", "dock.wire_mode"].map(String::from).into(),
            signals: ["wheel_opened", "proposal_next"].map(String::from).into(),
        }
    }

    fn example() -> Project {
        let mut doc = Document::new(Project::new("T"));
        novo_tests::motor_starter(&mut doc).unwrap();
        doc.project().clone()
    }

    fn lesson_toml(extra_top: &str, steps: &str) -> String {
        format!(
            "format = 1\nid = \"demo\"\nversion = 1\ntitle = \"Demo\"\nsummary = \"s\"\ntrack = \"draw\"\nproject = \"@example\"\n{extra_top}\n{steps}"
        )
    }

    fn run(toml: &str, example: &Project) -> Report {
        let mut files = Files::new();
        files.insert(
            "tracks.toml".into(),
            "[[track]]\nid = \"draw\"\ntitle = \"Drawing\"\n".into(),
        );
        files.insert(
            "glossary.toml".into(),
            "[term.pin]\ntext = \"A connection point.\"\n".into(),
        );
        files.insert("demo/lesson.toml".into(), toml.to_owned());
        let catalog = Catalog::from_files(&files);
        lint(&catalog, &files, &registry(), Some(example))
    }

    const GOOD_STEPS: &str = r#"
[[step]]
id = "tool"
title = "Tool"
text = "Press {key:tool.connect}, a {tip:pin} awaits. Or {ui:dock.connect}."
point = { ui = "dock.connect" }
expect = { command = "tool.connect" }
solve = [ { command = "tool.connect" } ]

[[step]]
id = "dup"
title = "Duplicate"
text = "Select **-K1**."
point = { at = { device = "-K1", pin = "A1" } }
expect = { count = { kind = "device", at_least = 99 } }
"#;

    #[test]
    fn a_good_lesson_is_clean() {
        let report = run(&lesson_toml("", GOOD_STEPS), &example());
        assert!(report.findings.is_empty(), "{:#?}", report.findings);
        assert_eq!(report.missing_nl, 2);
    }

    fn messages(report: &Report) -> Vec<String> {
        report.findings.iter().map(Finding::to_string).collect()
    }

    #[test]
    fn unknown_things_are_errors_in_a_ready_lesson_and_warnings_in_a_draft() {
        let steps = GOOD_STEPS
            .replace("{key:tool.connect}", "{key:tool.nope}")
            .replace("dock.connect\" }", "dock.gone\" }")
            .replace("{tip:pin}", "{tip:nothing}");
        let ready = run(&lesson_toml("", &steps), &example());
        let m = messages(&ready);
        assert!(
            m.iter()
                .any(|s| s.starts_with("error") && s.contains("tool.nope")),
            "{m:?}"
        );
        assert!(
            m.iter()
                .any(|s| s.starts_with("error") && s.contains("dock.gone")),
            "{m:?}"
        );
        assert!(
            m.iter()
                .any(|s| s.starts_with("error") && s.contains("nothing")),
            "{m:?}"
        );
        let draft = run(&lesson_toml("status = \"draft\"", &steps), &example());
        let m = messages(&draft);
        assert!(
            m.iter()
                .any(|s| s.starts_with("warning") && s.contains("tool.nope")),
            "{m:?}"
        );
        // A missing glossary term is a mistake whatever the status.
        assert!(
            m.iter()
                .any(|s| s.starts_with("error") && s.contains("nothing")),
            "{m:?}"
        );
    }

    #[test]
    fn a_first_step_that_already_holds_is_an_error() {
        let steps = r#"
[[step]]
id = "tool"
title = "Tool"
text = "Pick the select tool."
expect = { tool = "select" }
"#;
        let report = run(&lesson_toml("", steps), &example());
        assert_eq!(report.errors().count(), 1, "{:?}", messages(&report));
        // An event check never holds at the start.
        let steps = steps.replace("{ tool = \"select\" }", "{ command = \"edit.escape\" }");
        assert!(
            run(&lesson_toml("", &steps), &example())
                .findings
                .is_empty()
        );
    }

    #[test]
    fn a_state_check_the_sample_project_already_satisfies_is_caught() {
        let steps = r#"
[[step]]
id = "wire"
title = "Wire"
text = "Join them."
expect = { wire = { from = "-F1:2", to = "-K1:1" } }
"#;
        let report = run(&lesson_toml("", steps), &example());
        assert!(
            messages(&report)
                .iter()
                .any(|s| s.starts_with("error") && s.contains("tick at once")),
            "{:?}",
            messages(&report)
        );
    }

    #[test]
    fn targets_that_are_not_in_the_project_are_warnings() {
        let steps = GOOD_STEPS.replace("\"-K1\"", "\"-Q9\"");
        let report = run(&lesson_toml("page = \"Nowhere\"", &steps), &example());
        let m = messages(&report);
        assert_eq!(report.errors().count(), 0, "{m:?}");
        assert!(m.iter().any(|s| s.contains("-Q9")), "{m:?}");
        assert!(m.iter().any(|s| s.contains("Nowhere")), "{m:?}");
    }

    #[test]
    fn broken_references_between_lessons_and_files() {
        let report = run(
            &lesson_toml("needs = [\"ghost\"]\nnext = [\"demo\"]", GOOD_STEPS)
                .replace("track = \"draw\"", "track = \"nope\""),
            &example(),
        );
        let m = messages(&report);
        assert!(m.iter().any(|s| s.contains("tracks.toml")), "{m:?}");
        assert!(m.iter().any(|s| s.contains("ghost")), "{m:?}");
        assert!(!m.iter().any(|s| s.contains("`demo`")), "{m:?}");
        let report = run(
            &lesson_toml("", GOOD_STEPS).replace("@example", "gone.json"),
            &example(),
        );
        assert!(messages(&report).iter().any(|s| s.contains("gone.json")));
    }

    #[test]
    fn signals_and_finished_files_are_checked() {
        let steps = r#"
[[step]]
id = "wheel"
title = "Wheel"
text = "Hold Q."
expect = { signal = "wheel_opened" }
[[step]]
id = "tab"
title = "Tab"
text = "Press Tab."
expect = { signal = "proposal_nxt" }
"#;
        let report = run(
            &lesson_toml("finished = \"finished.json\"", steps),
            &example(),
        );
        let m = messages(&report);
        assert!(m.iter().any(|s| s.contains("proposal_nxt")), "{m:?}");
        assert!(!m.iter().any(|s| s.contains("wheel_opened")), "{m:?}");
        assert!(m.iter().any(|s| s.contains("finished.json")), "{m:?}");
    }

    #[test]
    fn a_rejected_lesson_is_reported_with_its_file() {
        let report = run("format = 1\nid = \"demo\"", &example());
        assert_eq!(report.errors().count(), 1);
        assert!(messages(&report)[0].contains("demo/lesson.toml"));
        assert!(summary(&report).contains("1 errors"));
    }
}
