//! `kopercad-cli parts`, driven through `novo_cli::run`.

use novo_cli::exit;

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

#[test]
fn import_reads_the_inbox_and_an_empty_inbox_asks_for_files() {
    let dir = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("inbox");
    let cat = dir.path().join("parts.json");
    let (inbox_s, cat_s) = (inbox.to_str().unwrap(), cat.to_str().unwrap());

    let (code, out) = novo(&["parts", "import", "--inbox", inbox_s, "--catalog", cat_s]);
    assert_eq!(code, exit::PROBLEMS, "{out}");
    assert!(out.contains("No catalog files were found"), "{out}");
    assert!(inbox.join("wago").is_dir());

    std::fs::write(
        inbox.join("wago/p.csv"),
        "Order number;Description\n2002-1201;Feed-through terminal\n",
    )
    .unwrap();
    let (code, out) = novo(&["parts", "import", "--inbox", inbox_s, "--catalog", cat_s]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("WAGO"), "{out}");
    assert!(cat.exists());
    assert!(cat.parent().unwrap().join("import-report.txt").exists());

    let (_, guide) = novo(&["parts", "guide", "--inbox", inbox_s]);
    assert!(
        guide.contains("WAGO (files found; LOGIN NEEDED)"),
        "{guide}"
    );
}
