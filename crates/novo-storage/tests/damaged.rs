//! A damaged project file is refused in words, never panics, and its
//! drawing can be salvaged into a recovered copy; the original is never
//! written to.

use std::fs;
use std::path::{Path, PathBuf};

use novo_core::{DeviceTag, Document};
use novo_storage::{ProjectFile, Session, StorageError, salvage};
use rusqlite::Connection;
use tempfile::TempDir;

fn temp() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plant.novo");
    (dir, path)
}

/// A saved project with `pages` pages of four devices each.
fn saved(path: &Path, pages: usize) -> Document {
    let mut doc = Document::default();
    doc.transact("Draw", |tx| {
        for p in 0..pages {
            tx.add_page((p + 1).to_string())?;
            for d in 0..4 {
                tx.add_device(
                    DeviceTag::parse(&format!("-K{}", p * 4 + d + 1)).unwrap(),
                    "Contactor",
                )?;
            }
        }
        Ok(())
    })
    .unwrap();
    Session::create(path, doc.project())
        .unwrap()
        .close()
        .unwrap();
    doc
}

fn cut(path: &Path, keep: f64) {
    let bytes = fs::read(path).unwrap();
    fs::write(path, &bytes[..(bytes.len() as f64 * keep) as usize]).unwrap();
}

#[test]
fn a_truncated_file_is_refused_as_damaged_in_words() {
    let (_dir, path) = temp();
    saved(&path, 150);
    cut(&path, 0.6);
    let err = Session::open(&path).unwrap_err();
    assert!(
        matches!(err, StorageError::Corrupt { .. }),
        "a damaged file says so, got {err:?}"
    );
    assert!(err.to_string().contains("damaged"), "{err}");
}

#[test]
fn salvage_of_a_truncated_file_writes_a_recovered_copy_and_leaves_the_original() {
    let (dir, path) = temp();
    let doc = saved(&path, 150);
    cut(&path, 0.6);
    let before = fs::read(&path).unwrap();

    let salvaged = salvage(&path).unwrap();
    assert_eq!(
        fs::read(&path).unwrap(),
        before,
        "the original is untouched"
    );
    assert_eq!(salvaged.copy, dir.path().join("plant.recovered.novo"));
    assert!(salvaged.recovered > 0, "something was read back");
    assert!(
        salvaged.cut_short,
        "the report says the end of the file was lost"
    );
    let total = doc.project().records().count();
    assert!(salvaged.recovered < total);

    // The copy is an ordinary, clean project file.
    let opened = Session::open(&salvaged.copy).unwrap();
    assert_eq!(opened.project.to_json(), salvaged.project.to_json());
    assert_eq!(opened.project.records().count(), salvaged.recovered);
    assert!(opened.recovered.is_none());
}

#[test]
fn salvage_skips_a_damaged_row_and_keeps_the_rest() {
    let (_dir, path) = temp();
    let doc = saved(&path, 3);
    let victim = doc.project().devices().next().unwrap().id;
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "UPDATE records SET data = '{\"record\": \"device\", ' WHERE id = ?1",
            [victim.raw() as i64],
        )
        .unwrap();
    }
    assert!(matches!(
        Session::open(&path).unwrap_err(),
        StorageError::Corrupt { .. }
    ));
    let salvaged = salvage(&path).unwrap();
    assert_eq!(salvaged.skipped, 1);
    assert!(!salvaged.cut_short);
    assert_eq!(
        salvaged.recovered,
        doc.project().records().count() - 1,
        "everything but the damaged object"
    );
    assert!(salvaged.project.device(victim).is_none());
    Session::open(&salvaged.copy).unwrap();
}

#[test]
fn salvage_drops_objects_whose_parent_was_lost() {
    let (_dir, path) = temp();
    let doc = saved(&path, 2);
    let page = doc.project().pages().next().unwrap().id;
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute("DELETE FROM records WHERE id = ?1", [page.raw() as i64])
            .unwrap();
    }
    let salvaged = salvage(&path).unwrap();
    // Whatever referred to the lost page is dropped, the rest is kept, and
    // the copy loads under every integrity rule.
    assert!(salvaged.project.page(page).is_none());
    Session::open(&salvaged.copy).unwrap();
}

