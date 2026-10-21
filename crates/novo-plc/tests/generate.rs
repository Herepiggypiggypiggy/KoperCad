//! Pages from an I/O list: cards, pages, terminals and wires from a list,
//! addresses kept, and one undo step.

use novo_core::{Document, SymbolRole};
use novo_plc::generate::{Options, apply, plan, read_list};
use novo_plc::{ChannelType, PlcIndex, cards};
use novo_symbols::{Library, Pack};

const LIST: &str = "Tag,Description,Type,Address\n\
    -B1,Feed pump running,DI,\n\
    -B2,Level high,DI,\n\
    -S1,Start,DI,\n\
    -Y1,Valve open,DQ,\n\
    -Y2,Valve close,DQ,\n\
    -B3,Pressure,4-20 mA in,\n";

fn library() -> &'static Library {
    Library::shared(Pack::Iec)
}

fn import(doc: &mut Document, text: &str, options: &Options) -> novo_plc::generate::Plan {
    let list = read_list(text.as_bytes()).unwrap();
    let plan = plan(doc.project(), &list, options);
    doc.transact("Import I/O list", |tx| apply(tx, library(), &list, &plan))
        .unwrap();
    plan
}

#[test]
fn a_list_becomes_cards_pages_terminals_and_wires() {
    let mut doc = Document::default();
    let plan = import(&mut doc, LIST, &Options::default());
    // One card per signal type: DI8, DQ8, AI2.
    assert_eq!(plan.cards.len(), 3);
    let project = doc.project();
    assert_eq!(project.pages_in_order().len(), 3);
    let all = cards(project);
    assert_eq!(all.len(), 3);
    // Three strips, six terminals, six channel wires.
    let strips = project
        .devices()
        .filter(|d| d.kind == novo_core::DeviceKind::TerminalStrip)
        .count();
    assert_eq!(strips, 3);
    assert_eq!(project.wires().count(), 6);
    // Every wire ends on a channel and a terminal: nothing open.
    assert_eq!(
        project
            .symbols()
            .filter(|s| matches!(s.role, SymbolRole::Terminal { .. }))
            .count(),
        6
    );
    let index = PlcIndex::build(project, library());
    let rows = index.io_rows(project);
    let drawn: Vec<_> = rows.iter().filter(|r| !r.spare).collect();
    assert_eq!(drawn.len(), 6);
    let b1 = drawn
        .iter()
        .find(|r| r.signal == "Feed pump running")
        .unwrap();
    assert_eq!(b1.address.as_deref(), Some("%IX0.0"));
    assert_eq!(b1.terminals.len(), 1);
    assert!(b1.drawn_at.is_some());
    // Five DI/DQ channels are spares of their 8-channel cards.
    assert_eq!(rows.iter().filter(|r| r.spare).count(), 5 + 6 + 1);
    // Analog value on a word.
    let ai = drawn
        .iter()
        .find(|r| r.kind == ChannelType::AnalogInput)
        .unwrap();
    assert_eq!(ai.address.as_deref(), Some("%IW2"));
}

#[test]
fn one_undo_removes_everything() {
    let mut doc = Document::default();
    import(&mut doc, LIST, &Options::default());
    doc.undo().unwrap();
    let project = doc.project();
    assert_eq!(cards(project).len(), 0);
    assert_eq!(project.pages_in_order().len(), 0);
    assert_eq!(project.wires().count(), 0);
    assert_eq!(project.symbols().count(), 0);
}

#[test]
fn importing_the_same_list_again_adds_nothing() {
    let mut doc = Document::default();
    import(&mut doc, LIST, &Options::default());
    let again = import(&mut doc, LIST, &Options::default());
    assert!(again.is_empty());
    assert!(
        again
            .notes
            .iter()
            .any(|n| n.text.contains("already in the project"))
    );
    assert_eq!(cards(doc.project()).len(), 3);
}

#[test]
fn a_changed_list_adds_only_the_new_signals_after_the_existing_cards() {
    let mut doc = Document::default();
    import(&mut doc, LIST, &Options::default());
    let more = format!("{LIST}-B9,Door closed,DI,\n");
    let plan = import(&mut doc, &more, &Options::default());
    assert_eq!(plan.signals(), 1);
    let index = PlcIndex::build(doc.project(), library());
    let rows = index.io_rows(doc.project());
    let b9 = rows.iter().find(|r| r.signal == "Door closed").unwrap();
    // Existing cards never move: the new card follows the AI card (bytes 2 to 5).
    assert_eq!(b9.address.as_deref(), Some("%IX6.0"));
}

#[test]
fn addresses_in_the_list_are_kept_with_spares_in_the_gaps() {
    let list = "Tag,Description,Address\n\
        -B1,One,%IX2.0\n\
        -B2,Two,%IX2.1\n\
        -B3,Three,%IX2.4\n\
        -B4,Far,%IX4.0\n\
        -Y1,Out,%QX0.2\n";
    let mut doc = Document::default();
    import(&mut doc, list, &Options::default());
    let index = PlcIndex::build(doc.project(), library());
    let rows = index.io_rows(doc.project());
    let at = |signal: &str| {
        rows.iter()
            .find(|r| r.signal == signal)
            .and_then(|r| r.address.clone())
    };
    assert_eq!(at("One").as_deref(), Some("%IX2.0"));
    assert_eq!(at("Two").as_deref(), Some("%IX2.1"));
    assert_eq!(at("Three").as_deref(), Some("%IX2.4"));
    assert_eq!(at("Far").as_deref(), Some("%IX4.0"));
    assert_eq!(at("Out").as_deref(), Some("%QX0.2"));
}

#[test]
fn a_clash_with_an_existing_card_falls_back_to_assigned_addresses() {
    let mut doc = Document::default();
    import(
        &mut doc,
        "Tag,Description,Type\n-B1,First,DI\n",
        &Options::default(),
    );
    // The list wants byte 0 again, where -K1 already is.
    let list = "Tag,Description,Address\n-B7,Second,%IX0.0\n";
    let plan = import(&mut doc, list, &Options::default());
    assert!(plan.notes.iter().any(|n| n.text.contains("overlap")));
    let index = PlcIndex::build(doc.project(), library());
    let rows = index.io_rows(doc.project());
    let second = rows.iter().find(|r| r.signal == "Second").unwrap();
    assert_eq!(second.address.as_deref(), Some("%IX1.0"));
}

#[test]
fn more_signals_than_a_card_holds_make_more_cards() {
    let mut text = String::from("Tag,Description,Type\n");
    for n in 0..20 {
        text.push_str(&format!("-B{n},Sensor {n},DI\n"));
    }
    let mut doc = Document::default();
    let plan = import(&mut doc, &text, &Options::default());
    assert_eq!(plan.cards.len(), 2);
    assert_eq!(plan.cards[0].channels.len(), 16);
    assert_eq!(plan.cards[1].channels.len(), 8);
    assert_eq!(plan.cards[1].spares(), 4);
}
