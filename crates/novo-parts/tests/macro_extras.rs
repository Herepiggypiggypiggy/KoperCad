use novo_core::{BlockSource, Record};
use novo_parts::{
    MACRO_BASE_VERSION, MACRO_FORMAT_VERSION, MacroFile, MacroLibrary, MacroPart, macro_updates,
    starter_macros, thumbnail,
};

fn part(device: u64, order: &str) -> MacroPart {
    MacroPart {
        device,
        manufacturer: "Acme".into(),
        order_number: order.into(),
        type_number: "T1".into(),
        description: "Contactor".into(),
    }
}

fn starter() -> MacroFile {
    starter_macros().into_iter().next().unwrap().1
}

fn lib() -> (tempfile::TempDir, MacroLibrary) {
    let dir = tempfile::tempdir().unwrap();
    let lib = MacroLibrary::new(dir.path().join("Macros"), "company");
    (dir, lib)
}

#[test]
fn parts_round_trip_and_bump_the_version() {
    let mut f = starter();
    assert_eq!(f.version, MACRO_BASE_VERSION);
    assert!(f.parts().is_empty());
    f.set_parts(&[part(3, "ABC-1"), part(4, "ABC-2")]);
    assert_eq!(f.version, 2);
    let back = MacroFile::from_json(&f.to_json(), "x.kcmacro".as_ref()).unwrap();
    assert_eq!(back.parts(), vec![part(3, "ABC-1"), part(4, "ABC-2")]);
    f.set_parts(&[]);
    assert_eq!(f.version, MACRO_BASE_VERSION);
    assert!(!f.extra.contains_key("parts"));
}

#[test]
fn files_without_parts_stay_version_one_and_old_files_load() {
    assert_eq!(starter().version, 1);
    let old = starter().to_json();
    let f = MacroFile::from_json(&old, "old.kcmacro".as_ref()).unwrap();
    assert!(f.parts().is_empty());
}

#[test]
fn a_newer_version_is_refused() {
    let text = starter().to_json().replace(
        "\"version\": 1",
        &format!("\"version\": {}", MACRO_FORMAT_VERSION + 1),
    );
    assert!(MacroFile::from_json(&text, "n.kcmacro".as_ref()).is_err());
}

#[test]
fn unreadable_part_entries_are_skipped() {
    let mut f = starter();
    f.extra.insert(
        "parts".into(),
        serde_json::json!([{"device": 1}, {"order_number": "X"}, {"device": 2, "order_number": "OK"}]),
    );
    let parts = f.parts();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].order_number, "OK");
}

#[test]
fn listing_counts_parts_and_packs_carry_the_version() {
    let (_d, lib) = lib();
    let mut f = starter();
    f.set_parts(&[part(1, "P")]);
    let path = lib.save("", &f).unwrap();
    assert_eq!(lib.list().0[0].parts, 1);
    let dest = _d.path().join("p.kcmacropack");
    lib.export_pack(&[path.as_str()], &dest).unwrap();
    let other = MacroLibrary::new(_d.path().join("Other"), "user");
    other
        .import(&dest, "", novo_parts::OnConflict::KeepBoth)
        .unwrap();
    assert_eq!(other.list().0[0].parts, 1);
}

#[test]
fn thumbnail_has_every_symbol_wire_and_a_bounding_box() {
    let (_, f) = starter_macros()
        .into_iter()
        .find(|(_, f)| {
            f.definition
                .members
                .iter()
                .any(|m| matches!(m, Record::Wire(_)))
        })
        .expect("a starter with wires");
    let t = thumbnail(&f.definition);
    let symbols = f
        .definition
        .members
        .iter()
        .filter(|m| matches!(m, Record::Symbol(_)))
        .count();
    assert_eq!(t.symbols.len(), symbols);
    assert!(!t.wires.is_empty());
    let (min, max) = t.bounds.unwrap();
    assert!(max.x > min.x && max.y > min.y);
    for p in t.wires.iter().flatten() {
        assert!(p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y);
    }
}

#[test]
fn an_empty_macro_has_no_thumbnail_box() {
    let mut f = starter();
    f.definition.members.clear();
    assert!(thumbnail(&f.definition).bounds.is_none());
}

#[test]
fn updates_are_offered_only_for_newer_readable_files() {
    let (_d, lib) = lib();
    let path = lib.save("", &starter()).unwrap();
    let mut newer = lib.load(&path).unwrap();
    newer.revision = 3;
    lib.replace(&path, &newer).unwrap();
    let rev = lib.load(&path).unwrap().revision;
    assert!(rev >= 2);
    let old = BlockSource {
        library: format!("company/{path}"),
        version: 1,
    };
    let current = BlockSource {
        library: format!("company/{path}"),
        version: rev,
    };
    let other_lib = BlockSource {
        library: format!("user/{path}"),
        version: 1,
    };
    let gone = BlockSource {
        library: "company/Nope.kcmacro".into(),
        version: 1,
    };
    let ups = macro_updates(
        &lib,
        [
            ("a", &old),
            ("b", &current),
            ("c", &other_lib),
            ("d", &gone),
        ],
    );
    assert_eq!(ups.len(), 1);
    assert_eq!(ups[0].definition, "a");
    assert_eq!(ups[0].project_revision, 1);
    assert_eq!(ups[0].library_revision, rev);
}
