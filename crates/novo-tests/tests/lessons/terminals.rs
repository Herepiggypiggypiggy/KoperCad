//! Lesson projects of the `terminals` track written by the terminals thread:
//! `terminal-strips`, `connection-screens` and `interruption-points`.
//!
//! Everything is placed through `novo_editor::edit::place` (what the canvas
//! calls on a click), so pins, bridges and pass-through paths are the symbol
//! library's. Terminals, cables and generated pages come from the same
//! engine calls the app makes (`accept_suggestions`, `accept_proposals`,
//! `add_generated_pages`).

use novo_core::{
    Device, DeviceId, DeviceTag, Document, Endpoint, PageId, Placement, Point, PotentialId,
    PotentialKind, Project, Result, Rotation, SymbolId, SymbolRole, TextAnchor, TextContent,
    TextPlacement, Tx, Wire, WireId,
};
use novo_editor::edit::{self, PlaceOptions};
use novo_symbols::Library;

use super::common::{assert_lint, check_file};

// ----- small helpers --------------------------------------------------------

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("lesson tags are valid")
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Places library symbols the way a click does.
struct Kit {
    library: Library,
}

impl Kit {
    fn new() -> Self {
        Kit {
            library: Library::builtin().expect("the built-in library loads"),
        }
    }

    /// A symbol at `(x, y)`. `device` names the device the symbol is the
    /// main function of (or an auxiliary contact of); `potential` is the
    /// potential of a feed.
    #[allow(clippy::too_many_arguments)]
    fn put(
        &self,
        tx: &mut Tx<'_>,
        page: PageId,
        key: &str,
        device: Option<&str>,
        potential: Option<PotentialId>,
        at: (f64, f64),
        rotation: Rotation,
    ) -> Result<SymbolId> {
        let symbol = self
            .library
            .get(key)
            .unwrap_or_else(|| panic!("`{key}` is in the library"));
        let options = PlaceOptions {
            parent: device.map(tag),
            potential,
            auto_connect: false,
            ..PlaceOptions::new()
        };
        edit::place(
            tx,
            page,
            symbol,
            Placement {
                origin: pt(at.0, at.1),
                rotation,
                mirrored: false,
            },
            &options,
        )
    }

    fn at(
        &self,
        tx: &mut Tx<'_>,
        page: PageId,
        key: &str,
        device: &str,
        at: (f64, f64),
    ) -> Result<SymbolId> {
        self.put(tx, page, key, Some(device), None, at, Rotation::R0)
    }

    fn feed(
        &self,
        tx: &mut Tx<'_>,
        page: PageId,
        potential: PotentialId,
        at: (f64, f64),
    ) -> Result<SymbolId> {
        self.put(
            tx,
            page,
            "iec/connectivity/potential_feed",
            None,
            Some(potential),
            at,
            Rotation::R0,
        )
    }
}

fn pin(project: &Project, symbol: SymbolId, name: &str) -> Endpoint {
    let s = project.symbol(symbol).expect("symbol exists");
    Endpoint::pin(
        symbol,
        s.pin_index(name)
            .unwrap_or_else(|| panic!("pin {name} on {}", s.symbol.0)),
    )
}

/// A wire between two pins with its bend points. A routed wire is marked as
/// drawn by hand, as the editor does for clicked corners.
fn wire(
    tx: &mut Tx<'_>,
    from: (SymbolId, &str),
    to: (SymbolId, &str),
    route: &[(f64, f64)],
) -> Result<WireId> {
    let a = pin(tx.project(), from.0, from.1);
    let b = pin(tx.project(), to.0, to.1);
    let id = tx.connect(a, b)?;
    if !route.is_empty() {
        let route: Vec<Point> = route.iter().map(|&(x, y)| pt(x, y)).collect();
        tx.update::<Wire>(id, |w| {
            w.route = route;
            w.hand_drawn = true;
        })?;
    }
    Ok(id)
}

fn label(tx: &mut Tx<'_>, page: PageId, text: &str, at: (f64, f64)) -> Result<()> {
    tx.add_text(
        page,
        TextContent::Literal { text: text.into() },
        TextPlacement {
            offset: pt(at.0, at.1),
            rotation: Rotation::R0,
            anchor: TextAnchor::TopLeft,
        },
        None,
    )?;
    Ok(())
}

