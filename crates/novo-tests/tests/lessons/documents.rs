//! Lesson projects: the documents track (`generated-pages`,
//! `wire-list-and-bom`, `pdf-export`).
//!
//! All three lessons continue one drawing, the pump starter of the Gate 1
//! fixture (`novo_tests::motor_starter_in`, the same builder as Open
//! example), polished for documentation: page structure `=A1+CAB`, wire
//! numbers and colours, part descriptions and the generated pages the app
//! adds on open. Each lesson owns its start and finished file:
//!
//! - `generated-pages`: start = the polished starter with its generated
//!   pages; finished = a pilot lamp `-H1` placed on the control page.
//! - `wire-list-and-bom`: start = the same, with the lamp wired and still
//!   without a part (so the bill of materials lists it under "Devices
//!   without parts"); finished = the lamp has a part.
//! - `pdf-export`: start = the finished project of the previous lesson;
//!   finished = revision A issued (the drawn pages frozen).

use novo_core::{
    Device, DeviceId, DeviceTag, Document, Endpoint, Page, PageId, Placement, Point, Project,
    Result, SymbolId, SymbolRole, Tx, Wire, WireId,
};
use novo_editor::edit::{PlaceOptions, place};
use novo_symbols::Library;
use novo_tests::{MotorStarter, motor_starter_in};

use super::common::{assert_lint, check_file};

const GENERATED: &str = "generated-pages";
const WIRES_BOM: &str = "wire-list-and-bom";
const PDF: &str = "pdf-export";

/// The fixture draws at x = 40 mm, where long tags such as `=A1+CAB-F3` run
/// over the frame; the polished project moves every page right by this much
/// (4 mm grid).
const SHIFT: f64 = 32.0;

/// Where the pilot lamp goes on the control page (4 mm grid).
const LAMP_AT: (f64, f64) = (72.0 + SHIFT, 168.0);

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("lesson tags are valid")
}

/// Illustrative catalogue data for the fixture's parts: (order number,
/// type number, description).
const PART_TEXT: &[(&str, &str, &str)] = &[
    ("3NW7033", "3NW7033", "Cylindrical fuse holder 3-pole, 32 A"),
    ("3RT2016-1AP01", "3RT2016", "Contactor 9 A, coil 230 V AC"),
    (
        "3RU2116-1FB0",
        "3RU2116",
        "Thermal overload relay 3.5 to 5 A",
    ),
    ("1LE1003-0EB42-2AA4", "1LE1003", "Motor 2.2 kW, 4-pole"),
    (
        "5SY6106-7",
        "5SY6106",
        "Miniature circuit breaker 1-pole, 6 A",
    ),
    ("3SU1100-0AB20-1CA0", "3SU1100", "Pushbutton, flush, red"),
    ("3SU1100-0AB40-1BA0", "3SU1100", "Pushbutton, flush, green"),
    ("3044076", "UT 2,5", "Feed-through terminal block 2.5 mm2"),
    (
        "3044089",
        "UT 2,5-PE",
        "Protective earth terminal block 2.5 mm2",
    ),
    ("1119304", "ÖLFLEX CLASSIC 110 4G1,5", "Motor cable 4G1.5"),
];

/// The polished pump starter, without generated pages.
fn polished(doc: &mut Document) -> Result<MotorStarter> {
    let ((starter, ()), _) = doc.transact("Draw the pump starter", |tx| {
        let starter = motor_starter_in(tx)?;
        polish(tx, &starter)?;
        Ok((starter, ()))
    })?;
    Ok(starter)
}

