//! The one-click import: library, state machine, Downloads fallback.

use std::fs;
use std::time::{Duration, SystemTime};

use novo_parts::*;

const STEP: &str = "ISO-10303-21;\nDATA;\n#9=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n#1=CARTESIAN_POINT('',(0.,0.,0.));\n#2=CARTESIAN_POINT('',(5.2,42.,36.));\nENDSEC;\nEND-ISO-10303-21;\n";

fn key() -> PartKey {
    PartKey::new("Example Maker", "9000-0001")
}

fn new_source(file_name: &str) -> NewSource<'_> {
    NewSource {
        manufacturer: "Example Maker",
        order_number: "9000-0001",
        file_name,
        url: None,
        flow: ImportFlow::Manual,
        imported: "d",
        terms_url: None,
    }
}

fn sources(enabled: bool) -> SourceTable {
    SourceTable::new(vec![MakerSource {
        manufacturer: "Example Maker".into(),
        fetch_enabled: enabled,
        page_url: Some("https://example.invalid/p/{order}".into()),
        terms_url: Some("https://example.invalid/cloud-terms".into()),
        page_verified: true,
    }])
}

fn fetcher(result: ScriptedAnswer) -> ScriptedFetcher {
    let mut f = ScriptedFetcher::default();
    f.files.insert(key(), result);
    f
}

fn step_file() -> (RemoteFile, Vec<u8>) {
    (
        RemoteFile {
            kind: FileKind::Step,
            url: "https://example.invalid/f/9000-0001.stp".into(),
            file_name: "9000-0001.stp".into(),
        },
        STEP.as_bytes().to_vec(),
    )
}

#[test]
fn one_click_fetches_stores_and_reads_the_size() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    let src = sources(true);
    let f = fetcher(Ok(vec![step_file()]));
    let mut job = ImportJob::new(&lib, &src, &f, "Example Maker", "9000-0001", "2026-10-15");

    // The first Import for a maker asks for the notice.
    job.run();
    assert_eq!(
        job.state(),
        &ImportState::NeedsNotice {
            manufacturer: "Example Maker".into()
        }
    );
    job.accept_notice().unwrap();
    assert!(lib.notice_accepted("example maker"));
    let ImportState::Done(out) = job.run().clone() else {
        panic!("expected Done, got {:?}", job.state())
    };

    assert_eq!(out.records.len(), 1);
    let r = &out.records[0];
    assert_eq!(r.flow, ImportFlow::Fetched);
    assert_eq!(
        r.url.as_deref(),
        Some("https://example.invalid/f/9000-0001.stp")
    );
    assert_eq!(r.sha256.len(), 64);
    assert_eq!(fs::read(lib.path_of(r)).unwrap(), STEP.as_bytes());
    let d = out.dimensions.unwrap();
    assert_eq!((d.depth_mm, d.width_mm, d.height_mm), (5.2, 36.0, 42.0));
    assert!(lib.has(&key()));

    // The notice is asked once: a second part goes straight through.
    let mut again = ImportJob::new(&lib, &src, &f, "Example Maker", "9000-0001", "2026-10-16");
    assert!(matches!(again.run(), ImportState::Done(_)));
    assert_eq!(
        lib.records().unwrap().len(),
        1,
        "same file replaces the record"
    );
}

#[test]
fn falls_back_when_not_enabled_or_the_source_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    lib.accept_notice("Example Maker").unwrap();

    let f = fetcher(Ok(vec![step_file()]));
    let off = sources(false);
    let mut job = ImportJob::new(&lib, &off, &f, "Example Maker", "9000-0001", "d");
    assert_eq!(
        job.run(),
        &ImportState::Fallback {
            reason: FallbackReason::NotEnabled,
            page_url: Some("https://example.invalid/p/9000-0001".into())
        }
    );
    assert!(!lib.has(&key()), "nothing was fetched");

    let on = sources(true);
    for (err, reason) in [
        (FetchError::NeedsLogin, FallbackReason::NeedsLogin),
        (FetchError::Blocked(403), FallbackReason::Blocked(403)),
        (FetchError::NotAvailable, FallbackReason::NotAvailable),
    ] {
        let f = fetcher(Err(err));
        let mut job = ImportJob::new(&lib, &on, &f, "Example Maker", "9000-0001", "d");
        assert!(matches!(job.run(), ImportState::Fallback { reason: r, .. } if *r == reason));
    }
    let f = fetcher(Err(FetchError::Network("timeout".into())));
    let mut job = ImportJob::new(&lib, &on, &f, "Example Maker", "9000-0001", "d");
    assert_eq!(job.run(), &ImportState::Failed("timeout".into()));
}

