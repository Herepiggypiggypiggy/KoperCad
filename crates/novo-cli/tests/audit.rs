//! `kopercad-cli audit`: sheet links, symbol lint, nets and search.

use std::path::Path;

use novo_cli::exit;
use novo_py::{Role, ScriptProject};

fn novo(args: &[&str]) -> (i32, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = novo_cli::run(
        std::iter::once("kopercad-cli").chain(args.iter().copied()),
        &mut out,
        &mut err,
    );
    (code, String::from_utf8(out).unwrap())
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// Pages 1 and 2; link `1` on both, link `7` only on page 1, one unnamed.
fn linked(dir: &Path, partner: bool) -> std::path::PathBuf {
    let path = dir.join("links.novo");
    let mut p = ScriptProject::create(&path, "Links", false).unwrap();
    let a = p.add_page("1", "").unwrap();
    let b = p.add_page("2", "").unwrap();
    p.place(a, "ip", 0.0, 0.0, 0, false, Role::Interruption("1".into()))
        .unwrap();
    if partner {
        p.place(b, "ip", 40.0, 0.0, 0, false, Role::Interruption("1".into()))
            .unwrap();
    }
    p.place(a, "ip", 80.0, 0.0, 0, false, Role::Interruption("7".into()))
        .unwrap();
    p.save().unwrap();
    p.close().unwrap();
    path
}

#[test]
fn links_find_the_ones_with_no_partner() {
    let dir = tempfile::tempdir().unwrap();
    let file = linked(dir.path(), true);
    let (code, out) = novo(&["audit", "links", s(&file)]);
    assert_eq!(code, exit::PROBLEMS, "{out}");
    assert!(out.contains("FAIL link 7"), "{out}");
    assert!(out.contains("no partner"), "{out}");
    assert!(out.contains("OK   link 1 at 1."), "{out}");
    assert!(out.contains("-> 2."), "the label names the partner: {out}");
    assert!(
        out.contains("2 failed") || out.contains("1 failed"),
        "{out}"
    );

    let (_, json) = novo(&["audit", "links", "--json", s(&file)]);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["fail"], 1);
    assert_eq!(v["checked"], 3);
}

#[test]
fn links_all_matched_is_clean() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ok.novo");
    let mut p = ScriptProject::create(&path, "Ok", false).unwrap();
    let a = p.add_page("1", "").unwrap();
    let b = p.add_page("2", "").unwrap();
    for page in [a, b] {
        p.place(
            page,
            "ip",
            0.0,
            0.0,
            0,
            false,
            Role::Interruption("1".into()),
        )
        .unwrap();
    }
    p.save().unwrap();
    p.close().unwrap();
    let (code, out) = novo(&["audit", "links", s(&path)]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("2 sheet links, 0 failed"), "{out}");
}

#[test]
fn links_on_one_page_warn_and_strict_fails() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("same.novo");
    let mut p = ScriptProject::create(&path, "Same", false).unwrap();
    let a = p.add_page("1", "").unwrap();
    for x in [0.0, 40.0] {
        p.place(a, "ip", x, 0.0, 0, false, Role::Interruption("1".into()))
            .unwrap();
    }
    p.save().unwrap();
    p.close().unwrap();
    let (code, out) = novo(&["audit", "links", s(&path)]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("WARN"), "{out}");
    let (code, _) = novo(&["audit", "links", "--strict", s(&path)]);
    assert_eq!(code, exit::PROBLEMS);
}

#[test]
fn symbol_lint_reads_the_built_in_packs_clean() {
    let packs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../novo-symbols/symbols");
    let (code, out) = novo(&["audit", "symbols", s(&packs)]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("0 failed"), "{out}");
}

#[test]
fn symbol_lint_names_what_is_wrong_in_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("pack");
    let cat = pack.join("mine").join("parts");
    std::fs::create_dir_all(&cat).unwrap();
    // Valid symbol: copy a built-in one under a new id.
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../novo-symbols/symbols/iec/connectivity/interruption_point.toml");
    let text = std::fs::read_to_string(source).unwrap();
    let good = text
        .replace("iec/connectivity/interruption_point", "mine/parts/good")
        .replace("category = \"connectivity\"", "category = \"parts\"")
        .replace("aliases = [\"ip\"]", "aliases = [\"mygood\"]");
    std::fs::write(cat.join("good.toml"), &good).unwrap();
    let (code, out) = novo(&["audit", "symbols", s(&pack)]);
    assert_eq!(code, exit::OK, "{out}");

    std::fs::write(cat.join("broken.toml"), "this is = not [valid").unwrap();
    std::fs::write(cat.join("wrongid.toml"), &good).unwrap();
    let (code, out) = novo(&["audit", "symbols", s(&pack)]);
    assert_eq!(code, exit::PROBLEMS, "{out}");
    assert!(out.contains("FAIL"), "{out}");
    assert!(out.contains("not a valid symbol file"), "{out}");
    assert!(
        out.contains("must equal its path `mine/parts/wrongid`"),
        "{out}"
    );
    assert!(
        out.contains("is also used by") || out.contains("more than once"),
        "{out}"
    );
    let (code, _) = novo(&["audit", "symbols", s(&dir.path().join("missing"))]);
    assert_eq!(code, exit::READ);
}

#[test]
fn nets_and_search_list_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("n.novo");
    let mut p = ScriptProject::create(&path, "N", false).unwrap();
    let page = p.add_page("1", "Main circuit").unwrap();
    let k1 = p.add_device("-K1", "Contactor", "general").unwrap();
    let coil = p
        .place(
            page,
            "coil",
            0.0,
            0.0,
            0,
            false,
            Role::Device {
                device: k1,
                main: true,
            },
        )
        .unwrap();
    let no = p
        .place(page, "no", 0.0, 40.0, 0, false, Role::Unassigned)
        .unwrap();
    p.connect((coil, "A2"), (no, "13")).unwrap();
    p.save().unwrap();
    p.close().unwrap();
    let (code, out) = novo(&["audit", "nets", s(&path)]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("nets,"), "{out}");
    let (code, out) = novo(&["audit", "search", s(&path), "k1"]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("-K1"), "{out}");
    let (code, _) = novo(&["audit", "search", s(&path), "zzz"]);
    assert_eq!(code, exit::PROBLEMS);
}
