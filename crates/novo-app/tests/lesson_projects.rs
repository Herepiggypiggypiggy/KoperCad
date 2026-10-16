//! The sample projects of the first lessons (`assets/lessons/<id>/`), built
//! with the same editor calls the program uses, so a start state and a
//! finished state are exactly what a person would draw.
//!
//! The committed `project.json` and `finished.json` files are the output of
//! this file. `cargo test -p novo-app --test lesson_projects` fails when a
//! file differs from what the builders make now; run it with
//! `KOPERCAD_UPDATE_LESSONS=1` to write the files again.
//!
//! The tests also check what each lesson teaches against the engine: that a
//! finished drawing has the wires and tags the lesson text names, that the
//! start state really lacks the work of the first step, and that the live
//! view lights what the lesson says it lights.

use std::collections::BTreeSet;
use std::path::PathBuf;

use novo_core::{
    DeviceTag, Document, Endpoint, PageId, Placement, Point, PotentialKind, Project, Rotation,
    SymbolId, Tx, WireId,
};
use novo_editor::edit::{self, PlaceOptions};
use novo_editor::geometry::Target;
use novo_editor::lines::draw_wire_exact;
use novo_sim::{PoleState, settle};
use novo_symbols::Library;

// ----- drawing helpers -----------------------------------------------------

/// A page under construction.
struct Draw<'a, 'b> {
    tx: &'a mut Tx<'b>,
    lib: &'a Library,
    page: PageId,
}

type Pin<'a> = (SymbolId, &'a str);

impl Draw<'_, '_> {
    fn options(&self) -> PlaceOptions {
        // Drawn by hand, wire by wire: facing pins are not joined at the
        // drop, the builders say which wires a lesson has at each stage.
        PlaceOptions {
            auto_connect: false,
            ..PlaceOptions::new()
        }
    }

    fn put(&mut self, key: &str, x: f64, y: f64, options: PlaceOptions) -> SymbolId {
        let symbol = self
            .lib
            .get(key)
            .unwrap_or_else(|| panic!("the library has no `{key}`"));
        edit::place(
            self.tx,
            self.page,
            symbol,
            Placement::at(Point::new(x, y)),
            &options,
        )
        .unwrap_or_else(|e| panic!("placing `{key}`: {e}"))
    }

    /// A device with a tag of its own (`-F1`, `-S1`, ...).
    fn symbol(&mut self, key: &str, x: f64, y: f64) -> SymbolId {
        let options = self.options();
        self.put(key, x, y, options)
    }

    /// The contact, coil or other symbol of an existing device (`coil K1`,
    /// `no K1` in the command field).
    fn of(&mut self, key: &str, x: f64, y: f64, device: &str) -> SymbolId {
        let options = PlaceOptions {
            parent: Some(DeviceTag::parse(device).expect("a valid tag")),
            ..self.options()
        };
        self.put(key, x, y, options)
    }

    /// A potential feed. `up` turns it round for the bottom of a circuit.
    fn feed(&mut self, name: &str, kind: PotentialKind, x: f64, y: f64, up: bool) -> SymbolId {
        let potential = edit::find_or_add_potential(self.tx, name, kind, None).expect("potential");
        let symbol = self
            .lib
            .get("iec/connectivity/potential_feed")
            .expect("feed");
        let options = PlaceOptions {
            potential: Some(potential),
            ..self.options()
        };
        let placement = Placement {
            rotation: if up { Rotation::R180 } else { Rotation::R0 },
            ..Placement::at(Point::new(x, y))
        };
        edit::place(self.tx, self.page, symbol, placement, &options).expect("feed")
    }

    fn line_feed(&mut self, name: &str, x: f64, y: f64) -> SymbolId {
        self.feed(name, PotentialKind::Line, x, y, false)
    }

    fn neutral(&mut self, x: f64, y: f64) -> SymbolId {
        self.feed("N", PotentialKind::Neutral, x, y, true)
    }

    fn describe(&mut self, symbol: SymbolId, text: &str) {
        let device = self
            .tx
            .project()
            .symbol(symbol)
            .and_then(|s| s.device())
            .expect("a device symbol");
        self.tx
            .update::<novo_core::Device>(device, |d| d.description = text.into())
            .expect("description");
    }

    fn target(&self, (symbol, pin): Pin<'_>) -> Target {
        let s = self.tx.project().symbol(symbol).expect("symbol");
        let index = s
            .pin_index(pin)
            .unwrap_or_else(|| panic!("no pin {pin} on {}", s.symbol.0));
        let end = Endpoint::pin(symbol, index);
        let at = self.tx.project().endpoint_position(end).expect("position");
        Target::Endpoint(end, at)
    }

    fn points(corners: &[(f64, f64)]) -> Vec<Point> {
        corners.iter().map(|&(x, y)| Point::new(x, y)).collect()
    }

    /// A wire the router lays between two pins, through the clicked corners
    /// (Corners mode, or a drag from a pin).
    fn wire(&mut self, from: Pin<'_>, corners: &[(f64, f64)], to: Pin<'_>) -> WireId {
        let (a, b) = (self.target(from), self.target(to));
        edit::draw_wire(self.tx, self.page, a, &Self::points(corners), b)
            .unwrap_or_else(|e| panic!("wire {}:{} to {}:{}: {e}", from.0.0, from.1, to.0.0, to.1))
    }

    /// A wire drawn segment by segment, as in Lines mode.
    fn line(&mut self, from: Pin<'_>, corners: &[(f64, f64)], to: Pin<'_>) -> WireId {
        let (a, b) = (self.target(from), self.target(to));
        draw_wire_exact(self.tx, self.page, a, &Self::points(corners), b)
            .unwrap_or_else(|e| panic!("line {}:{} to {}:{}: {e}", from.0.0, from.1, to.0.0, to.1))
    }