fn polish(tx: &mut Tx<'_>, starter: &MotorStarter) -> Result<()> {
    tx.set_info(|i| {
        i.name = "Pump starter, documentation".into();
        i.description = "Direct-on-line starter for a pump motor, used by the documents \
            lessons: the generated pages, the wire list and bill of materials, and the PDF \
            export. The parts are illustrative catalogue entries, not a verified parts list."
            .into();
        i.properties.insert("drawn_by".into(), "TV".into());
        i.properties.insert("checked_by".into(), "JV".into());
    })?;
    // Both pages belong to the cabinet, so a new device gets `=A1+CAB-...`.
    for page in [starter.main_page, starter.control_page] {
        tx.update::<Page>(page, |p| p.structure = tag("=A1+CAB"))?;
    }
    tx.update::<Device>(starter.x1, |d| {
        d.description = "Motor feeder terminals".into()
    })?;
    // Part texts for the bill of materials.
    let parts: Vec<_> = tx.project().parts().map(|p| p.id).collect();
    for id in parts {
        let order = tx.project().part(id).map(|p| p.order_number.clone());
        if let Some((_, type_number, description)) = PART_TEXT
            .iter()
            .find(|(o, ..)| Some(*o) == order.as_deref())
        {
            tx.update::<novo_core::Part>(id, |p| {
                p.type_number = (*type_number).into();
                p.description = (*description).into();
            })?;
        }
    }
    // The earth wire from the feed to -X1:PE leaves the terminal with a
    // corner instead of a slanted line.
    let pe = starter.motor_wires[3];
    tx.update::<Wire>(pe, |w| {
        w.route = vec![Point::new(72.0, 164.0), Point::new(64.0, 164.0)]
    })?;
    // The self-hold contact sits right of the start button, so its tag does
    // not run into the button's, and its two wires leave with corners.
    tx.move_symbol(starter.k1_aux, Placement::at(Point::new(72.0, 104.0)))?;
    let split = wire_between(
        tx.project(),
        Endpoint::junction(starter.hold_split),
        Endpoint::pin(starter.k1_aux, 0),
    );
    tx.update::<Wire>(split, |w| w.route = vec![Point::new(72.0, 96.0)])?;
    let join = wire_between(
        tx.project(),
        Endpoint::pin(starter.k1_aux, 1),
        Endpoint::junction(starter.hold_join),
    );
    tx.update::<Wire>(join, |w| w.route = vec![Point::new(72.0, 128.0)])?;
    number_wires(tx, starter)?;
    for page in [starter.main_page, starter.control_page] {
        let contents = tx.project().page_contents(page);
        let items: Vec<novo_core::AnyId> = contents
            .symbols
            .iter()
            .map(|&s| s.into())
            .chain(contents.junctions.iter().map(|&j| j.into()))
            .collect();
        tx.move_to_page(&items, page, Point::new(SHIFT, 0.0))?;
    }
    Ok(())
}

