//! End-to-end scenario (draft for the next milestone): open a project, place
//! and wire the Gate 1 motor starter, see it on the canvas with its
//! terminals and cross references, then save, reload and see the same.
//!
//! Every step goes through the crates the app wires together: `novo-storage`
//! for the file, `novo-core` transactions for the edit, and `novo-canvas`'s
//! `PageScene` for what the user sees.

use novo_canvas::model::{PageScene, PlaceholderArt, item_id};
use novo_canvas::vec2;
use novo_core::*;
use novo_storage::Session;
use novo_tests::{MotorStarter, motor_starter_with_changes};

/// What the user can see of the project: per page, the model objects on
/// the canvas and the canvas bounds; plus the `-X1` strip and the `-K1`
/// cross references.
#[derive(Debug, PartialEq)]
struct View {
    pages: Vec<(String, Vec<AnyId>)>,
    strip: Vec<(String, TerminalKind, Option<String>)>,
    k1_refs: Vec<(String, u16, bool)>,
}

fn view(project: &Project, s: &MotorStarter) -> View {
    let pages = project
        .pages()
        .map(|page| {
            let scene = PageScene::new(project, page.id, &PlaceholderArt);
            let mut ids: Vec<AnyId> = scene
                .scene()
                .iter()
                .filter_map(|(item, _)| scene.model_id(item))
                .collect();
            ids.sort();
            (page.name.clone(), ids)
        })
        .collect();
    let strip = project
        .terminals_of_strip(s.x1)
        .into_iter()
        .map(|t| (t.name.clone(), t.kind, project.terminal_designation(t.id)))
        .collect();
    let k1_refs = project
        .cross_references(s.k1)
        .into_iter()
        .map(|r| (r.location.page_name, r.location.column, r.main))
        .collect();
    View {
        pages,
        strip,
        k1_refs,
    }
}

#[test]
fn open_place_wire_see_terminals_save_and_reload() {
    // Open: a new, empty project file.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("starter.novo");
    let mut doc = Document::default();
    let mut session = Session::create(&path, doc.project()).unwrap();

    // Place and wire: one undo step, as inserting a macro would be.
    let (s, changes) = motor_starter_with_changes(&mut doc).unwrap();
    session.record(&changes);
    let mut main = PageScene::new(doc.project(), s.main_page, &PlaceholderArt);
    let control = PageScene::new(doc.project(), s.control_page, &PlaceholderArt);

    // A follow-up edit reaches the canvas through its change set alone, as
    // in the editor, and matches a full redraw.
    let (_, moved) = doc
        .transact("Move -X1:PE", |tx| {
            tx.move_symbol(s.x1_syms[3], Placement::at(Point::new(80.0, 136.0)))
        })
        .unwrap();
    session.record(&moved);
    assert!(main.apply(doc.project(), &moved, &PlaceholderArt));
    let redrawn = PageScene::new(doc.project(), s.main_page, &PlaceholderArt);
    assert_eq!(main.scene().len(), redrawn.scene().len());
    assert_eq!(main.scene().bounds(), redrawn.scene().bounds());

    // Every placed symbol and wire is on the canvas of its page.
    for (page, scene) in [(s.main_page, &main), (s.control_page, &control)] {
        let contents = doc.project().page_contents(page);
        for &symbol in &contents.symbols {
            assert!(scene.scene().contains(item_id(symbol)), "{symbol:?} drawn");
        }
        for &wire in &contents.wires {
            assert!(scene.scene().contains(item_id(wire)), "{wire:?} drawn");
        }
    }

    // See the terminals: a click on terminal -X1:2 on the canvas picks it
    // first (finding F2, fixed in #28: -X1:1's tag no longer runs across
    // -X1:2 and wins the pick), and the strip reads 1, 2, 3, PE with full
    // designations.
    let at = doc.project().symbol(s.x1_syms[1]).unwrap().placement.origin;
    let hits: Vec<Option<AnyId>> = main
        .scene()
        .pick_all(vec2(at.x as f32, at.y as f32 + 4.0), 1.0, 1.0)
        .into_iter()
        .map(|h| main.model_id(h.id))
        .collect();
    assert_eq!(
        hits.first(),
        Some(&Some(AnyId::from(s.x1_syms[1]))),
        "{hits:?}"
    );
    let picked = main
        .scene()
        .pick(vec2(at.x as f32, at.y as f32 + 4.0), 1.0, 1.0)
        .and_then(|h| main.model_id(h.id));
    assert_eq!(picked, Some(AnyId::from(s.x1_syms[1])));
    let before = view(doc.project(), &s);
    let names: Vec<&str> = before.strip.iter().map(|(n, _, _)| n.as_str()).collect();
    assert_eq!(names, ["1", "2", "3", "PE"]);
    assert_eq!(before.strip[1].2.as_deref(), Some("=A1+CAB-X1:2"));

    // -K1 is cross-referenced from its coil to its contacts on both pages.
    let pages: std::collections::BTreeSet<&str> =
        before.k1_refs.iter().map(|(p, _, _)| p.as_str()).collect();
    assert_eq!(pages.len(), 2, "{:?}", before.k1_refs);
    assert_eq!(
        before.k1_refs.iter().filter(|r| r.2).count(),
        1,
        "one main symbol"
    );

    // Save, close, reload: the user sees exactly the same.
    session.save(doc.project()).unwrap();
    session.close().unwrap();
    let opened = Session::open(&path).unwrap();
    assert!(opened.recovered.is_none());
    assert_eq!(view(&opened.project, &s), before);
    let reloaded = PageScene::new(&opened.project, s.main_page, &PlaceholderArt);
    assert_eq!(reloaded.scene().len(), main.scene().len());
    assert_eq!(reloaded.scene().bounds(), main.scene().bounds());
}
