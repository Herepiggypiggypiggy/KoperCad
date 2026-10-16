//! Lesson projects for the cables track: `cables-and-shields` and
//! `wire-numbers` (both share one small plant, a pump station).
//!
//! The plant is drawn through the same editor calls the program makes
//! (`novo_editor::edit::place` and `draw_wire`), so tags, terminals, pins
//! and wire routes are what a person would get. The cable steps then call
//! the novo-cables engine exactly as the Cables drawer and the C tool do
//! (`accept_cable_proposals`, `make_cable` in `novo-app/src/app/drawers.rs`).
//! Steps the program cannot do yet (part and length of a cable, a shield)
//! call the engine directly; see the `# GAP:` comments in the lessons.

use novo_cables::{
    CableProposal, ShieldConnection, accept_proposals, add_shield, check, connect_shield,
    proposal_for_wires, propose_cables,
};
use novo_core::{
    Cable, CableId, DeviceId, DeviceTag, Document, Endpoint, Page, PageId, PinRef, Placement,
    Point, PotentialKind, Project, Rotation, SymbolId, Tx, Value, Wire, WireId,
};
use novo_editor::catalog::Catalog;
use novo_editor::edit::{PlaceOptions, draw_wire, place};
use novo_editor::geometry::Target;
use novo_parts::{Catalog as PartsCatalog, assign_cable_part};
use novo_reports::{Report, WireList, add_generated_pages, default_generated_pages};

use crate::common::{assert_lint, check_file};

// ----- helpers --------------------------------------------------------------

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("lesson tags are valid")
}

struct Drawing<'a> {
    catalog: &'a Catalog,
    page: PageId,
}

impl Drawing<'_> {
    /// Places a library symbol the way the editor does.
    fn put(
        &self,
        tx: &mut Tx<'_>,
        key: &str,
        at: (f64, f64),
        rotation: Rotation,
        options: PlaceOptions,
    ) -> SymbolId {
        let symbol = self.catalog.get(key).expect("symbol is in the library");
        let placement = Placement {
            origin: Point::new(at.0, at.1),
            rotation,
            mirrored: false,
        };
        place(tx, self.page, symbol, placement, &options).expect("symbol is placed")
    }

    fn device(&self, tx: &mut Tx<'_>, key: &str, at: (f64, f64), parent: Option<&str>) -> SymbolId {
        let mut options = PlaceOptions::new();
        options.parent = parent.map(tag);
        self.put(tx, key, at, Rotation::R0, options)
    }

    fn feed(&self, tx: &mut Tx<'_>, at: (f64, f64), potential: novo_core::PotentialId) -> SymbolId {
        let mut options = PlaceOptions::new();
        options.potential = Some(potential);
        self.put(
            tx,
            "iec/connectivity/potential_feed",
            at,
            Rotation::R0,
            options,
        )
    }

    /// Draws a wire between two pins, routed like a drawn one.
    fn wire(&self, tx: &mut Tx<'_>, a: (SymbolId, u16), b: (SymbolId, u16)) -> WireId {
        let target = |tx: &Tx<'_>, (symbol, pin): (SymbolId, u16)| {
            let end = Endpoint::pin(symbol, pin);
            let at = tx
                .project()
                .endpoint_position(end)
                .expect("the pin has a position");
            Target::Endpoint(end, at)
        };
        let (from, to) = (target(tx, a), target(tx, b));
        draw_wire(tx, self.page, from, &[], to).expect("wire is drawn")
    }
}

fn find_device(project: &Project, text: &str) -> DeviceId {
    project
        .devices()
        .find(|d| d.tag.to_string().ends_with(text))
        .unwrap_or_else(|| panic!("no device {text}"))
        .id
}

/// The wire between two pins, by designation (`=P1+CAB-X1:4`).
fn wire_between(project: &Project, a: &str, b: &str) -> WireId {
    let name = |end: Endpoint| {
        end.as_pin()
            .and_then(|pin: PinRef| project.pin_designation(pin))
            .unwrap_or_default()
    };
    // A pattern ending in `:` names a whole strip or device (`-X1:`).
    let is = |designation: &str, pattern: &str| {
        if pattern.ends_with(':') {
            designation.contains(pattern)
        } else {
            designation.ends_with(pattern)
        }
    };
    project
        .wires()
        .find(|w| {
            let (f, t) = (name(w.from), name(w.to));
            (is(&f, a) && is(&t, b)) || (is(&f, b) && is(&t, a))
        })
        .unwrap_or_else(|| panic!("no wire between {a} and {b}"))
        .id
}

