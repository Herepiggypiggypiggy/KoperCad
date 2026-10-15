//! Text matching shared by the catalog search and the properties filter.

use crate::catalog::CatalogPart;

/// Lower case with everything but letters and digits removed, so order
/// numbers compare without spaces, dashes or dots.
pub fn normalize_code(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// A query split into lower-case words; every word must match.
#[derive(Debug, Clone, Default)]
pub struct Matcher {
    words: Vec<(String, String)>, // (lower case, normalized code)
}

impl Matcher {
    pub fn new(query: &str) -> Self {
        let words = query
            .split_whitespace()
            .map(|w| (w.to_lowercase(), normalize_code(w)))
            .collect();
        Matcher { words }
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Whether every word occurs in one of `fields` (case-insensitive).
    pub fn matches<'a>(&self, fields: impl IntoIterator<Item = &'a str> + Clone) -> bool {
        self.words.iter().all(|(word, _)| {
            fields
                .clone()
                .into_iter()
                .any(|f| f.to_lowercase().contains(word.as_str()))
        })
    }

    /// Score of a catalog part, or `None` if some word does not match.
    /// `doc` is the part's pre-lowered text, built once when the part is
    /// added, so a search over 100,000 parts allocates nothing per part.
    pub(crate) fn score_doc(&self, part: &CatalogPart, doc: &SearchDoc) -> Option<u32> {
        let mut total = 0;
        for (word, code) in &self.words {
            let mut best = 0;
            let long = code.len() >= 3;
            if !code.is_empty() {
                best = best.max(code_score(&doc.order, code, long, 100, 60, 30));
                best = best.max(code_score(&doc.type_number, code, long, 80, 50, 25));
            }
            for keyword in &doc.keywords {
                if keyword == word {
                    best = best.max(45);
                } else if keyword.split_whitespace().any(|k| k == word) {
                    best = best.max(30);
                } else if keyword.starts_with(word.as_str()) {
                    best = best.max(20);
                }
            }
            if doc.letter_codes.iter().any(|c| c == word) {
                best = best.max(35);
            }
            if !code.is_empty() && doc.class_codes.iter().any(|c| c == code) {
                best = best.max(40);
            }
            if !doc.function.is_empty() && doc.function == *word {
                best = best.max(35);
            }
            for text in [&doc.description, &doc.manufacturer] {
                if text.contains(word.as_str()) {
                    if text
                        .split(|c: char| !c.is_alphanumeric() && c != '.')
                        .any(|w| w == word)
                    {
                        best = best.max(15);
                    } else {
                        best = best.max(8);
                    }
                }
            }
            if best == 0 && doc.data.contains(word.as_str()) {
                best = 5;
            }
            if best == 0 {
                let _ = part;
                return None;
            }
            total += best;
        }
        Some(total)
    }
}

/// Lower-cased and normalised text of one part, kept next to it by the
/// catalog so a search does not rebuild it per query.
#[derive(Debug, Clone, Default)]
pub(crate) struct SearchDoc {
    order: String,
    type_number: String,
    description: String,
    manufacturer: String,
    data: String,
    keywords: Vec<String>,
    letter_codes: Vec<String>,
    /// Normalised ETIM and ECLASS codes.
    class_codes: Vec<String>,
    function: String,
}

impl SearchDoc {
    pub(crate) fn new(part: &CatalogPart) -> Self {
        SearchDoc {
            order: normalize_code(&part.order_number),
            type_number: normalize_code(&part.type_number),
            description: part.description.to_lowercase(),
            manufacturer: part.manufacturer.to_lowercase(),
            data: part.data_text().to_lowercase(),
            keywords: part.keywords.iter().map(|k| k.to_lowercase()).collect(),
            letter_codes: part.letter_codes.iter().map(|c| c.to_lowercase()).collect(),
            class_codes: [&part.class.etim, &part.class.eclass]
                .into_iter()
                .flatten()
                .map(|c| normalize_code(c))
                .collect(),
            function: part.function.to_lowercase(),
        }
    }
}

fn code_score(field: &str, code: &str, long: bool, exact: u32, prefix: u32, inner: u32) -> u32 {
    if field.is_empty() {
        0
    } else if field == code {
        exact
    } else if long && field.starts_with(code) {
        prefix
    } else if long && field.contains(code) {
        inner
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_order_numbers() {
        assert_eq!(normalize_code("3RT2016-1BB41"), "3rt20161bb41");
        assert_eq!(normalize_code(" 3RT 2016.1bb41 "), "3rt20161bb41");
    }

    #[test]
    fn every_word_must_match() {
        let m = Matcher::new("rated cur");
        assert!(m.matches(["Rated current", "A"]));
        assert!(!m.matches(["Rated voltage"]));
        assert!(Matcher::new("").is_empty());
    }
}
