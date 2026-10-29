//! The controller add-on (lane C3): the real `engine.wasm` built from `addons/controllers`,
//! packed by `scripts/build-controllers-addon.sh`, installed into a store and driven through the
//! add-on host, as the program does it.
//!
//! The package is built by CI (job `controllers-addon`) and found in `dist/`, or at the path in
//! `KOPERCAD_CONTROLLERS_PACK`. Without a package the tests pass with a note, unless
//! `KOPERCAD_REQUIRE_CONTROLLERS_PACK` is set (CI sets it), so `cargo test --workspace` does not
//! need the wasm32 target.

use std::path::{Path, PathBuf};

use novo_addons::{AppInfo, HostLimits, InstallOptions, Store, Version};
use serde_json::{Value, json};

fn pack_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("KOPERCAD_CONTROLLERS_PACK") {
        return Some(PathBuf::from(p));
    }
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist");
    std::fs::read_dir(dist)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("controllers-") && n.ends_with(".kcaddon"))
        })
}

/// The installed add-on's engine, or `None` (with a note) when no package was built.
fn engine(dir: &Path) -> Option<novo_addons::EngineHost> {
    let Some(path) = pack_path() else {
        assert!(
            std::env::var_os("KOPERCAD_REQUIRE_CONTROLLERS_PACK").is_none(),
            "no controllers package: run scripts/build-controllers-addon.sh"
        );
        eprintln!("no controllers package built; skipping (scripts/build-controllers-addon.sh)");
        return None;
    };
    let store = Store::new(
        dir.join("addons"),
        AppInfo {
            version: Version::new(0, 1, 0),
            api_version: 1,
        },
    );
    store
        .install_bytes(
            &std::fs::read(path).expect("package readable"),
            InstallOptions {
                accept_unsigned: true,
                enable: true,
            },
        )
        .expect("the controllers package installs");
    let set = store.scan();
    Some(
        set.load_engine("controllers", HostLimits::default())
            .expect("the engine loads")
            .expect("the add-on has an engine"),
    )
}

fn button(host: &mut novo_addons::EngineHost, id: &str) -> Value {
    host.call("button", &json!({"panel": "radio", "id": id}))
        .unwrap_or_else(|e| panic!("button {id}: {e}"))
}

fn lit(reply: &Value, lamp: &str) -> bool {
    reply["lamps"][lamp]["lit"].as_bool().unwrap()
}

#[test]
fn the_package_has_the_contents_the_manifest_promises() {
    let Some(path) = pack_path() else {
        assert!(std::env::var_os("KOPERCAD_REQUIRE_CONTROLLERS_PACK").is_none());
        return;
    };
    let package = novo_addons::Package::read(
        &std::fs::read(path).unwrap(),
        &novo_addons::TrustStore::default(),
    )
    .expect("reads");
    let m = &package.manifest;
    assert_eq!(m.addon.id, "controllers");
    assert_eq!(m.commands.len(), 1);
    assert_eq!(m.contributions.len(), 1);
    let wasm = &package.files["engine.wasm"];
    assert!(
        wasm.len() <= novo_addons::host::MAX_ENGINE_BYTES,
        "engine.wasm is {} bytes",
        wasm.len()
    );
    eprintln!("engine.wasm: {} bytes", wasm.len());
}

#[test]
fn a_press_of_the_radio_moves_the_signals_through_the_sandbox() {
    let dir = tempfile::tempdir().unwrap();
    let Some(mut host) = engine(dir.path()) else {
        return;
    };
    let r = button(&mut host, "press-start");
    assert!(lit(&r, "main"));
    assert!(lit(&r, "estop-ok"));
    let r = button(&mut host, "set-hoist-up2");
    assert!(lit(&r, "h-up-slow") && lit(&r, "h-up-fast"));
    let r = button(&mut host, "press-estop");
    assert!(!lit(&r, "main") && !lit(&r, "h-up-fast") && !lit(&r, "estop-ok"));
    let r = button(&mut host, "reset-estop");
    assert!(
        lit(&r, "estop-ok") && !lit(&r, "main"),
        "main needs a new start"
    );
    // a refusal is words, and the engine stays on
    assert!(host.call("button", &json!({"id": "press-ghost"})).is_err());
    assert!(host.stopped().is_none());
}

#[test]
fn a_call_uses_a_small_share_of_the_step_budget() {
    let dir = tempfile::tempdir().unwrap();
    let Some(mut host) = engine(dir.path()) else {
        return;
    };
    let before = host.fuel_used();
    button(&mut host, "press-start");
    let spent = host.fuel_used() - before;
    let budget = HostLimits::default().fuel_per_call;
    eprintln!("one button press: {spent} of {budget} steps");
    assert!(
        spent * 20 < budget,
        "{spent} steps is over a twentieth of the budget"
    );
}

#[test]
fn a_replay_in_the_sandbox_is_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let Some(mut host) = engine(dir.path()) else {
        return;
    };
    host.call("record.start", &json!({})).unwrap();
    button(&mut host, "press-start");
    host.call("advance", &json!({"dt_ms": 300})).unwrap();
    button(&mut host, "set-bridge-fwd1");
    let seq = host
        .call("record.stop", &json!({"name": "bridge"}))
        .unwrap()["sequence"]
        .as_str()
        .unwrap()
        .to_owned();
    // A recording starts from the start state (a latch toggles on replay), so it plays on a
    // fresh engine; and a replay runs on a copy, so playing twice gives the same trace.
    let other_dir = tempfile::tempdir().unwrap();
    let mut host = engine(other_dir.path()).unwrap();
    let data = json!({"sequence": seq, "sample_ms": 50, "settle_ms": 100});
    let a = host.call("replay", &data).unwrap();
    let b = host.call("replay", &data).unwrap();
    assert_eq!(a, b);
    let last = a["samples"].as_array().unwrap().last().unwrap();
    assert_eq!(last["signals"]["radio/B_FWD_SLOW"], 1.0);
}
