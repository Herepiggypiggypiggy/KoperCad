//! The evaluation harness for the co-engineer: scenarios with prompts and
//! automatic pass/fail checks, run with a scripted model (CI) or the real
//! one (a person with an API key).
//!
//! A scenario is a conversation: one or more turns, each a prompt and, for
//! the scripted model, the replies to give. The real model ignores the
//! scripts and answers the prompt itself. After the last turn the change
//! set is applied to the project the way the app applies it, and the checks
//! in [`Expect`] run on the result, using the program's own design checks,
//! standards report and netlist.
//!
//! A scripted run proves the tools, the draft, the checks, the one-step
//! undo and the wording rules work. It says nothing about how well a model
//! designs: only a run with `Mode::Real` does, and every report says which
//! kind it was.

use crate::agent::{Conversation, Event};
use crate::api::testing::Script;
use crate::api::{Client, Transport, UreqTransport};
use crate::config::AiSettings;
use crate::consent::DataScope;
use crate::engineer::snapshot::snapshot_of;
use crate::engineer::verify;
use crate::engineer::{EngineerHost, Resources, Step, actions, commit};
use crate::key::ApiKey;
use novo_core::{Document, Project};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

/// One scripted model reply: some words and the tools it calls.
#[derive(Clone, Debug, Deserialize)]
pub struct Reply {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub calls: Vec<Call>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Call {
    pub tool: String,
    #[serde(default)]
    pub input: Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Turn {
    pub prompt: String,
    /// What the scripted model answers, in order. The last reply has no calls.
    #[serde(default)]
    pub script: Vec<Reply>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Setup {
    pub action: String,
    #[serde(default)]
    pub args: Value,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Expect {
    /// Tags that must exist afterwards.
    pub devices_include: Vec<String>,
    /// Tags that must not exist afterwards.
    pub devices_exclude: Vec<String>,
    pub min_devices: Option<usize>,
    pub min_wires: Option<usize>,
    pub min_pages: Option<usize>,
    /// Design-check errors allowed after the change set. Default 0.
    pub errors_max: Option<usize>,
    pub warnings_max: Option<usize>,
    /// Open pins ("not connected" warnings) allowed.
    pub open_pins_max: Option<usize>,
    /// Every wire carries a number.
    pub all_wires_numbered: bool,
    /// The change set must resolve at least this many of the problems that were open before.
    pub resolves_min: Option<usize>,
    /// `run_checks` was called after the last edit.
    pub verified: bool,
    /// The plan checklist was shown.
    pub plan_shown: bool,
    /// Index of the turn in which Claude must ask a question (and make no edits).
    pub asks_in_turn: Option<usize>,
    /// Properties that must hold: device, key, value (text).
    pub properties: Vec<PropertyIs>,
    /// Words the last answer must contain (case-insensitive).
    pub text_contains: Vec<String>,
    /// Words the last answer must not contain.
    pub text_excludes: Vec<String>,
    /// Most edit steps in the change set (a model that wanders fails).
    pub steps_max: Option<usize>,
    /// Standards rules failing allowed (needs a selected standard in setup).
    pub standards_fail_max: Option<usize>,
    /// No devices may be removed that were there before.
    pub keep_existing: bool,
    /// Wire-number conflicts allowed against the project's own scheme.
    pub numbering_conflicts_max: Option<usize>,
    /// Devices that must have a non-empty description.
    pub described: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PropertyIs {
    pub device: String,
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Scenario {
    pub id: String,
    /// create, edit, polish or complete.
    pub category: String,
    pub title: String,
    #[serde(default)]
    pub setup: Vec<Setup>,
    pub turns: Vec<Turn>,
    #[serde(default)]
    pub expect: Expect,
}

impl Scenario {
    pub fn parse(json: &str) -> Result<Scenario, String> {
        serde_json::from_str(json).map_err(|e| format!("scenario: {e}"))
    }

    /// The replies for the app's replay file (`KOPERCAD_AI_REPLAY`): a JSON
    /// list of Messages API replies, in order, over all turns.
    pub fn replay(&self) -> Vec<Value> {
        let mut out = Vec::new();
        let mut n = 0;
        for t in &self.turns {
            for r in &t.script {
                n += 1;
                out.push(reply_message(r, n));
            }
        }
        out
    }
}

fn reply_message(r: &Reply, n: usize) -> Value {
    let mut content = Vec::new();
    if !r.text.is_empty() {
        content.push(json!({"type": "text", "text": r.text}));
    }
    for (i, c) in r.calls.iter().enumerate() {
        content.push(json!({"type": "tool_use", "id": format!("toolu_{n}_{i}"),
                            "name": c.tool, "input": c.input}));
    }
    json!({
        "id": format!("msg_{n}"), "type": "message", "role": "assistant",
        "model": crate::models::DEFAULT_MODEL,
        "content": content,
        "stop_reason": if r.calls.is_empty() { "end_turn" } else { "tool_use" },
        "usage": {"input_tokens": 0, "output_tokens": 0},
    })
}

/// Which model answers.
pub enum Mode {
    /// The scripts in the scenario. No network, no key, no cost.
    Scripted,
    /// The real model through the person's own key.
    Real {
        key: ApiKey,
        model: String,
        effort: String,
        /// Stop a scenario once its estimated cost passes this (USD).
        budget_usd: f64,
    },
}

impl Mode {
    pub fn label(&self) -> String {
        match self {
            Mode::Scripted => "scripted model (no real model involved)".into(),
            Mode::Real { model, .. } => format!("real model {model}"),
        }
    }
    pub fn is_real(&self) -> bool {
        matches!(self, Mode::Real { .. })
    }
}

#[derive(Clone, Debug)]
pub struct CheckResult {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct RunReport {
    pub id: String,
    pub category: String,
    pub title: String,
    pub real: bool,
    pub checks: Vec<CheckResult>,
    pub cost_usd: f64,
    pub tool_calls: Vec<String>,
    pub steps: Vec<String>,
    pub final_text: String,
    pub error: Option<String>,
    /// Problems open after the change set, one line each.
    pub problems_after: Vec<String>,
    pub diff: Vec<String>,
    /// Every tool call with its answer, in order.
    pub log: Vec<crate::engineer::host::LogEntry>,
}

impl RunReport {
    pub fn passed(&self) -> bool {
        self.error.is_none() && self.checks.iter().all(|c| c.ok)
    }
}

/// Words a final answer may not use about compliance or correctness. The
/// program's checks find problems; they do not certify anything.
pub const OVERCLAIMS: &[&str] = &[
    "is compliant",
    "are compliant",
    "fully compliant",
    "complies with",
    "meets iec",
    "meets nfpa",
    "meets ul",
    "meets the requirements",
    "certified",
    "guaranteed",
    "guarantee",
    "100% correct",
    "no errors at all",
    "is safe to build",
];

/// Overclaiming phrases in `text`, ignoring ones that are negated right before ("not compliant",
/// "does not certify", "no claim ... compliant").
pub fn overclaims(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut found = Vec::new();
    for phrase in OVERCLAIMS {
        let mut from = 0;
        while let Some(i) = lower[from..].find(phrase) {
            let at = from + i;
            let mut start = at.saturating_sub(24);
            while !lower.is_char_boundary(start) {
                start -= 1;
            }
            let before = &lower[start..at];
            let negated = ["not ", "n't ", "never ", "no ", "cannot ", "can't ", "nor "]
                .iter()
                .any(|n| before.contains(n));
            if !negated {
                found.push((*phrase).to_string());
            }
            from = at + phrase.len();
        }
    }
    found.sort();
    found.dedup();
    found
}

fn push(checks: &mut Vec<CheckResult>, name: &str, ok: bool, detail: impl Into<String>) {
    checks.push(CheckResult {
        name: name.into(),
        ok,
        detail: detail.into(),
    });
}

/// Builds the starting project of a scenario.
pub fn base_document(setup: &[Setup], resources: &Resources) -> Result<Document, String> {
    let mut doc = Document::new(Project::new("Evaluation"));
    for (i, s) in setup.iter().enumerate() {
        let env = actions::Env {
            library: resources.library,
            macros: &resources.macros,
        };
        doc.transact("Setup", |tx| {
            actions::run(tx, &env, &s.action, &s.args).map_err(|why| {
                novo_core::ModelError::Refused {
                    what: format!("setup step {}", i + 1),
                    reason: why,
                }
            })
        })
        .map_err(|e| e.to_string())?;
    }
    // Setup is not history: the person opened this project.
    doc.clear_history();
    Ok(doc)
}

/// Runs one scenario.
pub fn run(scenario: &Scenario, mode: &Mode) -> RunReport {
    let mut report = RunReport {
        id: scenario.id.clone(),
        category: scenario.category.clone(),
        title: scenario.title.clone(),
        real: mode.is_real(),
        checks: Vec::new(),
        cost_usd: 0.0,
        tool_calls: Vec::new(),
        steps: Vec::new(),
        final_text: String::new(),
        error: None,
        problems_after: Vec::new(),
        diff: Vec::new(),
        log: Vec::new(),
    };
    if let Err(e) = run_inner(scenario, mode, &mut report) {
        report.error = Some(e);
    }
    report
}

fn run_inner(s: &Scenario, mode: &Mode, report: &mut RunReport) -> Result<(), String> {
    let resources = Resources::new(Vec::new());
    let mut doc = base_document(&s.setup, &resources)?;
    let base: Project = doc.project().clone();
    let problems_before = verify::problems(&base);

    let script = Script::default();
    let (transport, key, model_id, effort): (Box<dyn Transport>, ApiKey, String, String) =
        match mode {
            Mode::Scripted => {
                for r in s.replay() {
                    script.push(200, r);
                }
                (
                    Box::new(script.clone()),
                    ApiKey::parse("sk-ant-api03-scripted-run-000000").map_err(|e| e.to_string())?,
                    crate::models::DEFAULT_MODEL.to_string(),
                    "medium".into(),
                )
            }
            Mode::Real {
                key, model, effort, ..
            } => (
                Box::new(UreqTransport::new()),
                key.clone(),
                model.clone(),
                effort.clone(),
            ),
        };
    let settings = AiSettings {
        enabled: true,
        model: model_id,
        effort,
        monthly_cap_usd: 0.0,
        ..AiSettings::default()
    };
    let mut client = Client::new(transport, key);
    let mut conversation = Conversation::default();
    let cancel = AtomicBool::new(false);

    let mut carried: Vec<Step> = Vec::new();
    let mut last = None;
    let mut asked_in: Option<usize> = None;
    let mut plan_shown = false;
    let mut tool_errors = 0usize;
    for (n, turn) in s.turns.iter().enumerate() {
        let mut host = EngineerHost::new(
            doc.project().clone(),
            carried.clone(),
            DataScope::Project,
            Resources::new(Vec::new()),
            Box::new(|p| snapshot_of(p, &[], DataScope::Project)),
        );
        let result = conversation
            .run_turn(
                &mut client,
                &mut host,
                &settings,
                0.0,
                &turn.prompt,
                &mut |e| {
                    if let Event::Tool { name, .. } = &e {
                        report.tool_calls.push(name.clone());
                    }
                },
                &cancel,
            )
            .map_err(|e| format!("turn {}: {e}", n + 1))?;
        report.cost_usd += result.cost_usd;
        if let Mode::Real { budget_usd, .. } = mode
            && report.cost_usd > *budget_usd
        {
            return Err(format!(
                "stopped: estimated cost passed the budget of ${budget_usd:.2}"
            ));
        }
        report.log.extend(host.log().iter().cloned());
        let out = host.outcome();
        if out.question.is_some() && asked_in.is_none() {
            asked_in = Some(n);
        }
        plan_shown |= !out.plan.is_empty();
        tool_errors += tool_error_count(&conversation);
        carried = out.steps.clone();
        report.final_text = result.text.clone();
        last = Some((out, host.draft().clone()));
        if result.step_limit {
            push(
                &mut report.checks,
                "finished within the step limit",
                false,
                "the model hit the step limit",
            );
        }
    }
    let (out, draft) = last.ok_or("the scenario has no turns")?;

    // Applying works and is one undo step; undoing restores the project.
    let depth = doc.undo_depth();
    let applied = if out.steps.is_empty() {
        None
    } else {
        Some(
            commit(&mut doc, &resources, &out.steps, "Claude")
                .map_err(|e| format!("apply: {e}"))?,
        )
    };
    if applied.is_some() {
        push(
            &mut report.checks,
            "applies as one undo step",
            doc.undo_depth() == depth + 1,
            format!("undo depth {} -> {}", depth, doc.undo_depth()),
        );
        push(
            &mut report.checks,
            "applying gives the draft the model checked",
            same_shape(doc.project(), &draft),
            "devices, wires, symbols and pages match the draft",
        );
    }
    let after = doc.project().clone();
    report.steps = out.steps.iter().map(|s| s.summary.clone()).collect();
    report.diff = out.diff.lines();
    let problems_after = verify::problems(&after);
    report.problems_after = problems_after
        .iter()
        .map(|l| format!("{}: {}", l.severity, l.message))
        .collect();
    if applied.is_some() {
        doc.undo().map_err(|e| e.to_string())?;
        push(
            &mut report.checks,
            "undo restores the project",
            same_shape(doc.project(), &base),
            "devices, wires, symbols and pages match the start",
        );
    }

    check_expect(
        &s.expect,
        &after,
        &base,
        &problems_before,
        &problems_after,
        &out,
        asked_in,
        plan_shown,
        report,
    );
    push(
        &mut report.checks,
        "no unqualified compliance claims in the answer",
        overclaims(&report.final_text).is_empty(),
        format!("{:?}", overclaims(&report.final_text)),
    );
    if !mode.is_real() {
        push(
            &mut report.checks,
            "scripted tool calls all succeed",
            tool_errors == 0,
            format!("{tool_errors} tool errors"),
        );
    }
    Ok(())
}

fn tool_error_count(c: &Conversation) -> usize {
    c.messages
        .iter()
        .filter_map(|m| m["content"].as_array())
        .flatten()
        .filter(|b| b["type"] == "tool_result" && b["is_error"] == true)
        .count()
}

fn same_shape(a: &Project, b: &Project) -> bool {
    let tags = |p: &Project| {
        let mut t: Vec<String> = p.devices().map(|d| d.tag.to_string()).collect();
        t.sort();
        t
    };
    tags(a) == tags(b)
        && a.wires().count() == b.wires().count()
        && a.symbols().count() == b.symbols().count()
        && a.pages().count() == b.pages().count()
}

#[allow(clippy::too_many_arguments)]
fn check_expect(
    e: &Expect,
    after: &Project,
    base: &Project,
    before: &[verify::Line],
    now: &[verify::Line],
    out: &crate::engineer::ChangeSetOutcome,
    asked_in: Option<usize>,
    plan_shown: bool,
    r: &mut RunReport,
) {
    let tags: Vec<String> = after.devices().map(|d| d.tag.to_string()).collect();
    for t in &e.devices_include {
        push(
            &mut r.checks,
            &format!("device {t} exists"),
            tags.contains(t),
            format!("devices: {}", tags.join(" ")),
        );
    }
    for t in &e.devices_exclude {
        push(
            &mut r.checks,
            &format!("device {t} is gone"),
            !tags.contains(t),
            "",
        );
    }
    if let Some(n) = e.min_devices {
        push(
            &mut r.checks,
            &format!("at least {n} devices"),
            tags.len() >= n,
            format!("{}", tags.len()),
        );
    }
    if let Some(n) = e.min_wires {
        push(
            &mut r.checks,
            &format!("at least {n} wires"),
            after.wires().count() >= n,
            format!("{}", after.wires().count()),
        );
    }
    if let Some(n) = e.min_pages {
        push(
            &mut r.checks,
            &format!("at least {n} pages"),
            after.pages().count() >= n,
            format!("{}", after.pages().count()),
        );
    }
    let tally = verify::Tally::of(now);
    let errors_max = e.errors_max.unwrap_or(0);
    push(
        &mut r.checks,
        &format!("design-check errors <= {errors_max}"),
        tally.errors <= errors_max,
        tally.text(),
    );
    if let Some(n) = e.warnings_max {
        push(
            &mut r.checks,
            &format!("design-check warnings <= {n}"),
            tally.warnings <= n,
            tally.text(),
        );
    }
    if let Some(n) = e.open_pins_max {
        let open = now.iter().filter(|l| l.kind == "UnconnectedPin").count();
        push(
            &mut r.checks,
            &format!("open-pin problems <= {n}"),
            open <= n,
            format!("{open}"),
        );
    }
    if e.all_wires_numbered {
        let missing = after.wires().filter(|w| w.number.is_none()).count();
        push(
            &mut r.checks,
            "every wire is numbered",
            missing == 0,
            format!("{missing} without number"),
        );
    }
    if let Some(n) = e.resolves_min {
        let d = verify::delta(before, now);
        push(
            &mut r.checks,
            &format!("resolves >= {n} earlier problems"),
            d.resolved.len() >= n,
            format!("{}", d.resolved.len()),
        );
    }
    if e.verified {
        push(
            &mut r.checks,
            "ran the checks after its last edit",
            out.verified,
            "run_checks after the last propose_edit",
        );
    }
    if e.plan_shown {
        push(&mut r.checks, "showed a plan", plan_shown, "");
    }
    if let Some(turn) = e.asks_in_turn {
        push(
            &mut r.checks,
            &format!("asked a question in turn {}", turn + 1),
            asked_in == Some(turn),
            format!("{asked_in:?}"),
        );
    }
    for p in &e.properties {
        let ok = after
            .devices()
            .find(|d| d.tag.to_string() == p.device)
            .and_then(|d| d.properties.get(&p.key))
            .is_some_and(|v| {
                v.to_string().trim_matches('"') == p.value || format!("{v:?}").contains(&p.value)
            });
        push(
            &mut r.checks,
            &format!("{}.{} = {}", p.device, p.key, p.value),
            ok,
            "",
        );
    }
    let lower = r.final_text.to_lowercase();
    for w in &e.text_contains {
        push(
            &mut r.checks,
            &format!("answer mentions {w:?}"),
            lower.contains(&w.to_lowercase()),
            "",
        );
    }
    for w in &e.text_excludes {
        push(
            &mut r.checks,
            &format!("answer avoids {w:?}"),
            !lower.contains(&w.to_lowercase()),
            "",
        );
    }
    if let Some(n) = e.steps_max {
        push(
            &mut r.checks,
            &format!("at most {n} edit steps"),
            out.steps.len() <= n,
            format!("{}", out.steps.len()),
        );
    }
    if let Some(n) = e.standards_fail_max {
        let report = novo_checks::standards::compliance(after);
        let fails = report.tally(None).fail;
        push(
            &mut r.checks,
            &format!("standards rules failing <= {n}"),
            fails <= n,
            format!("{fails}"),
        );
    }
    if let Some(n) = e.numbering_conflicts_max {
        let c = novo_numbering::check(after, &novo_numbering::Scheme::load(after)).len();
        push(
            &mut r.checks,
            &format!("wire-number conflicts <= {n}"),
            c <= n,
            format!("{c}"),
        );
    }
    for t in &e.described {
        let ok = after
            .devices()
            .find(|d| d.tag.to_string() == *t)
            .is_some_and(|d| !d.description.trim().is_empty());
        push(&mut r.checks, &format!("{t} has a description"), ok, "");
    }
    if e.keep_existing {
        let gone: Vec<String> = base
            .devices()
            .filter(|d| after.device(d.id).is_none())
            .map(|d| d.tag.to_string())
            .collect();
        push(
            &mut r.checks,
            "no existing device was deleted",
            gone.is_empty(),
            gone.join(" "),
        );
    }
}

/// The scenarios that ship with the program.
pub fn builtin() -> Vec<Scenario> {
    SOURCES
        .iter()
        .map(|(name, text)| Scenario::parse(text).unwrap_or_else(|e| panic!("{name}: {e}")))
        .collect()
}

const SOURCES: &[(&str, &str)] = &[
    (
        "create-dol-starter",
        include_str!("../eval/create-dol-starter.json"),
    ),
    (
        "create-reversing-starter",
        include_str!("../eval/create-reversing-starter.json"),
    ),
    (
        "create-plc-conveyor",
        include_str!("../eval/create-plc-conveyor.json"),
    ),
    (
        "ask-before-building",
        include_str!("../eval/ask-before-building.json"),
    ),
    (
        "edit-add-run-lamp",
        include_str!("../eval/edit-add-run-lamp.json"),
    ),
    (
        "edit-change-motor",
        include_str!("../eval/edit-change-motor.json"),
    ),
    (
        "edit-fix-check-errors",
        include_str!("../eval/edit-fix-check-errors.json"),
    ),
    (
        "polish-descriptions",
        include_str!("../eval/polish-descriptions.json"),
    ),
    (
        "polish-wire-numbers",
        include_str!("../eval/polish-wire-numbers.json"),
    ),
    (
        "complete-terminals",
        include_str!("../eval/complete-terminals.json"),
    ),
    (
        "complete-order-readiness",
        include_str!("../eval/complete-order-readiness.json"),
    ),
    (
        "honest-no-standard",
        include_str!("../eval/honest-no-standard.json"),
    ),
    (
        "honest-standard-selected",
        include_str!("../eval/honest-standard-selected.json"),
    ),
    (
        "safe-project-text",
        include_str!("../eval/safe-project-text.json"),
    ),
];

/// A report as Markdown: one section per scenario, honest about the kind of run.
pub fn markdown(mode: &Mode, reports: &[RunReport]) -> String {
    let passed = reports.iter().filter(|r| r.passed()).count();
    let mut out = String::new();
    out.push_str("# Ask Claude evaluation\n\n");
    out.push_str(&format!("Model: **{}**.\n\n", mode.label()));
    if !mode.is_real() {
        out.push_str(
            "> This was a SCRIPTED run: the replies were written in advance. It proves the \
             tools, the draft copy, the program's own checks, one-step undo and the wording \
             rules work. It does not measure how well a real model designs. Run \
             `scripts/ask-claude-eval.sh` with your own key for that.\n\n",
        );
    }
    out.push_str(&format!(
        "**{passed} of {} scenarios passed.**",
        reports.len()
    ));
    let cost: f64 = reports.iter().map(|r| r.cost_usd).sum();
    if mode.is_real() {
        out.push_str(&format!(" Estimated cost ${cost:.3} (list prices).\n\n"));
    } else {
        out.push('\n');
        out.push('\n');
    }
    out.push_str("| Scenario | Kind | Result | Steps | Tool calls |\n|---|---|---|---|---|\n");
    for r in reports {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            r.id,
            r.category,
            if r.passed() { "pass" } else { "FAIL" },
            r.steps.len(),
            r.tool_calls.len()
        ));
    }
    for r in reports {
        out.push_str(&format!(
            "\n## {} ({})\n\n{}\n\n",
            r.id, r.category, r.title
        ));
        if let Some(e) = &r.error {
            out.push_str(&format!("**Error:** {e}\n\n"));
        }
        for c in &r.checks {
            out.push_str(&format!(
                "- {} {}{}\n",
                if c.ok { "pass" } else { "**FAIL**" },
                c.name,
                if c.ok || c.detail.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", c.detail)
                }
            ));
        }
        if !r.diff.is_empty() {
            out.push_str(&format!("\nWhat changed: {}\n", r.diff.join("; ")));
        }
        if !r.problems_after.is_empty() {
            out.push_str("\nOpen after the change set:\n");
            for p in r.problems_after.iter().take(12) {
                out.push_str(&format!("- {p}\n"));
            }
        }
        if !r.final_text.is_empty() {
            out.push_str(&format!(
                "\nClaude's last answer:\n\n> {}\n",
                r.final_text.replace('\n', "\n> ")
            ));
        }
    }
    out
}
