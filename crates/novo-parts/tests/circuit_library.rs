//! The standard circuit library: every shipped file builds, every circuit
//! has a chapter and a mounting view that fits its plate, nothing shadows a
//! starter, and installing is idempotent and keeps what the user changed.

use std::collections::BTreeSet;
use std::path::Path;

use novo_core::{BlockDefinitionId, Document, Point, Project, Record};
use novo_parts::{
    CHAPTERS, CIRCUIT_ROOT, Imported, MacroLibrary, MountKind, build_circuit, circuit_library,
    starter_macros,
};

fn library() -> Vec<(String, novo_parts::MacroFile)> {
    circuit_library().unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn every_circuit_file_on_disk_is_shipped_and_every_shipped_file_exists() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("circuits");
    let mut on_disk = BTreeSet::new();
    for chapter in std::fs::read_dir(&root).unwrap() {
        let chapter = chapter.unwrap().path();
        if !chapter.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&chapter).unwrap() {
            let file = file.unwrap().path();
            if file.extension().is_some_and(|e| e == "toml") {
                on_disk.insert(format!(
                    "{}/{}",
                    chapter.file_name().unwrap().to_string_lossy(),
                    file.file_name().unwrap().to_string_lossy()
                ));
            }
        }
    }
    let shipped: BTreeSet<String> = novo_parts::circuit_sources()
        .iter()
        .map(|s| s.path.to_string())
        .collect();
    assert_eq!(on_disk, shipped, "add new circuit files to SOURCES");
}

#[test]
fn circuit_files_sit_in_the_folder_of_their_chapter() {
    for s in novo_parts::circuit_sources() {
        let folder = s.path.split('/').next().unwrap();
        let spec = novo_parts::CircuitSpec::parse(s.text, s.path).unwrap();
        let chapter = CHAPTERS.iter().find(|c| c.slug == spec.chapter).unwrap();
        assert_eq!(
            folder,
            format!("{}-{}", chapter.number, chapter.slug),
            "{}",
            s.path
        );
    }
}

#[test]
fn names_are_unique_and_do_not_shadow_a_starter() {
    let starters: BTreeSet<String> = starter_macros().into_iter().map(|(_, f)| f.name).collect();
    let mut seen = BTreeSet::new();
    for (folder, file) in library() {
        assert!(folder.starts_with(CIRCUIT_ROOT), "{folder}");
        assert!(seen.insert(file.name.clone()), "{} twice", file.name);
        assert!(!starters.contains(&file.name), "{} is a starter", file.name);
        assert!(
            !file.description.is_empty() && file.tags.len() >= 3,
            "{} needs a description and three tags",
            file.name
        );
    }
}

#[test]
fn every_chapter_has_circuits() {
    let lib = library();
    for c in CHAPTERS {
        assert!(
            lib.iter().any(|(folder, _)| *folder == c.folder()),
            "chapter {} is empty",
            c.title
        );
    }
}

#[test]
fn every_device_has_a_mounting_entry_that_fits() {
    for (_, file) in library() {
        let view = file
            .mounting()
            .unwrap_or_else(|| panic!("{} has no mounting view", file.name));
        assert_eq!(view.problems(), Vec::<String>::new(), "{}", file.name);
        let devices: Vec<(u64, String)> = file
            .definition
            .members
            .iter()
            .filter_map(|m| match m {
                Record::Device(d) => Some((d.id.raw(), d.tag.to_string())),
                _ => None,
            })
            .collect();
        for (id, tag) in &devices {
            assert!(
                view.items.iter().any(|i| i.device == *id && &i.tag == tag),
                "{}: {tag} is not in the mounting view",
                file.name
            );
        }
        assert_eq!(view.items.len(), devices.len(), "{}", file.name);
        assert!(
            view.panel().next().is_some(),
            "{} has nothing on the plate",
            file.name
        );
        // A field device says where it sits.
        for i in view.field() {
            assert!(i.place.is_some(), "{}: {} has no place", file.name, i.tag);
        }
        // The plate is not wasteful: it is at most 3 times what is needed.
        let (w, h) = view.needed_plate(0.0);
        assert!(
            view.plate.width <= w * 3.0 && view.plate.height <= h * 3.0,
            "{}: the plate is far bigger than its devices",
            file.name
        );
        assert!(
            view.items
                .iter()
                .all(|i| i.kind != MountKind::Din || i.rail.is_some()),
            "{}",
            file.name
        );
    }
}

