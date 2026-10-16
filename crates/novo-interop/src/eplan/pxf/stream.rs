//! A pull tokenizer for PXF: start tags, end tags and text, one at a time,
//! from any [`BufRead`]. Nothing is kept after an event is handed out, so a
//! 200 MB project is read in constant memory. Comments, processing
//! instructions and DOCTYPE are skipped and no reference is ever followed.

use std::io::BufRead;

use super::PxfError;
use crate::eplan::xml::entity;

/// One thing read from the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    Start {
        name: String,
        attrs: Vec<(String, String)>,
    },
    End {
        name: String,
    },
    Text(String),
}

/// The longest single tag or text run accepted (a hostile file with no `>`
/// must not fill memory).
const MAX_RUN: usize = 64 * 1024 * 1024;

pub(crate) struct Tokens<R: BufRead> {
    r: R,
    buf: Vec<u8>,
    line: usize,
    /// `<` has been consumed and the tag after it is next.
    in_tag: bool,
    pending_end: Option<String>,
}

impl<R: BufRead> Tokens<R> {
    pub(crate) fn new(r: R) -> Self {
        Self {
            r,
            buf: Vec::new(),
            line: 1,
            in_tag: false,
            pending_end: None,
        }
    }

    pub(crate) fn line(&self) -> usize {
        self.line
    }

    fn read_until(&mut self, delim: u8) -> Result<usize, PxfError> {
        let n = self
            .r
            .read_until(delim, &mut self.buf)
            .map_err(|e| PxfError::Io(e.to_string()))?;
        if self.buf.len() > MAX_RUN {
            return Err(PxfError::TooLarge(self.line));
        }
        Ok(n)
    }

    /// Reads on until the tag is complete or the file ends. Quotes and
    /// brackets are counted on the new bytes only, so a hostile tag with
    /// many `>` inside one quoted value costs linear time.
    fn read_tag(&mut self) -> Result<(), PxfError> {
        self.buf.clear();
        let (mut scanned, mut quotes, mut open, mut close) = (0usize, 0usize, 0usize, 0usize);
        loop {
            let n = self.read_until(b'>')?;
            if n == 0 {
                break;
            }
            for c in &self.buf[scanned..] {
                match c {
                    b'"' => quotes += 1,
                    b'[' => open += 1,
                    b']' => close += 1,
                    _ => {}
                }
            }
            scanned = self.buf.len();
            if !self.unfinished(quotes, open > close) {
                break;
            }
        }
        self.line += self.buf.iter().filter(|b| **b == b'\n').count();
        Ok(())
    }

    /// Whether the bytes read so far stop inside a comment, CDATA section,
    /// DOCTYPE subset or quoted value, where a `>` does not end the tag.
    fn unfinished(&self, quotes: usize, brackets_open: bool) -> bool {
        let b = &self.buf;
        if b.last() != Some(&b'>') {
            return false; // end of file: let the caller report it
        }
        if b.starts_with(b"!--") {
            return !b.ends_with(b"-->");
        }
        if b.starts_with(b"![CDATA[") {
            return !b.ends_with(b"]]>");
        }
        if b.starts_with(b"!") {
            return brackets_open;
        }
        quotes % 2 == 1
    }

