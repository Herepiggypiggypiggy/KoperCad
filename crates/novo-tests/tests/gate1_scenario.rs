//! Gate 1, "first real flow": create a project, place a motor starter (MCB,
//! contactor, overload relay, motor), wire it, see its terminals and
//! potentials, then save and reload.
//!
//! `scenario_on_main` runs the whole flow through `Command`s only (the path
//! the editor, scripts and the AI assistant share) and through the real
//! project file. The `gap_*` tests run single steps the way the user will do
//! them, once the crate that offers that step is on main. A step whose pull
//! request is still open is an ignored `gap_*` test that panics with the
//! number of that pull request, so `cargo test -p novo-tests --test
//! gate1_scenario -- --ignored` lists every open gap as a failure. When the
//! pull request merges, its gap test is filled in and un-ignored.

use novo_app::{ProjectBackend, StorageBackend, page_labels};
use novo_canvas::model::{PageScene, PlaceholderArt, item_id};
use novo_canvas::{Vec2, vec2};
use novo_core::*;
use novo_storage::Session;
use novo_symbols::Library;
use novo_workflow::insert::{InsertSearch, mnemonic, next_tag};

/// The ids `execute` created, of one kind.
fn created<T>(outcome: &Outcome, pick: impl Fn(AnyId) -> Option<T>) -> T {
    outcome
        .created
        .iter()
        .copied()
        .find_map(pick)
        .expect("the command created an object of this kind")
}

/// Where the open document lives, and where each edit's change set goes so
/// the next save writes it.
trait Store {
    fn doc(&self) -> &Document;
    fn doc_mut(&mut self) -> &mut Document;
    fn record(&mut self, changes: &ChangeSet);
}

/// The project file opened directly through `novo-storage`.
struct FileStore {
    doc: Document,
    session: Session,
}

impl Store for FileStore {
    fn doc(&self) -> &Document {
        &self.doc
    }
    fn doc_mut(&mut self) -> &mut Document {
        &mut self.doc
    }
    fn record(&mut self, changes: &ChangeSet) {
        self.session.record(changes);
    }
}

/// The app's project backend (#34), as the window drives it.
impl Store for StorageBackend {
    fn doc(&self) -> &Document {
        self.document().expect("a project is open")
    }
    fn doc_mut(&mut self) -> &mut Document {
        self.document_mut().expect("a project is open")
    }
    fn record(&mut self, changes: &ChangeSet) {
        ProjectBackend::record(self, changes);
    }
}

/// What the user does, step by step, through commands only.
struct Flow<'a, S: Store> {
    store: S,
    library: &'a Library,
    /// Every placed symbol with the library alias and page position it was
    /// placed at, to check the canvas against.
    placed: Vec<(SymbolId, &'static str, f64, f64)>,
}

impl<'a, S: Store> Flow<'a, S> {
    fn new(store: S, library: &'a Library) -> Self {
        Flow {
            store,
            library,
            placed: Vec::new(),
        }
    }

    fn project(&self) -> &Project {
        self.store.doc().project()
    }

    /// Runs a command as one undo step and records its own change set for
    /// the next save.
    fn run(&mut self, command: Command) -> Outcome {
        let outcome = self
            .store
            .doc_mut()
            .execute(command)
            .unwrap_or_else(|e| panic!("command refused: {e}"));
        self.store.record(&outcome.changes);
        outcome
    }

