//! A pasted parts list read as devices to place (Ctrl+K, `place list`).
//!
//! One line is one wish: an optional tag, what it is, an optional quantity.
//!
//! ```text
//! K1 contactor 25A
//! F1 mcb 3p 16A x5
//! 3RT2016-1BB41
//! 4x LC1D09
//! LC1D09<TAB>6          (cells copied from Excel)
//! ```
//!
//! [`parse`] reads the text; [`resolve`] decides for each line what would be
//! placed, without touching a project, so the window can show a preview
//! first. What a line names is looked up in this order:
//!
//! 1. an order or type number of a catalog part (exact, spaces and dashes
//!    ignored): that part, drawn with the symbol its words name;
//! 2. a symbol by its alias, name or function word (`mcb`, `contactor`),
//!    with the other words (`3p 16A`) narrowing the catalog. Exactly one
//!    fitting part is chosen; several or none leave the device without a
//!    part, and the preview says how many fitted, so nothing is guessed;
//! 3. anything else is unknown and is left out, with the reason.
//!
//! A leading tag (`K1`, `-K1`) names the first device; a quantity counts it
//! up (`F1 ... x3` gives `F1`, `F2`, `F3`). Without a tag, devices are
//! numbered by the editor as for any placed symbol.

use novo_symbols::{Library, Symbol};

use crate::catalog::{Catalog, CatalogPart, PartKey};
use crate::search::normalize_code;

/// The most devices one list may place, so a stray number cannot flood a page.
pub const MAX_DEVICES: usize = 500;
const MAX_QTY: u32 = 200;

/// One line of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// 1-based line in the pasted text.
    pub line: usize,
    /// What is asked for, without tag and quantity.
    pub text: String,
    pub qty: u32,
    /// The tag typed in front, as typed (`K1`).
    pub tag: Option<String>,
}

impl Wanted {
    /// The tag typed for the `n`th device of this line (0-based): the typed
    /// number counted up by `n`. `None` when no tag was typed.
    pub fn tag_for(&self, n: u32) -> Option<String> {
        let tag = self.tag.as_deref()?;
        let at = tag.find(|c: char| c.is_ascii_digit())?;
        let (letters, digits) = tag.split_at(at);
        let number: u32 = digits.parse().ok()?;
        Some(format!("{letters}{}", number + n))
    }
}

fn is_tag(word: &str) -> bool {
    let w = word.strip_prefix('-').unwrap_or(word);
    let letters = w.chars().take_while(char::is_ascii_uppercase).count();
    let digits = &w[letters..];
    (1..=3).contains(&letters)
        && (1..=4).contains(&digits.len())
        && digits.chars().all(|c| c.is_ascii_digit())
}

fn quantity_of(word: &str) -> Option<u32> {
    let w = word.trim_matches(|c| c == '(' || c == ')');
    let w = w
        .strip_prefix(['x', 'X', '×', '*'])
        .or_else(|| w.strip_suffix(['x', 'X', '×', '*']))?;
    w.parse().ok()
}

/// Reads the pasted text. Blank lines and lines that start with `#` are
/// skipped. A quantity is `x5`, `5x`, a leading number (`5 mcb`), or a cell
/// that is only a number when the line has tabs or semicolons.
pub fn parse(text: &str) -> Vec<Wanted> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut qty: Option<u32> = None;
        let mut words: Vec<String> = Vec::new();
        let cells: Vec<&str> = if line.contains(['\t', ';']) {
            line.split(['\t', ';']).map(str::trim).collect()
        } else {
            vec![line]
        };
        let tabular = cells.len() > 1;
        for cell in cells.iter().filter(|c| !c.is_empty()) {
            if tabular && qty.is_none() && cell.chars().all(|c| c.is_ascii_digit()) {
                qty = cell.parse().ok();
                continue;
            }
            for word in cell.split_whitespace() {
                match quantity_of(word) {
                    Some(q) if qty.is_none() => qty = Some(q),
                    _ => words.push(word.to_owned()),
                }
            }
        }
        // `5 mcb 3p`: a bare leading number is the quantity.
        if qty.is_none()
            && words.len() > 1
            && words[0].chars().all(|c| c.is_ascii_digit())
            && let Ok(q) = words[0].parse()
        {
            qty = Some(q);
            words.remove(0);
        }
        let mut tag = None;
        if words.len() > 1 && is_tag(&words[0]) {
            tag = Some(words.remove(0).trim_start_matches('-').to_owned());
        }
        if words.is_empty() {
            continue;
        }
        out.push(Wanted {
            line: i + 1,
            text: words.join(" "),
            qty: qty.unwrap_or(1).clamp(1, MAX_QTY),
            tag,
        });
    }
    out
}

