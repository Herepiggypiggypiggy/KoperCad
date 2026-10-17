//! WS-08 journeys against the Gate 1 motor starter (docs/WORKFLOW.md J4,
//! J6 and J8).

use novo_core::{
    AnyId, DeviceTag, Document, Pin, Placement, Point, Project, SymbolRole, TextContent,
    TextPlacement,
};
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
    // "page 2" and "pagina 2" go to page 2; they don't add a page.
    for query in ["page 2", "pagina 2"] {
        assert_eq!(
            palette.search(query, &src)[0].target,
            Target::Page(gate.control_page),
            "{query}"
        );
    }
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
    // Then every command under the menu it lives in (the submenu as the
    // grey detail), the keys-only ones last, with key and reason.
    let menu = &rows[3..];
    assert_eq!(menu.len(), novo_workflow::commands::COMMANDS.len() - 1);
    assert_eq!(menu[0].group, "File");
    assert_eq!(menu.last().unwrap().group, "Keys");
    let macros = menu
        .iter()
        .find(|r| r.target == Target::Command("macro.browser"))
        .unwrap();
    assert_eq!(macros.group, "File");
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
    let slots: Vec<_> = rows.iter().filter_map(|r| r.shortcut.clone()).collect();
    assert_eq!(slots, ["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    // The library's other favourites follow, without a digit.
    assert_eq!(rows.len(), library.favorites().count());
    assert_eq!(rows[0].target, Target::Symbol("iec/contacts/no".into()));
    let rows = palette.search_in(Mode::Place, "undo", &src);
    assert!(rows.iter().all(|r| r.kind == Kind::Symbol));
}

#[test]
fn dutch_and_english_command_words_both_work() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);

    // A Dutch verb with a Dutch argument runs as the English command.
    for query in ["hernummer pagina", "Hernummeren pagina", "renumber page"] {
        assert_eq!(
            palette.search(query, &src)[0].target,
            Target::Run {
                command: "edit.renumber",
                argument: "page".into()
            },
            "{query}"
        );
    }
    // The longest Dutch verb wins, as in English.
    assert_eq!(
        palette.search("alles accepteren", &src)[0].target,
        Target::Command("proposal.accept_all")
    );
    // Dutch argument chips.
    let rows = palette.search("hernummer sel", &src);
    assert!(rows.iter().any(|r| r.target
        == Target::Run {
            command: "edit.renumber",
            argument: "selection".into()
        }));
    // Dutch words find commands.
    for (query, id) in [
        ("opslaan", "file.save"),
        ("sneltoetsen", "view.shortcuts"),
        ("stuklijst", "export.bom"),
    ] {
        let rows = palette.search(query, &src);
        assert!(
            rows.iter().take(3).any(|r| r.target == Target::Command(id)),
            "{query}: {:?}",
            rows.iter().take(3).map(|r| &r.label).collect::<Vec<_>>()
        );
    }
    // Tags are never translated: a tag still jumps.
    assert_eq!(
        palette.search("K1", &src)[0].target,
        Target::Device(gate.k1)
    );
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
fn accept_all_is_one_command_not_accept_with_an_argument() {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);
    for query in ["accept all", "Accept All", "accept all "] {
        let rows = palette.search(query, &src);
        assert_eq!(
            rows[0].target,
            Target::Command("proposal.accept_all"),
            "{query}"
        );
        assert!(
            !rows.iter().any(|r| r.target
                == Target::Run {
                    command: "proposal.accept",
                    argument: "all".into()
                }),
            "{query}"
        );
    }
    assert_eq!(
        palette.search("accept", &src)[0].target,
        Target::Command("proposal.accept")
    );
}

