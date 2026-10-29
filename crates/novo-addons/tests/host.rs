//! The engine host: a well-behaved engine works, and every hostile or broken one is stopped or
//! refused without hurting the program. Engines are written in WebAssembly text and built here;
//! the program itself only ever reads binary.

mod common;

use common::*;
use novo_addons::{AddonError, CallError, EngineHost, HostLimits, InstallOptions};
use serde_json::json;

/// An engine that picks what to do by the first letter of the op it is sent.
fn engine(extra_imports: &str, extra: &str) -> Vec<u8> {
    let text = format!(
        r#"(module
  (import "kopercad" "log" (func $log (param i32 i32)))
  {extra_imports}
  (memory (export "memory") 4)
  (global $heap (mut i32) (i32.const 8192))
  (data (i32.const 0) "{{\"ok\":true,\"data\":{{\"api\":1}}}}")
  (data (i32.const 64) "{{\"ok\":false,\"error\":\"not now\"}}")
  (data (i32.const 128) "not json at all")
  (data (i32.const 160) "{{\"data\":1}}")
  (data (i32.const 192) "{{\"ok\":true,\"data\":{{\"grew\":false}}}}")
  (data (i32.const 256) "{{\"ok\":true,\"data\":{{\"grew\":true}}}}")
  (data (i32.const 320) "{{\"ok\":true,\"data\":")
  (data (i32.const 384) "engine says hello")
  (func (export "kc_alloc") (param $len i32) (result i32)
    (local $p i32)
    (if (i32.gt_u (global.get $heap) (i32.const 100000)) (then (global.set $heap (i32.const 8192))))
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (local.get $p) (local.get $len)))
    (local.get $p))
  (func $first (param $ptr i32) (param $len i32) (result i32)
    (local $i i32)
    (local.set $i (i32.sub (i32.add (local.get $ptr) (local.get $len)) (i32.const 3)))
    (block $done (loop $scan
      (br_if $done (i32.eq (i32.load8_u (local.get $i)) (i32.const 34)))
      (local.set $i (i32.sub (local.get $i) (i32.const 1)))
      (br $scan)))
    (i32.load8_u (i32.add (local.get $i) (i32.const 1))))
  (func $reply (param $at i32) (param $len i32) (result i64)
    (i64.or (i64.shl (i64.extend_i32_u (local.get $at)) (i64.const 32)) (i64.extend_i32_u (local.get $len))))
  (func $recurse (param $n i32) (result i32)
    (i32.add (call $recurse (i32.add (local.get $n) (i32.const 1))) (i32.const 1)))
  (func (export "kc_call") (param $ptr i32) (param $len i32) (result i64)
    (local $c i32) (local $out i32)
    (local.set $c (call $first (local.get $ptr) (local.get $len)))
    ;; hello
    (if (i32.eq (local.get $c) (i32.const 104)) (then (return (call $reply (i32.const 0) (i32.const 28)))))
    ;; refuse
    (if (i32.eq (local.get $c) (i32.const 114)) (then (return (call $reply (i32.const 64) (i32.const 30)))))
    ;; junk
    (if (i32.eq (local.get $c) (i32.const 106)) (then (return (call $reply (i32.const 128) (i32.const 15)))))
    ;; no ok
    (if (i32.eq (local.get $c) (i32.const 110)) (then (return (call $reply (i32.const 160) (i32.const 10)))))
    ;; spin
    (if (i32.eq (local.get $c) (i32.const 115)) (then (loop $l (br $l))))
    ;; trap
    (if (i32.eq (local.get $c) (i32.const 116)) (then unreachable))
    ;; huge reply length
    (if (i32.eq (local.get $c) (i32.const 117)) (then (return (call $reply (i32.const 0) (i32.const 2000000000)))))
    ;; reply outside memory
    (if (i32.eq (local.get $c) (i32.const 111)) (then (return (call $reply (i32.const 2000000000) (i32.const 100)))))
    ;; deep recursion
    (if (i32.eq (local.get $c) (i32.const 100)) (then (drop (call $recurse (i32.const 0)))))
    ;; grow memory by a lot
    (if (i32.eq (local.get $c) (i32.const 103)) (then
      (if (i32.eq (memory.grow (i32.const 100000)) (i32.const -1))
        (then (return (call $reply (i32.const 192) (i32.const 33))))
        (else (return (call $reply (i32.const 256) (i32.const 32)))))))
    ;; write a log line
    (if (i32.eq (local.get $c) (i32.const 119)) (then
      (call $log (i32.const 384) (i32.const 17))
      (return (call $reply (i32.const 0) (i32.const 28)))))
    {extra}
    ;; echo: {{"ok":true,"data":<request>}}
    (local.set $out (i32.const 16384))
    (memory.copy (local.get $out) (i32.const 320) (i32.const 18))
    (memory.copy (i32.add (local.get $out) (i32.const 18)) (local.get $ptr) (local.get $len))
    (i32.store8 (i32.add (i32.add (local.get $out) (i32.const 18)) (local.get $len)) (i32.const 125))
    (call $reply (local.get $out) (i32.add (local.get $len) (i32.const 19))))
)"#
    );
    wat::parse_str(text).expect("the test engine is valid text")
}

