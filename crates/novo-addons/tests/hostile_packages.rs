//! Hostile and broken `.kcaddon` files: every one must be refused with a plain message and
//! nothing may be written to disk.

mod common;

use common::*;
use novo_addons::package::Limits;
use novo_addons::{AddonError, Package};

const SIMPLE: &str = r#"[addon]
id = "evil"
name = "Evil"
version = "1.0.0"
publisher = "Nobody"
api_version = 1
permissions = ["menu"]

[[command]]
id = "x"
label = "X"
home = "palette-only"
"#;
// no engine and no panel: a command must open a panel or have an engine, so give it one
const WITH_ENGINE: &str = r#"[addon]
id = "evil"
name = "Evil"
version = "1.0.0"
publisher = "Nobody"
api_version = 1

[engine]
file = "engine.wasm"
host_api = 1
"#;

fn read(bytes: &[u8]) -> Result<Package, AddonError> {
    Package::read(bytes, &empty_trust())
}

fn refused(bytes: &[u8]) -> String {
    match read(bytes) {
        Ok(_) => panic!("a hostile package was accepted"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn not_a_zip_and_truncated() {
    assert!(refused(b"hello world").contains("not a readable add-on"));
    assert!(refused(b"").contains("not a readable add-on"));
    let good = package("demo", "1.0.0", "x", false);
    assert!(refused(&good[..good.len() / 2]).contains("not a readable add-on"));
}

#[test]
fn no_manifest() {
    assert!(refused(&raw_zip(&[("data/a.toml", b"x")])).contains("no manifest.toml"));
}

#[test]
fn path_traversal_and_friends_are_refused_by_name() {
    for name in [
        "../evil.txt",
        "data/../../evil.txt",
        "/etc/passwd",
        "C:/Windows/evil.dll",
        "data\\..\\evil",
        "data/con.txt",
        "data/aux",
        "data/file.txt:stream",
        "data/ends-with-dot.",
        "data//double",
    ] {
        let zip = raw_zip(&[("manifest.toml", WITH_ENGINE.as_bytes()), (name, b"x")]);
        let why = refused(&zip);
        assert!(why.contains("unsafe file name"), "{name}: {why}");
    }
}

#[test]
fn a_symbolic_link_entry_is_refused() {
    let mut out = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        zip.start_file("manifest.toml", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, WITH_ENGINE.as_bytes()).unwrap();
        zip.add_symlink(
            "data/link",
            "../../outside",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.finish().unwrap();
    }
    assert!(refused(&out).contains("symbolic link"));
}

#[test]
fn names_that_differ_only_in_case_are_refused() {
    let zip = raw_zip(&[
        ("manifest.toml", WITH_ENGINE.as_bytes()),
        ("data/A.toml", b"1"),
        ("data/a.toml", b"2"),
    ]);
    assert!(refused(&zip).contains("twice"));
}

#[test]
fn a_file_the_manifest_does_not_list_is_refused() {
    let zip = raw_package(WITH_ENGINE, &[("engine.wasm", b"\0asm")]);
    // add a stray file
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut a = zip::ZipArchive::new(std::io::Cursor::new(&zip)).unwrap();
    for i in 0..a.len() {
        let mut f = a.by_index(i).unwrap();
        let mut b = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut b).unwrap();
        entries.push((f.name().unwrap().into_owned(), b));
    }
    entries.push(("data/extra.dll".into(), b"MZ".to_vec()));
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    assert!(refused(&raw_zip(&refs)).contains("not listed in the manifest"));
}

#[test]
fn a_listed_file_that_is_missing_is_refused() {
    let mut text = WITH_ENGINE.to_owned();
    text.push_str(&format!(
        "\n[files]\n\"engine.wasm\" = \"{}\"\n",
        novo_addons::trust::sha256_hex(b"x")
    ));
    let zip = raw_zip(&[("manifest.toml", text.as_bytes())]);
    assert!(refused(&zip).contains("missing from the package"));
}

#[test]
fn a_changed_file_is_refused() {
    let zip = raw_package(WITH_ENGINE, &[("engine.wasm", b"\0asm1")]);
    // rebuild with different content but the old manifest
    let mut a = zip::ZipArchive::new(std::io::Cursor::new(&zip)).unwrap();
    let mut manifest = Vec::new();
    std::io::Read::read_to_end(&mut a.by_name("manifest.toml").unwrap(), &mut manifest).unwrap();
    let tampered = raw_zip(&[("manifest.toml", &manifest), ("engine.wasm", b"\0asm2")]);
    assert!(refused(&tampered).contains("engine.wasm has been changed"));
}

#[test]
fn an_oversized_manifest_is_refused() {
    let mut big = WITH_ENGINE.to_owned();
    big.push_str(&"# padding\n".repeat(40_000));
    let zip = raw_zip(&[("manifest.toml", big.as_bytes())]);
    let why = refused(&zip);
    assert!(
        why.contains("too large") || why.contains("256 KiB"),
        "{why}"
    );
}

#[test]
fn a_zip_bomb_that_lies_about_its_size_is_stopped_while_reading() {
    // 40 MiB of zeros compresses to ~40 KiB: a ratio no real file has.
    let zeros = vec![0u8; 40 * 1024 * 1024];
    let zip = raw_package(WITH_ENGINE, &[("engine.wasm", &zeros)]);
    assert!((zip.len() as u64) < 1024 * 1024);
    let why = refused(&zip);
    assert!(why.contains("too large or expands"), "{why}");
}

#[test]
fn the_expansion_ratio_and_total_limits_hold_with_smaller_files() {
    let zeros = vec![0u8; 3 * 1024 * 1024];
    let zip = raw_package(WITH_ENGINE, &[("engine.wasm", &zeros)]);
    let limits = Limits {
        max_ratio: 50,
        ..Limits::DEFAULT
    };
    let err = Package::read_with(&zip, &empty_trust(), limits)
        .unwrap_err()
        .to_string();
    assert!(err.contains("fold"), "{err}");
    let limits = Limits {
        max_total_bytes: 1024 * 1024,
        max_ratio: u64::MAX,
        ..Limits::DEFAULT
    };
    let err = Package::read_with(&zip, &empty_trust(), limits)
        .unwrap_err()
        .to_string();
    assert!(err.contains("add up to more"), "{err}");
    let limits = Limits {
        max_file_bytes: 1024 * 1024,
        ..Limits::DEFAULT
    };
    assert!(Package::read_with(&zip, &empty_trust(), limits).is_err());
}

#[test]
fn too_many_entries_and_a_too_large_file() {
    let names: Vec<String> = (0..30).map(|i| format!("data/f{i}.toml")).collect();
    let entries: Vec<(&str, &[u8])> = names
        .iter()
        .map(|n| (n.as_str(), b"x".as_slice()))
        .collect();
    let zip = raw_zip(&entries);
    let limits = Limits {
        max_entries: 10,
        ..Limits::DEFAULT
    };
    assert!(
        Package::read_with(&zip, &empty_trust(), limits)
            .unwrap_err()
            .to_string()
            .contains("entries")
    );
    let limits = Limits {
        max_package_bytes: 10,
        ..Limits::DEFAULT
    };
    assert!(
        Package::read_with(&zip, &empty_trust(), limits)
            .unwrap_err()
            .to_string()
            .contains("MiB")
    );
}

#[test]
fn rule_breaking_manifests_are_refused() {
    // a command that is in no menu and has nothing to run
    assert!(refused(&raw_package(SIMPLE, &[])).contains("no engine"));
    // wrong API generation
    let m = WITH_ENGINE.replace("api_version = 1", "api_version = 9");
    assert!(refused(&raw_package(&m, &[("engine.wasm", b"x")])).contains("API 9"));
    // engine needing another host API
    let m = WITH_ENGINE.replace("host_api = 1", "host_api = 5");
    assert!(refused(&raw_package(&m, &[("engine.wasm", b"x")])).contains("host API 5"));
    // engine named like something else
    let m = WITH_ENGINE.replace("engine.wasm", "engine.dll");
    assert!(refused(&raw_package(&m, &[("engine.dll", b"x")])).contains(".wasm"));
    // hostile icon data
    let m = format!("{WITH_ENGINE}\n[[contribute]]\nkind = \"x\"\nfile = \"data/icons.toml\"\n");
    let icons = "[[icon]]\nname = \"evil.a\"\nshape = [{ kind = \"line\", x1 = 0, y1 = 0, x2 = 900, y2 = 0 }]\n";
    assert!(
        refused(&raw_package(
            &m,
            &[("engine.wasm", b"x"), ("data/icons.toml", icons.as_bytes())]
        ))
        .contains("grid")
    );
}

#[test]
fn a_broken_signature_is_refused_even_though_unsigned_would_pass() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = trusting_store(store_dir.path());
    let good = package("demo", "1.0.0", "x", true);
    // swap in a manifest with a different description but keep the signature
    let mut a = zip::ZipArchive::new(std::io::Cursor::new(&good)).unwrap();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..a.len() {
        let mut f = a.by_index(i).unwrap();
        let mut b = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut b).unwrap();
        entries.push((f.name().unwrap().into_owned(), b));
    }
    for (n, b) in &mut entries {
        if n == "manifest.toml" {
            let t = String::from_utf8(b.clone())
                .unwrap()
                .replace("A demo add-on.", "A changed add-on.");
            *b = t.into_bytes();
        }
    }
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    let tampered = raw_zip(&refs);
    let err = store
        .install_bytes(&tampered, Default::default())
        .unwrap_err();
    assert!(matches!(err, AddonError::Tampered(_)), "{err}");
    assert!(
        listing(&store_dir.path().join("addons"))
            .iter()
            .all(|f| f == "trusted-keys.toml")
    );
}

#[test]
fn a_panel_layout_with_a_colour_or_an_unknown_widget_is_refused() {
    const M: &str = r#"[addon]
id = "evil"
name = "Evil"
version = "1.0.0"
publisher = "Nobody"
api_version = 1
permissions = ["floating-panel"]

[[panel]]
id = "p"
title = "P"
slot = "floating"
file = "data/p.panel.toml"
"#;
    for bad in [
        "[[widget]]\nkind = \"label\"\ntext = \"hi\"\ncolor = \"#ff0000\"\n",
        "[[widget]]\nkind = \"webview\"\nurl = \"http://example.com\"\n",
    ] {
        let bytes = raw_package(M, &[("data/p.panel.toml", bad.as_bytes())]);
        let why = refused(&bytes);
        assert!(why.contains("p.panel.toml"), "{why}");
    }
}

#[test]
fn names_that_differ_only_in_unicode_case_are_refused() {
    let bytes = raw_zip(&[
        ("manifest.toml", WITH_ENGINE.as_bytes()),
        ("data/\u{c9}.toml", b"a"),
        ("data/\u{e9}.toml", b"b"),
    ]);
    // refused for the clash or for the name itself; never accepted
    assert!(read(&bytes).is_err());
}
