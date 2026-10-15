//! Lesson project: `plc-io` (PLC cards and I/O).
//!
//! The start project is a small 24 V pump circuit on one page: a start button
//! `-S1` drawn left to right from the 24 V feed, and the running lamp `-H1`
//! hanging from 0 V. Nothing is connected to a PLC yet. The cabinet already
//! holds an analog input card `-K1` whose addresses start at input byte 4
//! (the digital card that sat before it was taken out), so `renumber io` has
//! something to show.
//!
//! The finished project is the start project after the lesson's steps, run
//! through the same engine calls the app makes: `add card DI16 DQ8`, `to plc`
//! on the button and on the lamp, `renumber io`.

use novo_core::{
    Device, DeviceId, DeviceTag, Document, Endpoint, Page, PageId, Placement, Point, PotentialId,
    PotentialKind, Project, Result, Rotation, SymbolId, Tx, Value,
};
use novo_editor::edit::{PlaceOptions, draw_wire, place};
use novo_editor::geometry::Target;
use novo_editor::naming::next_device_tag;
use novo_plc::commands::{
    NewCardLayout, add_card, apply_renumber, renumber_plan, signal_channel_type, to_plc,
};
use novo_plc::{CardLayout, PlcIndex, cards};
use novo_symbols::{Library, Pack};

use super::common::{assert_lint, check_file};

const ID: &str = "plc-io";

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("lesson tags are valid")
}

fn lib() -> &'static Library {
    Library::shared(Pack::Iec)
}

/// Places a library symbol the way the editor does (tags, pins, bridges),
/// without connecting it to anything.
fn put(
    tx: &mut Tx<'_>,
    page: PageId,
    key: &str,
    at: (f64, f64),
    parent: Option<&str>,
    potential: Option<PotentialId>,
) -> Result<SymbolId> {
    let symbol = lib()
        .get(key)
        .unwrap_or_else(|| panic!("`{key}` is in the library"));
    let options = PlaceOptions {
        parent: parent.map(tag),
        potential,
        auto_connect: false,
        ..PlaceOptions::new()
    };
    place(
        tx,
        page,
        symbol,
        Placement::at(Point::new(at.0, at.1)),
        &options,
    )
}

/// Draws a routed wire between two pins, as the wire tool does.
fn wire(tx: &mut Tx<'_>, page: PageId, a: (SymbolId, u16), b: (SymbolId, u16)) -> Result<()> {
    let target = |tx: &Tx<'_>, (symbol, pin): (SymbolId, u16)| {
        let end = Endpoint::pin(symbol, pin);
        let at = tx
            .project()
            .endpoint_position(end)
            .expect("pin has a position");
        Target::Endpoint(end, at)
    };
    let (from, to) = (target(tx, a), target(tx, b));
    draw_wire(tx, page, from, &[], to).map(|_| ())
}

/// Turns a symbol a quarter turn, so it runs left to right.
fn turn(tx: &mut Tx<'_>, symbol: SymbolId) -> Result<()> {
    tx.update::<novo_core::SymbolInstance>(symbol, |s| s.placement.rotation = Rotation::R90)
}

fn device_of(tx: &Tx<'_>, symbol: SymbolId) -> DeviceId {
    tx.project()
        .symbol(symbol)
        .and_then(|s| s.device())
        .expect("the symbol belongs to a device")
}

fn describe(tx: &mut Tx<'_>, symbol: SymbolId, text: &str) -> Result<()> {
    let device = device_of(tx, symbol);
    tx.update::<Device>(device, |d| d.description = text.into())
}

/// An illustrative part: manufacturer, order number, type, description.
fn part(
    tx: &mut Tx<'_>,
    device: DeviceId,
    (manufacturer, order, type_number, description): (&str, &str, &str, &str),
) -> Result<novo_core::PartId> {
    let part = tx.add_part(manufacturer, order)?;
    tx.update::<novo_core::Part>(part, |p| {
        p.type_number = type_number.into();
        p.description = description.into();
    })?;
    tx.assign_part(device, part)?;
    Ok(part)
}