fn good() -> Vec<u8> {
    engine("", "")
}

fn small() -> HostLimits {
    HostLimits {
        fuel_per_call: 200_000,
        ..HostLimits::default()
    }
}

fn load(wasm: &[u8]) -> EngineHost {
    EngineHost::load(wasm, small()).expect("loads")
}

fn refused_load(wasm: &[u8]) -> String {
    match EngineHost::load(wasm, small()) {
        Ok(_) => panic!("a bad engine was accepted"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_good_engine_answers_and_the_request_has_the_documented_shape() {
    let mut host = load(&good());
    let got = host.call("echo", &json!({"a": [1, 2]})).unwrap();
    assert_eq!(got, json!({"api": 1, "data": {"a": [1, 2]}, "op": "echo"}));
    assert!(host.calls() >= 2); // hello at load, then echo
    assert!(host.fuel_used() > 0);
    assert_eq!(host.stopped(), None);
}

#[test]
fn a_refusal_is_ordinary_and_the_engine_keeps_running() {
    let mut host = load(&good());
    assert_eq!(
        host.call("refuse", &json!({})),
        Err(CallError::Refused("not now".into()))
    );
    assert!(host.call("echo", &json!({})).is_ok());
    assert_eq!(host.stopped(), None);
}

#[test]
fn an_endless_loop_ends_at_the_budget_and_stops_the_add_on() {
    let mut host = load(&good());
    let Err(CallError::Stopped(why)) = host.call("spin", &json!({})) else {
        panic!("an endless loop was not stopped");
    };
    assert!(why.contains("step budget"), "{why}");
    // stopped means stopped, for every later call, with the same reason
    assert_eq!(
        host.call("echo", &json!({})),
        Err(CallError::Stopped(why.clone()))
    );
    assert_eq!(host.stopped(), Some(why.as_str()));
}

#[test]
fn a_trap_stops_the_add_on_and_the_program_goes_on() {
    let mut host = load(&good());
    assert!(matches!(
        host.call("trap", &json!({})),
        Err(CallError::Stopped(_))
    ));
    assert!(host.stopped().is_some());
}

#[test]
fn deep_recursion_is_a_stop_not_a_crash() {
    let mut host = load(&good());
    let Err(CallError::Stopped(why)) = host.call("deep", &json!({})) else {
        panic!("not stopped");
    };
    assert!(!why.is_empty());
}

#[test]
fn broken_replies_stop_the_add_on() {
    for (op, word) in [
        ("ultralarge", "larger than"),
        ("outside", "outside its memory"),
        ("junk", "not valid JSON"),
        ("nook", "no `ok`"),
    ] {
        let mut host = load(&good());
        let Err(CallError::Stopped(why)) = host.call(op, &json!({})) else {
            panic!("{op} was not stopped");
        };
        assert!(why.contains(word), "{op}: {why}");
    }
}

#[test]
fn memory_is_capped() {
    let mut host = load(&good());
    // 100000 pages is 6 GB; the cap is 32 MiB, so the engine sees a failed grow, not a crash
    assert_eq!(
        host.call("grow", &json!({})).unwrap(),
        json!({"grew": false})
    );
    // a module that starts with more than the cap is refused at load
    let big = wat::parse_str(
        r#"(module (memory (export "memory") 4000)
        (func (export "kc_alloc") (param i32) (result i32) i32.const 0)
        (func (export "kc_call") (param i32 i32) (result i64) i64.const 0))"#,
    )
    .unwrap();
    assert!(refused_load(&big).contains("could not start"));
}

#[test]
fn log_text_is_kept_and_bounded() {
    let mut host = EngineHost::load(
        &good(),
        HostLimits {
            log_bytes: 40,
            ..small()
        },
    )
    .unwrap();
    for _ in 0..10 {
        host.call("write", &json!({})).unwrap();
    }
    let log = host.log();
    assert!(!log.is_empty() && log.iter().all(|l| l == "engine says hello"));
    assert!(log.iter().map(String::len).sum::<usize>() <= 40, "{log:?}");
}

#[test]
fn an_engine_that_asks_for_anything_else_is_refused_at_load() {
    for imports in [
        r#"(import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))"#,
        r#"(import "env" "now" (func (result i64)))"#,
        r#"(import "kopercad" "open_file" (func (param i32 i32)))"#,
    ] {
        let why = refused_load(&engine(imports, ""));
        assert!(why.contains("does not give"), "{why}");
    }
}

#[test]
fn things_that_are_not_binary_wasm_are_refused() {
    assert!(refused_load(b"(module)").contains("not a WebAssembly binary"));
    assert!(refused_load(b"").contains("not a WebAssembly binary"));
    assert!(refused_load(b"\0asm\x01\0\0\0garbage").contains("not a valid module"));
    // an engine without the exports
    let bare = wat::parse_str(r#"(module (memory (export "memory") 1))"#).unwrap();
    assert!(refused_load(&bare).contains("kc_alloc"));
}

#[test]
fn a_start_function_that_never_ends_cannot_hang_the_program() {
    let wasm = wat::parse_str(
        r#"(module (memory (export "memory") 1)
        (func $s (loop $l (br $l)))
        (start $s)
        (func (export "kc_alloc") (param i32) (result i32) i32.const 0)
        (func (export "kc_call") (param i32 i32) (result i64) i64.const 0))"#,
    )
    .unwrap();
    let why = refused_load(&wasm);
    assert!(why.contains("step budget"), "{why}");
}

#[test]
fn an_engine_for_another_host_api_is_refused() {
    let wasm = wat::parse_str(
        r#"(module (memory (export "memory") 1)
        (data (i32.const 0) "{\"ok\":true,\"data\":{\"api\":2}}")
        (func (export "kc_alloc") (param i32) (result i32) i32.const 4096)
        (func (export "kc_call") (param i32 i32) (result i64) i64.const 28))"#,
    )
    .unwrap();
    assert!(refused_load(&wasm).contains("another host API"));
}

#[test]
fn the_same_calls_cost_the_same_steps() {
    let run = || {
        let mut h = load(&good());
        for _ in 0..5 {
            h.call("echo", &json!({"x": 1})).unwrap();
        }
        h.fuel_used()
    };
    assert_eq!(run(), run());
}

#[test]
fn an_installed_add_on_starts_its_engine_through_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let wasm = good();
    let manifest = manifest(
        "demo",
        "1.0.0",
        "[engine]\nfile = \"engine.wasm\"\nhost_api = 1\n",
    );
    let mut parts = files("demo", "x");
    parts.push(("engine.wasm".into(), wasm));
    let bytes = novo_addons::pack::pack_parts(manifest.as_bytes(), &parts, None).expect("packs");
    store
        .install_bytes(
            &bytes,
            InstallOptions {
                accept_unsigned: true,
                enable: true,
            },
        )
        .unwrap();
    let set = store.scan();
    let mut host = set
        .load_engine("demo", small())
        .unwrap()
        .expect("has an engine");
    assert!(host.call("echo", &json!({})).is_ok());
    // an add-on without an engine says so
    let dir2 = tempfile::tempdir().unwrap();
    let plain = self::store(dir2.path());
    plain
        .install_bytes(
            &package("plain", "1.0.0", "x", false),
            InstallOptions {
                accept_unsigned: true,
                enable: true,
            },
        )
        .unwrap();
    assert!(
        plain
            .scan()
            .load_engine("plain", small())
            .unwrap()
            .is_none()
    );
    // a turned-off add-on cannot be started
    store.set_enabled("demo", false).unwrap();
    assert!(matches!(
        store.scan().load_engine("demo", small()),
        Err(AddonError::Unknown(_))
    ));
}