#[test]
fn the_device_list_after_a_contact_is_in_natural_tag_order() {
    let mut doc = Document::default();
    doc.transact("Devices", |tx| {
        for tag in ["-K10", "-K2", "-K1"] {
            tx.add_device(DeviceTag::parse(tag)?, "")?;
        }
        Ok(())
    })
    .unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);
    let labels: Vec<String> = palette
        .search("no ", &src)
        .into_iter()
        .filter(|r| matches!(r.target, Target::Child { .. }))
        .map(|r| r.label)
        .collect();
    assert_eq!(
        labels,
        [
            "Make contact (NO) of -K1",
            "Make contact (NO) of -K2",
            "Make contact (NO) of -K10"
        ]
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
    // K1's coil is on page 2, but its main poles were copied: they become
    // a new contactor -K2 (flagged by the checker as having no coil), not
    // a second set of main poles on -K1.
    assert_eq!(tag(copy.devices[&gate.k1]), "=A1+CAB-K2");
    let k1_copy = project.symbol(copy.symbols[&gate.k1_main]).unwrap();
    assert_eq!(k1_copy.device(), Some(copy.devices[&gate.k1]));
    assert_eq!(project.symbols_of_device(gate.k1).len(), 3);
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
    // The copy connects like the original, except for PE: the copied
    // -X1:PE keeps its name, and new projects join terminals with the same
    // name on one strip (the Join terminal rule), so the copy's PE net
    // merges with the original's. That is right: PE is one conductor.
    assert_eq!(
        project.netlist().nets().len(),
        before.netlist().nets().len() + page1_nets(&before, gate.main_page) - 1
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
fn a_lone_auxiliary_contact_stays_with_its_device() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let items = [AnyId::from(gate.k1_aux)];
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
    // A second 13/14 contact of -K1, not a new device.
    assert!(copy.devices.is_empty());
    assert_eq!(
        project.symbol(copy.symbols[&gate.k1_aux]).unwrap().device(),
        Some(gate.k1)
    );
    assert_eq!(project.symbols_of_device(gate.k1).len(), 4);
}

#[test]
fn duplicate_copies_attached_texts_with_their_objects() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let page1 = doc.project().page_contents(gate.main_page);
    let wire = page1.wires[0];
    let label = TextPlacement {
        offset: Point::new(-6.0, 4.0),
        ..TextPlacement::default()
    };
    let ((k1_label, f1_label, wire_label, note), _) = doc
        .transact("Label", |tx| {
            let k1 = tx.add_text(
                gate.main_page,
                TextContent::DeviceTag,
                label,
                Some(gate.k1_main.into()),
            )?;
            let f1 = tx.add_text(
                gate.main_page,
                TextContent::DeviceTag,
                label,
                Some(gate.f1_sym.into()),
            )?;
            let w = tx.add_text(
                gate.main_page,
                TextContent::WireNumber,
                label,
                Some(wire.into()),
            )?;
            let note = tx.add_text(
                gate.main_page,
                TextContent::Literal {
                    text: "400 V 50 Hz".into(),
                },
                TextPlacement {
                    offset: Point::new(20.0, 10.0),
                    ..TextPlacement::default()
                },
                None,
            )?;
            Ok((k1, f1, w, note))
        })
        .unwrap();
    let before = doc.project().clone();

    let mut items: Vec<AnyId> = page1.symbols.iter().map(|&s| s.into()).collect();
    items.extend(page1.junctions.iter().map(|&j| AnyId::from(j)));
    // Selecting a label too must not copy it twice.
    items.push(k1_label.into());
    items.push(note.into());
    let offset = Point::new(100.0, 0.0);
    let (copy, _) = doc
        .transact("Smart duplicate", |tx| {
            let page = tx.add_page("3")?;
            duplicate(tx, &items, page, offset, TagMode::NextFree)
        })
        .unwrap();
    let project = doc.project();
    assert_eq!(copy.texts.len(), 4);
    assert_eq!(project.texts().count(), before.texts().count() + 4);

    // Device tag labels sit on the copied symbols and show the new tags.
    let k1_copy = project.text(copy.texts[&k1_label]).unwrap();
    assert_eq!(
        k1_copy.attached_to,
        Some(copy.symbols[&gate.k1_main].into())
    );
    assert_eq!(k1_copy.placement, label);
    assert_eq!(
        project.text_string(k1_copy.id).as_deref(),
        Some("=A1+CAB-K2")
    );
    assert_eq!(
        project.text_string(copy.texts[&f1_label]).as_deref(),
        Some("=A1+CAB-F4")
    );
    // The originals still show the original tags.
    assert_eq!(project.text_string(k1_label).as_deref(), Some("=A1+CAB-K1"));
    // A wire label follows the copied wire.
    assert_eq!(
        project.text(copy.texts[&wire_label]).unwrap().attached_to,
        Some(copy.wires[&wire].into())
    );
    // A free text moves by the offset, on the new page.
    let note_copy = project.text(copy.texts[&note]).unwrap();
    assert_eq!(note_copy.attached_to, None);
    assert_eq!(note_copy.placement.offset, Point::new(120.0, 10.0));
    assert_eq!(
        note_copy.page,
        project.symbol(copy.symbols[&gate.k1_main]).unwrap().page
    );
    // The copies are selected with the rest.
    assert!(copy.new_ids().contains(&AnyId::from(note_copy.id)));

    // One gesture, one undo step.
    doc.undo().unwrap();
    assert_eq!(doc.project().texts().count(), before.texts().count());
}

