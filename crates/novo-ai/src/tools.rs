//! What the assistant may look up and propose.
//!
//! Lookups read a [`Snapshot`] the app took of the open project when the
//! person sent the message, filtered by the data scope they agreed to: a tool
//! that needs a wider scope answers "not allowed" and returns nothing.
//!
//! Claude never edits the real project. In the app `propose_edit` works on a private draft
//! (see `engineer`); the app lists the change set with Apply and Reject buttons and applies it
//! through one transaction, so it is one undo step and nothing happens unseen.

use crate::consent::DataScope;
use serde_json::{Value, json};

/// The project as data, in three sizes. Which one a tool reads depends on the
/// scope; the app should fill only what the scope allows (the tools check too).
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// Counts, pages, device tags.
    pub outline: Value,
    /// The selected items in full.
    pub selection: Value,
    /// Sections of the whole project: `devices`, `wires`, `terminals`, ...
    pub project: Value,
}

/// An edit Claude suggests, waiting for the person.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub id: u32,
    /// The app's name for the edit, from [`EditAction`].
    pub action: String,
    pub args: Value,
    /// One line the person reads before pressing Apply.
    pub summary: String,
}

/// An edit the app knows how to apply.
#[derive(Clone, Debug)]
pub struct EditAction {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON schema of `args`.
    pub schema: Value,
}

/// What a tool returns to the model.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolReply {
    pub text: String,
    pub is_error: bool,
}

impl ToolReply {
    fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
        }
    }
    fn err(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
        }
    }
}

/// Runs the tools of one conversation.
pub trait ToolHost {
    fn specs(&self) -> Vec<Value>;
    fn call(&mut self, name: &str, input: &Value) -> ToolReply;
    /// Proposals recorded so far (taken by the caller after the turn).
    fn take_proposals(&mut self) -> Vec<Proposal>;
}

/// Checks a proposal against the live project and returns its one-line
/// summary, or the reason it cannot be done (which goes back to Claude).
pub type Validator = Box<dyn Fn(&str, &Value) -> Result<String, String> + Send>;

/// Longest tool answer sent back, in characters.
pub const MAX_TOOL_CHARS: usize = 30_000;
const DEFAULT_LIMIT: usize = 40;

pub struct SnapshotHost {
    snapshot: Snapshot,
    scope: DataScope,
    actions: Vec<EditAction>,
    validate: Validator,
    proposals: Vec<Proposal>,
    next_id: u32,
}

impl SnapshotHost {
    pub fn new(
        snapshot: Snapshot,
        scope: DataScope,
        actions: Vec<EditAction>,
        validate: Validator,
    ) -> Self {
        Self {
            snapshot,
            scope,
            actions,
            validate,
            proposals: Vec::new(),
            next_id: 1,
        }
    }

    /// Replaces what the read tools see (the project changed in a draft).
    pub fn set_snapshot(&mut self, snapshot: Snapshot) {
        self.snapshot = snapshot;
    }

    /// First proposal id to use (ids continue across turns).
    pub fn starting_at(mut self, next_id: u32) -> Self {
        self.next_id = next_id;
        self
    }

    fn denied(&self, needed: DataScope) -> ToolReply {
        ToolReply::err(format!(
            "Not allowed: this needs the \"{}\" data scope and the person has allowed \"{}\". \
             Tell them what you would need and why; do not guess.",
            needed.label(),
            self.scope.label()
        ))
    }

    fn read_project(&self, input: &Value) -> ToolReply {
        if !self.scope.allows(DataScope::Project) {
            return self.denied(DataScope::Project);
        }
        let Some(sections) = self.snapshot.project.as_object() else {
            return ToolReply::err("No project is open.");
        };
        let section = input["section"].as_str().unwrap_or_default();
        let Some(data) = sections.get(section) else {
            let names: Vec<&String> = sections.keys().collect();
            return ToolReply::err(format!("Unknown section {section:?}. Sections: {names:?}."));
        };
        let Some(items) = data.as_array() else {
            return ToolReply::ok(clip(&data.to_string()));
        };
        let needle = input["filter"].as_str().map(str::to_lowercase);
        let matching: Vec<&Value> = items
            .iter()
            .filter(|i| {
                needle
                    .as_deref()
                    .is_none_or(|n| i.to_string().to_lowercase().contains(n))
            })
            .collect();
        let offset = input["offset"].as_u64().unwrap_or(0) as usize;
        let limit = input["limit"]
            .as_u64()
            .map_or(DEFAULT_LIMIT, |l| l as usize)
            .clamp(1, 200);
        let page: Vec<&Value> = matching.iter().skip(offset).take(limit).copied().collect();
        let out = json!({
            "section": section,
            "total_matching": matching.len(),
            "offset": offset,
            "items": page,
        });
        ToolReply::ok(clip(&out.to_string()))
    }

