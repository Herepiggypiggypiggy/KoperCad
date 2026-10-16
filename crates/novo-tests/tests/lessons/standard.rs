//! Lesson project: `drawing-standard` (IEC or NFPA).
//!
//! The start project is a small 230 V control circuit drawn in IEC 60617:
//! circuit breaker `-F1`, stop `-S0`, start `-S1` with the self-holding
//! contact of `-K1`, the overload contact `-F2` and the coil `-K1`, and a
//! second branch with a make contact of `-K1` that lights the lamp `-H1`.
//! Every symbol in it has an NFPA twin in the built-in pack.
//!
//! The finished project is the start project after "Switch placed symbols":
//! `novo_workflow::switch::plan(project, Pack::Nfpa).command()` run as one
//! undo step, which sets the drawing standard rule and gives every placed
//! symbol the key of its twin. Nothing else changes: tags, wires, pins,
//! positions, parts and roles stay exactly as they were.

use novo_core::{
    Device, DeviceId, DeviceTag, Document, Endpoint, Page, PageId, Placement, Point, PotentialId,
    PotentialKind, Project, Result, SymbolId, Tx,
};
use novo_editor::edit::{PlaceOptions, draw_wire, place};
use novo_editor::geometry::Target;
use novo_symbols::{Library, Pack};
use novo_workflow::switch;

use super::common::{assert_lint, check_file};

const ID: &str = "drawing-standard";

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("lesson tags are valid")
}

fn lib() -> &'static Library {
    Library::shared(Pack::Iec)
}

/// Places a library symbol the way the editor does, without connecting it.
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

fn target(tx: &Tx<'_>, end: Endpoint) -> Target {
    let at = tx
        .project()
        .endpoint_position(end)
        .expect("end has a position");
    Target::Endpoint(end, at)
}

/// Draws a routed wire between two ends, as the wire tool does.
fn wire(tx: &mut Tx<'_>, page: PageId, a: Endpoint, b: Endpoint) -> Result<()> {
    let (from, to) = (target(tx, a), target(tx, b));
    draw_wire(tx, page, from, &[], to).map(|_| ())
}