    /// A wire from a pin onto an existing wire at `at`: a T-junction.
    fn tee(&mut self, from: Pin<'_>, corners: &[(f64, f64)], onto: WireId, at: (f64, f64)) {
        let a = self.target(from);
        let end = Target::Wire(onto, Point::new(at.0, at.1));
        edit::draw_wire(self.tx, self.page, a, &Self::points(corners), end)
            .unwrap_or_else(|e| panic!("tee from {}:{}: {e}", from.0.0, from.1));
    }
}

fn library() -> Library {
    Library::builtin().expect("the built-in library loads")
}

/// A new project as the program makes one, with the pages named.
fn project(
    name: &str,
    description: &str,
    pages: &[(&str, &str)],
    draw: impl FnOnce(&mut Draw<'_, '_>, &[PageId]) -> novo_core::Result<()>,
) -> Document {
    let lib = library();
    let mut doc = Document::new(Project::new(name));
    doc.transact("Lesson project", |tx| {
        let mut ids = Vec::new();
        for (page, title) in pages.iter() {
            let id = tx.add_page(*page)?;
            let title = (*title).to_owned();
            tx.update::<novo_core::Page>(id, |p| p.title = title)?;
            ids.push(id);
        }
        tx.set_info(|i| i.description = description.into())?;
        let mut d = Draw {
            tx,
            lib: &lib,
            page: ids[0],
        };
        draw(&mut d, &ids)
    })
    .unwrap_or_else(|e| panic!("building {name}: {e}"));
    doc
}

const ILLUSTRATIVE: &str = "Sample drawing for a KoperCAD lesson. Device descriptions are \
    illustrative, not a verified parts list.";

// ----- lesson: placing-parts -----------------------------------------------

/// The control branch: L1, a breaker, a push button, a relay coil, N.
fn control_branch(d: &mut Draw<'_, '_>, parts: bool) {
    let l1 = d.line_feed("L1", 80.0, 20.0);
    let n1 = d.neutral(80.0, 140.0);
    if parts {
        let f1 = d.symbol("iec/protection/mcb_1p", 80.0, 40.0);
        let s1 = d.symbol("iec/operators/pushbutton_no", 80.0, 72.0);
        let k1 = d.symbol("iec/coils/relay", 80.0, 104.0);
        d.describe(f1, "Control circuit breaker");
        d.describe(s1, "Start button");
        d.describe(k1, "Motor contactor coil");
        d.wire((l1, "1"), &[], (f1, "1"));
        d.wire((f1, "2"), &[], (s1, "13"));
        d.wire((s1, "14"), &[], (k1, "A1"));
        d.wire((k1, "A2"), &[], (n1, "1"));
    }
}

fn placing_parts(finished: bool) -> Document {
    project(
        "Lesson: Place parts",
        ILLUSTRATIVE,
        &[("1", "Control branch")],
        |d, _| {
            control_branch(d, finished);
            Ok(())
        },
    )
}

// ----- lesson: wiring-modes -------------------------------------------------

/// Two branches, each with one wire that needs a bend: the left one is
/// drawn in Corners, the right one in Lines. The wire from the breaker to
/// the left button is left for the drag from a pin.
fn wiring_modes(finished: bool) -> Document {
    project(
        "Lesson: Corners and Lines",
        ILLUSTRATIVE,
        &[("1", "Two start buttons")],
        |d, _| {
            let l1 = d.line_feed("L1", 60.0, 20.0);
            let f1 = d.symbol("iec/protection/mcb_1p", 60.0, 40.0);
            let s1 = d.symbol("iec/operators/pushbutton_no", 60.0, 72.0);
            let k1 = d.symbol("iec/coils/relay", 100.0, 104.0);
            let n1 = d.neutral(100.0, 140.0);
            let l2 = d.line_feed("L1", 140.0, 20.0);
            let f2 = d.symbol("iec/protection/mcb_1p", 140.0, 40.0);
            let s2 = d.symbol("iec/operators/pushbutton_no", 140.0, 72.0);
            let k2 = d.symbol("iec/coils/relay", 180.0, 104.0);
            let n2 = d.neutral(180.0, 140.0);
            d.describe(f1, "Breaker, left branch");
            d.describe(s1, "Start button, left branch");
            d.describe(k1, "Relay, left branch");
            d.describe(f2, "Breaker, right branch");
            d.describe(s2, "Start button, right branch");
            d.describe(k2, "Relay, right branch");
            d.wire((l1, "1"), &[], (f1, "1"));
            d.wire((k1, "A2"), &[], (n1, "1"));
            d.wire((l2, "1"), &[], (f2, "1"));
            d.wire((f2, "2"), &[], (s2, "13"));
            d.wire((k2, "A2"), &[], (n2, "1"));
            if finished {
                d.wire((f1, "2"), &[], (s1, "13"));
                d.wire((s1, "14"), &[(100.0, 96.0)], (k1, "A1"));
                d.line((s2, "14"), &[(140.0, 96.0), (180.0, 96.0)], (k2, "A1"));
            }
            Ok(())
        },
    )
}

// ----- lesson: potentials ----------------------------------------------------