/// What one line would place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The symbol, with this catalog part assigned.
    Part { key: PartKey, symbol: String },
    /// The symbol alone. `fitting` parts matched the other words (0 when
    /// there were none to match), none is chosen.
    Symbol { symbol: String, fitting: usize },
    /// Nothing can be placed for this line.
    Unknown(&'static str),
}

/// A line and what it resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub wanted: Wanted,
    pub outcome: Outcome,
}

impl Resolved {
    pub fn symbol(&self) -> Option<&str> {
        match &self.outcome {
            Outcome::Part { symbol, .. } | Outcome::Symbol { symbol, .. } => Some(symbol),
            Outcome::Unknown(_) => None,
        }
    }

    pub fn placeable(&self) -> bool {
        self.symbol().is_some()
    }
}

/// The devices the placeable lines would make.
pub fn device_count(lines: &[Resolved]) -> usize {
    lines
        .iter()
        .filter(|l| l.placeable())
        .map(|l| l.wanted.qty as usize)
        .sum()
}

fn symbol_by_phrase<'l>(library: &'l Library, phrase: &str) -> Option<&'l Symbol> {
    let p = phrase.to_lowercase();
    if let Some(s) = library.by_alias(&p) {
        return Some(s);
    }
    let mut named = library.iter().filter(|s| {
        s.name.to_lowercase() == p
            || (!p.contains(' ') && s.function.split('.').any(|seg| seg == p))
    });
    match (named.next(), named.next()) {
        (Some(one), None) => Some(one),
        _ => None,
    }
}

/// Decides each line. `symbol_of_part` is the app's rule for which symbol
/// pictures a catalog part.
pub fn resolve<'c>(
    wanted: Vec<Wanted>,
    catalog: &'c Catalog,
    library: &Library,
    symbol_of_part: &dyn Fn(&'c CatalogPart) -> Option<String>,
) -> Vec<Resolved> {
    wanted
        .into_iter()
        .map(|w| {
            let outcome = outcome_of(&w, catalog, library, symbol_of_part);
            Resolved { wanted: w, outcome }
        })
        .collect()
}

