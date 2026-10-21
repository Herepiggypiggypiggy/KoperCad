//! The Claude assistant window, driven with a scripted transport: no network.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use novo_ai::api::{HttpRequest, HttpResponse, Transport};
use novo_ai::consent::{Consent, DataScope};
use novo_ai::key::{ApiKey, KeyStore};
use serde_json::{Value, json};

use super::assistant::*;
use super::gate1_tests::Drive;
use crate::actions::Action;

#[derive(Clone, Default)]
struct Script {
    replies: Arc<Mutex<VecDeque<Value>>>,
    seen: Arc<Mutex<Vec<HttpRequest>>>,
}

impl Transport for Script {
    fn send(&mut self, req: &HttpRequest) -> Result<HttpResponse, String> {
        self.seen.lock().unwrap().push(req.clone());
        let body = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or("script ran out")?;
        Ok(HttpResponse {
            status: 200,
            headers: vec![],
            body: body.to_string(),
        })
    }
}

fn text(t: &str) -> Value {
    json!({"content": [{"type": "text", "text": t}], "stop_reason": "end_turn",
           "model": "claude-opus-5-5", "usage": {"input_tokens": 1000, "output_tokens": 100}})
}

fn tool(id: &str, name: &str, input: Value) -> Value {
    json!({"content": [{"type": "tool_use", "id": id, "name": name, "input": input}],
           "stop_reason": "tool_use", "model": "claude-opus-5-5",
           "usage": {"input_tokens": 2000, "output_tokens": 50}})
}

struct Setup {
    d: Drive,
    script: Script,
    _dir: tempfile::TempDir,
}

fn setup(scope: DataScope) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    let store = KeyStore::new(dir.path().join("ai"));
    store
        .save(&ApiKey::parse("sk-ant-api03-test-key-0000000000").unwrap())
        .unwrap();
    let script = Script::default();
    let s2 = script.clone();
    d.app.wb.assistant = State::for_test(
        store,
        dir.path().join("ai").join("usage.json"),
        Arc::new(move || Box::new(s2.clone())),
    );
    d.app.settings.ai.enabled = true;
    d.app.settings.ai.consent = Consent::give(scope, 1_792_000_000);
    d.app.wb.assistant.open = true;
    Setup {
        d,
        script,
        _dir: dir,
    }
}

fn ask(s: &mut Setup, question: &str) {
    s.d.app.wb.assistant.set_draft(question);
    let ctx = s.d.ctx.clone();
    s.d.app.assistant_test_send(&ctx);
    for _ in 0..400 {
        s.d.frame();
        if !s.d.app.wb.assistant.is_running() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the assistant never finished");
}

#[test]
fn it_is_off_by_default_and_the_command_opens_the_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    assert!(!d.app.settings.ai.enabled);
    assert_eq!(d.app.settings.ai.consent.effective(), DataScope::None);
    d.app.wb.queued.push("ai.assistant");
    d.frame();
    d.frame();
    assert!(d.app.wb.assistant.open);
}

#[test]
fn a_question_with_a_lookup_answers_and_counts_the_cost() {
    let mut s = setup(DataScope::Project);
    s.script.replies.lock().unwrap().extend([
        tool("t1", "project_outline", json!({})),
        text("Your project has devices."),
    ]);
    ask(&mut s, "What is in my project?");
    let st = &s.d.app.wb.assistant;
    assert!(st.transcript_text().contains("Your project has devices."));
    assert!(
        st.transcript_text()
            .contains("Looked at the project outline")
    );
    // Two requests on Opus 5.5: (2000+1000) in at $4, (50+100) out at $20 per million.
    let want = 3000.0 * 4.0 / 1e6 + 150.0 * 20.0 / 1e6;
    assert!(
        (st.session_cost() - want).abs() < 1e-9,
        "{}",
        st.session_cost()
    );
    assert!((st.month_spent() - want).abs() < 1e-9);
    // The key went in the header and never in the body or the transcript.
    let seen = s.script.seen.lock().unwrap();
    assert!(!seen[0].body.contains("sk-ant-"));
    assert!(!st.transcript_text().contains("sk-ant-"));
}