/// An engine whose op "z..." calls `kopercad.log` in an endless loop.
fn log_spammer() -> Vec<u8> {
    engine(
        "",
        r#"(if (i32.eq (local.get $c) (i32.const 122)) (then
      (loop $l (call $log (i32.const 384) (i32.const 17)) (br $l))))"#,
    )
}

#[test]
fn log_calls_cost_fuel_so_a_log_loop_stops_like_any_loop() {
    let mut host = EngineHost::load(
        &log_spammer(),
        HostLimits {
            log_bytes: 1 << 20,
            ..small()
        },
    )
    .unwrap();
    let err = host.call("zap", &json!({})).unwrap_err();
    assert!(
        matches!(&err, CallError::Stopped(why) if why.contains("step budget")),
        "{err:?}"
    );
    // 200 000 fuel at 2 000 per log call: at most 100 lines got in
    assert!(host.log().len() <= 100, "{} lines", host.log().len());
    assert!(host.stopped().is_some());
}

#[test]
fn a_log_loop_at_the_default_budget_stops_in_a_short_time() {
    let mut host = EngineHost::load(&log_spammer(), HostLimits::default()).unwrap();
    let started = std::time::Instant::now();
    assert!(host.call("zap", &json!({})).is_err());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn the_call_that_stops_counts_in_the_fuel_used() {
    let mut host = load(&good());
    let before = host.fuel_used();
    assert!(host.call("spin", &json!({})).is_err());
    assert!(
        host.fuel_used() - before >= small().fuel_per_call,
        "{} steps counted",
        host.fuel_used() - before
    );
}

#[test]
fn the_busiest_call_is_kept_and_put_into_words() {
    let mut h = load(&good());
    let after_hello = h.peak_fuel();
    assert!(after_hello > 0 && after_hello <= h.fuel_budget());
    h.call("echo", &json!({})).unwrap();
    assert!(h.peak_fuel() >= after_hello);
    assert_eq!(h.fuel_budget(), 200_000);
    // the words: quiet below half the budget, a warning from half on
    let (text, near) = novo_addons::budget_note(12, 10_000, 200_000);
    assert!(!near);
    assert_eq!(
        text,
        "Its busiest call used 5% of its step budget (12 calls so far)."
    );
    let (text, near) = novo_addons::budget_note(3, 100_000, 200_000);
    assert!(near);
    assert!(
        text.contains("50%") && text.contains("stops the add-on"),
        "{text}"
    );
    assert!(
        novo_addons::budget_note(1, 0, 0).1,
        "a zero budget is as full as it gets"
    );
}

#[test]
fn an_engine_over_four_mebibytes_is_refused_at_install_and_at_load() {
    let limit = novo_addons::host::MAX_ENGINE_BYTES;
    assert_eq!(limit, 4 * 1024 * 1024);
    // At load: a valid module padded with a custom section past the cap.
    let mut wasm = good();
    // Noise, so the package does not trip the expansion-ratio guard before the size cap.
    let mut x: u32 = 12345;
    while wasm.len() <= limit {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        wasm.push((x >> 24) as u8);
    }
    match EngineHost::load(&wasm, small()) {
        Err(AddonError::Engine(why)) => assert!(why.contains("larger than 4 MiB"), "{why}"),
        other => panic!("expected a refusal, got {:?}", other.map(|_| ())),
    }
    // At install: the package is refused before anything is written.
    let manifest = manifest(
        "big",
        "1.0.0",
        "[engine]\nfile = \"engine.wasm\"\nhost_api = 1\n",
    );
    let mut parts = files("big", "x");
    parts.push(("engine.wasm".into(), wasm));
    let bytes = novo_addons::pack::pack_parts(manifest.as_bytes(), &parts, None).expect("packs");
    let dir = tempfile::tempdir().unwrap();
    let err = store(dir.path())
        .install_bytes(
            &bytes,
            InstallOptions {
                accept_unsigned: true,
                enable: true,
            },
        )
        .unwrap_err();
    assert!(err.to_string().contains("too large"), "{err}");
}
