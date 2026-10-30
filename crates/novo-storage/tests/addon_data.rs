//! Add-on data survives save and open byte for byte, including for an add-on this program has
//! never heard of, and a file written before the block existed still opens.

use novo_core::{AddonBlock, Document};
use novo_storage::ProjectFile;

#[test]
fn blocks_round_trip_through_the_project_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.novo");
    let mut doc = Document::default();
    let text = "line one\nünïcode ✓ {\"json\": [1,2]}\n";
    doc.transact("Store", |tx| {
        tx.set_addon_block(
            "never-installed",
            "state",
            AddonBlock {
                format: 7,
                text: text.into(),
            },
        )
    })
    .unwrap();
    drop(ProjectFile::create(&path, doc.project()).unwrap());

    let loaded = ProjectFile::open(&path).unwrap().load().unwrap();
    let block = loaded
        .info()
        .addon_data
        .get("never-installed", "state")
        .expect("block kept");
    assert_eq!((block.format, block.text.as_str()), (7, text));

    // saved again by a session that knows nothing of the add-on: still there
    {
        let mut file = ProjectFile::open(&path).unwrap();
        file.save(&loaded).unwrap();
    }
    let again = ProjectFile::open(&path).unwrap().load().unwrap();
    assert_eq!(again.info().addon_data, loaded.info().addon_data);
}

#[test]
fn a_project_without_add_on_data_opens_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.novo");
    drop(ProjectFile::create(&path, Document::default().project()).unwrap());
    let loaded = ProjectFile::open(&path).unwrap().load().unwrap();
    assert!(loaded.info().addon_data.is_empty());
}