/// The start project, before the generated pages.
fn build(tx: &mut Tx<'_>) -> Result<()> {
    tx.set_info(|i| {
        i.name = "Pump with PLC I/O".into();
        i.description = "A 24 V pump circuit prepared for a PLC card, used by the PLC and I/O \
            lesson. The parts are illustrative catalogue entries, not a verified parts list."
            .into();
    })?;
    let page = tx.add_page("1")?;
    tx.update::<Page>(page, |p| {
        p.title = "Pump control".into();
        p.structure = tag("=A1+CAB");
    })?;
    let plus = tx.add_potential("24V", PotentialKind::DcPositive)?;
    let zero = tx.add_potential("0V", PotentialKind::DcNegative)?;
    let feed = "iec/connectivity/potential_feed";

    // ----- row 1: the start button, drawn left to right, its far end free ---
    // for a DI channel (the channel lands 50 mm to the right on the same line)
    let feed_a = put(tx, page, feed, (24.0, 44.0), None, Some(plus))?;
    turn(tx, feed_a)?;
    let s1 = put(
        tx,
        page,
        "iec/operators/pushbutton_no",
        (40.0, 44.0),
        Some("-S1"),
        None,
    )?;
    turn(tx, s1)?;
    describe(tx, s1, "Pump start button")?;
    wire(tx, page, (feed_a, 0), (s1, 0))?;

    // ----- row 2: the pump running lamp hangs from 0 V; its lower end is ----
    // free for a DQ channel
    let feed_b = put(tx, page, feed, (120.0, 28.0), None, Some(zero))?;
    let h1 = put(
        tx,
        page,
        "iec/signalling/lamp_green",
        (120.0, 48.0),
        Some("-H1"),
        None,
    )?;
    describe(tx, h1, "Pump running lamp")?;
    wire(tx, page, (feed_b, 0), (h1, 0))?;

    // ----- parts -----------------------------------------------------------
    let s1_device = device_of(tx, s1);
    let h1_device = device_of(tx, h1);
    part(
        tx,
        s1_device,
        (
            "Siemens",
            "3SU1100-0AB40-1BA0",
            "3SU1100",
            "Pushbutton, flush, green",
        ),
    )?;
    part(
        tx,
        h1_device,
        (
            "Siemens",
            "3SU1106-6AA40-1AA0",
            "3SU1106",
            "Indicator light, green, 24 V",
        ),
    )?;

    // ----- the cabinet's analog card, with a gap in front of it ------------
    let card_part = tx.add_part("Siemens", "6ES7134-6HD01-0BA1")?;
    tx.update::<novo_core::Part>(card_part, |p| {
        p.type_number = "ET 200SP AI 4xU/I".into();
        p.description = "Analog input module, 4 channels".into();
        p.properties
            .insert("plc.channels".into(), Value::from("AI4"));
    })?;
    let analog = add_card(
        tx,
        tag("=A1+CAB-K1"),
        "Analog input card (tank level)",
        NewCardLayout::Part(card_part),
        "",
        Some(1),
    )?;
    // The digital card that used to sit in front of it is gone; the analog
    // card keeps the addresses the PLC program already uses.
    tx.set_property(analog, "plc.start.i", Some(Value::Int(4)))?;
    Ok(())
}

/// Adds the generated pages the app adds when it opens a project.
fn with_generated_pages(doc: &mut Document) -> Result<()> {
    let specs = novo_reports::default_generated_pages(doc.project());
    doc.transact("Generated pages", |tx| {
        novo_reports::add_generated_pages(tx, &specs)
    })?;
    Ok(())
}

pub fn plc_io_start() -> Project {
    let mut doc = Document::default();
    doc.transact("Draw the pump circuit", build)
        .expect("the start project builds");
    with_generated_pages(&mut doc).expect("generated pages");
    doc.project().clone()
}

fn page_named(project: &Project, title: &str) -> PageId {
    project
        .pages()
        .find(|p| p.title == title)
        .map(|p| p.id)
        .expect("the page exists")
}

fn symbol_of(project: &Project, tag_text: &str, key_part: &str) -> SymbolId {
    let device = project
        .devices()
        .find(|d| d.tag.to_string() == tag_text)
        .unwrap_or_else(|| panic!("{tag_text} exists"))
        .id;
    project
        .symbols_of_device(device)
        .into_iter()
        .find(|s| s.symbol.0.contains(key_part))
        .unwrap_or_else(|| panic!("{tag_text} draws a `{key_part}` symbol"))
        .id
}

/// `add card DI16 DQ8` as the app runs it: the next free `-K` tag on the page.
fn add_digital_card(doc: &mut Document, page: PageId) -> DeviceId {
    let layout: CardLayout = "DI16 DQ8".parse().expect("a card layout");
    let (card, _) = doc
        .transact("Add PLC card", |tx| {
            let tag = next_device_tag(tx.project(), page, "K");
            add_card(tx, tag, "", NewCardLayout::Typed(layout), "", None)
        })
        .expect("add card");
    card
}

