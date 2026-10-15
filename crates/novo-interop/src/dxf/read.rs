//! ASCII DXF reader. See `docs/design/interop.md` §4.

use std::collections::BTreeMap;

use thiserror::Error;

use super::model::{Drawing, Entity, EntityKind, HAlign, Layer, Point, TextAlign, VAlign};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DxfError {
    #[error("this is a binary DXF file; save it as ASCII DXF and import that")]
    Binary,
    #[error("not a DXF file: no sections found")]
    NotDxf,
    #[error("the DXF file is cut off at line {0}")]
    Truncated(usize),
    #[error("line {line}: the group code `{text}` is not a number")]
    BadCode { line: usize, text: String },
}

/// What a read produced.
#[derive(Debug, Clone, PartialEq)]
pub struct DxfRead {
    /// Flat drawing, blocks exploded, in the file's own units
    /// (`mm_per_unit` from `$INSUNITS`).
    pub drawing: Drawing,
    /// Entity types that were not imported, with how many of each
    /// (`SPLINE`, `HATCH`, ...).
    pub skipped: BTreeMap<String, usize>,
    /// Things worth telling the user that are not whole entities skipped.
    pub warnings: Vec<String>,
}

type Pair = (i32, String);

const SENTINEL: &[u8] = b"AutoCAD Binary DXF";
const MAX_BLOCK_DEPTH: usize = 8;

/// Reads ASCII DXF bytes. UTF-8 when valid, else Windows-1252.
pub fn read_dxf(bytes: &[u8]) -> Result<DxfRead, DxfError> {
    if bytes.starts_with(SENTINEL) {
        return Err(DxfError::Binary);
    }
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s.trim_start_matches('\u{feff}').to_string(),
        Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
    };
    let pairs = pairs(&text)?;
    if !pairs.iter().any(|(c, v)| *c == 0 && v == "SECTION") {
        return Err(DxfError::NotDxf);
    }
    let mut reader = Reader::default();
    reader.sections(&pairs);
    Ok(reader.finish())
}

fn pairs(text: &str) -> Result<Vec<Pair>, DxfError> {
    let mut lines: Vec<&str> = text.lines().collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    let mut out = Vec::with_capacity(lines.len() / 2);
    let mut i = 0;
    while i < lines.len() {
        let code_text = lines[i].trim();
        let code: i32 = code_text.parse().map_err(|_| DxfError::BadCode {
            line: i + 1,
            text: code_text.to_string(),
        })?;
        let value = lines.get(i + 1).ok_or(DxfError::Truncated(i + 1))?;
        out.push((
            code,
            value.trim_end_matches(['\r', '\n']).trim().to_string(),
        ));
        i += 2;
    }
    Ok(out)
}

/// Windows-1252 byte to char.
fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9f => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// One entity's raw groups, in file order (`POLYLINE` carries its vertices).
#[derive(Debug, Default, Clone)]
struct Raw {
    kind: String,
    groups: Vec<Pair>,
    vertices: Vec<Vec<Pair>>,
}

impl Raw {
    fn get(&self, code: i32) -> Option<&str> {
        self.groups
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, v)| v.as_str())
    }
    fn f(&self, code: i32) -> Option<f64> {
        self.get(code).and_then(|v| v.parse().ok())
    }
    fn fd(&self, code: i32, default: f64) -> f64 {
        self.f(code).unwrap_or(default)
    }
    fn layer(&self) -> String {
        self.get(8).unwrap_or("0").to_string()
    }
    fn color(&self) -> Option<u16> {
        let c: i32 = self.get(62)?.parse().ok()?;
        // 0 is by block, 256 by layer, negative is a layer switched off.
        (1..256).contains(&c).then_some(c as u16)
    }
}

fn group_f(group: &[Pair], code: i32) -> Option<f64> {
    group
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, v)| v.parse().ok())
}

#[derive(Default)]
struct Block {
    base: Point,
    entities: Vec<Raw>,
}

#[derive(Default)]
struct Reader {
    mm_per_unit: f64,
    layers: Vec<Layer>,
    blocks: BTreeMap<String, Block>,
    entities: Vec<Raw>,
    skipped: BTreeMap<String, usize>,
    warnings: Vec<String>,
}

