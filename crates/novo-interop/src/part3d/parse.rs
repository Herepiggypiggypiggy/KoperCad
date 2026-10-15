//! ISO 10303-21 (STEP physical file) reader.
//!
//! One pass finds where each `#id=...;` statement starts and ends and what
//! it is called; the parameters of a statement are only parsed when
//! something asks for that entity, so a file with a million entities costs
//! one scan plus the entities the B-rep actually reaches.

use std::collections::HashMap;

/// A parameter value.
#[derive(Debug, Clone, PartialEq)]
pub enum Param {
    Ref(u32),
    Real(f64),
    Int(i64),
    Text(String),
    Enum(String),
    List(Vec<Param>),
    Typed(String, Vec<Param>),
    /// `$` (unset) or `*` (derived).
    Null,
}

impl Param {
    pub fn as_ref(&self) -> Option<u32> {
        match self {
            Param::Ref(r) => Some(*r),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Param::Real(v) => Some(*v),
            Param::Int(v) => Some(*v as f64),
            Param::Typed(_, v) if v.len() == 1 => v[0].as_f64(),
            _ => None,
        }
    }
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Param::Int(v) => Some(*v),
            Param::Real(v) => Some(*v as i64),
            _ => None,
        }
    }
    pub fn as_list(&self) -> Option<&[Param]> {
        match self {
            Param::List(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Param::Text(v) => Some(v),
            _ => None,
        }
    }
    /// `.T.` / `.F.` as a bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Param::Enum(e) if e == "T" => Some(true),
            Param::Enum(e) if e == "F" => Some(false),
            _ => None,
        }
    }
    pub fn refs(&self) -> Vec<u32> {
        self.as_list()
            .map(|l| l.iter().filter_map(Param::as_ref).collect())
            .unwrap_or_default()
    }
}

/// A parsed entity: one `(NAME, args)` part, or several for a complex
/// entity such as `#5=(A() B(1) C(2));`.
#[derive(Debug, Clone)]
pub struct Entity {
    pub parts: Vec<(String, Vec<Param>)>,
}

impl Entity {
    /// Arguments of the part called `name` (upper case).
    pub fn part(&self, name: &str) -> Option<&[Param]> {
        self.parts
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, a)| a.as_slice())
    }
    pub fn name(&self) -> &str {
        self.parts.first().map_or("", |(n, _)| n.as_str())
    }
    pub fn args(&self) -> &[Param] {
        self.parts.first().map_or(&[], |(_, a)| a.as_slice())
    }
    pub fn has(&self, name: &str) -> bool {
        self.parts.iter().any(|(n, _)| n == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("this is not a STEP file (no ISO-10303-21 header)")]
    NotStep,
    #[error("the STEP file has no DATA section")]
    NoData,
}

/// An indexed STEP file borrowing its text.
pub struct StepFile<'a> {
    text: &'a str,
    index: HashMap<u32, (usize, usize)>,
    by_name: HashMap<String, Vec<u32>>,
}

