//! EPLAN's multi-language strings: `en_US@Motor;de_DE@Motor;`.
//!
//! A `;` ends an entry, so a text that held a `;` arrives truncated; that is
//! how the format works and is not repaired here. `??_??` stands for "any
//! language". A value without any `@` is taken as plain text.

/// A string in several languages, in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MlText {
    pub entries: Vec<(String, String)>,
}

impl MlText {
    /// Whether `s` has the multi-language shape (`xx_XX@text;`).
    pub fn looks_like(s: &str) -> bool {
        let Some(at) = s.find('@') else { return false };
        let lang = &s[..at];
        lang.len() == 5
            && lang.as_bytes()[2] == b'_'
            && lang
                .chars()
                .enumerate()
                .all(|(i, c)| i == 2 || c.is_ascii_alphabetic() || c == '?')
    }

    pub fn parse(s: &str) -> Self {
        if !Self::looks_like(s) {
            let t = s.trim();
            return Self {
                entries: if t.is_empty() {
                    Vec::new()
                } else {
                    vec![(String::new(), t.to_string())]
                },
            };
        }
        let mut entries = Vec::new();
        for part in s.split_terminator(';') {
            if let Some((lang, text)) = part.split_once('@') {
                entries.push((lang.trim().to_string(), text.to_string()));
            }
        }
        Self { entries }
    }

    /// The text in `lang` (`en_US`), else the any-language entry, else the
    /// first non-empty one.
    pub fn get(&self, lang: &str) -> &str {
        self.entries
            .iter()
            .find(|(l, t)| l == lang && !t.is_empty())
            .or_else(|| {
                self.entries
                    .iter()
                    .find(|(l, t)| l == "??_??" && !t.is_empty())
            })
            .or_else(|| self.entries.iter().find(|(_, t)| !t.is_empty()))
            .map_or("", |(_, t)| t.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.entries
            .iter()
            .all(|(_, t)| t.trim().is_empty() || t == "-")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_language_then_any_then_first() {
        let t = MlText::parse("de_DE@Motor;en_US@Motor M1;");
        assert_eq!(t.get("en_US"), "Motor M1");
        assert_eq!(t.get("nl_NL"), "Motor");
        assert_eq!(MlText::parse("??_??@Lamp;").get("de_DE"), "Lamp");
    }

    #[test]
    fn plain_text_and_empty_values() {
        assert_eq!(MlText::parse("Pump 1").get("en_US"), "Pump 1");
        assert!(MlText::parse("??_??@;").is_empty());
        assert!(MlText::parse("-").entries.len() == 1 && MlText::parse("-").is_empty());
        assert!(MlText::parse("").is_empty());
    }

    #[test]
    fn a_semicolon_in_the_text_truncates_it() {
        assert_eq!(MlText::parse("en_US@a;b;").get("en_US"), "a");
    }
}