/// Wire numbers per connection (wires that meet at a junction share one),
/// in reading order: left to right, then top to bottom, page by page. Colours
/// and cross-sections follow; the motor wires take their colour from the
/// cable core. A feed wire reaches no second pin and stays unnumbered, as the
/// wire list leaves it out.
fn number_wires(tx: &mut Tx<'_>, starter: &MotorStarter) -> Result<()> {
    let project = tx.project();
    let feed_of = |end: Endpoint, potential| {
        end.as_pin().is_some_and(|pin| {
            project.symbol(pin.symbol).is_some_and(
                |s| matches!(s.role, SymbolRole::PotentialFeed { potential: p } if p == potential),
            )
        })
    };
    let page_index: Vec<PageId> = project.pages_in_order().iter().map(|p| p.id).collect();
    let wires: Vec<Wire> = {
        let mut all: Vec<Wire> = project.wires().cloned().collect();
        all.sort_by_key(|w| w.id);
        all
    };
    // Union of wires that meet at a junction.
    let mut parent: Vec<usize> = (0..wires.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (i, a) in wires.iter().enumerate() {
        for (j, b) in wires.iter().enumerate().skip(i + 1) {
            let shared = [a.from, a.to]
                .into_iter()
                .any(|e| matches!(e, Endpoint::Junction { .. }) && (b.from == e || b.to == e));
            if shared {
                let (x, y) = (find(&mut parent, i), find(&mut parent, j));
                parent[x] = y;
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for i in 0..wires.len() {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(i);
    }
    struct Group {
        members: Vec<usize>,
        key: (usize, f64, f64),
    }
    let mut ordered: Vec<Group> = Vec::new();
    for members in groups.into_values() {
        let pins = members
            .iter()
            .flat_map(|&i| [wires[i].from, wires[i].to])
            .filter(|e| e.as_pin().is_some())
            .filter(|e| {
                let role = e
                    .as_pin()
                    .and_then(|p| project.symbol(p.symbol))
                    .map(|s| &s.role);
                !matches!(role, Some(SymbolRole::PotentialFeed { .. }))
            })
            .count();
        if pins < 2 {
            continue;
        }
        let mut points = Vec::new();
        for &i in &members {
            points.extend(project.endpoint_position(wires[i].from));
            points.extend(project.endpoint_position(wires[i].to));
            points.extend(wires[i].route.iter().copied());
        }
        let min_x = points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        let min_y = points.iter().map(|p| p.y).fold(f64::MAX, f64::min);
        let page = page_index
            .iter()
            .position(|p| *p == wires[members[0]].page)
            .unwrap_or(usize::MAX);
        ordered.push(Group {
            members,
            key: (page, min_x, min_y),
        });
    }
    ordered.sort_by(|a, b| {
        a.key
            .0
            .cmp(&b.key.0)
            .then(a.key.1.total_cmp(&b.key.1))
            .then(a.key.2.total_cmp(&b.key.2))
    });
    let cores = ["BK", "BN", "GY", "GNYE"];
    let mut updates = Vec::new();
    for (n, group) in ordered.iter().enumerate() {
        let touches = |potential| {
            group
                .members
                .iter()
                .any(|&i| feed_of(wires[i].from, potential) || feed_of(wires[i].to, potential))
        };
        for &i in &group.members {
            let w = &wires[i];
            let motor = starter.motor_wires.iter().position(|m| *m == w.id);
            let (color, section) = match motor {
                Some(core) => (cores[core], 1.5),
                None if w.page == starter.main_page => ("BK", 2.5),
                None if touches(starter.n) => ("BU", 1.0),
                None => ("RD", 1.0),
            };
            updates.push((w.id, (n + 1).to_string(), color, section));
        }
    }
    for (id, number, color, section) in updates {
        tx.update::<Wire>(id, |w| {
            w.number = Some(number);
            w.color = Some(color.into());
            w.cross_section_mm2 = Some(section);
        })?;
    }
    Ok(())
}

/// Adds the generated pages the app adds on open (the same engine call).
fn with_generated_pages(doc: &mut Document) -> Result<()> {
    let specs = novo_reports::default_generated_pages(doc.project());
    doc.transact("Generated pages", |tx| {
        novo_reports::add_generated_pages(tx, &specs)
    })?;
    Ok(())
}

/// The pump starter with its generated pages: the start of `generated-pages`.
fn base() -> Document {
    let mut doc = Document::default();
    polished(&mut doc).expect("the pump starter builds");
    with_generated_pages(&mut doc).expect("generated pages");
    doc
}

fn control_page(project: &Project) -> PageId {
    project
        .pages()
        .find(|p| p.title == "Control circuit")
        .expect("the control page")
        .id
}

/// Places the green indicator lamp as the editor's place tool does.
fn place_lamp(doc: &mut Document) -> Result<SymbolId> {
    let library = Library::builtin().expect("built-in library");
    let symbol = library
        .get("iec/signalling/lamp_green")
        .expect("lamp symbol");
    let page = control_page(doc.project());
    let (id, _) = doc.transact("Place Indicator lamp, green", |tx| {
        place(
            tx,
            page,
            symbol,
            Placement::at(Point::new(LAMP_AT.0, LAMP_AT.1)),
            &PlaceOptions::new(),
        )
    })?;
    Ok(id)
}

fn device_of(project: &Project, symbol: SymbolId) -> DeviceId {
    match project.symbol(symbol).map(|s| &s.role) {
        Some(SymbolRole::Device { device, .. }) => *device,
        other => panic!("not a device symbol: {other:?}"),
    }
}

fn wire_between(project: &Project, a: Endpoint, b: Endpoint) -> WireId {
    project
        .wires()
        .find(|w| (w.from == a && w.to == b) || (w.from == b && w.to == a))
        .expect("the wire exists")
        .id
}

pub fn generated_pages_start() -> Project {
    base().into_project()
}

pub fn generated_pages_finished() -> Project {
    let mut doc = base();
    place_lamp(&mut doc).expect("lamp");
    doc.into_project()
}

/// The lamp wired in parallel with the coil, as the person would have done
/// after the first lesson; no part yet.
fn lamp_wired() -> (Document, DeviceId) {
    let mut doc = Document::default();
    let starter = polished(&mut doc).expect("the pump starter builds");
    with_generated_pages(&mut doc).expect("generated pages");
    let lamp = place_lamp(&mut doc).expect("lamp");
    let device = device_of(doc.project(), lamp);
    doc.transact("Wire the lamp", |tx| {
        let project = tx.project();
        let find = |key: &str, pin: &str| {
            project
                .symbols()
                .find(|s| s.symbol.0 == key && s.pins.iter().any(|p| p.name == pin))
                .map(|s| s.id)
                .expect("symbol")
        };
        let coil = find("iec/coils/relay", "A1");
        let thermal = find("iec/contacts/thermal_nc", "95");
        let neutral = project
            .symbols()
            .find(|s| {
                matches!(s.role, SymbolRole::PotentialFeed { .. })
                    && s.page == project.symbol(coil).unwrap().page
                    && s.placement.origin.y > 190.0
            })
            .map(|s| s.id)
            .expect("neutral feed");
        // Coil node: the wire from the overload contact to coil A1.
        let above = wire_between(project, Endpoint::pin(thermal, 1), Endpoint::pin(coil, 0));
        let below = wire_between(project, Endpoint::pin(coil, 1), Endpoint::pin(neutral, 0));
        let (top, _) = tx.split_wire(above, Point::new(40.0 + SHIFT, 160.0))?;
        let (bottom, _) = tx.split_wire(below, Point::new(40.0 + SHIFT, 192.0))?;
        let up = tx.connect(Endpoint::junction(top), Endpoint::pin(lamp, 0))?;
        tx.update::<Wire>(up, |w| w.route = vec![Point::new(LAMP_AT.0, 160.0)])?;
        let down = tx.connect(Endpoint::pin(lamp, 1), Endpoint::junction(bottom))?;
        tx.update::<Wire>(down, |w| w.route = vec![Point::new(LAMP_AT.0, 192.0)])?;
        number_wires(tx, &starter)?;
        tx.update::<Device>(device, |d| d.description = "Pump running".into())?;
        Ok(())
    })
    .expect("wire the lamp");
    (doc, device)
}

pub fn wire_list_and_bom_start() -> Project {
    lamp_wired().0.into_project()
}

pub fn wire_list_and_bom_finished() -> Project {
    let (mut doc, lamp) = lamp_wired();
    doc.transact("Assign part", |tx| {
        let part = tx.add_part("Siemens", "3SU1106-6AA40-1AA0")?;
        tx.update::<novo_core::Part>(part, |p| {
            p.type_number = "3SU1106".into();
            p.description = "Indicator lamp, green, 24 V".into();
        })?;
        tx.assign_part(lamp, part)
    })
    .expect("assign part");
    doc.into_project()
}

pub fn pdf_export_start() -> Project {
    wire_list_and_bom_finished()
}

pub fn pdf_export_finished() -> Project {
    let mut doc = Document::new(pdf_export_start());
    doc.transact("Issue revision A", |tx| {
        tx.issue_revision("A", "2026-10-15", "TV", "First issue")
    })
    .expect("issue");
    doc.into_project()
}

#[test]
fn generated_pages_files_are_current() {
    let (start, finished) = (generated_pages_start(), generated_pages_finished());
    check_file(GENERATED, "project.json", &start);
    check_file(GENERATED, "finished.json", &finished);
    assert_lint(&[(GENERATED, &start)]);
    extra_lint(GENERATED, &start);
}

#[test]
fn wire_list_and_bom_files_are_current() {
    let (start, finished) = (wire_list_and_bom_start(), wire_list_and_bom_finished());
    check_file(WIRES_BOM, "project.json", &start);
    check_file(WIRES_BOM, "finished.json", &finished);
    assert_lint(&[(WIRES_BOM, &start)]);
    extra_lint(WIRES_BOM, &start);
}

#[test]
fn pdf_export_files_are_current() {
    let (start, finished) = (pdf_export_start(), pdf_export_finished());
    check_file(PDF, "project.json", &start);
    check_file(PDF, "finished.json", &finished);
    assert_lint(&[(PDF, &start)]);
    extra_lint(PDF, &start);
}

// ---------------------------------------------------------------------------
// What the lessons promise, checked on the projects.

fn page_by_title<'a>(project: &'a Project, title: &str) -> &'a Page {
    project
        .pages()
        .find(|p| p.title == title)
        .unwrap_or_else(|| panic!("no page titled `{title}`"))
}

fn device_with_suffix<'a>(project: &'a Project, suffix: &str) -> Option<&'a Device> {
    project
        .devices()
        .find(|d| d.tag.to_string().ends_with(suffix))
}