/// A motor feeder drawn without its supply: breaker, contactor, motor.
/// Finished, it has the three line feeds and the earth feed.
fn potentials(finished: bool) -> Document {
    project(
        "Lesson: Potentials",
        ILLUSTRATIVE,
        &[("1", "Motor feeder")],
        |d, _| {
            let q1 = d.symbol("iec/protection/motor_protection_breaker", 80.0, 40.0);
            let k1 = d.symbol("iec/contactors/main_3p", 80.0, 72.0);
            let m1 = d.symbol("iec/motors/motor_3ph", 80.0, 112.0);
            d.describe(q1, "Motor protection circuit breaker");
            d.describe(k1, "Motor contactor");
            d.describe(m1, "Pump motor 2.2 kW");
            for (top, bottom) in [("2", "1"), ("4", "3"), ("6", "5")] {
                d.wire((q1, top), &[], (k1, bottom));
            }
            for (top, bottom) in [("2", "U1"), ("4", "V1"), ("6", "W1")] {
                d.wire((k1, top), &[], (m1, bottom));
            }
            if finished {
                for (x, name, pin) in [(80.0, "L1", "1"), (88.0, "L2", "3"), (96.0, "L3", "5")] {
                    let feed = d.line_feed(name, x, 20.0);
                    d.wire((feed, "1"), &[], (q1, pin));
                }
                let pe = d.feed("PE", PotentialKind::ProtectiveEarth, 104.0, 92.0, false);
                d.wire((pe, "1"), &[], (m1, "PE"));
            }
            Ok(())
        },
    )
}

// ----- the motor starter (lessons: motor-starter, cross-references, live-view)

/// The power page: the three line feeds and the earth feed at the top, the
/// motor at the bottom. `devices` adds the breaker and the contactor.
fn starter_power(d: &mut Draw<'_, '_>, devices: bool) {
    let feeds: Vec<SymbolId> = [(80.0, "L1"), (88.0, "L2"), (96.0, "L3")]
        .into_iter()
        .map(|(x, name)| d.line_feed(name, x, 20.0))
        .collect();
    let pe = d.feed("PE", PotentialKind::ProtectiveEarth, 104.0, 92.0, false);
    let m1 = d.symbol("iec/motors/motor_3ph", 80.0, 112.0);
    d.describe(m1, "Pump motor 2.2 kW");
    d.wire((pe, "1"), &[], (m1, "PE"));
    if devices {
        let q1 = d.symbol("iec/protection/motor_protection_breaker", 80.0, 40.0);
        let k1 = d.symbol("iec/contactors/main_3p", 80.0, 72.0);
        d.describe(q1, "Motor protection circuit breaker");
        d.describe(k1, "Motor contactor");
        for (feed, pin) in feeds.iter().zip(["1", "3", "5"]) {
            d.wire((*feed, "1"), &[], (q1, pin));
        }
        for (top, bottom) in [("2", "1"), ("4", "3"), ("6", "5")] {
            d.wire((q1, top), &[], (k1, bottom));
        }
        for (top, bottom) in [("2", "U1"), ("4", "V1"), ("6", "W1")] {
            d.wire((k1, top), &[], (m1, bottom));
        }
    }
}

/// The indicator lamp on the control page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lamp {
    /// None drawn.
    No,
    /// The lamp and its feeds, but not the contact between them.
    WithoutContact,
    /// The lamp fed through its own contact of -K1.
    Lit,
}

/// The control page: `hold` draws the self-hold contact in parallel with the
/// start button, or only the two feeds when `parts` is false.
fn starter_control(d: &mut Draw<'_, '_>, parts: bool, lamp: Lamp) {
    let l1 = d.line_feed("L1", 80.0, 20.0);
    let n = d.neutral(80.0, 168.0);
    if !parts {
        return;
    }
    let f1 = d.symbol("iec/protection/mcb_1p", 80.0, 40.0);
    let s1 = d.symbol("iec/operators/pushbutton_nc", 80.0, 72.0);
    let s2 = d.symbol("iec/operators/pushbutton_no", 80.0, 104.0);
    let k1 = d.of("iec/coils/relay", 80.0, 136.0, "-K1");
    d.describe(f1, "Control circuit breaker");
    d.describe(s1, "Stop button");
    d.describe(s2, "Start button");
    d.wire((l1, "1"), &[], (f1, "1"));
    d.wire((f1, "2"), &[], (s1, "11"));
    let above = d.wire((s1, "12"), &[], (s2, "13"));
    let below = d.wire((s2, "14"), &[], (k1, "A1"));
    d.wire((k1, "A2"), &[], (n, "1"));
    // The self-hold contact: two drags onto the wires either side of the
    // start button.
    let hold = d.of("iec/contacts/no", 96.0, 104.0, "-K1");
    d.tee((hold, "13"), &[(96.0, 96.0)], above, (80.0, 96.0));
    d.tee((hold, "14"), &[(96.0, 128.0)], below, (80.0, 128.0));
    if lamp != Lamp::No {
        let lf = d.line_feed("L1", 120.0, 84.0);
        let h1 = d.symbol("iec/signalling/lamp_green", 120.0, 136.0);
        let nl = d.neutral(120.0, 168.0);
        d.describe(h1, "Motor running");
        d.wire((h1, "X2"), &[], (nl, "1"));
        if lamp == Lamp::Lit {
            // The second contact of -K1 takes the next position, 23 and 24.
            let k1b = d.of("iec/contacts/no", 120.0, 104.0, "-K1");
            d.wire((lf, "1"), &[], (k1b, "23"));
            d.wire((k1b, "24"), &[], (h1, "X1"));
        }
    }
}

fn starter_pages() -> [(&'static str, &'static str); 2] {
    [("1", "Main circuit"), ("2", "Control circuit")]
}

const STARTER_NOTE: &str = "Direct-on-line starter for a pump motor: main and control circuit. \
    Sample drawing for a KoperCAD lesson; device descriptions are illustrative, not a verified \
    parts list.";

fn motor_starter(parts: bool, lamp: Lamp, name: &str) -> Document {
    project(name, STARTER_NOTE, &starter_pages(), |d, pages| {
        d.page = pages[0];
        starter_power(d, parts);
        d.page = pages[1];
        starter_control(d, parts, lamp);
        Ok(())
    })
}

// ----- the files -------------------------------------------------------------

