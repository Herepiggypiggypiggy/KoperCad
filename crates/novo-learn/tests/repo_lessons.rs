//! Every lesson in `assets/lessons` must parse and resolve its files. This
//! is the part of the lint that needs no running application, so a content
//! pull request fails here, in plain `cargo test`, when it breaks the
//! format. The checks that need this build's commands and anchors run in
//! `novo-app` (docs/design/learning-modules.md §7).

use std::path::PathBuf;

use novo_core::Project;
use novo_learn::catalog::{self, Catalog, ProjectRef};

fn lessons_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/lessons")
}

fn catalog() -> (Catalog, catalog::Files) {
    let (files, notes) = catalog::read_dir(&lessons_dir()).expect("assets/lessons is readable");
    assert!(notes.is_empty(), "files skipped: {notes:?}");
    (Catalog::from_files(&files), files)
}

#[test]
fn tracks_and_glossary_parse() {
    let (catalog, _) = catalog();
    assert!(catalog.notes.is_empty(), "{:?}", catalog.notes);
    let tracks: Vec<_> = catalog.tracks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        tracks,
        [
            "start",
            "circuits",
            "terminals",
            "outputs",
            "cabinet",
            "data"
        ]
    );
    assert!(catalog.glossary.contains_key("pin"));
}

#[test]
fn every_lesson_parses_and_names_files_that_exist() {
    let (catalog, files) = catalog();
    let rejected: Vec<_> = catalog
        .rejected
        .iter()
        .map(|(path, e)| format!("{path}: {e}"))
        .collect();
    assert!(rejected.is_empty(), "{rejected:#?}");
    for entry in &catalog.entries {
        let id = &entry.lesson.id;
        assert!(
            catalog.tracks.iter().any(|t| t.id == entry.lesson.track),
            "{id}: track `{}` is not in tracks.toml",
            entry.lesson.track
        );
        let project = entry.project.as_ref().unwrap_or_else(|| {
            panic!(
                "{id}: project `{}` is not among the files",
                entry.lesson.project
            )
        });
        // A core JSON format bump would break every lesson at once; this
        // names the file and the fix (assets/lessons/README.md).
        let mut sample = Vec::new();
        if let ProjectRef::File(path) = project {
            sample.push(path.as_str());
        }
        sample.extend(entry.finished.as_deref());
        for path in sample {
            Project::from_json(&files[path]).unwrap_or_else(|e| {
                panic!(
                    "{id}: {path} does not load ({e}); regenerate it, see assets/lessons/README.md"
                )
            });
        }
    }
}