    pub(crate) fn next_event(&mut self) -> Result<Option<Event>, PxfError> {
        if let Some(name) = self.pending_end.take() {
            return Ok(Some(Event::End { name }));
        }
        loop {
            if !self.in_tag {
                self.buf.clear();
                let n = self.read_until(b'<')?;
                if n == 0 {
                    return Ok(None);
                }
                let had_lt = self.buf.last() == Some(&b'<');
                if had_lt {
                    self.buf.pop();
                }
                self.line += self.buf.iter().filter(|b| **b == b'\n').count();
                self.in_tag = had_lt;
                let text = String::from_utf8_lossy(&self.buf);
                let text = text.trim();
                if !text.is_empty() {
                    return Ok(Some(Event::Text(entity(text))));
                }
                if !had_lt {
                    return Ok(None);
                }
            }
            self.read_tag()?;
            self.in_tag = false;
            if self.buf.last() != Some(&b'>') {
                return Err(PxfError::Truncated(self.line));
            }
            self.buf.pop();
            let raw = String::from_utf8_lossy(&self.buf).into_owned();
            if raw.starts_with("!--") || raw.starts_with('?') {
                continue;
            }
            if let Some(cdata) = raw.strip_prefix("![CDATA[") {
                let t = cdata.trim_end_matches("]]").trim();
                if t.is_empty() {
                    continue;
                }
                return Ok(Some(Event::Text(t.to_string())));
            }
            if raw.starts_with('!') {
                continue;
            }
            if let Some(name) = raw.strip_prefix('/') {
                return Ok(Some(Event::End {
                    name: local(name.trim()).to_string(),
                }));
            }
            let self_closing = raw.ends_with('/');
            let inner = raw.trim_end_matches('/');
            let (name, attrs) = split_tag(inner);
            if name.is_empty() {
                return Err(PxfError::BadTag(self.line));
            }
            if self_closing {
                self.pending_end = Some(name.clone());
            }
            return Ok(Some(Event::Start { name, attrs }));
        }
    }
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Splits `name a="1" b='2'` into the local name and the attributes.
fn split_tag(inner: &str) -> (String, Vec<(String, String)>) {
    let inner = inner.trim();
    let name_end = inner
        .find(|c: char| c.is_whitespace())
        .unwrap_or(inner.len());
    let name = local(&inner[..name_end]).to_string();
    let mut attrs = Vec::new();
    let mut rest = inner[name_end..].trim_start();
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].trim().to_string();
        let after = rest[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let Some(close) = after[1..].find(q) else {
            break;
        };
        attrs.push((key, entity(&after[1..1 + close])));
        rest = after[close + 2..].trim_start();
    }
    (name, attrs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(s: &str) -> Vec<Event> {
        let mut t = Tokens::new(s.as_bytes());
        let mut v = Vec::new();
        while let Some(e) = t.next_event().unwrap() {
            v.push(e);
        }
        v
    }

    #[test]
    fn tags_attributes_and_entities() {
        let ev = all(r#"<?xml version="1.0"?><!-- c --><A x="1 &amp; 2" y='z'><B/>t&lt;</A>"#);
        assert_eq!(
            ev[0],
            Event::Start {
                name: "A".into(),
                attrs: vec![("x".into(), "1 & 2".into()), ("y".into(), "z".into())]
            }
        );
        assert_eq!(ev[2], Event::End { name: "B".into() });
        assert_eq!(ev[3], Event::Text("t<".into()));
        assert_eq!(ev.len(), 5);
    }

    #[test]
    fn a_gt_inside_a_quoted_value_does_not_end_the_tag() {
        let ev = all(r#"<P a="x>y" b="2"/>"#);
        let Event::Start { attrs, .. } = &ev[0] else {
            panic!()
        };
        assert_eq!(attrs[0].1, "x>y");
        assert_eq!(attrs[1].1, "2");
    }

    #[test]
    fn doctype_with_subset_and_comments_are_skipped() {
        let ev = all("<!DOCTYPE a [<!ENTITY x \"y\">]><!-- a > b --><a/>");
        assert_eq!(ev.len(), 2);
    }

    #[test]
    fn many_gt_inside_one_quoted_value_stay_linear() {
        // 400 000 `>` in one value: quadratic rescanning would take minutes.
        let doc = format!("<P a=\"{}\"/>", ">".repeat(400_000));
        let t0 = std::time::Instant::now();
        let ev = all(&doc);
        assert!(t0.elapsed().as_secs() < 5);
        let Event::Start { attrs, .. } = &ev[0] else {
            panic!()
        };
        assert_eq!(attrs[0].1.len(), 400_000);
    }

    #[test]
    fn a_file_cut_inside_a_tag_is_an_error() {
        let mut t = Tokens::new(&b"<a><b x=\"1\""[..]);
        t.next_event().unwrap();
        assert!(matches!(t.next_event(), Err(PxfError::Truncated(_))));
    }
}