fn describe(tx: &mut Tx<'_>, tag_text: &str, description: &str) -> Result<()> {
    let device = tx
        .project()
        .devices_by_tag(&tag(tag_text))
        .first()
        .copied()
        .unwrap_or_else(|| panic!("device {tag_text}"));
    tx.update::<Device>(device, |d| d.description = description.into())
}

/// A catalogue entry for a device. Illustrative, as the project says.
fn part(tx: &mut Tx<'_>, tag_text: &str, manufacturer: &str, order: &str) -> Result<()> {
    let device = tx
        .project()
        .devices_by_tag(&tag(tag_text))
        .first()
        .copied()
        .unwrap_or_else(|| panic!("device {tag_text}"));
    let part = tx.add_part(manufacturer, order)?;
    tx.assign_part(device, part)
}

/// The generated pages the app adds on open (contents, lists, diagrams).
fn add_generated(doc: &mut Document) {
    let wanted = |_: &str| true;
    doc.transact("Generated pages", |tx| {
        let specs = novo_reports::default_generated_pages_with(tx.project(), &wanted);
        novo_reports::add_generated_pages(tx, &specs)
    })
    .expect("generated pages");
}

/// The terminal blocks of every strip (the strip's part per terminal kind,
/// as the Terminals drawer's style sets it). Illustrative articles.
fn strip_parts(doc: &mut Document) {
    use std::collections::BTreeMap;
    doc.transact("Terminal blocks", |tx| {
        let mut parts: BTreeMap<&'static str, novo_core::PartId> = BTreeMap::new();
        for strip in novo_terminals::strips(tx.project()) {
            let kinds: Vec<novo_core::TerminalKind> = {
                let mut k: Vec<_> = tx
                    .project()
                    .terminals_of_strip(strip)
                    .iter()
                    .map(|t| t.kind)
                    .collect();
                k.dedup();
                k
            };
            for kind in kinds {
                let order = match kind {
                    novo_core::TerminalKind::ProtectiveEarth => "3044089",
                    _ => "3044076",
                };
                let part = match parts.get(order) {
                    Some(p) => *p,
                    None => {
                        let p = tx.add_part("Phoenix Contact", order)?;
                        parts.insert(order, p);
                        p
                    }
                };
                tx.set_property(
                    strip,
                    novo_terminals::strip_part_key(kind),
                    Some(novo_core::Value::Int(
                        i64::try_from(part.raw()).expect("small id"),
                    )),
                )?;
            }
        }
        Ok(())
    })
    .expect("terminal blocks");
}

fn page_named(project: &Project, title: &str) -> PageId {
    project
        .pages()
        .find(|p| p.title == title)
        .map(|p| p.id)
        .unwrap_or_else(|| panic!("page {title}"))
}

fn device_named(project: &Project, tag_text: &str) -> DeviceId {
    project
        .devices_by_tag(&tag(tag_text))
        .first()
        .copied()
        .unwrap_or_else(|| panic!("device {tag_text}"))
}

fn terminal_named(project: &Project, strip: &str, name: &str) -> novo_core::TerminalId {
    let strip = device_named(project, strip);
    project
        .terminals_of_strip(strip)
        .into_iter()
        .find(|t| t.name == name)
        .map(|t| t.id)
        .unwrap_or_else(|| panic!("terminal {name}"))
}

// ----- the pump and valve feeder (lessons 1 and 2) ---------------------------

const FEEDER_PAGE: &str = "Pump and valve feeder";
const FEEDER_DESCRIPTION: &str = "A pump with a frequency converter and two 24 V valves. \
    The converter and the breakers stand in the cabinet (+CAB), the pump and the valves in \
    the field (+FLD), so every wire between them crosses a location boundary and needs a \
    terminal. The parts are illustrative catalogue entries, not a verified parts list.";