    /// Places a library symbol, found by its short name (alias) as the
    /// user types it, with the library's pins, bridges and pass-through.
    fn place(
        &mut self,
        page: PageId,
        alias: &'static str,
        x: f64,
        y: f64,
        role: SymbolRole,
    ) -> SymbolId {
        let symbol = self
            .library
            .by_alias(alias)
            .unwrap_or_else(|| panic!("no symbol for `{alias}`"));
        let pins = symbol
            .connections
            .iter()
            .map(|c| Pin::new(c.id.clone(), c.at[0], c.at[1]))
            .collect();
        let to_u16 = |g: &Vec<usize>| g.iter().map(|&i| u16::try_from(i).unwrap()).collect();
        let bridges: Vec<Vec<u16>> = symbol.bridges().iter().map(to_u16).collect();
        let pass_through = symbol
            .pass_through()
            .paths()
            .iter()
            .map(to_u16)
            .filter(|g| !bridges.contains(g))
            .collect();
        let outcome = self.run(Command::PlaceSymbol {
            page,
            symbol: SymbolKey(symbol.id.clone()),
            placement: Placement::at(Point::new(x, y)),
            role,
            pins,
            bridges,
            pass_through,
        });
        let id = created(&outcome, |id| match id {
            AnyId::Symbol(s) => Some(s),
            _ => None,
        });
        self.placed.push((id, alias, x, y));
        id
    }

    fn device(&mut self, tag: &str, description: &str, kind: DeviceKind) -> DeviceId {
        let outcome = self.run(Command::AddDevice {
            tag: DeviceTag::parse(tag).unwrap(),
            description: description.into(),
            kind,
        });
        created(&outcome, |id| match id {
            AnyId::Device(d) => Some(d),
            _ => None,
        })
    }

    /// A supply potential: lines at 400 V between lines, PE without a
    /// voltage.
    fn potential(&mut self, name: &str, kind: PotentialKind) -> PotentialId {
        let voltage = (kind == PotentialKind::Line).then_some(400.0);
        let outcome = self.run(Command::AddPotential {
            name: name.into(),
            kind,
            voltage,
        });
        created(&outcome, |id| match id {
            AnyId::Potential(p) => Some(p),
            _ => None,
        })
    }

    /// The index of the pin named `name` on a placed symbol.
    fn pin(&self, symbol: SymbolId, name: &str) -> Endpoint {
        let s = self.project().symbol(symbol).unwrap();
        let i = s.pins.iter().position(|p| p.name == name).unwrap();
        Endpoint::pin(symbol, u16::try_from(i).unwrap())
    }

    fn wire(&mut self, from: Endpoint, to: Endpoint) -> WireId {
        let outcome = self.run(Command::Connect {
            from,
            to,
            route: vec![],
        });
        created(&outcome, |id| match id {
            AnyId::Wire(w) => Some(w),
            _ => None,
        })
    }

    /// Where a pin should be on the page: the placement origin plus the
    /// library's connection point (the symbols are placed unrotated).
    fn expected_pin_position(&self, endpoint: Endpoint) -> Vec2 {
        let Endpoint::Pin { symbol, pin } = endpoint else {
            panic!("{endpoint:?} is not a pin");
        };
        let &(_, alias, x, y) = self.placed.iter().find(|p| p.0 == symbol).unwrap();
        let at = self.library.by_alias(alias).unwrap().connections[usize::from(pin)].at;
        vec2((x + at[0]) as f32, (y + at[1]) as f32)
    }
}

fn role(device: DeviceId) -> SymbolRole {
    SymbolRole::Device { device, main: true }
}

/// The Gate 1 starter as placed and wired by [`build`].
struct Starter {
    /// L1, L2, L3, PE, when it is fed from potential feed symbols.
    supply: Option<([PotentialId; 3], PotentialId)>,
    x1: DeviceId,
    m1_sym: SymbolId,
    /// `-X1:1`, `:2`, `:3`, `:PE`: the terminal and its symbol.
    terminals: Vec<(TerminalId, SymbolId)>,
    /// The three wires from `-Q1` to `-K1`, L1 to L3.
    q1_to_k1: Vec<WireId>,
}

