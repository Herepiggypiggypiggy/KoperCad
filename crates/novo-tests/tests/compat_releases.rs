//! REL-COMPAT: project files written by the release candidates themselves.
//!
//! `golden/releases/rc1` and `rc2` hold files written by the code of the tags
//! `v0.1.0-rc1` and `v0.1.0-rc2` (built in a side checkout of each tag and
//! saved with that tag's own `ProjectFile::create`): the motor starter with an
//! issued revision, its snapshot and a sidecar, a page of 300 objects, and
//! the finished project of every lesson the tag shipped (cables, cabinet
//! layout, terminals, PLC, revisions and more). `manifest.txt` lists, as that
//! tag counted them, the records, pages, symbols and wires of each file.
//!
//! A current build must read each file without writing to it, find the same
//! counts, open it for editing (running any pending migration), take an edit,
//! save and reopen. The files never change once added; a new release adds a
//! directory beside these. To add one: check out the tag, run a throwaway test
//! that writes the same files with that tag's code, copy them here.

use std::path::{Path, PathBuf};

use novo_core::*;
use novo_storage::{ProjectFile, Session};

fn releases() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/releases")
}

/// `(file stem, records, pages, symbols, wires)` as the release counted them.
fn manifest(release: &str) -> Vec<(String, usize, usize, usize, usize)> {
    let text = std::fs::read_to_string(releases().join(release).join("manifest.txt")).unwrap();
    text.lines()
        .map(|line| {
            let f: Vec<&str> = line.split(' ').collect();
            let n = |i: usize| f[i].parse::<usize>().unwrap();
            (f[0].to_owned(), n(1), n(2), n(3), n(4))
        })
        .collect()
}

fn counts(p: &Project) -> (usize, usize, usize, usize) {
    (
        p.records().count(),
        p.pages().count(),
        p.symbols().count(),
        p.wires().count(),
    )
}

fn every_file() -> Vec<(String, PathBuf, (usize, usize, usize, usize))> {
    let mut all = Vec::new();
    for release in ["rc1", "rc2"] {
        for (stem, records, pages, symbols, wires) in manifest(release) {
            let path = releases().join(release).join(format!("{stem}.novo"));
            assert!(path.exists(), "{}", path.display());
            all.push((
                format!("{release}/{stem}"),
                path,
                (records, pages, symbols, wires),
            ));
        }
    }
    all
}

#[test]
fn the_fixtures_are_all_there() {
    let all = every_file();
    assert!(all.len() >= 48, "{} files", all.len());
    let lessons = all.iter().filter(|(n, ..)| n.contains("lesson-")).count();
    assert!(lessons >= 40, "{lessons} lesson files");
}

#[test]
fn every_release_file_reads_with_the_counts_its_release_wrote_and_is_not_touched() {
    for (name, path, expected) in every_file() {
        let before = std::fs::read(&path).unwrap();
        let project = ProjectFile::read(&path)
            .unwrap_or_else(|e| panic!("{name} no longer opens read-only: {e}"));
        assert_eq!(counts(&project), expected, "{name}");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "{name}: reading changed the file"
        );
        let dir = path.parent().unwrap();
        let strays: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with("-journal") || n.ends_with(".bak") || n.ends_with(".saving"))
            .collect();
        assert!(
            strays.is_empty(),
            "{name}: left {strays:?} beside the fixture"
        );
    }
}

#[test]
fn every_release_file_opens_for_editing_takes_an_edit_and_reopens() {
    for (name, path, expected) in every_file() {
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join("copy.novo");
        std::fs::copy(&path, &copy).unwrap();

        let opened = Session::open(&copy)
            .unwrap_or_else(|e| panic!("{name} no longer opens for editing: {e}"));
        let mut session = opened.session;
        let mut doc = Document::new(opened.project);
        assert_eq!(counts(doc.project()), expected, "{name}");

        let (_, changes) = doc
            .transact("After the upgrade", |tx| tx.add_page("AFTER").map(|_| ()))
            .unwrap_or_else(|e| panic!("{name}: an edit is refused: {e}"));
        session.record(&changes);
        session.save(doc.project()).unwrap();
        session.close().unwrap();

        let again = Session::open(&copy).unwrap();
        assert_eq!(
            again.project.to_json(),
            doc.project().to_json(),
            "{name}: the edit did not come back"
        );
        assert_eq!(
            again.project.pages().count(),
            expected.1 + 1,
            "{name}: one page more"
        );
        again.session.close().unwrap();
    }
}

#[test]
fn a_release_file_saved_as_by_this_build_keeps_the_drawing_exactly() {
    for (name, path, _) in every_file() {
        let project = ProjectFile::read(&path).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.novo");
        Session::create(&out, &project).unwrap().close().unwrap();
        let again = ProjectFile::read(&out).unwrap();
        assert_eq!(again.to_json(), project.to_json(), "{name}");
    }
}

#[test]
fn the_issued_revision_snapshot_and_sidecar_of_each_release_survive() {
    for release in ["rc1", "rc2"] {
        let path = releases().join(release).join("motor-starter-issued.novo");
        let file = ProjectFile::read_only(&path).unwrap();
        let project = file.load().unwrap();
        let revisions: Vec<_> = file.snapshot_revisions().unwrap();
        assert_eq!(
            revisions.len(),
            1,
            "{release}: the issued revision's snapshot"
        );
        let snapshot = file.snapshot(revisions[0]).unwrap().expect("snapshot");
        assert_eq!(
            snapshot.records().count(),
            project.records().count(),
            "{release}"
        );
        assert_eq!(
            file.sidecar("commissioning").unwrap().as_deref(),
            Some("{\"checks\":[],\"punch\":[]}"),
            "{release}: the commissioning sidecar"
        );
    }
}