#[test]
fn a_second_salvage_does_not_overwrite_the_first_copy() {
    let (dir, path) = temp();
    saved(&path, 150);
    cut(&path, 0.6);
    let first = salvage(&path).unwrap();
    let second = salvage(&path).unwrap();
    assert_ne!(first.copy, second.copy);
    assert!(first.copy.exists() && second.copy.exists());
    assert_eq!(second.copy, dir.path().join("plant.recovered-2.novo"));
}

#[test]
fn a_file_that_is_not_a_database_cannot_be_salvaged() {
    let (_dir, path) = temp();
    fs::write(&path, b"hello, this is a letter").unwrap();
    assert!(matches!(
        salvage(&path).unwrap_err(),
        StorageError::NotAProject(_)
    ));
    fs::write(&path, b"").unwrap();
    assert!(salvage(&path).is_err());
}

#[test]
fn a_healthy_file_salvages_to_everything() {
    let (_dir, path) = temp();
    let doc = saved(&path, 3);
    let salvaged = salvage(&path).unwrap();
    assert_eq!(salvaged.skipped, 0);
    assert!(!salvaged.cut_short);
    assert_eq!(salvaged.project.to_json(), doc.project().to_json());
    ProjectFile::read(&salvaged.copy).unwrap();
}

#[test]
fn whatever_part_of_the_file_is_lost_open_never_panics_and_salvage_gives_a_clean_copy() {
    let (_dir, path) = temp();
    saved(&path, 150);
    let bytes = fs::read(&path).unwrap();
    let pages = bytes.len() / 4096;
    assert!(pages > 20, "a project big enough to have many pages");

    // Cut at several points, and wipe each page of the file in turn.
    let mut damaged: Vec<Vec<u8>> = [0.1, 0.25, 0.5, 0.75, 0.9, 0.99]
        .iter()
        .map(|f| bytes[..(bytes.len() as f64 * f) as usize].to_vec())
        .collect();
    for page in (1..pages).step_by(pages / 12 + 1) {
        let mut wiped = bytes.clone();
        wiped[page * 4096..(page + 1) * 4096].fill(0xA5);
        damaged.push(wiped);
    }
    for (n, image) in damaged.iter().enumerate() {
        let dir = tempfile::tempdir().unwrap();
        let broken = dir.path().join("broken.novo");
        fs::write(&broken, image).unwrap();
        // Opening either works (the damage was in free space) or says so.
        if let Err(e) = Session::open(&broken) {
            assert!(
                matches!(
                    e,
                    StorageError::Corrupt { .. } | StorageError::NotAProject(_)
                ),
                "image {n}: {e:?}"
            );
        }
        match salvage(&broken) {
            Ok(s) => {
                let reopened = Session::open(&s.copy)
                    .unwrap_or_else(|e| panic!("image {n}: the copy must open: {e}"));
                assert_eq!(reopened.project.records().count(), s.recovered, "image {n}");
            }
            Err(e) => assert!(
                matches!(e, StorageError::NotAProject(_)),
                "image {n}: only a file without a header is beyond salvage: {e:?}"
            ),
        }
    }
}

/// A number written as JSON text must read back as the same f64. The
/// default parser is up to 1 ULP off for some values (3.9000000000000004
/// came back as 3.9), so a saved and reopened project silently differed.
#[test]
fn numbers_survive_save_and_reopen_bit_for_bit() {
    use novo_core::Value;
    let (_dir, path) = temp();
    let mut doc = Document::default();
    let values = [
        3.9000000000000004,
        0.1 + 0.2,
        1e-7 * 3.0,
        123456.789e3,
        f64::MIN_POSITIVE,
    ];
    doc.transact("Numbers", |tx| {
        tx.set_info(|i| {
            for (n, v) in values.iter().enumerate() {
                i.properties.insert(format!("v{n}"), Value::Float(*v));
            }
        })
    })
    .unwrap();
    Session::create(&path, doc.project())
        .unwrap()
        .close()
        .unwrap();
    let opened = Session::open(&path).unwrap();
    for (n, v) in values.iter().enumerate() {
        let Some(Value::Float(got)) = opened.project.info().properties.get(&format!("v{n}")) else {
            panic!("v{n} missing");
        };
        assert_eq!(got.to_bits(), v.to_bits(), "v{n}: {got} vs {v}");
    }
}

