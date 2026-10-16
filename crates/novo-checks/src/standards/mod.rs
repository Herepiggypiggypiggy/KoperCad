//! Standards compliance: IEC 60204-1, NFPA 79 and UL 508A as rule sets inside
//! the design checks.
//!
//! A project selects the standards it is built to ([`select`]); every rule of
//! a selected set then runs with the other design checks. A rule that is not
//! met is a [`Problem`] of kind [`ProblemKind::Standard`] carrying the rule's
//! id, so the Issues list, F8, the PDF and waivers handle it like any other
//! problem. A waiver is stored per rule (`check.std.<rule id>`, see
//! [`waiver_key`]) and prints with its reason.
//!
//! Next to the problems, [`compliance`] returns a [`ComplianceReport`]: for
//! every rule of every selected set, whether it passed, failed, passed only
//! for what could be evaluated, could not be evaluated for lack of data, or
//! does not apply. Unknown input is reported as unknown: a rule never passes
//! because a value was missing.
//!
//! # What this is, and is not
//!
//! The checks assist; the engineer certifies. They read the drawing and the
//! properties on devices and parts and compare them with requirements stated
//! in our own words ([`Rule::requirement`]) and with a few limits and table
//! values ([`Rule::verify`]). They do not reproduce the text of any standard.
//! Clause numbers are pointers into the edition named by
//! [`StandardSet::edition`]; the engineer reads the clause in their own
//! licensed copy and confirms the values marked for verification. Every such
//! value can be overridden per project (`standards.<set>.<name>`).
//!
//! # Where the data comes from
//!
//! Standards select and tune through project properties (the keys are in
//! [`keys`]); the facts of the machine come from properties on devices and
//! their parts (a device's own property wins over its part's): `function`,
//! `actuator_colour`, `stop_category`, `flc_a`, `sccr_ka`, `lockable`, ...
//! Each rule's [`Rule::inputs`] says what it reads, and the report names what
//! is missing when a rule cannot be evaluated.

mod ampacity;
pub mod document;
mod iec60204;
mod netclass;
mod nfpa79;
pub mod provenance;
mod report;
pub mod rules;
mod sccr;
mod shared;
mod symbols;
mod ul508a;

use std::collections::HashMap;

use novo_core::{
    AnyId, DeviceId, DismissalId, Netlist, PageId, Project, Properties, Result, Tx, Value,
};

use crate::{Context, Problem, ProblemKind, Severity};

pub use ampacity::{Awg, awg_for_area};
pub use report::{NOTICE, lines, to_csv, to_markdown};
pub use rules::{RULES, Rule, rule, rules_of};
pub use sccr::{SccrComponent, SccrReport, SccrSource};
pub use symbols::Colour;

/// The standards a project can be checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StandardSet {
    /// IEC 60204-1: safety of machinery, electrical equipment of machines.
    Iec60204,
    /// NFPA 79: electrical standard for industrial machinery.
    Nfpa79,
    /// UL 508A: industrial control panels, with the short-circuit current
    /// rating method of its Supplement SB.
    Ul508a,
}

impl StandardSet {
    pub const ALL: [StandardSet; 3] = [
        StandardSet::Iec60204,
        StandardSet::Nfpa79,
        StandardSet::Ul508a,
    ];

    /// The id used in project properties and rule ids.
    pub fn id(self) -> &'static str {
        match self {
            StandardSet::Iec60204 => "iec60204",
            StandardSet::Nfpa79 => "nfpa79",
            StandardSet::Ul508a => "ul508a",
        }
    }

    pub fn parse(id: &str) -> Option<StandardSet> {
        StandardSet::ALL.into_iter().find(|s| s.id() == id.trim())
    }

    /// A short name for chips and headings.
    pub fn short(self) -> &'static str {
        match self {
            StandardSet::Iec60204 => "IEC 60204-1",
            StandardSet::Nfpa79 => "NFPA 79",
            StandardSet::Ul508a => "UL 508A",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            StandardSet::Iec60204 => "Safety of machinery: electrical equipment of machines",
            StandardSet::Nfpa79 => "Electrical standard for industrial machinery",
            StandardSet::Ul508a => "Industrial control panels",
        }
    }

    /// The edition the clause pointers refer to.
    pub fn edition(self) -> &'static str {
        match self {
            StandardSet::Iec60204 => "IEC 60204-1:2016 with Amendment 1:2021",
            StandardSet::Nfpa79 => "NFPA 79, 2021 edition",
            StandardSet::Ul508a => "UL 508A, third edition (2018)",
        }
    }

    /// Whether the standard is a paid document the engineer needs their own
    /// licensed copy of to check the clauses and values this tool points at.
    pub fn needs_licensed_copy(self) -> bool {
        true
    }
}

