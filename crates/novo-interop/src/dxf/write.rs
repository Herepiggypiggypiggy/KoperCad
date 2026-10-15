//! ASCII DXF writer, dialect AC1009 (R12). See `docs/design/interop.md` §3.

use std::fmt::Write;

use super::model::{Drawing, EntityKind, HAlign, VAlign};

/// Formats a number with up to six decimals, no trailing zeros, no `-0`.
pub(crate) fn num(v: f64) -> String {
    let mut s = format!("{v:.6}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".into() } else { s }
}

/// A layer name DXF accepts.
pub(crate) fn layer_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if matches!(
                c,
                '<' | '>' | '/' | '\\' | '"' | ':' | ';' | '?' | '*' | '|' | '=' | '`'
            ) || c.is_control()
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() { "0".into() } else { ascii(&s) }
}

/// 7-bit text: anything else becomes `\U+XXXX`.
fn ascii(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii() && !c.is_ascii_control() {
            out.push(c);
        } else if (c as u32) <= 0xFFFF {
            let _ = write!(out, "\\U+{:04X}", c as u32);
        } else {
            out.push('?');
        }
    }
    out
}

struct Out(String);

impl Out {
    fn pair(&mut self, code: u16, value: impl std::fmt::Display) {
        let _ = write!(self.0, "{code}\r\n{value}\r\n");
    }
    fn xyz(&mut self, base: u16, x: f64, y: f64) {
        self.pair(base, num(x));
        self.pair(base + 10, num(y));
        self.pair(base + 20, 0);
    }
}

/// The first line of every DXF this crate writes (a `999` comment).
pub const PRODUCER_COMMENT: &str = "Written by KoperCAD";

/// Whether `bytes` start like a DXF this crate wrote, so replacing the file
/// loses nothing the user made.
pub fn is_written_by_us(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(64)];
    head.starts_with(format!("999\r\n{PRODUCER_COMMENT}\r\n").as_bytes())
}

/// Writes `drawing` as ASCII DXF. Lengths are written as they are and
/// declared millimetres, so call [`Drawing::scaled_to_mm`] first for a
/// drawing in other units.
pub fn write_dxf(drawing: &Drawing) -> String {
    let mut o = Out(String::new());

    // A comment, so a later export can tell its own files from the user's.
    o.pair(999, PRODUCER_COMMENT);
    o.pair(0, "SECTION");
    o.pair(2, "HEADER");
    o.pair(9, "$ACADVER");
    o.pair(1, "AC1009");
    o.pair(9, "$INSUNITS");
    o.pair(70, 4);
    if let Some((min, max)) = drawing.bounds() {
        o.pair(9, "$EXTMIN");
        o.xyz(10, min.x, min.y);
        o.pair(9, "$EXTMAX");
        o.xyz(10, max.x, max.y);
    }
    o.pair(0, "ENDSEC");

    // Layers: "0" first, then the drawing's, then any an entity uses.
    let mut layers: Vec<(String, u16, bool)> = vec![("0".into(), 7, false)];
    let mut add = |name: String, color: u16, hidden: bool| {
        if !layers.iter().any(|l| l.0 == name) {
            layers.push((name, color, hidden));
        }
    };
    for l in &drawing.layers {
        add(layer_name(&l.name), l.color, l.hidden);
    }
    for e in &drawing.entities {
        add(layer_name(&e.layer), 7, false);
    }

    o.pair(0, "SECTION");
    o.pair(2, "TABLES");
    o.pair(0, "TABLE");
    o.pair(2, "LTYPE");
    o.pair(70, 1);
    o.pair(0, "LTYPE");
    o.pair(2, "CONTINUOUS");
    o.pair(70, 0);
    o.pair(3, "Solid line");
    o.pair(72, 65);
    o.pair(73, 0);
    o.pair(40, 0);
    o.pair(0, "ENDTAB");
    o.pair(0, "TABLE");
    o.pair(2, "LAYER");
    o.pair(70, layers.len());
    for (name, color, hidden) in &layers {
        o.pair(0, "LAYER");
        o.pair(2, name);
        o.pair(70, 0);
        o.pair(
            62,
            if *hidden {
                -(*color as i32)
            } else {
                *color as i32
            },
        );
        o.pair(6, "CONTINUOUS");
    }
    o.pair(0, "ENDTAB");
    o.pair(0, "ENDSEC");

    o.pair(0, "SECTION");
    o.pair(2, "ENTITIES");
    for e in &drawing.entities {
        let layer = layer_name(&e.layer);
        let head = |o: &mut Out, kind: &str| {
            o.pair(0, kind);
            o.pair(8, &layer);
            if let Some(c) = e.color {
                o.pair(62, c);
            }
        };
        match &e.kind {
            EntityKind::Line { a, b } => {
                head(&mut o, "LINE");
                o.xyz(10, a.x, a.y);
                o.xyz(11, b.x, b.y);
            }
            EntityKind::Polyline { points, closed } => {
                head(&mut o, "POLYLINE");
                o.pair(66, 1);
                o.xyz(10, 0.0, 0.0);
                o.pair(70, u8::from(*closed));
                for p in points {
                    o.pair(0, "VERTEX");
                    o.pair(8, &layer);
                    o.xyz(10, p.x, p.y);
                }
                o.pair(0, "SEQEND");
                o.pair(8, &layer);
            }
            EntityKind::Circle { center, radius } => {
                head(&mut o, "CIRCLE");
                o.xyz(10, center.x, center.y);
                o.pair(40, num(*radius));
            }
            EntityKind::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                head(&mut o, "ARC");
                o.xyz(10, center.x, center.y);
                o.pair(40, num(*radius));
                o.pair(50, num(start_deg.rem_euclid(360.0)));
                o.pair(51, num(end_deg.rem_euclid(360.0)));
            }
            EntityKind::Text {
                pos,
                height,
                rotation_deg,
                text,
                align,
            } => {
                let h = match align.h {
                    HAlign::Left => 0,
                    HAlign::Center => 1,
                    HAlign::Right => 2,
                };
                let v = match align.v {
                    VAlign::Baseline => 0,
                    VAlign::Bottom => 1,
                    VAlign::Middle => 2,
                    VAlign::Top => 3,
                };
                let r = rotation_deg.to_radians();
                // R12 TEXT is one line: further lines step down the text box.
                for (i, line) in text.lines().enumerate() {
                    let step = i as f64 * height * 1.4;
                    let p = (pos.x + r.sin() * step, pos.y - r.cos() * step);
                    head(&mut o, "TEXT");
                    o.xyz(10, p.0, p.1);
                    o.pair(40, num(*height));
                    o.pair(1, ascii(line));
                    if *rotation_deg != 0.0 {
                        o.pair(50, num(*rotation_deg));
                    }
                    if h != 0 || v != 0 {
                        o.pair(72, h);
                        o.xyz(11, p.0, p.1);
                        o.pair(73, v);
                    }
                }
            }
        }
    }
    o.pair(0, "ENDSEC");
    o.pair(0, "EOF");
    o.0
}