/// Steps 2 and 3: places the starter on `page` and wires it. With
/// `feeds`, the supply comes from L1, L2, L3 and PE potential feed symbols;
/// without, the supply side of `-Q1` and `-X1:PE` is left open.
fn build<S: Store>(flow: &mut Flow<'_, S>, page: PageId, feeds: bool) -> Starter {
    let supply = feeds.then(|| {
        let phases = ["L1", "L2", "L3"].map(|name| flow.potential(name, PotentialKind::Line));
        let pe = flow.potential("PE", PotentialKind::ProtectiveEarth);
        (phases, pe)
    });
    let feed_symbols: Vec<SymbolId> = supply
        .iter()
        .flat_map(|(phases, pe)| phases.iter().chain([pe]).copied().collect::<Vec<_>>())
        .zip([40.0, 48.0, 56.0, 72.0])
        .map(|(potential, x)| {
            flow.place(
                page,
                "feed",
                x,
                12.0,
                SymbolRole::PotentialFeed { potential },
            )
        })
        .collect();
    let q1 = flow.device("=A1+CAB-Q1", "Motor circuit breaker", DeviceKind::General);
    let k1 = flow.device("=A1+CAB-K1", "Motor contactor", DeviceKind::General);
    let f1 = flow.device("=A1+CAB-F1", "Overload relay", DeviceKind::General);
    let x1 = flow.device("=A1+CAB-X1", "Motor terminals", DeviceKind::TerminalStrip);
    let m1 = flow.device("=A1+FLD-M1", "Pump motor", DeviceKind::General);
    let q1_sym = flow.place(page, "mcb3", 40.0, 40.0, role(q1));
    let k1_sym = flow.place(page, "main3", 40.0, 72.0, role(k1));
    let f1_sym = flow.place(page, "ol", 40.0, 104.0, role(f1));
    let mut terminals = Vec::new();
    for (name, x) in [("1", 40.0), ("2", 48.0), ("3", 56.0), ("PE", 72.0)] {
        let outcome = flow.run(Command::AddTerminal {
            strip: x1,
            name: name.into(),
        });
        let terminal = created(&outcome, |id| match id {
            AnyId::Terminal(t) => Some(t),
            _ => None,
        });
        let alias = if name == "PE" { "xpe" } else { "term" };
        let symbol = flow.place(
            page,
            alias,
            x,
            136.0,
            SymbolRole::Terminal {
                terminals: vec![terminal],
            },
        );
        terminals.push((terminal, symbol));
    }
    // The PE clamp is a protective earth terminal, not a feed-through.
    let (pe_terminal, _) = terminals[3];
    let mut record = flow.project().terminal(pe_terminal).unwrap().clone();
    record.kind = TerminalKind::ProtectiveEarth;
    flow.run(Command::Replace {
        record: Record::Terminal(record),
    });
    let m1_sym = flow.place(page, "m3", 40.0, 180.0, role(m1));

    // Supply, MCB, contactor, overload, terminals, motor.
    let mut q1_to_k1 = Vec::new();
    for (i, (top, bottom)) in [("1", "2"), ("3", "4"), ("5", "6")].into_iter().enumerate() {
        if let Some(&feed) = feed_symbols.get(i) {
            let q1_in = flow.pin(q1_sym, top);
            flow.wire(Endpoint::pin(feed, 0), q1_in);
        }
        for (from, to) in [(q1_sym, k1_sym), (k1_sym, f1_sym)] {
            let (a, b) = (flow.pin(from, bottom), flow.pin(to, top));
            let wire = flow.wire(a, b);
            if from == q1_sym {
                q1_to_k1.push(wire);
            }
        }
        let (a, b) = (flow.pin(f1_sym, bottom), Endpoint::pin(terminals[i].1, 0));
        flow.wire(a, b);
    }
    if let Some(&feed) = feed_symbols.get(3) {
        flow.wire(Endpoint::pin(feed, 0), Endpoint::pin(terminals[3].1, 0));
    }
    for (i, motor_pin) in ["U1", "V1", "W1", "PE"].into_iter().enumerate() {
        let to = flow.pin(m1_sym, motor_pin);
        flow.wire(Endpoint::pin(terminals[i].1, 1), to);
    }
    Starter {
        supply,
        x1,
        m1_sym,
        terminals,
        q1_to_k1,
    }
}

const MOTOR_PINS: [&str; 4] = ["U1", "V1", "W1", "PE"];