fn cable_named(project: &Project, text: &str) -> CableId {
    project
        .cables()
        .find(|c| c.tag.to_string().ends_with(text))
        .unwrap_or_else(|| panic!("no cable {text}"))
        .id
}

// ----- the plant -----------------------------------------------------------

const DESCRIPTION: &str = "A pump station for the cables lessons: a pump motor and a level \
transmitter in the field (+FLD), wired to a terminal strip and a PLC input in the cabinet \
(+CAB). Wires cross from the strip to the field and no cable is drawn yet. The control \
circuit is not drawn. Parts are illustrative, not a verified parts list.";

/// The pump station with its wires and no cables: the start of
/// `cables-and-shields`.
pub fn cables_and_shields_start() -> Project {
    let mut doc = Document::default();
    let catalog = Catalog::builtin();
    doc.transact("Draw pump station", |tx| {
        tx.set_info(|i| {
            i.name = "Pump station cables".into();
            i.description = DESCRIPTION.into();
        })?;
        let page = tx.add_page("1")?;
        tx.update::<Page>(page, |p| {
            p.title = "Pump station".into();
            p.structure = tag("=P1+CAB");
        })?;
        let d = Drawing {
            catalog: &catalog,
            page,
        };

        let l1 = tx.add_potential("L1", PotentialKind::Line)?;
        let l2 = tx.add_potential("L2", PotentialKind::Line)?;
        let l3 = tx.add_potential("L3", PotentialKind::Line)?;
        let pe = tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
        let v24 = tx.add_potential("+24 V", PotentialKind::DcPositive)?;
        tx.update::<novo_core::Potential>(v24, |p| p.voltage = Some(24.0))?;

        // Cabinet: supply, contactor, the terminal strip.
        let feeds = [
            d.feed(tx, (40.0, 20.0), l1),
            d.feed(tx, (48.0, 20.0), l2),
            d.feed(tx, (56.0, 20.0), l3),
        ];
        let feed_pe = d.feed(tx, (72.0, 20.0), pe);
        let feed_24 = d.feed(tx, (88.0, 40.0), v24);
        let k1 = d.device(tx, "iec/contactors/main_3p", (40.0, 40.0), None);

        let terminal = |tx: &mut Tx<'_>, key: &str, x: f64| d.device(tx, key, (x, 96.0), None);
        let x1 = [
            terminal(tx, "iec/terminals/terminal", 40.0),
            terminal(tx, "iec/terminals/terminal", 48.0),
            terminal(tx, "iec/terminals/terminal", 56.0),
            terminal(tx, "iec/terminals/terminal_pe", 72.0),
            terminal(tx, "iec/terminals/terminal", 88.0),
            terminal(tx, "iec/terminals/terminal", 96.0),
        ];
        // The PLC analog input faces down, onto terminal 5.
        // It is a channel of PLC card -A1 (the library letter code is K, so
        // the card's tag is typed, as a designer would).
        let mut up = PlaceOptions::new();
        up.parent = Some(tag("=P1+CAB-A1"));
        let a1 = d.put(tx, "iec/plc/analog_input", (96.0, 76.0), Rotation::R180, up);

        // Field: motor and level transmitter.
        let m1 = d.device(
            tx,
            "iec/motors/motor_3ph",
            (40.0, 128.0),
            Some("=P1+FLD-M1"),
        );
        let b1 = d.device(
            tx,
            "iec/sensors/transmitter",
            (88.0, 128.0),
            Some("=P1+FLD-B1"),
        );

        // Cabinet wires.
        for (i, feed) in feeds.iter().enumerate() {
            let i = u16::try_from(i).expect("three feeds");
            d.wire(tx, (*feed, 0), (k1, 2 * i));
            d.wire(tx, (k1, 2 * i + 1), (x1[usize::from(i)], 0));
        }
        d.wire(tx, (feed_pe, 0), (x1[3], 0));
        d.wire(tx, (feed_24, 0), (x1[4], 0));
        d.wire(tx, (x1[5], 0), (a1, 0));
        // Wires that leave the cabinet.
        for i in 0..3u16 {
            d.wire(tx, (x1[usize::from(i)], 1), (m1, i));
        }
        d.wire(tx, (x1[3], 1), (m1, 3));
        d.wire(tx, (x1[4], 1), (b1, 0));
        d.wire(tx, (x1[5], 1), (b1, 1));

        // Descriptions, parts. Illustrative catalogue entries.
        let project = tx.project();
        let strip = find_device(project, "-X1");
        let names = [
            ("-K1", "Pump contactor"),
            ("-A1", "Level input"),
            ("-M1", "Pump motor 2.2 kW"),
            ("-B1", "Level transmitter 4-20 mA"),
            ("-X1", "Field terminals"),
        ];
        let ids: Vec<(DeviceId, &str)> = names
            .iter()
            .map(|(t, text)| (find_device(project, t), *text))
            .collect();
        for (device, text) in ids {
            tx.update::<novo_core::Device>(device, |dev| dev.description = (*text).into())?;
        }
        let parts = [
            ("-K1", "Siemens", "3RT2016-1AP01"),
            ("-A1", "Siemens", "6ES7531-7KF00-0AB0"),
            ("-M1", "Siemens", "1LE1003-0EB42-2AA4"),
            ("-B1", "Endress+Hauser", "PMC21-AA1U1PBWJA1"),
        ];
        for (t, maker, order) in parts {
            let device = find_device(tx.project(), t);
            let part = tx.add_part(maker, order)?;
            tx.assign_part(device, part)?;
        }
        for (kind, order) in [("feedthrough", "3044076"), ("protective_earth", "3044089")] {
            let part = tx.add_part("Phoenix Contact", order)?;
            tx.update::<novo_core::Device>(strip, |dev| {
                dev.properties.insert(
                    format!("terminal_part.{kind}"),
                    Value::Int(i64::try_from(part.0).expect("small id")),
                );
            })?;
        }
        Ok(())
    })
    .expect("the pump station is drawn");
    doc.into_project()
}