/// The drawing without terminals: seven devices in two locations, fourteen
/// wires, six of which cross from `+CAB` to `+FLD`.
fn feeder_drawing() -> Document {
    let kit = Kit::new();
    let mut doc = Document::new(Project::new("Pump and valve feeder"));
    doc.transact("Draw the pump and valve feeder", |tx| {
        tx.set_info(|i| {
            i.description = FEEDER_DESCRIPTION.into();
            // The lessons teach one strip per location; new projects put
            // power, control, signals and PE on separate strips.
            i.properties.insert(
                novo_terminals::GROUPING_KEY.into(),
                novo_core::Value::Text(novo_terminals::Grouping::Single.key().into()),
            );
        })?;
        let page = tx.add_page("1")?;
        tx.update::<novo_core::Page>(page, |p| p.title = FEEDER_PAGE.into())?;

        let l1 = tx.add_potential("L1", PotentialKind::Line)?;
        let l2 = tx.add_potential("L2", PotentialKind::Line)?;
        let l3 = tx.add_potential("L3", PotentialKind::Line)?;
        let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
        let p24 = tx.add_potential("24V", PotentialKind::DcPositive)?;
        let p0 = tx.add_potential("0V", PotentialKind::DcNegative)?;

        label(tx, page, "Cabinet +CAB", (28.0, 4.0))?;
        label(tx, page, "Field +FLD", (28.0, 128.0))?;

        // Supplies.
        let feeds = [(l1, 40.0), (l2, 48.0), (l3, 56.0), (pe, 64.0)]
            .map(|(p, x)| (p, x))
            .into_iter()
            .map(|(p, x)| kit.feed(tx, page, p, (x, 12.0)))
            .collect::<Result<Vec<_>>>()?;
        let feed_24a = kit.feed(tx, page, p24, (112.0, 12.0))?;
        let feed_0a = kit.feed(tx, page, p0, (120.0, 12.0))?;
        let feed_24b = kit.feed(tx, page, p24, (136.0, 12.0))?;
        let feed_0b = kit.feed(tx, page, p0, (144.0, 12.0))?;

        // Cabinet.
        let q1 = kit.at(tx, page, "iec/protection/mcb_3p", "+CAB-Q1", (40.0, 36.0))?;
        let u1 = kit.at(
            tx,
            page,
            "iec/drives/frequency_converter",
            "+CAB-U1",
            (40.0, 68.0),
        )?;
        let f2 = kit.at(tx, page, "iec/protection/mcb_2p", "+CAB-F2", (112.0, 44.0))?;
        let f3 = kit.at(tx, page, "iec/protection/mcb_2p", "+CAB-F3", (136.0, 44.0))?;

        // Field.
        let m1 = kit.at(tx, page, "iec/motors/motor_3ph", "+FLD-M1", (40.0, 148.0))?;
        let y1 = kit.at(
            tx,
            page,
            "iec/coils/solenoid_valve",
            "+FLD-Y1",
            (112.0, 148.0),
        )?;
        let y2 = kit.at(
            tx,
            page,
            "iec/coils/solenoid_valve",
            "+FLD-Y2",
            (136.0, 148.0),
        )?;

        // Cabinet wiring.
        for (feed, pole) in feeds.iter().take(3).zip(["1", "3", "5"]) {
            wire(tx, (*feed, "1"), (q1, pole), &[])?;
        }
        for (pole, input) in [("2", "L1"), ("4", "L2"), ("6", "L3")] {
            wire(tx, (q1, pole), (u1, input), &[])?;
        }
        wire(tx, (feeds[3], "1"), (u1, "PE"), &[])?;
        wire(tx, (feed_24a, "1"), (f2, "1"), &[])?;
        wire(tx, (feed_0a, "1"), (f2, "3"), &[])?;
        wire(tx, (feed_24b, "1"), (f3, "1"), &[])?;
        wire(tx, (feed_0b, "1"), (f3, "3"), &[])?;

        // Wires that cross from the cabinet to the field.
        wire(tx, (u1, "U"), (m1, "U1"), &[])?;
        wire(tx, (u1, "V"), (m1, "V1"), &[])?;
        wire(tx, (u1, "W"), (m1, "W1"), &[])?;
        wire(tx, (u1, "PE2"), (m1, "PE"), &[])?;
        wire(tx, (f2, "2"), (y1, "A1"), &[])?;
        wire(tx, (f2, "4"), (y1, "A2"), &[(120.0, 172.0), (112.0, 172.0)])?;
        wire(tx, (f3, "2"), (y2, "A1"), &[])?;
        wire(tx, (f3, "4"), (y2, "A2"), &[(144.0, 172.0), (136.0, 172.0)])?;

        for (t, d) in [
            ("+CAB-Q1", "Pump circuit breaker"),
            ("+CAB-U1", "Pump frequency converter"),
            ("+CAB-F2", "Valve 1 breaker, 24 V"),
            ("+CAB-F3", "Valve 2 breaker, 24 V"),
            ("+FLD-M1", "Pump motor 1.5 kW"),
            ("+FLD-Y1", "Inlet valve"),
            ("+FLD-Y2", "Outlet valve"),
        ] {
            describe(tx, t, d)?;
        }
        for (t, maker, order) in [
            ("+CAB-Q1", "Siemens", "5SY6316-7"),
            ("+CAB-U1", "Siemens", "6SL3210-5BB17-5UV1"),
            ("+CAB-F2", "Siemens", "5SY6202-7"),
            ("+CAB-F3", "Siemens", "5SY6202-7"),
            ("+FLD-M1", "Siemens", "1LE1001-0DA22-2AA4"),
            ("+FLD-Y1", "Buerkert", "6213-A-2.0-G1/4"),
            ("+FLD-Y2", "Buerkert", "6213-A-2.0-G1/4"),
        ] {
            part(tx, t, maker, order)?;
        }
        Ok(())
    })
    .expect("the feeder draws");
    doc
}