/// `to plc` as the app runs it, on the symbol `signal`.
fn send_to_plc(doc: &mut Document, signal: SymbolId) -> SymbolId {
    let library = lib();
    let project = doc.project();
    let instance = project.symbol(signal).expect("the signal symbol");
    let kind = library
        .get(&instance.symbol.0)
        .and_then(|s| signal_channel_type(&s.function))
        .expect("a contact, sensor or coil");
    let index = PlcIndex::build(project, library);
    let card = *index
        .cards_with_free(kind)
        .first()
        .expect("a card with a free channel");
    let pin = (0..instance.pins.len())
        .rev()
        .filter_map(|i| u16::try_from(i).ok())
        .find(|&i| project.wires_at(Endpoint::pin(signal, i)).next().is_none())
        .expect("a free pin");
    let key = match kind {
        novo_plc::ChannelType::DigitalInput => "iec/plc/digital_input",
        novo_plc::ChannelType::DigitalOutput => "iec/plc/digital_output",
        other => panic!("not used in this lesson: {other:?}"),
    };
    let channel = library.get(key).expect("the channel symbol");
    let pins = novo_editor::catalog::model_pins(channel);
    let origin = instance.placement.origin;
    let placement = Placement::at(Point::new(origin.x + 50.0, origin.y));
    let (placed, _) = doc
        .transact("Send signal to PLC", |tx| {
            to_plc(
                tx,
                library,
                card,
                Endpoint::pin(signal, pin),
                kind,
                key,
                placement,
                pins,
            )
        })
        .expect("to plc");
    placed
}

/// `renumber io` with Enter on the preview.
fn renumber_io(doc: &mut Document) {
    let project = doc.project();
    let stations: std::collections::BTreeSet<String> =
        cards(project).into_iter().map(|c| c.station).collect();
    let plans: Vec<_> = stations.iter().map(|s| renumber_plan(project, s)).collect();
    doc.transact("Renumber I/O", |tx| {
        plans.iter().try_for_each(|p| apply_renumber(tx, p))
    })
    .expect("renumber io");
}

/// The lesson after the card is added and both signals are sent to it, before
/// `renumber io`.
fn after_sending() -> Document {
    let mut doc = Document::default();
    doc.transact("Draw the pump circuit", build)
        .expect("the start project builds");
    with_generated_pages(&mut doc).expect("generated pages");
    let page = page_named(doc.project(), "Pump control");
    add_digital_card(&mut doc, page);
    let button = symbol_of(doc.project(), "=A1+CAB-S1", "pushbutton");
    send_to_plc(&mut doc, button);
    let lamp = symbol_of(doc.project(), "=A1+CAB-H1", "lamp");
    send_to_plc(&mut doc, lamp);
    doc
}

pub fn plc_io_finished() -> Project {
    let mut doc = after_sending();
    renumber_io(&mut doc);
    doc.project().clone()
}

#[test]
fn plc_io_files_are_current() {
    let start = plc_io_start();
    check_file(ID, "project.json", &start);
    check_file(ID, "finished.json", &plc_io_finished());
    assert_lint(&[(ID, &start)]);
}

#[test]
fn the_start_has_a_gap_and_nothing_wired_to_a_card() {
    let start = plc_io_start();
    let all = cards(&start);
    assert_eq!(all.len(), 1, "only the analog card");
    assert_eq!(all[0].start[0], Some(4), "input byte 4: the gap");
    let index = PlcIndex::build(&start, lib());
    assert!(index.channels().is_empty());
    assert!(index.issues().is_empty(), "{:?}", index.issues());
}

