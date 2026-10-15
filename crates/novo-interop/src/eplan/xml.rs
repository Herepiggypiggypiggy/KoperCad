//! A small, forgiving XML reader: elements, attributes, text, entities,
//! CDATA. Comments, processing instructions and DOCTYPE are skipped.
//! Names lose their namespace prefix. No external references are ever
//! followed, so a hostile file cannot reach the disk or the network.

use thiserror::Error;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    /// Text directly inside this element, trimmed and joined.
    pub text: String,
    /// 1-based line of the start tag.
    pub line: usize,
}

impl Node {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum XmlError {
    #[error("line {0}: the XML is cut off")]
    Truncated(usize),
    #[error("line {line}: </{found}> closes <{expected}>")]
    Mismatch {
        line: usize,
        expected: String,
        found: String,
    },
    #[error("the file has no XML element")]
    NoRoot,
    #[error("the XML nests deeper than {0} levels")]
    TooDeep(usize),
}

const MAX_DEPTH: usize = 128;

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn entity(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let ch = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16)
                .ok()
                .and_then(char::from_u32),
            n if n.starts_with('#') => n[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Parses `text` into its root element.
pub fn parse(text: &str) -> Result<Node, XmlError> {
    let text = text.trim_start_matches('\u{feff}');
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut line = 1;
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;

    let count_lines =
        |from: usize, to: usize| bytes[from..to].iter().filter(|b| **b == b'\n').count();

    while i < bytes.len() {
        if bytes[i] != b'<' {
            let next = text[i..].find('<').map_or(bytes.len(), |p| i + p);
            if let Some(top) = stack.last_mut() {
                let t = entity(text[i..next].trim());
                if !t.is_empty() {
                    if !top.text.is_empty() {
                        top.text.push(' ');
                    }
                    top.text.push_str(&t);
                }
            }
            line += count_lines(i, next);
            i = next;
            continue;
        }
        let rest = &text[i..];
        if rest.starts_with("<!--") {
            let end = rest.find("-->").ok_or(XmlError::Truncated(line))?;
            line += count_lines(i, i + end);
            i += end + 3;
        } else if rest.starts_with("<![CDATA[") {
            let end = rest.find("]]>").ok_or(XmlError::Truncated(line))?;
            if let Some(top) = stack.last_mut() {
                let t = rest[9..end].trim();
                if !t.is_empty() {
                    if !top.text.is_empty() {
                        top.text.push(' ');
                    }
                    top.text.push_str(t);
                }
            }
            line += count_lines(i, i + end);
            i += end + 3;
        } else if rest.starts_with("<?") || rest.starts_with("<!") {
            // Processing instruction or DOCTYPE (possibly with an internal
            // subset in brackets); skipped, never expanded.
            let mut depth = 0i32;
            let mut j = i + 1;
            while j < bytes.len() {
                match bytes[j] {
                    b'[' => depth += 1,
                    b']' => depth -= 1,
                    b'>' if depth <= 0 => break,
                    _ => {}
                }
                j += 1;
            }
            if j >= bytes.len() {
                return Err(XmlError::Truncated(line));
            }
            line += count_lines(i, j);
            i = j + 1;
        } else if rest.starts_with("</") {
            let end = rest.find('>').ok_or(XmlError::Truncated(line))?;
            let name = local(rest[2..end].trim()).to_string();
            let node = stack.pop().ok_or(XmlError::Mismatch {
                line,
                expected: String::new(),
                found: name.clone(),
            })?;
            if !node.name.eq_ignore_ascii_case(&name) {
                return Err(XmlError::Mismatch {
                    line,
                    expected: node.name,
                    found: name,
                });
            }
            match stack.last_mut() {
                Some(parent) => parent.children.push(node),
                None => root = Some(node),
            }
            i += end + 1;
        } else {
            // Start tag: find its end, outside quotes.
            let mut j = i + 1;
            let mut quote = 0u8;
            while j < bytes.len() {
                let b = bytes[j];
                if quote != 0 {
                    if b == quote {
                        quote = 0;
                    }
                } else if b == b'"' || b == b'\'' {
                    quote = b;
                } else if b == b'>' {
                    break;
                }
                j += 1;
            }
            if j >= bytes.len() {
                return Err(XmlError::Truncated(line));
            }
            let inner = &text[i + 1..j];
            let self_closing = inner.ends_with('/');
            let inner = inner.trim_end_matches('/');
            let (name, attrs) = tag(inner);
            let node = Node {
                name,
                attrs,
                children: Vec::new(),
                text: String::new(),
                line,
            };
            line += count_lines(i, j);
            i = j + 1;
            if self_closing {
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None if root.is_none() => root = Some(node),
                    None => {}
                }
            } else {
                if stack.len() >= MAX_DEPTH {
                    return Err(XmlError::TooDeep(MAX_DEPTH));
                }
                stack.push(node);
            }
        }
    }
    if !stack.is_empty() {
        return Err(XmlError::Truncated(line));
    }
    root.ok_or(XmlError::NoRoot)
}

/// Splits `name a="1" b='2'` into the local name and attributes.
fn tag(inner: &str) -> (String, Vec<(String, String)>) {
    let inner = inner.trim();
    let name_end = inner
        .find(|c: char| c.is_whitespace())
        .unwrap_or(inner.len());
    let name = local(&inner[..name_end]).to_string();
    let mut attrs = Vec::new();
    let mut rest = inner[name_end..].trim_start();
    while let Some(eq) = rest.find('=') {
        let key = local(rest[..eq].trim()).to_string();
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
