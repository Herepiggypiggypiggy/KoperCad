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

#[test]
fn a_library_is_exported_to_a_bundle_and_opened_on_another_profile() {
    use novo_parts::{
        Catalog, CatalogPart, ImportFlow, LocalLibrary, NewSource, PartKey, Provenance,
    };
    let dir = tempfile::tempdir().unwrap();
    let (cat_a, lib_a) = (dir.path().join("a/parts.json"), dir.path().join("a/lib"));
    let mut cat = Catalog::new();
    let mut own = CatalogPart::new("Workshop", "W-1");
    own.description = "My bracket".into();
    own.provenance = Some(Provenance {
        source: "authored".into(),
        ..Default::default()
    });
    let mut theirs = CatalogPart::new("Schneider Electric", "LC1D09BD");
    theirs.description = "Maker description".into();
    theirs.provenance = Some(Provenance {
        source: "import:bmecat".into(),
        ..Default::default()
    });
    let lib = LocalLibrary::at(&lib_a);
    for (p, bytes) in [(&own, &b"my step"[..]), (&theirs, &b"maker step"[..])] {
        lib.store(
            &NewSource {
                manufacturer: &p.manufacturer,
                order_number: &p.order_number,
                file_name: "m.stp",
                url: None,
                flow: ImportFlow::Manual,
                imported: "2026-10-28",
                terms_url: None,
            },
            bytes,
        )
        .unwrap();
    }
    cat.upsert(own);
    cat.upsert(theirs);
    cat.save(&cat_a).unwrap();
    let (cat_a_s, lib_a_s) = (cat_a.to_str().unwrap(), lib_a.to_str().unwrap());

    // Shared: only the part the user wrote, no files without a confirmation.
    let share = dir.path().join("share.kcpl");
    let share_s = share.to_str().unwrap();
    let args = |extra: &[&'static str]| -> Vec<&str> {
        let mut v = vec![
            "parts",
            "export-library",
            share_s,
            "--catalog",
            cat_a_s,
            "--library",
            lib_a_s,
        ];
        v.extend_from_slice(extra);
        v
    };
    let (code, out) = novo(&args(&["--files"]));
    assert_eq!(code, exit::WRITE, "{out}");
    assert!(!share.exists());
    let (code, out) = novo(&args(&[]));
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("1 parts, 0 files"), "{out}");
    assert!(
        out.contains("1 parts from makers' catalogs are listed"),
        "{out}"
    );

    let (cat_b, lib_b) = (dir.path().join("b/parts.json"), dir.path().join("b/lib"));
    let (cat_b_s, lib_b_s) = (cat_b.to_str().unwrap(), lib_b.to_str().unwrap());
    let (code, out) = novo(&[
        "parts",
        "import-library",
        share_s,
        "--catalog",
        cat_b_s,
        "--library",
        lib_b_s,
    ]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("Added 1"), "{out}");
    let got = Catalog::load(&cat_b).unwrap();
    assert!(got.get(&PartKey::new("Workshop", "W-1")).is_some());
    assert!(
        got.get(&PartKey::new("Schneider Electric", "LC1D09BD"))
            .is_none()
    );

    // Backup: everything, and the files need the confirmation.
    let backup = dir.path().join("backup.kcpl");
    let backup_s = backup.to_str().unwrap();
    let (code, out) = novo(&[
        "parts",
        "export-library",
        backup_s,
        "--backup",
        "--catalog",
        cat_a_s,
        "--library",
        lib_a_s,
    ]);
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("2 parts, 2 files"), "{out}");
    let (cat_c, lib_c) = (dir.path().join("c/parts.json"), dir.path().join("c/lib"));
    let (cat_c_s, lib_c_s) = (cat_c.to_str().unwrap(), lib_c.to_str().unwrap());
    let base = [
        "parts",
        "import-library",
        backup_s,
        "--catalog",
        cat_c_s,
        "--library",
        lib_c_s,
    ];
    let (code, out) = novo(&[&base[..], &["--dry-run"]].concat());
    assert_eq!(code, exit::OK, "{out}");
    assert!(out.contains("2 new parts"), "{out}");
    assert!(!cat_c.exists(), "a dry run changes nothing");
    let (code, out) = novo(&base);
    assert_eq!(code, exit::PROBLEMS, "{out}");
    assert!(!cat_c.exists(), "nothing changed without the confirmation");
    let (code, out) = novo(&[&base[..], &["--confirm-own-backup"]].concat());
    assert_eq!(code, exit::OK, "{out}");
    assert!(
        out.contains("Added 2") && out.contains("2 files stored"),
        "{out}"
    );
    assert_eq!(Catalog::load(&cat_c).unwrap().len(), 2);
}