/// Property keys the standards read, on the project (`ProjectInfo`
/// properties) unless noted.
pub mod keys {
    /// Comma-separated set ids the project is checked against.
    pub const SETS: &str = "standards.sets";
    /// Available fault current at the supply, kA (RMS, symmetrical).
    pub const SUPPLY_SHORT_CIRCUIT_KA: &str = "supply.short_circuit_ka";
    /// Short-circuit current rating marked on the panel, kA.
    pub const PANEL_SCCR_KA: &str = "panel.sccr_ka";
    /// `false` when the machine needs no emergency stop (risk assessment).
    pub const ESTOP_REQUIRED: &str = "machine.estop_required";
    pub const RATED_VOLTAGE_V: &str = "machine.rated_voltage_v";
    pub const PHASES: &str = "machine.phases";
    pub const FREQUENCY_HZ: &str = "machine.frequency_hz";
    pub const FULL_LOAD_CURRENT_A: &str = "machine.full_load_current_a";
    /// Prefix of per-standard overrides: `standards.<set>.<name>`.
    pub const OVERRIDE_PREFIX: &str = "standards.";
    /// Device or part: short-circuit current rating, kA.
    pub const SCCR_KA: &str = "sccr_ka";
    /// Device or part: interrupting rating of a protective device, kA.
    pub const BREAKING_CAPACITY_KA: &str = "breaking_capacity_ka";
    /// Device or part: a protective device that limits let-through current.
    pub const CURRENT_LIMITING: &str = "current_limiting";
    /// Device or part: the let-through current of a current-limiting device
    /// at the available fault current, kA.
    pub const LET_THROUGH_KA: &str = "let_through_ka";
    /// Motor: full-load current, A.
    pub const FLC_A: &str = "flc_a";
}

/// The name a waiver of a standards rule is stored under.
pub fn waiver_key(rule_id: &str) -> String {
    format!("check.std.{rule_id}")
}

/// The standards the project is checked against, in table order. Empty for a
/// project that selected none, which then gets no standards problems at all.
pub fn selected(project: &Project) -> Vec<StandardSet> {
    let Some(Value::Text(list)) = project.info().properties.get(keys::SETS) else {
        return Vec::new();
    };
    let mut sets: Vec<StandardSet> = list.split(',').filter_map(StandardSet::parse).collect();
    sets.sort();
    sets.dedup();
    sets
}

/// Selects the standards the project is checked against (one undo step with
/// the caller's transaction). An empty list turns the standards off.
pub fn select(tx: &mut Tx<'_>, sets: &[StandardSet]) -> Result<()> {
    let mut sets = sets.to_vec();
    sets.sort();
    sets.dedup();
    let text = sets.iter().map(|s| s.id()).collect::<Vec<_>>().join(",");
    set_info_property(
        tx,
        keys::SETS,
        (!text.is_empty()).then_some(Value::Text(text)),
    )
}

/// Sets or clears a project property the standards read: the supply fault
/// current, the machine data, a limit override. Validates nothing: a value
/// of the wrong type reads as missing and the rule says so.
pub fn set_info_property(tx: &mut Tx<'_>, key: &str, value: Option<Value>) -> Result<()> {
    tx.set_info(|info| match value {
        Some(v) => {
            info.properties.insert(key.to_owned(), v);
        }
        None => {
            info.properties.remove(key);
        }
    })
}