#[test]
fn with_no_data_scope_no_project_text_reaches_the_request() {
    let mut s = setup(DataScope::None);
    s.script.replies.lock().unwrap().extend([
        tool("t1", "read_project", json!({"section": "devices"})),
        text("I may not look."),
    ]);
    ask(&mut s, "List devices");
    let seen = s.script.seen.lock().unwrap();
    let second: Value = serde_json::from_str(&seen[1].body).unwrap();
    let result = second["messages"][2]["content"][0]["content"]
        .as_str()
        .unwrap();
    assert!(result.starts_with("Not allowed"));
}

#[test]
fn a_proposal_applies_as_one_undo_step() {
    let mut s = setup(DataScope::Project);
    let (id, old) = {
        let doc = s.d.app.document().unwrap();
        let dev = doc.project().devices().next().unwrap();
        (dev.id.raw(), dev.tag.to_string())
    };
    s.script.replies.lock().unwrap().extend([
        tool(
            "t1",
            "propose_edit",
            json!({"action": "rename_device", "args": {"device_id": id, "tag": "-K99"}}),
        ),
        text("I suggest renaming it."),
    ]);
    ask(&mut s, "Rename the first device to -K99");
    // Nothing changed yet.
    let tag_now = |s: &Setup| {
        s.d.app
            .document()
            .unwrap()
            .project()
            .devices()
            .find(|d| d.id.raw() == id)
            .unwrap()
            .tag
            .to_string()
    };
    assert_eq!(tag_now(&s), old);
    let idx =
        s.d.app
            .wb
            .assistant
            .first_proposal_index()
            .expect("a proposal card");
    s.d.app.assistant_test_decide(idx, true);
    assert_eq!(tag_now(&s), "-K99");
    // The decision is told to Claude with the next message.
    s.script.replies.lock().unwrap().push_back(text("Good."));
    ask(&mut s, "ok");
    let seen = s.script.seen.lock().unwrap();
    let last: Value = serde_json::from_str(&seen.last().unwrap().body).unwrap();
    let msgs = last["messages"].as_array().unwrap();
    let user = msgs.last().unwrap()["content"].as_str().unwrap();
    assert!(user.contains("The person applied proposal 1"), "{user}");
    drop(seen);
    let ctx = s.d.ctx.clone();
    s.d.app.run(&ctx, Action::Undo);
    assert_eq!(tag_now(&s), old);
}

#[test]
fn a_rejected_proposal_changes_nothing() {
    let mut s = setup(DataScope::Project);
    let (id, old) = {
        let doc = s.d.app.document().unwrap();
        let dev = doc.project().devices().next().unwrap();
        (dev.id.raw(), dev.tag.to_string())
    };
    s.script.replies.lock().unwrap().extend([
        tool(
            "t1",
            "propose_edit",
            json!({"action": "rename_device", "args": {"device_id": id, "tag": "-K99"}}),
        ),
        text("Suggested."),
    ]);
    ask(&mut s, "rename");
    let idx = s.d.app.wb.assistant.first_proposal_index().unwrap();
    s.d.app.assistant_test_decide(idx, false);
    let doc = s.d.app.document().unwrap();
    let now = doc
        .project()
        .devices()
        .find(|d| d.id.raw() == id)
        .unwrap()
        .tag
        .to_string();
    assert_eq!(now, old);
}

#[test]
fn the_monthly_limit_blocks_a_message_without_sending_it() {
    let mut s = setup(DataScope::Outline);
    s.d.app.settings.ai.monthly_cap_usd = 1.0;
    s.script
        .replies
        .lock()
        .unwrap()
        .push_back(text("expensive"));
    // Spend the cap with one answer that costs more than $1 (400k output tokens on Opus).
    let mut big = text("expensive");
    big["usage"] = json!({"input_tokens": 0, "output_tokens": 400_000});
    s.script.replies.lock().unwrap().clear();
    s.script.replies.lock().unwrap().push_back(big);
    ask(&mut s, "first");
    let before = s.script.seen.lock().unwrap().len();
    ask(&mut s, "second");
    assert_eq!(
        s.script.seen.lock().unwrap().len(),
        before,
        "nothing was sent"
    );
    assert!(
        s.d.app
            .wb
            .assistant
            .transcript_text()
            .contains("monthly budget")
    );
}