/// Lesson 1 start: the drawing and the pages the app adds on open.
pub fn terminal_strips_start() -> Project {
    let mut doc = feeder_drawing();
    add_generated(&mut doc);
    doc.project().clone()
}

/// Lesson 1 finished: the six proposals accepted (the first with Enter, the
/// rest with accept all, which numbers the same way), the two 0 V terminals
/// connected, the PE terminal switched by hand and handed back to the
/// automatic rule, then moved to the strip `-X2`.
pub fn terminal_strips_finished() -> Project {
    let mut doc = Document::new(terminal_strips_start());
    // Enter accepts the highlighted proposal (the first in drawing order).
    doc.transact("Accept terminal", |tx| {
        let first = novo_terminals::suggest_terminals(tx.project());
        novo_terminals::accept_suggestions(tx, &first[..1])
    })
    .expect("first terminal");
    // Accept all takes the rest.
    doc.transact("Accept all terminals", |tx| {
        let rest = novo_terminals::suggest_terminals(tx.project());
        novo_terminals::accept_suggestions(tx, &rest)
    })
    .expect("all terminals");

    let project = doc.project();
    let zero_volt: Vec<_> = {
        let strip = device_named(project, "+CAB-X1");
        let mut names: Vec<_> = project
            .terminals_of_strip(strip)
            .into_iter()
            .filter(|t| t.name == "5" || t.name == "7")
            .map(|t| t.id)
            .collect();
        names.sort();
        names
    };
    let pe = terminal_named(project, "+CAB-X1", "PE");
    doc.transact("Connect terminals", |tx| {
        novo_terminals::connect_terminals(tx, &zero_volt)
    })
    .expect("jumper");
    doc.transact("Toggle PE terminal", |tx| {
        novo_terminals::toggle_pe(tx, &[pe])
    })
    .expect("toggle");
    doc.transact("Reset PE override", |tx| {
        novo_terminals::reset_pe_override(tx, &[pe])?;
        novo_terminals::mark_earthed(tx).map(|_| ())
    })
    .expect("reset");
    doc.transact("Move terminals to another strip", |tx| {
        novo_terminals::move_to_strip(tx, &[pe], novo_terminals::MoveTo::NextNumber).map(|_| ())
    })
    .expect("move");
    doc.project().clone()
}

#[test]
fn terminal_strips_files_are_current() {
    let start = terminal_strips_start();
    let finished = terminal_strips_finished();
    check_file("terminal-strips", "project.json", &start);
    check_file("terminal-strips", "finished.json", &finished);
    assert_lint(&[("terminal-strips", &start)]);
}

// ----- connection-screens ------------------------------------------------------

