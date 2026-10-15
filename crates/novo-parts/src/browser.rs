//! Model of the part browser: a list of parts with one button each.
//!
//! No layout here (the screen design comes from `docs/DESIGN.md`); the app
//! draws [`BrowserRow`]s and starts an [`crate::ImportJob`] when the user
//! clicks a row's button.

use crate::catalog::{Catalog, CatalogPart};
use crate::import::SourceTable;
use crate::library::LocalLibrary;
use crate::search::Matcher;

/// What a row's button says and does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowAction {
    /// Files are stored: show "In library", offer Remove.
    InLibrary,
    /// One click fetches the files.
    Import,
    /// Fetching is off for this maker: open the page, then the Downloads
    /// watch imports the file. `None` when the app has no page for the part
    /// either; the user drops a file instead.
    OpenPage(Option<String>),
}

#[derive(Debug, Clone)]
pub struct BrowserRow<'a> {
    pub part: &'a CatalogPart,
    pub action: RowAction,
}

/// The rows for a search text (empty shows everything, in catalog order).
pub fn browser_rows<'a>(
    catalog: &'a Catalog,
    library: &LocalLibrary,
    sources: &SourceTable,
    query: &str,
) -> Vec<BrowserRow<'a>> {
    let matcher = Matcher::new(query);
    catalog
        .parts()
        .iter()
        .filter(|p| {
            matcher.is_empty()
                || matcher.matches([
                    p.manufacturer.as_str(),
                    p.order_number.as_str(),
                    p.type_number.as_str(),
                    p.description.as_str(),
                ])
        })
        .map(|part| {
            let action = if library.has(&part.key()) {
                RowAction::InLibrary
            } else {
                match sources.get(&part.manufacturer) {
                    Some(s) if s.fetch_enabled => RowAction::Import,
                    Some(s) => RowAction::OpenPage(s.page_for(&part.order_number)),
                    None => RowAction::OpenPage(None),
                }
            };
            BrowserRow { part, action }
        })
        .collect()
}
