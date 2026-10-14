//! Where things are: page and column of every symbol of a device, in the
//! `/3.4` form cross references use (page 3, column 4).
//!
//! Schematic pages are divided into equal-width columns across the page,
//! numbered from 1 at the left. The count comes from the page property
//! `columns` and defaults to [`DEFAULT_COLUMNS`].

use std::cmp::Ordering;
use std::fmt;

use novo_core::{
    DeviceId, DeviceTag, Page, PageId, Project, SymbolId, SymbolInstance, SymbolRole, Value,
    natural_cmp,
};

/// Columns on a page without a `columns` property, as on a typical EPLAN
/// A3 frame.
pub const DEFAULT_COLUMNS: u32 = 10;

/// Page property holding the number of columns.
pub const COLUMNS_PROPERTY: &str = "columns";

/// One place a device is drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub symbol: SymbolId,
    pub page: PageId,
    pub page_name: String,
    /// 1-based column on the page.
    pub column: u32,
    /// The device's main function (the coil of a contactor).
    pub main: bool,
}

impl Location {
    /// `/3.4`.
    pub fn reference(&self) -> String {
        format!("/{}.{}", self.page_name, self.column)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reference())
    }
}

/// Number of columns on a page.
pub fn columns(page: &Page) -> u32 {
    match page.properties.get(COLUMNS_PROPERTY) {
        Some(Value::Int(n)) if *n > 0 => u32::try_from(*n).unwrap_or(DEFAULT_COLUMNS),
        _ => DEFAULT_COLUMNS,
    }
}

/// The 1-based column of an x position on a page, clamped to the page.
pub fn column_at(page: &Page, x: f64) -> u32 {
    let count = columns(page);
    let (width, _) = page.format.size_mm();
    if width <= 0.0 {
        return 1;
    }
    let raw = (x / width * f64::from(count)).floor();
    // Clamp before converting; the cast is then exact.
    let clamped = raw.clamp(0.0, f64::from(count - 1));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let index = clamped as u32;
    index + 1
}

/// Where a placed symbol is.
pub fn locate_symbol(project: &Project, symbol: &SymbolInstance) -> Option<Location> {
    let page = project.page(symbol.page)?;
    Some(Location {
        symbol: symbol.id,
        page: page.id,
        page_name: page.name.clone(),
        column: column_at(page, symbol.placement.origin.x),
        main: matches!(symbol.role, SymbolRole::Device { main: true, .. }),
    })
}

/// Every place a device is drawn: main function first, then by page (in
/// natural order) and column.
pub fn locations(project: &Project, device: DeviceId) -> Vec<Location> {
    let mut out: Vec<Location> = project
        .symbols_of_device(device)
        .into_iter()
        .filter_map(|s| locate_symbol(project, s))
        .collect();
    out.sort_by(|a, b| {
        b.main
            .cmp(&a.main)
            .then_with(|| page_order(&a.page_name, &b.page_name))
            .then(a.column.cmp(&b.column))
            .then(a.symbol.cmp(&b.symbol))
    });
    out
}

fn page_order(a: &str, b: &str) -> Ordering {
    natural_cmp(a, b)
}

/// Devices a typed tag names. `-K1`, `K1` and `=A1+CAB1-K1` all work:
/// a full tag must match exactly; a bare product code (`K1`, `-K1`)
/// matches every device whose product aspect is that code, whatever its
/// function and location.
pub fn devices_named(project: &Project, text: &str) -> Vec<DeviceId> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let parsed = if text.starts_with(['=', '+', '-']) {
        DeviceTag::parse(text)
    } else {
        DeviceTag::parse(&format!("-{text}"))
    };
    let Ok(tag) = parsed else {
        return Vec::new();
    };
    if tag.product.is_empty() {
        return Vec::new();
    }
    let bare = tag.function.is_empty() && tag.location.is_empty();
    let mut out: Vec<(&DeviceTag, DeviceId)> = project
        .devices()
        .filter(|d| {
            if bare {
                d.tag
                    .product
                    .iter()
                    .map(|s| s.to_ascii_uppercase())
                    .eq(tag.product.iter().map(|s| s.to_ascii_uppercase()))
            } else {
                d.tag.to_string().eq_ignore_ascii_case(&tag.to_string())
            }
        })
        .map(|d| (&d.tag, d.id))
        .collect();
    out.sort();
    out.into_iter().map(|(_, id)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{Placement, Point};

    #[test]
    fn columns_split_the_page_width() {
        let mut page = Page {
            id: PageId::from(1),
            name: "3".into(),
            title: String::new(),
            kind: Default::default(),
            format: Default::default(),
            properties: Default::default(),
        };
        // A3 landscape is 420 mm wide: ten columns of 42 mm.
        assert_eq!(column_at(&page, 0.0), 1);
        assert_eq!(column_at(&page, 41.9), 1);
        assert_eq!(column_at(&page, 42.0), 2);
        assert_eq!(column_at(&page, 419.0), 10);
        assert_eq!(column_at(&page, 999.0), 10);
        assert_eq!(column_at(&page, -5.0), 1);
        page.properties
            .insert(COLUMNS_PROPERTY.into(), Value::Int(8));
        assert_eq!(column_at(&page, 419.0), 8);
    }

    #[test]
    fn reference_text() {
        let loc = Location {
            symbol: SymbolId::from(7),
            page: PageId::from(1),
            page_name: "3".into(),
            column: 4,
            main: false,
        };
        assert_eq!(loc.reference(), "/3.4");
        let _ = Placement::at(Point::new(0.0, 0.0));
    }
}