/// Every project file of the first lessons: folder, file name, document.
fn all() -> Vec<(&'static str, &'static str, Document)> {
    vec![
        ("placing-parts", "project.json", placing_parts(false)),
        ("placing-parts", "finished.json", placing_parts(true)),
        ("wiring-modes", "project.json", wiring_modes(false)),
        ("wiring-modes", "finished.json", wiring_modes(true)),
        ("potentials", "project.json", potentials(false)),
        ("potentials", "finished.json", potentials(true)),
        (
            "motor-starter",
            "project.json",
            motor_starter(false, Lamp::No, "Lesson: Motor starter"),
        ),
        (
            "motor-starter",
            "finished.json",
            motor_starter(true, Lamp::No, "Lesson: Motor starter (finished)"),
        ),
        (
            "cross-references",
            "project.json",
            motor_starter(true, Lamp::WithoutContact, "Lesson: Cross references"),
        ),
        (
            "cross-references",
            "finished.json",
            motor_starter(true, Lamp::Lit, "Lesson: Cross references (finished)"),
        ),
    ]
}

fn lessons_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/lessons")
}

#[test]
fn the_committed_files_are_what_the_builders_make() {
    let update = std::env::var_os("KOPERCAD_UPDATE_LESSONS").is_some();
    for (folder, file, doc) in all() {
        let path = lessons_dir().join(folder).join(file);
        let json = format!("{}\n", doc.project().to_json());
        if update {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &json).unwrap();
            continue;
        }
        let have = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "{} is missing: run with KOPERCAD_UPDATE_LESSONS=1",
                path.display()
            )
        });
        assert!(
            have == json,
            "{folder}/{file} is out of date: run with KOPERCAD_UPDATE_LESSONS=1"
        );
        let size = have.len();
        assert!(size < 200 * 1024, "{folder}/{file} is {size} bytes");
        Project::from_json(&have).unwrap_or_else(|e| panic!("{folder}/{file} does not load: {e}"));
    }
}

fn describe(project: &Project) -> Vec<String> {
    let mut out = Vec::new();
    for page in project.pages_in_order() {
        out.push(format!("page {} {}", page.name, page.title));
        for s in project.page_contents(page.id).symbols {
            let sym = project.symbol(s).unwrap();
            let tag = sym
                .device()
                .and_then(|d| project.device(d))
                .map(|d| d.tag.to_string())
                .unwrap_or_default();
            let pins: Vec<&str> = sym.pins.iter().map(|p| p.name.as_str()).collect();
            out.push(format!(
                "  {tag:8} {} @({}, {}) {}",
                sym.symbol.0,
                sym.placement.origin.x,
                sym.placement.origin.y,
                pins.join(" ")
            ));
        }
        for w in project.wires().filter(|w| w.page == page.id) {
            let name = |e: Endpoint| match e {
                Endpoint::Pin { symbol, pin } => {
                    let s = project.symbol(symbol).unwrap();
                    let tag = s
                        .device()
                        .and_then(|d| project.device(d))
                        .map(|d| d.tag.to_string())
                        .unwrap_or_else(|| "feed".into());
                    format!("{tag}:{}", s.pins[usize::from(pin)].name)
                }
                other => format!("{other:?}"),
            };
            out.push(format!(
                "  wire {} -> {} via {:?}",
                name(w.from),
                name(w.to),
                w.route
            ));
        }
    }
    out
}

// ----- what the lessons claim -------------------------------------------------

fn symbol_of(project: &Project, tag: &str, key: &str) -> SymbolId {
    project
        .symbols()
        .find(|s| {
            s.symbol.0 == key
                && s.device()
                    .and_then(|d| project.device(d))
                    .is_some_and(|d| d.tag.to_string() == tag)
        })
        .unwrap_or_else(|| panic!("no {key} for {tag}"))
        .id
}

fn wires_at(project: &Project, symbol: SymbolId, pin: &str) -> usize {
    let index = project.symbol(symbol).unwrap().pin_index(pin).unwrap();
    project.wires_at(Endpoint::pin(symbol, index)).count()
}

/// Whether a wire joins the two pins, directly or through junctions.
fn joined(project: &Project, a: (SymbolId, &str), b: (SymbolId, &str)) -> bool {
    let netlist = project.netlist();
    let end = |(s, pin): (SymbolId, &str)| {
        Endpoint::pin(s, project.symbol(s).unwrap().pin_index(pin).unwrap())
    };
    let net = netlist.net_of(end(a));
    net.is_some() && net == netlist.net_of(end(b))
}

#[test]
fn every_start_state_lacks_the_work_of_its_lesson() {
    let wires = |d: &Document| d.project().wires().count();
    assert!(wires(&placing_parts(false)) == 0 && wires(&placing_parts(true)) == 4);
    assert_eq!(wires(&wiring_modes(true)) - wires(&wiring_modes(false)), 3);
    assert_eq!(wires(&potentials(true)) - wires(&potentials(false)), 4);
    let start = motor_starter(false, Lamp::No, "x");
    let done = motor_starter(true, Lamp::No, "y");
    assert_eq!(
        start.project().devices().count(),
        1,
        "only the motor is drawn"
    );
    assert_eq!(done.project().devices().count(), 6);
    // Cross references: the lamp is there, the contact that feeds it is not.
    let wires = |d: &Document| d.project().wires().count();
    let dead = motor_starter(true, Lamp::WithoutContact, "x");
    let lit = motor_starter(true, Lamp::Lit, "y");
    assert_eq!(wires(&lit) - wires(&dead), 2);
}

