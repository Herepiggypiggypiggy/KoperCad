//! Values a query works with, and how they compare.

use std::cmp::Ordering;

use novo_core::{Value, natural_cmp};

use crate::table::Cell;

/// One value in a query row.
#[derive(Debug, Clone, PartialEq)]
pub enum QValue {
    /// Nothing there: an empty cell, a missing size.
    Null,
    Bool(bool),
    Int(i64),
    Num(f64),
    Text(String),
}

impl QValue {
    pub fn is_null(&self) -> bool {
        matches!(self, QValue::Null)
    }

    pub(crate) fn text(text: impl Into<String>) -> QValue {
        let text = text.into();
        if text.is_empty() {
            QValue::Null
        } else {
            QValue::Text(text)
        }
    }

    /// The value as a number, when it is one or reads as one.
    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self {
            QValue::Int(i) => Some(*i as f64),
            QValue::Num(n) => Some(*n),
            QValue::Bool(b) => Some(f64::from(u8::from(*b))),
            QValue::Text(t) => t.trim().replace(',', ".").parse().ok(),
            QValue::Null => None,
        }
    }

    /// Truth for `WHERE`: only a real `true` passes (a `NULL` does not).
    pub(crate) fn truthy(&self) -> bool {
        match self {
            QValue::Bool(b) => *b,
            QValue::Int(i) => *i != 0,
            QValue::Num(n) => *n != 0.0,
            QValue::Text(t) => !t.is_empty(),
            QValue::Null => false,
        }
    }

    /// The cell a report table shows for this value.
    pub fn to_cell(&self) -> Cell {
        match self {
            QValue::Null => Cell::Empty,
            QValue::Bool(b) => Cell::Text(if *b { "yes" } else { "no" }.into()),
            QValue::Int(i) => Cell::Int(*i),
            QValue::Num(n) => Cell::Number(*n),
            QValue::Text(t) => Cell::text(t.clone()),
        }
    }

    /// The text of the value (`NULL` is empty).
    pub fn display(&self) -> String {
        self.to_cell().to_string()
    }
}

impl From<Cell> for QValue {
    fn from(cell: Cell) -> Self {
        match cell {
            Cell::Empty => QValue::Null,
            Cell::Text(t) => QValue::text(t),
            Cell::Int(i) => QValue::Int(i),
            Cell::Number(n) => QValue::Num(n),
        }
    }
}

impl From<&Value> for QValue {
    fn from(value: &Value) -> Self {
        match value {
            Value::Bool(b) => QValue::Bool(*b),
            Value::Int(i) => QValue::Int(*i),
            Value::Float(f) => QValue::Num(*f),
            Value::Text(t) => QValue::text(t.clone()),
        }
    }
}

impl From<usize> for QValue {
    fn from(n: usize) -> Self {
        QValue::Int(i64::try_from(n).unwrap_or(i64::MAX))
    }
}

/// Orders two values. Numbers compare by value (a text that reads as a
/// number counts as one), other text compares "naturally" and without
/// regard to case, so `K2` comes before `K10`. `None` when either is `NULL`.
pub(crate) fn compare(a: &QValue, b: &QValue) -> Option<Ordering> {
    if a.is_null() || b.is_null() {
        return None;
    }
    let numeric = |v: &QValue| !matches!(v, QValue::Text(_));
    if numeric(a) || numeric(b) {
        if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) {
            return x.partial_cmp(&y);
        }
    } else if let (QValue::Text(x), QValue::Text(y)) = (a, b) {
        // Two texts that both read as numbers are numbers (`10` > `9`).
        if let (Some(p), Some(q)) = (a.as_f64(), b.as_f64())
            && x.trim().chars().next().is_some_and(|c| c.is_ascii_digit())
            && y.trim().chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            return p.partial_cmp(&q);
        }
        return Some(natural_cmp(&x.to_lowercase(), &y.to_lowercase()));
    }
    Some(natural_cmp(
        &a.display().to_lowercase(),
        &b.display().to_lowercase(),
    ))
}

/// `LIKE`: `%` matches any run, `_` any one character, case does not matter.
pub(crate) fn like(text: &str, pattern: &str) -> bool {
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let (mut i, mut j) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while i < t.len() {
        if j < p.len() && (p[j] == '_' || p[j] == t[i]) && p[j] != '%' {
            i += 1;
            j += 1;
        } else if j < p.len() && p[j] == '%' {
            star = Some((j, i));
            j += 1;
        } else if let Some((sj, si)) = star {
            j = sj + 1;
            i = si + 1;
            star = Some((sj, si + 1));
        } else {
            return false;
        }
    }
    while j < p.len() && p[j] == '%' {
        j += 1;
    }
    j == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_matches_wildcards() {
        assert!(like("-K10", "-k%"));
        assert!(like("-K10", "%10"));
        assert!(like("-K10", "-K_0"));
        assert!(!like("-K10", "-Q%"));
        assert!(like("abc", "%"));
        assert!(like("", "%"));
        assert!(!like("", "_"));
    }

    #[test]
    fn numbers_and_tags_order_naturally() {
        let t = |s: &str| QValue::Text(s.into());
        assert_eq!(compare(&t("-K2"), &t("-K10")), Some(Ordering::Less));
        assert_eq!(compare(&t("10"), &t("9")), Some(Ordering::Greater));
        assert_eq!(compare(&QValue::Int(2), &t("10")), Some(Ordering::Less));
        assert_eq!(compare(&QValue::Null, &QValue::Int(1)), None);
    }
}
