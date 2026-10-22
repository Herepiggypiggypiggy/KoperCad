//! The tool host of a co-engineer turn, and committing its change set.

use super::actions::{self, Env};
use super::verify::{self, CheckDelta, Diff, Line};
use crate::consent::DataScope;
use crate::tools::{EditAction, Proposal, Snapshot, SnapshotHost, ToolHost, ToolReply};
use novo_core::{ChangeSet, Document, ModelError, Project};
use novo_parts::MacroFile;
use novo_symbols::Library;
use serde_json::{Value, json};
use std::sync::OnceLock;

/// Most edits in one change set, so a confused loop cannot rewrite a project.
pub const MAX_STEPS: usize = 120;

/// One edit in a change set.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub action: String,
    pub args: Value,
    /// One line the person reads, written by the program.
    pub summary: String,
    /// What the step replaces, for the card only (never sent to the model).
    pub note: Option<String>,
}

/// An item of the plan Claude shows while it works.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanItem {
    pub title: String,
    /// `todo`, `doing`, `done` or `skipped`.
    pub status: String,
}

/// A question for the person; Claude stops until they answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    pub text: String,
    pub options: Vec<String>,
}

/// What the program can place: the symbol library and the macros.
pub struct Resources {
    pub library: &'static Library,
    pub macros: Vec<MacroFile>,
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| Library::builtin().expect("the built-in symbol library loads"))
}

/// The macros that ship with the program.
pub fn starter_macros() -> Vec<MacroFile> {
    static MACROS: OnceLock<Vec<MacroFile>> = OnceLock::new();
    MACROS
        .get_or_init(|| {
            novo_parts::starter_macros()
                .into_iter()
                .map(|(_, f)| f)
                .collect()
        })
        .clone()
}

impl Resources {
    /// The built-in symbols and the starter macros, plus `extra` macros
    /// (the person's own, read by the app).
    pub fn new(extra: Vec<MacroFile>) -> Self {
        let mut macros = starter_macros();
        for m in extra {
            if !macros.iter().any(|x| x.name.eq_ignore_ascii_case(&m.name)) {
                macros.push(m);
            }
        }
        Resources {
            library: library(),
            macros,
        }
    }

    pub fn env(&self) -> Env<'_> {
        Env {
            library: self.library,
            macros: &self.macros,
        }
    }
}

/// How a turn's change set ended up, for the card and for evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct ChangeSetOutcome {
    /// Every step, carried-over ones first.
    pub steps: Vec<Step>,
    pub diff: Diff,
    pub checks: CheckDelta,
    /// Carried-over steps that no longer fit the project and were dropped.
    pub dropped: Vec<String>,
    pub plan: Vec<PlanItem>,
    pub question: Option<Question>,
    /// Whether `run_checks` was called after the last edit.
    pub verified: bool,
    pub tool_calls: Vec<String>,
}

type SnapshotFn = Box<dyn Fn(&Project) -> Snapshot + Send>;

pub struct EngineerHost {
    reads: SnapshotHost,
    snapshot_of: SnapshotFn,
    scope: DataScope,
    resources: Resources,
    base: Project,
    draft: Project,
    before: Vec<Line>,
    steps: Vec<Step>,
    carried: usize,
    dropped: Vec<String>,
    plan: Vec<PlanItem>,
    question: Option<Question>,
    verified: bool,
    calls: Vec<String>,
    log: Vec<LogEntry>,
}

/// One tool call and what came back, for evaluation reports.
#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub tool: String,
    pub input: Value,
    pub reply: String,
    pub is_error: bool,
}

impl EngineerHost {
    /// `base` is the real project when the turn starts. `carried` are the
    /// steps of a change set the person has not decided yet: they are
    /// replayed first, so Claude builds on them. `snapshot_of` makes the
    /// data the read tools see from any copy of the project, cut to the
    /// agreed scope.
    pub fn new(
        base: Project,
        carried: Vec<Step>,
        scope: DataScope,
        resources: Resources,
        snapshot_of: SnapshotFn,
    ) -> Self {
        let before = verify::problems(&base);
        let mut host = EngineerHost {
            reads: SnapshotHost::new(
                snapshot_of(&base),
                scope,
                Vec::new(),
                Box::new(|_, _| Ok(String::new())),
            ),
            snapshot_of,
            scope,
            resources,
            draft: base.clone(),
            base,
            before,
            steps: Vec::new(),
            carried: 0,
            dropped: Vec::new(),
            plan: Vec::new(),
            question: None,
            verified: false,
            calls: Vec::new(),
            log: Vec::new(),
        };
        for step in carried {
            match host.apply(&step.action, &step.args) {
                Ok(_) => {}
                Err(why) => host.dropped.push(format!("{} ({why})", step.summary)),
            }
        }
        host.carried = host.steps.len();
        host.refresh();
        host
    }

