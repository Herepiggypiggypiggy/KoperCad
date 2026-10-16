//! CPU timings of the model adapter on a big page: 10,000 symbols, each with
//! a bent wire. Run with
//! `cargo test --release -p novo-canvas --test model_perf -- --nocapture`;
//! in debug builds the bounds are deliberately loose.

use novo_canvas::model::{NoteAnchor, SymbolNote};
use novo_canvas::model::{PageScene, PlaceholderArt};
use novo_canvas::{Aabb, BoxMode, Vec2};
use novo_core::{DeviceTag, Document, Endpoint, Pin, Placement, Point, SymbolRole};
use std::time::{Duration, Instant};

const SYMBOLS: usize = 10_000;

fn time<T>(label: &str, runs: u32, mut f: impl FnMut() -> T) -> Duration {
    let start = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(f());
    }
    let per = start.elapsed() / runs;
    eprintln!("{label}: {:.3} ms", per.as_secs_f64() * 1000.0);
    per
}

#[test]
fn big_page_cpu_budget() {
    let mut doc = Document::default();
    let ((page, first), _) = doc
        .transact("big", |tx| {
            let page = tx.add_page("1")?;
            let strip = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
            let mut first = None;
            for i in 0..SYMBOLS {
                let t = tx.add_terminal(strip, i.to_string())?;
                let (x, y) = ((i % 60) as f64 * 40.0, (i / 60) as f64 * 40.0);
                let sym = tx.place_symbol(
                    page,
                    "iec/terminal",
                    Placement::at(Point::new(x, y)),
                    vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                    SymbolRole::Terminal { terminals: vec![t] },
                )?;
                first.get_or_insert(sym);
                let wire = tx.connect(
                    Endpoint::pin(sym, 0),
                    Endpoint::Free {
                        at: Point::new(x + 20.0, y - 12.5),
                    },
                )?;
                tx.update::<novo_core::Wire>(wire, |w| {
                    w.route = vec![Point::new(x, y - 10.0), Point::new(x + 20.0, y - 10.0)];
                })?;
            }
            Ok((page, first.unwrap()))
        })
        .unwrap();
    let build = time("build page scene", 1, || {
        PageScene::new(doc.project(), page, &PlaceholderArt)
    });
    let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
    let mut i = 0usize;
    let part = time("wire part under the pointer", 50, || {
        i += 1;
        let p = Vec2::new((i % 60) as f32 * 40.0 + 7.0, (i % 50) as f32 * 40.0);
        ps.wire_part_at(doc.project(), p, 2.0)
    });
    time("page_contents (10k symbols, 10k wires)", 20, || {
        doc.project().page_contents(page)
    });
    let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
    let mut k = 0.0;
    let mut edit_worst = Duration::ZERO;
    for _ in 0..20 {
        k += 1.0;
        let (_, changes) = doc
            .transact("move", |tx| {
                tx.move_symbol(first, Placement::at(Point::new(k, 0.0)))
            })
            .unwrap();
        let start = Instant::now();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        edit_worst = edit_worst.max(start.elapsed());
    }
    eprintln!("apply one moved symbol (worst of 20): {edit_worst:?}");
    // Cross-reference notes are handed over every frame; unchanged notes
    // must cost a comparison.
    let notes: Vec<SymbolNote> = doc
        .project()
        .page_contents(page)
        .symbols
        .into_iter()
        .map(|symbol| SymbolNote {
            symbol,
            lines: vec!["-K1 3.4".into()],
            warning: false,
            anchor: NoteAnchor::default(),
            rows: Vec::new(),
            density: Default::default(),
        })
        .collect();
    let first_notes = time("set 10k notes (first)", 1, || {
        ps.set_notes(doc.project(), notes.clone(), &PlaceholderArt)
    });
    let same_notes = time("set 10k notes (unchanged)", 5, || {
        ps.set_notes(doc.project(), notes.clone(), &PlaceholderArt)
    });
    // An edit with 10k notes on the page only places the notes near it.
    let mut with_notes_worst = Duration::ZERO;
    for _ in 0..20 {
        k += 1.0;
        let (_, changes) = doc
            .transact("move", |tx| {
                tx.move_symbol(first, Placement::at(Point::new(k, 0.0)))
            })
            .unwrap();
        let start = Instant::now();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        with_notes_worst = with_notes_worst.max(start.elapsed());
    }
    eprintln!("apply one moved symbol, 10k notes (worst of 20): {with_notes_worst:?}");
    let zoom = 1.0;
    let mut j = 0usize;
    let pick = time("pick under the pointer", 200, || {
        j += 1;
        let p = Vec2::new((j % 60) as f32 * 40.0 + 1.0, (j % 160) as f32 * 40.0);
        ps.scene().pick(p, 2.0, zoom)
    });
    let rect = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(600.0, 600.0));
    let boxed = time("box select 15 x 15 symbols", 20, || {
        ps.scene().box_select(&rect, BoxMode::Window, zoom)
    });
    let limit = if cfg!(debug_assertions) { 2000 } else { 50 };
    assert!(edit_worst < Duration::from_millis(if cfg!(debug_assertions) { 1000 } else { 40 }));
    let debug = cfg!(debug_assertions);
    assert!(with_notes_worst < Duration::from_millis(if debug { 1000 } else { 40 }));
    assert!(first_notes < Duration::from_secs(if debug { 30 } else { 2 }));
    assert!(same_notes < Duration::from_millis(if debug { 500 } else { 40 }));
    assert!(pick < Duration::from_millis(if debug { 50 } else { 2 }));
    assert!(boxed < Duration::from_millis(if debug { 100 } else { 5 }));
    assert!(build < Duration::from_secs(20));
    assert!(part < Duration::from_millis(limit), "{part:?}");
}