#[test]
fn unknown_maker_has_no_fetch_and_no_page() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    let f = ScriptedFetcher::default();
    let none = SourceTable::default();
    let mut job = ImportJob::new(&lib, &none, &f, "Nobody", "1", "d");
    assert_eq!(
        job.run(),
        &ImportState::Fallback {
            reason: FallbackReason::NotEnabled,
            page_url: None
        }
    );
}

#[test]
fn downloads_watcher_waits_for_a_finished_matching_file() {
    let downloads = tempfile::tempdir().unwrap();
    let before = SystemTime::now() - Duration::from_secs(5);
    let mut w = DownloadsWatcher::new(downloads.path(), before, "9000-0001");

    fs::write(downloads.path().join("holiday.jpg"), "x").unwrap();
    fs::write(downloads.path().join("other-part.stp"), STEP).unwrap();
    fs::write(downloads.path().join("9000-0001.stp.crdownload"), "half").unwrap();
    assert!(w.poll().is_empty());

    fs::write(downloads.path().join("WAGO_9000_0001.stp"), STEP).unwrap();
    assert!(w.poll().is_empty(), "first sight: size not yet seen twice");
    let ready = w.poll();
    assert_eq!(ready.len(), 1);
    assert!(ready[0].names_the_part);
    assert!(ready[0].path.ends_with("WAGO_9000_0001.stp"));

    // The passed-over model is listed, the picture and the half download are not.
    let names: Vec<_> = w
        .skipped()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["other-part.stp"]);

    // A file that is older than the click is ignored.
    let late = SystemTime::now() + Duration::from_secs(3600);
    let mut w = DownloadsWatcher::new(downloads.path(), late, "9000-0001");
    w.poll();
    assert!(w.poll().is_empty());
}

#[test]
fn imports_the_downloaded_file_and_a_bmecat_catalog() {
    let lib_dir = tempfile::tempdir().unwrap();
    let downloads = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(lib_dir.path());
    let f = ScriptedFetcher::default();
    let src = SourceTable::default();

    let xml = downloads.path().join("catalog.xml");
    fs::write(
        &xml,
        "<BMECAT><T_NEW_CATALOG><PRODUCT><SUPPLIER_PID>9000-0001</SUPPLIER_PID><PRODUCT_DETAILS><DESCRIPTION_SHORT>Terminal</DESCRIPTION_SHORT></PRODUCT_DETAILS></PRODUCT></T_NEW_CATALOG></BMECAT>",
    )
    .unwrap();
    let mut job = ImportJob::new(&lib, &src, &f, "Example Maker", "9000-0001", "2026-10-15");
    let ImportState::Done(out) = job.import_file(&xml, ImportFlow::Downloads).clone() else {
        panic!()
    };
    assert_eq!(out.parts.len(), 1);
    assert_eq!(out.parts[0].manufacturer, "Example Maker");
    assert_eq!(out.records[0].flow, ImportFlow::Downloads);

    // A STEP file the reader cannot measure is still stored, with a note.
    let bad = downloads.path().join("9000-0001.stp");
    fs::write(&bad, "not step").unwrap();
    let mut job = ImportJob::new(&lib, &src, &f, "Example Maker", "9000-0001", "d");
    let ImportState::Done(out) = job.import_file(&bad, ImportFlow::Manual).clone() else {
        panic!()
    };
    assert_eq!(out.records.len(), 1);
    assert!(out.dimensions.is_none() && out.notes.len() == 1);

    // Remove deletes files and records.
    assert_eq!(lib.remove(&key()).unwrap(), 2);
    assert!(!lib.has(&key()));
    assert!(!lib.path_of(&out.records[0]).exists());
}

