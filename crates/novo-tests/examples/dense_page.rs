//! Writes a project of many symbols on few or many pages, to open in the
//! app and time (Commissioning, the idle frame, edits). The Windows hardware
//! test used 5000 symbols on one page (HPR-05) and 1000 pages of 50 symbols
//! with a wire to nearly every symbol (HPR-04).
//!
//! ```text
//! cargo run --release -p novo-tests --example dense_page -- target/dense5000.novo 5000
//! cargo run --release -p novo-tests --example dense_page -- target/p1000.novo 50000 1000
//! ```

use novo_core::{Document, Endpoint, Pin, Placement, Point, PotentialKind, Result, SymbolRole, Tx};

fn build(tx: &mut Tx<'_>, count: usize, pages: usize) -> Result<()> {
    tx.set_info(|i| i.name = format!("Dense project, {count} symbols on {pages} pages"))?;
    let potential = tx.add_potential("L1", PotentialKind::Line)?;
    let pages: Vec<_> = (1..=pages)
        .map(|n| tx.add_page(format!("{n}")))
        .collect::<Result<_>>()?;
    let per_page = count.div_ceil(pages.len());
    let strip = tx.add_terminal_strip(novo_core::DeviceTag::parse("-X1").expect("tag"))?;
    let columns = 80usize;
    let mut previous = None;
    for n in 0..count {
        let page = pages[n / per_page];
        let k = n % per_page;
        let (x, y) = (
            20.0 + (k % columns) as f64 * 12.0,
            20.0 + (k / columns) as f64 * 14.0,
        );
        let symbol = if n % 5 == 0 {
            tx.place_symbol(
                page,
                "iec/connectivity/potential_feed",
                Placement::at(Point::new(x, y)),
                vec![Pin::new("1", 0.0, 8.0)],
                SymbolRole::PotentialFeed { potential },
            )?
        } else {
            let terminal = tx.add_terminal(strip, format!("{n}"))?;
            tx.place_symbol(
                page,
                "iec/terminals/terminal",
                Placement::at(Point::new(x, y)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                SymbolRole::Terminal {
                    terminals: vec![terminal],
                },
            )?
        };
        if k > 0
            && let Some(prev) = previous
        {
            tx.connect(Endpoint::pin(prev, 0), Endpoint::pin(symbol, 0))?;
        }
        previous = Some(symbol);
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).expect("usage: dense_page FILE [COUNT]");
    let count: usize = args.get(2).map_or(5000, |c| c.parse().expect("COUNT"));
    let mut doc = Document::default();
    let pages: usize = args.get(3).map_or(1, |c| c.parse().expect("PAGES"));
    doc.transact("Dense page", |tx| build(tx, count, pages))
        .expect("build");
    let _ = std::fs::remove_file(out);
    novo_storage::ProjectFile::create(std::path::Path::new(out), doc.project()).expect("save");
    eprintln!("wrote {out}: {count} symbols on {pages} pages");
}
