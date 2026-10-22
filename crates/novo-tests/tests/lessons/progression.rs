//! Lesson projects: the advanced end of the circuits track. Each one places
//! a starter macro of the program (`novo_parts::starter_macros`, the same
//! circuits a user places from the macro browser) on a page, feeds its open
//! points from potential feeds, and for a challenge takes one wire out.
//!
//! The circuits are the program's own macros, so a lesson cannot drift from
//! them and they pass the same checks as any placed macro.

use novo_core::{
    AnyId, BlockInstance, BlockInstanceId, DeviceId, Direction, Document, Endpoint, PageId, Pin,
    Placement, Point, PotentialId, PotentialKind, Project, Record, Result, SymbolId, SymbolRole,
    Tx, WireId,
};
use novo_editor::edit::draw_wire;
use novo_editor::geometry::Target;
use novo_parts::{MacroFile, starter_macros};

use super::common::{assert_lint, check_file, check_file_up_to};

fn macro_named(name: &str) -> MacroFile {
    starter_macros()
        .into_iter()
        .map(|(_, f)| f)
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no starter macro `{name}`"))
}

/// One supply connection: a device (`-F1`) or a library key tail
/// (`terminal_pe`), its pin, and the potential that feeds it.
pub struct Feed {
    pub owner: &'static str,
    pub pin: &'static str,
    pub potential: &'static str,
}

pub const fn feed(owner: &'static str, pin: &'static str, potential: &'static str) -> Feed {
    Feed {
        owner,
        pin,
        potential,
    }
}

fn kind_of(name: &str) -> (PotentialKind, f64) {
    match name {
        "L1" | "L2" | "L3" => (PotentialKind::Line, 400.0),
        "L" => (PotentialKind::Line, 230.0),
        "N" => (PotentialKind::Neutral, 0.0),
        "PE" => (PotentialKind::ProtectiveEarth, 0.0),
        "+24V" => (PotentialKind::DcPositive, 24.0),
        "0V" => (PotentialKind::DcNegative, 0.0),
        other => panic!("no potential `{other}`"),
    }
}

fn potential(tx: &mut Tx<'_>, name: &str) -> Result<PotentialId> {
    if let Some(p) = tx.project().potentials().find(|p| p.name == name) {
        return Ok(p.id);
    }
    let (kind, volts) = kind_of(name);
    let id = tx.add_potential(name, kind)?;
    if volts > 0.0 {
        tx.update::<novo_core::Potential>(id, |p| p.voltage = Some(volts))?;
    }
    Ok(id)
}

/// Which placed macro a tag or key belongs to. `None` searches the project.
pub type Scope = Option<BlockInstanceId>;

/// The symbols that belong to `owner`: a device tag as the macro names it
/// (`-F1`), or the tail of a library key (`terminal_pe`).
fn owner_symbols(project: &Project, scope: Scope, owner: &str) -> Vec<SymbolId> {
    let members: Option<&BlockInstance> = scope.and_then(|i| project.block_instance(i));
    let in_scope = |id: AnyId| members.is_none_or(|m| m.members.iter().any(|x| x.actual == id));
    let devices: Vec<DeviceId> = if owner.starts_with('-') {
        match members {
            Some(m) => {
                let def = project
                    .block_definition(m.definition)
                    .expect("the definition is in the project");
                def.members
                    .iter()
                    .filter_map(|r| match r {
                        Record::Device(d) if d.tag.to_string() == owner => Some(d.id),
                        _ => None,
                    })
                    .filter_map(|template| {
                        m.members.iter().find_map(|x| match (x.template, x.actual) {
                            (AnyId::Device(t), AnyId::Device(a)) if t == template => Some(a),
                            _ => None,
                        })
                    })
                    .collect()
            }
            None => project
                .devices()
                .filter(|d| d.tag.to_string().ends_with(owner))
                .map(|d| d.id)
                .collect(),
        }
    } else {
        Vec::new()
    };
    project
        .symbols()
        .filter(|s| in_scope(AnyId::Symbol(s.id)))
        .filter(|s| {
            if owner.starts_with('-') {
                matches!(s.role, SymbolRole::Device { device, .. } if devices.contains(&device))
            } else {
                s.symbol.0.ends_with(owner)
            }
        })
        .map(|s| s.id)
        .collect()
}