/// A value read as a number: integers, floats and text that parses.
pub(crate) fn as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::Text(t) => t
            .trim()
            .trim_end_matches(|c: char| c.is_alphabetic() || c == ' ')
            .replace(',', ".")
            .parse()
            .ok(),
        Value::Bool(_) => None,
    }
}

/// A value read as a yes/no.
pub(crate) fn as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(b) => Some(*b),
        Value::Int(i) => Some(*i != 0),
        Value::Text(t) => match t.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "y" | "1" | "ja" | "x" => Some(true),
            "false" | "no" | "n" | "0" | "nee" | "nein" | "-" => Some(false),
            _ => None,
        },
        Value::Float(_) => None,
    }
}

/// A device property, or the first of its parts' that states it.
pub(crate) fn device_property<'a>(
    project: &'a Project,
    device: DeviceId,
    key: &str,
) -> Option<&'a Value> {
    let device = project.device(device)?;
    device.properties.get(key).or_else(|| {
        device
            .parts
            .iter()
            .filter_map(|p| project.part(*p))
            .find_map(|p| p.properties.get(key))
    })
}

/// Project properties with the per-standard overrides.
pub(crate) struct Params<'a> {
    props: &'a Properties,
}

impl<'a> Params<'a> {
    pub(crate) fn new(project: &'a Project) -> Params<'a> {
        Params {
            props: &project.info().properties,
        }
    }

    pub(crate) fn value(&self, key: &str) -> Option<&Value> {
        self.props.get(key)
    }

    pub(crate) fn number(&self, key: &str) -> Option<f64> {
        self.value(key).and_then(as_f64)
    }

    pub(crate) fn flag(&self, key: &str) -> Option<bool> {
        self.value(key).and_then(as_bool)
    }

    pub(crate) fn text(&self, key: &str) -> Option<&str> {
        match self.value(key)? {
            Value::Text(t) => Some(t.as_str()),
            _ => None,
        }
    }

    /// A limit of a standard: the project's override or the default.
    pub(crate) fn limit(&self, set: StandardSet, name: &str, default: f64) -> f64 {
        self.number(&format!("{}{}.{name}", keys::OVERRIDE_PREFIX, set.id()))
            .unwrap_or(default)
    }

    /// A colour list override (`standards.iec60204.colour.ac_power = "BK,BN"`).
    pub(crate) fn colours(&self, set: StandardSet, name: &str) -> Option<Vec<Colour>> {
        let text = self.text(&format!(
            "{}{}.colour.{name}",
            keys::OVERRIDE_PREFIX,
            set.id()
        ))?;
        let colours: Vec<Colour> = text.split(',').filter_map(Colour::parse).collect();
        (!colours.is_empty()).then_some(colours)
    }
}

/// How a rule came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleStatus {
    /// Everything the rule applies to was evaluated and met it.
    Pass,
    /// At least one open (not waived) finding.
    Fail,
    /// What could be evaluated passed, but some items lacked data.
    Partial,
    /// Nothing could be evaluated: data is missing. Not a pass.
    NotEvaluated,
    /// Nothing in the project falls under the rule.
    NotApplicable,
    /// Every finding is waived with a reason.
    Waived,
}

impl RuleStatus {
    pub fn label(self) -> &'static str {
        match self {
            RuleStatus::Pass => "Pass",
            RuleStatus::Fail => "Fail",
            RuleStatus::Partial => "Partial",
            RuleStatus::NotEvaluated => "Not evaluated",
            RuleStatus::NotApplicable => "Not applicable",
            RuleStatus::Waived => "Waived",
        }
    }
}

/// Something a rule could not judge, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Unknown {
    pub at: Vec<AnyId>,
    pub why: String,
}

/// A finding that was waived, with the reason.
#[derive(Debug, Clone, PartialEq)]
pub struct WaivedFinding {
    pub problem: Problem,
    pub reason: String,
    pub waiver: DismissalId,
}

/// The outcome of one rule.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleResult {
    pub rule: &'static Rule,
    pub status: RuleStatus,
    /// How many things the rule evaluated (passed, failed or waived).
    pub checked: usize,
    /// Findings that are not waived.
    pub open: Vec<Problem>,
    pub waived: Vec<WaivedFinding>,
    pub unknown: Vec<Unknown>,
    /// What passed, one short line per thing (a device tag, a net): the
    /// evidence behind a pass. May be shorter than `checked`.
    pub evidence: Vec<String>,
}

