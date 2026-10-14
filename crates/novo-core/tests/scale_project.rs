//! A 10k-symbol project, shared by the scale test and the scale benchmark.

use novo_core::*;

pub const PAGES: usize = 100;
pub const SYMBOLS_PER_PAGE: usize = 100;

/// Builds `PAGES` pages of `SYMBOLS_PER_PAGE` two-pin symbols, chained by
/// wires within each page and across pages through interruption points.
/// Every third symbol is a switched pole, and each page starts with a feed
/// of one of three phases.
pub fn build(doc: &mut Document) -> Vec<PotentialId> {
    let (phases, _) = doc
        .transact("Scale project", |tx| {
            let mut phases = Vec::new();
            for name in ["L1", "L2", "L3"] {
                phases.push(tx.add_potential(name, PotentialKind::Line)?);
            }
            for p in 0..PAGES {
                let page = tx.add_page(format!("{}", p + 1))?;
                let feed = tx.place_symbol(
                    page,
                    "iec/connectivity/potential_feed",
                    Placement::at(Point::new(0.0, 0.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::PotentialFeed {
                        potential: phases[p % 3],
                    },
                )?;
                let mut previous = Endpoint::pin(feed, 0);
                for i in 0..SYMBOLS_PER_PAGE {
                    let symbol = tx.place_symbol(
                        page,
                        "iec/contacts/no",
                        Placement::at(Point::new(10.0 * i as f64, 20.0)),
                        vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)],
                        SymbolRole::Unassigned,
                    )?;
                    if i % 3 == 0 {
                        tx.update::<SymbolInstance>(symbol, |s| s.pass_through = vec![vec![0, 1]])?;
                    }
                    tx.connect(previous, Endpoint::pin(symbol, 0))?;
                    previous = Endpoint::pin(symbol, 1);
                }
                let out = tx.place_symbol(
                    page,
                    "iec/connectivity/interruption",
                    Placement::at(Point::new(1000.0, 20.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Interruption {
                        name: format!("P{p}"),
                    },
                )?;
                tx.connect(previous, Endpoint::pin(out, 0))?;
            }
            Ok(phases)
        })
        .unwrap();
    phases
}