/// What the lesson steps do, through the engine calls the program uses.
/// `GAP` steps have no screen control yet; they call the engine directly.
pub fn cables_and_shields_finished() -> Project {
    let mut doc = Document::new(cables_and_shields_start());

    // Step: the C tool across the transmitter's two wires (make_cable).
    doc.transact("Draw cable", |tx| {
        let p = tx.project();
        let wires = [
            wire_between(p, "-X1:4", "-B1:+"),
            wire_between(p, "-X1:5", "-B1:-"),
        ];
        let proposal = proposal_for_wires(p, &wires).expect("wires land on devices");
        accept_proposals(tx, &[proposal], None)
    })
    .expect("cable is drawn");

    // Step: "Accept 1 proposed" in the Cables drawer (accept_cable_proposals).
    doc.transact("Accept cables", |tx| {
        let proposals: Vec<CableProposal> = propose_cables(tx.project());
        accept_proposals(tx, &proposals, None)
    })
    .expect("cable is accepted");

    // GAP step: part and length of the motor cable. Part from the starter
    // catalog through the same command the property sheet will use.
    let motor = cable_named(doc.project(), "-W2");
    let catalog = PartsCatalog::starter();
    let part = catalog
        .find("Lapp", "1119304")
        .expect("starter catalog has the 4G1.5 cable");
    let command = assign_cable_part(doc.project(), motor, part).expect("part changes the cable");
    doc.execute(command).expect("part is assigned");
    doc.transact("Cable length", |tx| {
        tx.update::<Cable>(motor, |c| c.length_m = Some(12.0))
    })
    .expect("length is set");

    // GAP step: a shield on the transmitter cable, earthed at the cabinet.
    let signal = cable_named(doc.project(), "-W1");
    doc.transact("Shield", |tx| {
        let core = add_shield(tx, signal)?;
        connect_shield(tx, signal, core, Some(ShieldConnection::Pe))?;
        tx.update::<Cable>(signal, |c| c.length_m = Some(18.0))
    })
    .expect("shield is added");
    doc.into_project()
}

