//! The card's own words (design "Language"): Back, Skip step, Restart
//! lesson and the rest live in `assets/lessons/ui.toml` with `_nl` twins,
//! so the application has no English typed in Rust for the lesson screens.

use std::collections::BTreeMap;

use crate::catalog::Term;
use crate::lesson::Lang;

/// Interface words by key.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiStrings {
    words: BTreeMap<String, Term>,
}

impl UiStrings {
    /// Reads `ui.toml`. A file that does not parse gives no words, and the
    /// screens then show the keys, which is plain but never blank.
    pub fn parse(text: &str) -> Result<UiStrings, String> {
        toml::from_str::<BTreeMap<String, Term>>(text)
            .map(|words| UiStrings { words })
            .map_err(|e| e.to_string())
    }

    /// The word for `key` in `lang`, falling back to English and then to
    /// the key itself.
    pub fn get<'a>(&'a self, key: &'a str, lang: Lang) -> &'a str {
        self.words.get(key).map_or(key, |t| t.text(lang))
    }

    /// `get` with `{name}` placeholders filled from `args`.
    pub fn format(&self, key: &str, lang: Lang, args: &[(&str, &str)]) -> String {
        let mut text = self.get(key, lang).to_owned();
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.words.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = r#"
[back]
text = "Back"
text_nl = "Terug"

[step_of]
text = "{n} of {total}"
"#;

    #[test]
    fn words_fall_back_to_english_and_then_the_key() {
        let ui = UiStrings::parse(TEXT).unwrap();
        assert_eq!(ui.get("back", Lang::Nl), "Terug");
        assert_eq!(ui.get("back", Lang::En), "Back");
        assert_eq!(ui.get("step_of", Lang::Nl), "{n} of {total}");
        assert_eq!(ui.get("missing", Lang::En), "missing");
    }

    #[test]
    fn placeholders_are_filled() {
        let ui = UiStrings::parse(TEXT).unwrap();
        assert_eq!(
            ui.format("step_of", Lang::En, &[("n", "2"), ("total", "9")]),
            "2 of 9"
        );
    }

    #[test]
    fn unknown_keys_in_a_word_are_refused() {
        assert!(UiStrings::parse("[a]\ntext = \"x\"\ntypo = 1\n").is_err());
    }
}