/// The symbol of `owner` that has a pin `pin` nobody else is using yet.
fn find_pin(
    project: &Project,
    scope: Scope,
    owner: &str,
    pin: &str,
    used: &[Endpoint],
) -> (SymbolId, u16) {
    for id in owner_symbols(project, scope, owner) {
        let s = project.symbol(id).expect("symbol");
        for (i, p) in s.pins.iter().enumerate() {
            let end = Endpoint::pin(s.id, i as u16);
            if p.name == pin && !used.contains(&end) {
                return (s.id, i as u16);
            }
        }
    }
    panic!("no free pin {owner}:{pin}");
}

/// The macro placed at `at`, with its open points fed. Returns the placed
/// instance, which scopes later lookups of its tags.
pub fn place_macro(
    tx: &mut Tx<'_>,
    page: PageId,
    name: &str,
    at: (f64, f64),
    feeds: &[Feed],
) -> Result<BlockInstanceId> {
    let file = macro_named(name);
    let placed = novo_workflow::macro_place::place_macro(
        tx,
        file.definition.clone(),
        page,
        Point::new(at.0, at.1),
        &novo_workflow::macro_place::PlaceOptions::default(),
    )?;
    let scope = Some(placed.instance);
    let mut used = Vec::new();
    for f in feeds {
        let (symbol, pin) = find_pin(tx.project(), scope, f.owner, f.pin, &used);
        let end = Endpoint::pin(symbol, pin);
        used.push(end);
        let pot = potential(tx, f.potential)?;
        let (p, dir) = {
            let s = tx.project().symbol(symbol).expect("symbol");
            (
                tx.project().endpoint_position(end).expect("position"),
                s.pins[usize::from(pin)].direction,
            )
        };
        // The feed sits beyond the pin, on the side the wire leaves.
        let down = matches!(dir, Some(Direction::Down))
            || (dir.is_none() && matches!(f.potential, "N" | "0V"));
        let origin = if down {
            Point::new(p.x, p.y + 6.0 - 8.0)
        } else {
            Point::new(p.x, p.y - 6.0 - 8.0)
        };
        let feed_symbol = tx.place_symbol(
            page,
            "iec/connectivity/potential_feed",
            Placement::at(origin),
            vec![Pin::new("1", 0.0, 8.0)],
            SymbolRole::PotentialFeed { potential: pot },
        )?;
        let from = Endpoint::pin(feed_symbol, 0);
        let a = tx.project().endpoint_position(from).expect("feed pin");
        draw_wire(
            tx,
            page,
            Target::Endpoint(from, a),
            &[],
            Target::Endpoint(end, p),
        )?;
    }
    Ok(placed.instance)
}

/// The macro on its own page, fed.
fn single(title: &str, name: &str, feeds: &[Feed]) -> (Document, BlockInstanceId) {
    let mut doc = Document::new(Project::new(""));
    let mut instance = None;
    doc.transact("place", |tx| {
        let page = tx.add_page("1")?;
        tx.update::<novo_core::Page>(page, |p| p.title = title.into())?;
        instance = Some(place_macro(tx, page, name, (20.0, 20.0), feeds)?);
        Ok(())
    })
    .expect("place");
    (doc, instance.expect("placed"))
}

/// A pin written `(owner, pin)` in a placed macro.
type PinRef<'a> = (&'a str, &'a str);

fn endpoint(project: &Project, scope: Scope, (owner, pin): PinRef<'_>) -> Endpoint {
    let (symbol, index) = find_pin(project, scope, owner, pin, &[]);
    Endpoint::pin(symbol, index)
}

/// The wire joining two pins, if any.
pub fn wire_between(
    project: &Project,
    scope: Scope,
    a: PinRef<'_>,
    b: PinRef<'_>,
) -> Option<WireId> {
    let (ea, eb) = (endpoint(project, scope, a), endpoint(project, scope, b));
    project
        .wires()
        .find(|w| (w.from == ea && w.to == eb) || (w.from == eb && w.to == ea))
        .map(|w| w.id)
}

