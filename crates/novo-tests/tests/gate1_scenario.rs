//! Gate 1, "first real flow": create a project, place a motor starter (MCB,
//! contactor, overload relay, motor), wire it, see its terminals and
//! potentials, then save and reload.
//!
//! `scenario_on_main` runs every step that main can do today, through
//! `Command`s only (the path the editor, scripts and the AI assistant share)
//! and through the real project file. Each step main cannot do yet is an
//! ignored test named `gap_*`, whose reason names the pull request that
//! should close it. `cargo test -p novo-tests --test gate1_scenario --
//! --ignored` lists them; when a PR lands, its gap test is filled in and
//! un-ignored.

use novo_canvas::model::{PageScene, PlaceholderArt, item_id};
use novo_core::*;
use novo_storage::Session;
use novo_symbols::Library;

/// The ids `execute` created, of one kind.
fn created<T>(outcome: &Outcome, pick: impl Fn(AnyId) -> Option<T>) -> T {
    outcome
        .created
        .iter()
        .copied()
        .find_map(pick)
        .expect("the command created an object of this kind")
}

/// What the user does, step by step, through commands only.
struct Flow<'a> {
    doc: Document,
    library: &'a Library,
}

impl Flow<'_> {
    fn run(&mut self, command: Command) -> Outcome {
        self.doc
            .execute(command)
            .unwrap_or_else(|e| panic!("command refused: {e}"))
    }

    /// Places a library symbol, found by its short name (alias) as the
    /// user types it, with the library's pins, bridges and pass-through.
    fn place(&mut self, page: PageId, alias: &str, x: f64, y: f64, role: SymbolRole) -> SymbolId {
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
        created(&outcome, |id| match id {
            AnyId::Symbol(s) => Some(s),
            _ => None,
        })
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

    fn potential(&mut self, name: &str, kind: PotentialKind) -> PotentialId {
        let outcome = self.run(Command::AddPotential {
            name: name.into(),
            kind,
            voltage: Some(230.0),
        });
        created(&outcome, |id| match id {
            AnyId::Potential(p) => Some(p),
            _ => None,
        })
    }

    /// The index of the pin named `name` on a placed symbol.
    fn pin(&self, symbol: SymbolId, name: &str) -> Endpoint {
        let s = self.doc.project().symbol(symbol).unwrap();
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
}

fn role(device: DeviceId) -> SymbolRole {
    SymbolRole::Device { device, main: true }
}

#[test]
fn scenario_on_main() {
    let library = Library::builtin().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gate1.novo");

    // 1. Create a project and its file.
    let mut flow = Flow {
        doc: Document::new(Project::new("Gate 1")),
        library: &library,
    };
    let mut session = Session::create(&path, flow.doc.project()).unwrap();
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

    // 2. Place the supply and the starter: MCB, contactor, overload relay,
    //    terminals, motor, each found by the short name the user types.
    let phases = ["L1", "L2", "L3"].map(|name| flow.potential(name, PotentialKind::Line));
    let pe = flow.potential("PE", PotentialKind::ProtectiveEarth);
    let feeds: Vec<SymbolId> = phases
        .iter()
        .chain([&pe])
        .zip([40.0, 48.0, 56.0, 72.0])
        .map(|(&potential, x)| {
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
    let mut record = flow.doc.project().terminal(pe_terminal).unwrap().clone();
    record.kind = TerminalKind::ProtectiveEarth;
    flow.run(Command::Replace {
        record: Record::Terminal(record),
    });
    let m1_sym = flow.place(page, "m3", 40.0, 180.0, role(m1));

    // 3. Wire it: supply, MCB, contactor, overload, terminals, motor.
    for (i, (top, bottom)) in [("1", "2"), ("3", "4"), ("5", "6")].into_iter().enumerate() {
        let feed = Endpoint::pin(feeds[i], 0);
        let q1_in = flow.pin(q1_sym, top);
        flow.wire(feed, q1_in);
        for (from, to) in [(q1_sym, k1_sym), (k1_sym, f1_sym)] {
            let (a, b) = (flow.pin(from, bottom), flow.pin(to, top));
            flow.wire(a, b);
        }
        let (a, b) = (flow.pin(f1_sym, bottom), Endpoint::pin(terminals[i].1, 0));
        flow.wire(a, b);
    }
    flow.wire(Endpoint::pin(feeds[3], 0), Endpoint::pin(terminals[3].1, 0));
    for (i, motor_pin) in ["U1", "V1", "W1", "PE"].into_iter().enumerate() {
        let to = flow.pin(m1_sym, motor_pin);
        flow.wire(Endpoint::pin(terminals[i].1, 1), to);
    }

    // 4. See the potentials: each phase reaches the motor through the
    //    breaker, contactor, overload and terminal; PE reaches its pin.
    let net = flow.doc.project().netlist();
    for (i, motor_pin) in ["U1", "V1", "W1", "PE"].into_iter().enumerate() {
        let expected = phases.get(i).copied().unwrap_or(pe);
        assert_eq!(
            net.potential_of(flow.pin(m1_sym, motor_pin)),
            Some(expected),
            "-M1:{motor_pin}"
        );
    }
    assert_eq!(net.potential_conflicts().count(), 0);

    // 5. See the terminals: the strip in order, with full designations and
    //    kinds, and each terminal's motor side on the motor's net.
    let p = flow.doc.project();
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
    for (i, motor_pin) in ["U1", "V1", "W1", "PE"].into_iter().enumerate() {
        assert!(net.connected(
            Endpoint::pin(terminals[i].1, 1),
            flow.pin(m1_sym, motor_pin)
        ));
    }
    // Searching for the strip's tag finds it.
    assert!(p.search("-X1").iter().any(|h| h.id == AnyId::Device(x1)));

    // 6. The page draws every placed object.
    let scene = PageScene::new(p, page, &PlaceholderArt);
    let contents = p.page_contents(page);
    assert_eq!(contents.symbols.len(), 4 + 3 + 4 + 1);
    assert_eq!(contents.wires.len(), 3 * 4 + 1 + 4);
    for id in contents.symbols.iter().map(|&s| item_id(s)) {
        assert!(scene.scene().contains(id));
    }

    // 7. Save and reload: the same project, nets and potentials.
    let mut all = ChangeSet {
        info: true,
        ..ChangeSet::default()
    };
    all.added.extend(p.records().map(|r| r.id()));
    session.record(&all);
    session.save(p).unwrap();
    session.close().unwrap();
    let opened = Session::open(&path).unwrap();
    assert_eq!(opened.project.to_json(), p.to_json());
    let reloaded = opened.project.netlist();
    for (i, motor_pin) in ["U1", "V1", "W1", "PE"].into_iter().enumerate() {
        let expected = phases.get(i).copied().unwrap_or(pe);
        assert_eq!(
            reloaded.potential_of(flow.pin(m1_sym, motor_pin)),
            Some(expected)
        );
    }
}

// ----- Known gaps: steps of the flow main cannot do yet -------------------

#[test]
#[ignore = "known gap: the app's project backend (new_project, open, save, record) is in #34"]
fn gap_create_open_and_save_through_the_app() {
    // Drive steps 1 and 7 through novo_app::StorageBackend instead of
    // novo_storage::Session, and NovoApp headless for the window.
}

#[test]
#[ignore = "known gap: placing by search with next free tag and auto-connect in a column is in #38 (novo-editor)"]
fn gap_place_by_search_with_auto_tags_and_self_wiring() {
    // Steps 2 and 3 as the user does them: I + `mcb3`, `main3`, `ol`, `m3`
    // stacked in one column gives -Q1, -K1, -F1, -M1 and the 9 main wires
    // without wire clicks.
}

#[test]
#[ignore = "known gap: mixed-case short names (`MCB3`) need #37; mnemonic search and favourites need #29"]
fn gap_insert_search_ignores_case() {
    // Library::by_alias("MCB3") and the I search in novo-workflow.
}

#[test]
#[ignore = "known gap: giving selected wires L1/L2/L3 in one step is in #35 (AssignPotentials)"]
fn gap_assign_potentials_to_selected_wires() {
    // Step 4 without potential feed symbols: select three supply wires and
    // assign ThreePhase { 400 V }.
}

#[test]
#[ignore = "known gap: putting terminals on wires in one call and the strip screen are in #30 (novo-terminals)"]
fn gap_place_terminals_on_wires_and_show_the_strip() {
    // Step 5 as the user does it: place_terminals on the four motor wires
    // (numbers 1, 2, 3, PE, PE inferred), then screen() and check() clean.
}

#[test]
#[ignore = "known gap: assigning catalog parts is in #36 (novo-parts)"]
fn gap_assign_parts_from_the_catalog() {
    // Every device gets a catalog part; missing_parts() is empty.
}

#[test]
#[ignore = "known gap: drawing library artwork and labels on the canvas is in #38 (LibraryArt) and #28"]
fn gap_draw_real_symbols() {
    // Step 6 with art::LibraryArt instead of PlaceholderArt, tags as
    // pickable text (#28), and no tag covering a neighbour (finding F2).
}