/// The feeder with its strip: every proposal accepted, the two 0 V
/// terminals bridged, the wires numbered on both sides of the strip and the
/// pump cable `-W1` drawn. The valve cables are still proposals.
fn feeder_with_strip() -> Document {
    let mut doc = feeder_drawing();
    doc.transact("Accept all terminals", |tx| {
        let all = novo_terminals::suggest_terminals(tx.project());
        novo_terminals::accept_suggestions(tx, &all).map(|_| ())
    })
    .expect("terminals");

    let project = doc.project();
    let jumpered = [
        terminal_named(project, "+CAB-X1", "5"),
        terminal_named(project, "+CAB-X1", "7"),
    ];
    doc.transact("Connect terminals", |tx| {
        novo_terminals::connect_terminals(tx, &jumpered)
    })
    .expect("jumper");

    // Wire numbers: 101.. on the cabinet side, 201.. on the field side, in
    // the order of the strip's rows.
    let project = doc.project();
    let strip = device_named(project, "+CAB-X1");
    let view = novo_terminals::strip_view(project, strip);
    let mut numbers: Vec<(WireId, String)> = Vec::new();
    for (i, row) in view.rows.iter().enumerate() {
        let n = i + 1;
        for o in &row.occurrences {
            numbers.extend(
                o.internal
                    .wires
                    .iter()
                    .map(|&w| (w, format!("{}", 100 + n))),
            );
            numbers.extend(
                o.external
                    .wires
                    .iter()
                    .map(|&w| (w, format!("{}", 200 + n))),
            );
        }
    }
    doc.transact("Number the wires", |tx| {
        for (wire, number) in &numbers {
            tx.update::<Wire>(*wire, |w| w.number = Some(number.clone()))?;
        }
        Ok(())
    })
    .expect("numbers");

    // The pump cable, as the Cables drawer proposes it.
    doc.transact("Accept cable", |tx| {
        let proposals = novo_cables::propose_cables(tx.project());
        let motor = device_named(tx.project(), "+FLD-M1");
        let pump = proposals
            .into_iter()
            .find(|p| p.ends.iter().any(|e| e.device == motor))
            .expect("a cable is proposed for the pump");
        let created = novo_cables::accept_proposals(tx, &[pump], None)?;
        let part = tx.add_part("Lapp", "1119304")?;
        tx.update::<novo_core::Cable>(created[0], |c| {
            c.part = Some(part);
            c.length_m = Some(12.0);
            c.description = "Pump cable 4G1.5".into();
        })
    })
    .expect("cable");
    doc
}

/// Lesson 2 start: a finished strip with a cable on the pump, and the
/// generated pages, among them the terminal diagram.
pub fn connection_screens_start() -> Project {
    let mut doc = feeder_with_strip();
    doc.transact("Describe the project", |tx| {
        tx.set_info(|i| {
            i.name = "Pump feeder, terminal strip -X1".into();
            i.description = format!(
                "{FEEDER_DESCRIPTION} The strip +CAB-X1 is finished and the pump cable is drawn; \
                 the valve cables are still proposals."
            );
        })
    })
    .expect("info");
    strip_parts(&mut doc);
    add_generated(&mut doc);
    doc.project().clone()
}

/// Lesson 2 finished: the two valve cables accepted in the Cables drawer
/// and terminal 4 renamed to 11 (the same edit as the double-click on its
/// name).
pub fn connection_screens_finished() -> Project {
    let mut doc = Document::new(connection_screens_start());
    doc.transact("Accept cables", |tx| {
        let proposals = novo_cables::propose_cables(tx.project());
        novo_cables::accept_proposals(tx, &proposals, None).map(|_| ())
    })
    .expect("valve cables");
    let project = doc.project();
    let four = terminal_named(project, "+CAB-X1", "4");
    let symbol = project
        .symbols_of_terminal(four)
        .first()
        .expect("terminal 4 is drawn")
        .id;
    doc.transact("Rename terminal", |tx| {
        edit::rename_label(tx, symbol, "11").map_err(novo_core::ModelError::Format)
    })
    .expect("rename");
    doc.project().clone()
}

#[test]
fn connection_screens_files_are_current() {
    let start = connection_screens_start();
    let finished = connection_screens_finished();
    check_file("connection-screens", "project.json", &start);
    check_file("connection-screens", "finished.json", &finished);
    assert_lint(&[("connection-screens", &start)]);
}

