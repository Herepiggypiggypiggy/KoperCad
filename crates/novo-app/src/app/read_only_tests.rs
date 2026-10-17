//! File > Open read-only: the project can be looked at, never changed or
//! written, and says so in the title and the status bar.

use super::gate1_tests::Drive;
use crate::actions::Action;

fn saved_project(dir: &std::path::Path) -> (Drive, std::path::PathBuf) {
    let mut d = Drive::new(&dir.join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    let path = dir.join("plant.novo");
    assert!(d.app.save_to(&path));
    d.app.run(&ctx, Action::CloseProject);
    d.frame();
    (d, path)
}

fn add_page(d: &mut Drive, name: &str) {
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Add page", |tx| tx.add_page(name))
        .unwrap();
}

fn page_count(d: &Drive) -> usize {
    d.app.backend.document().unwrap().project().pages().count()
}

#[test]
fn open_read_only_shows_it_and_takes_edits_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut d, path) = saved_project(dir.path());
    let before = std::fs::read(&path).unwrap();
    d.app.open_path_as(&path, true);
    d.frame();
    d.frame();
    assert!(d.app.backend.is_read_only());
    assert_eq!(d.app.saved_state(), "Read-only");
    assert_eq!(d.app.window_title, "plant (read-only) - KoperCAD");
    assert!(!d.app.is_enabled(Action::SaveProject));
    assert!(d.app.is_enabled(Action::SaveProjectAs));
    assert!(!d.app.is_enabled(Action::Undo) && !d.app.is_enabled(Action::Redo));

    let pages = page_count(&d);
    add_page(&mut d, "2");
    assert_eq!(page_count(&d), pages + 1);
    d.frame();
    assert_eq!(page_count(&d), pages, "the edit was taken back");
    assert!(
        d.app
            .messages()
            .iter()
            .any(|m| m.text.contains("open read-only")),
        "the user is told why"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before, "file untouched");
}

#[test]
fn the_command_is_in_the_file_menu_and_the_palette() {
    let file = novo_workflow::menus::layout("File");
    let found = file.iter().any(|item| {
        matches!(item, novo_workflow::menus::MenuItem::Submenu { name: "Open", items }
            if items.iter().any(|c| c.id == "file.open_read_only"))
    });
    assert!(found, "File > Open > Open read-only");
    assert_eq!(
        Action::from_id("file.open_read_only"),
        Some(Action::OpenReadOnly)
    );
}

#[test]
fn save_as_gives_an_editable_copy() {
    let dir = tempfile::tempdir().unwrap();
    let (mut d, path) = saved_project(dir.path());
    d.app.open_path_as(&path, true);
    d.frame();
    let copy = dir.path().join("copy.novo");
    assert!(d.app.save_to(&copy));
    d.frame();
    assert!(!d.app.backend.is_read_only());
    assert_eq!(d.app.saved_state(), "Saved");
    let pages = page_count(&d);
    add_page(&mut d, "2");
    d.frame();
    assert_eq!(page_count(&d), pages + 1, "editable again");
}

#[test]
fn a_normal_open_is_not_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let (mut d, path) = saved_project(dir.path());
    d.app.open_path(&path);
    d.frame();
    assert!(!d.app.backend.is_read_only());
    assert!(d.app.is_enabled(Action::SaveProject));
}

#[test]
fn open_read_only_is_not_added_to_recent() {
    let dir = tempfile::tempdir().unwrap();
    let (mut d, path) = saved_project(dir.path());
    d.app.settings.recent_projects.clear();
    d.app.open_path_as(&path, true);
    assert!(d.app.settings.recent_projects.is_empty());
    d.app.open_path(&path);
    assert_eq!(d.app.settings.recent_projects.len(), 1);
}
