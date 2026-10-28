//! Claude's answers arrive as Markdown. The window shows them as text with
//! headings, bullets, tables and bold, not as `**` and `|` characters.
//! Parsing is pure and tested; only [`show`] touches egui.

use eframe::egui;

/// One block of an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Block {
    Heading(String),
    Para(String),
    /// A list item with its marker (`•` or `1.`).
    Item(String, String),
    Code(String),
    Table {
        head: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Rule,
}

/// A piece of one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Span {
    Plain(String),
    Bold(String),
    Italic(String),
    Code(String),
}

fn cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_owned()).collect()
}

fn is_separator(line: &str) -> bool {
    let t = line.trim();
    t.contains('-') && t.contains('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn is_rule(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*'))
}

fn item(line: &str) -> Option<(String, String)> {
    let t = line.trim_start();
    for m in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(m) {
            return Some(("•".into(), rest.trim().to_owned()));
        }
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if (1..=3).contains(&digits)
        && let Some(rest) = t[digits..]
            .strip_prefix(". ")
            .or_else(|| t[digits..].strip_prefix(") "))
    {
        return Some((format!("{}.", &t[..digits]), rest.trim().to_owned()));
    }
    None
}

/// Splits an answer into blocks. Text that is not Markdown comes out as
/// paragraphs, one per line, unchanged.
pub(super) fn blocks(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    // One paragraph per line: an answer's own line breaks are kept.
    let mut para: Vec<String> = Vec::new();
    let flush = |para: &mut Vec<String>, out: &mut Vec<Block>| {
        out.extend(para.drain(..).map(Block::Para));
    };
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.starts_with("```") {
            flush(&mut para, &mut out);
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                code.push(lines[i]);
                i += 1;
            }
            out.push(Block::Code(code.join("\n")));
        } else if t.is_empty() {
            flush(&mut para, &mut out);
        } else if t.starts_with('#') && t.trim_start_matches('#').starts_with(' ') {
            flush(&mut para, &mut out);
            out.push(Block::Heading(t.trim_start_matches('#').trim().to_owned()));
        } else if t.contains('|') && i + 1 < lines.len() && is_separator(lines[i + 1]) {
            flush(&mut para, &mut out);
            let head = cells(line);
            i += 2;
            let mut rows = Vec::new();
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(cells(lines[i]));
                i += 1;
            }
            out.push(Block::Table { head, rows });
            continue;
        } else if is_rule(t) {
            flush(&mut para, &mut out);
            out.push(Block::Rule);
        } else if let Some((marker, body)) = item(line) {
            flush(&mut para, &mut out);
            out.push(Block::Item(marker, body));
        } else {
            para.push(t.to_owned());
        }
        i += 1;
    }
    flush(&mut para, &mut out);
    out
}

/// Bold, italic and code inside one line. An unclosed marker stays as typed.
pub(super) fn spans(line: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut rest = line;
    let push_plain = |plain: &mut String, out: &mut Vec<Span>| {
        if !plain.is_empty() {
            out.push(Span::Plain(std::mem::take(plain)));
        }
    };
    while !rest.is_empty() {
        let found = [("**", 2usize), ("`", 1), ("*", 1)]
            .into_iter()
            .find(|(m, _)| rest.starts_with(m));
        if let Some((marker, n)) = found
            && let Some(end) = rest[n..].find(marker)
            && end > 0
            && !rest[n..n + end].starts_with(char::is_whitespace)
            && !rest[n..n + end].ends_with(char::is_whitespace)
        {
            let inner = rest[n..n + end].to_owned();
            push_plain(&mut plain, &mut out);
            out.push(match marker {
                "**" => Span::Bold(inner),
                "`" => Span::Code(inner),
                _ => Span::Italic(inner),
            });
            rest = &rest[n + end + n..];
            continue;
        }
        let c = rest.chars().next().unwrap_or(' ');
        plain.push(c);
        rest = &rest[c.len_utf8()..];
    }
    push_plain(&mut plain, &mut out);
    out
}