// ----- interruption-points -----------------------------------------------------

const SIGNAL_DESCRIPTION: &str = "A start signal that leaves page 1 and continues on page 2, \
    where it energises the coil of contactor -K1. Two contacts of -K1 on page 3 switch an \
    indicator lamp each. The parts are illustrative catalogue entries, not a verified parts list.";

/// Lesson 3 start: three pages, no interruption points yet.
pub fn interruption_points_start() -> Project {
    let kit = Kit::new();
    let mut doc = Document::new(Project::new("Start signal across pages"));
    doc.transact("Draw the start signal", |tx| {
        tx.set_info(|i| i.description = SIGNAL_DESCRIPTION.into())?;
        let one = tx.add_page("1")?;
        let two = tx.add_page("2")?;
        let three = tx.add_page("3")?;
        for (page, title) in [
            (one, "Start signal"),
            (two, "Contactor coil"),
            (three, "Indicator lamps"),
        ] {
            tx.update::<novo_core::Page>(page, |p| p.title = title.into())?;
        }
        let l = tx.add_potential("L", PotentialKind::Line)?;
        let n = tx.add_potential("N", PotentialKind::Neutral)?;
        let top = |kit: &Kit, tx: &mut Tx<'_>, page, x| kit.feed(tx, page, l, (x, 12.0));
        // A feed hanging below a coil or lamp is turned half a turn.
        let bottom = |kit: &Kit, tx: &mut Tx<'_>, page, x, y: f64| {
            kit.put(
                tx,
                page,
                "iec/connectivity/potential_feed",
                None,
                Some(n),
                (x, y + 8.0),
                Rotation::R180,
            )
        };

        // Page 1: supply, breaker, stop and start. The start button's lower
        // pin is left open: the signal continues on page 2.
        let feed = top(&kit, tx, one, 40.0)?;
        let f1 = kit.at(tx, one, "iec/protection/mcb_1p", "-F1", (40.0, 28.0))?;
        let s0 = kit.at(tx, one, "iec/operators/pushbutton_nc", "-S0", (40.0, 56.0))?;
        let s1 = kit.at(tx, one, "iec/operators/pushbutton_no", "-S1", (40.0, 88.0))?;
        wire(tx, (feed, "1"), (f1, "1"), &[])?;
        wire(tx, (f1, "2"), (s0, "11"), &[])?;
        wire(tx, (s0, "12"), (s1, "13"), &[])?;

        // Page 2: the coil, with its neutral below.
        let k1 = kit.at(tx, two, "iec/coils/relay", "-K1", (40.0, 64.0))?;
        let neutral = bottom(&kit, tx, two, 40.0, 96.0)?;
        wire(tx, (k1, "A2"), (neutral, "1"), &[])?;

        // Page 3: two contacts of -K1, each switching a lamp.
        for (x, key, contact, lamp_key, lamp) in [
            (
                40.0,
                "iec/contacts/no",
                "-K1",
                "iec/signalling/lamp_green",
                "-H1",
            ),
            (
                72.0,
                "iec/contacts/nc",
                "-K1",
                "iec/signalling/lamp_red",
                "-H2",
            ),
        ] {
            let feed = top(&kit, tx, three, x)?;
            let c = kit.at(tx, three, key, contact, (x, 28.0))?;
            let h = kit.at(tx, three, lamp_key, lamp, (x, 60.0))?;
            let neutral = bottom(&kit, tx, three, x, 92.0)?;
            let (a, b) = {
                let s = tx.project().symbol(c).expect("contact");
                (s.pins[0].name.clone(), s.pins[1].name.clone())
            };
            wire(tx, (feed, "1"), (c, &a), &[])?;
            wire(tx, (c, &b), (h, "X1"), &[])?;
            wire(tx, (h, "X2"), (neutral, "1"), &[])?;
        }

        for (t, d) in [
            ("-F1", "Control circuit breaker"),
            ("-S0", "Stop"),
            ("-S1", "Start"),
            ("-K1", "Pump contactor"),
            ("-H1", "Running lamp"),
            ("-H2", "Stopped lamp"),
        ] {
            describe(tx, t, d)?;
        }
        for (t, maker, order) in [
            ("-F1", "Siemens", "5SY6106-7"),
            ("-S0", "Siemens", "3SU1100-0AB20-1CA0"),
            ("-S1", "Siemens", "3SU1100-0AB40-1BA0"),
            ("-K1", "Siemens", "3RT2016-1AP01"),
            ("-H1", "Siemens", "3SU1156-6AA40-1AA0"),
            ("-H2", "Siemens", "3SU1156-6AA20-1AA0"),
        ] {
            part(tx, t, maker, order)?;
        }
        Ok(())
    })
    .expect("the signal draws");
    add_generated(&mut doc);
    doc.project().clone()
}

