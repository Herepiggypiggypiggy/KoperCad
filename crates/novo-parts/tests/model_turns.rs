//! The quarter turns a user sets for a part's model are kept with the part
//! in the local library, next to its source records.

use novo_parts::*;

fn key() -> PartKey {
    PartKey::new("Example Maker", "9000-0001")
}

#[test]
fn turns_are_stored_per_part_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    assert_eq!(lib.turn_of(&key()), [0; 3]);
    lib.set_turns(&key(), [1, 0, 3]).unwrap();
    assert_eq!(lib.turn_of(&key()), [1, 0, 3]);
    // Spelled differently, the same part.
    assert_eq!(
        lib.turn_of(&PartKey::new("example maker", "9000 0001")),
        [1, 0, 3]
    );
    assert_eq!(lib.turn_of(&PartKey::new("Example Maker", "other")), [0; 3]);
}

#[test]
fn counts_wrap_and_all_zero_forgets_the_part() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    lib.set_turns(&key(), [5, 4, 2]).unwrap();
    assert_eq!(lib.turn_of(&key()), [1, 0, 2]);
    lib.set_turns(&key(), [4, 0, 0]).unwrap();
    assert!(lib.turns().unwrap().is_empty());
}

#[test]
fn removing_the_part_forgets_its_turns() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    lib.set_turns(&key(), [0, 1, 0]).unwrap();
    lib.remove(&key()).unwrap();
    assert_eq!(lib.turn_of(&key()), [0; 3]);
}

#[test]
fn a_records_file_from_before_turns_still_reads() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("records.json"),
        r#"{"format":1,"records":[],"notices":["Example Maker"]}"#,
    )
    .unwrap();
    let lib = LocalLibrary::at(dir.path());
    assert!(lib.notice_accepted("Example Maker"));
    assert_eq!(lib.turn_of(&key()), [0; 3]);
}

#[test]
fn a_hand_edited_count_is_folded_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    lib.set_turns(&key(), [1, 2, 3]).unwrap();
    let path = dir.path().join("records.json");
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let table = json["turns"].as_object_mut().unwrap();
    let name = table.keys().next().unwrap().clone();
    table.insert(name, serde_json::json!([301, 2, 3]));
    std::fs::write(&path, json.to_string()).unwrap();
    assert_eq!(lib.turn_of(&key()), [1, 2, 3]);
}
