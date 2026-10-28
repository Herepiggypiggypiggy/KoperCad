//! Reading a pasted or typed I/O table: terminal label, function text and an optional step.

/// Most rows one table keeps. An I/O page has tens of rows; a paste of a whole workbook must
/// not freeze the window (the proposal grows faster than linearly with the row count).
pub const MAX_ROWS: usize = 2000;

/// One row of the table, as the person wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    /// Line in the pasted text, counted from 1 (0 when the row came from somewhere else).
    pub line: usize,
    /// Terminal or pin label, for example `13`, `X1:3` or `COM`.
    pub terminal: String,
    /// Function text, for example `Hoist up 1S`.
    pub function: String,
    /// Content of the step column, empty when there is none.
    pub step: String,
    /// What the pin is wired to in a drawing (from a device), empty when unknown.
    pub target: String,
}

/// Which columns of a pasted grid hold what.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Columns {
    pub terminal: Option<usize>,
    pub function: Option<usize>,
    pub step: Option<usize>,
    pub target: Option<usize>,
}

/// A table read from text, with what was guessed on the way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    pub rows: Vec<Row>,
    pub columns: Columns,
    /// True when the first line was a header and was not read as a row.
    pub had_header: bool,
    /// Plain-language notes on the guesses (delimiter, columns, header).
    pub notes: Vec<String>,
}

const TERMINAL_HEADS: &[&str] = &[
    "terminal",
    "terminals",
    "klem",
    "klemmen",
    "pin",
    "pins",
    "aansluiting",
    "connection",
    "nr",
    "no",
    "label",
    "tag",
    "adres",
    "address",
];
const FUNCTION_HEADS: &[&str] = &[
    "function",
    "functie",
    "omschrijving",
    "description",
    "text",
    "tekst",
    "name",
    "naam",
    "signal",
    "signaal",
    "betekenis",
    "meaning",
];
const TARGET_HEADS: &[&str] = &["target", "doel", "wired to", "bestemming", "naar", "to"];
const STEP_HEADS: &[&str] = &[
    "step", "stap", "stand", "speed", "snelheid", "notch", "stage",
];

/// Number in a step cell or step word: `1`, `2S`, `S2`, `1e`, `2nd`, roman `II`.
pub fn parse_step_cell(cell: &str) -> Option<u8> {
    let c = cell.trim().to_lowercase();
    let c = c.trim_matches(|ch: char| ch == '.' || ch == '(' || ch == ')');
    match c {
        "i" => return Some(1),
        "ii" => return Some(2),
        "iii" => return Some(3),
        _ => {}
    }
    let digits: String = c.chars().filter(char::is_ascii_digit).collect();
    let rest: String = c.chars().filter(|ch| !ch.is_ascii_digit()).collect();
    let rest_ok = matches!(
        rest.as_str(),
        "" | "s" | "e" | "de" | "ste" | "st" | "nd" | "rd" | "th" | "stap" | "step"
    );
    if digits.len() == 1 && rest_ok {
        let n: u8 = digits.parse().ok()?;
        (1..=5).contains(&n).then_some(n)
    } else {
        None
    }
}

/// Splits one line into cells with the delimiter; double quotes keep a delimiter inside a cell.
fn split_delimited(line: &str, delim: char) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c == delim && !quoted => cells.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    cells.push(cur);
    cells.into_iter().map(|c| c.trim().to_owned()).collect()
}

fn split_wide(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut spaces = 0;
    for c in line.trim().chars() {
        if c == ' ' {
            spaces += 1;
            continue;
        }
        if spaces >= 2 && !cur.is_empty() {
            cells.push(std::mem::take(&mut cur));
        } else if spaces == 1 {
            cur.push(' ');
        }
        spaces = 0;
        cur.push(c);
    }
    cells.push(cur);
    cells
}