    fn propose(&mut self, input: &Value) -> ToolReply {
        // Suggesting changes to a project it has not been allowed to see is guesswork.
        if !self.scope.allows(DataScope::Outline) {
            return self.denied(DataScope::Outline);
        }
        let action = input["action"].as_str().unwrap_or_default();
        if !self.actions.iter().any(|a| a.name == action) {
            let names: Vec<&str> = self.actions.iter().map(|a| a.name).collect();
            return ToolReply::err(format!("Unknown action {action:?}. Actions: {names:?}."));
        }
        let args = input["args"].clone();
        match (self.validate)(action, &args) {
            Err(why) => ToolReply::err(format!("Cannot propose that: {why}")),
            Ok(summary) => {
                let id = self.next_id;
                self.next_id += 1;
                self.proposals.push(Proposal {
                    id,
                    action: action.to_string(),
                    args,
                    summary: summary.clone(),
                });
                ToolReply::ok(format!(
                    "Proposal {id} recorded: {summary}. It is NOT applied; the person must \
                     press Apply. Do not say it is done."
                ))
            }
        }
    }
}

/// [`clip`] for the other tool hosts of this crate.
pub(crate) fn clip_for_tools(s: &str) -> String {
    clip(s)
}

fn clip(s: &str) -> String {
    if s.chars().count() <= MAX_TOOL_CHARS {
        return s.to_string();
    }
    let cut: String = s.chars().take(MAX_TOOL_CHARS).collect();
    format!("{cut}\n…[cut at {MAX_TOOL_CHARS} characters; narrow with filter, limit and offset]")
}

