//! `kopercad-cli macros`, driven through `novo_cli::run`.

use novo_cli::exit;

fn novo(args: &[&str]) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = novo_cli::run(
        std::iter::once("kopercad-cli").chain(args.iter().copied()),
        &mut out,
        &mut err,
    );
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn starters_list_export_and_import_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    let (a, b) = (a.to_str().unwrap(), b.to_str().unwrap());

    let (code, out, _) = novo(&["macros", "install-starters", "--library", a]);
    assert_eq!(code, exit::OK);
    assert!(out.contains("added Starter/"), "{out}");

    // Second run overwrites nothing.
    let (_, again, _) = novo(&["macros", "install-starters", "--library", a]);
    assert!(
        again.contains("skipped") && !again.contains("added"),
        "{again}"
    );

    let (code, listed, _) = novo(&["macros", "list", "--library", a, "--json"]);
    assert_eq!(code, exit::OK);
    let rows: Vec<serde_json::Value> = serde_json::from_str(&listed).unwrap();
    assert!(rows.len() >= 2);
    let first = rows[0]["path"].as_str().unwrap().to_owned();
    let second = rows[1]["path"].as_str().unwrap().to_owned();

    // One macro to a file, then into the other library.
    let file = dir.path().join("one.kcmacro");
    let file = file.to_str().unwrap();
    let (code, _, _) = novo(&["macros", "export", &first, "--library", a, "-o", file]);
    assert_eq!(code, exit::OK);
    let (code, out, _) = novo(&["macros", "import", file, "--library", b]);
    assert_eq!(code, exit::OK);
    assert!(out.starts_with("added "), "{out}");
    // Importing again keeps both; --skip leaves it; --replace replaces.
    let (_, out, _) = novo(&["macros", "import", file, "--library", b]);
    assert!(out.starts_with("added "), "{out}");
    let (_, out, _) = novo(&["macros", "import", file, "--library", b, "--skip"]);
    assert!(out.starts_with("skipped "), "{out}");
    let (_, out, _) = novo(&["macros", "import", file, "--library", b, "--replace"]);
    assert!(out.starts_with("replaced "), "{out}");

    // Two macros make a pack, which imports as two.
    let pack = dir.path().join("set.kcmacropack");
    let pack = pack.to_str().unwrap();
    let (code, _, _) = novo(&[
        "macros",
        "export",
        &first,
        &second,
        "--library",
        a,
        "-o",
        pack,
    ]);
    assert_eq!(code, exit::OK);
    let c = dir.path().join("c");
    let c = c.to_str().unwrap();
    let (_, out, _) = novo(&["macros", "import", pack, "--library", c]);
    assert_eq!(out.lines().count(), 2, "{out}");
}

#[test]
fn a_missing_macro_or_file_fails_with_a_message() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("lib");
    let lib = lib.to_str().unwrap();
    let out = dir.path().join("x.kcmacro");
    let (code, _, err) = novo(&[
        "macros",
        "export",
        "Nope.kcmacro",
        "--library",
        lib,
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, exit::WRITE);
    assert!(err.contains("Nope"), "{err}");
    let (code, _, _) = novo(&["macros", "import", "/no/such.kcmacro", "--library", lib]);
    assert_eq!(code, exit::READ);
    let (code, out, _) = novo(&["macros", "list", "--library", lib]);
    assert_eq!(code, exit::OK);
    assert!(out.contains("No macros."));
}