/// Lesson 3 finished: a point under `-S1`, its partner above the coil
/// (turned half a turn so the arrow points up the sheet), both named alike
/// and numbered.
pub fn interruption_points_finished() -> Project {
    let kit = Kit::new();
    let mut doc = Document::new(interruption_points_start());
    let one = page_named(doc.project(), "Start signal");
    let two = page_named(doc.project(), "Contactor coil");
    let (s1, k1) = {
        let p = doc.project();
        (
            p.symbols_of_device(device_named(p, "-S1"))[0].id,
            p.symbols_of_device(device_named(p, "-K1"))
                .into_iter()
                .find(|s| matches!(s.role, SymbolRole::Device { main: true, .. }))
                .expect("the coil")
                .id,
        )
    };
    let key = "iec/connectivity/interruption_point";
    let (first, second) = doc
        .transact("Interruption points", |tx| {
            // Placing names the first IP1 and the second IP2, as the editor does.
            let first = kit.put(tx, one, key, None, None, (40.0, 120.0), Rotation::R0)?;
            wire(tx, (s1, "14"), (first, "1"), &[])?;
            let second = kit.put(tx, two, key, None, None, (40.0, 40.0), Rotation::R180)?;
            wire(tx, (second, "1"), (k1, "A1"), &[])?;
            Ok((first, second))
        })
        .expect("points")
        .0;
    doc.transact("Rename the partner", |tx| {
        edit::rename_label(tx, second, "IP1").map_err(novo_core::ModelError::Format)
    })
    .expect("rename");
    doc.transact("Number interruption points", |tx| {
        novo_xref::renumber_interruptions(tx, &[]).map(|_| ())
    })
    .expect("renumber");
    let _ = first;
    doc.project().clone()
}

#[test]
fn interruption_points_files_are_current() {
    let start = interruption_points_start();
    let finished = interruption_points_finished();
    check_file("interruption-points", "project.json", &start);
    check_file("interruption-points", "finished.json", &finished);
    assert_lint(&[("interruption-points", &start)]);
}

// ----- what each lesson's steps expect, asked of the model -----------------------

fn symbols_on(project: &Project, title: &str) -> usize {
    project
        .page_contents(page_named(project, title))
        .symbols
        .len()
}

fn strip_names(project: &Project, strip: &str) -> Vec<String> {
    let strip = device_named(project, strip);
    novo_terminals::strip_view(project, strip)
        .rows
        .iter()
        .map(|r| r.name.clone())
        .collect()
}

#[test]
fn terminal_strips_steps_hold_on_the_finished_project() {
    let start = terminal_strips_start();
    let finished = terminal_strips_finished();

    // The start has eight proposals and no strip; the checks are false there.
    assert_eq!(novo_terminals::suggest_terminals(&start).len(), 8);
    assert!(start.terminals().next().is_none());
    assert!(
        start
            .devices()
            .all(|d| d.kind != novo_core::DeviceKind::TerminalStrip)
    );
    assert_eq!(symbols_on(&start, FEEDER_PAGE), 15);

    // accept-one / accept-all / open-drawer / select-two / connect / move.
    assert_eq!(symbols_on(&finished, FEEDER_PAGE), 23);
    assert_eq!(
        strip_names(&finished, "+CAB-X1"),
        ["1", "2", "3", "4", "5", "6", "7"]
    );
    assert_eq!(strip_names(&finished, "+CAB-X2"), ["PE"]);
    let strip = device_named(&finished, "+CAB-X1");
    let jumpers: Vec<(String, Option<u32>)> = finished
        .terminals_of_strip(strip)
        .iter()
        .map(|t| (t.name.clone(), t.jumper))
        .collect();
    assert_eq!(
        jumpers
            .iter()
            .filter(|(_, j)| j.is_some())
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>(),
        ["5", "7"]
    );
    // The PE terminal is PE again after the override was reset.
    let pe = terminal_named(&finished, "+CAB-X2", "PE");
    assert_eq!(
        finished.terminal(pe).unwrap().kind,
        novo_core::TerminalKind::ProtectiveEarth
    );
    // Nothing is left to propose.
    assert!(novo_terminals::suggest_terminals(&finished).is_empty());
}