#[test]
fn stored_names_cannot_escape_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path().join("lib"));
    let r = lib
        .store(&new_source("..\\..\\evil:name?.stp"), b"x")
        .unwrap();
    let p = lib.path_of(&r);
    assert!(p.starts_with(dir.path().join("lib")));
    assert!(!r.file_name.contains(['\\', '/', ':', '?']));
}

#[test]
fn browser_rows_show_one_action_per_part() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    let mut cat = Catalog::default();
    let mut a = CatalogPart::new("Example Maker", "9000-0001");
    a.description = "Terminal".into();
    let b = CatalogPart::new("Example Maker", "9000-0002");
    let c = CatalogPart::new("Unlisted", "7");
    for p in [a, b, c] {
        cat.upsert(p);
    }
    lib.store(&new_source("a.stp"), b"x").unwrap();
    let rows = browser_rows(&cat, &lib, &sources(true), "");
    let acts: Vec<_> = rows.iter().map(|r| r.action.clone()).collect();
    assert_eq!(
        acts,
        vec![
            RowAction::InLibrary,
            RowAction::Import,
            RowAction::OpenPage(None)
        ]
    );
    let rows = browser_rows(&cat, &lib, &sources(false), "9000-0002");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].action,
        RowAction::OpenPage(Some("https://example.invalid/p/9000-0002".into()))
    );
}

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut w = zip::ZipWriter::new(&mut buf);
    let opts = zip::write::SimpleFileOptions::default();
    for (name, data) in files {
        w.start_file(*name, opts).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap();
    buf.into_inner()
}

#[test]
fn imports_a_portal_zip_with_one_step_file_and_keeps_the_terms_link() {
    let lib_dir = tempfile::tempdir().unwrap();
    let downloads = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(lib_dir.path());
    let zip = zip_of(&[
        ("2002-1201_HI.stp", STEP.as_bytes()),
        (
            "readme.txt",
            b"Generated by the portal.\nTerms of use: https://example.invalid/terms-of-use-3d-cad-models.\n",
        ),
    ]);
    let path = downloads.path().join("WAGO_20021201HI.zip");
    fs::write(&path, &zip).unwrap();

    // The watcher finds the ZIP by the digits of the order number.
    let mut w = DownloadsWatcher::new(
        downloads.path(),
        SystemTime::now() - Duration::from_secs(5),
        "2002-1201",
    );
    w.poll();
    assert_eq!(
        w.poll().iter().map(|a| a.path.clone()).collect::<Vec<_>>(),
        vec![path.clone()]
    );

    let f = ScriptedFetcher::default();
    let src = SourceTable::default();
    let mut job = ImportJob::new(&lib, &src, &f, "WAGO", "2002-1201", "2026-10-15");
    let ImportState::Done(out) = job.import_file(&path, ImportFlow::Downloads).clone() else {
        panic!()
    };
    let r = &out.records[0];
    assert_eq!(r.kind, FileKind::Zip);
    assert_eq!(
        r.terms_url.as_deref(),
        Some("https://example.invalid/terms-of-use-3d-cad-models")
    );
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    let d = out.dimensions.unwrap();
    assert_eq!((d.depth_mm, d.width_mm, d.height_mm), (5.2, 36.0, 42.0));
    assert_eq!(fs::read(lib.path_of(r)).unwrap(), zip);
}

#[test]
fn a_zip_without_step_or_not_a_zip_is_stored_with_a_note() {
    let lib_dir = tempfile::tempdir().unwrap();
    let downloads = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(lib_dir.path());
    let f = ScriptedFetcher::default();
    let src = SourceTable::default();
    for (name, bytes) in [
        ("a.zip", zip_of(&[("readme.txt", b"hi")])),
        ("b.zip", b"not a zip".to_vec()),
    ] {
        let p = downloads.path().join(name);
        fs::write(&p, bytes).unwrap();
        let mut job = ImportJob::new(&lib, &src, &f, "WAGO", "1", "d");
        let ImportState::Done(out) = job.import_file(&p, ImportFlow::Manual).clone() else {
            panic!()
        };
        assert_eq!(out.records.len(), 1);
        assert!(out.dimensions.is_none() && !out.notes.is_empty());
    }
}