fn outcome_of<'c>(
    w: &Wanted,
    catalog: &'c Catalog,
    library: &Library,
    symbol_of_part: &dyn Fn(&'c CatalogPart) -> Option<String>,
) -> Outcome {
    let words: Vec<&str> = w.text.split_whitespace().collect();
    // 1. An article number: the whole text or one word of it.
    let candidates = std::iter::once(w.text.as_str()).chain(words.iter().copied());
    for candidate in candidates {
        let code = normalize_code(candidate);
        if code.len() < 4 {
            continue;
        }
        let exact = catalog.find_order_number(candidate).or_else(|| {
            catalog
                .search(candidate, 20)
                .into_iter()
                .find(|p| normalize_code(&p.type_number) == code)
        });
        if let Some(part) = exact {
            return match symbol_of_part(part) {
                Some(symbol) => Outcome::Part {
                    key: part.key(),
                    symbol,
                },
                None => Outcome::Unknown("no symbol for this part"),
            };
        }
    }
    // 2. A symbol word or phrase (longest first), the rest narrows parts.
    for len in (1..=words.len().min(3)).rev() {
        for start in 0..=words.len() - len {
            let phrase = words[start..start + len].join(" ");
            let Some(symbol) = symbol_by_phrase(library, &phrase) else {
                continue;
            };
            let rest: Vec<&str> = words[..start]
                .iter()
                .chain(&words[start + len..])
                .copied()
                .collect();
            if rest.is_empty() {
                return Outcome::Symbol {
                    symbol: symbol.id.clone(),
                    fitting: 0,
                };
            }
            let fitting: Vec<&CatalogPart> = catalog
                .search(&rest.join(" "), 300)
                .into_iter()
                .filter(|p| symbol_of_part(p).as_deref() == Some(symbol.id.as_str()))
                .collect();
            return match fitting.as_slice() {
                [one] => Outcome::Part {
                    key: one.key(),
                    symbol: symbol.id.clone(),
                },
                many => Outcome::Symbol {
                    symbol: symbol.id.clone(),
                    fitting: many.len(),
                },
            };
        }
    }
    Outcome::Unknown("not a part number or symbol")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str) -> Vec<Wanted> {
        parse(text)
    }

    #[test]
    fn quantities_read_in_every_form() {
        let got = w("F1 mcb 3p 16A x5\n4x LC1D09\n3 relay\nLC1D09\t6\nmcb;2");
        let q: Vec<u32> = got.iter().map(|l| l.qty).collect();
        assert_eq!(q, [5, 4, 3, 6, 2]);
        assert_eq!(got[0].tag.as_deref(), Some("F1"));
        assert_eq!(got[0].text, "mcb 3p 16A");
        assert_eq!(got[2].text, "relay");
    }

    #[test]
    fn blank_comment_and_tag_only_lines_are_skipped() {
        assert!(w("\n# note\n   \n").is_empty());
        // A lone tag-shaped word may be an article number, so it stays text.
        assert_eq!(w("K1")[0].text, "K1");
        assert_eq!(w("-Q3 mpcb")[0].tag.as_deref(), Some("Q3"));
    }

    #[test]
    fn a_part_number_is_not_taken_for_a_tag() {
        // `LC1D09` has letters, a digit, a letter: not a tag.
        let got = w("LC1D09 contactor");
        assert_eq!(got[0].tag, None);
        assert_eq!(got[0].text, "LC1D09 contactor");
    }

    #[test]
    fn typed_tags_count_up() {
        let l = &w("F1 mcb x3")[0];
        assert_eq!(l.tag_for(0).as_deref(), Some("F1"));
        assert_eq!(l.tag_for(2).as_deref(), Some("F3"));
        assert_eq!(w("mcb")[0].tag_for(0), None);
    }

    fn symbols_by_keyword<'a>(
        library: &'a Library,
    ) -> impl Fn(&CatalogPart) -> Option<String> + 'a {
        move |p: &CatalogPart| {
            p.keywords
                .iter()
                .find_map(|k| library.by_alias(k))
                .map(|s| s.id.clone())
        }
    }

    #[test]
    fn lines_resolve_to_a_part_a_symbol_or_nothing() {
        let library = Library::builtin().unwrap();
        let catalog = Catalog::starter();
        let part = catalog
            .parts()
            .iter()
            .find(|p| symbols_by_keyword(&library)(p).is_some())
            .expect("a starter part with a symbol");
        let of = symbols_by_keyword(&library);
        let text = format!(
            "{}\nzzz nothing here\nmcb\n{} x2",
            part.order_number, part.order_number
        );
        let got = resolve(parse(&text), &catalog, &library, &of);
        assert!(matches!(&got[0].outcome, Outcome::Part { key, .. } if *key == part.key()));
        assert!(matches!(got[1].outcome, Outcome::Unknown(_)));
        assert!(matches!(
            &got[2].outcome,
            Outcome::Symbol { fitting: 0, .. }
        ));
        assert_eq!(device_count(&got), 4);
    }

    #[test]
    fn several_fitting_parts_choose_none() {
        let library = Library::builtin().unwrap();
        let catalog = Catalog::starter();
        let of = symbols_by_keyword(&library);
        let got = resolve(parse("mcb 16A"), &catalog, &library, &of);
        match &got[0].outcome {
            Outcome::Symbol { fitting, .. } => assert!(*fitting != 1),
            Outcome::Part { .. } => {}
            Outcome::Unknown(why) => panic!("{why}"),
        }
    }

    #[test]
    fn quantity_is_clamped() {
        assert_eq!(w("mcb x99999")[0].qty, MAX_QTY);
        assert_eq!(w("mcb x0")[0].qty, 1);
    }
}
