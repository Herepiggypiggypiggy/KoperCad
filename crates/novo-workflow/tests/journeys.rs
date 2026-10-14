//! WS-08 journeys against the Gate 1 motor starter (docs/WORKFLOW.md J4,
//! J6 and J8).

use novo_core::{AnyId, Document, Point, Project, SymbolRole};
use novo_symbols::Library;
use novo_tests::motor_starter;
use novo_workflow::commands::Context;
use novo_workflow::duplicate::{TagMode, duplicate};
use novo_workflow::keymap::Keymap;
use novo_workflow::locate;
use novo_workflow::palette::{Kind, Mode, Palette, Sources, Target};

fn sources<'a>(project: &'a Project, library: &'a Library, keymap: &'a Keymap) -> Sources<'a> {
    Sources {
        project: Some(project),
        library: Some(library),
        keymap,
        context: Context {
            has_project: true,
            has_page: true,
            ..Context::default()
        },
    }
}

#[test]
fn exact_tag_lists_every_location_first() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);

    for query in ["K1", "-K1", "k1", "=A1+CAB-K1", " -K1 "] {
        let rows = palette.search(query, &src);
        assert_eq!(rows[0].target, Target::Device(gate.k1), "{query}");
        let places: Vec<String> = rows[1..4].iter().map(|r| r.label.clone()).collect();
        // The coil (main function) first, then by page and column.
        assert_eq!(
            places,
            ["=A1+CAB-K1 /2.1", "=A1+CAB-K1 /1.1", "=A1+CAB-K1 /2.2"],
            "{query}"
        );
        assert!(rows[1..4].iter().all(|r| r.kind == Kind::Location));
        assert_eq!(
            rows[1].target,
            Target::Location {
                device: gate.k1,
                symbol: gate.k1_coil
            }
        );
    }
    // A full tag in another location does not match.
    let rows = palette.search("=A1+FLD-K1", &src);
    assert!(!rows.iter().any(|r| r.target == Target::Device(gate.k1)));
}

#[test]
fn locations_are_page_and_column() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let refs: Vec<String> = locate::locations(doc.project(), gate.k1)
        .iter()
        .map(locate::reference)
        .collect();
    assert_eq!(refs, ["/2.1", "/1.1", "/2.2"]);
}

#[test]
fn pages_by_number_and_title() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);
    assert_eq!(
        palette.search("2", &src)[0].target,
        Target::Page(gate.control_page)
    );
    assert_eq!(
        palette.search("control circuit", &src)[0].target,
        Target::Page(gate.control_page)
    );
}

#[test]
fn commands_show_shortcuts_and_reasons() {
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let project = Project::new("empty");
    let palette = Palette::default();
    let src = sources(&project, &library, &keymap);

    let rows = palette.search("undo", &src);
    assert_eq!(rows[0].target, Target::Command("edit.undo"));
    assert_eq!(rows[0].shortcut.as_deref(), Some("Ctrl+Z"));
    assert_eq!(rows[0].disabled, Some("Nothing to undo"));

    let rows = palette.search("dup", &src);
    assert_eq!(rows[0].target, Target::Command("edit.duplicate"));
    assert_eq!(rows[0].shortcut.as_deref(), Some("Ctrl+D"));

    // Symbols are in the same list.
    let rows = palette.search("mpcb", &src);
    assert!(rows.iter().any(|r| r.kind == Kind::Symbol
        && r.target == Target::Symbol("iec/protection/motor_protection_breaker".into())));
}