/// The potential at each of the motor's U1, V1, W1 and PE.
fn motor_potentials<S: Store>(
    flow: &Flow<'_, S>,
    project: &Project,
    s: &Starter,
) -> Vec<Option<PotentialId>> {
    let net = project.netlist();
    MOTOR_PINS
        .map(|pin| net.potential_of(flow.pin(s.m1_sym, pin)))
        .to_vec()
}

fn supply_order(s: &Starter) -> Vec<Option<PotentialId>> {
    let (phases, pe) = s.supply.unwrap();
    phases.iter().chain([&pe]).map(|&p| Some(p)).collect()
}

#[test]
fn scenario_on_main() {
    let library = Library::builtin().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gate1.novo");

    // 1. Create a project and its file.
    let doc = Document::new(Project::new("Gate 1"));
    let session = Session::create(&path, doc.project()).unwrap();
    let mut flow = Flow::new(FileStore { doc, session }, &library);
    let page = created(
        &flow.run(Command::AddPage {
            name: "1".into(),
            title: "Motor starter".into(),
            kind: PageKind::default(),
            format: PageFormat::default(),
        }),
        |id| match id {
            AnyId::Page(p) => Some(p),
            _ => None,
        },
    );

    // 2, 3. Place the supply and the starter (MCB, contactor, overload
    //    relay, terminals, motor), each found by the short name the user
    //    types, and wire it.
    let s = build(&mut flow, page, true);
    let x1 = s.x1;

    // 4. See the potentials: each phase reaches the motor through the
    //    breaker, contactor, overload and terminal; PE reaches its pin.
    //    Lines are 400 V between lines; PE has no voltage.
    let p = flow.project();
    assert_eq!(motor_potentials(&flow, p, &s), supply_order(&s));
    assert_eq!(p.netlist().potential_conflicts().count(), 0);
    let voltages: Vec<(String, Option<f64>)> = supply_order(&s)
        .into_iter()
        .map(|id| p.potential(id.unwrap()).unwrap())
        .map(|p| (p.name.clone(), p.voltage))
        .collect();
    assert_eq!(
        voltages,
        [
            ("L1", Some(400.0)),
            ("L2", Some(400.0)),
            ("L3", Some(400.0)),
            ("PE", None)
        ]
        .map(|(n, v)| (n.to_string(), v))
    );

    // 5. See the terminals: the strip in order, with full designations and
    //    kinds, and each terminal's motor side on the motor's net.
    let strip: Vec<(String, TerminalKind, String)> = p
        .terminals_of_strip(x1)
        .into_iter()
        .map(|t| {
            (
                t.name.clone(),
                t.kind,
                p.terminal_designation(t.id).unwrap(),
            )
        })
        .collect();
    assert_eq!(
        strip,
        [
            ("1", TerminalKind::Feedthrough, "=A1+CAB-X1:1"),
            ("2", TerminalKind::Feedthrough, "=A1+CAB-X1:2"),
            ("3", TerminalKind::Feedthrough, "=A1+CAB-X1:3"),
            ("PE", TerminalKind::ProtectiveEarth, "=A1+CAB-X1:PE"),
        ]
        .map(|(n, k, d)| (n.to_string(), k, d.to_string()))
    );
    let net = p.netlist();
    for (i, motor_pin) in MOTOR_PINS.into_iter().enumerate() {
        assert!(net.connected(
            Endpoint::pin(s.terminals[i].1, 1),
            flow.pin(s.m1_sym, motor_pin)
        ));
    }
    // Searching for the strip's tag finds it.
    assert!(p.search("-X1").iter().any(|h| h.id == AnyId::Device(x1)));

    // 6. The page draws every placed object where it was placed: each
    //    symbol's connection points at its placement plus the library's
    //    connection points, and each wire from pin to pin.
    let scene = PageScene::new(p, page, &PlaceholderArt);
    let contents = p.page_contents(page);
    assert_eq!(contents.symbols.len(), 4 + 3 + 4 + 1);
    assert_eq!(contents.wires.len(), 3 * 4 + 1 + 4);
    assert_eq!(flow.placed.len(), contents.symbols.len());
    for &(symbol, alias, _, _) in &flow.placed {
        let item = scene.scene().get(item_id(symbol)).expect("symbol drawn");
        let drawn: Vec<Vec2> = item.snap_points.iter().map(|s| s.pos).collect();
        let pins = library.by_alias(alias).unwrap().connections.len();
        let expected: Vec<Vec2> = (0..pins)
            .map(|i| flow.expected_pin_position(Endpoint::pin(symbol, u16::try_from(i).unwrap())))
            .collect();
        assert_eq!(drawn, expected, "{alias} {symbol:?}");
    }
    for &wire in &contents.wires {
        let item = scene.scene().get(item_id(wire)).expect("wire drawn");
        let w = p.wire(wire).unwrap();
        let ends: Vec<Vec2> = item.snap_points.iter().map(|s| s.pos).collect();
        assert_eq!(
            ends,
            [
                flow.expected_pin_position(w.from),
                flow.expected_pin_position(w.to)
            ],
            "{wire:?}"
        );
    }

    // 7. Save and reload: every edit recorded its own change set, so the
    //    save writes the project as built; it reopens the same.
    let before = p.to_json();
    let FileStore { doc, mut session } = flow.store;
    session.save(doc.project()).unwrap();
    session.close().unwrap();
    let opened = Session::open(&path).unwrap();
    assert!(opened.recovered.is_none());
    assert_eq!(opened.project.to_json(), before);
    let reflow = Flow::new(
        FileStore {
            doc: Document::new(opened.project),
            session: opened.session,
        },
        &library,
    );
    assert_eq!(
        motor_potentials(&reflow, reflow.project(), &s),
        supply_order(&s)
    );
}