#[test]
fn the_tags_the_lessons_name_are_the_tags_the_engine_gives() {
    let doc = motor_starter(true, Lamp::No, "x");
    let p = doc.project();
    for (tag, key) in [
        ("-Q1", "iec/protection/motor_protection_breaker"),
        ("-K1", "iec/contactors/main_3p"),
        ("-K1", "iec/coils/relay"),
        ("-K1", "iec/contacts/no"),
        ("-M1", "iec/motors/motor_3ph"),
        ("-F1", "iec/protection/mcb_1p"),
        ("-S1", "iec/operators/pushbutton_nc"),
        ("-S2", "iec/operators/pushbutton_no"),
    ] {
        symbol_of(p, tag, key);
    }
    // The contact the lesson places with `no K1` takes the first auxiliary
    // position, 13 and 14.
    let hold = symbol_of(p, "-K1", "iec/contacts/no");
    assert_eq!(wires_at(p, hold, "13"), 1);
    assert_eq!(wires_at(p, hold, "14"), 1);
    // With the indicator branch drawn first, the self-hold contact is 23/24.
    let doc = motor_starter(true, Lamp::Lit, "x");
    let p = doc.project();
    let contacts: Vec<_> = p
        .symbols()
        .filter(|s| s.symbol.0 == "iec/contacts/no")
        .map(|s| s.pins[0].name.clone())
        .collect();
    assert_eq!(contacts, ["13", "23"]);
}

#[test]
fn the_finished_starter_joins_what_the_lesson_says() {
    let doc = motor_starter(true, Lamp::No, "x");
    let p = doc.project();
    let q1 = symbol_of(p, "-Q1", "iec/protection/motor_protection_breaker");
    let k1 = symbol_of(p, "-K1", "iec/contactors/main_3p");
    let m1 = symbol_of(p, "-M1", "iec/motors/motor_3ph");
    assert!(joined(p, (q1, "2"), (k1, "1")));
    assert!(joined(p, (k1, "6"), (m1, "W1")));
    let s1 = symbol_of(p, "-S1", "iec/operators/pushbutton_nc");
    let s2 = symbol_of(p, "-S2", "iec/operators/pushbutton_no");
    let coil = symbol_of(p, "-K1", "iec/coils/relay");
    let hold = symbol_of(p, "-K1", "iec/contacts/no");
    // The self-hold contact is in parallel with the start button.
    assert!(joined(p, (s1, "12"), (hold, "13")));
    assert!(joined(p, (s1, "12"), (s2, "13")));
    assert!(joined(p, (s2, "14"), (hold, "14")));
    assert!(joined(p, (s2, "14"), (coil, "A1")));
}

/// The live-view lesson, step by step, against the simulation the Shift+L
/// view uses.
#[test]
fn the_live_view_lesson_lights_what_it_says() {
    let doc = motor_starter(true, Lamp::Lit, "x");
    let p = doc.project();
    let netlist = p.netlist();
    let none = BTreeSet::new();
    let q1 = symbol_of(p, "-Q1", "iec/protection/motor_protection_breaker");
    let f1 = symbol_of(p, "-F1", "iec/protection/mcb_1p");
    let s1 = symbol_of(p, "-S1", "iec/operators/pushbutton_nc");
    let s2 = symbol_of(p, "-S2", "iec/operators/pushbutton_no");
    let coil = symbol_of(p, "-K1", "iec/coils/relay");
    let m1 = symbol_of(p, "-M1", "iec/motors/motor_3ph");
    let h1 = symbol_of(p, "-P1", "iec/signalling/lamp_green");

    // Everything open, as the view starts: nothing runs.
    let rest = settle(p, &netlist, &PoleState::all_open(), &none, &none);
    assert!(rest.energized.is_empty());
    assert!(novo_sim::loads_on(p, &netlist, &rest.solution).is_empty());

    // Breakers on, stop button closed, start not pressed: still nothing.
    let mut hand = PoleState::all_open();
    for s in [q1, f1, s1] {
        hand.set_all(p, s, true);
    }
    let ready = settle(p, &netlist, &hand, &none, &none);
    assert!(ready.energized.is_empty());
    assert!(novo_sim::loads_on(p, &netlist, &ready.solution).is_empty());

    // Start pressed: the coil pulls in, the motor and the lamp have current.
    let mut pressed = hand.clone();
    pressed.set_all(p, s2, true);
    let on = settle(p, &netlist, &pressed, &none, &none);
    assert!(on.energized.contains(&coil));
    let loads = novo_sim::loads_on(p, &netlist, &on.solution);
    assert!(loads.contains(&m1) && loads.contains(&h1), "{loads:?}");

    // Start released: the self-hold contact keeps the coil in.
    let held = settle(p, &netlist, &hand, &none, &on.energized);
    assert!(held.energized.contains(&coil));
    let loads = novo_sim::loads_on(p, &netlist, &held.solution);
    assert!(loads.contains(&m1) && loads.contains(&h1));

    // Stop pressed (the NC pole opens): everything drops and stays down.
    let mut stop = hand.clone();
    stop.set_all(p, s1, false);
    let off = settle(p, &netlist, &stop, &none, &held.energized);
    assert!(off.energized.is_empty());
    let after = settle(p, &netlist, &hand, &none, &off.energized);
    assert!(after.energized.is_empty(), "start must be pressed again");

    // A stop button that was never closed keeps the coil off: the view
    // starts with every pole open, the NC stop button included.
    let mut no_stop = PoleState::all_open();
    for s in [q1, f1, s2] {
        no_stop.set_all(p, s, true);
    }
    assert!(
        settle(p, &netlist, &no_stop, &none, &none)
            .energized
            .is_empty()
    );
}

// ----- replaying the lessons through the editor ---------------------------------
//
// The lesson text says what to press and click. These tests do the same
// through the editor the canvas drives (pointer in, model edits out) from
// each lesson's start state and compare the result with its finished state,
// so the finished drawing is what a person following the steps ends with.

use novo_core::{AnyId, Endpoint as End};
use novo_editor::Editor;
use novo_editor::tool::Clicked;

/// Pick radius in millimetres, as at a normal zoom.
const TOL: f64 = 1.5;

