//! The Claude assistant window, driven with a scripted transport: no network.

use std::sync::Arc;

use eframe::egui;
use novo_ai::api::testing::Script;
use novo_ai::consent::{Consent, DataScope};
use novo_ai::key::{ApiKey, KeyStore};
use novo_ai::tools::Proposal;
use serde_json::{Value, json};

use super::assistant::*;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn queue<const N: usize>(script: &Script, replies: [Value; N]) {
    for r in replies {
        script.push(200, r);
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
    queue(
        &s.script,
        [
            tool("t1", "project_outline", json!({})),
            text("Your project has devices."),
        ],
    );
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
    queue(
        &s.script,
        [
            tool("t1", "read_project", json!({"section": "devices"})),
            text("I may not look."),
        ],
    );
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
    queue(
        &s.script,
        [
            tool(
                "t1",
                "propose_edit",
                json!({"action": "rename_device", "args": {"device_id": id, "tag": "-K99"}}),
            ),
            text("I suggest renaming it."),
        ],
    );
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
    queue(&s.script, [text("Good.")]);
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
    queue(
        &s.script,
        [
            tool(
                "t1",
                "propose_edit",
                json!({"action": "rename_device", "args": {"device_id": id, "tag": "-K99"}}),
            ),
            text("Suggested."),
        ],
    );
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
    // Spend the cap with one answer that costs more than $1 (400k output tokens on Opus).
    let mut big = text("expensive");
    big["usage"] = json!({"input_tokens": 0, "output_tokens": 400_000});
    queue(&s.script, [big]);
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
    queue(
        &s.script,
        [
            tool("t1", "read_project", json!({"section": "devices"})),
            text("First answer."),
        ],
    );
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
    queue(&s.script, [text("Second answer.")]);
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
    queue(&s.script, [text("One.")]);
    ask(&mut s, "hi");
    s.d.app.settings.ai.consent = Consent::give(DataScope::Project, 1_792_000_100);
    queue(&s.script, [text("Two.")]);
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
    let prop = |d: &Drive| {
        d.app
            .document()
            .unwrap()
            .project()
            .device(dev.into())
            .unwrap()
            .properties
            .get("note")
            .cloned()
    };
    assert!(prop(&d).is_some());
    // The card says what the next change replaces.
    let again = Proposal {
        id: 1,
        action: "set_property".into(),
        args: json!({"device_id": dev, "key": "note", "value": "other"}),
        summary: String::new(),
    };
    let note = proposal_note(d.app.document().unwrap().project(), &again).unwrap();
    assert!(note.contains("check"), "{note}");
    d.app.run(&ctx, Action::Undo);
    assert!(prop(&d).is_none(), "one undo removes the property");
    // Long keys and values are refused.
    let long = "x".repeat(MAX_VALUE_CHARS + 1);
    assert!(
        proposal_command(
            &known,
            "set_property",
            &json!({"device_id": dev, "key": "note", "value": long}),
        )
        .is_err()
    );
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
    // An existing part assigns and undoes.
    let parts_of = |d: &Drive| {
        d.app
            .document()
            .unwrap()
            .project()
            .device(dev.into())
            .unwrap()
            .parts
            .clone()
    };
    let before = parts_of(&d);
    let part = d
        .app
        .document()
        .unwrap()
        .project()
        .parts()
        .map(|x| x.id)
        .find(|id| !before.contains(id))
        .expect("the example has a part not on the first device");
    let (cmd, summary) = proposal_command(
        &known,
        "assign_part",
        &json!({"device_id": dev, "part_id": part.raw()}),
    )
    .unwrap();
    assert!(summary.starts_with("Assign part"));
    assert!(d.app.execute(cmd).unwrap().is_ok());
    assert!(parts_of(&d).contains(&part));
    d.app.run(&ctx, Action::Undo);
    assert_eq!(parts_of(&d), before);
}

#[test]
fn assign_part_gives_nothing_away_below_the_whole_project() {
    let mut s = setup(DataScope::Outline);
    let (dev, part) = {
        let p = s.d.app.document().unwrap().project();
        (
            p.devices().next().unwrap().id.raw(),
            p.parts().next().map(|x| x.id.raw()).unwrap_or(1),
        )
    };
    queue(
        &s.script,
        [
            tool(
                "t1",
                "propose_edit",
                json!({"action": "assign_part", "args": {"device_id": dev, "part_id": part}}),
            ),
            tool(
                "t2",
                "propose_edit",
                json!({"action": "assign_part", "args": {"device_id": dev, "part_id": 999_999}}),
            ),
            text("done"),
        ],
    );
    ask(&mut s, "Assign a part");
    let seen = s.script.seen.lock().unwrap();
    let last: Value = serde_json::from_str(&seen.last().unwrap().body).unwrap();
    let results = last["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["content"].as_array())
        .flatten()
        .filter(|c| c["type"] == "tool_result")
        .map(|c| c["content"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0], results[1],
        "same answer for a real and a missing part"
    );
    assert!(results[0].contains("not allowed"), "{}", results[0]);
    drop(seen);
    assert!(
        s.d.app.wb.assistant.first_proposal_index().is_none(),
        "no card was made"
    );
}

#[test]
fn ctrl_k_offers_to_ask_claude_when_nothing_matches_and_only_when_it_is_on() {
    let mut s = setup(DataScope::Outline);
    s.script.push(200, text("Because of the load."));
    let ctx = s.d.ctx.clone();
    s.d.app
        .wb
        .field
        .open(novo_workflow::palette::Mode::All, None);
    s.d.app.wb.field.type_text("why does my motor trip qqzzxx");
    s.d.frame();
    s.d.frame_with(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    for _ in 0..400 {
        s.d.frame();
        if !s.d.app.wb.assistant.is_running()
            && s.d.app.wb.assistant.transcript_text().contains("load")
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let text_now = s.d.app.wb.assistant.transcript_text();
    assert!(
        text_now.contains("why does my motor trip qqzzxx"),
        "{text_now}"
    );
    assert!(text_now.contains("Because of the load."));
    assert!(s.d.app.wb.assistant.open);
    let _ = ctx;
    // Claude off: the same search runs nothing and sends nothing.
    let mut s = setup(DataScope::Outline);
    s.d.app.settings.ai.enabled = false;
    s.d.app
        .wb
        .field
        .open(novo_workflow::palette::Mode::All, None);
    s.d.app.wb.field.type_text("why does my motor trip qqzzxx");
    s.d.frame();
    s.d.frame_with(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(s.script.seen.lock().unwrap().is_empty());
}

#[test]
fn a_ctrl_k_question_while_claude_answers_is_refused_not_doubled() {
    let mut s = setup(DataScope::Outline);
    s.script.push(200, text("First."));
    s.d.app.wb.assistant.set_draft("first question");
    let ctx = s.d.ctx.clone();
    s.d.app.assistant_test_send(&ctx);
    assert!(s.d.app.wb.assistant.is_running());
    s.d.app.ask_claude(&ctx, "second question here".into());
    for _ in 0..400 {
        s.d.frame();
        if !s.d.app.wb.assistant.is_running() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(s.script.seen.lock().unwrap().len(), 1, "one request only");
    let t = s.d.app.wb.assistant.transcript_text();
    assert!(t.contains("still answering") && t.contains("First."), "{t}");
}

#[test]
fn a_single_typed_word_is_not_offered_to_claude() {
    let mut s = setup(DataScope::Outline);
    s.d.app
        .wb
        .field
        .open(novo_workflow::palette::Mode::All, None);
    s.d.app.wb.field.type_text("qqzzxx");
    s.d.frame();
    s.d.frame_with(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    s.d.frame();
    assert!(s.script.seen.lock().unwrap().is_empty());
}

#[test]
fn a_question_queued_from_elsewhere_opens_the_window_and_is_sent_once() {
    let mut s = setup(DataScope::Outline);
    s.d.app.wb.assistant.open = false;
    s.script.push(200, text("It means the wire has no end."));
    s.d.app
        .wb
        .assistant
        .ask_later("Explain this problem".into());
    for _ in 0..400 {
        s.d.frame();
        if s.d.app.wb.assistant.transcript_text().contains("no end") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(s.d.app.wb.assistant.open);
    assert_eq!(s.script.seen.lock().unwrap().len(), 1);
}

#[test]
fn a_queued_question_is_not_sent_while_claude_is_off() {
    let mut s = setup(DataScope::Outline);
    s.d.app.settings.ai.enabled = false;
    s.d.app
        .wb
        .assistant
        .ask_later("Explain this problem".into());
    for _ in 0..20 {
        s.d.frame();
    }
    assert!(s.script.seen.lock().unwrap().is_empty());
    assert!(
        s.d.app
            .wb
            .assistant
            .transcript_text()
            .contains("nothing was sent")
    );
}

#[test]
fn a_picture_of_the_window_goes_with_one_message_and_never_unasked() {
    let mut s = setup(DataScope::None);
    queue(&s.script, [text("I see a drawing."), text("Ok.")]);
    ask(&mut s, "hello");
    assert!(!s.script.seen.lock().unwrap()[0].body.contains("\"image\""));
    s.d.app.wb.assistant.attach_picture_requested();
    s.d.frame();
    let shot = egui::ColorImage::filled([40, 20], egui::Color32::from_rgb(10, 120, 200));
    s.d.frame_with(vec![egui::Event::Screenshot {
        viewport_id: egui::ViewportId::ROOT,
        user_data: egui::UserData::default(),
        image: Arc::new(shot),
    }]);
    assert!(s.d.app.wb.assistant.has_picture());
    ask(&mut s, "what is on screen?");
    let seen = s.script.seen.lock().unwrap();
    let body: Value = serde_json::from_str(&seen[1].body).unwrap();
    let content = &body["messages"][2]["content"];
    assert_eq!(content[0]["type"], "image");
    assert!(
        content[0]["source"]["data"]
            .as_str()
            .unwrap()
            .starts_with("iVBORw0KGgo")
    );
    drop(seen);
    assert!(
        !s.d.app.wb.assistant.has_picture(),
        "the picture is used once"
    );
}

fn attach(s: &mut Setup) {
    s.d.app.wb.assistant.attach_picture_requested();
    s.d.frame();
    let shot = egui::ColorImage::filled([40, 20], egui::Color32::from_rgb(10, 120, 200));
    s.d.frame_with(vec![egui::Event::Screenshot {
        viewport_id: egui::ViewportId::ROOT,
        user_data: egui::UserData::default(),
        image: Arc::new(shot),
    }]);
    assert!(s.d.app.wb.assistant.has_picture());
}

#[test]
fn clear_chat_drops_an_attached_picture() {
    let mut s = setup(DataScope::Project);
    s.script.push(200, text("Ok."));
    attach(&mut s);
    s.d.app.wb.assistant.clear_chat();
    assert!(!s.d.app.wb.assistant.has_picture());
    ask(&mut s, "hello");
    assert!(!s.script.seen.lock().unwrap()[0].body.contains("\"image\""));
}

#[test]
fn narrowing_the_scope_drops_an_attached_picture() {
    let mut s = setup(DataScope::Project);
    s.script.push(200, text("Ok."));
    attach(&mut s);
    s.d.app.settings.ai.consent = Consent::give(DataScope::None, 1_792_000_100);
    ask(&mut s, "hello");
    assert!(!s.script.seen.lock().unwrap()[0].body.contains("\"image\""));
    assert!(
        s.d.app
            .wb
            .assistant
            .transcript_text()
            .contains("attached picture")
    );
}

#[test]
fn a_panel_can_leave_a_question_and_only_when_claude_is_on() {
    let mut s = setup(DataScope::Outline);
    s.script.push(200, text("The cabinet stays cool."));
    s.d.frame();
    let ctx = s.d.ctx.clone();
    assert!(super::assistant::claude_is_on(&ctx));
    super::assistant::ask_claude_later(&ctx, "Describe this heat result".into());
    for _ in 0..400 {
        s.d.frame();
        if s.d
            .app
            .wb
            .assistant
            .transcript_text()
            .contains("stays cool")
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(s.script.seen.lock().unwrap().len(), 1);
    // Off: the button is not offered and a left question is dropped.
    let mut s = setup(DataScope::Outline);
    s.d.app.settings.ai.enabled = false;
    s.d.frame();
    let ctx = s.d.ctx.clone();
    assert!(!super::assistant::claude_is_on(&ctx));
    super::assistant::ask_claude_later(&ctx, "Describe this heat result".into());
    for _ in 0..20 {
        s.d.frame();
    }
    assert!(s.script.seen.lock().unwrap().is_empty());
}
