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

/// The whole path a user takes with a real-style file: BMEcat 1.2, ETIM
/// codes, a price per 100, markup in a description. The ETIM codes are made
/// up; the release tables are in the layout the reader documents.
#[test]
fn etim_release_install_then_a_bmecat_1_2_file_with_codes_imports_with_price_and_size() {
    let dir = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("inbox");
    let cat = dir.path().join("parts.json");
    let release = dir.path().join("release");
    std::fs::create_dir_all(&release).unwrap();
    std::fs::write(
        release.join("features.csv"),
        "Code;Desc EN\nEF900001;Width\nEF900002;Height\nEF900003;Depth\n",
    )
    .unwrap();
    std::fs::write(
        release.join("units.csv"),
        "Code;Desc EN;Abbreviation\nEU900001;Millimetre;mm\nEU900002;Ampere;A\n",
    )
    .unwrap();
    let (inbox_s, cat_s, rel_s) = (
        inbox.to_str().unwrap(),
        cat.to_str().unwrap(),
        release.to_str().unwrap(),
    );

    let (code, out) = novo(&["parts", "etim", "--catalog", cat_s]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("No ETIM release is installed"), "{out}");

    novo(&["parts", "prepare", "--inbox", inbox_s]);
    let file = r#"<?xml version="1.0"?>
<BMECAT version="1.2"><HEADER><CATALOG><CURRENCY>EUR</CURRENCY></CATALOG>
<SUPPLIER><SUPPLIER_NAME>WAGO</SUPPLIER_NAME></SUPPLIER></HEADER><T_NEW_CATALOG>
<ARTICLE><SUPPLIER_AID>2002-1201</SUPPLIER_AID>
<ARTICLE_DETAILS><DESCRIPTION_SHORT>Terminal <b>2.5</b> &amp; marker</DESCRIPTION_SHORT></ARTICLE_DETAILS>
<ARTICLE_FEATURES>
<FEATURE><FNAME>EF900001</FNAME><FVALUE>5.2</FVALUE><FUNIT>EU900001</FUNIT></FEATURE>
<FEATURE><FNAME>EF900002</FNAME><FVALUE>62</FVALUE><FUNIT>EU900001</FUNIT></FEATURE>
<FEATURE><FNAME>EF900003</FNAME><FVALUE>36</FVALUE><FUNIT>EU900001</FUNIT></FEATURE>
</ARTICLE_FEATURES>
<ARTICLE_ORDER_DETAILS><ORDER_UNIT>C62</ORDER_UNIT><PRICE_QUANTITY>100</PRICE_QUANTITY></ARTICLE_ORDER_DETAILS>
<ARTICLE_PRICE_DETAILS><ARTICLE_PRICE price_type="net_list"><PRICE_AMOUNT>42.00</PRICE_AMOUNT></ARTICLE_PRICE></ARTICLE_PRICE_DETAILS>
</ARTICLE></T_NEW_CATALOG></BMECAT>"#;
    std::fs::write(inbox.join("wago/wago-1.2.xml"), file).unwrap();

    // Before the release is installed: honest about what was not read.
    let (code, out) = novo(&[
        "parts",
        "import",
        "--dry-run",
        "--inbox",
        inbox_s,
        "--catalog",
        cat_s,
    ]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("no ETIM release is installed"), "{out}");
    assert!(!cat.exists(), "a dry run writes nothing");

    let (code, out) = novo(&["parts", "etim", rel_s, "--catalog", cat_s]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("3 features"), "{out}");
    let (_, shown) = novo(&["parts", "etim", "--catalog", cat_s]);
    assert!(shown.contains("Installed: release"), "{shown}");

    let (code, out) = novo(&["parts", "import", "--inbox", inbox_s, "--catalog", cat_s]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(!out.contains("ETIM code"), "{out}");
    let saved = std::fs::read_to_string(&cat).unwrap();
    assert!(saved.contains("Terminal 2.5 & marker"), "{saved}");
    assert!(saved.contains("\"per\": 100"), "{saved}");
    assert!(saved.contains("\"width_mm\""), "{saved}");
}
