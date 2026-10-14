//! Cross-references: where the parts of a device, and the other ends of an
//! interruption point, are drawn.
//!
//! A location is a page and a grid column, written `page.column` (`3.4`).
//! Everything here is derived from the project; cross-reference text on the
//! drawing (WS-10) is built from these queries, never stored.

use std::fmt;

use crate::entity::{SymbolInstance, SymbolRole};
use crate::id::{DeviceId, PageId, SymbolId};
use crate::project::{Project, natural_cmp};

/// Where a symbol is drawn: its page and the grid column of its origin.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    pub page: PageId,
    /// The page's name, e.g. `3`.
    pub page_name: String,
    /// Grid column, 1-based (see [`PageFormat::columns`](crate::PageFormat::columns)).
    pub column: u16,
}

impl fmt::Display for Location {
    /// `3.4` for page `3`, column 4.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.page_name, self.column)
    }
}

/// One symbol in a cross-reference list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossReference {
    pub symbol: SymbolId,
    /// The device's main symbol (usually the coil), as opposed to its
    /// contacts and other parts.
    pub main: bool,
    pub location: Location,
}

impl Project {
    /// Where a symbol is drawn.
    pub fn location_of(&self, symbol: SymbolId) -> Option<Location> {
        let symbol = self.symbol(symbol)?;
        let page = self.page(symbol.page)?;
        Some(Location {
            page: page.id,
            page_name: page.name.clone(),
            column: page.format.column_at(symbol.placement.origin.x),
        })
    }

    /// Every symbol of a device in reading order (pages in natural order,
    /// then column, then left to right), main symbols included and marked.
    /// A coil shows the entries with `main == false` as its contact image;
    /// each contact points back to the entry with `main == true`.
    pub fn cross_references(&self, device: DeviceId) -> Vec<CrossReference> {
        let symbols = self.symbols_of_device(device);
        self.in_reading_order(symbols)
            .into_iter()
            .filter_map(|symbol| {
                Some(CrossReference {
                    symbol: symbol.id,
                    main: matches!(symbol.role, SymbolRole::Device { main: true, .. }),
                    location: self.location_of(symbol.id)?,
                })
            })
            .collect()
    }

    /// The other interruption points with the same name as this one, in
    /// reading order: where the connection continues. Empty for a symbol that
    /// is not an interruption point.
    pub fn interruption_partners(&self, symbol: SymbolId) -> Vec<CrossReference> {
        let Some(SymbolRole::Interruption { name }) = self.symbol(symbol).map(|s| &s.role) else {
            return Vec::new();
        };
        let partners = self
            .symbols()
            .filter(|s| {
                s.id != symbol
                    && matches!(&s.role, SymbolRole::Interruption { name: n } if n == name)
            })
            .collect();
        self.in_reading_order(partners)
            .into_iter()
            .filter_map(|s| {
                Some(CrossReference {
                    symbol: s.id,
                    main: false,
                    location: self.location_of(s.id)?,
                })
            })
            .collect()
    }

    fn in_reading_order<'a>(
        &self,
        mut symbols: Vec<&'a SymbolInstance>,
    ) -> Vec<&'a SymbolInstance> {
        symbols.sort_by(|a, b| {
            let page_name = |s: &SymbolInstance| self.page(s.page).map_or("", |p| p.name.as_str());
            natural_cmp(page_name(a), page_name(b))
                .then(a.page.cmp(&b.page))
                .then(a.placement.origin.x.total_cmp(&b.placement.origin.x))
                .then(a.placement.origin.y.total_cmp(&b.placement.origin.y))
                .then(a.id.cmp(&b.id))
        });
        symbols
    }
}