// ----- wire numbers ---------------------------------------------------------

/// The pump station after the cables lesson, with three motor wires
/// numbered by hand (the last number is a typo: 2 twice) and the generated
/// pages the program adds on open. Nothing else has a number.
pub fn wire_numbers_start() -> Project {
    let mut doc = Document::new(cables_and_shields_finished());
    doc.transact("Prepare wire numbers", |tx| {
        tx.set_info(|i| {
            i.name = "Pump station wire numbers".into();
            i.description = "The pump station of the cables lessons, with its cables, for the \
wire number lesson. Parts are illustrative, not a verified parts list."
                .into();
        })?;
        for (end, number) in [("-M1:U1", "1"), ("-M1:V1", "2"), ("-M1:W1", "2")] {
            let wire = wire_between(tx.project(), end, "-X1:");
            tx.update::<Wire>(wire, |w| w.number = Some(number.into()))?;
        }
        Ok(())
    })
    .expect("project is prepared");
    // The program adds the generated pages when a project opens.
    let specs = default_generated_pages(doc.project());
    doc.transact("Generated pages", |tx| add_generated_pages(tx, &specs))
        .expect("generated pages are added");
    doc.into_project()
}

/// The wire numbers the lesson ends with, by a pin each net touches: what
/// "Number wires", the typed `number wire 100` and "Renumber wires" leave
/// (the app test `wire_numbers_each_instruction_works_in_the_app` replays
/// the steps and compares).
pub const WIRE_NUMBERS_FINISHED: &[(&str, &str)] = &[
    ("-K1:1", "1"),
    ("-X1:1", "2"),
    ("-K1:3", "3"),
    ("-K1:5", "4"),
    ("-X1:PE", "5"),
    ("-X1:2", "6"),
    ("-X1:3", "7"),
    ("-X1:5", "8"),
    ("-X1:4", "100"),
];

/// The lesson steps done: every net numbered, the level loop typed as 100.
pub fn wire_numbers_finished() -> Project {
    let mut doc = Document::new(wire_numbers_start());
    doc.transact("Number wires", |tx| {
        for (pin, number) in WIRE_NUMBERS_FINISHED {
            let wires: Vec<WireId> = tx
                .project()
                .wires()
                .filter(|w| {
                    [w.from, w.to].iter().any(|end| {
                        end.as_pin()
                            .and_then(|p| tx.project().pin_designation(p))
                            .is_some_and(|d| d.ends_with(pin))
                    })
                })
                .map(|w| w.id)
                .collect();
            assert!(!wires.is_empty(), "no wire at {pin}");
            for wire in wires {
                tx.update::<Wire>(wire, |w| w.number = Some((*number).into()))?;
            }
        }
        Ok(())
    })
    .expect("numbers are set");
    doc.into_project()
}

// ----- tests ----------------------------------------------------------------

#[test]
fn cables_and_shields_files_are_current() {
    let start = cables_and_shields_start();
    check_file("cables-and-shields", "project.json", &start);
    check_file(
        "cables-and-shields",
        "finished.json",
        &cables_and_shields_finished(),
    );
    assert_lint(&[("cables-and-shields", &start)]);
}

#[test]
fn wire_numbers_files_are_current() {
    let start = wire_numbers_start();
    check_file("wire-numbers", "project.json", &start);
    check_file("wire-numbers", "finished.json", &wire_numbers_finished());
    assert_lint(&[("wire-numbers", &start)]);
}

#[test]
fn the_pump_station_starts_with_two_proposals_and_no_cable() {
    let start = cables_and_shields_start();
    assert_eq!(start.cables().count(), 0);
    let proposals = propose_cables(&start);
    assert_eq!(proposals.len(), 2, "motor and transmitter");
    let names: Vec<String> = proposals.iter().map(|p| p.cable_type.name()).collect();
    assert!(names.contains(&"4G1.5".to_string()), "{names:?}");
    // No terminal proposals: the strip is already drawn.
    assert!(novo_terminals::suggest_terminals(&start).is_empty());
}