impl Reader {
    fn sections(&mut self, pairs: &[Pair]) {
        self.mm_per_unit = 1.0;
        let mut i = 0;
        while i < pairs.len() {
            if pairs[i] == (0, "SECTION".into()) && i + 1 < pairs.len() && pairs[i + 1].0 == 2 {
                let name = pairs[i + 1].1.clone();
                let start = i + 2;
                let mut end = start;
                while end < pairs.len() && pairs[end] != (0, "ENDSEC".into()) {
                    end += 1;
                }
                let body = &pairs[start..end.min(pairs.len())];
                match name.as_str() {
                    "HEADER" => self.header(body),
                    "TABLES" => self.tables(body),
                    "BLOCKS" => self.blocks(body),
                    "ENTITIES" => self.entities = raw_entities(body),
                    _ => {}
                }
                i = end;
            }
            i += 1;
        }
    }

    fn header(&mut self, body: &[Pair]) {
        let mut i = 0;
        while i < body.len() {
            if body[i] == (9, "$INSUNITS".into())
                && let Some((70, v)) = body.get(i + 1)
            {
                {
                    self.mm_per_unit = match v.trim() {
                        "1" => 25.4,
                        "2" => 304.8,
                        "3" => 1609344.0,
                        "5" => 10.0,
                        "6" => 1000.0,
                        "7" => 1_000_000.0,
                        "8" => 25.4e-6,
                        "9" => 0.0254,
                        "10" => 914.4,
                        "13" => 0.001,
                        "14" => 100.0,
                        _ => 1.0,
                    };
                }
            }
            i += 1;
        }
    }

    fn tables(&mut self, body: &[Pair]) {
        let mut i = 0;
        while i < body.len() {
            if body[i] == (0, "LAYER".into()) {
                let mut name = None;
                let mut color = 7i32;
                let mut flags = 0i32;
                i += 1;
                while i < body.len() && body[i].0 != 0 {
                    match body[i].0 {
                        2 => name = Some(body[i].1.clone()),
                        62 => color = body[i].1.parse().unwrap_or(7),
                        70 => flags = body[i].1.parse().unwrap_or(0),
                        _ => {}
                    }
                    i += 1;
                }
                if let Some(name) = name {
                    self.layers.push(Layer {
                        name,
                        color: color.unsigned_abs().clamp(1, 255) as u16,
                        hidden: color < 0 || flags & 1 != 0,
                    });
                }
            } else {
                i += 1;
            }
        }
    }

    fn blocks(&mut self, body: &[Pair]) {
        let mut i = 0;
        while i < body.len() {
            if body[i] == (0, "BLOCK".into()) {
                let mut j = i + 1;
                while j < body.len() && body[j].0 != 0 {
                    j += 1;
                }
                let head = &body[i + 1..j];
                let name = head
                    .iter()
                    .find(|(c, _)| *c == 2)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                let base = Point::new(
                    group_f(head, 10).unwrap_or(0.0),
                    group_f(head, 20).unwrap_or(0.0),
                );
                let mut k = j;
                while k < body.len() && body[k] != (0, "ENDBLK".into()) {
                    k += 1;
                }
                let entities = raw_entities(&body[j..k]);
                self.blocks.insert(name, Block { base, entities });
                i = k;
            }
            i += 1;
        }
    }

    fn finish(mut self) -> DxfRead {
        let raws = std::mem::take(&mut self.entities);
        let mut out = Vec::new();
        let ident = Xf::identity();
        for r in &raws {
            self.convert(r, &ident, None, 0, &mut out);
        }
        let mut layers = std::mem::take(&mut self.layers);
        for e in &out {
            if !layers.iter().any(|l| l.name == e.layer) {
                layers.push(Layer::new(e.layer.clone(), 7));
            }
        }
        DxfRead {
            drawing: Drawing {
                mm_per_unit: self.mm_per_unit,
                layers,
                entities: out,
            },
            skipped: self.skipped,
            warnings: self.warnings,
        }
    }

    fn skip(&mut self, kind: &str) {
        *self.skipped.entry(kind.to_string()).or_insert(0) += 1;
    }

