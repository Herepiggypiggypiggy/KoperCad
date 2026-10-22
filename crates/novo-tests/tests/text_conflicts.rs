//! Text collisions on the stress matrix (`novo_tests::text_stress`).
//!
//! The baseline holds the number of conflicts per cause as of the build it was written
//! from. The test fails when any cause grows (a placement change made things worse) and
//! says so when one shrinks, so the baseline can be lowered with
//! `UPDATE_TEXT_BASELINE=1 cargo test -p novo-tests --test text_conflicts`. The aim, set
//! by docs/TEXT-PLACEMENT.md, is an empty baseline for every case that has room.

use std::collections::BTreeMap;

use novo_canvas::model::SymbolArt;
use novo_core::Document;
use novo_editor::art::LibraryArt;
use novo_editor::catalog::Catalog;
use novo_tests::text_conflicts::{Conflict, collect, conflicts};
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