struct Replay {
    doc: Document,
    page: PageId,
    editor: Editor,
}

impl Replay {
    fn open(doc: &Document) -> Replay {
        let doc = Document::new(Project::from_json(&doc.project().to_json()).unwrap());
        let page = doc.project().pages_in_order()[0].id;
        Replay {
            doc,
            page,
            editor: Editor::default(),
        }
    }

    /// Page Down / Page Up to the page with this name.
    fn show(&mut self, name: &str) {
        self.page = self
            .doc
            .project()
            .pages_in_order()
            .into_iter()
            .find(|p| p.name == name)
            .unwrap()
            .id;
        self.editor.page_changed();
    }

    fn click(&mut self, x: f64, y: f64) -> Clicked {
        self.editor
            .click(&mut self.doc, self.page, Point::new(x, y), TOL)
    }

    fn pin(&self, tag: &str, key: &str, pin: &str) -> Point {
        let p = self.doc.project();
        let s = symbol_of(p, tag, key);
        let index = p.symbol(s).unwrap().pin_index(pin).unwrap();
        p.endpoint_position(End::pin(s, index)).unwrap()
    }

    /// Q, the name, Enter, then a click where the symbol goes.
    fn place(&mut self, symbol: &str, parent: Option<&str>, x: f64, y: f64) {
        assert!(
            self.editor.begin_place(symbol, parent),
            "{symbol} is placeable"
        );
        assert_eq!(self.click(x, y), Clicked::Edited, "{symbol} at {x},{y}");
        self.editor.leave_tool();
    }

    /// W, then the clicks. The first and last are on pins or wires.
    fn wire(&mut self, clicks: &[(f64, f64)]) {
        self.editor.begin_wire();
        for &(x, y) in clicks {
            self.click(x, y);
        }
        self.editor.leave_tool();
    }

    fn project(&self) -> &Project {
        self.doc.project()
    }
}

fn pin_label(project: &Project, end: Endpoint) -> Option<String> {
    let Endpoint::Pin { symbol, pin } = end else {
        return None;
    };
    let s = project.symbol(symbol)?;
    let who = match &s.role {
        novo_core::SymbolRole::PotentialFeed { potential } => format!(
            "feed {}@{},{}",
            project.potential(*potential)?.name,
            s.placement.origin.x,
            s.placement.origin.y
        ),
        _ => s
            .device()
            .and_then(|d| project.device(d))
            .map(|d| format!("{}[{}]", d.tag, s.symbol.0.rsplit('/').next().unwrap_or("")))?,
    };
    Some(format!("{who}:{}", s.pins[usize::from(pin)].name))
}

/// Which pins are joined, as a set of sets, independent of routes and ids.
fn nets_of(project: &Project) -> BTreeSet<BTreeSet<String>> {
    project
        .netlist()
        .nets()
        .iter()
        .map(|n| {
            n.endpoints
                .iter()
                .filter_map(|e| pin_label(project, *e))
                .collect::<BTreeSet<_>>()
        })
        .filter(|n| n.len() > 1)
        .collect()
}

fn routes_of(project: &Project) -> BTreeSet<String> {
    project
        .wires()
        .map(|w| {
            let name = |e: Endpoint| pin_label(project, e).unwrap_or_else(|| "junction".into());
            let (a, b) = (name(w.from), name(w.to));
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            format!(
                "{a} - {b} via {:?}",
                w.route.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>()
            )
        })
        .collect()
}

#[test]
fn placing_parts_by_name_wheel_and_digit_draws_the_finished_branch() {
    // The keys the lesson names reach these symbols.
    let lib = library();
    assert_eq!(lib.by_alias("mcb").unwrap().id, "iec/protection/mcb_1p");
    assert_eq!(
        novo_workflow::insert::DEFAULT_FAVOURITES[2],
        "iec/coils/relay",
        "digit 3 is the relay coil"
    );
    let mut r = Replay::open(&placing_parts(false));
    // Q, "mcb", Enter, click below the L1 feed.
    r.place("iec/protection/mcb_1p", None, 80.0, 40.0);
    // Hold Q, Switch, Push NO.
    r.place("iec/operators/pushbutton_no", None, 80.0, 72.0);
    // Q, 3.
    r.place("iec/coils/relay", None, 80.0, 104.0);
    let finished = placing_parts(true);
    assert_eq!(nets_of(r.project()), nets_of(finished.project()));
    assert_eq!(routes_of(r.project()), routes_of(finished.project()));
    let tags: BTreeSet<String> = r.project().devices().map(|d| d.tag.to_string()).collect();
    assert_eq!(tags, ["-F1", "-K1", "-S1"].map(String::from).into());
}

#[test]
fn the_wheel_offers_the_parts_the_lessons_use() {
    // wheel.rs: Switch has Push NO, Relay has Coil, Breaker has MPCB and
    // Overload, Points has Feed and PE.
    let src =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/app/wheel.rs"))
            .unwrap();
    for key in [
        "iec/operators/pushbutton_no",
        "iec/coils/relay",
        "iec/protection/motor_protection_breaker",
        "iec/protection/mcb_1p",
        "iec/connectivity/potential_feed",
        "iec/earth/protective_earth",
    ] {
        assert!(src.contains(key), "the wheel has {key}");
    }
}

