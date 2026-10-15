use novo_core::{BlockDefinition, BlockDefinitionId, BlockParameter, Value};
use novo_parts::{Imported, MacroError, MacroFile, MacroLibrary, OnConflict, user_macros_dir};

fn def(name: &str) -> BlockDefinition {
    BlockDefinition {
        id: BlockDefinitionId(1),
        name: name.into(),
        description: "Direct on line starter".into(),
        parameters: vec![BlockParameter {
            name: "power".into(),
            default: Value::Float(4.0),
            description: String::new(),
        }],
        members: vec![],
        source: None,
        properties: Default::default(),
    }
}

fn file(name: &str) -> MacroFile {
    MacroFile::new(def(name))
        .with_author("TV")
        .with_tags(&["motor", "starter"])
}

fn lib() -> (tempfile::TempDir, MacroLibrary) {
    let dir = tempfile::tempdir().unwrap();
    let lib = MacroLibrary::new(dir.path().join("Macros"), "user");
    (dir, lib)
}

#[test]
fn save_list_load_round_trip() {
    let (_d, lib) = lib();
    let path = lib.save("Motors", &file("DOL")).unwrap();
    assert_eq!(path, "Motors/DOL.kcmacro");
    let (entries, problems) = lib.list();
    assert!(problems.is_empty());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].folder, "Motors");
    assert_eq!(entries[0].parameters, 1);
    assert_eq!(lib.load(&path).unwrap(), file("DOL"));
    assert_eq!(lib.folders(), vec!["Motors"]);
}

#[test]
fn save_refuses_a_taken_name_and_replace_bumps_the_revision() {
    let (_d, lib) = lib();
    let path = lib.save("", &file("DOL")).unwrap();
    assert!(matches!(
        lib.save("", &file("DOL")),
        Err(MacroError::Exists(_))
    ));
    lib.replace(&path, &file("DOL")).unwrap();
    assert_eq!(lib.load(&path).unwrap().revision, 2);
    let def = lib.definition_for_project(&path).unwrap();
    let source = def.source.unwrap();
    assert_eq!(source.library, "user/DOL.kcmacro");
    assert_eq!(source.version, 2);
}

#[test]
fn search_matches_every_word_and_puts_name_hits_first() {
    let (_d, lib) = lib();
    lib.save("", &file("Star-delta")).unwrap();
    lib.save(
        "Safety",
        &MacroFile::new(def("E-stop chain")).with_tags(&["motor"]),
    )
    .unwrap();
    let hits = lib.search("motor");
    assert_eq!(hits.len(), 2);
    assert_eq!(lib.search("stop motor").len(), 1);
    assert_eq!(lib.search("safety").len(), 1);
    assert_eq!(lib.search("").len(), 2);
    assert!(lib.search("nothing").is_empty());
}

#[test]
fn rename_move_delete() {
    let (_d, lib) = lib();
    let p = lib.save("A", &file("One")).unwrap();
    let p = lib.rename(&p, "Uno").unwrap();
    assert_eq!(p, "A/Uno.kcmacro");
    let loaded = lib.load(&p).unwrap();
    assert_eq!(
        (loaded.name.as_str(), loaded.definition.name.as_str()),
        ("Uno", "Uno")
    );
    let p = lib.move_to(&p, "B/C").unwrap();
    assert_eq!(p, "B/C/Uno.kcmacro");
    lib.save("B", &file("Dos")).unwrap();
    assert!(lib.rename(&p, "Dos").is_ok(), "other folder, no clash");
    lib.delete_folder("A").unwrap();
    assert!(matches!(
        lib.delete_folder("B"),
        Err(MacroError::NotEmpty(_))
    ));
    lib.delete("B/Dos.kcmacro").unwrap();
    assert!(matches!(
        lib.delete("B/Dos.kcmacro"),
        Err(MacroError::NotFound(_))
    ));
}

#[test]
fn names_cannot_escape_the_library() {
    let (_d, lib) = lib();
    assert!(lib.save("../outside", &file("x")).is_err());
    assert!(
        lib.save("/abs", &file("x")).is_ok(),
        "leading slash is just the top"
    );
    assert!(lib.save("", &file("CON")).is_err());
    assert!(lib.load("../x.kcmacro").is_err());
    assert!(lib.delete("..\\x.kcmacro").is_err());
    let p = lib.save("", &file("a/b:c?")).unwrap();
    assert_eq!(p, "a_b_c_.kcmacro");
}

#[test]
fn export_import_between_libraries_with_conflicts() {
    let (d, a) = lib();
    let b = MacroLibrary::new(d.path().join("Company"), "company");
    let p = a.save("Motors", &file("DOL")).unwrap();
    let out = d.path().join("share").join("DOL.kcmacro");
    a.export(&p, &out).unwrap();

    let r = b.import(&out, "Imported", OnConflict::Skip).unwrap();
    assert_eq!(r, vec![Imported::Added("Imported/DOL.kcmacro".into())]);
    let r = b.import(&out, "Imported", OnConflict::Skip).unwrap();
    assert_eq!(r, vec![Imported::Skipped("Imported/DOL.kcmacro".into())]);
    let r = b.import(&out, "Imported", OnConflict::KeepBoth).unwrap();
    assert_eq!(r, vec![Imported::Added("Imported/DOL 2.kcmacro".into())]);
    assert_eq!(
        b.load("Imported/DOL 2.kcmacro").unwrap().definition.name,
        "DOL 2"
    );
    let r = b.import(&out, "Imported", OnConflict::Replace).unwrap();
    assert_eq!(r, vec![Imported::Replaced("Imported/DOL.kcmacro".into())]);
    assert_eq!(b.load("Imported/DOL.kcmacro").unwrap().revision, 2);
}

#[test]
fn pack_round_trip_keeps_folders() {
    let (d, a) = lib();
    let p1 = a.save("Motors", &file("DOL")).unwrap();
    let p2 = a.save("", &file("Supply")).unwrap();
    let pack = d.path().join("team.kcmacropack");
    a.export_pack(&[&p1, &p2], &pack).unwrap();
    let b = MacroLibrary::new(d.path().join("Other"), "user");
    let r = b.import(&pack, "", OnConflict::Skip).unwrap();
    assert_eq!(r.len(), 2);
    assert!(b.load("Motors/DOL.kcmacro").is_ok());
    assert!(b.load("Supply.kcmacro").is_ok());
}

#[test]
fn bad_and_newer_files_are_reported_not_hidden() {
    let (_d, lib) = lib();
    lib.save("", &file("Good")).unwrap();
    let root = lib.root().to_path_buf();
    std::fs::write(root.join("junk.kcmacro"), "not json").unwrap();
    let mut newer = file("Newer");
    newer.version = 99;
    std::fs::write(root.join("newer.kcmacro"), newer.to_json()).unwrap();
    std::fs::write(root.join("notes.txt"), "ignored").unwrap();
    let (entries, problems) = lib.list();
    assert_eq!(entries.len(), 1);
    assert_eq!(problems.len(), 2);
    assert!(
        problems
            .iter()
            .any(|p| matches!(p, MacroError::TooNew { .. }))
    );
}

#[test]
fn the_user_folder_is_in_the_profile() {
    if let Some(dir) = user_macros_dir() {
        assert!(dir.ends_with("Macros") || dir.ends_with("macros"));
    }
}