#[test]
fn every_circuit_places_and_clears_its_open_points() {
    for (_, file) in library() {
        let mut doc = Document::new(Project::new(""));
        doc.transact("place", |tx| {
            let page = tx.add_page("1")?;
            let mut definition = file.definition.clone();
            definition.id = BlockDefinitionId(9_000_000);
            let def = definition.id;
            tx.insert_record(Record::BlockDefinition(definition))?;
            tx.place_block(def, page, Point::new(20.0, 20.0), vec![])?;
            Ok(())
        })
        .unwrap_or_else(|e| panic!("{}: {e}", file.name));
        assert!(doc.project().devices().count() >= 3, "{}", file.name);
    }
}

#[test]
fn a_circuit_that_leaves_a_pin_unwired_is_refused() {
    let text = r#"
name = "Loose"
chapter = "supply"
description = "A fuse with nothing on it"
device = [{ id = "f1", tag = "-F1", description = "Fuse" }]
symbol = [{ id = "f1s", key = "iec/protection/mcb_1p", at = [40, 40], device = "f1" }]
"#;
    let err = build_circuit(text, "loose.toml").unwrap_err().to_string();
    assert!(err.contains("f1s.1 is not wired"), "{err}");
    assert!(err.contains("f1s.2 is not wired"), "{err}");
}

#[test]
fn a_misspelt_pin_or_symbol_names_what_exists() {
    let text = |key: &str, pin: &str| {
        format!(
            r#"
name = "Typo"
chapter = "supply"
description = "x"
open = ["f1s.1"]
device = [{{ id = "f1", tag = "-F1", description = "Fuse" }}]
symbol = [{{ id = "f1s", key = "{key}", at = [40, 40], device = "f1" }}]
wire = [{{ from = "f1s.{pin}", to = "f1s.2" }}]
"#
        )
    };
    let err = build_circuit(&text("iec/protection/mcb_1p", "9"), "t").unwrap_err();
    assert!(err.to_string().contains("has no pin 9"), "{err}");
    let err = build_circuit(&text("iec/nothing/here", "1"), "t").unwrap_err();
    assert!(err.to_string().contains("does not have"), "{err}");
}

#[test]
fn unknown_fields_and_chapters_are_refused() {
    let err = build_circuit("name = 'x'\nchapterr = 'y'", "t").unwrap_err();
    assert!(err.to_string().contains("chapterr"), "{err}");
    let text = r#"
name = "Where"
chapter = "nowhere"
description = "x"
device = []
symbol = []
"#;
    let err = build_circuit(text, "t").unwrap_err();
    assert!(err.to_string().contains("chapter 'nowhere'"), "{err}");
}

#[test]
fn installing_circuits_is_idempotent_and_keeps_user_edits() {
    let dir = tempfile::tempdir().unwrap();
    let lib = MacroLibrary::new(dir.path().join("Macros"), "user");
    let first = lib.install_circuits().unwrap();
    assert_eq!(first.len(), library().len());
    assert!(first.iter().all(|r| matches!(r, Imported::Added(_))));
    let (folder, file) = library().into_iter().next().unwrap();
    let path = format!("{folder}/{}.kcmacro", file.name);
    let mut edited = lib.load(&path).unwrap();
    edited.description = "mine".into();
    lib.replace(&path, &edited).unwrap();
    let second = lib.install_circuits().unwrap();
    assert!(second.iter().all(|r| matches!(r, Imported::Skipped(_))));
    assert_eq!(lib.load(&path).unwrap().description, "mine");
    // The mounting view survives the round trip through the file.
    assert!(lib.load(&path).unwrap().mounting().is_some());
}

#[test]
fn install_standard_adds_starters_and_circuits_without_clashes() {
    let dir = tempfile::tempdir().unwrap();
    let lib = MacroLibrary::new(dir.path().join("Macros"), "user");
    let all = lib.install_standard().unwrap();
    assert_eq!(all.len(), starter_macros().len() + library().len());
    assert!(all.iter().all(|r| matches!(r, Imported::Added(_))));
    assert!(!lib.search("hoist").is_empty());
}

#[test]
fn the_text_is_searchable_by_chapter_words_and_tags() {
    let dir = tempfile::tempdir().unwrap();
    let lib = MacroLibrary::new(dir.path().join("Macros"), "user");
    lib.install_circuits().unwrap();
    let hits = lib.search("rectifier");
    assert!(
        hits.iter().any(|e| e.folder.starts_with(CIRCUIT_ROOT)),
        "{hits:?}"
    );
}
