//! The Gate 0 stress page: one page with about 10,000 model objects.

use novo_core::{
    DeviceTag, Document, Endpoint, PageId, Pin, Placement, Point, Result, SymbolId, SymbolRole,
};

/// Builds one page holding at least `objects` model objects (devices, placed
/// symbols and wires, about a third each) as one undo step. Symbols sit on a
/// 100-column grid, wired in one long chain.
pub fn stress_page(doc: &mut Document, objects: usize) -> Result<PageId> {
    let cells = objects.div_ceil(3);
    let (page, _) = doc.transact("Stress page", |tx| {
        let page = tx.add_page("stress")?;
        let mut previous: Option<SymbolId> = None;
        for i in 0..cells {
            let device = tx.add_device(DeviceTag::parse(&format!("-K{}", i + 1))?, "")?;
            let (col, row) = (i % 100, i / 100);
            let symbol = tx.place_symbol(
                page,
                "iec/contacts/no",
                Placement::at(Point::new(col as f64 * 8.0, row as f64 * 24.0)),
                vec![Pin::new("13", 0.0, 0.0), Pin::new("14", 0.0, 16.0)],
                SymbolRole::Device { device, main: true },
            )?;
            if let Some(left) = previous {
                tx.connect(Endpoint::pin(left, 1), Endpoint::pin(symbol, 0))?;
            }
            previous = Some(symbol);
        }
        Ok(page)
    })?;
    Ok(page)
}
