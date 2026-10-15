//! Step text markup (design §3): `**bold**`, `{key:command.id}`,
//! `{ui:anchor.id}`, `{tip:term}` and blank-line paragraphs.

/// One piece of step text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Span {
    Text(String),
    Bold(String),
    /// The key of a command, read from the live keymap.
    Key(String),
    /// The visible name of a control.
    Ui(String),
    /// A glossary term with a hover tooltip.
    Tip(String),
    /// A paragraph break.
    Break,
}

/// Why a text did not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkupError(pub String);

impl std::fmt::Display for MarkupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MarkupError {}

/// Splits step text into spans. Single newlines are spaces; a blank line is
/// a [`Span::Break`]. Unbalanced `**`, an unclosed `{` and an unknown
/// `{kind:...}` are errors, so a typo is caught by the lint.
pub fn parse_markup(text: &str) -> Result<Vec<Span>, MarkupError> {
    let mut spans = Vec::new();
    for (i, paragraph) in split_paragraphs(text).iter().enumerate() {
        if i > 0 {
            spans.push(Span::Break);
        }
        parse_paragraph(paragraph, &mut spans)?;
    }
    Ok(spans)
}

fn split_paragraphs(text: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join(" "));
                current.clear();
            }
        } else {
            current.push(line.trim());
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join(" "));
    }
    paragraphs
}

fn parse_paragraph(text: &str, out: &mut Vec<Span>) -> Result<(), MarkupError> {
    let mut rest = text;
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Span>| {
        if !plain.is_empty() {
            out.push(Span::Text(std::mem::take(plain)));
        }
    };
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**") {
            let end = after
                .find("**")
                .ok_or_else(|| MarkupError(format!("`**` is not closed in `{text}`")))?;
            flush(&mut plain, out);
            out.push(Span::Bold(after[..end].to_owned()));
            rest = &after[end + 2..];
        } else if let Some(after) = rest.strip_prefix('{') {
            let end = after
                .find('}')
                .ok_or_else(|| MarkupError(format!("`{{` is not closed in `{text}`")))?;
            let body = &after[..end];
            let (kind, value) = body
                .split_once(':')
                .filter(|(_, v)| !v.trim().is_empty())
                .ok_or_else(|| {
                    MarkupError(format!("`{{{body}}}` needs the form {{kind:value}}"))
                })?;
            let value = value.trim().to_owned();
            flush(&mut plain, out);
            out.push(match kind {
                "key" => Span::Key(value),
                "ui" => Span::Ui(value),
                "tip" => Span::Tip(value),
                other => {
                    return Err(MarkupError(format!(
                        "`{{{other}:...}}` is not markup (key, ui or tip)"
                    )));
                }
            });
            rest = &after[end + 1..];
        } else {
            let ch = rest.chars().next().expect("rest is not empty");
            plain.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    flush(&mut plain, out);
    Ok(())
}

/// The commands, anchors and glossary terms a text refers to, for the lint.
pub fn references(spans: &[Span]) -> impl Iterator<Item = (&'static str, &str)> {
    spans.iter().filter_map(|s| match s {
        Span::Key(v) => Some(("key", v.as_str())),
        Span::Ui(v) => Some(("ui", v.as_str())),
        Span::Tip(v) => Some(("tip", v.as_str())),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_bold_key_and_breaks() {
        let spans = parse_markup(
            "Press {key:tool.connect}, or click **Connect**.\nIt waits.\n\nThen {tip:pin} {ui:dock.connect}",
        )
        .unwrap();
        assert_eq!(
            spans,
            vec![
                Span::Text("Press ".into()),
                Span::Key("tool.connect".into()),
                Span::Text(", or click ".into()),
                Span::Bold("Connect".into()),
                Span::Text(". It waits.".into()),
                Span::Break,
                Span::Text("Then ".into()),
                Span::Tip("pin".into()),
                Span::Text(" ".into()),
                Span::Ui("dock.connect".into()),
            ]
        );
    }

    #[test]
    fn mistakes_are_errors() {
        assert!(parse_markup("**open").is_err());
        assert!(parse_markup("{key:tool.connect").is_err());
        assert!(parse_markup("{key}").is_err());
        assert!(parse_markup("{key: }").is_err());
        assert!(parse_markup("{color:red}").is_err());
    }

    #[test]
    fn non_ascii_text_survives() {
        let spans = parse_markup("Contactor – schakelaar ✓").unwrap();
        assert_eq!(spans, vec![Span::Text("Contactor – schakelaar ✓".into())]);
    }

    #[test]
    fn references_lists_what_the_lint_checks() {
        let spans = parse_markup("{key:a.b} {ui:c.d} {tip:e}").unwrap();
        let refs: Vec<_> = references(&spans).collect();
        assert_eq!(refs, vec![("key", "a.b"), ("ui", "c.d"), ("tip", "e")]);
    }
}