#[test]
fn wiring_in_corners_and_in_lines_draws_the_finished_wires() {
    let mut r = Replay::open(&wiring_modes(false));
    let finished = wiring_modes(true);
    let key = |tag: &str| match tag {
        t if t.starts_with("-F") => "iec/protection/mcb_1p",
        t if t.starts_with("-S") => "iec/operators/pushbutton_no",
        _ => "iec/coils/relay",
    };
    // Drag (or click) from -F1:2 to -S1:13: the router picks the route.
    r.wire(&[
        {
            let p = r.pin("-F1", key("-F1"), "2");
            (p.x, p.y)
        },
        {
            let p = r.pin("-S1", key("-S1"), "13");
            (p.x, p.y)
        },
    ]);
    // Corners (the default): -S1:14, one click where it turns, -K1:A1.
    assert_eq!(r.editor.wire_mode(), novo_editor::tool::WiringMode::Corners);
    let p = |r: &Replay, t: &str, pin: &str| {
        let p = r.pin(t, key(t), pin);
        (p.x, p.y)
    };
    let (a, b) = (p(&r, "-S1", "14"), p(&r, "-K1", "A1"));
    r.wire(&[a, (100.0, 96.0), b]);
    // Shift+W, then the same thing segment by segment on the right.
    r.editor.toggle_wire_mode();
    assert_eq!(r.editor.wire_mode(), novo_editor::tool::WiringMode::Lines);
    let (a, b) = (p(&r, "-S2", "14"), p(&r, "-K2", "A1"));
    r.wire(&[a, (140.0, 96.0), (180.0, 96.0), b]);
    assert_eq!(nets_of(r.project()), nets_of(finished.project()));
    assert_eq!(routes_of(r.project()), routes_of(finished.project()));
}

#[test]
fn a_row_of_three_feeds_reads_l1_l2_l3_and_a_fourth_becomes_pe_by_its_chip() {
    let mut r = Replay::open(&potentials(false));
    let feed = "iec/connectivity/potential_feed";
    // Q, Points, Feed, then press at the first pole and drag across three.
    assert!(r.editor.begin_place(feed, None));
    r.editor.hover(&r.doc, r.page, Point::new(80.0, 20.0), TOL);
    assert!(r.editor.begin_row());
    r.editor.row_follow(Point::new(97.0, 20.0), 8.0);
    assert!(r.editor.row_active());
    assert_eq!(r.editor.end_row(&mut r.doc, r.page), Clicked::Edited);
    r.editor.leave_tool();
    let names: Vec<String> = {
        let p = r.project();
        let mut feeds: Vec<_> = p
            .symbols()
            .filter(|s| s.symbol.0 == feed)
            .map(|s| {
                (
                    s.placement.origin.x,
                    pin_label(p, Endpoint::pin(s.id, 0)).unwrap(),
                )
            })
            .collect();
        feeds.sort_by(|a, b| a.0.total_cmp(&b.0));
        feeds.into_iter().map(|(_, n)| n).collect()
    };
    assert_eq!(
        names,
        ["feed L1@80,20:1", "feed L2@88,20:1", "feed L3@96,20:1"]
    );
    // One feed above the motor's PE pin is called L4 until it is told what
    // it is: right-click, the PE chip.
    r.place(feed, None, 104.0, 92.0);
    let fourth = r
        .project()
        .symbols()
        .find(|s| s.placement.origin == Point::new(104.0, 92.0))
        .unwrap()
        .id;
    let menu =
        novo_editor::potential_menu::feed_menu(r.project(), &[AnyId::Symbol(fourth)]).unwrap();
    assert_eq!(menu.current.as_deref(), Some("L4"));
    let pe = menu.chips.iter().find(|c| c.name == "PE").unwrap().clone();
    assert!(r.editor.set_feed_potential(&mut r.doc, &[fourth], &pe));
    assert_eq!(
        nets_of(r.project()),
        nets_of(potentials(true).project()),
        "the wires drew themselves where the pins face each other"
    );
}

#[test]
fn building_the_starter_by_keys_draws_the_finished_starter() {
    let mut r = Replay::open(&motor_starter(false, Lamp::No, "x"));
    // Page 1: favourites 5 (motor protection breaker) and 4 (main contacts).
    assert_eq!(
        novo_workflow::insert::DEFAULT_FAVOURITES[4],
        "iec/protection/motor_protection_breaker"
    );
    assert_eq!(
        novo_workflow::insert::DEFAULT_FAVOURITES[3],
        "iec/contactors/main_3p"
    );
    r.place("iec/protection/motor_protection_breaker", None, 80.0, 40.0);
    r.place("iec/contactors/main_3p", None, 80.0, 72.0);
    // Page 2: `mcb`, `pbnc`, `pb`, then the coil and the contact of -K1.
    r.show("2");
    r.place("iec/protection/mcb_1p", None, 80.0, 40.0);
    r.place("iec/operators/pushbutton_nc", None, 80.0, 72.0);
    r.place("iec/operators/pushbutton_no", None, 80.0, 104.0);
    r.place("iec/coils/relay", Some("-K1"), 80.0, 136.0);
    r.place("iec/contacts/no", Some("-K1"), 96.0, 104.0);
    // Two drags from the contact onto the wires above and below the start button.
    let hold = "iec/contacts/no";
    let above = r.pin("-K1", hold, "13");
    r.wire(&[(above.x, above.y), (80.0, 96.0)]);
    let below = r.pin("-K1", hold, "14");
    r.wire(&[(below.x, below.y), (80.0, 128.0)]);
    let finished = motor_starter(true, Lamp::No, "y");
    assert_eq!(nets_of(r.project()), nets_of(finished.project()));
    assert_eq!(routes_of(r.project()), routes_of(finished.project()));
}

#[test]
fn the_lamp_contact_placed_between_its_feed_and_lamp_wires_itself() {
    let mut r = Replay::open(&motor_starter(true, Lamp::WithoutContact, "x"));
    r.show("2");
    // Q, `no K1`, Enter, click between the feed and the lamp: both pins face
    // a partner, so both wires are drawn with the drop.
    r.place("iec/contacts/no", Some("-K1"), 120.0, 104.0);
    let placed = r
        .project()
        .symbols()
        .find(|s| s.placement.origin == Point::new(120.0, 104.0))
        .unwrap();
    assert_eq!(
        placed
            .pins
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["23", "24"],
        "the next free contact position of -K1"
    );
    let finished = motor_starter(true, Lamp::Lit, "y");
    assert_eq!(nets_of(r.project()), nets_of(finished.project()));
    assert_eq!(routes_of(r.project()), routes_of(finished.project()));
}