#[test]
fn every_cables_step_expect_holds_on_the_finished_project() {
    let finished = cables_and_shields_finished();
    let tags: Vec<String> = finished.cables().map(|c| c.tag.to_string()).collect();
    assert_eq!(tags, ["=P1-W1", "=P1-W2"], "C tool first, then Accept");
    let w1 = finished.cable(cable_named(&finished, "-W1")).unwrap();
    assert!(w1.cores.len() >= 3, "{:?}", w1.cores);
    let w2 = finished.cable(cable_named(&finished, "-W2")).unwrap();
    assert!(w2.cores.len() >= 4);
    assert_eq!(
        w2.cores.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["BN", "BK", "GY", "GNYE"]
    );
    assert!(w2.part.is_some(), "part step");
    assert_eq!(w2.length_m, Some(12.0));
    assert!(w1.cores.iter().any(|c| c.shield), "shield step");
    // The shield is connected, so the cable checks have nothing to say.
    assert!(check(&finished).is_empty(), "{:?}", check(&finished));
    // Every proposal is gone.
    assert!(propose_cables(&finished).is_empty());
    // The cable diagram page would appear on the next open.
    assert!(
        default_generated_pages(&finished)
            .iter()
            .any(|s| s.generator == "cables"),
        "the cable diagram is proposed once cables exist"
    );
}

#[test]
fn the_unshielded_signal_cable_has_nothing_to_report_before_the_shield_step() {
    // Right after the C tool and Accept the checks are quiet, so the GAP
    // shield step is what introduces the shield and its check.
    let mut doc = Document::new(cables_and_shields_start());
    doc.transact("cables", |tx| {
        let proposals = propose_cables(tx.project());
        accept_proposals(tx, &proposals, None)
    })
    .unwrap();
    assert!(
        check(doc.project()).is_empty(),
        "{:?}",
        check(doc.project())
    );
    let signal = doc
        .project()
        .cables()
        .find(|c| c.cores.len() == 3)
        .expect("numbered 3x1.5 signal cable")
        .id;
    // Adding a shield without connecting it is a finding.
    doc.transact("shield", |tx| add_shield(tx, signal).map(|_| ()))
        .unwrap();
    assert_eq!(check(doc.project()).len(), 1);
}

#[test]
fn every_wire_numbers_step_expect_holds_on_the_finished_project() {
    let start = wire_numbers_start();
    let finished = wire_numbers_finished();
    // The start has the typo (2 twice) and only three numbers; the finished one numbers every wire.
    let numbers =
        |p: &Project| -> Vec<Option<String>> { p.wires().map(|w| w.number.clone()).collect() };
    assert_eq!(numbers(&start).iter().flatten().count(), 3);
    assert!(numbers(&finished).iter().all(Option::is_some));
    let number =
        |p: &Project, a: &str, b: &str| p.wire(wire_between(p, a, b)).unwrap().number.clone();
    assert_eq!(number(&start, "-M1:V1", "-X1:").as_deref(), Some("2"));
    assert_eq!(number(&start, "-M1:W1", "-X1:").as_deref(), Some("2"));
    assert_eq!(number(&finished, "-B1:+", "-X1:4").as_deref(), Some("100"));

    // The wire list shows the numbers and the potential and cable core columns.
    let list = WireList.generate(&finished);
    let rows = list.sections[0].rows.len();
    assert_eq!(rows, 10, "K1 to strip 3, strip to field 6, input 1");
    let text = format!("{:?}", list.sections[0]);
    assert!(text.contains("+24 V"), "potential column");
    assert!(text.contains("=P1-W2 GY"), "cable core column");

    // The generated pages are on the project, as the program adds them.
    let titles: Vec<&str> = start.pages().map(|p| p.title.as_str()).collect();
    assert!(titles.contains(&"Wire list"), "{titles:?}");
    // The cable drawer shows the potential of each core.
    let screen = novo_cables::cable_screen(&finished, cable_named(&finished, "-W1")).unwrap();
    assert_eq!(screen.rows[0].potential.as_deref(), Some("+24 V"));
}
