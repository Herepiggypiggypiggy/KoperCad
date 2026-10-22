//! Text conflict finder for drawn pages.
//!
//! Builds the same page scene the canvas draws (`PageScene`) and checks every
//! text box in it against every other text box and against every drawn line,
//! in millimetres, without a window. The check is geometric: a text box is the
//! estimated box the canvas itself uses for hit testing (`Label::corners`, glyph
//! advance 0.6 of the height), so a real font can be a little narrower or wider.
//! It answers "do these two things share paper", which is what the pictures
//! then confirm.
//!
//! Each conflict names the two parties by role (device tag, pin number, wire
//! number, terminal name, strip name, note, symbol art, frame...) so findings group
//! by cause: which text element ran into which.

use std::collections::BTreeMap;

use novo_canvas::model::{PageScene, SymbolArt, SymbolNote, symbol_text};
use novo_canvas::{Label, Scene, Vec2};
use novo_core::{AnyId, PageId, Project, SymbolRole};

/// Penetration below this (mm) is two boxes touching, not overlapping.
pub const TOUCH_MM: f32 = 0.3;

/// What a piece of text is, from the model it was drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    DeviceTag,
    TerminalName,
    FeedName,
    InterruptionName,
    PinName,
    WireNumber,
    CableLabel,
    StripName,
    Note,
    Annotation,
    SymbolArt,
    Frame,
    Other,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::DeviceTag => "device tag",
            Role::TerminalName => "terminal name",
            Role::FeedName => "potential feed name",
            Role::InterruptionName => "interruption name",
            Role::PinName => "pin name",
            Role::WireNumber => "wire number",
            Role::CableLabel => "cable or core label",
            Role::StripName => "strip name",
            Role::Note => "symbol note (description, xref, part)",
            Role::Annotation => "free text",
            Role::SymbolArt => "text inside symbol art",
            Role::Frame => "page frame or title block",
            Role::Other => "other text",
        }
    }
}

/// One text box with its provenance.
#[derive(Clone, Debug)]
pub struct Boxed {
    pub role: Role,
    pub text: String,
    pub owner: Option<AnyId>,
    pub label: Label,
    pub hidden_by_floor: bool,
}

/// A line the drawing owns, with its provenance.
#[derive(Clone, Debug)]
pub struct Stroke {
    pub a: Vec2,
    pub b: Vec2,
    pub owner: Option<AnyId>,
    pub kind: LineKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineKind {
    Wire,
    Symbol,
    Frame,
    Other,
}

impl LineKind {
    pub fn name(self) -> &'static str {
        match self {
            LineKind::Wire => "wire",
            LineKind::Symbol => "symbol line",
            LineKind::Frame => "frame line",
            LineKind::Other => "other line",
        }
    }
}

/// What the text runs into.
#[derive(Clone, Debug)]
pub enum Hit {
    Text(Boxed),
    Line(Stroke),
    /// The text box leaves the sheet.
    OutsideSheet,
}

#[derive(Clone, Debug)]
pub struct Conflict {
    pub page: PageId,
    pub text: Boxed,
    pub hit: Hit,
    /// How far they overlap (mm); for a line, how deep the line cuts into the box.
    pub depth: f32,
    /// Text against text with air under 0.5 mm: not on top of each other, but they read as one.
    pub tight: bool,
}

impl Conflict {
    /// The grouping key: `role vs role` or `role vs line kind`.
    pub fn cause(&self) -> String {
        match &self.hit {
            Hit::Text(o) => {
                let (a, b) = (self.text.role.name(), o.role.name());
                let (a, b) = if a <= b { (a, b) } else { (b, a) };
                let own = if self.text.owner.is_some() && self.text.owner == o.owner {
                    "own symbol"
                } else {
                    "neighbour"
                };
                format!(
                    "{a} / {b} ({own}){}",
                    if self.tight { " [tight]" } else { "" }
                )
            }
            Hit::Line(l) => {
                let own = if self.text.owner.is_some() && self.text.owner == l.owner {
                    "own symbol"
                } else {
                    "neighbour"
                };
                format!("{} / {} ({own})", self.text.role.name(), l.kind.name())
            }
            Hit::OutsideSheet => format!("{} / outside the sheet", self.text.role.name()),
        }
    }
}

fn tight_near(a: &novo_canvas::Aabb, b: &novo_canvas::Aabb) -> bool {
    a.min.x < b.max.x + 1.0
        && b.min.x < a.max.x + 1.0
        && a.min.y < b.max.y + 1.0
        && b.min.y < a.max.y + 1.0
}

/// A rotated rectangle: centre, half sizes, unit axes.
struct Obb {
    c: Vec2,
    h: Vec2,
    u: Vec2,
    v: Vec2,
}

fn obb(l: &Label, shrink: f32) -> Obb {
    let k = l.corners();
    let u = (k[1] - k[0]).normalize_or_zero();
    let v = (k[3] - k[0]).normalize_or_zero();
    let w = (k[1] - k[0]).length();
    let h = (k[3] - k[0]).length();
    Obb {
        c: (k[0] + k[2]) * 0.5,
        h: Vec2::new((w * 0.5 - shrink).max(0.01), (h * 0.5 - shrink).max(0.01)),
        u,
        v,
    }
}

