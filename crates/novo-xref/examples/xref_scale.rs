//! Times the cross-reference index on 10,000 symbols:
//! `cargo run --release -p novo-xref --example xref_scale`.

use std::time::Instant;

use novo_core::{DeviceTag, Document, PageId, Pin, Placement, Point, SymbolRole};
use novo_xref::{FromKey, XrefIndex};

fn main() -> novo_core::Result<()> {
    let mut doc = Document::default();
    doc.transact("Big", |tx| {
        let pages: Vec<PageId> = (1..=200)
            .map(|n| tx.add_page(n.to_string()))
            .collect::<Result<_, _>>()?;
        for d in 0..2000usize {
            let device = tx.add_device(DeviceTag::product(format!("K{}", d + 1)), "")?;
            for c in 0..5usize {
                let (key, pins) = if c == 0 {
                    ("iec/coils/relay", ["A1".to_owned(), "A2".to_owned()])
                } else {
                    ("iec/contacts/no", [format!("{c}3"), format!("{c}4")])
                };
                tx.place_symbol(
                    pages[(d * 7 + c * 13) % 200],
                    key,
                    Placement::at(Point::new(40.0 * c as f64, 100.0)),
                    pins.iter()
                        .map(|p| Pin::new(p.as_str(), 0.0, 0.0))
                        .collect(),
                    SymbolRole::Device {
                        device,
                        main: c == 0,
                    },
                )?;
            }
        }
        Ok(())
    })?;
    let project = doc.project();
    let start = Instant::now();
    let n = project.symbols().count();
    println!("iterate {n} symbols: {:?}", start.elapsed());
    let runs: u32 = std::env::var("RUNS")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(20);
    let start = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(XrefIndex::build(project, &FromKey));
    }
    println!("build index: {:?} per run", start.elapsed() / runs);
    Ok(())
}