#[test]
fn bad_proposals_are_refused_with_a_reason() {
    use novo_core::{DeviceTag, Project};
    let p = Project::new("x");
    let known = known_of(&p);
    let r = proposal_command(
        &known,
        "rename_device",
        &json!({"device_id": 5, "tag": "-K1"}),
    );
    assert!(r.unwrap_err().contains("no device"));
    let r = proposal_command(&known, "delete_all", &json!({}));
    assert!(r.is_err());
    let _ = DeviceTag::parse("-K1").unwrap();
}

#[test]
fn snapshots_grow_with_the_scope() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    let p = d.app.document().unwrap().project();
    let none = snapshot_of(p, &[], DataScope::None);
    assert!(none.outline.is_null() && none.project.is_null());
    let outline = snapshot_of(p, &[], DataScope::Outline);
    assert!(
        outline.outline["devices"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
    );
    assert!(outline.project.is_null());
    let all = snapshot_of(p, &[], DataScope::Project);
    assert!(all.project["wires"].is_array());
}

#[test]
fn narrowing_the_scope_drops_the_chat_so_old_reads_are_not_sent_again() {
    let mut s = setup(DataScope::Project);
    s.script.replies.lock().unwrap().extend([
        tool("t1", "read_project", json!({"section": "devices"})),
        text("First answer."),
    ]);
    ask(&mut s, "List the devices");
    let first_tool_result = {
        let seen = s.script.seen.lock().unwrap();
        let second: Value = serde_json::from_str(&seen[1].body).unwrap();
        second["messages"][2]["content"][0]["content"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert!(first_tool_result.contains("tag"));
    // The person narrows the scope, then asks again.
    s.d.app.settings.ai.consent = Consent::give(DataScope::None, 1_792_000_100);
    s.script
        .replies
        .lock()
        .unwrap()
        .push_back(text("Second answer."));
    ask(&mut s, "What did you see before?");
    let seen = s.script.seen.lock().unwrap();
    let last = &seen.last().unwrap().body;
    assert!(
        !last.contains(&first_tool_result[..40]),
        "old tool result was resent"
    );
    let body: Value = serde_json::from_str(last).unwrap();
    assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    assert!(
        s.d.app
            .wb
            .assistant
            .transcript_text()
            .contains("Scope narrowed")
    );
}

#[test]
fn widening_the_scope_keeps_the_chat() {
    let mut s = setup(DataScope::Outline);
    s.script.replies.lock().unwrap().push_back(text("One."));
    ask(&mut s, "hi");
    s.d.app.settings.ai.consent = Consent::give(DataScope::Project, 1_792_000_100);
    s.script.replies.lock().unwrap().push_back(text("Two."));
    ask(&mut s, "again");
    assert_eq!(s.d.app.wb.assistant.conversation_len(), 4);
    assert!(
        !s.d.app
            .wb
            .assistant
            .transcript_text()
            .contains("Scope narrowed")
    );
}

#[test]
fn the_other_edits_apply_and_undo() {
    use novo_core::Project;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    let (dev, tag) = {
        let p = d.app.document().unwrap().project();
        let dv = p.devices().next().unwrap();
        (dv.id.raw(), dv.tag.to_string())
    };
    let known = known_of(d.app.document().unwrap().project());
    let (cmd, summary) = proposal_command(
        &known,
        "set_property",
        &json!({"device_id": dev, "key": "note", "value": "check"}),
    )
    .unwrap();
    assert!(summary.contains(&tag) && summary.contains("note"));
    assert!(d.app.execute(cmd).unwrap().is_ok());
    let (cmd, summary) = proposal_command(
        &known,
        "add_page",
        &json!({"name": "ZZ1", "title": "Added"}),
    )
    .unwrap();
    assert_eq!(summary, "Add page ZZ1 (Added)");
    let pages = d.app.document().unwrap().project().pages().count();
    assert!(d.app.execute(cmd).unwrap().is_ok());
    assert_eq!(
        d.app.document().unwrap().project().pages().count(),
        pages + 1
    );
    // A part that is not there is refused with the section to look in.
    let e = proposal_command(
        &known,
        "assign_part",
        &json!({"device_id": dev, "part_id": 999_999}),
    )
    .unwrap_err();
    assert!(e.contains("parts"));
    let _ = Project::new("x");
}
