//! Delimited text, written with quotes where needed and read back with
//! the delimiter found in the first line.

/// One line of fields.
pub fn line(fields: &[&str], delimiter: char) -> String {
    let mut out = String::new();
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push(delimiter);
        }
        if f.contains([delimiter, '"', '\n', '\r']) {
            out.push('"');
            out.push_str(&f.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(f);
        }
    }
    out.push_str("\r\n");
    out
}

/// A field that is always quoted (Studio 5000 quotes descriptions).
pub fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// Reads delimited text into rows of fields. The delimiter is the one of
/// `,` `;` and tab that the first non-empty line has most of. A byte order
/// mark is ignored.
pub fn rows(text: &str) -> Vec<Vec<String>> {
    let mut out = rows_with_blanks(text);
    out.retain(|r| r.iter().any(|f| !f.trim().is_empty()));
    out
}

/// Like [`rows`], but empty lines stay, so a row's index is its line in the file.
pub fn rows_with_blanks(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let delimiter = [',', ';', '\t']
        .into_iter()
        .max_by_key(|d| first.matches(*d).count())
        .unwrap_or(',');
    let mut out = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        any = true;
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => in_quotes = true,
            c if c == delimiter => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                out.push(std::mem::take(&mut row));
                any = false;
            }
            c => field.push(c),
        }
    }
    if any || !field.is_empty() {
        row.push(field);
        out.push(row);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_awkward_fields() {
        let text = line(&["a;b", "say \"hi\"", "two\nlines", "plain"], ';');
        let read = rows(&text);
        assert_eq!(read, vec![vec!["a;b", "say \"hi\"", "two\nlines", "plain"]]);
    }

    #[test]
    fn finds_the_delimiter() {
        assert_eq!(
            rows("\u{feff}a;b;c\r\n1;2;3"),
            vec![vec!["a", "b", "c"], vec!["1", "2", "3"]]
        );
        assert_eq!(rows("a\tb\n1\t2\n\n"), vec![vec!["a", "b"], vec!["1", "2"]]);
    }
}