// ----- Gaps whose pull request is merged ----------------------------------

/// Steps 1 and 7 through the app's project backend (#34): a new project is
/// kept on disk from the start, the first save moves it to the chosen file,
/// and opening that file shows the same starter. (Driving the window itself
/// headless, through `NovoApp::execute`, waits for #46.)
#[test]
fn gap_create_open_and_save_through_the_app() {
    let library = Library::builtin().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let untitled = dir.path().join("untitled");
    let path = dir.path().join("gate1.novo");
    let novo_files = |d: &std::path::Path| {
        std::fs::read_dir(d)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|e| e.path().extension().is_some_and(|x| x == "novo"))
                    .count()
            })
            .unwrap_or(0)
    };

    // 1. File > New: one empty page, saved state, already kept on disk.
    let mut backend = StorageBackend::with_untitled_dir(&untitled);
    backend.new_project("Gate 1").unwrap();
    assert!(!backend.is_dirty());
    assert_eq!(novo_files(&untitled), 1, "the new project is on disk");
    let pages = page_labels(backend.doc());
    assert_eq!(pages.len(), 1);
    let page = pages[0].0;

    // 2-6 on that page, every change set recorded with the backend.
    let mut flow = Flow::new(backend, &library);
    let s = build(&mut flow, page, true);
    assert!(flow.store.is_dirty());
    let before = flow.project().to_json();

    // 7. Save to a chosen file: clean, and the untitled file is gone.
    flow.store.save(&path).unwrap();
    assert!(!flow.store.is_dirty());
    assert_eq!(novo_files(&untitled), 0, "the untitled file moved");
    flow.store.close().unwrap();
    assert!(flow.store.document().is_none());

    // File > Open in a fresh window: the same project and potentials.
    let mut reopened = StorageBackend::default();
    let report = reopened.open(&path).unwrap();
    assert!(report.set_aside.is_none());
    assert!(reopened.recovery_offer().is_none());
    assert!(!reopened.is_dirty());
    assert_eq!(reopened.doc().project().to_json(), before);
    let reflow = Flow::new(reopened, &library);
    assert_eq!(
        motor_potentials(&reflow, reflow.project(), &s),
        supply_order(&s)
    );
}

