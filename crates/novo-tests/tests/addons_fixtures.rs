//! The add-on fixtures of `qa/fixtures/addons`: the committed `.kcaddon` files are exactly what
//! the packer makes from their source folders, and the whole path a person or a script takes
//! (inspect, install, list, update, roll back, remove) works through the command-line tool.
//!
//! To rebuild the committed files after changing a source folder:
//! `KOPERCAD_REGENERATE=1 cargo test -p novo-tests --test addons_fixtures`.

use std::path::{Path, PathBuf};

use novo_addons::pack::pack_dir;

const FIXTURES: [(&str, &str); 4] = [
    ("demo-tools", "demo-tools-0.1.0.kcaddon"),
    ("demo-tools-2", "demo-tools-0.2.0.kcaddon"),
    ("future-addon", "future-addon-1.0.0.kcaddon"),
    ("demo-engine", "demo-engine-0.1.0.kcaddon"),
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../qa/fixtures/addons")
}

fn cli(args: &[&str]) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let argv = std::iter::once("kopercad-cli").chain(args.iter().copied());
    let code = novo_cli::run(argv.map(std::ffi::OsString::from), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn committed_packages_are_what_the_packer_makes() {
    let regenerate = std::env::var_os("KOPERCAD_REGENERATE").is_some();
    // The binary engine of `demo-engine` comes from its text source, before the packing.
    let wasm = ::wat::parse_file(root().join("engine-src/demo-engine.wat"))
        .expect("the demo engine source is valid");
    let engine_path = root().join("src/demo-engine/engine.wasm");
    if regenerate {
        std::fs::write(&engine_path, &wasm).unwrap();
    } else {
        assert!(
            std::fs::read(&engine_path).unwrap_or_default() == wasm,
            "engine.wasm is out of date with engine-src/demo-engine.wat; rebuild it with KOPERCAD_REGENERATE=1 cargo test -p novo-tests --test addons_fixtures"
        );
    }
    for (src, file) in FIXTURES {
        let made = pack_dir(&root().join("src").join(src), None).unwrap();
        let path = root().join(file);
        if regenerate {
            std::fs::write(&path, &made).unwrap();
            continue;
        }
        let have = std::fs::read(&path).unwrap_or_default();
        assert!(
            have == made,
            "{file} is out of date with qa/fixtures/addons/src/{src}; rebuild it with KOPERCAD_REGENERATE=1 cargo test -p novo-tests --test addons_fixtures"
        );
    }
}

#[test]
fn the_command_line_takes_an_add_on_through_its_whole_life() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path().join("addons");
    let r = r.to_str().unwrap();
    let v1 = root().join("demo-tools-0.1.0.kcaddon");
    let v2 = root().join("demo-tools-0.2.0.kcaddon");
    let v1 = v1.to_str().unwrap();
    let v2 = v2.to_str().unwrap();

    let (code, out, _) = cli(&["addons", "list", "--root", r]);
    assert_eq!(code, 0);
    assert!(out.contains("No add-ons installed"), "{out}");

    // inspect changes nothing and says what the file asks for
    let (code, out, _) = cli(&["addons", "inspect", v1, "--root", r]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Demo tools 0.1.0"), "{out}");
    assert!(out.contains("unsigned, from file"), "{out}");
    assert!(out.contains("menu, floating-panel, rail-item"), "{out}");
    assert!(!Path::new(r).exists());

    // the tick is needed
    let (code, _, err) = cli(&["addons", "install", v1, "--root", r]);
    assert_ne!(code, 0);
    assert!(err.contains("tick the box"), "{err}");

    let (code, out, _) = cli(&["addons", "install", v1, "--trust-unsigned", "--root", r]);
    assert_eq!(code, 0, "{out}");
    let (_, out, _) = cli(&["addons", "list", "--root", r]);
    assert!(out.contains("demo-tools\t0.1.0\tDemo tools\ton"), "{out}");

    // a newer program is needed for this one: refused, nothing written
    let future = root().join("future-addon-1.0.0.kcaddon");
    let (code, _, err) = cli(&[
        "addons",
        "install",
        future.to_str().unwrap(),
        "--trust-unsigned",
        "--root",
        r,
    ]);
    assert_ne!(code, 0);
    assert!(err.contains("needs KoperCAD 9.0.0 or newer"), "{err}");

    // JSON for scripts
    let (_, out, _) = cli(&["addons", "list", "--json", "--root", r]);
    let json: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(json["addons"][0]["status"], "enabled");
    assert_eq!(json["addons"][0]["commands"].as_array().unwrap().len(), 3);

    // update, roll back, off, remove
    let (code, _, _) = cli(&["addons", "install", v2, "--trust-unsigned", "--root", r]);
    assert_eq!(code, 0);
    let (_, out, _) = cli(&["addons", "list", "--root", r]);
    assert!(out.contains("0.2.0"), "{out}");
    let (code, out, _) = cli(&["addons", "rollback", "demo-tools", "--root", r]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("0.1.0"));
    let (code, _, _) = cli(&["addons", "disable", "demo-tools", "--root", r]);
    assert_eq!(code, 0);
    let (_, out, _) = cli(&["addons", "list", "--root", r]);
    assert!(out.contains("\toff"), "{out}");
    let (code, _, _) = cli(&["addons", "remove", "demo-tools", "--root", r]);
    assert_eq!(code, 0);
    let (_, out, _) = cli(&["addons", "list", "--root", r]);
    assert!(out.contains("No add-ons installed"), "{out}");
}

#[test]
fn pack_builds_the_same_file_and_signs_when_asked() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("demo.kcaddon");
    let src = root().join("src/demo-tools");
    let (code, text, _) = cli(&[
        "addons",
        "pack",
        src.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{text}");
    assert_eq!(
        std::fs::read(&out).unwrap(),
        std::fs::read(root().join("demo-tools-0.1.0.kcaddon")).unwrap()
    );

    // signed with a key file; a store that trusts the key installs it with no tick
    let seed = dir.path().join("seed.txt");
    std::fs::write(&seed, "07".repeat(32)).unwrap();
    let signed = dir.path().join("signed.kcaddon");
    let (code, text, _) = cli(&[
        "addons",
        "pack",
        src.to_str().unwrap(),
        "-o",
        signed.to_str().unwrap(),
        "--key-id",
        "test-key",
        "--seed-file",
        seed.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{text}");
    let sk = ed25519_public_hex();
    let store_root = dir.path().join("addons");
    std::fs::create_dir_all(&store_root).unwrap();
    std::fs::write(
        store_root.join("trusted-keys.toml"),
        format!("[[key]]\nid = \"test-key\"\nowner = \"Test\"\npublic = \"{sk}\"\n"),
    )
    .unwrap();
    let (code, text, err) = cli(&[
        "addons",
        "install",
        signed.to_str().unwrap(),
        "--root",
        store_root.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{text}{err}");
    let (_, text, _) = cli(&[
        "addons",
        "list",
        "--json",
        "--root",
        store_root.to_str().unwrap(),
    ]);
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["addons"][0]["trust"]["signed"], true);
    assert_eq!(json["addons"][0]["trust"]["owner"], "Test");
}

/// The public key for the seed `07` x 32, derived with the crate's own verifier dependency.
fn ed25519_public_hex() -> String {
    use novo_addons::trust::to_hex;
    // Sign a message with the seed through the packer's path and read the key off the
    // package: simplest is to derive it through ed25519-dalek via novo-addons' dev helper.
    to_hex(&novo_addons::trust::public_key_for_seed(&[7u8; 32]))
}

#[test]
fn the_demo_engine_runs_from_its_file_and_a_broken_one_stops_alone() {
    use novo_addons::{AppInfo, CallError, HostLimits, InstallOptions, Store, Version};
    use serde_json::json;

    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(
        dir.path().join("addons"),
        AppInfo {
            version: Version::new(0, 1, 0),
            api_version: 1,
        },
    );
    store
        .install_file(
            &root().join("demo-engine-0.1.0.kcaddon"),
            InstallOptions {
                accept_unsigned: true,
                enable: true,
            },
        )
        .unwrap();
    let set = store.scan();
    let mut host = set
        .load_engine("demo-engine", HostLimits::default())
        .unwrap()
        .expect("the add-on has an engine");
    let started = host
        .call("button", &json!({"panel": "panel", "id": "start"}))
        .unwrap();
    assert_eq!(
        started["lamps"]["power"],
        json!({"state": "ok", "lit": true})
    );
    let stopped = host
        .call("button", &json!({"panel": "panel", "id": "stop"}))
        .unwrap();
    assert_eq!(stopped["lamps"]["power"]["lit"], json!(false));
    let pong = host.call("command", &json!({"id": "ping"})).unwrap();
    assert_eq!(pong["notice"], json!("Pong from the demo engine"));
    assert!(matches!(
        host.call("command", &json!({"id": "crash"})),
        Err(CallError::Stopped(_))
    ));
    assert!(host.stopped().is_some());
}