/// The wires that end on this pin.
fn wires_at(project: &Project, scope: Scope, pin: PinRef<'_>) -> Vec<WireId> {
    let end = endpoint(project, scope, pin);
    project
        .wires()
        .filter(|w| w.from == end || w.to == end)
        .map(|w| w.id)
        .collect()
}

pub fn remove_wire(doc: &mut Document, id: WireId) {
    doc.transact("fault", |tx| tx.remove(id)).expect("remove");
}

/// Draws a wire between two pins, as the wire tool does.
fn connect(doc: &mut Document, scope: Scope, a: PinRef<'_>, b: PinRef<'_>) {
    let (ea, eb) = (
        endpoint(doc.project(), scope, a),
        endpoint(doc.project(), scope, b),
    );
    let Endpoint::Pin { symbol, .. } = ea else {
        panic!("a pin");
    };
    let page = doc.project().symbol(symbol).expect("symbol").page;
    doc.transact("fix", |tx| {
        let (pa, pb) = (
            tx.project().endpoint_position(ea).expect("a"),
            tx.project().endpoint_position(eb).expect("b"),
        );
        draw_wire(
            tx,
            page,
            Target::Endpoint(ea, pa),
            &[],
            Target::Endpoint(eb, pb),
        )
        .map(|_| ())
    })
    .expect("connect");
}

fn three_phase_feeds() -> Vec<Feed> {
    vec![
        feed("-F1", "1", "L1"),
        feed("-F1", "3", "L2"),
        feed("-F1", "5", "L3"),
        feed("terminal_pe", "1", "PE"),
        feed("-F3", "1", "L"),
        feed("-K1", "A2", "N"),
    ]
}

// ---- Reversing starter -------------------------------------------------

const REVERSING: &str = "reversing-starter";
const REVERSING_WIRE: (PinRef<'static>, PinRef<'static>) = (("-K1", "22"), ("-K2", "A1"));

/// The reversing starter, fed, with the wire from the forward interlock
/// contact to the reverse coil taken out: reverse cannot pull in.
fn reversing_fault() -> Document {
    let (mut doc, scope) = single(
        "Reversing starter",
        "Motor starter reversing",
        &three_phase_feeds(),
    );
    let wire = wire_between(
        doc.project(),
        Some(scope),
        REVERSING_WIRE.0,
        REVERSING_WIRE.1,
    )
    .expect("the interlock wire exists");
    remove_wire(&mut doc, wire);
    doc
}

fn reversing_fixed() -> Document {
    let mut doc = reversing_fault();
    let scope = only_instance(&doc);
    connect(&mut doc, scope, REVERSING_WIRE.0, REVERSING_WIRE.1);
    doc
}

fn only_instance(doc: &Document) -> Scope {
    doc.project().block_instances().next().map(|i| i.id)
}

#[test]
fn reversing_files_are_current() {
    let start = reversing_fault();
    check_file(REVERSING, "project.json", start.project());
    check_file(REVERSING, "finished.json", reversing_fixed().project());
    assert_lint(&[(REVERSING, start.project())]);
}

#[test]
fn the_reversing_fault_is_one_missing_interlock_wire() {
    let (start, fixed) = (reversing_fault(), reversing_fixed());
    assert_eq!(
        start.project().wires().count() + 1,
        fixed.project().wires().count()
    );
    let (a, b) = REVERSING_WIRE;
    assert!(wire_between(start.project(), only_instance(&start), a, b).is_none());
    assert!(wire_between(fixed.project(), only_instance(&fixed), a, b).is_some());
}

// ---- Star-delta starter ------------------------------------------------

const STAR_DELTA: &str = "star-delta-starter";
const STAR_DELTA_WIRE: (PinRef<'static>, PinRef<'static>) = (("-K4", "18"), ("-K3", "21"));