fn symbols_on(project: &Project, title: &str) -> usize {
    let page = page_by_title(project, title).id;
    project.symbols().filter(|s| s.page == page).count()
}

/// The sheet names the outline lists, in order, with their report.
fn sheets(project: &Project) -> Vec<(String, String)> {
    novo_reports::LiveReports::standard()
        .generated_sheets(project)
        .into_iter()
        .map(|s| (s.name, s.report))
        .collect()
}

fn report_rows(project: &Project, id: &str) -> Vec<Vec<String>> {
    let output = novo_reports::generate_all(project)
        .into_iter()
        .find(|r| r.report == id)
        .expect("report");
    output.sections.iter().flat_map(|s| s.text_rows()).collect()
}

fn names(list: &[(String, String)]) -> Vec<&str> {
    list.iter().map(|(n, _)| n.as_str()).collect()
}

#[test]
fn generated_pages_start_and_finish_as_the_lesson_says() {
    let (start, finished) = (generated_pages_start(), generated_pages_finished());
    // Opening the project adds nothing: every kind is already there.
    assert!(novo_reports::default_generated_pages(&start).is_empty());
    assert!(novo_reports::default_generated_pages(&finished).is_empty());
    // The pages the lesson walks through, in this order, with these sheets.
    assert_eq!(
        names(&sheets(&start)),
        ["0a", "0b", "10", "20", "30", "40", "50a", "50b"]
    );
    for title in ["Table of contents", "Control circuit", "Device list"] {
        page_by_title(&start, title);
    }
    // Step place-lamp: false at the start, true at the end.
    assert_eq!(symbols_on(&start, "Control circuit"), 8);
    assert!(symbols_on(&finished, "Control circuit") >= 9);
    assert!(device_with_suffix(&start, "-P1").is_none());
    let lamp = device_with_suffix(&finished, "-P1").expect("the lamp is a device");
    assert_eq!(lamp.tag.to_string(), "=A1+CAB-P1");
    // Step new-row: the device list follows, and sits where the pointer is.
    let (before, after) = (
        report_rows(&start, "devices"),
        report_rows(&finished, "devices"),
    );
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after[4][0], "=A1+CAB-P1", "the fifth row, y = 59 mm");
    // The lamp has no part yet, so the bill of materials gains a second sheet:
    // 10 becomes 10a and 10b (step sheet-names).
    assert_eq!(
        names(&sheets(&finished)),
        ["0a", "0b", "10a", "10b", "20", "30", "40", "50a", "50b"]
    );
}