    fn convert(
        &mut self,
        r: &Raw,
        xf: &Xf,
        insert_layer: Option<&str>,
        depth: usize,
        out: &mut Vec<Entity>,
    ) {
        let layer = match (r.layer().as_str(), insert_layer) {
            ("0", Some(l)) => l.to_string(),
            (l, _) => l.to_string(),
        };
        let color = r.color();
        let mut emit = |kind: EntityKind| {
            out.push(Entity {
                layer: layer.clone(),
                color,
                kind,
            });
        };
        let pt = |x: f64, y: f64| xf.apply(Point::new(x, y));
        match r.kind.as_str() {
            "LINE" => emit(EntityKind::Line {
                a: pt(r.fd(10, 0.0), r.fd(20, 0.0)),
                b: pt(r.fd(11, 0.0), r.fd(21, 0.0)),
            }),
            "CIRCLE" => {
                let radius = r.fd(40, 0.0) * xf.scale();
                emit(EntityKind::Circle {
                    center: pt(r.fd(10, 0.0), r.fd(20, 0.0)),
                    radius,
                });
            }
            "ARC" => emit(xf.arc(
                Point::new(r.fd(10, 0.0), r.fd(20, 0.0)),
                r.fd(40, 0.0),
                r.fd(50, 0.0),
                r.fd(51, 360.0),
            )),
            "LWPOLYLINE" => {
                let mut verts: Vec<(Point, f64)> = Vec::new();
                for (c, v) in &r.groups {
                    let Ok(n) = v.parse::<f64>() else { continue };
                    match c {
                        10 => verts.push((Point::new(n, 0.0), 0.0)),
                        20 => {
                            if let Some(l) = verts.last_mut() {
                                l.0.y = n;
                            }
                        }
                        42 => {
                            if let Some(l) = verts.last_mut() {
                                l.1 = n;
                            }
                        }
                        _ => {}
                    }
                }
                let closed = r.get(70).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) & 1 == 1;
                polyline(&verts, closed, xf, &mut emit);
            }
            "POLYLINE" => {
                let flags = r.get(70).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
                if flags & (8 | 16 | 64) != 0 {
                    // 3D polyline, polygon mesh, polyface mesh.
                    self.skip("POLYLINE (3D/mesh)");
                    return;
                }
                let verts: Vec<(Point, f64)> = r
                    .vertices
                    .iter()
                    .map(|v| {
                        (
                            Point::new(
                                group_f(v, 10).unwrap_or(0.0),
                                group_f(v, 20).unwrap_or(0.0),
                            ),
                            group_f(v, 42).unwrap_or(0.0),
                        )
                    })
                    .collect();
                polyline(&verts, flags & 1 == 1, xf, &mut emit);
            }
            "TEXT" | "ATTRIB" => {
                let hv = r.get(72).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
                let vv = r.get(73).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
                // Alignment point (11) when aligned, else the insertion point.
                let aligned = (hv != 0 || vv != 0) && r.f(11).is_some();
                let (x, y) = if aligned {
                    (r.fd(11, 0.0), r.fd(21, 0.0))
                } else {
                    (r.fd(10, 0.0), r.fd(20, 0.0))
                };
                let text = decode_text(r.get(1).unwrap_or(""));
                if text.trim().is_empty() {
                    return;
                }
                emit(EntityKind::Text {
                    pos: pt(x, y),
                    height: r.fd(40, 2.5) * xf.scale(),
                    rotation_deg: r.fd(50, 0.0) + xf.rotation_deg(),
                    text,
                    align: TextAlign {
                        h: match hv {
                            1 | 4 => HAlign::Center,
                            2 => HAlign::Right,
                            _ => HAlign::Left,
                        },
                        v: match vv {
                            1 => VAlign::Bottom,
                            2 => VAlign::Middle,
                            3 => VAlign::Top,
                            _ => VAlign::Baseline,
                        },
                    },
                });
            }
            "MTEXT" => {
                // Long text arrives as code 3 chunks followed by code 1.
                let mut raw = String::new();
                for (c, v) in &r.groups {
                    if *c == 3 || *c == 1 {
                        raw.push_str(v);
                    }
                }
                let text = mtext_to_plain(&raw);
                if text.trim().is_empty() {
                    return;
                }
                let attach = r.get(71).and_then(|v| v.parse::<i32>().ok()).unwrap_or(1);
                // Attachment 1..9: row top/middle/bottom, column left/center/right.
                let col = (attach - 1).rem_euclid(3);
                let row = (attach - 1) / 3;
                // Direction vector (11, 21) overrides the rotation.
                let rot = match (r.f(11), r.f(21)) {
                    (Some(dx), Some(dy)) if dx != 0.0 || dy != 0.0 => dy.atan2(dx).to_degrees(),
                    _ => r.fd(50, 0.0),
                };
                emit(EntityKind::Text {
                    pos: pt(r.fd(10, 0.0), r.fd(20, 0.0)),
                    height: r.fd(40, 2.5) * xf.scale(),
                    rotation_deg: rot + xf.rotation_deg(),
                    text,
                    align: TextAlign {
                        h: [HAlign::Left, HAlign::Center, HAlign::Right][col as usize],
                        v: [VAlign::Top, VAlign::Middle, VAlign::Bottom][row.clamp(0, 2) as usize],
                    },
                });
            }
            "INSERT" => {
                let name = r.get(2).unwrap_or("").to_string();
                if depth >= MAX_BLOCK_DEPTH {
                    self.warnings
                        .push(format!("block `{name}` is nested too deep and was skipped"));
                    return;
                }
                let Some(block) = self.blocks.get(&name) else {
                    self.warnings
                        .push(format!("block `{name}` is used but not defined"));
                    return;
                };
                let base = block.base;
                let inner = block.entities.clone();
                let sx = r.fd(41, 1.0);
                let sy = r.fd(42, sx);
                let place = Xf::insert(
                    base,
                    Point::new(r.fd(10, 0.0), r.fd(20, 0.0)),
                    sx,
                    sy,
                    r.fd(50, 0.0),
                );
                let total = xf.then(&place);
                let own = r.layer();
                for e in &inner {
                    self.convert(e, &total, Some(&own), depth + 1, out);
                }
            }
            "VERTEX" | "SEQEND" | "ATTDEF" | "VIEWPORT" | "POINT" => {}
            other => self.skip(other),
        }
    }
}