/// The star-delta starter, fed, with the wire from the timer's closing
/// contact to the star contactor's interlock taken out: the delta contactor
/// can never pull in, so the motor stays in star.
fn star_delta_fault() -> Document {
    let (mut doc, scope) = single(
        "Star-delta starter",
        "Star-delta starter",
        &three_phase_feeds(),
    );
    let wire = wire_between(
        doc.project(),
        Some(scope),
        STAR_DELTA_WIRE.0,
        STAR_DELTA_WIRE.1,
    )
    .expect("the delta path wire exists");
    remove_wire(&mut doc, wire);
    doc
}

fn star_delta_fixed() -> Document {
    let mut doc = star_delta_fault();
    let scope = only_instance(&doc);
    connect(&mut doc, scope, STAR_DELTA_WIRE.0, STAR_DELTA_WIRE.1);
    doc
}

#[test]
fn star_delta_files_are_current() {
    let start = star_delta_fault();
    check_file(STAR_DELTA, "project.json", start.project());
    check_file(STAR_DELTA, "finished.json", star_delta_fixed().project());
    assert_lint(&[(STAR_DELTA, start.project())]);
}

#[test]
fn the_star_delta_fault_is_the_delta_path() {
    let (start, fixed) = (star_delta_fault(), star_delta_fixed());
    let (a, b) = STAR_DELTA_WIRE;
    assert!(wire_between(start.project(), only_instance(&start), a, b).is_none());
    assert!(wire_between(fixed.project(), only_instance(&fixed), a, b).is_some());
}

// ---- Emergency stop ----------------------------------------------------

const SAFETY: &str = "safety-circuit";
const SAFETY_WIRE: (PinRef<'static>, PinRef<'static>) = (("-K1", "S21"), ("-S1", "21"));

fn safety_feeds() -> Vec<Feed> {
    vec![
        feed("-K1", "A1", "+24V"),
        feed("-K1", "13", "+24V"),
        feed("-K1", "23", "+24V"),
        feed("-S2", "13", "+24V"),
        feed("-K1", "A2", "0V"),
        feed("-K2", "A2", "0V"),
        feed("-K3", "A2", "0V"),
    ]
}

/// The two-channel emergency stop on a safety relay, fed with 24 V DC, with
/// channel 2 cut between the relay's S21 and the button's contact 21.
fn safety_fault() -> Document {
    let (mut doc, scope) = single(
        "Emergency stop",
        "Emergency stop with safety relay",
        &safety_feeds(),
    );
    let wire = wire_between(doc.project(), Some(scope), SAFETY_WIRE.0, SAFETY_WIRE.1)
        .expect("the channel 2 wire exists");
    remove_wire(&mut doc, wire);
    doc
}

fn safety_fixed() -> Document {
    let mut doc = safety_fault();
    let scope = only_instance(&doc);
    connect(&mut doc, scope, SAFETY_WIRE.0, SAFETY_WIRE.1);
    doc
}

#[test]
fn safety_files_are_current() {
    let start = safety_fault();
    check_file(SAFETY, "project.json", start.project());
    check_file(SAFETY, "finished.json", safety_fixed().project());
    assert_lint(&[(SAFETY, start.project())]);
}

#[test]
fn the_safety_fault_cuts_channel_two() {
    let (start, fixed) = (safety_fault(), safety_fixed());
    let (a, b) = SAFETY_WIRE;
    assert!(wire_between(start.project(), only_instance(&start), a, b).is_none());
    assert!(wire_between(fixed.project(), only_instance(&fixed), a, b).is_some());
}

// ---- Crane hoist -------------------------------------------------------

const CRANE: &str = "crane-hoist";

/// The crane hoist, fed, with the wire from the up contactor's auxiliary
/// contact to the brake taken out: going up, the brake stays locked.
fn crane_fault() -> Document {
    let (mut doc, scope) = single("Crane hoist", "Crane hoist up/down", &three_phase_feeds());
    let wires = wires_at(doc.project(), Some(scope), ("-K1", "14"));
    assert_eq!(wires.len(), 1, "one wire leaves the up auxiliary contact");
    remove_wire(&mut doc, wires[0]);
    doc
}