/// The I search takes short names in any case (#37's `by_alias`, #29's
/// insert search) and proposes the next free tag for the symbol (#29).
#[test]
fn gap_insert_search_ignores_case() {
    let library = Library::builtin().unwrap();
    for alias in ["mcb3", "main3", "ol", "m3", "term", "xpe"] {
        let lower = library.by_alias(alias).unwrap();
        for typed in [alias.to_uppercase(), format!(" {} ", alias.to_uppercase())] {
            assert_eq!(library.by_alias(&typed).map(|s| &s.id), Some(&lower.id));
            assert_eq!(mnemonic(&library, &typed).map(|s| &s.id), Some(&lower.id));
        }
        // The exact short name comes first, in either case.
        let search = InsertSearch::default();
        for typed in [alias.to_string(), alias.to_uppercase()] {
            let hits = search.search(&library, &typed);
            let first = hits.first().unwrap_or_else(|| panic!("no hit for {typed}"));
            assert_eq!(first.symbol.id, lower.id, "{typed}");
            assert_eq!(first.mnemonic, Some(alias), "{typed}");
        }
    }

    // In the Gate 1 project the next contactor is -K2, the next motor -M2
    // in the field, and a new location starts again at 1. An MCB is coded
    // F (protection), so after -F1 it gets -F2; the fixture's -Q1 is typed
    // by hand (observation O1 in docs/TEST_REPORT.md).
    let doc = Document::new(Project::new("Gate 1"));
    let dir = tempfile::tempdir().unwrap();
    let session = Session::create(dir.path().join("gate1.novo"), doc.project()).unwrap();
    let mut flow = Flow::new(FileStore { doc, session }, &library);
    let page = created(
        &flow.run(Command::AddPage {
            name: "1".into(),
            title: String::new(),
            kind: PageKind::default(),
            format: PageFormat::default(),
        }),
        |id| match id {
            AnyId::Page(p) => Some(p),
            _ => None,
        },
    );
    build(&mut flow, page, true);
    let next = |structure: &str, alias: &str| {
        let structure = DeviceTag::parse(structure).unwrap();
        let symbol = mnemonic(&library, alias).unwrap();
        next_tag(flow.project(), &structure, symbol).map(|t| t.to_string())
    };
    assert_eq!(next("=A1+CAB", "MAIN3").as_deref(), Some("=A1+CAB-K2"));
    assert_eq!(next("=A1+FLD", "M3").as_deref(), Some("=A1+FLD-M2"));
    assert_eq!(next("=A1+CAB", "MCB3").as_deref(), Some("=A1+CAB-F2"));
    assert_eq!(next("=A1+CAB", "OL").as_deref(), Some("=A1+CAB-F2"));
    assert_eq!(next("=A2+CAB", "MAIN3").as_deref(), Some("=A2+CAB-K1"));
}