#[test]
fn empty_field_is_the_menu() {
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let project = Project::new("empty");
    let mut palette = Palette::default();
    palette.picked(Target::Command("view.fit"));
    palette.picked(Target::Command("page.new"));
    let src = sources(&project, &library, &keymap);
    let rows = palette.search("", &src);
    // Repeat last tool first, then recent picks, newest first.
    assert_eq!(rows[0].target, Target::Command("tool.repeat"));
    assert_eq!(rows[1].target, Target::Command("page.new"));
    assert_eq!(rows[1].group, "Recent");
    assert_eq!(rows[2].target, Target::Command("view.fit"));
    // Then every command, by group, with its key and reason.
    let menu = &rows[3..];
    assert_eq!(menu.len(), novo_workflow::commands::COMMANDS.len() - 1);
    assert_eq!(menu[0].group, "Project");
    assert_eq!(menu.last().unwrap().group, "Export");
    let undo = menu
        .iter()
        .find(|r| r.target == Target::Command("edit.undo"))
        .unwrap();
    assert_eq!(undo.shortcut.as_deref(), Some("Ctrl+Z"));
    assert_eq!(undo.disabled, Some("Nothing to undo"));
    // A short query still prefers the recent pick.
    assert_eq!(
        palette.search("n", &src)[0].target,
        Target::Command("page.new")
    );
}

#[test]
fn place_mode_shows_favourites_and_only_symbols() {
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let project = Project::new("empty");
    let palette = Palette::default();
    let src = sources(&project, &library, &keymap);
    let rows = palette.search_in(Mode::Place, "", &src);
    assert_eq!(rows.len(), 9);
    assert_eq!(rows[0].shortcut.as_deref(), Some("1"));
    assert_eq!(rows[0].target, Target::Symbol("iec/contacts/no".into()));
    let rows = palette.search_in(Mode::Place, "undo", &src);
    assert!(rows.iter().all(|r| r.kind == Kind::Symbol));
}

#[test]
fn grammar_verb_noun_and_bare_verb() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);

    // A mnemonic with a device places a child of that device.
    for query in ["no K1", "K1 no", "no -K1", "nc =A1+CAB-K1"] {
        let rows = palette.search(query, &src);
        assert!(
            matches!(&rows[0].target, Target::Child { device, .. } if *device == gate.k1),
            "{query}: {:?}",
            rows[0]
        );
    }
    let rows = palette.search("no K1", &src);
    assert_eq!(rows[0].label, "Make contact (NO) of =A1+CAB-K1");
    // A verb with words after it runs on them.
    let rows = palette.search("renumber page", &src);
    assert_eq!(
        rows[0].target,
        Target::Run {
            command: "edit.renumber",
            argument: "page".into()
        }
    );
    // A bare verb runs.
    assert_eq!(
        palette.search("problems", &src)[0].target,
        Target::Command("check.problems")
    );
    assert_eq!(
        palette.search("export pdf", &src)[0].target,
        Target::Command("export.pdf")
    );
    // A verb must be a whole word: "pdfs" is not "pdf" plus "s".
    assert!(
        !palette
            .search("pdfs", &src)
            .iter()
            .any(|r| matches!(r.target, Target::Run { .. }))
    );
    // A noun still jumps.
    assert_eq!(
        palette.search("K1", &src)[0].target,
        Target::Device(gate.k1)
    );
}