/// Every rule of the selected standards, evaluated.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplianceReport {
    pub project_name: String,
    pub sets: Vec<StandardSet>,
    pub results: Vec<RuleResult>,
    /// The short-circuit current rating calculation, when a set that needs
    /// it is selected.
    pub sccr: Option<SccrReport>,
}

/// Counts of rules by outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tally {
    pub pass: usize,
    pub fail: usize,
    pub partial: usize,
    pub not_evaluated: usize,
    pub not_applicable: usize,
    pub waived: usize,
}

impl ComplianceReport {
    /// Rule results of one standard.
    pub fn of(&self, set: StandardSet) -> impl Iterator<Item = &RuleResult> {
        self.results.iter().filter(move |r| r.rule.set == set)
    }

    pub fn tally(&self, set: Option<StandardSet>) -> Tally {
        let mut t = Tally::default();
        for r in self
            .results
            .iter()
            .filter(|r| set.is_none_or(|s| r.rule.set == s))
        {
            match r.status {
                RuleStatus::Pass => t.pass += 1,
                RuleStatus::Fail => t.fail += 1,
                RuleStatus::Partial => t.partial += 1,
                RuleStatus::NotEvaluated => t.not_evaluated += 1,
                RuleStatus::NotApplicable => t.not_applicable += 1,
                RuleStatus::Waived => t.waived += 1,
            }
        }
        t
    }

    /// Whether any rule has an open finding.
    pub fn has_open_findings(&self) -> bool {
        self.results.iter().any(|r| !r.open.is_empty())
    }

    /// Every standards problem, open or waived, as the design checks list
    /// them (waived ones are filtered by [`crate::check_open`]).
    pub(crate) fn all_problems(&self) -> impl Iterator<Item = &Problem> {
        self.results
            .iter()
            .flat_map(|r| r.open.iter().chain(r.waived.iter().map(|w| &w.problem)))
    }
}

/// What a rule collected while it ran.
#[derive(Default)]
struct Acc {
    checked: usize,
    fails: Vec<(Vec<AnyId>, String)>,
    unknown: Vec<Unknown>,
    evidence: Vec<String>,
}

/// What the rules read: the project, its circuits and the parameters. Shared
/// and immutable during an evaluation.
pub(crate) struct Env<'a> {
    pub(crate) project: &'a Project,
    pub(crate) cx: &'a Context<'a>,
    pub(crate) nets: netclass::Nets<'a>,
    pub(crate) params: Params<'a>,
    sets: Vec<StandardSet>,
}

impl Env<'_> {
    pub(crate) fn on(&self, set: StandardSet) -> bool {
        self.sets.contains(&set)
    }

    /// Where a finding about the project as a whole is marked: the first
    /// page, so it can be jumped to and waived. Empty for an empty project.
    pub(crate) fn anchor(&self) -> Vec<AnyId> {
        self.project
            .pages_in_order()
            .first()
            .map(|p| vec![AnyId::Page(p.id)])
            .unwrap_or_default()
    }

    /// The label of a device for messages.
    pub(crate) fn tag(&self, device: DeviceId) -> String {
        self.project
            .device(device)
            .map_or_else(|| "a device".to_owned(), |d| d.tag.to_string())
    }
}

/// What the rules found, collected per rule.
#[derive(Default)]
pub(crate) struct Sink {
    acc: HashMap<&'static str, Acc>,
    pub(crate) sccr: Option<SccrReport>,
}

impl Sink {
    /// One more thing evaluated and found fine.
    pub(crate) fn pass(&mut self, id: &'static str) {
        self.acc.entry(id).or_default().checked += 1;
    }

    /// One more thing evaluated and found fine, with what it was.
    pub(crate) fn pass_note(&mut self, id: &'static str, note: String) {
        let acc = self.acc.entry(id).or_default();
        acc.checked += 1;
        if !acc.evidence.contains(&note) {
            acc.evidence.push(note);
        }
    }