impl<'a> StepFile<'a> {
    pub fn parse(text: &'a str) -> Result<StepFile<'a>, ParseError> {
        let head = text.trim_start_matches('\u{feff}').trim_start();
        if !head.starts_with("ISO-10303-21") {
            return Err(ParseError::NotStep);
        }
        let bytes = text.as_bytes();
        let mut pos = find_data(bytes).ok_or(ParseError::NoData)?;
        let mut index = HashMap::new();
        let mut by_name: HashMap<String, Vec<u32>> = HashMap::new();
        let n = bytes.len();
        while pos < n {
            pos = skip_ws_comments(bytes, pos);
            if pos >= n {
                break;
            }
            if bytes[pos] != b'#' {
                // ENDSEC; or a stray token: stop at ENDSEC.
                if bytes[pos..].len() >= 6 && bytes[pos..pos + 6].eq_ignore_ascii_case(b"ENDSEC") {
                    break;
                }
                // Skip to the next ';'.
                pos = statement_end(bytes, pos) + 1;
                continue;
            }
            let mut i = pos + 1;
            let mut id = 0u64;
            while i < n && bytes[i].is_ascii_digit() {
                id = id * 10 + u64::from(bytes[i] - b'0');
                i += 1;
            }
            i = skip_ws_comments(bytes, i);
            if i >= n || bytes[i] != b'=' {
                pos = statement_end(bytes, pos) + 1;
                continue;
            }
            i = skip_ws_comments(bytes, i + 1);
            let end = statement_end(bytes, i);
            let id = id as u32;
            index.insert(id, (i, end));
            if bytes.get(i) == Some(&b'(') {
                for name in complex_names(&text[i..end]) {
                    by_name.entry(name).or_default().push(id);
                }
            } else {
                let mut j = i;
                while j < end && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                by_name
                    .entry(text[i..j].to_ascii_uppercase())
                    .or_default()
                    .push(id);
            }
            pos = end + 1;
        }
        Ok(StepFile {
            text,
            index,
            by_name,
        })
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Ids of every entity that has a part called `name` (upper case).
    pub fn ids_of(&self, name: &str) -> &[u32] {
        self.by_name.get(name).map_or(&[], Vec::as_slice)
    }

    /// Parses entity `id` (None if missing or malformed).
    pub fn entity(&self, id: u32) -> Option<Entity> {
        let &(s, e) = self.index.get(&id)?;
        let mut p = Parser {
            b: self.text.as_bytes(),
            i: s,
            end: e,
        };
        p.entity()
    }
}

fn find_data(b: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i + 5 <= b.len() {
        if b[i..i + 4].eq_ignore_ascii_case(b"DATA") {
            let mut j = i + 4;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if b.get(j) == Some(&b';') && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
                return Some(j + 1);
            }
        }
        i += 1;
    }
    None
}

fn skip_ws_comments(b: &[u8], mut i: usize) -> usize {
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
        } else {
            return i;
        }
    }
}