#[test]
fn wire_list_and_bom_start_and_finish_as_the_lesson_says() {
    let (start, finished) = (wire_list_and_bom_start(), wire_list_and_bom_finished());
    assert!(novo_reports::default_generated_pages(&start).is_empty());
    assert!(novo_reports::default_generated_pages(&finished).is_empty());
    // Step find-wire: wire 4 is the fourth row.
    let wires = report_rows(&start, "wires");
    assert_eq!(wires.len(), 21);
    assert_eq!(wires[3][..3], ["4", "=A1+CAB-X1:1", "=A1+FLD-M1:U1"]);
    assert_eq!(wires[3][6], "=A1-W1 BK", "core BK of cable -W1");
    // Step read-cable-schedule: -W1 uses 4 of 4 cores.
    let cables = report_rows(&start, "cables");
    assert_eq!(cables[0][0], "=A1-W1");
    assert_eq!(cables[0][5..], ["4", "4", "0"]);
    // Steps read-parts and see-missing: two BOM sheets, 3044076 counted three
    // times, the lamp alone under "Devices without parts".
    assert_eq!(
        names(&sheets(&start)),
        ["0a", "0b", "10a", "10b", "20", "30", "40", "50a", "50b"]
    );
    let bom = report_rows(&start, "bom");
    assert_eq!(bom[1][1..4], ["3", "Phoenix Contact", "3044076"]);
    assert_eq!(bom.last().unwrap()[0], "=A1+CAB-P1");
    assert_eq!(bom.len(), 10 + 1);
    // Step assign-part: false at the start, true at the end.
    let lamp = |p: &Project| device_with_suffix(p, "-P1").map(|d| d.parts.len());
    assert_eq!(lamp(&start), Some(0));
    assert_eq!(lamp(&finished), Some(1));
    // Step bom-follows: one sheet named 10 again, the lamp the tenth row.
    assert_eq!(
        names(&sheets(&finished)),
        ["0a", "0b", "10", "20", "30", "40", "50a", "50b"]
    );
    let bom = report_rows(&finished, "bom");
    assert_eq!(bom.len(), 11);
    assert_eq!(bom[9][3], "3SU1106-6AA40-1AA0", "the tenth row, y = 89 mm");
    assert_eq!(bom[9][1], "1", "Qty 1");
}

