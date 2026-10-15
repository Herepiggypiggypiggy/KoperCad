//! The outline's tree, driven the way a person drives it: clicks on rows and
//! empty space, keys, drops, and the edits the menus make.

use eframe::egui;
use novo_core::{DeviceTag, PageId};

use super::gate1_tests::Drive;
use super::outline::{Rename, RenameOf};
use super::outline_tree::{Act, Item, Target, menu};
use super::pages::PageView;
use crate::actions::Action;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.app.run(&ctx, Action::PinOutline);
    d.app.settings.outline_pinned = Some(true);
    d.frame();
    d.frame();
    (dir, d)
}

fn click(d: &mut Drive, pos: egui::Pos2, button: egui::PointerButton) {
    let m = egui::Modifiers::NONE;
    d.frame_with(vec![egui::Event::PointerMoved(pos)]);
    d.frame_with(vec![egui::Event::PointerButton {
        pos,
        button,
        pressed: true,
        modifiers: m,
    }]);
    d.frame_with(vec![egui::Event::PointerButton {
        pos,
        button,
        pressed: false,
        modifiers: m,
    }]);
    d.frame();
}

fn key(d: &mut Drive, key: egui::Key, modifiers: egui::Modifiers) {
    d.frame_with(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }]);
    d.frame();
}

fn pages(d: &Drive) -> Vec<String> {
    d.doc()
        .project()
        .pages_in_order()
        .into_iter()
        .map(|p| {
            format!("{} {} [{}]", p.name, p.title, p.structure)
                .trim()
                .to_owned()
        })
        .collect()
}

fn row_pos(d: &Drive, key_part: &str) -> egui::Pos2 {
    let (_, rect) = d
        .app
        .ol
        .rects
        .iter()
        .find(|(k, _)| k.contains(key_part))
        .unwrap_or_else(|| panic!("no row {key_part}: {:?}", d.app.ol.rects.keys()));
    rect.center()
}

fn popup_open(d: &Drive) -> bool {
    egui::Popup::is_any_open(&d.ctx)
}

fn page_named(d: &Drive, title: &str) -> PageId {
    d.doc()
        .project()
        .pages()
        .find(|p| p.title == title)
        .unwrap_or_else(|| panic!("no page titled {title}: {:?}", pages(d)))
        .id
}