#[test]
fn duplicating_the_main_circuit_renumbers_without_asking() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let before = doc.project().clone();
    let page1 = doc.project().page_contents(gate.main_page);
    let mut items: Vec<AnyId> = page1.symbols.iter().map(|&s| s.into()).collect();
    items.extend(page1.junctions.iter().map(|&j| AnyId::from(j)));

    let (copy, _) = doc
        .transact("Smart duplicate", |tx| {
            let page = tx.add_page("3")?;
            duplicate(tx, &items, page, Point::new(0.0, 0.0), TagMode::NextFree)
        })
        .unwrap();
    let project = doc.project();
    let tag = |id| project.device(id).unwrap().tag.to_string();

    // F1 had a main symbol on page 1: it becomes the next free F.
    assert_eq!(tag(copy.devices[&gate.f1]), "=A1+CAB-F4");
    assert_eq!(tag(copy.devices[&gate.f2]), "=A1+CAB-F5");
    assert_eq!(tag(copy.devices[&gate.m1]), "=A1+FLD-M2");
    // K1's coil is on page 2, so its copied main contacts stay with -K1.
    assert!(!copy.devices.contains_key(&gate.k1));
    let k1_copy = project.symbol(copy.symbols[&gate.k1_main]).unwrap();
    assert_eq!(k1_copy.device(), Some(gate.k1));
    // Terminals continue the strip, PE keeps its name.
    let names: Vec<String> = gate
        .x1_syms
        .iter()
        .map(|s| {
            let SymbolRole::Terminal { terminals } = &project.symbol(copy.symbols[s]).unwrap().role
            else {
                panic!("terminal role")
            };
            project.terminal(terminals[0]).unwrap().name.clone()
        })
        .collect();
    assert_eq!(names, ["4", "5", "6", "PE"]);
    // Every wire on page 1 came along, without wire numbers.
    assert_eq!(copy.wires.len(), page1.wires.len());
    assert!(
        copy.wires
            .values()
            .all(|&w| project.wire(w).unwrap().number.is_none())
    );
    // The copy connects like the original.
    assert_eq!(
        project.netlist().nets().len(),
        before.netlist().nets().len() + page1_nets(&before, gate.main_page)
    );

    // One gesture, one undo step.
    doc.undo().unwrap();
    assert_eq!(doc.project().devices().count(), before.devices().count());
    assert_eq!(doc.project().symbols().count(), before.symbols().count());
    assert_eq!(
        doc.project().terminals().count(),
        before.terminals().count()
    );
}

/// Nets that lie entirely on one page.
fn page1_nets(project: &Project, page: novo_core::PageId) -> usize {
    let netlist = project.netlist();
    netlist
        .nets()
        .iter()
        .filter(|net| netlist.pages_of_net(project, net.id) == [page])
        .count()
}

#[test]
fn duplicating_a_coil_takes_its_contacts_along() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let page2 = doc.project().page_contents(gate.control_page);
    let mut items: Vec<AnyId> = page2.symbols.iter().map(|&s| s.into()).collect();
    items.extend(page2.junctions.iter().map(|&j| AnyId::from(j)));
    items.extend(page2.wires.iter().map(|&w| AnyId::from(w)));

    let (copy, _) = doc
        .transact("Smart duplicate", |tx| {
            duplicate(
                tx,
                &items,
                gate.control_page,
                Point::new(84.0, 0.0),
                TagMode::NextFree,
            )
        })
        .unwrap();
    let project = doc.project();
    let k2 = copy.devices[&gate.k1];
    assert_eq!(project.device(k2).unwrap().tag.to_string(), "=A1+CAB-K2");
    // The copied hold contact belongs to the copied coil.
    assert_eq!(
        project.symbol(copy.symbols[&gate.k1_aux]).unwrap().device(),
        Some(k2)
    );
    // The overload contact's relay is on page 1: it stays -F2.
    assert_eq!(
        project.symbol(copy.symbols[&gate.f2_aux]).unwrap().device(),
        Some(gate.f2)
    );
    assert_eq!(
        project
            .device(copy.devices[&gate.s0])
            .unwrap()
            .tag
            .to_string(),
        "=A1+CAB-S2"
    );
    assert_eq!(
        project
            .device(copy.devices[&gate.s1])
            .unwrap()
            .tag
            .to_string(),
        "=A1+CAB-S3"
    );
    assert!(project.duplicate_tags().is_empty());
    // K2 is found by the palette at its new location, two columns right.
    let refs: Vec<String> = locate::locations(project, k2)
        .iter()
        .map(locate::reference)
        .collect();
    assert_eq!(refs, ["/2.3", "/2.4"]);
}

#[test]
fn paste_with_original_tags_keeps_them() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let items = [AnyId::from(gate.s1_sym)];
    let (copy, _) = doc
        .transact("Paste", |tx| {
            let page = tx.add_page("3")?;
            duplicate(tx, &items, page, Point::new(0.0, 0.0), TagMode::Keep)
        })
        .unwrap();
    let project = doc.project();
    assert_eq!(
        project
            .device(copy.devices[&gate.s1])
            .unwrap()
            .tag
            .to_string(),
        "=A1+CAB-S1"
    );
    assert_eq!(project.duplicate_tags().len(), 1);
}
