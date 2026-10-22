//! File > Open > Recover a damaged project: reachable from the palette, the
//! menu and the start screen, and it says where the recovered copy is.

use super::gate1_tests::Drive;
use crate::actions::Action;

fn damaged_project(dir: &std::path::Path) -> (Drive, std::path::PathBuf, Vec<u8>) {
    let mut d = Drive::new(&dir.join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Pages", |tx| {
            for n in 2..=150 {
                tx.add_page(n.to_string())?;
            }
            Ok(())
        })
        .unwrap();
    let path = dir.join("plant.novo");
    assert!(d.app.save_to(&path));
    d.app.run(&ctx, Action::CloseProject);
    d.frame();
    let bytes = std::fs::read(&path).unwrap();
    let cut = bytes[..bytes.len() * 6 / 10].to_vec();
    std::fs::write(&path, &cut).unwrap();
    (d, path, cut)
}

#[test]
fn the_command_is_in_the_menu_the_palette_and_the_action_list() {
    let file = novo_workflow::menus::layout("File");
    let in_menu = file.iter().any(|item| {
        matches!(item, novo_workflow::menus::MenuItem::Submenu { name: "Open", items }
            if items.iter().any(|c| c.id == "file.recover"))
    });
    assert!(in_menu, "File > Open > Recover a damaged project");
    assert_eq!(
        Action::from_id("file.recover"),
        Some(Action::RecoverProject)
    );
    assert_eq!(Action::RecoverProject.label(), "Recover a damaged project…");
    assert_eq!(
        novo_workflow::commands::get("file.recover").map(|c| c.label),
        Some("Recover a damaged project")
    );
}

#[test]
fn recovering_names_the_copy_and_leaves_the_damaged_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (mut d, path, cut) = damaged_project(dir.path());
    d.app.open_salvaged(&path);
    d.frame();
    let copy = dir.path().join("plant.recovered.novo");
    assert_eq!(d.app.project_path.as_deref(), Some(copy.as_path()));
    let said = d
        .app
        .messages()
        .iter()
        .map(|m| m.text.clone())
        .find(|t| t.contains("recovered copy"))
        .expect("a message says what happened");
    assert!(said.contains("plant.recovered.novo"), "{said}");
    assert!(said.contains("beside the original"), "{said}");
    assert!(said.contains("Recover a damaged project"), "{said}");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        cut,
        "the original is untouched"
    );
}

#[test]
fn recovering_a_file_with_nothing_readable_says_so_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("junk.novo");
    std::fs::write(&path, b"this is not a project file at all").unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.open_salvaged(&path);
    d.frame();
    assert!(d.app.project_path.is_none());
    assert!(
        d.app
            .messages()
            .iter()
            .any(|m| m.text.contains("could not be recovered")
                || m.text.contains("nothing could be read")),
        "{:?}",
        d.app.messages()
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"this is not a project file at all"
    );
}

#[test]
fn recovering_a_healthy_file_opens_it_as_itself_and_writes_no_copy() {
    let dir = tempfile::tempdir().unwrap();
    let (mut d, path, cut) = damaged_project(dir.path());
    // A healthy project next to the damaged one.
    let healthy = dir.path().join("fine.novo");
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    assert!(d.app.save_to(&healthy));
    d.app.run(&ctx, Action::CloseProject);
    d.frame();
    let before = std::fs::read(&healthy).unwrap();

    d.app.recover_path(&healthy);
    d.frame();
    assert_eq!(d.app.project_path.as_deref(), Some(healthy.as_path()));
    assert!(!dir.path().join("fine.recovered.novo").exists());
    let texts: Vec<String> = d.app.messages().iter().map(|m| m.text.clone()).collect();
    assert!(
        texts.iter().any(|t| t.contains("is not damaged")),
        "{texts:?}"
    );
    assert!(!texts.iter().any(|t| t.contains("is damaged")), "{texts:?}");
    d.app.run(&ctx, Action::CloseProject);
    assert_eq!(std::fs::read(&healthy).unwrap(), before, "file untouched");

    // The damaged one still goes to salvage.
    d.app.recover_path(&path);
    d.frame();
    assert!(dir.path().join("plant.recovered.novo").exists());
    assert_eq!(std::fs::read(&path).unwrap(), cut);
}

#[test]
fn a_recovered_copy_is_not_called_saved_or_clean_until_it_is_checked() {
    let dir = tempfile::tempdir().unwrap();
    let (mut d, path, _cut) = damaged_project(dir.path());
    d.app.open_salvaged(&path);
    d.frame();
    // No green claim: the status says it is a recovered copy, and the
    // issue pill is the recovered-copy pill while nothing is flagged.
    assert_eq!(d.app.saved_state(), "Saved (recovered copy)");
    let lines = d.app.wb.recovered.clone().expect("flagged as unchecked");
    let said = lines.join("\n");
    assert!(
        said.contains("In the copy: 25 pages, 0 symbols, 0 wires"),
        "{said}"
    );
    assert!(said.contains("The end of the file was lost"), "{said}");
    // The message carries the same account.
    let toast = d
        .app
        .messages()
        .iter()
        .map(|m| m.text.clone())
        .find(|t| t.contains("recovered copy"))
        .unwrap();
    assert!(toast.contains("In the copy: 25 pages"), "{toast}");
    assert!(toast.contains("Check every page"), "{toast}");
    // Another project is not a recovered copy.
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::CloseProject);
    d.frame();
    assert!(d.app.wb.recovered.is_none());
}