fn inline(ui: &mut egui::Ui, text: &str, size: Option<f32>, strong: bool) {
    let font = size.map_or_else(
        || egui::TextStyle::Body.resolve(ui.style()),
        egui::FontId::proportional,
    );
    let color = ui.visuals().text_color();
    let bold_color = ui.visuals().strong_text_color();
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = ui.available_width();
    for span in spans(text) {
        let (s, mut f) = match span {
            Span::Plain(s) => (
                s,
                egui::TextFormat::simple(font.clone(), if strong { bold_color } else { color }),
            ),
            Span::Bold(s) => (s, egui::TextFormat::simple(font.clone(), bold_color)),
            Span::Italic(s) => {
                let mut f = egui::TextFormat::simple(font.clone(), color);
                f.italics = true;
                (s, f)
            }
            Span::Code(s) => {
                let mut f =
                    egui::TextFormat::simple(egui::TextStyle::Monospace.resolve(ui.style()), color);
                f.background = ui.visuals().code_bg_color;
                (s, f)
            }
        };
        f.valign = egui::Align::BOTTOM;
        job.append(&s, 0.0, f);
    }
    ui.add(egui::Label::new(job));
}

/// Draws an answer. `id` keeps two tables on screen apart.
pub(super) fn show(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, text: &str) {
    for (n, block) in blocks(text).iter().enumerate() {
        match block {
            Block::Heading(h) => {
                ui.add_space(2.0);
                inline(ui, h, Some(16.0), true);
            }
            Block::Para(p) => inline(ui, p, None, false),
            Block::Item(marker, body) => {
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0);
                    ui.label(marker);
                    ui.vertical(|ui| inline(ui, body, None, false));
                });
            }
            Block::Code(c) => {
                ui.add(egui::Label::new(egui::RichText::new(c).monospace()));
            }
            Block::Table { head, rows } => {
                egui::Grid::new((&id, n))
                    .striped(true)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        for h in head {
                            inline(ui, h, None, true);
                        }
                        ui.end_row();
                        for r in rows {
                            for c in r {
                                inline(ui, c, None, false);
                            }
                            ui.end_row();
                        }
                    });
            }
            Block::Rule => {
                ui.separator();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_italic_and_code_lose_their_markers() {
        assert_eq!(
            spans("a **b** and *c* and `d` e"),
            vec![
                Span::Plain("a ".into()),
                Span::Bold("b".into()),
                Span::Plain(" and ".into()),
                Span::Italic("c".into()),
                Span::Plain(" and ".into()),
                Span::Code("d".into()),
                Span::Plain(" e".into()),
            ]
        );
    }

    #[test]
    fn an_unclosed_marker_stays_as_typed() {
        assert_eq!(
            spans("5 * 3 and **x"),
            vec![Span::Plain("5 * 3 and **x".into())]
        );
    }

    #[test]
    fn a_table_becomes_cells_not_pipes() {
        let b = blocks("Result:\n\n| Net | Wire |\n|---|---|\n| L1 | 101 |\n| L2 | 102 |\n\nDone");
        assert_eq!(b.len(), 3);
        assert_eq!(
            b[1],
            Block::Table {
                head: vec!["Net".into(), "Wire".into()],
                rows: vec![
                    vec!["L1".into(), "101".into()],
                    vec!["L2".into(), "102".into()]
                ],
            }
        );
    }

    #[test]
    fn headings_lists_and_code() {
        let b = blocks("## Plan\n- one\n2. two\n```\nx = 1\n```\n");
        assert_eq!(
            b,
            vec![
                Block::Heading("Plan".into()),
                Block::Item("•".into(), "one".into()),
                Block::Item("2.".into(), "two".into()),
                Block::Code("x = 1".into()),
            ]
        );
    }

    #[test]
    fn plain_text_is_unchanged() {
        assert_eq!(
            blocks("Three devices have no part.\nCheck -K1."),
            vec![
                Block::Para("Three devices have no part.".into()),
                Block::Para("Check -K1.".into())
            ]
        );
    }
}