#[test]
fn imports_an_edz_package_unchanged_and_hands_out_its_xml() {
    let lib_dir = tempfile::tempdir().unwrap();
    let downloads = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(lib_dir.path());
    let edz = zip_of(&[
        ("PXC.3044076.xml", b"<parts><part/></parts>"),
        ("PXC.3044076.ema", b"macro data"),
        ("PXC.3044076.pdf", b"%PDF"),
    ]);
    let path = downloads.path().join("3044076.edz");
    fs::write(&path, &edz).unwrap();

    // An EDZ is found by type, since generator output is not named after the article.
    let mut w = DownloadsWatcher::new(
        downloads.path(),
        SystemTime::now() - Duration::from_secs(5),
        "3044076-x",
    );
    w.poll();
    assert_eq!(
        w.poll().iter().map(|a| a.path.clone()).collect::<Vec<_>>(),
        vec![path.clone()]
    );

    let f = ScriptedFetcher::default();
    let src = sources(false);
    let mut job = ImportJob::new(&lib, &src, &f, "Example Maker", "3044076", "2026-10-15");
    let ImportState::Done(out) = job.import_file(&path, ImportFlow::Downloads).clone() else {
        panic!()
    };
    let r = &out.records[0];
    assert_eq!(r.kind, FileKind::Edz);
    assert_eq!(
        r.terms_url.as_deref(),
        Some("https://example.invalid/cloud-terms")
    );
    assert_eq!(out.xml_members.len(), 1);
    assert_eq!(out.xml_members[0].0, "PXC.3044076.xml");
    assert!(
        out.dimensions.is_none() && out.notes.is_empty(),
        "{:?}",
        out.notes
    );
    assert_eq!(fs::read(lib.path_of(r)).unwrap(), edz, "stored unchanged");
}

#[test]
fn a_downloaded_dxf_or_zip_of_one_is_stored_and_handed_out() {
    let lib_dir = tempfile::tempdir().unwrap();
    let downloads = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(lib_dir.path());
    let f = ScriptedFetcher::default();
    let src = SourceTable::default();
    let dxf = "0\nSECTION\n2\nENTITIES\n0\nLINE\n0\nENDSEC\n0\nEOF\n";

    let direct = downloads.path().join("PXC.3044076.dxf");
    fs::write(&direct, dxf).unwrap();
    let zipped = downloads.path().join("3044076.zip");
    fs::write(&zipped, zip_of(&[("outline.DXF", dxf.as_bytes())])).unwrap();

    for path in [&direct, &zipped] {
        let mut job = ImportJob::new(&lib, &src, &f, "Example Maker", "3044076", "d");
        let ImportState::Done(out) = job.import_file(path, ImportFlow::Downloads).clone() else {
            panic!()
        };
        assert_eq!(out.dxf_members.len(), 1, "{path:?}");
        assert_eq!(out.dxf_members[0].1, dxf);
    }
    assert_eq!(lib.records().unwrap().len(), 2);
}

#[test]
fn the_built_in_sources_never_fetch_and_label_unconfirmed_pages() {
    let t = SourceTable::builtin();
    for maker in ["Schneider Electric", "WAGO", "Phoenix Contact", "Siemens"] {
        assert!(!t.get(maker).unwrap().fetch_enabled, "{maker}");
    }
    let se = t.get("schneider electric").unwrap();
    assert!(!se.page_verified);
    assert_eq!(
        se.page_for("LC1D09BD").as_deref(),
        Some("https://www.se.com/ww/en/product/LC1D09BD/")
    );
    let wago = t.get("WAGO").unwrap();
    assert!(wago.page_verified);
    assert_eq!(
        wago.page_for("2002-1201").as_deref(),
        Some("https://wago.partcommunity.com/3d-cad-models/?catalog=wago&part=2002-1201")
    );
    assert!(
        t.get("Siemens")
            .unwrap()
            .page_for("3RT2016-1BB41")
            .is_none()
    );
}

