//! A structure survey of a PXF file: which object kinds, elements and
//! attribute names occur and how often, with the *shape* of values (digits
//! as `0`, letters as `a`) and never the values themselves. A user can run it
//! on an export that must not leave their machine and share the output, so
//! the field table can be checked against real files without the content.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::BufRead;

use super::PxfError;
use super::stream::{Event, Tokens};

/// Distinct value shapes kept per attribute.
const SHAPES: usize = 3;
/// Longest shape kept (longer values are cut and end in `…`).
const SHAPE_LEN: usize = 24;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct AttrStat {
    count: usize,
    shapes: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ElemStat {
    count: usize,
    attrs: BTreeMap<String, AttrStat>,
}

/// The result of [`survey_pxf`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PxfSurvey {
    /// Object kind (`O<n>`) to its own attributes and the elements inside it.
    kinds: BTreeMap<u32, (ElemStat, BTreeMap<String, ElemStat>)>,
    /// Elements outside any object.
    pub outside: usize,
}

fn shape(v: &str) -> String {
    let mut s: String = v
        .chars()
        .take(SHAPE_LEN)
        .map(|c| {
            if c.is_ascii_digit() {
                '0'
            } else if c.is_alphabetic() {
                'a'
            } else if c.is_whitespace() {
                ' '
            } else {
                c
            }
        })
        .collect();
    if v.chars().count() > SHAPE_LEN {
        s.push('…');
    }
    s
}

fn note(stat: &mut ElemStat, attrs: &[(String, String)]) {
    stat.count += 1;
    for (k, v) in attrs {
        let a = stat.attrs.entry(k.clone()).or_default();
        a.count += 1;
        let sh = shape(v);
        if a.shapes.len() < SHAPES && !a.shapes.contains(&sh) {
            a.shapes.push(sh);
        }
    }
}

impl PxfSurvey {
    /// The object kinds found, with how many of each.
    pub fn kinds(&self) -> impl Iterator<Item = (u32, usize)> + '_ {
        self.kinds.iter().map(|(k, (o, _))| (*k, o.count))
    }

    /// Plain text, one block per object kind.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (kind, (own, elems)) in &self.kinds {
            let _ = writeln!(out, "O{kind}  x{}", own.count);
            write_attrs(&mut out, "  ", &own.attrs);
            for (name, e) in elems {
                let _ = writeln!(out, "  {name}  x{}", e.count);
                write_attrs(&mut out, "    ", &e.attrs);
            }
        }
        out
    }
}

fn write_attrs(out: &mut String, indent: &str, attrs: &BTreeMap<String, AttrStat>) {
    for (k, a) in attrs {
        let _ = writeln!(out, "{indent}{k}  x{}  [{}]", a.count, a.shapes.join(" | "));
    }
}

/// Surveys a PXF stream. Nothing but names, counts and value shapes is kept.
pub fn survey_pxf<R: BufRead>(r: R) -> Result<PxfSurvey, PxfError> {
    let mut tokens = Tokens::new(r);
    let mut out = PxfSurvey::default();
    // Open elements: Some(kind) for an object.
    let mut stack: Vec<Option<u32>> = Vec::new();
    let mut started = false;
    while let Some(ev) = tokens.next_event()? {
        match ev {
            Event::Start { name, attrs } => {
                if !started {
                    if name != "EplanPxfRoot" {
                        return Err(PxfError::NotPxf(name));
                    }
                    started = true;
                    stack.push(None);
                    continue;
                }
                let kind = name.strip_prefix('O').and_then(|n| n.parse::<u32>().ok());
                match kind {
                    Some(k) => {
                        note(&mut out.kinds.entry(k).or_default().0, &attrs);
                    }
                    None => {
                        if let Some(k) = stack.iter().rev().find_map(|f| *f) {
                            note(
                                out.kinds.entry(k).or_default().1.entry(name).or_default(),
                                &attrs,
                            );
                        } else {
                            out.outside += 1;
                        }
                    }
                }
                stack.push(kind);
            }
            Event::End { .. } => {
                stack.pop();
                if stack.is_empty() {
                    break;
                }
            }
            Event::Text(_) => {}
        }
    }
    if !started {
        return Err(PxfError::Empty);
    }
    if !stack.is_empty() {
        return Err(PxfError::Truncated(tokens.line()));
    }
    Ok(out)
}