#[test]
fn pdf_export_start_and_finish_as_the_lesson_says() {
    let (start, finished) = (pdf_export_start(), pdf_export_finished());
    assert!(start.latest_revision().is_none());
    let latest = finished.latest_revision().expect("revision A is issued");
    assert_eq!(latest.index, "A");
    for page in finished.pages() {
        let generated = matches!(page.kind, novo_core::PageKind::Generated { .. });
        assert_eq!(
            page.frozen.as_ref().map(|f| f.revision.as_str()),
            if generated { None } else { Some("A") },
            "page {}",
            page.name
        );
    }
    // The export the lesson asks for, and what the person checks in it.
    let bytes = novo_pdf::export_pdf(&finished, &novo_pdf::PdfOptions::default()).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    for bookmark in [
        "(1  Main circuit)",
        "(2  Control circuit)",
        "(10  Bill of materials)",
    ] {
        assert!(text.contains(bookmark), "bookmark {bookmark}");
    }
    assert!(
        text.contains("Go to page 2"),
        "-K1 links to its coil on page 2"
    );
    assert!(text.contains("Go to page 1"), "and back");
    assert!(
        text.contains("TV / JV"),
        "drawn / checked in the title block"
    );
    // Opening an issued project must not add generated pages: they exist.
    assert!(novo_reports::default_generated_pages(&finished).is_empty());
}

// ---------------------------------------------------------------------------
// Extra lint of lesson.toml: what the shared lint does not look at.

fn lesson_toml(id: &str) -> toml::Table {
    let path = super::common::lessons_dir().join(id).join("lesson.toml");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .parse()
        .expect("parses")
}

/// Every `{ area = { page, x, y, w, h } }` and `{ page = ".." }` names a page
/// of the project, areas lie on the sheet, and every English text has its
/// Dutch twin.
fn extra_lint(id: &str, project: &Project) {
    let doc = lesson_toml(id);
    let mut problems = Vec::new();
    let titles: Vec<&str> = project.pages().map(|p| p.title.as_str()).collect();
    for key in ["title", "summary"] {
        if doc.get(&format!("{key}_nl")).is_none() {
            problems.push(format!("lesson {key} has no _nl"));
        }
    }
    for step in doc["step"].as_array().unwrap() {
        let step = step.as_table().unwrap();
        let sid = step["id"].as_str().unwrap();
        for key in ["title", "text"] {
            if step.get(&format!("{key}_nl")).is_none() {
                problems.push(format!("{sid}: {key} has no _nl"));
            }
        }
        let mut targets: Vec<&toml::Value> = Vec::new();
        targets.extend(step.get("point"));
        if let Some(notes) = step.get("note").and_then(|n| n.as_array()) {
            for note in notes {
                if note.get("text_nl").is_none() {
                    problems.push(format!("{sid}: a note has no text_nl"));
                }
                targets.push(note);
            }
            if notes.len() > 3 {
                problems.push(format!("{sid}: more than three notes"));
            }
        }
        for t in targets {
            if let Some(a) = t.get("area") {
                let page = a["page"].as_str().unwrap_or_default();
                if !titles.contains(&page) {
                    problems.push(format!("{sid}: area page `{page}` is not a page title"));
                }
                let num = |k: &str| {
                    a[k].as_float()
                        .or(a[k].as_integer().map(|i| i as f64))
                        .unwrap()
                };
                let (x, y, w, h) = (num("x"), num("y"), num("w"), num("h"));
                if x < 0.0 || y < 0.0 || x + w > 420.0 || y + h > 297.0 {
                    problems.push(format!("{sid}: area is off the A3 sheet"));
                }
            }
        }
        let check = step.get("expect").and_then(|e| e.as_table());
        for (kind, value) in check.into_iter().flatten() {
            let page = match kind.as_str() {
                "page" => value.as_str(),
                "count" => value.get("page").and_then(|p| p.as_str()),
                _ => None,
            };
            if let Some(page) = page
                && !titles.contains(&page)
            {
                problems.push(format!("{sid}: expect page `{page}` is not a page title"));
            }
        }
        for op in step
            .get("solve")
            .and_then(|s| s.as_array())
            .into_iter()
            .flatten()
        {
            if let Some(page) = op.get("page").and_then(|p| p.as_str())
                && !titles.contains(&page)
            {
                problems.push(format!("{sid}: solve page `{page}` is not a page title"));
            }
        }
    }
    assert!(problems.is_empty(), "{id}:\n{}", problems.join("\n"));
}