#[test]
fn reserved_windows_names_are_prefixed() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    for (given, want) in [
        ("nul.step", "_nul.step"),
        ("COM1", "_COM1"),
        ("com10.stp", "com10.stp"),
        ("console.stp", "console.stp"),
    ] {
        let r = lib.store(&new_source(given), b"x").unwrap();
        assert_eq!(r.file_name, want, "{given}");
    }
}

#[test]
fn a_newer_records_file_leaves_no_orphan_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("records.json"), r#"{"format": 99}"#).unwrap();
    let lib = LocalLibrary::at(dir.path());
    assert!(lib.store(&new_source("a.stp"), b"x").is_err());
    assert!(!dir.path().join("sources").exists());
}

#[test]
fn a_zip_with_many_xml_members_reads_at_most_sixteen() {
    use std::io::Write;
    let big = vec![0u8; 1024 * 1024];
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        // 20 members of 1 MB: only 16 are read. (The 256 MB total budget is not tested: it needs a 256 MB archive.)
        for i in 0..20 {
            w.start_file(format!("m{i}.xml"), opts).unwrap();
            w.write_all(&big).unwrap();
        }
        w.finish().unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    let p = dir.path().join("pack.zip");
    fs::write(&p, buf.into_inner()).unwrap();
    let f = ScriptedFetcher::default();
    let src = SourceTable::default();
    let mut job = ImportJob::new(&lib, &src, &f, "M", "1", "d");
    let ImportState::Done(out) = job.import_file(&p, ImportFlow::Manual).clone() else {
        panic!()
    };
    assert!(
        out.xml_members.len() <= 16,
        "at most 16 xml members are read"
    );
}

struct FlakyFetcher;

impl Fetcher for FlakyFetcher {
    fn resolve(&self, _: &PartKey) -> Result<Vec<RemoteFile>, FetchError> {
        Ok(["a.stp", "b.stp"]
            .iter()
            .map(|n| RemoteFile {
                kind: FileKind::Step,
                url: format!("https://example.invalid/{n}"),
                file_name: (*n).into(),
            })
            .collect())
    }

    fn fetch(&self, file: &RemoteFile) -> Result<Vec<u8>, FetchError> {
        if file.file_name == "a.stp" {
            Ok(STEP.as_bytes().to_vec())
        } else {
            Err(FetchError::Network("connection reset".into()))
        }
    }
}

#[test]
fn a_failed_job_still_lists_what_it_stored() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    lib.accept_notice("Example Maker").unwrap();
    let src = sources(true);
    let mut job = ImportJob::new(&lib, &src, &FlakyFetcher, "Example Maker", "9000-0001", "d");
    assert_eq!(job.run(), &ImportState::Failed("connection reset".into()));
    assert_eq!(job.stored_so_far().records.len(), 1);
    assert!(job.stored_so_far().dimensions.is_some());
    assert_eq!(lib.records().unwrap().len(), 1);
}

#[test]
fn a_file_saved_right_at_the_click_is_not_missed_to_rounded_file_times() {
    let downloads = tempfile::tempdir().unwrap();
    let click = SystemTime::now();
    let path = downloads.path().join("9000-0001.stp");
    fs::write(&path, STEP).unwrap();
    // The file system stamped it a little before the click's clock reading.
    let f = fs::File::options().write(true).open(&path).unwrap();
    f.set_modified(click - Duration::from_millis(900)).unwrap();
    let mut w = DownloadsWatcher::new(downloads.path(), click, "9000-0001");
    w.poll();
    assert_eq!(w.poll().len(), 1);

    // A file well before the click is still ignored.
    let old = downloads.path().join("9000-0001-old.stp");
    fs::write(&old, STEP).unwrap();
    let f = fs::File::options().write(true).open(&old).unwrap();
    f.set_modified(click - Duration::from_secs(30)).unwrap();
    let mut w = DownloadsWatcher::new(downloads.path(), click, "9000-0001");
    w.poll();
    let got = w.poll();
    assert_eq!(got.len(), 1);
    assert!(got[0].path.ends_with("9000-0001.stp"));
}