/// `cargo test -p novo-app --test lesson_projects dump -- --ignored --nocapture`
/// prints every project as tags, pins and wires, for writing lesson text.
#[test]
#[ignore = "prints the projects for a person to read"]
fn dump() {
    for (folder, file, doc) in all() {
        println!("== {folder}/{file}");
        for line in describe(doc.project()) {
            println!("{line}");
        }
    }
}

#[test]
#[ignore = "prints the cross references for a person to read"]
fn dump_xref() {
    for (folder, file, doc) in all() {
        if folder != "cross-references" {
            continue;
        }
        println!("== {folder}/{file}");
        let index = novo_xref::XrefIndex::build(doc.project(), &novo_xref::FromKey);
        for page in doc.project().pages_in_order() {
            for label in index.labels_on_page(page.id) {
                let s = doc.project().symbol(label.symbol).unwrap();
                println!(
                    "  page {} @({},{}) {:?} {:?}",
                    page.name,
                    s.placement.origin.x,
                    s.placement.origin.y,
                    label.anchor,
                    label.lines
                );
            }
        }
        for s in doc.project().symbols() {
            if matches!(
                s.device()
                    .and_then(|d| doc.project().device(d))
                    .map(|d| d.tag.to_string())
                    .as_deref(),
                Some("-K1")
            ) {
                let h = index.highlight(s.id);
                println!("  hint for {}: {}", s.symbol.0, h.dock_hint());
            }
        }
    }
}

#[test]
fn the_names_the_lessons_type_find_the_symbols_they_name() {
    let editor = Editor::default();
    for (query, symbol, parent) in [
        ("mcb", "iec/protection/mcb_1p", None),
        ("pb", "iec/operators/pushbutton_no", None),
        ("pbnc", "iec/operators/pushbutton_nc", None),
        ("feed", "iec/connectivity/potential_feed", None),
        ("coil K1", "iec/coils/relay", Some("-K1")),
        ("no K1", "iec/contacts/no", Some("-K1")),
        ("coil", "iec/coils/relay", None),
        ("mpcb", "iec/protection/motor_protection_breaker", None),
        ("main", "iec/contactors/main_3p", None),
    ] {
        let hit = &editor.catalog.search(query)[0];
        assert_eq!(hit.symbol, symbol, "{query}");
        assert_eq!(hit.parent.as_deref(), parent, "{query}");
    }
    // Digits 4 and 5 on an empty place query.
    let favourites = novo_workflow::insert::DEFAULT_FAVOURITES;
    assert_eq!(favourites[3], "iec/contactors/main_3p");
    assert_eq!(favourites[4], "iec/protection/motor_protection_breaker");
}

// ----- the placing-parts steps that turn and copy -----------------------------
//
// "Press R, it turns a quarter" was refused on a part that already had wires
// ("it would run along another wire"). The lesson turns the part in hand
// instead, where no wire exists yet, and copies a part that has none.

#[test]
fn r_turns_the_part_in_hand_and_a_spare_goes_in_empty_space() {
    let mut r = Replay::open(&placing_parts(true));
    let old: BTreeSet<_> = r.project().symbols().map(|s| s.id).collect();
    let before = old.len();
    assert!(r.editor.begin_place("iec/operators/pushbutton_no", None));
    assert!(
        r.editor
            .key(&mut r.doc, novo_editor::EditorKey::Rotate, &[])
    );
    assert_eq!(
        r.click(130.0, 72.0),
        Clicked::Edited,
        "the turned part lands"
    );
    r.editor.leave_tool();
    assert_eq!(r.project().symbols().count(), before + 1);
    let spare = r
        .project()
        .symbols()
        .find(|s| !old.contains(&s.id))
        .expect("the spare");
    assert_eq!(spare.placement.rotation, novo_core::Rotation::R90);
    // Ctrl+Z takes it away again.
    r.doc.undo().unwrap();
    assert_eq!(r.project().symbols().count(), before);
}

#[test]
fn r_on_a_wired_part_is_refused_so_the_lesson_never_asks_for_it() {
    let mut r = Replay::open(&placing_parts(true));
    let s1 = symbol_of(r.project(), "-S1", "iec/operators/pushbutton_no");
    let ok = r.editor.rotate_selection(&mut r.doc, &[AnyId::from(s1)]);
    let turned = r.project().symbol(s1).unwrap().placement.rotation;
    assert!(
        !ok || turned == novo_core::Rotation::R0,
        "a part with wires on both sides must not be turned in the lesson"
    );
}

#[test]
fn duplicating_a_lone_push_button_makes_the_next_number_and_undo_removes_it() {
    let mut r = Replay::open(&placing_parts(true));
    let s1 = symbol_of(r.project(), "-S1", "iec/operators/pushbutton_no");
    let page = r.page;
    let (copy, _) = r
        .doc
        .transact("Duplicate", |tx| {
            novo_workflow::duplicate::duplicate(
                tx,
                &[AnyId::from(s1)],
                page,
                Point::new(20.0, 0.0),
                novo_workflow::duplicate::TagMode::NextFree,
            )
        })
        .unwrap();
    assert_eq!(copy.symbols.len(), 1);
    let tags: BTreeSet<String> = r.project().devices().map(|d| d.tag.to_string()).collect();
    assert!(tags.contains("-S2"), "{tags:?}");
    r.doc.undo().unwrap();
    let tags: BTreeSet<String> = r.project().devices().map(|d| d.tag.to_string()).collect();
    assert!(!tags.contains("-S2"));
}
