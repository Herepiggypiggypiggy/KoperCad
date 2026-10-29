//! The engine's requests, natively. The same requests run against the built `engine.wasm` in
//! `crates/novo-tests/tests/controllers_addon.rs`.

use controllers_engine::Engine;
use serde_json::{Value, json};

fn call(e: &mut Engine, op: &str, data: Value) -> Value {
    let reply = e.handle(&json!({"api": 1, "op": op, "data": data}).to_string());
    serde_json::from_str(&reply).expect("the reply is JSON")
}

fn ok(e: &mut Engine, op: &str, data: Value) -> Value {
    let r = call(e, op, data);
    assert_eq!(r["ok"], true, "{op}: {r}");
    r["data"].clone()
}

fn refused(e: &mut Engine, op: &str, data: Value) -> String {
    let r = call(e, op, data);
    assert_eq!(r["ok"], false, "{op} should be refused: {r}");
    r["error"].as_str().unwrap().to_owned()
}

fn level(snapshot: &Value, signal: &str) -> bool {
    snapshot["instances"][0]["signals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == signal)
        .unwrap_or_else(|| panic!("no signal {signal}"))["high"]
        .as_bool()
        .unwrap()
}

fn act(e: &mut Engine, action: Value) -> Value {
    ok(e, "act", json!({"instance": "radio", "action": action}))["snapshot"].clone()
}

#[test]
fn hello_and_the_starting_state() {
    let mut e = Engine::new();
    assert_eq!(ok(&mut e, "hello", json!({})), json!({"api": 1}));
    let s = ok(&mut e, "snapshot", json!({}));
    assert_eq!(s["instances"][0]["id"], "radio");
    assert_eq!(s["instances"][0]["generic"], true);
    // nothing is high before main is on
    assert!(!level(&s, "H_UP_SLOW"));
    assert!(!level(&s, "MAIN"));
}

#[test]
fn rocker_steps_follow_the_two_step_truth_table_through_the_panel_buttons() {
    let mut e = Engine::new();
    ok(
        &mut e,
        "button",
        json!({"panel": "radio", "id": "press-start"}),
    );
    let lamps = |r: &Value, id: &str| r["lamps"][id]["lit"].as_bool().unwrap();
    let r = ok(
        &mut e,
        "button",
        json!({"panel": "radio", "id": "set-hoist-up1"}),
    );
    assert!(lamps(&r, "main"));
    assert!(lamps(&r, "h-up-slow"));
    assert!(!lamps(&r, "h-up-fast"));
    let r = ok(
        &mut e,
        "button",
        json!({"panel": "radio", "id": "set-hoist-up2"}),
    );
    assert!(
        lamps(&r, "h-up-slow") && lamps(&r, "h-up-fast"),
        "step 2 includes step 1"
    );
    let r = ok(
        &mut e,
        "button",
        json!({"panel": "radio", "id": "set-hoist-off"}),
    );
    assert!(!lamps(&r, "h-up-slow") && !lamps(&r, "h-up-fast"));
    // the selector id has an underscore in the profile and a hyphen on the panel
    let r = ok(
        &mut e,
        "button",
        json!({"panel": "radio", "id": "set-hoist-sel-mast"}),
    );
    assert!(!lamps(&r, "h-up-slow"));
}

#[test]
fn estop_drops_main_and_needs_a_reset() {
    let mut e = Engine::new();
    act(&mut e, json!({"action": "press", "control": "start"}));
    let s = act(
        &mut e,
        json!({"action": "set_position", "control": "bridge", "position": "fwd2"}),
    );
    assert!(level(&s, "B_FWD_FAST"));
    let s = act(&mut e, json!({"action": "press", "control": "estop"}));
    assert!(!level(&s, "B_FWD_FAST"));
    assert!(!level(&s, "MAIN"));
    assert!(!level(&s, "ESTOP_OK"));
    // main does not come back by itself
    let s = act(&mut e, json!({"action": "reset", "control": "estop"}));
    assert!(level(&s, "ESTOP_OK"));
    assert!(!level(&s, "MAIN"));
}

#[test]
fn refusals_leave_the_engine_usable() {
    let mut e = Engine::new();
    assert!(refused(&mut e, "nope", json!({})).contains("unknown op"));
    assert!(refused(&mut e, "button", json!({"id": "press-ghost"})).contains("unknown button"));
    assert!(
        refused(&mut e, "button", json!({"id": "set-hoist-sideways"})).contains("unknown button")
    );
    assert!(
        refused(
            &mut e,
            "act",
            json!({"instance": "ghost", "action": {"action":"press","control":"start"}})
        )
        .contains("ghost")
    );
    assert!(
        refused(
            &mut e,
            "act",
            json!({"instance": "radio", "action": {"action":"fly"}})
        )
        .contains("not an action")
    );
    assert!(refused(&mut e, "advance", json!({"dt_ms": 99_999_999_999u64})).contains("hour"));
    let r = e.handle("this is not json");
    assert!(r.contains("\"ok\":false"));
    let r = e.handle(&json!({"api": 2, "op": "hello"}).to_string());
    assert!(r.contains("host API 1"));
    assert_eq!(ok(&mut e, "hello", json!({})), json!({"api": 1}));
}

#[test]
fn profiles_are_validated_and_instances_are_independent() {
    let mut e = Engine::empty();
    assert!(
        refused(&mut e, "instance.add", json!({"id":"a","profile":"ghost"}))
            .contains("no controller profile")
    );
    let bad = refused(
        &mut e,
        "profile.add",
        json!({"toml": "[controller]\nid = \"x\"\nname = \"X\"\n[[control]]\nid = \"b\"\nkind = \"button\"\n[[signal]]\nname = \"S\"\ncontrol = \"ghost\"\n"}),
    );
    assert!(bad.contains("problem"), "{bad}");
    let radio = include_str!("../../pack/data/generic-radio-3x2.controller.toml");
    let added = ok(&mut e, "profile.add", json!({"toml": radio}));
    assert_eq!(added["id"], "generic-radio-3x2");
    ok(
        &mut e,
        "instance.add",
        json!({"id":"one","label":"One","profile":"generic-radio-3x2"}),
    );
    ok(
        &mut e,
        "instance.duplicate",
        json!({"from":"one","id":"two","label":"Two"}),
    );
    ok(
        &mut e,
        "act",
        json!({"instance":"one","action":{"action":"press","control":"start"}}),
    );
    let s = ok(&mut e, "snapshot", json!({}));
    let main = |i: usize| {
        s["instances"][i]["signals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["name"] == "MAIN")
            .unwrap()["high"]
            .as_bool()
            .unwrap()
    };
    assert!(main(0));
    assert!(!main(1), "the duplicate starts in its own start state");
    assert!(
        refused(
            &mut e,
            "instance.add",
            json!({"id":"one","label":"","profile":"generic-radio-3x2"})
        )
        .contains("already exists")
    );
    ok(
        &mut e,
        "instance.rename",
        json!({"id":"two","label":"Crab 2"}),
    );
    ok(&mut e, "instance.remove", json!({"id":"two"}));
    assert_eq!(
        ok(&mut e, "snapshot", json!({}))["instances"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn project_data_round_trips() {
    let mut e = Engine::new();
    act(&mut e, json!({"action": "press", "control": "start"}));
    act(
        &mut e,
        json!({"action": "set_position", "control": "hoist", "position": "down2"}),
    );
    let before = ok(&mut e, "snapshot", json!({}));
    let saved = ok(&mut e, "records.save", json!({}));
    let mut fresh = Engine::new();
    let after = ok(&mut fresh, "records.load", saved);
    assert_eq!(before, after);
}

#[test]
fn recording_replays_to_the_same_signals_twice_and_leaves_the_live_set_alone() {
    let mut e = Engine::new();
    ok(&mut e, "record.start", json!({}));
    act(&mut e, json!({"action": "press", "control": "start"}));
    ok(&mut e, "advance", json!({"dt_ms": 500}));
    act(
        &mut e,
        json!({"action": "set_position", "control": "hoist", "position": "up1"}),
    );
    let seq = ok(&mut e, "record.stop", json!({"name": "hoist up"}))["sequence"]
        .as_str()
        .unwrap()
        .to_owned();
    let live = ok(&mut e, "snapshot", json!({}));
    // replay on a fresh engine: the recording started from the start state
    let mut other = Engine::new();
    let a = ok(
        &mut other,
        "replay",
        json!({"sequence": seq, "sample_ms": 100, "settle_ms": 200}),
    );
    let b = ok(
        &mut other,
        "replay",
        json!({"sequence": seq, "sample_ms": 100, "settle_ms": 200}),
    );
    assert_eq!(a, b);
    let last = a["samples"].as_array().unwrap().last().unwrap();
    assert_eq!(last["signals"]["radio/H_UP_SLOW"], 1.0);
    // a replay runs on a copy: the engine it ran in is still in its start state
    let calm = ok(&mut other, "snapshot", json!({}));
    assert_eq!(calm, ok(&mut Engine::new(), "snapshot", json!({})));
    assert_eq!(
        live["instances"][0]["controls"].as_array().unwrap().len(),
        7
    );
    assert!(refused(&mut e, "record.stop", json!({})).contains("no recording"));
}

#[test]
fn the_panel_files_agree_with_the_profile() {
    // Every lamp and button the panel declares exists in the engine, so no button is dead.
    let panel: toml_free::Panel = toml_free::read(include_str!("../../pack/data/radio.panel.toml"));
    let mut e = Engine::new();
    let snap = ok(&mut e, "snapshot", json!({}));
    let signals: Vec<String> = snap["instances"][0]["signals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            s["name"]
                .as_str()
                .unwrap()
                .to_ascii_lowercase()
                .replace('_', "-")
        })
        .collect();
    for lamp in &panel.lamps {
        assert!(signals.contains(lamp), "panel lamp `{lamp}` has no signal");
    }
    for signal in &signals {
        assert!(
            panel.lamps.contains(signal),
            "signal `{signal}` has no lamp on the panel"
        );
    }
    // the lamps the panel draws before the first press agree with the engine's start state
    let high: Vec<String> = snap["instances"][0]["signals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["high"] == true)
        .map(|s| {
            s["name"]
                .as_str()
                .unwrap()
                .to_ascii_lowercase()
                .replace('_', "-")
        })
        .collect();
    assert_eq!(panel.lit_at_start, high);
    for button in &panel.buttons {
        let r = call(&mut e, "button", json!({"panel": "radio", "id": button}));
        assert_eq!(r["ok"], true, "panel button `{button}` is dead: {r}");
    }
}

/// A tiny reader for the panel file: ids of lamps and buttons, without a TOML dependency.
mod toml_free {
    pub struct Panel {
        pub lamps: Vec<String>,
        pub lit_at_start: Vec<String>,
        pub buttons: Vec<String>,
    }
    pub fn read(text: &str) -> Panel {
        let mut p = Panel {
            lamps: vec![],
            lit_at_start: vec![],
            buttons: vec![],
        };
        for chunk in text.split("{ kind = \"").skip(1) {
            let kind = chunk.split('"').next().unwrap();
            let id = chunk
                .split("id = \"")
                .nth(1)
                .and_then(|r| r.split('"').next())
                .unwrap()
                .to_owned();
            match kind {
                "lamp" => {
                    if chunk.contains("lit = true") && chunk.find("lit = true") < chunk.find('}') {
                        p.lit_at_start.push(id.clone());
                    }
                    p.lamps.push(id)
                }
                "button" => p.buttons.push(id),
                _ => {}
            }
        }
        p
    }
}