#[test]
fn the_finished_project_has_the_channels_wired_and_renumbered() {
    let finished = plc_io_finished();
    let library = lib();
    let index = PlcIndex::build(&finished, library);
    assert!(index.issues().is_empty(), "{:?}", index.issues());

    // The new card is the next free -K tag, with the typed layout.
    let digital = cards(&finished)
        .into_iter()
        .find(|c| {
            finished
                .device(c.device)
                .is_some_and(|d| d.tag.to_string() == "=A1+CAB-K2")
        })
        .expect("the added card is -K2");
    assert_eq!(digital.layout.as_ref().unwrap().to_string(), "DI16 DQ8");
    // Renumber io packed the cards in slot order: the analog card (slot 1)
    // first, the digital card (no slot) after its 8 bytes.
    let analog = cards(&finished)
        .into_iter()
        .find(|c| c.slot == Some(1))
        .expect("the analog card");
    assert_eq!(analog.start[0], Some(0));
    assert_eq!(digital.start, [Some(8), Some(0)]);

    // DI 0 is wired to the button, DQ 0 to the lamp.
    let channels = index.channels();
    assert_eq!(channels.len(), 2);
    let di = channels
        .iter()
        .find(|c| c.kind == novo_plc::ChannelType::DigitalInput)
        .unwrap();
    let dq = channels
        .iter()
        .find(|c| c.kind == novo_plc::ChannelType::DigitalOutput)
        .unwrap();
    assert_eq!(di.card, digital.device);
    assert_eq!(index.address_label(di.symbol).as_deref(), Some("%IX8.0"));
    assert_eq!(index.address_label(dq.symbol).as_deref(), Some("%QX0.0"));
    let button = symbol_of(&finished, "=A1+CAB-S1", "pushbutton");
    let lamp = symbol_of(&finished, "=A1+CAB-H1", "lamp");
    let joined = |a: SymbolId, pa: u16, b: SymbolId| {
        finished.wires_at(Endpoint::pin(a, pa)).any(|w| {
            finished
                .wire(w)
                .is_some_and(|w| [w.from, w.to].contains(&Endpoint::pin(b, 0)))
        })
    };
    assert!(joined(button, 1, di.symbol), "S1:14 to the DI pin");
    assert!(joined(lamp, 1, dq.symbol), "H1:X2 to the DQ pin");

    // The I/O list lists the channels with the signal's description.
    let io = novo_reports::IoList;
    let output = novo_reports::Report::generate(&io, &finished);
    assert!(
        output.row_count() >= 3,
        "AI spares plus the two digital rows"
    );
}

/// The rows of the I/O list that are not spare: (address, type, signal, drawn at).
fn drawn_rows(project: &Project) -> Vec<(String, String, String, String)> {
    let output = novo_reports::Report::generate(&novo_reports::IoList, project);
    let text = |c: &novo_reports::Cell| match c {
        novo_reports::Cell::Text(t) => t.clone(),
        _ => String::new(),
    };
    output.sections[0]
        .rows
        .iter()
        .filter(|r| !text(&r.cells[8]).is_empty())
        .map(|r| {
            (
                text(&r.cells[0]),
                text(&r.cells[1]),
                text(&r.cells[5]),
                text(&r.cells[8]),
            )
        })
        .collect()
}

#[test]
fn the_io_list_shows_the_channels_before_and_after_renumbering() {
    let rows = |p: &Project| {
        drawn_rows(p)
            .into_iter()
            .map(|(a, t, s, d)| format!("{a} {t} {s} {d}"))
            .collect::<Vec<_>>()
    };
    let before = after_sending();
    assert_eq!(
        rows(before.project()),
        [
            "%IX12.0 DI Pump start button =A1+CAB/1.3",
            "%QX0.0 DQ Pump running lamp =A1+CAB/1.5"
        ]
    );
    assert_eq!(
        rows(&plc_io_finished()),
        [
            "%IX8.0 DI Pump start button =A1+CAB/1.3",
            "%QX0.0 DQ Pump running lamp =A1+CAB/1.5"
        ]
    );
    // The list page is in the start project, so the person sees it live.
    let start = plc_io_start();
    assert!(start.pages().any(|p| p.title == "I/O list"));
}

#[test]
fn renumber_shows_a_preview_with_two_moves() {
    let doc = after_sending();
    let plan = renumber_plan(doc.project(), "");
    let moves: Vec<_> = plan.moves.iter().map(|m| (m.from, m.to)).collect();
    assert_eq!(moves, [(Some(4), 0), (Some(12), 8)]);
}

#[test]
fn the_step_checks_are_false_at_the_start_and_true_at_the_end() {
    let on_page = |p: &Project| {
        let page = page_named(p, "Pump control");
        p.symbols().filter(|s| s.page == page).count()
    };
    let (start, finished) = (plc_io_start(), plc_io_finished());
    // count >= 5 after the DI, >= 6 after the DQ.
    assert_eq!(on_page(&start), 4);
    assert_eq!(on_page(&finished), 6);
    let has = |p: &Project, tag_text: &str| p.devices().any(|d| d.tag.to_string() == tag_text);
    assert!(!has(&start, "=A1+CAB-K2") && has(&finished, "=A1+CAB-K2"));
    // The lesson opens on the drawing page, not on the I/O list.
    assert_eq!(
        start
            .pages_in_order()
            .iter()
            .filter(|p| p.title == "Pump control")
            .count(),
        1
    );
}