/// A short label with a digit or a bare common/earth name: what a terminal column looks like.
fn looks_like_terminal(cell: &str) -> bool {
    let c = cell.trim();
    if c.is_empty() || c.len() > 12 || c.contains(' ') {
        return false;
    }
    c.chars().any(|ch| ch.is_ascii_digit())
        || matches!(
            c.to_lowercase().as_str(),
            "com" | "common" | "pe" | "n" | "l" | "c"
        )
}

fn head_kind(cell: &str) -> Option<u8> {
    let c = cell.trim().to_lowercase();
    let c = c.trim_end_matches('.');
    if TERMINAL_HEADS.contains(&c) {
        Some(0)
    } else if FUNCTION_HEADS.contains(&c) {
        Some(1)
    } else if STEP_HEADS.contains(&c) {
        Some(2)
    } else if TARGET_HEADS.contains(&c) {
        Some(3)
    } else {
        None
    }
}

impl Table {
    /// Reads pasted text from Excel (tabs), a CSV file (`;` or `,`), a pipe table or columns
    /// separated by several spaces. Blank lines and lines starting with `#` are skipped.
    pub fn from_text(text: &str) -> Table {
        let lines: Vec<(usize, &str)> = text
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l))
            .filter(|(_, l)| {
                let t = l.trim();
                !t.is_empty()
                    && !t.starts_with('#')
                    && !t.chars().all(|c| matches!(c, '-' | '|' | '+' | ':' | ' '))
            })
            .collect();
        let mut notes = Vec::new();
        let delim = detect_delimiter(&lines);
        notes.push(match delim {
            Delim::Char('\t') => "Columns separated by tabs (pasted from a spreadsheet)".to_owned(),
            Delim::Char(c) => format!("Columns separated by `{c}`"),
            Delim::Wide => "Columns separated by two or more spaces".to_owned(),
            Delim::First => {
                "No column separator found: the first word is the terminal, the rest the function"
                    .to_owned()
            }
        });
        let grid: Vec<(usize, Vec<String>)> = lines
            .iter()
            .map(|&(n, l)| {
                let cells = match delim {
                    Delim::Char('|') => split_delimited(l.trim().trim_matches('|'), '|'),
                    Delim::Char(c) => split_delimited(l, c),
                    Delim::Wide => split_wide(l),
                    Delim::First => {
                        let l = l.trim();
                        match l.split_once(char::is_whitespace) {
                            Some((a, b)) => vec![a.to_owned(), b.trim().to_owned()],
                            None => vec![l.to_owned()],
                        }
                    }
                };
                (n, cells)
            })
            .collect();
        let mut table = Table::from_grid(&grid);
        notes.append(&mut table.notes);
        table.notes = notes;
        table
    }

    /// Reads a grid of cells (a spreadsheet range) given with their line numbers.
    pub fn from_grid(grid: &[(usize, Vec<String>)]) -> Table {
        let mut table = Table::default();
        let mut body = grid;
        if let Some((_, first)) = grid.first() {
            let kinds: Vec<Option<u8>> = first.iter().map(|c| head_kind(c)).collect();
            if kinds.iter().flatten().count() >= 2
                || (kinds.iter().flatten().count() == 1 && first.len() == 1)
            {
                for (i, k) in kinds.iter().enumerate() {
                    match k {
                        Some(0) if table.columns.terminal.is_none() => {
                            table.columns.terminal = Some(i)
                        }
                        Some(1) if table.columns.function.is_none() => {
                            table.columns.function = Some(i)
                        }
                        Some(2) if table.columns.step.is_none() => table.columns.step = Some(i),
                        Some(3) if table.columns.target.is_none() => table.columns.target = Some(i),
                        _ => {}
                    }
                }
                table.had_header = true;
                body = &grid[1..];
                table.notes.push("First line read as a header".to_owned());
            }
        }
        if !table.had_header {
            table.columns = guess_columns(body);
            table.notes.push(
                "No header found: columns guessed from the content. Check the preview".to_owned(),
            );
        }
        let width = body.iter().map(|(_, c)| c.len()).max().unwrap_or(0);
        if table.columns.function.is_none() {
            // Fall back to the widest text column that is not the terminal or step column.
            let taken = [
                table.columns.terminal,
                table.columns.step,
                table.columns.target,
            ];
            table.columns.function = (0..width)
                .filter(|c| !taken.contains(&Some(*c)))
                .max_by_key(|&c| {
                    body.iter()
                        .filter_map(|(_, r)| r.get(c))
                        .map(String::len)
                        .sum::<usize>()
                });
        }
        let cell =
            |r: &[String], c: Option<usize>| c.and_then(|c| r.get(c)).cloned().unwrap_or_default();
        for (line, cells) in body {
            if cells.iter().all(|c| c.trim().is_empty()) {
                continue;
            }
            table.rows.push(Row {
                line: *line,
                terminal: cell(cells, table.columns.terminal),
                function: cell(cells, table.columns.function),
                step: cell(cells, table.columns.step),
                target: cell(cells, table.columns.target),
            });
        }
        if table.rows.len() > MAX_ROWS {
            table.rows.truncate(MAX_ROWS);
            table.notes.push(format!(
                "Only the first {MAX_ROWS} rows were read: a controller page is much shorter than that"
            ));
        }
        let name = |c: Option<usize>| c.map_or("none".to_owned(), |c| format!("{}", c + 1));
        table.notes.push(format!(
            "Terminal column {}, function column {}, step column {}, target column {}",
            name(table.columns.terminal),
            name(table.columns.function),
            name(table.columns.step),
            name(table.columns.target)
        ));
        table
    }
}

