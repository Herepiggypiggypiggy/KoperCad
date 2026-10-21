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

#[test]
fn paths_teach_prerequisites_first_and_cover_every_lesson() {
    let (catalog, _) = catalog();
    let notes = novo_learn::lint::path_notes(&catalog);
    assert!(notes.is_empty(), "{notes:#?}");
}

#[test]
fn every_ui_key_the_program_asks_for_has_words() {
    use novo_learn::lesson::Lang;
    let text = std::fs::read_to_string(lessons_dir().join("ui.toml")).unwrap();
    let ui = novo_learn::UiStrings::parse(&text).expect("ui.toml parses");
    // A missing key would show on screen as the key itself ("learn_menu").
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../novo-app/src");
    let mut missing = Vec::new();
    let mut seen = 0;
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                && !path.to_string_lossy().contains("_tests")
            {
                let code = std::fs::read_to_string(&path).unwrap();
                for call in ["say(\"", "say_with(\""] {
                    for part in code.split(call).skip(1) {
                        let key: String = part
                            .chars()
                            .take_while(|c| {
                                c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_'
                            })
                            .collect();
                        if part[key.len()..].starts_with('"') {
                            seen += 1;
                            if ui.get(&key, Lang::En) == key {
                                missing.push(format!("{}: {key}", path.display()));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(seen > 20, "the scan found the program's keys ({seen})");
    assert!(missing.is_empty(), "ui keys without words: {missing:#?}");
    assert_eq!(ui.get("learn_menu", Lang::En), "Learn…");
}
