//! Text collisions on the stress matrix (`novo_tests::text_stress`).
//!
//! The baseline holds the number of conflicts per cause as of the build it was written
//! from. The test fails when any cause grows (a placement change made things worse) and
//! says so when one shrinks, so the baseline can be lowered with
//! `UPDATE_TEXT_BASELINE=1 cargo test -p novo-tests --test text_conflicts`. The aim, set
//! by docs/TEXT-PLACEMENT.md, is an empty baseline for every case that has room.
//!
//! The counts depend on the symbol artwork as well as on placement: a changed symbol
//! drawing can turn the ratchet red with no placement change. Before running the
//! update, look at which causes moved (the failure lists them) and why.
//!
//! Two ratchets exist. This one is the independent oracle on the 90 stress pages and
//! counts a device tag over its own symbol. `novo_canvas::text_audit` (tests/text_baseline.rs)
//! is the engine's measure on the Gate 1 motor starter; since stage 0b it counts the same
//! own-symbol case, so on the motor starter the two should agree. A placement stage that
//! lowers one must re-check the other (`kit_and_engine_audit_agree_on_the_motor_starter`).

use std::collections::BTreeMap;

use novo_canvas::model::SymbolArt;
use novo_core::Document;
use novo_editor::art::LibraryArt;
use novo_editor::catalog::Catalog;
use novo_tests::text_conflicts::{Conflict, Hit, LineKind, collect, conflicts};
use novo_tests::text_stress::matrix;

const BASELINE: &str = "tests/golden/text_conflicts_baseline.txt";

fn run() -> (Vec<Conflict>, BTreeMap<String, usize>) {
    let mut doc = Document::default();
    let pages = matrix(&mut doc).expect("matrix builds");
    let catalog = Catalog::builtin();
    let art = LibraryArt::new(catalog.library(), novo_canvas::Stroke::default());
    let art_ref: &dyn SymbolArt = &art;
    let art_labels = |id: &str| -> Vec<String> {
        catalog
            .get(id)
            .map(|s| {
                novo_editor::art::labels(s)
                    .into_iter()
                    .map(|l| l.text)
                    .collect()
            })
            .unwrap_or_default()
    };
    let project = doc.project();
    let xref = novo_xref::XrefIndex::build(project, catalog.library());
    let mut all = Vec::new();
    for page in pages {
        let (ps, boxes, lines) = collect(project, page, art_ref, &art_labels, Some(&xref));
        let sheet = ps.scene().sheet().map(|s| (s.rect.min, s.rect.max));
        all.extend(conflicts(page, &boxes, &lines, sheet));
    }
    let mut by_cause: BTreeMap<String, usize> = BTreeMap::new();
    for c in &all {
        *by_cause.entry(c.cause()).or_default() += 1;
    }
    (all, by_cause)
}

fn parse(text: &str) -> BTreeMap<String, usize> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let (n, cause) = l.split_once('\t')?;
            Some((cause.to_string(), n.parse().ok()?))
        })
        .collect()
}

#[test]
fn text_conflicts_do_not_grow() {
    let (_, now) = run();
    let path = format!("{}/{BASELINE}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("UPDATE_TEXT_BASELINE").is_some() {
        let mut out =
            String::from("# conflicts per cause on the text stress matrix: count<TAB>cause\n");
        for (cause, n) in &now {
            out.push_str(&format!("{n}\t{cause}\n"));
        }
        std::fs::write(&path, out).unwrap();
        return;
    }
    let base = parse(&std::fs::read_to_string(&path).expect("baseline file"));
    let mut worse = Vec::new();
    let mut better = Vec::new();
    for (cause, &n) in &now {
        let was = base.get(cause).copied().unwrap_or(0);
        if n > was {
            worse.push(format!("{cause}: {was} -> {n}"));
        }
    }
    for (cause, &was) in &base {
        let n = now.get(cause).copied().unwrap_or(0);
        if n < was {
            better.push(format!("{cause}: {was} -> {n}"));
        }
    }
    if !better.is_empty() {
        eprintln!(
            "fewer conflicts, lower the baseline (UPDATE_TEXT_BASELINE=1):\n{}",
            better.join("\n")
        );
    }
    assert!(
        worse.is_empty(),
        "more text conflicts than the baseline:\n{}",
        worse.join("\n")
    );
}