/// Penetration depth of two boxes (0 when apart), by the separating axis test.
fn penetration(a: &Obb, b: &Obb) -> f32 {
    let mut least = f32::INFINITY;
    for axis in [a.u, a.v, b.u, b.v] {
        let ra = a.h.x * a.u.dot(axis).abs() + a.h.y * a.v.dot(axis).abs();
        let rb = b.h.x * b.u.dot(axis).abs() + b.h.y * b.v.dot(axis).abs();
        let d = (a.c - b.c).dot(axis).abs();
        let gap = ra + rb - d;
        if gap <= 0.0 {
            return 0.0;
        }
        least = least.min(gap);
    }
    least
}

/// How deep the segment cuts into the box (0 when it misses), measured as the
/// length of the segment inside the box's shorter extent.
fn cut_depth(b: &Obb, p: Vec2, q: Vec2) -> f32 {
    // To the box's frame, then clip the segment to the rectangle (Liang-Barsky).
    let to = |x: Vec2| Vec2::new((x - b.c).dot(b.u), (x - b.c).dot(b.v));
    let (p, q) = (to(p), to(q));
    let d = q - p;
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    for (pp, dd, lo, hi) in [(p.x, d.x, -b.h.x, b.h.x), (p.y, d.y, -b.h.y, b.h.y)] {
        if dd.abs() < 1e-6 {
            if pp < lo || pp > hi {
                return 0.0;
            }
        } else {
            let (mut a, mut c) = ((lo - pp) / dd, (hi - pp) / dd);
            if a > c {
                std::mem::swap(&mut a, &mut c);
            }
            t0 = t0.max(a);
            t1 = t1.min(c);
            if t0 >= t1 {
                return 0.0;
            }
        }
    }
    (t1 - t0) * d.length()
}

fn role_of(project: &Project, owner: Option<AnyId>, l: &Label, art_texts: &[String]) -> Role {
    let text = l.text.as_str();
    match owner {
        Some(AnyId::Wire(w)) => {
            if project.wire(w).and_then(|w| w.number.as_deref()) == Some(text) {
                Role::WireNumber
            } else {
                Role::CableLabel
            }
        }
        Some(AnyId::Cable(_)) => Role::CableLabel,
        Some(AnyId::Text(_)) => Role::Annotation,
        Some(AnyId::Symbol(s)) => {
            let Some(sym) = project.symbol(s) else {
                return Role::Other;
            };
            if symbol_text(project, sym).as_deref() == Some(text) {
                return match sym.role {
                    SymbolRole::Device { .. } => Role::DeviceTag,
                    SymbolRole::Terminal { .. } => Role::TerminalName,
                    SymbolRole::PotentialFeed { .. } => Role::FeedName,
                    SymbolRole::Interruption { .. } => Role::InterruptionName,
                    SymbolRole::Unassigned => Role::Other,
                };
            }
            if sym.pins.iter().any(|p| p.name == text) {
                return Role::PinName;
            }
            if art_texts.iter().any(|t| t == text) {
                return Role::SymbolArt;
            }
            if l.priority == novo_canvas::declutter::Priority::Secondary {
                return Role::Note;
            }
            // Strip name drawn with the first terminal of a run.
            if matches!(sym.role, SymbolRole::Terminal { .. }) && text.starts_with('-') {
                return Role::StripName;
            }
            Role::Other
        }
        None => Role::Frame,
        _ => Role::Other,
    }
}