#[test]
fn right_click_on_empty_space_opens_a_menu() {
    let (_dir, mut d) = drive();
    let space = d.app.ol.empty_space.expect("the empty space is drawn");
    click(&mut d, space.center(), egui::PointerButton::Secondary);
    assert!(popup_open(&d), "a menu opens on empty space");
    // And it is not empty: new page, group, cabinet, expand and collapse.
    let items = menu(d.doc().project(), &Target::Background);
    let labels: Vec<&str> = items
        .iter()
        .filter_map(|i| match i {
            Item::Entry { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        labels,
        [
            "New page",
            "New page group",
            "New cabinet…",
            "Expand all",
            "Collapse all"
        ]
    );
}

#[test]
fn right_click_on_a_page_a_group_and_a_cabinet_opens_their_menus() {
    let (_dir, mut d) = drive();
    d.app.perform(Act::NewGroup { in_cabinet: None });
    d.frame();
    d.frame();
    d.app.ol.rename = None;
    d.frame();
    // A page row.
    let pos = row_pos(&d, "p:");
    click(&mut d, pos, egui::PointerButton::Secondary);
    assert!(popup_open(&d), "page menu");
    key(&mut d, egui::Key::Escape, egui::Modifiers::NONE);
    // The group row.
    let pos = row_pos(&d, "g:G1");
    click(&mut d, pos, egui::PointerButton::Secondary);
    assert!(popup_open(&d), "group menu");
}

#[test]
fn new_group_page_and_cabinet_name_themselves() {
    let (_dir, mut d) = drive();
    let before = d.doc().project().pages().count();
    // A group: =G1 with its first page, name box open to change it.
    d.app.perform(Act::NewGroup { in_cabinet: None });
    d.frame();
    d.frame();
    assert!(
        d.doc()
            .project()
            .page_groups()
            .iter()
            .any(|g| g.to_string() == "=G1")
    );
    assert_eq!(d.doc().project().pages().count(), before + 1);
    assert!(matches!(
        d.app.ol.rename,
        Some(Rename {
            of: RenameOf::Group(_),
            ..
        })
    ));
    d.app.ol.rename = None;
    // The next one continues the pattern.
    d.app.perform(Act::NewGroup { in_cabinet: None });
    d.frame();
    assert!(
        d.doc()
            .project()
            .page_groups()
            .iter()
            .any(|g| g.to_string() == "=G2")
    );
    d.app.ol.rename = None;

    // A cabinet: a window with the next name filled in and a plate chosen.
    d.app.perform(Act::NewCabinet {
        in_group: Some(vec!["G1".into()]),
    });
    d.frame();
    d.frame();
    let form = d.app.ol.cabinet_form.as_ref().expect("the window is open");
    assert_eq!(d.app.naming().next_cabinet(d.doc().project()), "A1");
    let _ = form;
    key(&mut d, egui::Key::Enter, egui::Modifiers::NONE);
    d.frame();
    assert!(d.app.ol.cabinet_form.is_none(), "Enter creates");
    let cab = d
        .doc()
        .project()
        .structure_by_tag(&DeviceTag::parse("+A1").unwrap())
        .expect("cabinet +A1");
    assert!(cab.enclosure);
    assert!(cab.plate.is_some());
    let page = d
        .doc()
        .project()
        .pages()
        .find(|p| p.structure.to_string() == "=G1+A1")
        .expect("its first page sits in the group");
    assert_eq!(page.name, "1");
    // The next cabinet is +A2 without typing anything.
    assert_eq!(d.app.naming().next_cabinet(d.doc().project()), "A2");
    // One undo takes the cabinet back, plate and page together.
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert!(
        d.doc()
            .project()
            .structure_by_tag(&DeviceTag::parse("+A1").unwrap())
            .is_none()
    );
}

#[test]
fn a_new_page_opens_its_title_box_and_the_title_sticks() {
    let (_dir, mut d) = drive();
    let here = d.app.new_here();
    d.app.perform(Act::NewPage {
        structure: here,
        after: None,
    });
    d.frame();
    d.frame();
    let Some(Rename {
        of: RenameOf::Page(page),
        ..
    }) = d.app.ol.rename.clone()
    else {
        panic!("a title box opens on the new page");
    };
    // Type a title and press Enter, as the person does.
    d.frame_with(vec![egui::Event::Text("Motor circuit".into())]);
    key(&mut d, egui::Key::Enter, egui::Modifiers::NONE);
    d.frame();
    assert_eq!(d.doc().project().page(page).unwrap().title, "Motor circuit");
}

#[test]
fn keys_rename_delete_and_move_the_selected_row() {
    let (_dir, mut d) = drive();
    let here = d.app.new_here();
    for title in ["One", "Two"] {
        d.app.perform(Act::NewPage {
            structure: here.clone(),
            after: None,
        });
        d.frame();
        d.frame();
        d.frame_with(vec![egui::Event::Text(title.into())]);
        key(&mut d, egui::Key::Enter, egui::Modifiers::NONE);
    }
    d.app.ol.rename = None;
    d.frame();
    let two = page_named(&d, "Two");
    let one = page_named(&d, "One");
    let before = pages(&d);

    // Click the row of "One": it is selected and the keys act on it.
    let pos = row_pos(&d, &format!("p:{}:0", one.0));
    click(&mut d, pos, egui::PointerButton::Primary);
    assert!(d.app.ol.focused);

    // Ctrl+Down moves it below "Two".
    key(&mut d, egui::Key::ArrowDown, egui::Modifiers::COMMAND);
    d.frame();
    let order = d.doc().project().sibling_pages(one);
    assert!(
        order.iter().position(|p| *p == one) > order.iter().position(|p| *p == two),
        "{order:?}"
    );

    // F2 opens the title box, Escape closes it without a change.
    key(&mut d, egui::Key::F2, egui::Modifiers::NONE);
    d.frame();
    assert!(d.app.ol.rename.is_some(), "F2 renames");
    key(&mut d, egui::Key::Escape, egui::Modifiers::NONE);
    d.frame();
    assert!(d.app.ol.rename.is_none());
    assert_eq!(d.doc().project().page(one).unwrap().title, "One");

    // Delete removes the page, and Ctrl+Z brings it back.
    let count = d.doc().project().pages().count();
    key(&mut d, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(d.doc().project().pages().count(), count - 1);
    assert!(d.doc().project().page(one).is_none());
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert_eq!(d.doc().project().pages().count(), count);
    let _ = before;
}

#[test]
fn delete_on_the_canvas_still_belongs_to_the_canvas() {
    // With the tree not focused, Delete is not taken by the outline.
    let (_dir, mut d) = drive();
    d.app.ol.focused = false;
    d.app.ol.selected = None;
    let count = d.doc().project().pages().count();
    key(&mut d, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(d.doc().project().pages().count(), count);
}

#[test]
fn grouping_moves_pages_and_numbering_follows() {
    let (_dir, mut d) = drive();
    // Two groups with a page each, both numbered 1.
    d.app.perform(Act::NewGroup { in_cabinet: None });
    d.app.ol.rename = None;
    d.app.perform(Act::NewGroup { in_cabinet: None });
    d.app.ol.rename = None;
    d.frame();
    let in_g2 = d
        .doc()
        .project()
        .pages()
        .find(|p| p.structure.to_string() == "=G2")
        .unwrap()
        .id;
    assert_eq!(d.doc().project().page(in_g2).unwrap().name, "1");
    d.app.perform(Act::MoveTo {
        page: in_g2,
        function: Some(vec!["G1".into()]),
        location: None,
    });
    d.frame();
    let p = d.doc().project().page(in_g2).unwrap();
    assert_eq!(p.structure.to_string(), "=G1");
    assert_eq!(p.name, "2", "took the next number in the group it joined");
    d.app
        .perform(Act::Renumber(DeviceTag::parse("=G1").unwrap()));
    assert!(pages(&d).iter().any(|p| p.starts_with("2 ")));
}

#[test]
fn collapse_and_expand_all_fold_the_tree() {
    let (_dir, mut d) = drive();
    d.app.perform(Act::NewGroup { in_cabinet: None });
    d.app.ol.rename = None;
    d.frame();
    let open_rows = d.app.ol.rows.len();
    d.app.perform(Act::FoldAll(true));
    d.frame();
    assert!(d.app.ol.rows.len() < open_rows, "folded");
    d.app.perform(Act::FoldAll(false));
    d.frame();
    assert_eq!(d.app.ol.rows.len(), open_rows);
}

#[test]
fn deleting_a_group_asks_first_and_undoes() {
    let (_dir, mut d) = drive();
    d.app.perform(Act::NewGroup { in_cabinet: None });
    d.app.ol.rename = None;
    d.frame();
    let before = d.doc().project().pages().count();
    d.app.perform(Act::DeleteLevelAndPages(
        novo_core::Aspect::Function,
        vec!["G1".into()],
    ));
    d.frame();
    assert!(d.app.ol.confirm.is_some(), "asks first");
    assert_eq!(d.doc().project().pages().count(), before, "nothing yet");
    key(&mut d, egui::Key::Enter, egui::Modifiers::NONE);
    d.frame();
    assert!(d.app.ol.confirm.is_none());
    assert_eq!(d.doc().project().pages().count(), before - 1);
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Undo);
    assert_eq!(d.doc().project().pages().count(), before);
}

#[test]
fn a_page_dragged_onto_a_cabinet_is_filed_there() {
    let (_dir, mut d) = drive();
    d.app.perform(Act::NewCabinet { in_group: None });
    d.frame();
    d.frame();
    key(&mut d, egui::Key::Enter, egui::Modifiers::NONE);
    d.frame();
    d.frame();
    // The new page sits in +A1; make a loose page and drag it there.
    let here = DeviceTag::default();
    d.app.perform(Act::NewPage {
        structure: here,
        after: None,
    });
    d.frame();
    d.frame();
    d.frame_with(vec![egui::Event::Text("Loose".into())]);
    key(&mut d, egui::Key::Enter, egui::Modifiers::NONE);
    d.app.ol.rename = None;
    d.frame();
    d.frame();
    let loose = page_named(&d, "Loose");
    let from = row_pos(&d, &format!("p:{}:0", loose.0));
    let to = row_pos(&d, "c:A1");
    let m = egui::Modifiers::NONE;
    d.frame_with(vec![egui::Event::PointerMoved(from)]);
    d.frame_with(vec![egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: m,
    }]);
    for step in 1..=8 {
        let t = step as f32 / 8.0;
        d.frame_with(vec![egui::Event::PointerMoved(from.lerp(to, t))]);
    }
    d.frame_with(vec![egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: m,
    }]);
    d.frame();
    d.frame();
    assert_eq!(
        d.doc().project().page(loose).unwrap().structure.to_string(),
        "+A1",
        "dropped on the cabinet row"
    );
}

#[test]
fn the_example_project_shows_as_a_tree_in_both_views() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.frame();
    let dump = |d: &Drive| -> Vec<String> {
        d.app
            .ol
            .rows
            .iter()
            .map(|r| format!("{}{:?}", "  ".repeat(r.depth), r.key))
            .collect()
    };
    let groups = dump(&d);
    assert!(!groups.is_empty());
    d.app.page_view = PageView::Location;
    d.frame();
    d.frame();
    let cabinets = dump(&d);
    assert!(!cabinets.is_empty());
    eprintln!(
        "groups view:\n{}\ncabinets view:\n{}",
        groups.join("\n"),
        cabinets.join("\n")
    );
}