/// The target of the placement work: no conflict at all on pages that have room.
/// Stays ignored until the engine lands; run it with `--ignored` to see the gap.
#[test]
#[ignore = "target of docs/TEXT-PLACEMENT.md, not met yet"]
fn no_text_conflicts_on_roomy_pages() {
    let (all, _) = run();
    let roomy: Vec<_> = all.iter().filter(|c| !c.tight).collect();
    assert!(roomy.is_empty(), "{} overlapping texts", roomy.len());
}

#[test]
fn the_finder_sees_a_collision_it_should() {
    use novo_canvas::{Anchor, Label, Vec2};
    use novo_tests::text_conflicts::{Boxed, Role};
    let b = |x: f32, text: &str| Boxed {
        role: Role::DeviceTag,
        text: text.into(),
        owner: None,
        label: Label::new(Vec2::new(x, 10.0), text, 3.0).anchored(Anchor::CenterLeft),
        hidden_by_floor: false,
    };
    let page = novo_core::PageId(1);
    // 5 chars at 3 mm and 0.6 advance are 9 mm wide: 5 mm apart overlaps, 12 mm apart does not.
    let boxes = [b(0.0, "-K100"), b(5.0, "-K101")];
    assert_eq!(conflicts(page, &boxes, &[], None).len(), 1);
    let boxes = [b(0.0, "-K100"), b(12.0, "-K101")];
    assert!(conflicts(page, &boxes, &[], None).is_empty());
}

/// The committed fixtures hold what `matrix()` builds: same page names in the same order.
/// When this fails, regenerate with
/// `cargo run -p novo-tests --example text_conflicts -- qa/fixtures` and commit the two `.novo` files.
#[test]
fn the_committed_fixture_is_the_matrix() {
    let mut doc = Document::default();
    matrix(&mut doc).expect("matrix builds");
    let built: Vec<String> = doc
        .project()
        .pages_in_order()
        .iter()
        .map(|p| p.name.clone())
        .collect();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/text-stress-matrix.novo");
    let project = novo_storage::ProjectFile::open(&path)
        .and_then(|f| f.load())
        .expect("fixture opens");
    let stored: Vec<String> = project
        .pages_in_order()
        .iter()
        .map(|p| p.name.clone())
        .collect();
    assert_eq!(
        built, stored,
        "regenerate qa/fixtures/text-stress-matrix.novo"
    );
}

/// On the Gate 1 motor starter the kit and the engine's `text_audit` measure the same
/// thing. Neither may find a text conflict the other has no trace of.
#[test]
fn kit_and_engine_audit_agree_on_the_motor_starter() {
    let catalog = Catalog::builtin();
    let art = LibraryArt::new(catalog.library(), novo_canvas::Stroke::default());
    let art_ref: &dyn SymbolArt = &art;
    let art_labels = |id: &str| -> Vec<String> {
        catalog
            .get(id)
            .map(|s| {
                novo_editor::art::labels(s)
                    .into_iter()
                    .map(|l| l.text)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut doc = Document::default();
    let m = novo_tests::motor_starter(&mut doc).unwrap();
    let project = doc.project();
    for page in [m.main_page, m.control_page] {
        let (ps, boxes, lines) = collect(project, page, art_ref, &art_labels, None);
        let sheet = ps.scene().sheet().map(|s| (s.rect.min, s.rect.max));
        // The engine counts one conflict per label and item and leaves the frame's ink out;
        // the kit counts every line segment a label crosses. Compare like with like.
        let mut seen = std::collections::BTreeSet::new();
        for c in conflicts(page, &boxes, &lines, sheet)
            .iter()
            .filter(|c| !c.tight)
        {
            let other = match &c.hit {
                Hit::Text(o) => format!("{:?}{}", o.owner, o.text),
                Hit::Line(l) if l.kind == LineKind::Frame => continue,
                Hit::Line(l) => format!("{:?}", l.owner),
                Hit::OutsideSheet => continue,
            };
            seen.insert(format!("{:?}{}|{other}", c.text.owner, c.text.text));
        }
        let kit = seen.len();
        let engine = novo_canvas::text_audit::audit(ps.scene()).total();
        eprintln!("page {page:?}: kit {kit}, engine {engine}");
        assert_eq!(kit, engine, "kit {kit} vs engine {engine}");
    }
}
