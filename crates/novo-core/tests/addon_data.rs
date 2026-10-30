//! Add-on data in the project: any add-on id can keep named blocks that the program never
//! interprets, edits are undoable, and bad names or oversized blocks are refused.

use novo_core::{AddonBlock, AddonData, Document, ProjectInfo};

fn block(text: &str) -> AddonBlock {
    AddonBlock {
        format: 1,
        text: text.into(),
    }
}

#[test]
fn a_block_is_stored_read_back_and_undone() {
    let mut doc = Document::default();
    doc.transact("Store", |tx| {
        tx.set_addon_block("controllers", "project", block("hello"))
    })
    .unwrap();
    let data = &doc.project().info().addon_data;
    assert_eq!(data.get("controllers", "project"), Some(&block("hello")));
    assert_eq!(data.addons().collect::<Vec<_>>(), ["controllers"]);

    doc.undo().unwrap();
    assert!(doc.project().info().addon_data.is_empty());
}

#[test]
fn empty_text_removes_the_block_and_the_empty_addon() {
    let mut data = AddonData::default();
    data.set("a", "x", block("1")).unwrap();
    data.set("a", "y", block("2")).unwrap();
    data.set("a", "x", block("")).unwrap();
    assert_eq!(data.blocks("a").count(), 1);
    data.set("a", "y", block("")).unwrap();
    assert!(data.is_empty());
}

#[test]
fn names_and_size_are_checked() {
    let mut doc = Document::default();
    for bad in ["", "Upper", "has space", "a/b", &"x".repeat(65)] {
        assert!(
            doc.transact("Store", |tx| tx.set_addon_block(bad, "k", block("t")))
                .is_err(),
            "{bad:?} accepted"
        );
    }
    let big = block(&"x".repeat(AddonData::MAX_BLOCK_BYTES + 1));
    assert!(
        doc.transact("Store", |tx| tx.set_addon_block("a", "k", big))
            .is_err()
    );
    assert!(doc.project().info().addon_data.is_empty());
    assert_eq!(doc.undo_depth(), 0, "a refused edit leaves no undo step");
}

#[test]
fn removing_an_addon_drops_all_its_blocks_in_one_step() {
    let mut doc = Document::default();
    doc.transact("Store", |tx| {
        tx.set_addon_block("a", "x", block("1"))?;
        tx.set_addon_block("a", "y", block("2"))?;
        tx.set_addon_block("b", "x", block("3"))
    })
    .unwrap();
    let (n, _) = doc
        .transact("Remove", |tx| tx.remove_addon_data("a"))
        .unwrap();
    assert_eq!(n, 2);
    assert_eq!(
        doc.project().info().addon_data.addons().collect::<Vec<_>>(),
        ["b"]
    );
}

#[test]
fn info_without_addon_data_reads_and_writes_as_before() {
    let json = serde_json::to_string(&ProjectInfo::default()).unwrap();
    assert!(
        !json.contains("addon_data"),
        "empty data adds nothing: {json}"
    );
    let old: ProjectInfo = serde_json::from_str(r#"{"name":"P"}"#).unwrap();
    assert!(old.addon_data.is_empty());
}