fn raw_entities(body: &[Pair]) -> Vec<Raw> {
    let mut out: Vec<Raw> = Vec::new();
    let mut i = 0;
    while i < body.len() {
        if body[i].0 == 0 {
            let kind = body[i].1.clone();
            let mut j = i + 1;
            while j < body.len() && body[j].0 != 0 {
                j += 1;
            }
            let groups = body[i + 1..j].to_vec();
            match kind.as_str() {
                "VERTEX" => {
                    if let Some(last) = out.last_mut().filter(|l| l.kind == "POLYLINE") {
                        last.vertices.push(groups);
                    }
                }
                "SEQEND" => {}
                _ => out.push(Raw {
                    kind,
                    groups,
                    vertices: Vec::new(),
                }),
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Emits a polyline: one `Polyline` when no segment bulges, else lines and
/// arcs so rounded corners stay round.
fn polyline(verts: &[(Point, f64)], closed: bool, xf: &Xf, emit: &mut impl FnMut(EntityKind)) {
    if verts.len() < 2 {
        return;
    }
    if verts.iter().all(|v| v.1 == 0.0) {
        emit(EntityKind::Polyline {
            points: verts.iter().map(|v| xf.apply(v.0)).collect(),
            closed,
        });
        return;
    }
    let n = verts.len();
    let segments = if closed { n } else { n - 1 };
    for k in 0..segments {
        let (p1, bulge) = verts[k];
        let p2 = verts[(k + 1) % n].0;
        match bulge_arc(p1, p2, bulge) {
            Some((center, radius, start, end)) => emit(xf.arc(center, radius, start, end)),
            None => emit(EntityKind::Line {
                a: xf.apply(p1),
                b: xf.apply(p2),
            }),
        }
    }
}

/// Centre, radius and CCW start/end angles (degrees) of the arc a bulge
/// describes between two vertices; `None` for a straight segment.
fn bulge_arc(p1: Point, p2: Point, bulge: f64) -> Option<(Point, f64, f64, f64)> {
    let chord = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt();
    if bulge == 0.0 || chord < 1e-12 {
        return None;
    }
    let theta = 4.0 * bulge.atan();
    let half = theta / 2.0;
    let ux = (p2.x - p1.x) / chord;
    let uy = (p2.y - p1.y) / chord;
    let offset = (chord / 2.0) / half.tan();
    let center = Point::new(
        (p1.x + p2.x) / 2.0 - uy * offset,
        (p1.y + p2.y) / 2.0 + ux * offset,
    );
    let radius = (chord / (2.0 * half.sin())).abs();
    let ang = |p: Point| (p.y - center.y).atan2(p.x - center.x).to_degrees();
    if bulge > 0.0 {
        Some((center, radius, ang(p1), ang(p2)))
    } else {
        Some((center, radius, ang(p2), ang(p1)))
    }
}

/// A 2D similarity transform (scale, optional mirror, rotation, shift).
#[derive(Debug, Clone, Copy)]
struct Xf {
    // x' = a*x + c*y + e ; y' = b*x + d*y + f
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Xf {
    fn identity() -> Self {
        Xf {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Block coordinates to world: move `base` to the origin, scale,
    /// rotate, then place at `at`.
    fn insert(base: Point, at: Point, sx: f64, sy: f64, rot_deg: f64) -> Self {
        let (s, c) = rot_deg.to_radians().sin_cos();
        let a = c * sx;
        let b = s * sx;
        let cc = -s * sy;
        let d = c * sy;
        Xf {
            a,
            b,
            c: cc,
            d,
            e: at.x - (a * base.x + cc * base.y),
            f: at.y - (b * base.x + d * base.y),
        }
    }

    /// `self` applied after `inner`.
    fn then(&self, inner: &Xf) -> Xf {
        Xf {
            a: self.a * inner.a + self.c * inner.b,
            b: self.b * inner.a + self.d * inner.b,
            c: self.a * inner.c + self.c * inner.d,
            d: self.b * inner.c + self.d * inner.d,
            e: self.a * inner.e + self.c * inner.f + self.e,
            f: self.b * inner.e + self.d * inner.f + self.f,
        }
    }

    fn apply(&self, p: Point) -> Point {
        Point::new(
            self.a * p.x + self.c * p.y + self.e,
            self.b * p.x + self.d * p.y + self.f,
        )
    }

    fn det(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    /// Length scale (uniform part).
    fn scale(&self) -> f64 {
        self.det().abs().sqrt()
    }

    fn rotation_deg(&self) -> f64 {
        self.b.atan2(self.a).to_degrees()
    }

    /// An arc through the transform; a mirror swaps start and end.
    fn arc(&self, center: Point, radius: f64, start: f64, end: f64) -> EntityKind {
        if (self.a, self.b, self.c, self.d, self.e, self.f) == (1.0, 0.0, 0.0, 1.0, 0.0, 0.0) {
            return EntityKind::Arc {
                center,
                radius,
                start_deg: start,
                end_deg: end,
            };
        }
        let at = |deg: f64| {
            let r = deg.to_radians();
            Point::new(center.x + radius * r.cos(), center.y + radius * r.sin())
        };
        let c = self.apply(center);
        let ang = |p: Point| (p.y - c.y).atan2(p.x - c.x).to_degrees();
        let (s, e) = (ang(self.apply(at(start))), ang(self.apply(at(end))));
        let (s, e) = if self.det() < 0.0 { (e, s) } else { (s, e) };
        EntityKind::Arc {
            center: c,
            radius: radius * self.scale(),
            start_deg: s.rem_euclid(360.0),
            end_deg: e.rem_euclid(360.0),
        }
    }
}

/// `%%d`, `%%p`, `%%c` shortcuts and `\U+XXXX` escapes.
fn decode_text(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 2 < chars.len() && chars[i + 1] == '%' {
            match chars[i + 2].to_ascii_lowercase() {
                'd' => out.push('°'),
                'p' => out.push('±'),
                'c' => out.push('⌀'),
                '%' => out.push('%'),
                _ => {
                    out.push_str("%%");
                    out.push(chars[i + 2]);
                }
            }
            i += 3;
        } else if chars[i] == '\\'
            && i + 6 < chars.len()
            && chars[i + 1].eq_ignore_ascii_case(&'u')
            && chars[i + 2] == '+'
        {
            let hex: String = chars[i + 3..i + 7].iter().collect();
            match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                Some(c) => {
                    out.push(c);
                    i += 7;
                }
                None => {
                    out.push('\\');
                    i += 1;
                }
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// MTEXT markup to plain text with `\n` line breaks.
fn mtext_to_plain(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' if i + 1 < chars.len() => {
                let n = chars[i + 1];
                match n {
                    'P' | 'p' => {
                        out.push('\n');
                        i += 2;
                    }
                    '~' => {
                        out.push(' ');
                        i += 2;
                    }
                    '\\' | '{' | '}' => {
                        out.push(n);
                        i += 2;
                    }
                    'U' | 'u' if chars.get(i + 2) == Some(&'+') => {
                        // Leave \U+XXXX for decode_text.
                        out.push('\\');
                        i += 1;
                    }
                    'f' | 'F' | 'H' | 'C' | 'c' | 'Q' | 'T' | 'W' | 'A' | 'S' => {
                        // Parameterised codes end at `;`.
                        let mut j = i + 2;
                        while j < chars.len() && chars[j] != ';' {
                            j += 1;
                        }
                        i = j + 1;
                    }
                    'L' | 'l' | 'O' | 'o' | 'K' | 'k' => i += 2,
                    _ => {
                        out.push(n);
                        i += 2;
                    }
                }
            }
            '{' | '}' => i += 1,
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    decode_text(&out)
}