impl ToolHost for SnapshotHost {
    fn specs(&self) -> Vec<Value> {
        let mut specs = vec![
            json!({
                "name": "project_outline",
                "description": "Counts, page names and device tags of the open project. Cheap; start here.",
                "input_schema": {"type": "object", "properties": {}, "additionalProperties": false},
                "strict": true,
            }),
            json!({
                "name": "selection",
                "description": "Full details of what the person has selected right now.",
                "input_schema": {"type": "object", "properties": {}, "additionalProperties": false},
                "strict": true,
            }),
            json!({
                "name": "read_project",
                "description": "Reads one section of the whole project (see project_outline for \
                                section names). Returns a page of items; use filter (text match), \
                                limit and offset to narrow.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "section": {"type": "string"},
                        "filter": {"type": "string", "description": "Case-insensitive text an item must contain."},
                        "limit": {"type": "integer"},
                        "offset": {"type": "integer"}
                    },
                    "required": ["section"],
                    "additionalProperties": false
                },
            }),
        ];
        if !self.actions.is_empty() {
            let names: Vec<&str> = self.actions.iter().map(|a| a.name).collect();
            let catalogue: Vec<Value> = self
                .actions
                .iter()
                .map(|a| json!({"action": a.name, "what": a.description, "args": a.schema}))
                .collect();
            specs.push(json!({
                "name": "propose_edit",
                "description": format!(
                    "Suggests a change to the project. Nothing is changed: the person sees it \
                     and presses Apply or Reject. Available actions: {}",
                    Value::Array(catalogue)
                ),
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "action": {"type": "string", "enum": names},
                        "args": {"type": "object"}
                    },
                    "required": ["action", "args"]
                },
            }));
        }
        specs
    }

    fn call(&mut self, name: &str, input: &Value) -> ToolReply {
        match name {
            "project_outline" => {
                if !self.scope.allows(DataScope::Outline) {
                    return self.denied(DataScope::Outline);
                }
                ToolReply::ok(clip(&self.snapshot.outline.to_string()))
            }
            "selection" => {
                if !self.scope.allows(DataScope::Selection) {
                    return self.denied(DataScope::Selection);
                }
                ToolReply::ok(clip(&self.snapshot.selection.to_string()))
            }
            "read_project" => self.read_project(input),
            "propose_edit" => self.propose(input),
            other => ToolReply::err(format!("Unknown tool {other:?}.")),
        }
    }

    fn take_proposals(&mut self) -> Vec<Proposal> {
        std::mem::take(&mut self.proposals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            outline: json!({"pages": 3, "devices": 12}),
            selection: json!([{"tag": "-K1"}]),
            project: json!({
                "devices": [{"tag": "-K1", "part": "Relay X"}, {"tag": "-Q1", "part": "Breaker"},
                            {"tag": "-K2", "part": "Relay Y"}],
            }),
        }
    }

    fn action() -> EditAction {
        EditAction {
            name: "set_note",
            description: "Sets a device note",
            schema: json!({"type": "object"}),
        }
    }

    fn host(scope: DataScope) -> SnapshotHost {
        SnapshotHost::new(
            snapshot(),
            scope,
            vec![action()],
            Box::new(|a, args| {
                if args["tag"].as_str() == Some("-K1") {
                    Ok(format!("{a} on -K1"))
                } else {
                    Err("no such device".into())
                }
            }),
        )
    }

    #[test]
    fn a_narrow_scope_gets_nothing_wider() {
        let mut h = host(DataScope::None);
        for n in ["project_outline", "selection", "read_project"] {
            let r = h.call(n, &json!({"section": "devices"}));
            assert!(r.is_error, "{n}");
            assert!(!r.text.contains("Relay"), "{n} leaked");
        }
        let mut h = host(DataScope::Outline);
        assert!(!h.call("project_outline", &json!({})).is_error);
        assert!(h.call("selection", &json!({})).is_error);
        assert!(
            h.call("read_project", &json!({"section": "devices"}))
                .is_error
        );
        let mut h = host(DataScope::Selection);
        assert!(!h.call("selection", &json!({})).is_error);
        assert!(
            h.call("read_project", &json!({"section": "devices"}))
                .is_error
        );
    }

    #[test]
    fn the_project_scope_reads_sections_with_filter_and_paging() {
        let mut h = host(DataScope::Project);
        let r = h.call(
            "read_project",
            &json!({"section": "devices", "filter": "relay"}),
        );
        let v: Value = serde_json::from_str(&r.text).unwrap();
        assert_eq!(v["total_matching"], 2);
        let r = h.call(
            "read_project",
            &json!({"section": "devices", "limit": 1, "offset": 1}),
        );
        let v: Value = serde_json::from_str(&r.text).unwrap();
        assert_eq!(v["items"][0]["tag"], "-Q1");
        let r = h.call("read_project", &json!({"section": "nope"}));
        assert!(r.is_error && r.text.contains("devices"));
    }

    #[test]
    fn long_answers_are_cut_with_a_hint() {
        let big: Vec<Value> = (0..2000)
            .map(|i| json!({"tag": format!("-K{i}"), "pad": "x".repeat(400)}))
            .collect();
        let mut s = snapshot();
        s.project = json!({"devices": big});
        let mut h = SnapshotHost::new(
            s,
            DataScope::Project,
            vec![],
            Box::new(|_, _| Ok(String::new())),
        );
        let r = h.call("read_project", &json!({"section": "devices", "limit": 200}));
        assert!(r.text.chars().count() < MAX_TOOL_CHARS + 200);
        assert!(r.text.contains("narrow with filter"));
    }

    #[test]
    fn a_proposal_is_recorded_not_applied_and_the_model_is_told() {
        let mut h = host(DataScope::Outline);
        let r = h.call(
            "propose_edit",
            &json!({"action": "set_note", "args": {"tag": "-K1"}}),
        );
        assert!(!r.is_error);
        assert!(r.text.contains("NOT applied"));
        let p = h.take_proposals();
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].id, p[0].action.as_str()), (1, "set_note"));
        assert!(h.take_proposals().is_empty());
    }

    #[test]
    fn no_proposals_without_any_project_access() {
        let mut h = host(DataScope::None);
        let r = h.call(
            "propose_edit",
            &json!({"action": "set_note", "args": {"tag": "-K1"}}),
        );
        assert!(r.is_error);
        assert!(h.take_proposals().is_empty());
    }

    #[test]
    fn bad_proposals_go_back_to_the_model_as_errors() {
        let mut h = host(DataScope::Outline);
        let r = h.call(
            "propose_edit",
            &json!({"action": "set_note", "args": {"tag": "-Z9"}}),
        );
        assert!(r.is_error && r.text.contains("no such device"));
        let r = h.call(
            "propose_edit",
            &json!({"action": "delete_everything", "args": {}}),
        );
        assert!(r.is_error);
        assert!(h.take_proposals().is_empty());
    }

    #[test]
    fn without_edit_actions_there_is_no_propose_tool() {
        let h = SnapshotHost::new(
            snapshot(),
            DataScope::Project,
            vec![],
            Box::new(|_, _| Ok(String::new())),
        );
        assert!(!h.specs().iter().any(|s| s["name"] == "propose_edit"));
        assert!(
            host(DataScope::Project)
                .specs()
                .iter()
                .any(|s| s["name"] == "propose_edit")
        );
    }
}