#[test]
fn salvage_says_when_only_the_project_information_was_lost() {
    let (_dir, path) = temp();
    let doc = saved(&path, 3);
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute("UPDATE meta SET value = '{broken' WHERE key = 'info'", [])
            .unwrap();
    }
    let salvaged = salvage(&path).unwrap();
    assert!(salvaged.info_lost);
    assert_eq!(salvaged.skipped, 0, "every drawing object was read back");
    assert_eq!(salvaged.recovered, doc.project().records().count());
    assert!(
        salvaged.summary().contains("project information"),
        "the warning names it: {}",
        salvaged.summary()
    );

    // A healthy file does not claim it.
    let (_dir, path) = temp();
    saved(&path, 1);
    assert!(!salvage(&path).unwrap().info_lost);
}

#[test]
fn the_report_names_what_is_in_the_copy_and_what_was_left_out() {
    let (_dir, path) = temp();
    let doc = saved(&path, 3);
    let victim = doc.project().devices().next().unwrap().id;
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "UPDATE records SET data = '{\"record\": \"device\", ' WHERE id = ?1",
            [victim.raw() as i64],
        )
        .unwrap();
    }
    let salvaged = salvage(&path).unwrap();
    assert_eq!(salvaged.lost_text().as_deref(), Some("1 device"));
    // Pages, symbols and wires are always named, even at zero: an empty
    // page is the thing to notice.
    assert!(
        salvaged
            .kept_text()
            .starts_with("3 pages, 0 symbols, 0 wires"),
        "{}",
        salvaged.kept_text()
    );
    assert!(salvaged.kept_text().contains("11 devices"));
    let lines = salvaged.report_lines().join("\n");
    assert!(
        lines.contains("Found but unreadable, left out: 1 device."),
        "{lines}"
    );
    assert!(!lines.contains("end of the file"), "{lines}");
}

#[test]
fn the_report_says_so_when_the_end_of_the_file_is_gone() {
    let (_dir, path) = temp();
    saved(&path, 150);
    cut(&path, 0.6);
    let salvaged = salvage(&path).unwrap();
    assert!(salvaged.cut_short);
    let lines = salvaged.report_lines().join("\n");
    assert!(
        lines.contains("The end of the file was lost") && lines.contains("cannot be counted"),
        "{lines}"
    );
}

#[test]
fn the_recovered_copy_keeps_the_extension_of_the_damaged_file() {
    for name in ["dmg.kopercad", "dmg.novo", "dmg"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        saved(&path, 150);
        cut(&path, 0.6);
        let salvaged = salvage(&path).unwrap();
        let expect = if name == "dmg.novo" {
            "dmg.recovered.novo"
        } else {
            "dmg.recovered.kopercad"
        };
        assert_eq!(salvaged.copy, dir.path().join(expect));
        // A second recovery takes the next free name, same extension.
        let second = salvage(&path).unwrap();
        assert_eq!(
            second.copy,
            dir.path()
                .join(expect.replace(".recovered.", ".recovered-2."))
        );
    }
}

#[test]
fn both_extensions_are_project_extensions() {
    use std::ffi::OsStr;
    assert_eq!(novo_storage::PROJECT_EXTENSION, "kopercad");
    assert!(novo_storage::is_project_extension(OsStr::new("kopercad")));
    assert!(novo_storage::is_project_extension(OsStr::new("NOVO")));
    assert!(!novo_storage::is_project_extension(OsStr::new("pdf")));
}
