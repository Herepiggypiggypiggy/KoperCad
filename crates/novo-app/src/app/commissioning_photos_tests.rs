//! Photos on punch items: attached from files, kept beside the project,
//! carried in a field results file.

use novo_commissioning::{CommissioningStore, FieldPackage, PunchItem, PunchState, Severity};

use super::gate1_tests::Drive;
use crate::actions::Action;

fn example() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    // a project with a file, so the photos folder is beside it (not Documents)
    assert!(d.app.save_to(&dir.path().join("proj.novo")));
    d.frame();
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    (dir, d)
}

fn add_item(d: &mut Drive) -> u64 {
    d.app
        .wb
        .commissioning
        .as_mut()
        .unwrap()
        .store
        .add_punch(PunchItem {
            id: 0,
            title: "loose ferrule".into(),
            note: String::new(),
            severity: Severity::Minor,
            state: PunchState::Open,
            target: None,
            check: None,
            raised_by: "tv".into(),
            raised_at: "2026-10-16T09:00:00Z".into(),
            closed_by: None,
            closed_at: None,
            photos: Vec::new(),
            signature: None,
        })
}

#[test]
fn a_photo_is_copied_beside_the_project_and_named_by_content() {
    let (dir, mut d) = example();
    let id = add_item(&mut d);
    let picture = dir.path().join("IMG_0001.JPG");
    std::fs::write(&picture, b"not really a jpeg").unwrap();
    // the same picture twice is one photo
    assert_eq!(d.app.attach_photo_files(id, &[picture.clone(), picture]), 1);
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    let photos = &mode.store.punch(id).unwrap().photos;
    assert_eq!(photos.len(), 1);
    let kept = d.app.field_photos_dir().unwrap().join(&photos[0].name);
    assert_eq!(std::fs::read(kept).unwrap(), b"not really a jpeg");
    assert!(photos[0].name.ends_with(".jpg"), "{}", photos[0].name);
    // a file that cannot be read is skipped, not fatal
    assert_eq!(
        d.app
            .attach_photo_files(id, &[dir.path().join("missing.png")]),
        0
    );
}

#[test]
fn photos_travel_through_a_field_results_file() {
    let (dir, mut d) = example();
    let id = add_item(&mut d);
    let picture = dir.path().join("a.png");
    std::fs::write(&picture, [1u8, 2, 3, 4, 5]).unwrap();
    d.app.attach_photo_files(id, &[picture]);
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    let mut package = FieldPackage::new(
        "Example",
        "tablet",
        "2026-10-16T10:00:00Z",
        &mode.store,
        d.doc().project(),
    );
    d.app.pack_field_photos(&mut package);
    let file = dir.path().join("walk.kcfield");
    std::fs::write(&file, package.to_json()).unwrap();
    // the desk: the photos folder is gone and the store is empty again
    let photos_dir = d.app.field_photos_dir().unwrap();
    std::fs::remove_dir_all(&photos_dir).unwrap();
    d.app.wb.commissioning.as_mut().unwrap().store = CommissioningStore::new();
    d.app.import_field_results_from(&file);
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    let item = mode.store.punch_items().next().expect("the item came over");
    assert_eq!(item.photos.len(), 1);
    assert_eq!(
        std::fs::read(photos_dir.join(&item.photos[0].name)).unwrap(),
        [1u8, 2, 3, 4, 5],
        "the picture was written back"
    );
}