    fn refresh(&mut self) {
        self.reads.set_snapshot((self.snapshot_of)(&self.draft));
    }

    /// Applies one action to the draft and records it as a step.
    pub fn apply(&mut self, action: &str, args: &Value) -> Result<String, String> {
        if self.steps.len() >= MAX_STEPS {
            return Err(format!(
                "this change set already has {MAX_STEPS} steps; stop and let the person apply it"
            ));
        }
        let note = actions::note_before(&self.draft, action, args);
        let summary = actions::run_on(&mut self.draft, &self.resources.env(), action, args)?;
        self.steps.push(Step {
            action: action.to_string(),
            args: args.clone(),
            summary: summary.clone(),
            note,
        });
        self.verified = false;
        Ok(summary)
    }

    /// Every tool call of the turn with its answer.
    pub fn log(&self) -> &[LogEntry] {
        &self.log
    }

    pub fn draft(&self) -> &Project {
        &self.draft
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// The change set as it stands.
    pub fn outcome(&self) -> ChangeSetOutcome {
        ChangeSetOutcome {
            steps: self.steps.clone(),
            diff: verify::diff(&self.base, &self.draft),
            checks: verify::delta(&self.before, &verify::problems(&self.draft)),
            dropped: self.dropped.clone(),
            plan: self.plan.clone(),
            question: self.question.clone(),
            verified: self.verified,
            tool_calls: self.calls.clone(),
        }
    }

    fn denied(&self, needed: DataScope) -> ToolReply {
        ToolReply {
            text: format!(
                "Not allowed: this needs the \"{}\" data scope and the person has allowed \"{}\". \
                 Tell them what you would need and why; do not guess.",
                needed.label(),
                self.scope.label()
            ),
            is_error: true,
        }
    }

    fn propose(&mut self, input: &Value) -> ToolReply {
        if !self.scope.allows(DataScope::Outline) {
            return self.denied(DataScope::Outline);
        }
        let action = input["action"].as_str().unwrap_or_default();
        // Part names and wire lists are above the outline: the answer to these
        // two goes back to the model, so they need the whole project.
        if matches!(action, "assign_part" | "set_wire_number")
            && !self.scope.allows(DataScope::Project)
        {
            return ToolReply {
                text: format!("{action} is not allowed above the agreed scope"),
                is_error: true,
            };
        }
        let args = if input["args"].is_null() {
            json!({})
        } else {
            input["args"].clone()
        };
        match self.apply(action, &args) {
            Err(why) => ToolReply {
                text: format!("Cannot do that: {why}. The draft is unchanged."),
                is_error: true,
            },
            Ok(summary) => {
                self.refresh();
                ToolReply {
                    text: format!(
                        "Done in the draft (step {}): {summary}. Nothing is applied to the real \
                         project: the person applies the whole change set. Call run_checks \
                         when you have finished a part, and fix what it reports.",
                        self.steps.len()
                    ),
                    is_error: false,
                }
            }
        }
    }

    fn run_checks(&mut self) -> ToolReply {
        if !self.scope.allows(DataScope::Project) {
            return self.denied(DataScope::Project);
        }
        self.verified = true;
        let now = verify::problems(&self.draft);
        ToolReply {
            text: super::super::tools::clip_for_tools(
                &verify::checks_json(&self.before, &now).to_string(),
            ),
            is_error: false,
        }
    }

    fn standards(&self) -> ToolReply {
        if !self.scope.allows(DataScope::Project) {
            return self.denied(DataScope::Project);
        }
        ToolReply {
            text: super::super::tools::clip_for_tools(
                &verify::standards_json(&self.draft).to_string(),
            ),
            is_error: false,
        }
    }

    fn read_report(&self, input: &Value) -> ToolReply {
        if !self.scope.allows(DataScope::Project) {
            return self.denied(DataScope::Project);
        }
        let reports = novo_reports::standard_reports();
        let Some(id) = input["id"].as_str() else {
            let ids: Vec<&str> = reports.iter().map(|r| r.id()).collect();
            return ToolReply {
                text: json!({"reports": ids, "tip": "call again with id"}).to_string(),
                is_error: false,
            };
        };
        let Some(report) = reports.iter().find(|r| r.id() == id) else {
            let ids: Vec<&str> = reports.iter().map(|r| r.id()).collect();
            return ToolReply {
                text: format!("No report {id:?}. Reports: {ids:?}"),
                is_error: true,
            };
        };
        let out = report.generate(&self.draft);
        let sections: Vec<Value> = out
            .sections
            .iter()
            .map(|s| {
                json!({
                    "title": s.title,
                    "columns": s.columns.iter().map(|c| c.header.as_str()).collect::<Vec<_>>(),
                    "rows": s.rows.iter().take(60)
                        .map(|r| r.cells.iter().map(|c| c.to_string()).collect::<Vec<_>>())
                        .collect::<Vec<_>>(),
                    "rows_total": s.rows.len(),
                })
            })
            .collect();
        ToolReply {
            text: super::super::tools::clip_for_tools(
                &json!({"id": out.report, "title": out.title, "sections": sections}).to_string(),
            ),
            is_error: false,
        }
    }

    fn list_macros(&self) -> ToolReply {
        let items: Vec<Value> = self
            .resources
            .macros
            .iter()
            .map(|m| {
                json!({
                    "name": m.name,
                    "what": m.description,
                    "parameters": m.definition.parameters.iter()
                        .map(|p| json!({"name": p.name, "default": format!("{:?}", p.default)}))
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        ToolReply {
            text: json!({"macros": items}).to_string(),
            is_error: false,
        }
    }

    fn list_symbols(&self, input: &Value) -> ToolReply {
        let q = input["query"].as_str().unwrap_or_default().to_lowercase();
        let limit = input["limit"]
            .as_u64()
            .map_or(25, |l| l as usize)
            .clamp(1, 60);
        // Exact alias or id first, then names and functions, then keywords.
        let rank = |s: &novo_symbols::Symbol| -> Option<u8> {
            if q.is_empty() {
                return Some(2);
            }
            if s.aliases.iter().any(|a| a.to_lowercase() == q) || s.id.to_lowercase() == q {
                return Some(0);
            }
            let hay = [s.id.as_str(), s.name.as_str(), s.function.as_str()];
            if hay.iter().any(|f| f.to_lowercase().contains(&q)) {
                return Some(1);
            }
            (s.keywords.iter().any(|k| k.to_lowercase() == q)).then_some(2)
        };
        let mut ranked: Vec<(u8, &novo_symbols::Symbol)> = self
            .resources
            .library
            .iter()
            .filter_map(|s| rank(s).map(|r| (r, s)))
            .collect();
        ranked.sort_by(|a, b| (a.0, &a.1.id).cmp(&(b.0, &b.1.id)));
        let hits: Vec<Value> = ranked
            .into_iter()
            .take(limit)
            .map(|(_, s)| {
                json!({
                    "id": s.id, "name": s.name, "function": s.function,
                    "aliases": s.aliases,
                    "pins": s.connections.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
                })
            })
            .collect();
        ToolReply {
            text: json!({"symbols": hits, "tip": "narrow with query; ids go to place_symbol"})
                .to_string(),
            is_error: false,
        }
    }

    fn set_plan(&mut self, input: &Value) -> ToolReply {
        let Some(items) = input["steps"].as_array() else {
            return ToolReply {
                text: "steps must be a list".into(),
                is_error: true,
            };
        };
        self.plan = items
            .iter()
            .take(30)
            .filter_map(|i| {
                Some(PlanItem {
                    title: i["title"].as_str()?.chars().take(160).collect(),
                    status: match i["status"].as_str() {
                        Some(s @ ("doing" | "done" | "skipped")) => s.to_string(),
                        _ => "todo".into(),
                    },
                })
            })
            .collect();
        ToolReply {
            text: "Plan shown to the person.".into(),
            is_error: false,
        }
    }

    fn ask(&mut self, input: &Value) -> ToolReply {
        let Some(text) = input["question"].as_str().filter(|t| !t.trim().is_empty()) else {
            return ToolReply {
                text: "question is missing".into(),
                is_error: true,
            };
        };
        let options: Vec<String> = input["options"]
            .as_array()
            .map(|o| {
                o.iter()
                    .filter_map(|x| x.as_str())
                    .take(5)
                    .map(|x| x.chars().take(80).collect())
                    .collect()
            })
            .unwrap_or_default();
        self.question = Some(Question {
            text: text.chars().take(400).collect(),
            options,
        });
        ToolReply {
            text: "The question is on the person's screen. Stop now: write one short sentence \
                   and end your turn. Do not continue until they answer."
                .into(),
            is_error: false,
        }
    }
}

impl ToolHost for EngineerHost {
    fn specs(&self) -> Vec<Value> {
        let mut specs = self.reads.specs();
        let names: Vec<&str> = actions::catalogue().iter().map(|a| a.name).collect();
        let catalogue: Vec<Value> = actions::catalogue()
            .iter()
            .map(
                |a: &EditAction| json!({"action": a.name, "what": a.description, "args": a.schema}),
            )
            .collect();
        specs.push(json!({
            "name": "propose_edit",
            "description": format!(
                "Makes one edit in your DRAFT copy of the project. The result (tags created, open \
                 pins) comes back at once and later tools see it. Nothing touches the real \
                 project until the person applies the whole change set. Actions: {}",
                Value::Array(catalogue)),
            "input_schema": {"type": "object", "properties": {
                "action": {"type": "string", "enum": names},
                "args": {"type": "object"}}, "required": ["action", "args"]},
        }));
        specs.push(json!({
            "name": "run_checks",
            "description": "Runs the program's design checks on your draft and reports the open \
                            problems, which are new since the start and which you resolved. Run \
                            it after building something and before you say you are done.",
            "input_schema": {"type": "object", "properties": {}, "additionalProperties": false},
        }));
        specs.push(json!({
            "name": "standards_report",
            "description": "The program's standards rules (IEC 60204-1, NFPA 79, UL 508A) on your \
                            draft, for the standards the project selected. Short statements \
                            with clause pointers; a pass is not certification.",
            "input_schema": {"type": "object", "properties": {}, "additionalProperties": false},
        }));
        specs.push(json!({
            "name": "read_report",
            "description": "Reads one of the program's generated reports on your draft (bill of \
                            materials, device list, wire list, contents). Without id it lists \
                            them. Use it to see what is still missing before an order or a \
                            build.",
            "input_schema": {"type": "object", "properties": {"id": {"type": "string"}}},
        }));
        specs.push(json!({
            "name": "list_macros",
            "description": "Ready-made circuits for place_macro, with their parameters.",
            "input_schema": {"type": "object", "properties": {}, "additionalProperties": false},
        }));
        specs.push(json!({
            "name": "list_symbols",
            "description": "Searches the symbol library by id, name, function or alias and \
                            returns pin names, for place_symbol and connect.",
            "input_schema": {"type": "object", "properties": {
                "query": {"type": "string"}, "limit": {"type": "integer"}}},
        }));
        specs.push(json!({
            "name": "set_plan",
            "description": "Shows the person your plan as a checklist and keeps it current. Call \
                            it first for any job of more than two edits, and again as steps \
                            finish. Each call replaces the whole list.",
            "input_schema": {"type": "object", "properties": {"steps": {"type": "array", "items": {
                "type": "object", "properties": {
                    "title": {"type": "string"},
                    "status": {"type": "string", "enum": ["todo", "doing", "done", "skipped"]}},
                "required": ["title", "status"]}}}, "required": ["steps"]},
        }));
        specs.push(json!({
            "name": "ask_user",
            "description": "Asks the person a question when the request is ambiguous in a way \
                            that changes the design (supply voltage, motor power, which page, \
                            which standard). Offer up to 5 short options. Then end your turn. \
                            Do not ask what you can read from the project.",
            "input_schema": {"type": "object", "properties": {
                "question": {"type": "string"},
                "options": {"type": "array", "items": {"type": "string"}}},
                "required": ["question"]},
        }));
        specs
    }

    fn call(&mut self, name: &str, input: &Value) -> ToolReply {
        self.calls.push(name.to_string());
        let reply = self.dispatch(name, input);
        self.log.push(LogEntry {
            tool: name.to_string(),
            input: input.clone(),
            reply: reply.text.clone(),
            is_error: reply.is_error,
        });
        reply
    }

    fn take_proposals(&mut self) -> Vec<Proposal> {
        self.steps
            .iter()
            .enumerate()
            .map(|(i, s)| Proposal {
                id: i as u32 + 1,
                action: s.action.clone(),
                args: s.args.clone(),
                summary: s.summary.clone(),
            })
            .collect()
    }
}

impl EngineerHost {
    fn dispatch(&mut self, name: &str, input: &Value) -> ToolReply {
        match name {
            "propose_edit" => self.propose(input),
            "run_checks" => self.run_checks(),
            "read_report" => self.read_report(input),
            "standards_report" => self.standards(),
            "list_macros" => self.list_macros(),
            "list_symbols" => self.list_symbols(input),
            "set_plan" => self.set_plan(input),
            "ask_user" => self.ask(input),
            other => self.reads.call(other, input),
        }
    }
}

/// Applies the steps to the real document as ONE undo step. All or nothing:
/// if any step no longer fits, nothing changes and the error names it.
pub fn commit(
    doc: &mut Document,
    resources: &Resources,
    steps: &[Step],
    label: &str,
) -> Result<ChangeSet, String> {
    let env = resources.env();
    doc.transact(label, |tx| {
        for (i, s) in steps.iter().enumerate() {
            actions::run(tx, &env, &s.action, &s.args).map_err(|why| ModelError::Refused {
                what: format!("Step {} ({})", i + 1, s.summary),
                reason: why,
            })?;
        }
        Ok(())
    })
    .map(|(_, changes)| changes)
    .map_err(|e| match e {
        ModelError::Refused { what, reason } => {
            format!("{what} no longer fits the project: {reason}")
        }
        other => other.to_string(),
    })
}