    /// One thing evaluated and found wanting.
    pub(crate) fn fail(&mut self, id: &'static str, at: Vec<AnyId>, message: String) {
        let acc = self.acc.entry(id).or_default();
        acc.checked += 1;
        acc.fails.push((at, message));
    }

    /// Something the rule could not judge. Several for one rule are fine.
    pub(crate) fn unknown(&mut self, id: &'static str, at: Vec<AnyId>, why: String) {
        self.acc
            .entry(id)
            .or_default()
            .unknown
            .push(Unknown { at, why });
    }
}

/// Evaluates the selected standards with a fresh netlist.
pub fn compliance(project: &Project) -> ComplianceReport {
    compliance_with(project, &project.netlist())
}

/// Evaluates the selected standards, reading connectivity from `netlist`,
/// which must be current for `project`.
pub fn compliance_with(project: &Project, netlist: &Netlist) -> ComplianceReport {
    let cx = Context::new(project);
    run(&cx, netlist)
}

/// The standards problems for `check_with`: open and waived alike.
pub(crate) fn problems(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    if selected(cx.project).is_empty() {
        return;
    }
    let report = run(cx, netlist);
    out.extend(report.all_problems().cloned());
}

fn run(cx: &Context<'_>, netlist: &Netlist) -> ComplianceReport {
    let project = cx.project;
    let sets = selected(project);
    let env = Env {
        project,
        cx,
        nets: netclass::Nets::new(project, netlist),
        params: Params::new(project),
        sets: sets.clone(),
    };
    let mut sink = Sink::default();
    if sets.contains(&StandardSet::Nfpa79) || sets.contains(&StandardSet::Ul508a) {
        sink.sccr = sccr::compute(&env);
    }
    if !sets.is_empty() {
        iec60204::run(&env, &mut sink);
        nfpa79::run(&env, &mut sink);
        ul508a::run(&env, &mut sink);
    }
    finish(&env, sink)
}

fn finish(env: &Env<'_>, mut sink: Sink) -> ComplianceReport {
    let project = env.project;
    let cx = env.cx;
    let mut results = Vec::new();
    for rule in RULES.iter().filter(|r| env.sets.contains(&r.set)) {
        let acc = sink.acc.remove(rule.id).unwrap_or_default();
        let key = waiver_key(rule.id);
        let mut open = Vec::new();
        let mut waived = Vec::new();
        for (mut at, message) in acc.fails {
            at.sort();
            at.dedup();
            let mut problem = Problem::new(ProblemKind::Standard, at.clone(), message);
            problem.severity = rule.severity;
            problem.rule = Some(rule.id);
            cx.locate(&mut problem);
            match project.dismissal(&key, &at) {
                Some(d) => {
                    let reason = match d.properties.get(crate::waiver::REASON) {
                        Some(Value::Text(t)) => t.clone(),
                        _ => String::new(),
                    };
                    waived.push(WaivedFinding {
                        problem,
                        reason,
                        waiver: d.id,
                    });
                }
                None => open.push(problem),
            }
        }
        open.sort_by(|a, b| a.message.cmp(&b.message));
        let status = if !open.is_empty() {
            RuleStatus::Fail
        } else if !waived.is_empty() {
            RuleStatus::Waived
        } else if acc.checked == 0 && acc.unknown.is_empty() {
            RuleStatus::NotApplicable
        } else if acc.checked == 0 {
            RuleStatus::NotEvaluated
        } else if !acc.unknown.is_empty() {
            RuleStatus::Partial
        } else {
            RuleStatus::Pass
        };
        results.push(RuleResult {
            rule,
            status,
            checked: acc.checked,
            open,
            waived,
            unknown: acc.unknown,
            evidence: acc.evidence,
        });
    }
    ComplianceReport {
        project_name: project.info().name.clone(),
        sets: env.sets.clone(),
        results,
        sccr: sink.sccr,
    }
}

/// The page a problem is on, by name, for report lines.
pub(crate) fn page_name(project: &Project, page: Option<PageId>) -> Option<String> {
    page.and_then(|id| project.page(id)).map(|p| p.name.clone())
}

#[allow(dead_code)]
pub(crate) fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}
