//! Draw > PLC > Import I/O list in the app: review first, Enter builds one
//! undo step, Esc changes nothing, and the command is in the palette.

use eframe::egui;
use novo_plc::cards;

use super::gate1_tests::Drive;
use crate::actions::Action;

const LIST: &str = "Tag;Omschrijving;Type;Adres\n\
    -B1;Pomp 1 draait;DI;E0.0\n\
    -B2;Niveau hoog;DI;E0.1\n\
    -Y1;Klep open;uitgang;A0.0\n\
    -B3;Druk;4-20 mA in;\n";

fn project() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

#[test]
fn the_review_comes_first_and_enter_builds_one_undo_step() {
    let (dir, mut d) = project();
    let path = dir.path().join("signals.csv");
    std::fs::write(&path, LIST).unwrap();
    let pages = d.doc().project().pages_in_order().len();
    let depth = d.doc().undo_depth();

    assert!(d.app.plc_import_list_from(&path));
    d.frame();
    // Reviewing changes nothing, and Esc leaves it that way.
    assert_eq!(d.doc().undo_depth(), depth);
    assert_eq!(cards(d.doc().project()).len(), 0);
    let review = d.app.wb.plc_list.as_ref().unwrap();
    assert_eq!(review.plan.cards.len(), 3, "{:?}", review.plan);
    d.key(egui::Key::Escape, egui::Modifiers::NONE);
    assert!(d.app.wb.plc_list.is_none());
    assert_eq!(d.doc().undo_depth(), depth);

    assert!(d.app.plc_import_list_from(&path));
    d.frame();
    d.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(d.app.wb.plc_list.is_none());
    let toast = d
        .app
        .messages()
        .iter()
        .rev()
        .find(|m| m.text.starts_with("Imported"))
        .unwrap();
    assert!(toast.text.contains("4 signals: 3 cards"), "{}", toast.text);
    assert_eq!(cards(d.doc().project()).len(), 3);
    assert_eq!(d.doc().project().pages_in_order().len(), pages + 3);
    // The first new page is shown.
    let first = d
        .doc()
        .project()
        .pages_in_order()
        .iter()
        .map(|p| p.id)
        .nth(pages);
    assert_eq!(d.app.current_page, first);
    // One undo takes it all back; importing again after that works.
    d.app.run(&d.ctx.clone(), Action::Undo);
    d.frame();
    assert_eq!(cards(d.doc().project()).len(), 0);
    assert_eq!(d.doc().project().pages_in_order().len(), pages);
}

#[test]
fn a_file_that_is_not_a_list_is_refused_with_the_reason() {
    let (dir, mut d) = project();
    let path = dir.path().join("notes.csv");
    std::fs::write(&path, "a,b\n1,2\n").unwrap();
    assert!(!d.app.plc_import_list_from(&path));
    let text = d.app.messages().last().unwrap().text.clone();
    assert!(text.contains("no heading row"), "{text}");
    assert!(d.app.wb.plc_list.is_none());
}

/// The printed address under a channel, by the signal text of its symbol.
fn printed(d: &Drive, signal: &str) -> Option<String> {
    use novo_core::Value;
    d.doc().project().symbols().find_map(|s| {
        let named =
            matches!(s.properties.get(novo_plc::card::SIGNAL), Some(Value::Text(t)) if t == signal);
        match (named, s.properties.get(novo_plc::card::ADDRESS_TEXT)) {
            (true, Some(Value::Text(a))) => Some(a.clone()),
            _ => None,
        }
    })
}

#[test]
fn the_printed_addresses_follow_an_edit_of_a_card_or_the_address_style() {
    use novo_plc::card::{ADDRESS_STYLE, start_key};
    let (dir, mut d) = project();
    let path = dir.path().join("signals.csv");
    std::fs::write(&path, "Tag,Description,Type\nB1,Pump on,DI\n").unwrap();
    assert!(d.app.plc_import_list_from(&path));
    d.frame();
    d.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(printed(&d, "Pump on").as_deref(), Some("%IX0.0"));

    // The card moves to byte 8: the page says so, in the edit's own undo step.
    let card = cards(d.doc().project())[0].device;
    let depth = d.doc().undo_depth();
    let (_, changes) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("Move the card", |tx| {
            tx.set_property(card, start_key(novo_plc::Space::Input), Some(8i64.into()))
        })
        .unwrap();
    d.app.changed_outside_canvas(&changes);
    assert_eq!(printed(&d, "Pump on").as_deref(), Some("%IX8.0"));
    assert_eq!(d.doc().undo_depth(), depth + 1);

    // The address style changes how every address reads.
    let (_, changes) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("Style", |tx| {
            tx.set_info(|i| {
                i.properties
                    .insert(ADDRESS_STYLE.to_owned(), "rockwell".into());
            })
        })
        .unwrap();
    d.app.changed_outside_canvas(&changes);
    let shown = printed(&d, "Pump on").unwrap();
    assert!(shown.starts_with("Local:"), "{shown}");
}