/// Everything drawn on `page` as text boxes and lines.
pub fn collect(
    project: &Project,
    page: PageId,
    art: &dyn SymbolArt,
    art_labels: &dyn Fn(&str) -> Vec<String>,
    xref: Option<&novo_xref::XrefIndex>,
) -> (PageScene, Vec<Boxed>, Vec<Stroke>) {
    let mut ps = PageScene::new(project, page, art);
    if let Some(index) = xref {
        // Cross references are drawn page content (the app hands them to the scene).
        let notes: Vec<_> = index
            .labels_on_page(page)
            .into_iter()
            .map(note_of)
            .collect();
        ps.set_notes(project, notes, art);
    }
    let (boxes, lines) = {
        let scene: &Scene = ps.scene();
        let mut boxes = Vec::new();
        let mut lines = Vec::new();
        for (id, item) in scene.iter() {
            let owner = ps.model_id(id);
            let kind = match owner {
                Some(AnyId::Wire(_)) => LineKind::Wire,
                Some(AnyId::Symbol(_)) => LineKind::Symbol,
                None => LineKind::Frame,
                _ => LineKind::Other,
            };
            let art_texts = match owner {
                Some(AnyId::Symbol(s)) => project
                    .symbol(s)
                    .map(|s| art_labels(&s.symbol.0))
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            for l in &item.labels {
                if l.text.trim().is_empty() {
                    continue;
                }
                boxes.push(Boxed {
                    role: role_of(project, owner, l, &art_texts),
                    text: l.text.clone(),
                    owner,
                    label: l.clone(),
                    hidden_by_floor: false,
                });
            }
            for p in &item.primitives {
                if p.stroke.is_none() {
                    continue;
                }
                for run in p.shape.outline(0.2) {
                    for w in run.windows(2) {
                        lines.push(Stroke {
                            a: w[0],
                            b: w[1],
                            owner,
                            kind,
                        });
                    }
                }
            }
        }
        (boxes, lines)
    };
    (ps, boxes, lines)
}

/// Conflicts of one page. `sheet` is the drawn sheet rectangle (minimum, maximum).
pub fn conflicts(
    page: PageId,
    boxes: &[Boxed],
    lines: &[Stroke],
    sheet: Option<(Vec2, Vec2)>,
) -> Vec<Conflict> {
    let mut out = Vec::new();
    let obbs: Vec<Obb> = boxes.iter().map(|b| obb(&b.label, 0.15)).collect();
    let tight: Vec<Obb> = boxes.iter().map(|b| obb(&b.label, -0.25)).collect();
    for (i, a) in boxes.iter().enumerate() {
        let ab = a.label.bounds();
        // Frame text lives inside frame cells on purpose: only text against text there.
        for (j, b) in boxes.iter().enumerate().skip(i + 1) {
            if a.role == Role::Frame && b.role == Role::Frame {
                continue;
            }
            // Text of one symbol's own art (digits inside a body) is drawn together on purpose.
            if a.owner == b.owner
                && a.owner.is_some()
                && a.role == Role::SymbolArt
                && b.role == Role::SymbolArt
            {
                continue;
            }
            if !ab.intersects(&b.label.bounds()) && !tight_near(&ab, &b.label.bounds()) {
                continue;
            }
            let d = penetration(&obbs[i], &obbs[j]);
            if d <= TOUCH_MM && penetration(&tight[i], &tight[j]) > TOUCH_MM {
                out.push(Conflict {
                    page,
                    text: a.clone(),
                    hit: Hit::Text(b.clone()),
                    depth: 0.0,
                    tight: true,
                });
            }
            if d > TOUCH_MM {
                out.push(Conflict {
                    page,
                    text: a.clone(),
                    hit: Hit::Text(b.clone()),
                    depth: d,
                    tight: false,
                });
            }
        }
        if a.role == Role::Frame {
            continue;
        }
        for s in lines {
            // Text inside its own symbol art belongs there.
            if a.role == Role::SymbolArt && a.owner == s.owner {
                continue;
            }
            // A pin number stands against its own pin line.
            if a.role == Role::PinName && a.owner == s.owner {
                continue;
            }
            let sb = novo_canvas::Aabb::from_corners(s.a, s.b);
            if !ab.intersects(&sb) {
                continue;
            }
            let d = cut_depth(&obbs[i], s.a, s.b);
            if d > TOUCH_MM {
                out.push(Conflict {
                    page,
                    text: a.clone(),
                    hit: Hit::Line(s.clone()),
                    depth: d,
                    tight: false,
                });
            }
        }
        if let Some((lo, hi)) = sheet
            && (ab.min.x < lo.x - 0.5
                || ab.min.y < lo.y - 0.5
                || ab.max.x > hi.x + 0.5
                || ab.max.y > hi.y + 0.5)
        {
            out.push(Conflict {
                page,
                text: a.clone(),
                hit: Hit::OutsideSheet,
                depth: 0.0,
                tight: false,
            });
        }
    }
    out
}

/// Conflicts grouped by cause with a count, the worst depth and one example.
pub fn group(conflicts: &[Conflict]) -> BTreeMap<String, (usize, f32)> {
    let mut m: BTreeMap<String, (usize, f32)> = BTreeMap::new();
    for c in conflicts {
        let e = m.entry(c.cause()).or_default();
        e.0 += 1;
        e.1 = e.1.max(c.depth);
    }
    m
}

/// A cross-reference label as the note the page scene draws (same mapping as the app's).
pub fn note_of(label: novo_xref::Label) -> SymbolNote {
    use novo_canvas::model::{NoteAnchor, NoteDensity, NoteGlyph, NoteRow};
    use novo_xref::{ContactKind, LabelAnchor};
    let rows: Vec<NoteRow> = label
        .rows
        .iter()
        .map(|r| NoteRow {
            glyph: match r.kind {
                ContactKind::Power => NoteGlyph::Power,
                ContactKind::NormallyClosed => NoteGlyph::NormallyClosed,
                ContactKind::Changeover => NoteGlyph::Changeover,
                ContactKind::NormallyOpen | ContactKind::Other => NoteGlyph::NormallyOpen,
            },
            pins: r.pins.clone(),
            location: r.location.clone(),
        })
        .collect();
    let lines = if rows.is_empty() {
        label.lines
    } else {
        label.extra_lines().to_vec()
    };
    SymbolNote {
        symbol: label.symbol,
        lines,
        warning: label.warning,
        anchor: match label.anchor {
            LabelAnchor::Below => NoteAnchor::Below,
            LabelAnchor::Right => NoteAnchor::Right,
        },
        rows,
        density: NoteDensity::default(),
    }
}