fn pin(symbol: SymbolId, n: u16) -> Endpoint {
    Endpoint::pin(symbol, n)
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

fn part(
    tx: &mut Tx<'_>,
    symbol: SymbolId,
    (manufacturer, order, type_number, description): (&str, &str, &str, &str),
) -> Result<()> {
    let device = device_of(tx, symbol);
    let part = tx.add_part(manufacturer, order)?;
    tx.update::<novo_core::Part>(part, |p| {
        p.type_number = type_number.into();
        p.description = description.into();
    })?;
    tx.assign_part(device, part)
}

fn build(tx: &mut Tx<'_>) -> Result<()> {
    tx.set_info(|i| {
        i.name = "Pump control, drawing standard".into();
        i.description = "A stop and start circuit with a lamp, drawn in IEC 60617 and used by the \
            drawing standard lesson. The parts are illustrative catalogue entries, not a \
            verified parts list."
            .into();
    })?;
    let page = tx.add_page("1")?;
    tx.update::<Page>(page, |p| {
        p.title = "Control circuit".into();
        p.structure = tag("=A1+CAB");
    })?;
    let line = tx.add_potential("L", PotentialKind::Line)?;
    let neutral = tx.add_potential("N", PotentialKind::Neutral)?;
    let feed = "iec/connectivity/potential_feed";

    // ----- column 1: stop, start with self-hold, overload, coil ------------
    let feed_l = put(tx, page, feed, (40.0, 20.0), None, Some(line))?;
    let f1 = put(
        tx,
        page,
        "iec/protection/mcb_1p",
        (40.0, 40.0),
        Some("-F1"),
        None,
    )?;
    describe(tx, f1, "Control circuit breaker")?;
    let s0 = put(
        tx,
        page,
        "iec/operators/pushbutton_nc",
        (40.0, 72.0),
        Some("-S0"),
        None,
    )?;
    describe(tx, s0, "Stop")?;
    let s1 = put(
        tx,
        page,
        "iec/operators/pushbutton_no",
        (40.0, 104.0),
        Some("-S1"),
        None,
    )?;
    describe(tx, s1, "Start")?;
    let f2 = put(
        tx,
        page,
        "iec/contacts/thermal_nc",
        (40.0, 136.0),
        Some("-F2"),
        None,
    )?;
    describe(tx, f2, "Overload relay, auxiliary contact")?;
    let k1 = put(
        tx,
        page,
        "iec/coils/contactor",
        (40.0, 168.0),
        Some("-K1"),
        None,
    )?;
    describe(tx, k1, "Pump contactor")?;
    let hold = put(
        tx,
        page,
        "iec/contacts/no",
        (60.0, 104.0),
        Some("-K1"),
        None,
    )?;
    let feed_n = put(tx, page, feed, (40.0, 192.0), None, Some(neutral))?;

    let split = tx.add_junction(page, Point::new(40.0, 96.0))?;
    let join = tx.add_junction(page, Point::new(40.0, 128.0))?;
    wire(tx, page, pin(feed_l, 0), pin(f1, 0))?;
    wire(tx, page, pin(f1, 1), pin(s0, 0))?;
    wire(tx, page, pin(s0, 1), Endpoint::junction(split))?;
    wire(tx, page, Endpoint::junction(split), pin(s1, 0))?;
    wire(tx, page, Endpoint::junction(split), pin(hold, 0))?;
    wire(tx, page, pin(s1, 1), Endpoint::junction(join))?;
    wire(tx, page, pin(hold, 1), Endpoint::junction(join))?;
    wire(tx, page, Endpoint::junction(join), pin(f2, 0))?;
    wire(tx, page, pin(f2, 1), pin(k1, 0))?;
    wire(tx, page, pin(k1, 1), pin(feed_n, 0))?;

    // ----- column 2: a second make contact of -K1 lights the lamp ----------
    let feed_l2 = put(tx, page, feed, (104.0, 40.0), None, Some(line))?;
    let run = put(
        tx,
        page,
        "iec/contacts/no",
        (104.0, 60.0),
        Some("-K1"),
        None,
    )?;
    let h1 = put(
        tx,
        page,
        "iec/signalling/lamp_green",
        (104.0, 92.0),
        Some("-H1"),
        None,
    )?;
    describe(tx, h1, "Pump running lamp")?;
    let feed_n2 = put(tx, page, feed, (104.0, 124.0), None, Some(neutral))?;
    wire(tx, page, pin(feed_l2, 0), pin(run, 0))?;
    wire(tx, page, pin(run, 1), pin(h1, 0))?;
    wire(tx, page, pin(h1, 1), pin(feed_n2, 0))?;

    // ----- parts -----------------------------------------------------------
    part(
        tx,
        f1,
        (
            "Siemens",
            "5SY6106-7",
            "5SY6106",
            "Miniature circuit breaker 1-pole, 6 A",
        ),
    )?;
    part(
        tx,
        s0,
        (
            "Siemens",
            "3SU1100-0AB20-1CA0",
            "3SU1100",
            "Pushbutton, flush, red",
        ),
    )?;
    part(
        tx,
        s1,
        (
            "Siemens",
            "3SU1100-0AB40-1BA0",
            "3SU1100",
            "Pushbutton, flush, green",
        ),
    )?;
    part(
        tx,
        f2,
        (
            "Siemens",
            "3RU2116-1FB0",
            "3RU2116",
            "Thermal overload relay 3.5 to 5 A",
        ),
    )?;
    part(
        tx,
        k1,
        (
            "Siemens",
            "3RT2016-1AP01",
            "3RT2016",
            "Contactor 9 A, coil 230 V AC",
        ),
    )?;
    part(
        tx,
        h1,
        (
            "Siemens",
            "3SU1106-6AA40-1AA0",
            "3SU1106",
            "Indicator light, green",
        ),
    )?;
    Ok(())
}

fn with_generated_pages(doc: &mut Document) -> Result<()> {
    let specs = novo_reports::default_generated_pages(doc.project());
    doc.transact("Generated pages", |tx| {
        novo_reports::add_generated_pages(tx, &specs)
    })?;
    Ok(())
}

fn start_document() -> Document {
    let mut doc = Document::default();
    doc.transact("Draw the control circuit", build)
        .expect("the start project builds");
    with_generated_pages(&mut doc).expect("generated pages");
    doc
}

pub fn drawing_standard_start() -> Project {
    start_document().project().clone()
}

/// "Switch placed symbols" for NFPA: the standard rule and every symbol's key
/// in one undo step.
pub fn drawing_standard_finished() -> Project {
    let mut doc = start_document();
    let plan = switch::plan(doc.project(), Pack::Nfpa);
    doc.execute(plan.command()).expect("the switch applies");
    doc.project().clone()
}

#[test]
fn drawing_standard_files_are_current() {
    let start = drawing_standard_start();
    check_file(ID, "project.json", &start);
    check_file(ID, "finished.json", &drawing_standard_finished());
    assert_lint(&[(ID, &start)]);
}

#[test]
fn every_symbol_has_a_twin_with_the_same_connection_points() {
    let start = drawing_standard_start();
    let iec = Library::shared(Pack::Iec);
    let nfpa = Library::shared(Pack::Nfpa);
    let mut same_art = Vec::new();
    for symbol in start.symbols() {
        let key = symbol.symbol.0.as_str();
        let twin = Pack::counterpart(key, Pack::Nfpa).expect("an NFPA id");
        let (a, b) = (
            iec.get(key).expect("iec"),
            nfpa.get(&twin).expect("an NFPA twin"),
        );
        assert_eq!(a.function, b.function, "{key}");
        assert_eq!(
            a.connections
                .iter()
                .map(|c| (&c.id, c.at))
                .collect::<Vec<_>>(),
            b.connections
                .iter()
                .map(|c| (&c.id, c.at))
                .collect::<Vec<_>>(),
            "connection points of {key}"
        );
        if format!("{:?}", a.graphics) == format!("{:?}", b.graphics) {
            same_art.push(key.to_owned());
        }
    }
    same_art.sort();
    same_art.dedup();
    // Only the potential feed is drawn the same in both standards.
    assert_eq!(same_art, ["iec/connectivity/potential_feed"]);
}

#[test]
fn switching_changes_keys_and_the_rule_and_nothing_else() {
    let start = drawing_standard_start();
    let plan = switch::plan(&start, Pack::Nfpa);
    assert!(plan.is_complete(), "{:?}", plan.without_counterpart);
    assert_eq!(plan.foreign, 0);
    assert_eq!(plan.rekey.len(), start.symbols().count());

    let finished = drawing_standard_finished();
    assert_eq!(finished.info().rules.standard, "nfpa");
    assert!(start.info().rules.standard.is_empty());
    assert!(finished.symbols().all(|s| s.symbol.0.starts_with("nfpa/")));

    // Same JSON once the keys and the rule are mapped back.
    fn unkey(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::String(s) if s.starts_with("nfpa/") => {
                *s = format!("iec/{}", &s["nfpa/".len()..]);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(unkey),
            serde_json::Value::Object(o) => o.values_mut().for_each(unkey),
            _ => {}
        }
    }
    let mut after: serde_json::Value = serde_json::from_str(&finished.to_json()).unwrap();
    let removed = after["info"]["rules"]
        .as_object_mut()
        .unwrap()
        .remove("standard");
    assert_eq!(removed, Some(serde_json::Value::from("nfpa")));
    unkey(&mut after);
    let before: serde_json::Value = serde_json::from_str(&start.to_json()).unwrap();
    assert_eq!(
        before, after,
        "tags, wires, pins, positions and parts are untouched"
    );
}

#[test]
fn one_undo_goes_back_to_iec_and_switching_back_restores_the_start() {
    let mut doc = start_document();
    let start = doc.project().to_json();
    let plan = switch::plan(doc.project(), Pack::Nfpa);
    doc.execute(plan.command()).unwrap();
    let nfpa = doc.project().to_json();
    doc.undo().unwrap();
    assert_eq!(doc.project().to_json(), start);
    doc.redo().unwrap();
    assert_eq!(doc.project().to_json(), nfpa);
    // Back to IEC with the same command from the other side.
    let back = switch::plan(doc.project(), Pack::Iec);
    assert!(back.is_complete());
    doc.execute(back.command()).unwrap();
    // Oddity: the command writes the explicit id "iec" where the Project
    // settings window writes "" for IEC; both read as IEC, but the file
    // differs by that one rule.
    assert_eq!(doc.project().info().rules.standard, "iec");
    let mut again = doc.project().clone();
    again
        .transact("Empty rule", |tx| tx.set_info(|i| i.rules.standard.clear()))
        .unwrap();
    assert_eq!(again.to_json(), start);
}

#[test]
fn the_engines_read_the_circuit_the_same_in_both_standards() {
    let iec = drawing_standard_start();
    let nfpa = drawing_standard_finished();
    // Cross references (the coil -K1 and its two make contacts) and the
    // potentials do not depend on the pack.
    let rows = |p: &Project| {
        let index = novo_xref::XrefIndex::build(p, Library::shared(switch::current(p)));
        let mut rows: Vec<String> = p
            .devices()
            .flat_map(|d| {
                index
                    .device_references(d.id)
                    .into_iter()
                    .map(move |r| (d.tag.to_string(), r.row()))
            })
            .map(|(tag, row)| format!("{tag} {row}"))
            .collect();
        rows.sort();
        rows
    };
    assert_eq!(rows(&iec), rows(&nfpa));
    assert!(
        rows(&nfpa).iter().any(|r| r.contains("NO 13-14")),
        "{:?}",
        rows(&nfpa)
    );
    let nets = |p: &Project| p.netlist().nets().len();
    assert_eq!(nets(&iec), nets(&nfpa));
}