/// Step 4 without potential feed symbols (#35): select the three wires
/// from `-Q1` to `-K1` and assign a 400 V three-phase set in one step; the
/// motor reads L1, L2, L3. PE comes from the `-X1:PE` terminal by the
/// project's terminal-name rule once the project has a PE potential.
#[test]
fn gap_assign_potentials_to_selected_wires() {
    let library = Library::builtin().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let doc = Document::new(Project::new("Gate 1"));
    let session = Session::create(dir.path().join("gate1.novo"), doc.project()).unwrap();
    let mut flow = Flow::new(FileStore { doc, session }, &library);
    assert!(flow.project().info().rules.potentials.from_terminal_names);
    let page = created(
        &flow.run(Command::AddPage {
            name: "1".into(),
            title: String::new(),
            kind: PageKind::default(),
            format: PageFormat::default(),
        }),
        |id| match id {
            AnyId::Page(p) => Some(p),
            _ => None,
        },
    );
    let s = build(&mut flow, page, false);
    assert!(flow.project().potentials().next().is_none());
    assert_eq!(
        motor_potentials(&flow, flow.project(), &s),
        [None; 4].to_vec()
    );

    let depth = flow.store.doc().undo_depth();

    // One step: three wires, the set has L1, L2, L3 and PE; the extra PE
    // is ignored.
    let set = PotentialSet::ThreePhase {
        voltage: 400.0,
        neutral: false,
        pe: true,
        pen: false,
    };
    let outcome = flow.run(Command::AssignPotentials {
        wires: s.q1_to_k1.clone(),
        potentials: set.specs(),
    });
    let p = flow.project();
    let made: Vec<(String, PotentialKind, Option<f64>)> = outcome
        .created
        .iter()
        .map(|&id| match id {
            AnyId::Potential(id) => p.potential(id).unwrap(),
            other => panic!("created {other:?}"),
        })
        .map(|p| (p.name.clone(), p.kind, p.voltage))
        .collect();
    let line = |n: &str| (n.to_string(), PotentialKind::Line, Some(400.0));
    assert_eq!(made, [line("L1"), line("L2"), line("L3")]);
    let by_name = |name: &str| p.potentials().find(|q| q.name == name).map(|q| q.id);
    let (l1, l2, l3) = (by_name("L1"), by_name("L2"), by_name("L3"));
    assert_eq!(motor_potentials(&flow, p, &s), [l1, l2, l3, None].to_vec());
    assert_eq!(p.netlist().potential_conflicts().count(), 0);
    assert_eq!(flow.store.doc().undo_label(), Some("Assign potential"));

    // Running it again reuses the potentials and creates nothing.
    let again = flow.run(Command::AssignPotentials {
        wires: s.q1_to_k1.clone(),
        potentials: set.specs(),
    });
    assert!(again.created.is_empty());

    // More wires than potentials is refused.
    let mut four = s.q1_to_k1.clone();
    four.push(s.q1_to_k1[0]);
    assert!(
        flow.store
            .doc_mut()
            .execute(Command::AssignPotentials {
                wires: four,
                potentials: PotentialSet::ThreePhase {
                    voltage: 400.0,
                    neutral: false,
                    pe: false,
                    pen: false,
                }
                .specs(),
            })
            .is_err()
    );

    // PE: the terminal named PE defines it once the project has one.
    let pe = flow.potential("PE", PotentialKind::ProtectiveEarth);
    assert_eq!(
        motor_potentials(&flow, flow.project(), &s),
        [l1, l2, l3, Some(pe)].to_vec()
    );

    // Undo takes it all off again, one step per edit.
    while flow.store.doc().undo_depth() > depth {
        flow.store.doc_mut().undo().unwrap().unwrap();
    }
    assert!(flow.project().potentials().next().is_none());
    assert_eq!(
        motor_potentials(&flow, flow.project(), &s),
        [None; 4].to_vec()
    );
}

// ----- Gaps whose pull request is still open ------------------------------

#[test]
#[ignore = "known gap: placing by search with next free tag and auto-connect in a column is in #38 (novo-editor)"]
fn gap_place_by_search_with_auto_tags_and_self_wiring() {
    // Steps 2 and 3 as the user does them: I + `mcb3`, `main3`, `ol`, `m3`
    // stacked in one column gives -Q1, -K1, -F1, -M1 and the 9 main wires
    // without wire clicks.
    panic!("gap: closed by #38");
}

#[test]
#[ignore = "known gap: putting terminals on wires in one call and the strip screen are in #30 (novo-terminals)"]
fn gap_place_terminals_on_wires_and_show_the_strip() {
    // Step 5 as the user does it: place_terminals on the four motor wires
    // (numbers 1, 2, 3, PE, PE inferred), then screen() and check() clean.
    panic!("gap: closed by #30");
}

#[test]
#[ignore = "known gap: assigning catalog parts is in #36 (novo-parts)"]
fn gap_assign_parts_from_the_catalog() {
    // Every device gets a catalog part; missing_parts() is empty.
    panic!("gap: closed by #36");
}

#[test]
#[ignore = "known gap: drawing library artwork on the canvas is in #38 (LibraryArt)"]
fn gap_draw_real_symbols() {
    // Step 6 with art::LibraryArt instead of PlaceholderArt. Pickable tags
    // and F2 (a tag covering its neighbour) are fixed by #28 and checked in
    // e2e_motor_starter.rs.
    panic!("gap: closed by #38");
}