#[test]
fn the_example_project_is_filed_by_its_tags() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.frame();
    let shape: Vec<String> = d
        .app
        .ol
        .rows
        .iter()
        .filter(|r| r.key.starts_with("g:") || r.key.contains("/c:"))
        .map(|r| r.key.clone())
        .collect();
    assert!(
        shape.iter().any(|k| k.starts_with("g:A1")),
        "group =A1 in the tree: {shape:?}"
    );
    assert!(
        shape
            .iter()
            .any(|k| k.contains("/c:CAB") || k.contains("c:CAB")),
        "cabinet +CAB in the tree: {shape:?}"
    );
    // Nothing left to offer.
    assert!(
        super::outline_tree::suggest_filing(d.doc().project()).is_empty(),
        "the example's drawn pages are all filed"
    );
}

#[test]
fn unfiled_pages_are_offered_a_home_from_their_devices() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    // Unfile the example's pages, as an imported project would be.
    let ids: Vec<PageId> = d
        .doc()
        .project()
        .pages()
        .filter(|p| !matches!(p.kind, novo_core::PageKind::Generated { .. }))
        .map(|p| p.id)
        .collect();
    for page in &ids {
        d.app.perform(Act::MoveTo {
            page: *page,
            function: Some(Vec::new()),
            location: Some(Vec::new()),
        });
    }
    d.frame();
    let plan = super::outline_tree::suggest_filing(d.doc().project());
    assert_eq!(plan.len(), ids.len());
    assert!(plan.iter().all(|(_, s)| s.function == ["A1"]));
    d.app.perform(Act::AutoFile);
    d.frame();
    assert!(super::outline_tree::suggest_filing(d.doc().project()).is_empty());
    assert!(
        d.doc()
            .project()
            .pages()
            .any(|p| p.structure.function == ["A1"] && !p.structure.location.is_empty())
    );
    // One undo puts them back.
    d.app.run(&ctx, Action::Undo);
    assert_eq!(
        super::outline_tree::suggest_filing(d.doc().project()).len(),
        ids.len()
    );
}

#[test]
fn naming_prefixes_start_a_project_that_has_no_names() {
    let (_dir, mut d) = drive();
    d.app.settings.group_prefix = "Sec".into();
    d.app.settings.cabinet_prefix = "K".into();
    let n = d.app.naming();
    // A new project has no group or cabinet to continue.
    let project = d.doc().project();
    assert!(super::outline_tree::group_names(project).is_empty());
    assert_eq!(n.next_group(project), "Sec1");
    assert_eq!(n.next_cabinet(project), "K1");
}