#[test]
fn copied_devices_are_numbered_in_natural_tag_order() {
    let mut doc = Document::default();
    let two_pin = || vec![Pin::new("A1", 0.0, 0.0), Pin::new("A2", 0.0, 16.0)];
    let ((k10, k2, items), _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let k10 = tx.add_device(DeviceTag::parse("-K10").unwrap(), "")?;
            let k2 = tx.add_device(DeviceTag::parse("-K2").unwrap(), "")?;
            let mut items = Vec::new();
            for (device, x) in [(k10, 0.0), (k2, 20.0)] {
                let coil = tx.place_symbol(
                    page,
                    "iec/coils/relay",
                    Placement::at(Point::new(x, 0.0)),
                    two_pin(),
                    SymbolRole::Device { device, main: true },
                )?;
                items.push(AnyId::from(coil));
            }
            Ok((k10, k2, items))
        })
        .unwrap();
    let (copy, _) = doc
        .transact("Smart duplicate", |tx| {
            let page = tx.add_page("2")?;
            duplicate(tx, &items, page, Point::new(0.0, 0.0), TagMode::NextFree)
        })
        .unwrap();
    let tag = |id| doc.project().device(id).unwrap().tag.to_string();
    // -K2 before -K10, not text order.
    assert_eq!(tag(copy.devices[&k2]), "-K11");
    assert_eq!(tag(copy.devices[&k10]), "-K12");
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

    // Terminals keep their names too.
    let terminal = gate.x1_syms[0];
    let (copy, _) = doc
        .transact("Paste", |tx| {
            let page = tx.add_page("4")?;
            duplicate(
                tx,
                &[AnyId::from(terminal)],
                page,
                Point::new(0.0, 0.0),
                TagMode::Keep,
            )
        })
        .unwrap();
    let project = doc.project();
    let name = |symbol| {
        let SymbolRole::Terminal { terminals } = &project.symbol(symbol).unwrap().role else {
            panic!("terminal role")
        };
        project.terminal(terminals[0]).unwrap().name.clone()
    };
    assert_eq!(name(copy.symbols[&terminal]), name(terminal));
}

/// Picks rows the way a mouse user would: type a few letters, then follow
/// completions until a row runs. Returns the target that runs.
fn pick_path(palette: &Palette, src: &Sources<'_>, start: &str, picks: &[&str]) -> Target {
    let mut field = start.to_owned();
    for want in picks {
        let rows = palette.search(&field, src);
        let row = rows
            .iter()
            .find(|r| r.label.contains(want))
            .unwrap_or_else(|| panic!("no row with {want:?} for {field:?}: {rows:#?}"));
        match &row.completion {
            Some(next) => field = next.clone(),
            None => return row.target.clone(),
        }
    }
    panic!("picks ended before a row ran");
}

#[test]
fn every_grammar_form_can_be_picked_without_typing_syntax() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);

    // Verb, then parameter chip.
    assert_eq!(
        pick_path(
            &palette,
            &src,
            "renum",
            &["Renumber devices", "Renumber devices: page"]
        ),
        Target::Run {
            command: "edit.renumber",
            argument: "page".into()
        }
    );
    // Symbol verb, then object.
    assert_eq!(
        pick_path(
            &palette,
            &src,
            "make contact",
            &["of a device", "of =A1+CAB-K1"]
        ),
        Target::Child {
            symbol: "iec/contacts/no".into(),
            device: gate.k1
        }
    );
    // The object list offers every device, in tag order, before anything else.
    let rows = palette.search("no ", &src);
    assert_eq!(
        rows[0].target,
        Target::Child {
            symbol: "iec/contacts/no".into(),
            device: gate.f1
        }
    );
    assert!(
        rows[..7]
            .iter()
            .all(|r| matches!(r.target, Target::Child { .. }))
    );
    // A bare verb and a noun run or jump on the first pick.
    assert_eq!(
        pick_path(&palette, &src, "probl", &["Problems"]),
        Target::Command("check.problems")
    );
    assert_eq!(
        pick_path(&palette, &src, "K1", &["=A1+CAB-K1"]),
        Target::Device(gate.k1)
    );
}

#[test]
fn wiring_mode_is_one_key_and_findable_by_its_names() {
    use novo_workflow::keymap::{Chord, Focus, Key};
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);

    // Shift+W switches, W still connects; neither types into a field.
    let shift_w = Chord::shift(Key::letter('w'));
    assert_eq!(keymap.resolve(shift_w, Focus::Canvas), Some("wire.mode"));
    assert_eq!(keymap.resolve(shift_w, Focus::TextField), None);
    assert_eq!(
        keymap.resolve(Chord::plain(Key::letter('w')), Focus::Canvas),
        Some("tool.connect")
    );
    assert_eq!(
        keymap.shortcut_text("wire.mode").as_deref(),
        Some("Shift+W")
    );

    // Typed "mode", "lines" and the names of the two programs find it.
    for query in ["mode", "lines", "eplan", "autocad", "modus", "lijnen"] {
        let rows = palette.search(query, &src);
        assert!(
            rows.iter()
                .take(3)
                .any(|r| r.target == Target::Command("wire.mode")),
            "{query}: {:?}",
            rows.iter().take(3).map(|r| &r.label).collect::<Vec<_>>()
        );
    }
    // The verb with a mode runs that mode, in either language.
    for (query, mode) in [
        ("wiring mode lines", "lines"),
        ("wiring mode corners", "corners"),
        ("bedradingsmodus lijnen", "lines"),
        ("bedradingsmodus hoeken", "corners"),
    ] {
        assert_eq!(
            palette.search(query, &src)[0].target,
            Target::Run {
                command: "wire.mode",
                argument: mode.into()
            },
            "{query}"
        );
    }
}