fn guess_columns(body: &[(usize, Vec<String>)]) -> Columns {
    let width = body.iter().map(|(_, c)| c.len()).max().unwrap_or(0);
    let filled = |c: usize| -> Vec<&str> {
        body.iter()
            .filter_map(|(_, r)| r.get(c))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect()
    };
    let share = |c: usize, f: &dyn Fn(&str) -> bool| -> f64 {
        let cells = filled(c);
        if cells.is_empty() {
            0.0
        } else {
            cells.iter().filter(|s| f(s)).count() as f64 / cells.len() as f64
        }
    };
    let mut cols = Columns::default();
    if width == 1 {
        cols.function = Some(0);
        return cols;
    }
    // The step column holds only step-like cells; the terminal column mostly short labels.
    for c in 0..width {
        if share(c, &|s| parse_step_cell(s).is_some()) >= 0.7 && filled(c).len() >= 2 && c > 0 {
            cols.step = Some(c);
        }
    }
    for c in 0..width {
        if Some(c) != cols.step && share(c, &looks_like_terminal) >= 0.6 {
            cols.terminal = Some(c);
            break;
        }
    }
    if cols.terminal.is_none() && width >= 2 {
        cols.terminal = Some(0);
    }
    cols.function = (0..width)
        .filter(|c| Some(*c) != cols.terminal && Some(*c) != cols.step)
        .max_by_key(|&c| filled(c).iter().map(|s| s.len()).sum::<usize>());
    cols
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delim {
    Char(char),
    Wide,
    First,
}

fn detect_delimiter(lines: &[(usize, &str)]) -> Delim {
    if lines.is_empty() {
        return Delim::First;
    }
    for d in ['\t', ';', '|', ','] {
        let with = lines.iter().filter(|(_, l)| l.contains(d)).count();
        if with * 5 >= lines.len() * 4 {
            return Delim::Char(d);
        }
    }
    let wide = lines
        .iter()
        .filter(|(_, l)| l.trim().contains("  "))
        .count();
    if wide * 5 >= lines.len() * 4 {
        Delim::Wide
    } else {
        Delim::First
    }
}

/// Rows as text the wizard reads back: tab separated with a header, so a table found in a
/// drawing or an export can be shown, corrected by hand and read again like a pasted one.
pub fn rows_to_text(rows: &[Row]) -> String {
    let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    let mut out = String::from("Terminal\tFunction\tStep\tTarget\n");
    for r in rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            clean(&r.terminal),
            clean(&r.function),
            clean(&r.step),
            clean(&r.target)
        ));
    }
    out
}