/// Index of the `;` that ends the statement starting at `i` (quotes and
/// comments respected), or the text length.
fn statement_end(b: &[u8], mut i: usize) -> usize {
    while i < b.len() {
        match b[i] {
            b'\'' => {
                i += 1;
                while i < b.len() {
                    if b[i] == b'\'' {
                        if b.get(i + 1) == Some(&b'\'') {
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 1;
            }
            b';' => return i,
            _ => {}
        }
        i += 1;
    }
    b.len()
}

/// Names of the parts of a complex entity body `(A(..) B(..))`.
fn complex_names(body: &str) -> Vec<String> {
    let b = body.as_bytes();
    let mut names = Vec::new();
    let mut depth = 0i32;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\'' => {
                i += 1;
                while i < b.len() && b[i] != b'\'' {
                    i += 1;
                }
            }
            b'(' => depth += 1,
            b')' => depth -= 1,
            c if depth == 1 && (c.is_ascii_alphabetic() || c == b'_') => {
                let s = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                names.push(body[s..i].to_ascii_uppercase());
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    names
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
    end: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        self.i = skip_ws_comments(self.b, self.i).min(self.end);
    }

    fn peek(&self) -> Option<u8> {
        if self.i < self.end {
            Some(self.b[self.i])
        } else {
            None
        }
    }

    fn ident(&mut self) -> String {
        let s = self.i;
        while self.i < self.end
            && (self.b[self.i].is_ascii_alphanumeric() || self.b[self.i] == b'_')
        {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.b[s..self.i]).to_ascii_uppercase()
    }

    fn entity(&mut self) -> Option<Entity> {
        self.ws();
        if self.peek()? == b'(' {
            self.i += 1;
            let mut parts = Vec::new();
            loop {
                self.ws();
                match self.peek()? {
                    b')' => break,
                    _ => {
                        let name = self.ident();
                        if name.is_empty() {
                            return None;
                        }
                        self.ws();
                        let args = self.args()?;
                        parts.push((name, args));
                    }
                }
            }
            Some(Entity { parts })
        } else {
            let name = self.ident();
            if name.is_empty() {
                return None;
            }
            self.ws();
            let args = if self.peek() == Some(b'(') {
                self.args()?
            } else {
                Vec::new()
            };
            Some(Entity {
                parts: vec![(name, args)],
            })
        }
    }

    /// `( p, p, ... )`
    fn args(&mut self) -> Option<Vec<Param>> {
        if self.peek()? != b'(' {
            return None;
        }
        self.i += 1;
        let mut out = Vec::new();
        loop {
            self.ws();
            match self.peek()? {
                b')' => {
                    self.i += 1;
                    return Some(out);
                }
                b',' => self.i += 1,
                _ => out.push(self.param()?),
            }
        }
    }

    fn param(&mut self) -> Option<Param> {
        self.ws();
        let c = self.peek()?;
        match c {
            b'#' => {
                self.i += 1;
                let mut v = 0u64;
                while self.i < self.end && self.b[self.i].is_ascii_digit() {
                    v = v * 10 + u64::from(self.b[self.i] - b'0');
                    self.i += 1;
                }
                Some(Param::Ref(v as u32))
            }
            b'$' | b'*' => {
                self.i += 1;
                Some(Param::Null)
            }
            b'\'' => {
                self.i += 1;
                let mut s = Vec::new();
                while self.i < self.end {
                    if self.b[self.i] == b'\'' {
                        if self.b.get(self.i + 1) == Some(&b'\'') {
                            s.push(b'\'');
                            self.i += 2;
                            continue;
                        }
                        self.i += 1;
                        break;
                    }
                    s.push(self.b[self.i]);
                    self.i += 1;
                }
                Some(Param::Text(String::from_utf8_lossy(&s).into_owned()))
            }
            b'.' if self
                .b
                .get(self.i + 1)
                .is_some_and(|c| c.is_ascii_alphabetic()) =>
            {
                self.i += 1;
                let s = self.i;
                while self.i < self.end && self.b[self.i] != b'.' {
                    self.i += 1;
                }
                let e = String::from_utf8_lossy(&self.b[s..self.i]).to_ascii_uppercase();
                self.i += 1;
                Some(Param::Enum(e))
            }
            b'(' => Some(Param::List(self.args()?)),
            c if c.is_ascii_alphabetic() => {
                let name = self.ident();
                self.ws();
                let args = self.args()?;
                Some(Param::Typed(name, args))
            }
            _ => {
                let s = self.i;
                while self.i < self.end
                    && matches!(
                        self.b[self.i],
                        b'0'..=b'9' | b'+' | b'-' | b'.' | b'e' | b'E'
                    )
                {
                    self.i += 1;
                }
                if self.i == s {
                    return None;
                }
                let t = std::str::from_utf8(&self.b[s..self.i]).ok()?;
                if t.bytes()
                    .all(|c| c.is_ascii_digit() || c == b'-' || c == b'+')
                {
                    t.parse::<i64>().ok().map(Param::Int)
                } else {
                    t.parse::<f64>().ok().map(Param::Real)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "ISO-10303-21;\nHEADER;\nFILE_NAME('a;b','',(''),(''),'','','');\nENDSEC;\nDATA;\n/* c; */\n#1=CARTESIAN_POINT('p''q',(0.,-1.5E-01,3));\n#2 = LINE ( '' , #1 , #3 ) ;\n#3=VECTOR('',#4,1.);\n#5=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\nENDSEC;\nEND-ISO-10303-21;\n";

    #[test]
    fn indexes_and_parses_lazily() {
        let f = StepFile::parse(SRC).unwrap();
        assert_eq!(f.len(), 4);
        let p = f.entity(1).unwrap();
        assert_eq!(p.name(), "CARTESIAN_POINT");
        assert_eq!(p.args()[0].as_text(), Some("p'q"));
        let c: Vec<f64> = p.args()[1]
            .as_list()
            .unwrap()
            .iter()
            .filter_map(Param::as_f64)
            .collect();
        assert_eq!(c, vec![0.0, -0.15, 3.0]);
        assert_eq!(f.entity(2).unwrap().args()[1].as_ref(), Some(1));
        assert_eq!(f.ids_of("VECTOR"), &[3]);
    }

    #[test]
    fn complex_entities_have_every_part() {
        let f = StepFile::parse(SRC).unwrap();
        let e = f.entity(5).unwrap();
        assert!(e.has("LENGTH_UNIT") && e.has("SI_UNIT"));
        assert_eq!(f.ids_of("SI_UNIT"), &[5]);
        let si = e.part("SI_UNIT").unwrap();
        assert_eq!(si[0].as_bool(), None);
        assert_eq!(si[0], Param::Enum("MILLI".into()));
    }

    #[test]
    fn rejects_other_text() {
        assert!(matches!(StepFile::parse("hello"), Err(ParseError::NotStep)));
        assert!(matches!(
            StepFile::parse("ISO-10303-21;\nHEADER;\nENDSEC;"),
            Err(ParseError::NoData)
        ));
    }
}