#[test]
fn link_is_a_verb_bare_or_with_two_tags() {
    use novo_workflow::palette::link_tags;
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);

    // Bare `link` and the Dutch `koppel` find the command.
    for query in ["link", "koppel"] {
        let rows = palette.search(query, &src);
        assert!(
            rows.iter()
                .take(3)
                .any(|r| r.target == Target::Command("xref.link")
                    || matches!(
                        &r.target,
                        Target::Run {
                            command: "xref.link",
                            ..
                        }
                    )),
            "{query}: {:?}",
            rows.iter().take(3).map(|r| &r.label).collect::<Vec<_>>()
        );
    }
    // With two tags it runs, and the tags come back in order.
    assert_eq!(
        palette.search("link -K3 -K1", &src)[0].target,
        Target::Run {
            command: "xref.link",
            argument: "-K3 -K1".into()
        }
    );
    assert_eq!(link_tags("-K3 -K1"), Some(("-K3", "-K1")));
    assert_eq!(link_tags(" -K3  -K1 "), Some(("-K3", "-K1")));
    assert_eq!(link_tags(""), None);
    assert_eq!(link_tags("-K3"), None);
    assert_eq!(link_tags("-K3 -K1 -K2"), None);
}

#[test]
fn cables_are_found_by_tag_and_wires_by_number() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let wire = gate.motor_wires[0];
    doc.transact("Number", |tx| {
        tx.update::<novo_core::Wire>(wire, |w| w.number = Some("105".into()))
    })
    .unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);

    let rows = palette.search("W1", &src);
    let cable = rows
        .iter()
        .find(|r| r.kind == Kind::Cable)
        .expect("a cable row");
    assert_eq!(cable.target, Target::Cable(gate.w1));

    for query in ["wire 105", "105", "draad 105"] {
        let rows = palette.search(query, &src);
        let found = rows.iter().find(|r| r.kind == Kind::Wire);
        assert_eq!(
            found.map(|r| r.target.clone()),
            Some(Target::Wire(wire)),
            "{query}"
        );
    }
    let rows = palette.search("wire 999", &src);
    assert!(!rows.iter().any(|r| r.kind == Kind::Wire));
}

#[test]
fn a_typed_command_shows_the_menu_path_it_lives_in() {
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let project = Project::new("empty");
    let palette = Palette::default();
    let src = sources(&project, &library, &keymap);
    let rows = palette.search("import dxf", &src);
    let row = rows
        .iter()
        .find(|r| r.target == Target::Command("file.import_dxf_background"))
        .expect("the DXF import");
    assert_eq!(row.detail, "File › Import");
    // The path is searchable: a submenu name lists what is in it.
    let rows = palette.search("revisions", &src);
    assert!(
        rows.iter()
            .any(|r| r.target == Target::Command("revision.issue"))
    );
}

#[test]
fn the_report_designer_is_findable_in_both_languages() {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);
    for query in [
        "report designer",
        "report template",
        "title block",
        "rapportontwerper",
        "sjabloon",
    ] {
        let rows = palette.search(query, &src);
        assert!(
            rows.iter()
                .take(3)
                .any(|r| r.target == Target::Command("report.designer")),
            "{query}: {:?}",
            rows.iter().take(3).map(|r| &r.target).collect::<Vec<_>>()
        );
    }
}

#[test]
fn the_selection_property_sheet_is_findable_in_both_languages() {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let library = Library::builtin().unwrap();
    let keymap = Keymap::workflow();
    let palette = Palette::default();
    let src = sources(doc.project(), &library, &keymap);
    for query in [
        "properties of the selection",
        "bulk edit",
        "property sheet",
        "eigenschappen",
        "bulk bewerken",
    ] {
        let rows = palette.search(query, &src);
        assert!(
            rows.iter()
                .take(3)
                .any(|r| r.target == Target::Command("edit.shared_properties")),
            "{query}: {:?}",
            rows.iter().take(3).map(|r| &r.target).collect::<Vec<_>>()
        );
    }
}
