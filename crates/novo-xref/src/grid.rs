//! Page columns and the locations cross references print (`/3.4`).

use std::cmp::Ordering;
use std::fmt;

use novo_core::{Page, PageId, Value};

/// Page property overriding the number of columns (an integer).
pub const COLUMNS_PROPERTY: &str = "xref.columns";
/// Page property: left margin before column 1, in millimetres.
pub const MARGIN_LEFT_PROPERTY: &str = "xref.margin_left_mm";
/// Page property: right margin after the last column, in millimetres.
pub const MARGIN_RIGHT_PROPERTY: &str = "xref.margin_right_mm";
/// Columns per page when the page does not say otherwise.
pub const DEFAULT_COLUMNS: u32 = 10;

/// How a page is divided into numbered columns, left to right from 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGrid {
    /// Page width in millimetres.
    pub width_mm: f64,
    pub columns: u32,
    pub margin_left_mm: f64,
    pub margin_right_mm: f64,
}

impl PageGrid {
    /// The grid of a page: its paper width, [`DEFAULT_COLUMNS`] columns and
    /// no margins, unless the page's `xref.*` properties say otherwise.
    pub fn of(page: &Page) -> Self {
        let (width_mm, _) = page.format.size_mm();
        let number = |key: &str| match page.properties.get(key) {
            Some(Value::Int(v)) => Some(*v as f64),
            Some(Value::Float(v)) => Some(*v),
            _ => None,
        };
        let columns = number(COLUMNS_PROPERTY)
            .filter(|c| *c >= 1.0)
            .map_or(DEFAULT_COLUMNS, |c| c.min(f64::from(u16::MAX)) as u32);
        PageGrid {
            width_mm,
            columns,
            margin_left_mm: number(MARGIN_LEFT_PROPERTY).unwrap_or(0.0).max(0.0),
            margin_right_mm: number(MARGIN_RIGHT_PROPERTY).unwrap_or(0.0).max(0.0),
        }
    }

    /// The column holding page x coordinate `x`, clamped to the page.
    pub fn column_at(&self, x: f64) -> u32 {
        let inner = self.width_mm - self.margin_left_mm - self.margin_right_mm;
        if inner <= 0.0 || !x.is_finite() {
            return 1;
        }
        let width = inner / f64::from(self.columns);
        let column = ((x - self.margin_left_mm) / width).floor() + 1.0;
        column.clamp(1.0, f64::from(self.columns)) as u32
    }
}

/// Where something is drawn: page and column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    pub page: PageId,
    /// The page's name, e.g. `3` or `3.1`.
    pub page_name: String,
    pub column: u32,
    /// Position of the page in reading order, for sorting.
    pub page_rank: usize,
}

impl Location {
    /// Reading order: page, then column.
    pub fn reading_cmp(&self, other: &Location) -> Ordering {
        self.page_rank
            .cmp(&other.page_rank)
            .then(self.column.cmp(&other.column))
    }

    /// The reference text, `/3.4`.
    pub fn reference(&self) -> String {
        format!("/{self}")
    }
}

/// `3.4`: page name, a dot, column.
impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.page_name, self.column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{PageFormat, PageKind, PaperSize, Properties};

    fn page(props: &[(&str, Value)]) -> Page {
        Page {
            id: PageId::from(1),
            name: "1".into(),
            title: String::new(),
            kind: PageKind::Schematic,
            format: PageFormat {
                size: PaperSize::A3,
                landscape: true,
            },
            properties: props
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect::<Properties>(),
        }
    }

    #[test]
    fn a3_landscape_has_ten_42mm_columns() {
        let grid = PageGrid::of(&page(&[]));
        assert_eq!(grid.width_mm, 420.0);
        assert_eq!(grid.column_at(0.0), 1);
        assert_eq!(grid.column_at(41.9), 1);
        assert_eq!(grid.column_at(42.0), 2);
        assert_eq!(grid.column_at(419.0), 10);
        assert_eq!(grid.column_at(-5.0), 1);
        assert_eq!(grid.column_at(900.0), 10);
    }

    #[test]
    fn page_properties_set_columns_and_margins() {
        let grid = PageGrid::of(&page(&[
            (COLUMNS_PROPERTY, Value::Int(8)),
            (MARGIN_LEFT_PROPERTY, Value::Float(20.0)),
            (MARGIN_RIGHT_PROPERTY, Value::Int(0)),
        ]));
        // 400 mm over 8 columns: 50 mm each, starting at x = 20.
        assert_eq!(grid.column_at(19.0), 1);
        assert_eq!(grid.column_at(69.0), 1);
        assert_eq!(grid.column_at(70.0), 2);
        assert_eq!(grid.column_at(419.0), 8);
    }

    #[test]
    fn location_text() {
        let loc = Location {
            page: PageId::from(1),
            page_name: "3".into(),
            column: 4,
            page_rank: 0,
        };
        assert_eq!(loc.to_string(), "3.4");
        assert_eq!(loc.reference(), "/3.4");
    }
}