fn crane_fixed() -> Document {
    let mut doc = crane_fault();
    let scope = only_instance(&doc);
    connect(&mut doc, scope, ("-K1", "14"), ("-K2", "14"));
    doc
}

#[test]
fn crane_files_are_current() {
    let start = crane_fault();
    check_file(CRANE, "project.json", start.project());
    check_file(CRANE, "finished.json", crane_fixed().project());
    assert_lint(&[(CRANE, start.project())]);
}

#[test]
fn the_crane_fault_leaves_the_up_contact_open() {
    let (start, fixed) = (crane_fault(), crane_fixed());
    assert!(wires_at(start.project(), only_instance(&start), ("-K1", "14")).is_empty());
    assert_eq!(
        wires_at(fixed.project(), only_instance(&fixed), ("-K1", "14")).len(),
        1
    );
}

// ---- The whole machine -------------------------------------------------

const MACHINE: &str = "conveyor-line";

/// A conveyor with a reversing drive, a mixer on a star-delta starter, a
/// two-channel emergency stop and a PLC input card: four macros on four
/// pages, each fed from potentials. Returns the document and the four
/// instances in page order.
fn machine() -> (Document, [Scope; 4]) {
    let mut doc = Document::new(Project::new(""));
    let mut scopes = [None; 4];
    doc.transact("machine", |tx| {
        let plan: [(&str, &str, Vec<Feed>); 4] = [
            ("Conveyor", "Motor starter reversing", three_phase_feeds()),
            ("Mixer", "Star-delta starter", three_phase_feeds()),
            ("Safety", "Emergency stop with safety relay", safety_feeds()),
            (
                "PLC inputs",
                "PLC input card",
                vec![feed("-A1", "L+", "+24V"), feed("-A1", "M", "0V")],
            ),
        ];
        for (n, (title, name, feeds)) in plan.into_iter().enumerate() {
            let page = tx.add_page(&format!("{}", n + 1))?;
            tx.update::<novo_core::Page>(page, |p| p.title = title.into())?;
            scopes[n] = Some(place_macro(tx, page, name, (20.0, 20.0), &feeds)?);
        }
        Ok(())
    })
    .expect("machine");
    (doc, scopes)
}

/// The three faults, as macro-tagged pins and the page they are on.
const MACHINE_FAULTS: [(usize, PinRef<'static>, PinRef<'static>); 3] = [
    // Conveyor: stop button to the overload contact, so no control supply.
    (0, ("-S0", "12"), ("-F2", "95")),
    // Mixer: the delta contactor's interlock to the star coil.
    (1, ("-K2", "22"), ("-K3", "A1")),
    // Safety: reset button to the relay's reset input.
    (2, ("-S2", "14"), ("-K1", "S34")),
];

fn machine_fault() -> Document {
    let (mut doc, scopes) = machine();
    for (page, a, b) in MACHINE_FAULTS {
        let wire = wire_between(doc.project(), scopes[page], a, b)
            .unwrap_or_else(|| panic!("wire {a:?} {b:?} exists"));
        remove_wire(&mut doc, wire);
    }
    doc
}

fn machine_fixed() -> Document {
    let (mut doc, scopes) = machine();
    // Same project, nothing removed: scopes are the same instances.
    let _ = (&mut doc, scopes);
    doc
}

#[test]
fn conveyor_line_files_are_current() {
    let start = machine_fault();
    check_file_up_to(MACHINE, "project.json", start.project(), 400);
    check_file_up_to(MACHINE, "finished.json", machine_fixed().project(), 400);
    assert_lint(&[(MACHINE, start.project())]);
}

#[test]
fn the_machine_has_three_faults_across_three_pages() {
    let (start, fixed) = (machine_fault(), machine_fixed());
    assert_eq!(
        start.project().wires().count() + 3,
        fixed.project().wires().count()
    );
    let open = |d: &Document| {
        novo_checks::check(d.project())
            .into_iter()
            .filter(|p| p.kind == novo_checks::ProblemKind::UnconnectedPin)
            .count()
    };
    assert!(open(&start) > open(&fixed));
}
