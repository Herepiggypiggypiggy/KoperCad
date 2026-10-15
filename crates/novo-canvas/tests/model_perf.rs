//! CPU timings of the model adapter on a big page: 3,000 symbols, each with
//! a bent wire. Run with
//! `cargo test --release -p novo-canvas --test model_perf -- --nocapture`;
//! in debug builds the bounds are deliberately loose.

use novo_canvas::Vec2;
use novo_canvas::model::{PageScene, PlaceholderArt};
use novo_core::{DeviceTag, Document, Endpoint, Pin, Placement, Point, SymbolRole};
use std::time::{Duration, Instant};

const SYMBOLS: usize = 3_000;

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
    let limit = if cfg!(debug_assertions) { 2000 } else { 50 };
    assert!(edit_worst < Duration::from_millis(if cfg!(debug_assertions) { 500 } else { 20 }));
    assert!(build < Duration::from_secs(20));
    assert!(part < Duration::from_millis(limit), "{part:?}");
}
