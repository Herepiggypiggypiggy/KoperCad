//! Building a query by picking: a table, columns, filters and a sort. The
//! result is plain query text, so the person can read what was built, change
//! it by hand and keep it as a preset.

use super::parse::parse;

/// How a filter compares a column with a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    Equals,
    NotEquals,
    Contains,
    StartsWith,
    Greater,
    Less,
    /// The column has nothing in it (the value is ignored).
    Empty,
    /// The column has something in it (the value is ignored).
    NotEmpty,
}

impl FilterOp {
    pub const ALL: [FilterOp; 8] = [
        FilterOp::Equals,
        FilterOp::NotEquals,
        FilterOp::Contains,
        FilterOp::StartsWith,
        FilterOp::Greater,
        FilterOp::Less,
        FilterOp::Empty,
        FilterOp::NotEmpty,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FilterOp::Equals => "is",
            FilterOp::NotEquals => "is not",
            FilterOp::Contains => "contains",
            FilterOp::StartsWith => "starts with",
            FilterOp::Greater => "is more than",
            FilterOp::Less => "is less than",
            FilterOp::Empty => "is empty",
            FilterOp::NotEmpty => "is not empty",
        }
    }

    /// Whether the filter needs a value typed.
    pub fn takes_value(self) -> bool {
        !matches!(self, FilterOp::Empty | FilterOp::NotEmpty)
    }
}

/// One condition on a column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub column: String,
    pub op: FilterOp,
    pub value: String,
}

/// A query put together from choices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Builder {
    pub table: String,
    /// The columns to show; empty means all of them.
    pub columns: Vec<String>,
    pub filters: Vec<Filter>,
    /// A row must meet every filter (`true`) or any one of them.
    pub match_all: bool,
    /// Column and direction (`true` is descending).
    pub sort: Vec<(String, bool)>,
}

impl Builder {
    pub fn new(table: &str) -> Self {
        Builder {
            table: table.to_owned(),
            columns: Vec::new(),
            filters: Vec::new(),
            match_all: true,
            sort: Vec::new(),
        }
    }

    /// The query text for these choices.
    pub fn to_sql(&self) -> String {
        let columns = if self.columns.is_empty() {
            "*".to_owned()
        } else {
            self.columns
                .iter()
                .map(|c| name(c))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut sql = format!("SELECT {columns}\nFROM {}", name(&self.table));
        let conditions: Vec<String> = self.filters.iter().map(condition).collect();
        if !conditions.is_empty() {
            let glue = if self.match_all { " AND " } else { " OR " };
            sql.push_str(&format!("\nWHERE {}", conditions.join(glue)));
        }
        if !self.sort.is_empty() {
            let keys: Vec<String> = self
                .sort
                .iter()
                .map(|(c, desc)| format!("{}{}", name(c), if *desc { " DESC" } else { "" }))
                .collect();
            sql.push_str(&format!("\nORDER BY {}", keys.join(", ")));
        }
        sql
    }
}

/// A column or table name, in double quotes when it needs them.
fn name(text: &str) -> String {
    let plain = !text.is_empty()
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !text.starts_with(|c: char| c.is_ascii_digit());
    // A word the language reserves must be quoted; ask the parser.
    if plain && parse(&format!("SELECT {text} FROM t")).is_ok() {
        text.to_owned()
    } else {
        format!("\"{}\"", text.replace('"', "\"\""))
    }
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn condition(f: &Filter) -> String {
    let c = name(&f.column);
    match f.op {
        FilterOp::Equals => format!("{c} = {}", quote(&f.value)),
        FilterOp::NotEquals => format!("{c} != {}", quote(&f.value)),
        FilterOp::Contains => format!("{c} LIKE {}", quote(&format!("%{}%", f.value))),
        FilterOp::StartsWith => format!("{c} LIKE {}", quote(&format!("{}%", f.value))),
        FilterOp::Greater => format!("{c} > {}", quote(&f.value)),
        FilterOp::Less => format!("{c} < {}", quote(&f.value)),
        FilterOp::Empty => format!("{c} IS NULL"),
        FilterOp::NotEmpty => format!("{c} IS NOT NULL"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_text_parses_and_reads_back() {
        let mut b = Builder::new("wires");
        b.columns = vec!["number".into(), "from".into(), "to".into()];
        b.filters = vec![
            Filter {
                column: "color".into(),
                op: FilterOp::Equals,
                value: "it's".into(),
            },
            Filter {
                column: "cross_section_mm2".into(),
                op: FilterOp::Empty,
                value: String::new(),
            },
        ];
        b.sort = vec![("number".into(), true)];
        let sql = b.to_sql();
        assert!(sql.contains("WHERE color = 'it''s' AND cross_section_mm2 IS NULL"));
        assert!(sql.contains("ORDER BY number DESC"));
        // `from` and `to` are fine as words once quoted where reserved.
        assert!(sql.contains("\"from\""), "{sql}");
        parse(&sql).unwrap();
    }

    #[test]
    fn empty_choices_select_everything() {
        assert_eq!(Builder::new("parts").to_sql(), "SELECT *\nFROM parts");
    }
}