#[test]
fn connection_screens_steps_hold_on_the_finished_project() {
    let start = connection_screens_start();
    let finished = connection_screens_finished();

    // Start: -W1 only, two valve cables proposed, the generated pages present.
    assert!(start.cables().any(|c| c.tag.to_string() == "-W1"));
    assert!(!start.cables().any(|c| c.tag.to_string() == "-W2"));
    assert_eq!(novo_cables::propose_cables(&start).len(), 2);
    let titles: Vec<&str> = start.pages().map(|p| p.title.as_str()).collect();
    assert!(titles.contains(&"Terminal diagram"), "{titles:?}");
    assert_eq!(
        strip_names(&start, "+CAB-X1"),
        ["1", "2", "3", "PE", "4", "5", "6", "7"]
    );

    // Finished: the valve cables and the new number.
    let w2 = finished
        .cables()
        .find(|c| c.tag.to_string() == "-W2")
        .unwrap();
    assert!(w2.cores.iter().filter(|c| c.wire.is_some()).count() >= 2);
    assert!(finished.cables().any(|c| c.tag.to_string() == "-W3"));
    assert_eq!(
        strip_names(&finished, "+CAB-X1"),
        ["1", "2", "3", "PE", "11", "5", "6", "7"]
    );
    assert!(novo_cables::propose_cables(&finished).is_empty());

    // The diagram follows: its Cable column names -W2 on the valve rows.
    let strip = device_named(&finished, "+CAB-X1");
    let screen = novo_terminals::screen(&finished, strip);
    assert!(
        screen
            .rows
            .iter()
            .any(|r| r.cables.iter().any(|c| c.starts_with("-W2")))
    );
    let before = novo_terminals::screen(&start, device_named(&start, "+CAB-X1"));
    assert!(
        before
            .rows
            .iter()
            .all(|r| r.cables.iter().all(|c| !c.starts_with("-W2")))
    );
}

#[test]
fn interruption_points_steps_hold_on_the_finished_project() {
    let start = interruption_points_start();
    let finished = interruption_points_finished();

    // The checks of the drop steps are false at the start.
    assert_eq!(symbols_on(&start, "Start signal"), 4);
    assert_eq!(symbols_on(&start, "Contactor coil"), 2);
    assert!(
        start
            .symbols()
            .all(|s| !matches!(s.role, SymbolRole::Interruption { .. }))
    );
    // And true on the finished project.
    assert_eq!(symbols_on(&finished, "Start signal"), 5);
    assert_eq!(symbols_on(&finished, "Contactor coil"), 3);

    // Both points are named 1 and each shows the other's page.column.
    let names: Vec<String> = finished
        .symbols()
        .filter_map(|s| match &s.role {
            SymbolRole::Interruption { name } => Some(name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["1", "1"]);
    let index = novo_xref::XrefIndex::build(&finished, &novo_xref::FromKey);
    let lines = |title: &str| -> Vec<Vec<String>> {
        index
            .labels_on_page(page_named(&finished, title))
            .into_iter()
            .map(|l| l.lines)
            .collect()
    };
    assert_eq!(lines("Start signal"), [["2.1"]]);
    assert_eq!(lines("Contactor coil")[0], ["1.1"]);
    assert_eq!(lines("Contactor coil")[1], ["13-14 3.1", "21-22 3.2"]);
    assert!(index.issues().is_empty(), "{:?}", index.issues());

    // The pair is one net: no lone interruption point, no open pin.
    assert!(novo_checks::check(&finished).is_empty());
    let start_problems = novo_checks::check(&start);
    assert_eq!(start_problems.len(), 2, "two open pins in the start");
}
