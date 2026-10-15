//! Adapter from the `novo-core` data model to a canvas [`Scene`] (feature
//! `model`). This is the only part of the crate that knows the model; the
//! rest of the canvas stays model-agnostic.
//!
//! A [`PageScene`] mirrors one page: every symbol, wire, junction and text
//! record becomes an item whose [`ItemId`] is the model's raw id (ids are
//! unique across the project), so canvas picks and selections map straight
//! back to model ids.
//! After each edit, pass the edit's [`ChangeSet`] to [`PageScene::apply`] and
//! only the affected items are rebuilt.
//!
//! Symbol artwork comes from a [`SymbolArt`] provider. WS-05 owns the real
//! IEC 60617 artwork; until then [`PlaceholderArt`] draws a box with pin
//! stubs so every symbol is visible and connectable.

use crate::scene::{Handle, Hit, Item, ItemId, Layer, LayerId, Scene, Sheet, SnapPoint};
use crate::shape::{Placement, Primitive, Shape};
use crate::style::{Color, Stroke};
use crate::text::{Anchor, Label};
use glam::Vec2;
use novo_core::{
    AnyId, ChangeSet, Endpoint, JunctionId, PageId, PageKind, Point, Project, Rotation, SymbolId,
    SymbolInstance, SymbolKey, SymbolRole, TextAnchor, TextContent, TextId, WireId,
};
use std::collections::{BTreeSet, HashMap};
use std::f32::consts::FRAC_PI_2;

/// Model point to canvas vector.
pub fn vec(p: Point) -> Vec2 {
    Vec2::new(p.x as f32, p.y as f32)
}

/// Model placement to canvas placement. Both mirror about the local y axis
/// first and then rotate; the model's rotations are counter-clockwise on the
/// page, canvas angles are clockwise on screen.
pub fn placement(p: &novo_core::Placement) -> Placement {
    let rotation = angle(p.rotation);
    Placement {
        offset: vec(p.origin),
        rotation,
        mirror: p.mirrored,
        scale: 1.0,
    }
}

/// A model rotation (counter-clockwise on the page) as a canvas angle
/// (clockwise on screen).
pub fn angle(r: Rotation) -> f32 {
    match r {
        Rotation::R0 => 0.0,
        Rotation::R90 => -FRAC_PI_2,
        Rotation::R180 => -2.0 * FRAC_PI_2,
        Rotation::R270 => -3.0 * FRAC_PI_2,
    }
}

/// A model text anchor as a canvas anchor.
pub fn anchor(a: TextAnchor) -> Anchor {
    match a {
        TextAnchor::TopLeft => Anchor::TopLeft,
        TextAnchor::TopCenter => Anchor::TopCenter,
        TextAnchor::TopRight => Anchor::TopRight,
        TextAnchor::MiddleLeft => Anchor::CenterLeft,
        TextAnchor::MiddleCenter => Anchor::Center,
        TextAnchor::MiddleRight => Anchor::CenterRight,
        TextAnchor::BottomLeft => Anchor::BottomLeft,
        TextAnchor::BottomCenter => Anchor::BottomCenter,
        TextAnchor::BottomRight => Anchor::BottomRight,
    }
}

/// A canvas anchor as a model text anchor (the inverse of [`anchor`]).
pub fn text_anchor(a: Anchor) -> TextAnchor {
    match a {
        Anchor::TopLeft => TextAnchor::TopLeft,
        Anchor::TopCenter => TextAnchor::TopCenter,
        Anchor::TopRight => TextAnchor::TopRight,
        Anchor::CenterLeft => TextAnchor::MiddleLeft,
        Anchor::Center => TextAnchor::MiddleCenter,
        Anchor::CenterRight => TextAnchor::MiddleRight,
        Anchor::BottomLeft => TextAnchor::BottomLeft,
        Anchor::BottomCenter => TextAnchor::BottomCenter,
        Anchor::BottomRight => TextAnchor::BottomRight,
    }
}

/// Artwork for a symbol in symbol coordinates (mm, y down, unplaced).
pub trait SymbolArt {
    /// Primitives for the symbol, or `None` to fall back to the placeholder.
    fn artwork(&self, symbol: &SymbolInstance) -> Option<Vec<Primitive>>;

    /// What the artwork depends on. Instances with the same key share one
    /// drawing, so the same clamp type always looks exactly the same and is
    /// built once per page. The default is the symbol key plus the pin
    /// offsets; return `None` for art that depends on more (properties),
    /// which is then built per instance.
    fn cache_key(&self, symbol: &SymbolInstance) -> Option<ArtKey> {
        Some(ArtKey::of(symbol))
    }

    /// Fixed text of the artwork (the M in a motor circle, 3~), in symbol
    /// coordinates. The canvas applies the placement and keeps it readable
    /// (mirroring never mirrors glyphs), and draws it in the symbol colour.
    fn labels(&self, _symbol: &SymbolInstance) -> Vec<Label> {
        Vec::new()
    }

    /// Where the device tag goes, in symbol coordinates, and which point of
    /// the text box sits there. `None` puts it right of the pins.
    fn tag_anchor(&self, _symbol: &SymbolInstance) -> Option<(Vec2, Anchor)> {
        None
    }
}

/// A cached symbol drawing in symbol coordinates.
#[derive(Clone)]
struct LocalArt {
    primitives: std::sync::Arc<[Primitive]>,
    labels: std::sync::Arc<[Label]>,
}

/// Identity of a symbol drawing: its symbol key and pin offsets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArtKey {
    pub symbol: SymbolKey,
    pins: Vec<(u64, u64)>,
}

impl ArtKey {
    pub fn of(symbol: &SymbolInstance) -> Self {
        Self {
            symbol: symbol.symbol.clone(),
            pins: symbol
                .pins
                .iter()
                .map(|p| (p.offset.x.to_bits(), p.offset.y.to_bits()))
                .collect(),
        }
    }
}

/// A box around the pins with a stub to each pin.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlaceholderArt;

impl PlaceholderArt {
    pub fn draw(symbol: &SymbolInstance, stroke: Stroke) -> Vec<Primitive> {
        let pins: Vec<Vec2> = symbol.pins.iter().map(|p| vec(p.offset)).collect();
        // A body centred on the symbol origin. Each pin sticks out 3 mm along
        // the axis it mainly lies on, and the body covers it on the other.
        let mut half = Vec2::splat(2.5);
        for p in &pins {
            let d = p.abs();
            if d.y >= d.x {
                half.x = half.x.max(d.x + 3.0);
                half.y = half.y.max(d.y - 3.0);
            } else {
                half.x = half.x.max(d.x - 3.0);
                half.y = half.y.max(d.y + 3.0);
            }
        }
        let c = Vec2::ZERO;
        let body = crate::Aabb::from_center(c, half);
        let mut out = vec![Primitive::stroked(Shape::rect(body.min, body.max), stroke)];
        for p in pins {
            if body.contains_point(p) {
                continue;
            }
            // Stub from the pin to the nearest point on the body outline.
            let q = p.clamp(body.min, body.max);
            out.push(Primitive::stroked(Shape::line(p, q), stroke));
        }
        out
    }
}

impl SymbolArt for PlaceholderArt {
    fn artwork(&self, _: &SymbolInstance) -> Option<Vec<Primitive>> {
        None
    }
}

/// Look of model objects on the canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelStyle {
    pub symbol: Stroke,
    pub wire: Stroke,
    pub junction_radius: f32,
    /// Accent (copper) colour of junction dots: true junctions only, where
    /// three or more conductors meet.
    pub joint_color: Color,
    /// Colour of the small dot where a wire lands on a pin (the wire
    /// colour, not the junction accent).
    pub pin_joint_color: Color,
    /// Radius of that dot, in mm (world size for now, not zoom-stable).
    pub pin_joint_radius: f32,
    /// Open ring on a wire ended unconnected ([`wire_end_glyph`]).
    pub wire_end_color: Color,
    /// Flag pole and pennant beside that ring.
    pub unconnected_color: Color,
    /// Whether a click on a wire's line picks the whole wire. Turn it off
    /// once the editor picks wires by corner and segment
    /// ([`PageScene::wire_part_at`]); a box still selects whole wires.
    pub wires_pick_whole: bool,
    /// Half-size (screen points, the same at every zoom) of the square shown
    /// on every wire corner; `0.0` draws none (print).
    pub corner_half: f32,
    /// Colour of a wire's corners.
    pub corner_color: Color,
    /// Colour of the wire part under the pointer or selected, and of the
    /// corners that belong to it.
    pub focus_color: Color,
    /// Radius of the ring, in mm; the flag scales with it (world size for
    /// now, not zoom-stable).
    pub wire_end_radius: f32,
    pub tag_height: f32,
    pub tag_color: Color,
    pub wire_number_height: f32,
    /// Fill of the sheet inside its outline (`None` leaves the canvas
    /// background showing, as on paper).
    pub sheet_fill: Option<Color>,
    /// Sheet outline, drawing frame and title text.
    pub frame_color: Color,
    /// Cross-reference text next to symbols ([`SymbolNote`]).
    pub note_color: Color,
    /// A [`SymbolNote`] flagged as a warning.
    pub note_warning_color: Color,
}

impl Default for ModelStyle {
    fn default() -> Self {
        Self {
            symbol: Stroke::world(0.35, Color::rgb(20, 24, 32)),
            wire: Stroke::world(0.35, Color::rgb(30, 70, 200)),
            junction_radius: 0.7,
            joint_color: Color::rgb(176, 104, 56),
            pin_joint_color: Color::rgb(30, 70, 200),
            pin_joint_radius: 0.6,
            wire_end_color: Color::rgb(176, 104, 56),
            unconnected_color: Color::rgb(190, 120, 20),
            wires_pick_whole: true,
            corner_half: 2.5,
            corner_color: Color::rgb(30, 70, 200),
            focus_color: Color::rgb(255, 150, 30),
            wire_end_radius: 0.9,
            tag_height: 2.5,
            tag_color: Color::rgb(20, 24, 32),
            wire_number_height: 2.0,
            sheet_fill: None,
            frame_color: Color::rgb(90, 90, 90),
            note_color: Color::rgb(60, 90, 140),
            note_warning_color: Color::rgb(190, 120, 20),
        }
    }
}

/// How a junction is drawn. Wiring is pin to pin, so a junction's shape is
/// never placed by hand: [`JunctionGlyph::from_directions`] derives it from
/// the directions its wires leave in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JunctionGlyph {
    /// A connection dot.
    Dot,
    /// A T: the dot plus stubs on the three connected sides. `stem` is the
    /// quarter turn (0 = down, 1 = left, 2 = up, 3 = right, clockwise on
    /// screen) of the branch leaving the through line.
    Tee { stem: u8 },
    /// A corner joining two sides; `quarter_turns` rotates the default
    /// (right and down) clockwise.
    Corner { quarter_turns: u8 },
    /// Nothing drawn: fewer than three conductors join here (a straight
    /// run, a loose end), so it is no connection point on paper.
    Plain,
}

impl JunctionGlyph {
    /// The glyph for wires leaving a point in `dirs`, one entry per wire
    /// (any length; they are snapped to the nearest of the four axes).
    /// Wires are counted, not sides: three or more wires are an electrical
    /// node and always get a dot (a T when they use exactly three sides).
    /// Fewer get no dot (docs/DESIGN.md §2): two wires at a right angle
    /// make a corner, anything else (a straight run, a loose end) is
    /// [`Plain`](Self::Plain).
    pub fn from_directions(dirs: impl IntoIterator<Item = Vec2>) -> Self {
        let mut sides = [false; 4];
        let mut wires = 0usize;
        for d in dirs {
            wires += 1;
            if d.length_squared() > 1e-12 {
                sides[quarter_of(d) as usize] = true;
            }
        }
        let used = sides.iter().filter(|s| **s).count();
        match (wires, used) {
            (3.., 3) => {
                let missing = sides.iter().position(|s| !s).unwrap_or(0) as u8;
                JunctionGlyph::Tee {
                    stem: (missing + 2) % 4,
                }
            }
            (2, 2) => (0..4u8)
                .find(|&q| sides[q as usize] && sides[((q + 3) % 4) as usize])
                .map_or(JunctionGlyph::Plain, |q| JunctionGlyph::Corner {
                    quarter_turns: q,
                }),
            (3.., _) => JunctionGlyph::Dot,
            _ => JunctionGlyph::Plain,
        }
    }
}

/// The quarter turn (see [`quarter_dir`]) nearest to a direction.
fn quarter_of(d: Vec2) -> u8 {
    if d.x.abs() >= d.y.abs() {
        if d.x > 0.0 { 3 } else { 1 }
    } else if d.y > 0.0 {
        0
    } else {
        2
    }
}

/// Width of the page frame margin, in mm.
pub const FRAME_MARGIN: f32 = 10.0;
const TITLE_BLOCK_WIDTH: f32 = 140.0;
const TITLE_ROW: f32 = 6.0;
const TITLE_ROWS: usize = 6;

/// Length of the orientation stubs on oriented junctions, in mm.
pub const JUNCTION_STUB: f32 = 1.5;

/// Unit direction of a quarter turn, clockwise on screen from "down"
/// (y grows downwards).
fn quarter_dir(q: u8) -> Vec2 {
    match q % 4 {
        0 => Vec2::new(0.0, 1.0),
        1 => Vec2::new(-1.0, 0.0),
        2 => Vec2::new(0.0, -1.0),
        _ => Vec2::new(1.0, 0.0),
    }
}

/// The primitives of a junction glyph at `pos`.
pub fn junction_glyph(glyph: JunctionGlyph, pos: Vec2, style: &ModelStyle) -> Vec<Primitive> {
    let dot = Primitive::filled(Shape::circle(pos, style.junction_radius), style.joint_color);
    let stubs = |dirs: &[Vec2]| {
        dirs.iter()
            .map(|d| Primitive::stroked(Shape::line(pos, pos + *d * JUNCTION_STUB), style.wire))
            .collect::<Vec<_>>()
    };
    match glyph {
        JunctionGlyph::Dot => vec![dot],
        JunctionGlyph::Plain => Vec::new(),
        JunctionGlyph::Tee { stem } => {
            let stem = stem % 4;
            // The stem and the through line across it.
            let mut out = stubs(&[
                quarter_dir(stem),
                quarter_dir(stem + 1),
                quarter_dir(stem + 3),
            ]);
            out.push(dot);
            out
        }
        JunctionGlyph::Corner { quarter_turns } => {
            let quarter_turns = quarter_turns % 4;
            stubs(&[quarter_dir(quarter_turns + 3), quarter_dir(quarter_turns)])
        }
    }
}

/// The mark on a wire ended unconnected (a double click in open space): an
/// open ring on the end and a flag, so a loose end is never mistaken for a
/// connection. `away` points from the wire out past its end; the flag leans
/// to the side that keeps it clear of the wire.
pub fn wire_end_glyph(end: Vec2, away: Vec2, style: &ModelStyle) -> Vec<Primitive> {
    let r = style.wire_end_radius;
    let out = if away.length_squared() > 0.0 {
        away.normalize()
    } else {
        Vec2::X
    };
    let side = Vec2::new(-out.y, out.x);
    let foot = end + out * r;
    let top = foot + side * (r * 4.0);
    let pennant = [
        top,
        top + out * (r * 2.8) + side * (r * 0.9),
        top + side * (r * 1.8),
    ];
    let flag = Stroke::world(0.25, style.unconnected_color);
    vec![
        Primitive::stroked(
            Shape::circle(end, r),
            Stroke::world(0.3, style.wire_end_color),
        ),
        Primitive::stroked(Shape::line(foot, top), flag),
        Primitive::filled(
            Shape::polyline(pennant.to_vec(), true),
            style.unconnected_color,
        ),
    ]
}

/// Layers a page scene uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelLayers {
    pub frame: LayerId,
    pub wires: LayerId,
    pub symbols: LayerId,
    /// Text records (free texts and labels), above the symbols.
    pub texts: LayerId,
}

/// Text drawn next to a symbol that the model does not hold itself, such
/// as a cross reference (`/2.3` under a contact, a coil's contact image).
/// It is part of the printed drawing, so it lives in the scene: the canvas
/// and the PDF show the same text in the same place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolNote {
    pub symbol: SymbolId,
    /// One line per row.
    pub lines: Vec<String>,
    /// Draw in [`ModelStyle::note_warning_color`].
    pub warning: bool,
    /// Where the note sits relative to its symbol.
    pub anchor: NoteAnchor,
}

/// One printed sheet of a generated page (a report): text boxes and rules
/// laid out by its generator (novo-reports' `layout_sheet`), in page
/// millimetres with the origin at the sheet's top left. The canvas stays
/// independent of the report crate; callers convert.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SheetContent {
    /// Printed where the page name goes: `20`, or `20b` for the second
    /// of several sheets.
    pub name: String,
    /// "n of m" for the title block when the caller counts sheets (a
    /// generated page can print as several); `None` counts pages.
    pub number: Option<(usize, usize)>,
    pub texts: Vec<SheetText>,
    pub lines: Vec<SheetLine>,
}

/// A line of text in a box; text wider than the box is cut to fit.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetText {
    /// Top-left corner, mm.
    pub min: Vec2,
    pub size: Vec2,
    pub text: String,
    /// Character height, mm.
    pub height: f32,
    /// Where in the box: `CenterLeft`, `Center` or `CenterRight` (the text
    /// is always centred vertically).
    pub align: Anchor,
    /// A header cell, drawn bold.
    pub bold: bool,
}

/// A rule of a sheet's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetLine {
    pub from: Vec2,
    pub to: Vec2,
    /// The outline and header rule; thin between rows.
    pub heavy: bool,
}

/// `text` cut with an ellipsis so that it fits `width` mm at `height`,
/// by the same width estimate as [`Label::size`].
fn fit_text(text: &str, width: f32, height: f32, bold: bool) -> String {
    let advance = crate::text::advance(bold) * height;
    if advance <= 0.0 {
        return text.to_owned();
    }
    let max = (width / advance).floor().max(0.0) as usize;
    if text.chars().count() <= max {
        return text.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// Where a [`SymbolNote`] sits relative to its symbol's ink.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NoteAnchor {
    /// Above the top right corner.
    #[default]
    TopRight,
    /// Under the symbol, left-aligned with it: a coil's contact image
    /// (IEC 61082).
    Below,
    /// Right of the symbol on its centre line, under the tag when it has
    /// one: a contact's or interruption point's single reference.
    Right,
}

/// The canvas scene of one model page, kept in sync with the project.
pub struct PageScene {
    page: PageId,
    scene: Scene,
    layers: ModelLayers,
    ids: HashMap<ItemId, AnyId>,
    art_cache: HashMap<ArtKey, LocalArt>,
    /// Junction ends of each drawn wire, to redraw them when it goes.
    wire_junctions: HashMap<WireId, [Option<JunctionId>; 2]>,
    /// What each drawn text is attached to, to redraw it when the text goes
    /// (a device tag text hides the symbol's own tag).
    text_hosts: HashMap<TextId, AnyId>,
    /// Notes by symbol, kept across rebuilds; see [`Self::set_notes`].
    notes: HashMap<SymbolId, SymbolNote>,
    /// What a generated page shows; see [`Self::set_sheet`].
    sheet: Option<SheetContent>,
    /// The first terminal symbol of each run of one strip on this page,
    /// with the strip's tag it shows (see [`strip_heads`]).
    strip_heads: HashMap<SymbolId, String>,
    /// The wire part under the pointer or selected, drawn emphasised.
    focus: Option<WirePart>,
    pub style: ModelStyle,
}

/// What a double click acts on ([`PageScene::double_click_target`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoubleClickTarget {
    /// Text to edit: the object that owns it (a symbol, wire or text record).
    Text(AnyId),
    Component(SymbolId),
    Wire(WireId),
    Empty,
}

/// Which kind of text a label is, for rename and edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelKind {
    /// The tag of a device's symbol.
    DeviceTag,
    /// The name of a terminal on a strip (`1`, `PE`).
    TerminalName,
    /// The name of an interruption point.
    InterruptionName,
    WireNumber,
    /// The name of a potential, on its feed or its wire.
    Potential,
    Literal,
    Property,
    /// Text the canvas adds itself (a symbol's art text, the strip name, a
    /// cross-reference note): shown, not edited by double click.
    Fixed,
}

/// The kind of the name a symbol of this role shows.
fn role_label_kind(role: &SymbolRole) -> LabelKind {
    match role {
        SymbolRole::Terminal { .. } => LabelKind::TerminalName,
        SymbolRole::Interruption { .. } => LabelKind::InterruptionName,
        SymbolRole::PotentialFeed { .. } => LabelKind::Potential,
        SymbolRole::Device { .. } | SymbolRole::Unassigned => LabelKind::DeviceTag,
    }
}

/// The label under a pick ([`PageScene::label_hit`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LabelHit {
    /// The object the text belongs to: the host of a text record, the symbol
    /// of its tag, the wire of its number.
    pub owner: AnyId,
    /// The text record, when the text is one.
    pub text: Option<TextId>,
    pub kind: LabelKind,
}

/// A part of a wire that can be picked on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WirePart {
    pub wire: WireId,
    pub kind: WirePartKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WirePartKind {
    /// A bend point: index into the wire's `route`.
    Corner(usize),
    /// A straight run between two points: 0 is the one leaving the first
    /// end; the last is the one reaching the second end. Its corners are
    /// the route points at `i - 1` and `i` (an end is not a corner).
    Segment(usize),
}

/// Item id of a model object.
pub fn item_id(id: impl Into<AnyId>) -> ItemId {
    ItemId(id.into().raw())
}

impl PageScene {
    /// Builds the scene of `page` from scratch.
    pub fn new(project: &Project, page: PageId, art: &dyn SymbolArt) -> Self {
        let mut scene = Scene::new();
        let layers = ModelLayers {
            frame: scene.add_layer(Layer::new("Frame", -10)),
            wires: scene.add_layer(Layer::new("Wires", 0)),
            symbols: scene.add_layer(Layer::new("Symbols", 10)),
            texts: scene.add_layer(Layer::new("Texts", 20)),
        };
        let mut s = Self {
            page,
            scene,
            layers,
            ids: HashMap::new(),
            art_cache: HashMap::new(),
            wire_junctions: HashMap::new(),
            text_hosts: HashMap::new(),
            notes: HashMap::new(),
            sheet: None,
            strip_heads: HashMap::new(),
            focus: None,
            style: ModelStyle::default(),
        };
        s.rebuild(project, art);
        s
    }

    /// Changes the colours and sizes and redraws every item.
    pub fn set_style(&mut self, style: ModelStyle, project: &Project, art: &dyn SymbolArt) {
        self.style = style;
        self.rebuild(project, art);
    }

    pub fn page(&self) -> PageId {
        self.page
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Mutable access for layer visibility and locks.
    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn layers(&self) -> ModelLayers {
        self.layers
    }

    /// The model object behind a canvas item.
    pub fn model_id(&self, item: ItemId) -> Option<AnyId> {
        self.ids.get(&item).copied()
    }

    /// The wire part at `p` within `tolerance` (mm): a corner when one is
    /// near, else the nearest straight run. Wires are never picked whole by
    /// a click; a double click selects the whole wire.
    pub fn wire_part_at(&self, project: &Project, p: Vec2, tolerance: f32) -> Option<WirePart> {
        let mut best: Option<(f32, WirePart)> = None;
        let mut offer = |d: f32, part: WirePart, pref: f32| {
            let score = d - pref;
            if d <= tolerance && best.is_none_or(|(b, _)| score < b) {
                best = Some((score, part));
            }
        };
        for wire in project.page_contents(self.page).wires {
            let Some(pts) = self.wire_points(project, wire) else {
                continue;
            };
            // A corner within reach beats a run through the same spot.
            for (i, c) in pts[1..pts.len().saturating_sub(1)].iter().enumerate() {
                let part = WirePart {
                    wire,
                    kind: WirePartKind::Corner(i),
                };
                offer(c.distance(p), part, tolerance);
            }
            for (k, w) in pts.windows(2).enumerate() {
                let part = WirePart {
                    wire,
                    kind: WirePartKind::Segment(k),
                };
                offer(crate::geom::dist_point_segment(p, w[0], w[1]), part, 0.0);
            }
        }
        best.map(|(_, part)| part)
    }

    /// Emphasises one wire part (or none): the pointer's, or the selected
    /// one. Only the wires involved are redrawn.
    pub fn set_wire_focus(&mut self, project: &Project, focus: Option<WirePart>) {
        if self.focus == focus {
            return;
        }
        let old = std::mem::replace(&mut self.focus, focus);
        for wire in [old, focus].into_iter().flatten().map(|p| p.wire) {
            self.sync_wire(project, wire);
        }
    }

    /// What a double click at a pick should act on: the text when text was
    /// hit (a tag, a wire number or a free text), else the component or
    /// wire, else nothing. Wires are told from components so a double click
    /// on a wire can end it there while one on text edits it.
    pub fn double_click_target(&self, hit: Option<&Hit>) -> DoubleClickTarget {
        let Some(hit) = hit else {
            return DoubleClickTarget::Empty;
        };
        let Some(id) = self.model_id(hit.id) else {
            return DoubleClickTarget::Empty;
        };
        match id {
            AnyId::Text(_) => DoubleClickTarget::Text(id),
            _ if hit.label.is_some() => DoubleClickTarget::Text(id),
            AnyId::Symbol(s) => DoubleClickTarget::Component(s),
            AnyId::Wire(w) => DoubleClickTarget::Wire(w),
            _ => DoubleClickTarget::Empty,
        }
    }

    /// The label a pick hit, with what it belongs to and what kind of text
    /// it is, so a double click can rename a tag, a wire number or a free
    /// text. `None` when the pick did not hit text.
    pub fn label_hit(&self, project: &Project, hit: &Hit) -> Option<LabelHit> {
        let index = hit.label?;
        let id = self.model_id(hit.id)?;
        match id {
            AnyId::Text(t) => {
                let text = project.text(t)?;
                let kind = match text.content {
                    TextContent::DeviceTag => text
                        .attached_to
                        .and_then(|h| match h {
                            AnyId::Symbol(s) => project.symbol(s),
                            _ => None,
                        })
                        .map_or(LabelKind::DeviceTag, |s| role_label_kind(&s.role)),
                    TextContent::WireNumber => LabelKind::WireNumber,
                    TextContent::Potential => LabelKind::Potential,
                    TextContent::Literal { .. } => LabelKind::Literal,
                    TextContent::Property { .. } => LabelKind::Property,
                };
                Some(LabelHit {
                    owner: text.attached_to.unwrap_or(id),
                    text: Some(t),
                    kind,
                })
            }
            // A symbol's own tag is its first label when no text record
            // draws it; everything after is the canvas's own text.
            AnyId::Symbol(s) => {
                let sym = project.symbol(s)?;
                let has_record = project.texts_of(s).into_iter().any(|t| {
                    project
                        .text(t)
                        .is_some_and(|t| t.content == TextContent::DeviceTag)
                });
                let tag = index == 0 && !has_record && symbol_text(project, sym).is_some();
                Some(LabelHit {
                    owner: id,
                    text: None,
                    kind: if tag {
                        role_label_kind(&sym.role)
                    } else {
                        LabelKind::Fixed
                    },
                })
            }
            AnyId::Wire(_) => Some(LabelHit {
                owner: id,
                text: None,
                kind: LabelKind::WireNumber,
            }),
            _ => None,
        }
    }

    /// Replaces the notes drawn next to symbols (cross references). Only
    /// symbols whose note changed are redrawn, so calling this every frame
    /// with the same notes costs a comparison. Returns whether anything
    /// changed.
    pub fn set_notes(
        &mut self,
        project: &Project,
        notes: impl IntoIterator<Item = SymbolNote>,
        art: &dyn SymbolArt,
    ) -> bool {
        let mut fresh: HashMap<SymbolId, SymbolNote> = HashMap::new();
        for note in notes {
            if !note.lines.iter().all(|l| l.trim().is_empty()) {
                fresh.insert(note.symbol, note);
            }
        }
        let mut touched: Vec<SymbolId> = self
            .notes
            .iter()
            .filter(|(id, n)| fresh.get(id) != Some(n))
            .map(|(id, _)| *id)
            .collect();
        touched.extend(
            fresh
                .iter()
                .filter(|(id, n)| self.notes.get(id) != Some(n))
                .map(|(id, _)| *id),
        );
        touched.sort();
        touched.dedup();
        self.notes = fresh;
        for &id in &touched {
            self.sync_symbol(project, id, art);
        }
        !touched.is_empty()
    }

    /// The notes currently drawn, by symbol.
    pub fn notes(&self) -> impl Iterator<Item = &SymbolNote> {
        self.notes.values()
    }

    /// Shows `sheet` inside the frame: the content of a generated page
    /// (a report) and its sheet name and number in the title block. `None`
    /// goes back to the page alone. Kept across rebuilds. Returns whether
    /// anything changed.
    pub fn set_sheet(&mut self, project: &Project, sheet: Option<SheetContent>) -> bool {
        if sheet == self.sheet {
            return false;
        }
        self.sheet = sheet;
        self.frame(project);
        true
    }

    /// The sheet shown, if any.
    pub fn sheet(&self) -> Option<&SheetContent> {
        self.sheet.as_ref()
    }

    /// Rebuilds every item, for example after loading a file or changing
    /// the symbol library (this also drops the cached symbol drawings).
    pub fn rebuild(&mut self, project: &Project, art: &dyn SymbolArt) {
        self.scene.clear();
        self.ids.clear();
        self.art_cache.clear();
        self.wire_junctions.clear();
        self.text_hosts.clear();
        self.frame(project);
        self.strip_heads = strip_heads(project, self.page);
        let contents = project.page_contents(self.page);
        for id in contents.symbols {
            self.sync_symbol(project, id, art);
        }
        self.sync_bars(project);
        for id in contents.junctions {
            self.sync_junction(project, id);
        }
        for id in contents.wires {
            self.sync_wire(project, id);
        }
        for id in contents.texts {
            self.sync_text(project, id);
        }
    }

    /// Updates the scene after an edit. Returns whether anything changed.
    pub fn apply(&mut self, project: &Project, changes: &ChangeSet, art: &dyn SymbolArt) -> bool {
        if project.page(self.page).is_none() {
            let changed = !self.scene.is_empty();
            self.scene.clear();
            self.ids.clear();
            self.wire_junctions.clear();
            self.text_hosts.clear();
            return changed;
        }
        let mut dirty: BTreeSet<AnyId> = BTreeSet::new();
        let mut frame_dirty = changes.info;
        for &id in changes
            .added
            .iter()
            .chain(&changes.modified)
            .chain(&changes.removed)
        {
            dirty.insert(id);
            match id {
                // Wires follow the symbols and junctions they end on.
                AnyId::Symbol(s) => {
                    let pins = project.symbol(s).map_or(0, |s| s.pins.len());
                    for pin in 0..pins as u16 {
                        dirty.extend(project.wires_at(Endpoint::pin(s, pin)).map(AnyId::Wire));
                    }
                }
                AnyId::Junction(j) => {
                    dirty.extend(project.wires_at(Endpoint::junction(j)).map(AnyId::Wire));
                }
                // Tags are drawn on the device's symbols, and a strip's tag
                // is part of its terminals' designations.
                AnyId::Device(d) => {
                    dirty.extend(
                        project
                            .symbols_of_device(d)
                            .into_iter()
                            .map(|s| AnyId::Symbol(s.id)),
                    );
                    for t in project.terminals_of_strip(d) {
                        dirty.extend(
                            project
                                .symbols_of_terminal(t.id)
                                .into_iter()
                                .map(|s| AnyId::Symbol(s.id)),
                        );
                    }
                }
                // Feed symbols show the potential's name, and so do the
                // potential labels of the wires that define it.
                AnyId::Potential(_) => {
                    for r in project.referrers(id) {
                        match r {
                            AnyId::Symbol(_) => {
                                dirty.insert(r);
                            }
                            AnyId::Wire(w) => {
                                dirty.extend(project.texts_of(w).into_iter().map(AnyId::Text));
                            }
                            _ => {}
                        }
                    }
                }
                // A text's host draws its own tag only while no device tag
                // text is attached to it, before and after the edit.
                AnyId::Text(t) => {
                    let old = self.text_hosts.get(&t).copied();
                    let new = project.text(t).and_then(|t| t.attached_to);
                    dirty.extend(
                        old.into_iter()
                            .chain(new)
                            .filter(|h| matches!(h, AnyId::Symbol(_))),
                    );
                }
                AnyId::Terminal(t) => dirty.extend(
                    project
                        .symbols_of_terminal(t)
                        .into_iter()
                        .map(|s| AnyId::Symbol(s.id)),
                ),
                // Any page coming, going or changing can change "n of m".
                AnyId::Page(_) => frame_dirty = true,
                _ => {}
            }
        }
        // A strip's name sits at the first terminal of each run: any
        // terminal edit can move it to another symbol.
        let terminals_touched = dirty.iter().any(|id| match id {
            AnyId::Device(_) | AnyId::Terminal(_) => true,
            AnyId::Symbol(s) => {
                self.strip_heads.contains_key(s)
                    // A removed symbol may have split a run.
                    || project
                        .symbol(*s)
                        .is_none_or(|s| matches!(s.role, SymbolRole::Terminal { .. }))
            }
            _ => false,
        });
        if terminals_touched {
            self.sync_bars(project);
            let heads = strip_heads(project, self.page);
            for (s, tag) in self.strip_heads.iter().chain(&heads) {
                if self.strip_heads.get(s) != Some(tag) || heads.get(s) != Some(tag) {
                    dirty.insert(AnyId::Symbol(*s));
                }
            }
            self.strip_heads = heads;
        }
        // A junction's glyph follows the wires leaving it, before and after
        // the edit (a removed wire is gone from the project).
        let wires: Vec<WireId> = dirty
            .iter()
            .filter_map(|id| match id {
                AnyId::Wire(w) => Some(*w),
                _ => None,
            })
            .collect();
        for w in wires {
            let old = self.wire_junctions.get(&w).copied().unwrap_or_default();
            let new = project.wire(w).map(junctions_of).unwrap_or_default();
            dirty.extend(old.into_iter().chain(new).flatten().map(AnyId::Junction));
        }
        // Labels move with their host and show its values (a wire's number
        // or potential, a symbol's tag).
        let hosts: Vec<AnyId> = dirty
            .iter()
            .copied()
            .filter(|id| matches!(id, AnyId::Symbol(_) | AnyId::Junction(_) | AnyId::Wire(_)))
            .collect();
        for host in hosts {
            dirty.extend(project.texts_of(host).into_iter().map(AnyId::Text));
        }
        let rev = self.scene.revision();
        if frame_dirty {
            self.frame(project);
        }
        for id in dirty {
            match id {
                AnyId::Symbol(s) => self.sync_symbol(project, s, art),
                AnyId::Junction(j) => self.sync_junction(project, j),
                AnyId::Wire(w) => self.sync_wire(project, w),
                AnyId::Text(t) => self.sync_text(project, t),
                _ => {}
            }
        }
        self.scene.revision() != rev
    }

    fn remove(&mut self, id: AnyId) {
        let item = item_id(id);
        self.scene.remove(item);
        self.ids.remove(&item);
    }

    fn put(&mut self, id: AnyId, item: Item) {
        let key = item_id(id);
        self.ids.insert(key, id);
        self.scene.insert(key, item);
    }

    fn frame(&mut self, project: &Project) {
        let Some(page) = project.page(self.page) else {
            return;
        };
        let (w, h) = page.format.size_mm();
        let size = Vec2::new(w as f32, h as f32);
        let color = self.style.frame_color;
        let ink = Stroke::world(0.5, color);
        let thin = Stroke::world(0.25, color);
        // The sheet's fill is painted by the view under the grid dots, not
        // by this item, so the dots show inside the page.
        let sheet = Primitive::stroked(Shape::rect(Vec2::ZERO, size), thin);
        let inner_min = Vec2::splat(FRAME_MARGIN);
        let inner_max = size - Vec2::splat(FRAME_MARGIN);
        let mut item = Item::new(self.layers.frame)
            .with(sheet)
            .with(Primitive::stroked(Shape::rect(inner_min, inner_max), ink))
            .unselectable();

        // Column numbers in the top and bottom margins, on the same grid as
        // PageFormat::column_at, so a cross reference like 3.4 reads off
        // the page.
        let columns = page.format.columns.max(1);
        let col_w = size.x / f32::from(columns);
        for c in 0..columns {
            let x0 = f32::from(c) * col_w;
            let x1 = x0 + col_w;
            if c > 0 {
                for (y0, y1) in [(0.0, FRAME_MARGIN), (inner_max.y, size.y)] {
                    item.primitives.push(Primitive::stroked(
                        Shape::line(Vec2::new(x0, y0), Vec2::new(x0, y1)),
                        thin,
                    ));
                }
            }
            let cx = (x0 + x1) * 0.5;
            for cy in [FRAME_MARGIN * 0.5, size.y - FRAME_MARGIN * 0.5] {
                item.labels.push(
                    Label::new(Vec2::new(cx, cy), (c + 1).to_string(), 3.0)
                        .anchored(Anchor::Center)
                        .colored(color),
                );
            }
        }

        if let Some(sheet) = &self.sheet {
            for l in &sheet.lines {
                let stroke = if l.heavy { ink } else { thin };
                item.primitives
                    .push(Primitive::stroked(Shape::line(l.from, l.to), stroke));
            }
            for t in &sheet.texts {
                let fx = t.align.fractions().x;
                let pos = Vec2::new(t.min.x + fx * t.size.x, t.min.y + 0.5 * t.size.y);
                let anchor = match fx {
                    f if f < 0.25 => Anchor::CenterLeft,
                    f if f > 0.75 => Anchor::CenterRight,
                    _ => Anchor::Center,
                };
                let text = fit_text(&t.text, t.size.x, t.height, t.bold);
                if !text.is_empty() {
                    item.labels.push(
                        Label::new(pos, text, t.height)
                            .anchored(anchor)
                            .colored(self.style.tag_color)
                            .bolded(t.bold),
                    );
                }
            }
        }
        self.title_block(project, page, inner_max, &mut item);
        let mut lines = Vec::new();
        for p in &item.primitives {
            frame_segments(&p.shape, &mut lines);
        }
        self.scene.set_sheet(Some(Sheet {
            rect: crate::Aabb::from_corners(Vec2::ZERO, size),
            fill: self.style.sheet_fill,
            lines,
        }));
        self.scene.insert(item_id(self.page), item);
    }

    /// The title block in the bottom-right corner of the inner frame
    /// (docs/DESIGN.md §3): project, page name and title, "n of m", the
    /// page's `=function +location`, revision and drawn / checked by.
    ///
    /// Until novo-core has revisions these come from properties: the page's
    /// `structure`, and the project's `revision`, `revision_date`,
    /// `drawn_by` and `checked_by`.
    fn title_block(
        &self,
        project: &Project,
        page: &novo_core::Page,
        corner: Vec2,
        item: &mut Item,
    ) {
        let color = self.style.frame_color;
        let thin = Stroke::world(0.25, color);
        let size = Vec2::new(TITLE_BLOCK_WIDTH, TITLE_ROW * TITLE_ROWS as f32);
        let min = corner - size;
        item.primitives
            .push(Primitive::stroked(Shape::rect(min, corner), thin));
        for r in 1..TITLE_ROWS {
            let y = min.y + TITLE_ROW * r as f32;
            item.primitives.push(Primitive::stroked(
                Shape::line(Vec2::new(min.x, y), Vec2::new(corner.x, y)),
                thin,
            ));
        }
        // Schematic pages count among themselves: generated pages (reports)
        // number their own sheets, so a growing report never renumbers a
        // frozen schematic page (DESIGN.md §8).
        let generated = |p: &novo_core::Page| matches!(p.kind, PageKind::Generated { .. });
        let order: Vec<_> = project
            .pages_in_order()
            .into_iter()
            .filter(|p| generated(p) == generated(page))
            .collect();
        let (n, of) = self
            .sheet
            .as_ref()
            .and_then(|s| s.number)
            .unwrap_or_else(|| {
                let n = order
                    .iter()
                    .position(|p| p.id == page.id)
                    .map_or(0, |i| i + 1);
                (n, order.len())
            });
        let text = |props: &novo_core::Properties, key: &str| match props.get(key) {
            Some(novo_core::Value::Text(t)) => t.clone(),
            Some(novo_core::Value::Int(i)) => i.to_string(),
            _ => String::new(),
        };
        let info = &project.info().properties;
        // The page's own structure; older files kept it in a property.
        let structure = if page.structure.is_empty() {
            text(&page.properties, "structure")
        } else {
            page.structure.to_string()
        };
        let revision = [text(info, "revision"), text(info, "revision_date")]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("  ");
        let people = match (text(info, "drawn_by"), text(info, "checked_by")) {
            (d, c) if d.is_empty() && c.is_empty() => String::new(),
            (d, c) => format!("{d} / {c}"),
        };
        let name = self.sheet.as_ref().map_or(&page.name, |s| &s.name);
        let page_line = if page.title.is_empty() {
            name.clone()
        } else {
            format!("{name}  {}", page.title)
        };
        let rows = [
            ("Project", project.info().name.clone()),
            ("Page", page_line),
            ("Sheet", format!("{n} of {of}")),
            ("Structure", structure),
            ("Revision", revision),
            ("Drawn / checked", people),
        ];
        for (r, (caption, value)) in rows.into_iter().enumerate() {
            let y = min.y + TITLE_ROW * (r as f32 + 0.5);
            item.labels.push(
                Label::new(Vec2::new(min.x + 2.0, y), caption, 2.0)
                    .anchored(Anchor::CenterLeft)
                    .colored(color),
            );
            if !value.is_empty() {
                item.labels.push(
                    Label::new(Vec2::new(min.x + 24.0, y), value, 3.0)
                        .anchored(Anchor::CenterLeft)
                        .colored(color),
                );
            }
        }
    }

    /// Redraws the bridge bars of the page's jumper groups.
    fn sync_bars(&mut self, project: &Project) {
        let bars = bridge_bars(project, self.page);
        if bars.is_empty() {
            self.scene.remove(BARS_ITEM);
            return;
        }
        let stroke = Stroke::world(0.5, self.style.symbol.color);
        let mut item = Item::new(self.layers.symbols).unselectable();
        for (a, b) in bars {
            item = item.with(Primitive::stroked(Shape::line(a, b), stroke));
        }
        self.scene.insert(BARS_ITEM, item);
    }

    fn sync_symbol(&mut self, project: &Project, id: SymbolId, art: &dyn SymbolArt) {
        let Some(sym) = project.symbol(id).filter(|s| s.page == self.page) else {
            self.remove(id.into());
            return;
        };
        let place = placement(&sym.placement);
        let style = self.style.symbol;
        let draw = || LocalArt {
            primitives: art
                .artwork(sym)
                .unwrap_or_else(|| PlaceholderArt::draw(sym, style))
                .into(),
            labels: art.labels(sym).into(),
        };
        let local = match art.cache_key(sym) {
            Some(key) => self.art_cache.entry(key).or_insert_with(draw).clone(),
            None => draw(),
        };
        let mut item = Item::new(self.layers.symbols);
        item.primitives = local
            .primitives
            .iter()
            .map(|p| p.transformed(&place))
            .collect();
        item.labels = local
            .labels
            .iter()
            .map(|l| l.transformed(&place).colored(style.color))
            .collect();
        item.snap_points = (0..sym.pins.len() as u16)
            .filter_map(|i| sym.pin_position(i))
            .map(|p| SnapPoint::connection(vec(p)))
            .collect();
        // A device tag text record draws the tag itself; never draw it twice.
        let tag_text = project.texts_of(id).into_iter().any(|t| {
            project
                .text(t)
                .is_some_and(|t| t.content == TextContent::DeviceTag)
        });
        let text = symbol_text(project, sym).filter(|_| !tag_text);
        let mut tag = None;
        if let Some(text) = text {
            // To the right of the placed symbol, vertically centred on its
            // pins, whatever its rotation or mirroring.
            let origin = vec(sym.placement.origin);
            let b = crate::Aabb::from_points(
                (0..sym.pins.len() as u16)
                    .filter_map(|i| sym.pin_position(i))
                    .map(vec),
            );
            let (pos, anchor) = match art.tag_anchor(sym) {
                Some((p, a)) => (place.affine().transform_point2(p), a),
                None if b.is_empty() => (origin + Vec2::new(5.0, 0.0), Anchor::CenterLeft),
                None => (
                    Vec2::new(b.max.x.max(origin.x + 2.5) + 5.0, b.center().y),
                    Anchor::CenterLeft,
                ),
            };
            // The tag is the device's own text: always horizontal, and
            // always the item's first label.
            let label = Label::new(pos, text, self.style.tag_height)
                .anchored(anchor)
                .colored(self.style.tag_color);
            tag = Some(label.clone());
            item.labels.insert(0, label);
        }
        if let Some(strip) = self.strip_heads.get(&id) {
            // Left of the run's first terminal and of every label already
            // on its row (the terminal's own name, a pin number), so the
            // two never overlap at any zoom: label boxes are in mm.
            let b = item
                .primitives
                .iter()
                .fold(crate::Aabb::EMPTY, |b, p| b.union(&p.shape.bounds()));
            let (mut x, y) = if b.is_empty() {
                let o = vec(sym.placement.origin);
                (o.x, o.y)
            } else {
                (b.min.x, b.center().y)
            };
            let h = self.style.tag_height;
            for l in &item.labels {
                let lb = l.bounds();
                if lb.min.y < y + h * 0.5 && lb.max.y > y - h * 0.5 {
                    x = x.min(lb.min.x);
                }
            }
            item.labels.push(
                Label::new(Vec2::new(x - 2.0, y), strip.clone(), h)
                    .anchored(Anchor::CenterRight)
                    .colored(self.style.tag_color),
            );
        }
        if let Some(note) = self.notes.get(&id) {
            let label = self.note_label(&item, tag.as_ref(), note);
            item.labels.push(label);
        }
        self.put(id.into(), item);
    }

    /// A note next to the symbol's ink (see [`NoteAnchor`]), in the tag's
    /// size so both read alike. A warning keeps its colour while the
    /// symbol is selected.
    fn note_label(&self, item: &Item, tag: Option<&Label>, note: &SymbolNote) -> Label {
        let ink = item
            .primitives
            .iter()
            .fold(crate::Aabb::EMPTY, |b, p| b.union(&p.shape.bounds()));
        let ink = if ink.is_empty() {
            let at = item.snap_points.first().map_or(Vec2::ZERO, |s| s.pos);
            crate::Aabb::from_corners(at, at)
        } else {
            ink
        };
        let h = self.style.tag_height;
        let (pos, anchor) = match note.anchor {
            NoteAnchor::TopRight => (
                Vec2::new(ink.max.x + 1.0, ink.min.y - 0.5),
                Anchor::BottomLeft,
            ),
            NoteAnchor::Below => (Vec2::new(ink.min.x, ink.max.y + 1.0), Anchor::TopLeft),
            NoteAnchor::Right => match tag {
                // One line under the tag, aligned with it.
                Some(t) => {
                    let b = t.bounds();
                    (Vec2::new(b.min.x, b.max.y + 0.2 * h), Anchor::TopLeft)
                }
                None => (
                    Vec2::new(ink.max.x + 1.0, ink.center().y),
                    Anchor::CenterLeft,
                ),
            },
        };
        let label = Label::new(pos, note.lines.join("\n"), h).anchored(anchor);
        if note.warning {
            label.colored(self.style.note_warning_color).keeping_color()
        } else {
            label.colored(self.style.note_color)
        }
    }

    fn sync_junction(&mut self, project: &Project, id: JunctionId) {
        let Some(j) = project.junction(id).filter(|j| j.page == self.page) else {
            self.remove(id.into());
            return;
        };
        let p = vec(j.position);
        // Which way each wire leaves: towards its first bend or far end.
        let dirs: Vec<Vec2> = project
            .wires_at(Endpoint::junction(id))
            .filter_map(|w| self.wire_points(project, w))
            .map(|pts| {
                let next = if pts[0].distance(p) < 1e-4 {
                    pts.iter().find(|q| q.distance(p) > 1e-4)
                } else {
                    pts.iter().rev().find(|q| q.distance(p) > 1e-4)
                };
                // A wire that never leaves the point still counts.
                next.map_or(Vec2::ZERO, |q| *q - p)
            })
            .collect();
        let mut item = Item::new(self.layers.wires).with_snap(SnapPoint::connection(p));
        item.primitives = junction_glyph(JunctionGlyph::from_directions(dirs), p, &self.style);
        self.put(id.into(), item);
    }

    fn sync_text(&mut self, project: &Project, id: TextId) {
        let Some(text) = project.text(id).filter(|t| t.page == self.page) else {
            self.text_hosts.remove(&id);
            self.remove(id.into());
            return;
        };
        match text.attached_to {
            Some(host) => self.text_hosts.insert(id, host),
            None => self.text_hosts.remove(&id),
        };
        // A label whose source is missing (an unnumbered wire) shows nothing
        // and has nothing to pick.
        let shown = project.text_string(id).unwrap_or_default();
        let Some(pos) = project.text_position(id).filter(|_| !shown.is_empty()) else {
            self.remove(id.into());
            return;
        };
        let color = match text.content {
            TextContent::DeviceTag => self.style.tag_color,
            TextContent::WireNumber | TextContent::Potential => self.style.wire.color,
            TextContent::Literal { .. } | TextContent::Property { .. } => self.style.symbol.color,
        };
        let mut item = Item::new(self.layers.texts);
        item.labels.push(
            Label::new(vec(pos), shown, text.height_mm as f32)
                .anchored(anchor(text.placement.anchor))
                .rotated(angle(text.placement.rotation))
                .colored(color),
        );
        self.put(id.into(), item);
    }

    /// A wire's polyline: start, route bends, end.
    fn wire_points(&self, project: &Project, id: WireId) -> Option<Vec<Vec2>> {
        let w = project.wire(id)?;
        let a = project.endpoint_position(w.from)?;
        let b = project.endpoint_position(w.to)?;
        let mut pts = vec![vec(a)];
        pts.extend(w.route.iter().map(|p| vec(*p)));
        pts.push(vec(b));
        Some(pts)
    }

    fn sync_wire(&mut self, project: &Project, id: WireId) {
        let Some(w) = project.wire(id).filter(|w| w.page == self.page) else {
            self.wire_junctions.remove(&id);
            self.remove(id.into());
            return;
        };
        self.wire_junctions.insert(id, junctions_of(w));
        let Some(pts) = self.wire_points(project, id) else {
            self.remove(id.into());
            return;
        };
        let mut item = Item::new(self.layers.wires)
            .with(Primitive::stroked(
                Shape::polyline(pts.clone(), false),
                self.style.wire,
            ))
            .with_snap(SnapPoint::connection(pts[0]))
            .with_snap(SnapPoint::connection(pts[pts.len() - 1]));
        // A dot where the wire lands on a pin marks the connection.
        for (e, p) in w.ends().into_iter().zip([pts[0], pts[pts.len() - 1]]) {
            if matches!(e, Endpoint::Pin { .. }) {
                item = item.with(Primitive::filled(
                    Shape::circle(p, self.style.pin_joint_radius),
                    self.style.pin_joint_color,
                ));
            }
        }
        item.click_select = self.style.wires_pick_whole;
        // Every corner shows as a small square, so it can be told from the
        // straight runs and picked on its own.
        if self.style.corner_half > 0.0 {
            for c in &pts[1..pts.len() - 1] {
                item.handles.push(Handle::square(
                    *c,
                    self.style.corner_half,
                    self.style.corner_color,
                ));
            }
        }
        // The part under the pointer or selected: that run drawn heavier,
        // with its corners filled in the focus colour.
        if let Some(part) = self.focus.filter(|p| p.wire == id) {
            let focus = self.style.focus_color;
            let big = self.style.corner_half * 1.8;
            let mut corners = Vec::new();
            match part.kind {
                WirePartKind::Segment(k) if k + 1 < pts.len() => {
                    item.primitives.push(Primitive::stroked(
                        Shape::line(pts[k], pts[k + 1]),
                        Stroke::world(self.style.wire.width * 2.5, focus),
                    ));
                    corners.extend(
                        [k, k + 1]
                            .into_iter()
                            .filter(|&i| i > 0 && i + 1 < pts.len()),
                    );
                }
                WirePartKind::Corner(c) if c + 2 < pts.len() => corners.push(c + 1),
                _ => {}
            }
            for i in corners {
                item.handles.push(Handle::square(pts[i], big, focus));
            }
        }
        // An open end (a wire ended unconnected) shows a ring and a flag.
        let n = pts.len();
        for (e, end, inner) in [(w.from, pts[0], pts[1]), (w.to, pts[n - 1], pts[n - 2])] {
            if matches!(e, Endpoint::Free { .. }) {
                item.primitives
                    .extend(wire_end_glyph(end, end - inner, &self.style));
            }
        }
        if let Some(n) = w.number.as_deref().filter(|n| !n.is_empty()) {
            // Along the longest segment, just above (or left of) the wire.
            let (s0, s1) = pts
                .windows(2)
                .map(|s| (s[0], s[1]))
                .max_by(|x, y| x.0.distance(x.1).total_cmp(&y.0.distance(y.1)))
                .unwrap_or((pts[0], pts[0]));
            let mid = (s0 + s1) * 0.5;
            let vertical = (s1 - s0).y.abs() > (s1 - s0).x.abs();
            let label = if vertical {
                Label::new(mid + Vec2::new(-1.0, 0.0), n, self.style.wire_number_height)
                    .anchored(Anchor::BottomCenter)
                    .rotated(-FRAC_PI_2)
            } else {
                Label::new(mid + Vec2::new(0.0, -0.8), n, self.style.wire_number_height)
                    .anchored(Anchor::BottomCenter)
            };
            item.labels.push(label.colored(self.style.wire.color));
        }
        self.put(id.into(), item);
    }
}

fn junctions_of(w: &novo_core::Wire) -> [Option<JunctionId>; 2] {
    w.ends().map(|e| match e {
        Endpoint::Junction { junction } => Some(junction),
        Endpoint::Pin { .. } | Endpoint::Free { .. } => None,
    })
}

/// The straight segments of a frame shape, for keeping grid dots clear.
fn frame_segments(shape: &Shape, out: &mut Vec<(Vec2, Vec2)>) {
    match shape {
        Shape::Line { a, b } => out.push((*a, *b)),
        Shape::Rect { min, max } => {
            let c = [*min, Vec2::new(max.x, min.y), *max, Vec2::new(min.x, max.y)];
            out.extend((0..4).map(|i| (c[i], c[(i + 1) % 4])));
        }
        Shape::Polyline { points, closed } => {
            out.extend(points.windows(2).map(|w| (w[0], w[1])));
            if let (true, Some(f), Some(l)) = (*closed, points.first(), points.last()) {
                out.push((*l, *f));
            }
        }
        _ => {}
    }
}

/// The text shown next to a symbol: its device tag, terminal designation,
/// interruption point name or potential name.
pub fn symbol_text(project: &Project, sym: &SymbolInstance) -> Option<String> {
    match &sym.role {
        SymbolRole::Device { device, .. } => project.device(*device).map(|d| d.tag.to_string()),
        // Terminals sit a few mm apart on a strip, so they show only their
        // own name (1, 2, ...); the full designation (-X1:2) would run over
        // the neighbours. One name per level; levels usually share it.
        SymbolRole::Terminal { terminals } => {
            let mut names: Vec<String> = Vec::new();
            for t in terminals {
                if let Some(n) = project.terminal(*t).map(|t| t.name.clone())
                    && !names.contains(&n)
                {
                    names.push(n);
                }
            }
            (!names.is_empty()).then(|| names.join(" / "))
        }
        SymbolRole::Interruption { name } => Some(name.clone()),
        SymbolRole::PotentialFeed { potential } => {
            project.potential(*potential).map(|p| p.name.clone())
        }
        SymbolRole::Unassigned => None,
    }
}

/// Terminals of one strip further apart than this (mm, between symbol
/// origins) start a new run, which names its strip again.
const STRIP_RUN_GAP: f64 = 20.0;

/// Item id of the bridge bars of a page's jumper groups (no model object
/// has this id).
const BARS_ITEM: ItemId = ItemId(u64::MAX - 1);

/// Height of a connected terminal's bridge bar in its symbol, in mm
/// (`terminal_bridged.toml`: the dot sits on the upper lead at this height).
const BRIDGE_BAR_Y: f64 = 1.4;

/// The symbol that carries the bridge dot.
const BRIDGED_SYMBOL: &str = "iec/terminals/terminal_bridged";

/// One heavy bar per run of connected terminals on a page, from the first
/// lead to the last, never past them: the connected terminals of a jumper
/// group on one strip that are chained by neighbours at most
/// [`STRIP_RUN_GAP`] apart. A lone terminal gets none.
fn bridge_bars(project: &Project, page: PageId) -> Vec<(Vec2, Vec2)> {
    let mut groups: HashMap<(novo_core::DeviceId, u32), Vec<Point>> = HashMap::new();
    for id in project.page_contents(page).symbols {
        let Some(sym) = project.symbol(id) else {
            continue;
        };
        let SymbolRole::Terminal { terminals } = &sym.role else {
            continue;
        };
        if sym.symbol.0 != BRIDGED_SYMBOL {
            continue;
        }
        let [t] = terminals.as_slice() else {
            continue;
        };
        if let Some((strip, group)) = project
            .terminal(*t)
            .and_then(|t| t.jumper.map(|j| (t.strip, j)))
        {
            groups
                .entry((strip, group))
                .or_default()
                .push(sym.placement.apply(Point::new(0.0, BRIDGE_BAR_Y)));
        }
    }
    let mut bars = Vec::new();
    for (_, mut leads) in groups {
        leads.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        let mut start = 0;
        for i in 1..=leads.len() {
            let broken = i == leads.len()
                || (leads[i].x - leads[i - 1].x).hypot(leads[i].y - leads[i - 1].y) > STRIP_RUN_GAP;
            if broken {
                if i - start >= 2 {
                    bars.push((vec(leads[start]), vec(leads[i - 1])));
                }
                start = i;
            }
        }
    }
    bars.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.0.y.total_cmp(&b.0.y)));
    bars
}

/// The symbols that name their strip on `page`: the first terminal (left
/// most, then top most) of each run of adjacent terminals of one strip, as
/// IEC 61082 drawings name `-X1` once beside a group of its terminals.
/// Maps each to the strip's tag.
fn strip_heads(project: &Project, page: PageId) -> HashMap<SymbolId, String> {
    let mut by_strip: HashMap<novo_core::DeviceId, Vec<(Point, SymbolId)>> = HashMap::new();
    for id in project.page_contents(page).symbols {
        let Some(sym) = project.symbol(id) else {
            continue;
        };
        let SymbolRole::Terminal { terminals } = &sym.role else {
            continue;
        };
        if let Some(t) = terminals.first().and_then(|t| project.terminal(*t)) {
            by_strip
                .entry(t.strip)
                .or_default()
                .push((sym.placement.origin, id));
        }
    }
    let mut heads = HashMap::new();
    for (strip, mut syms) in by_strip {
        let Some(tag) = project.device(strip).map(|d| d.tag.to_string()) else {
            continue;
        };
        if tag.is_empty() {
            continue;
        }
        syms.sort_by(|a, b| {
            a.0.x
                .total_cmp(&b.0.x)
                .then(a.0.y.total_cmp(&b.0.y))
                .then(a.1.cmp(&b.1))
        });
        // Runs are the groups of terminals chained by neighbours at most
        // STRIP_RUN_GAP apart, whatever their order (two rows side by side
        // stay two runs); each run is named at its first terminal in the
        // sort order (leftmost, then topmost).
        let near = |a: Point, b: Point| (a.x - b.x).hypot(a.y - b.y) <= STRIP_RUN_GAP;
        let mut run: Vec<usize> = (0..syms.len()).collect();
        fn root(run: &mut [usize], mut i: usize) -> usize {
            while run[i] != i {
                run[i] = run[run[i]];
                i = run[i];
            }
            i
        }
        for i in 0..syms.len() {
            for j in i + 1..syms.len() {
                if near(syms[i].0, syms[j].0) {
                    let (a, b) = (root(&mut run, i), root(&mut run, j));
                    // The earlier terminal stays the root, so it heads the run.
                    run[a.max(b)] = a.min(b);
                }
            }
        }
        for (i, (_, id)) in syms.iter().enumerate() {
            if root(&mut run, i) == i {
                heads.insert(*id, tag.clone());
            }
        }
    }
    heads
}

/// A direct-on-line motor starter on one A3 page: the Gate 1 circuit, drawn
/// with placeholder symbols. Used by the app until real pages can be drawn.
pub fn demo_document() -> novo_core::Result<(novo_core::Document, PageId)> {
    use novo_core::{DeviceTag, Document, Pin, Placement as MPlacement};
    let mut doc = Document::default();
    let (page, _) = doc.transact("Demo motor starter", |tx| {
        let page = tx.add_page("1")?;
        tx.update::<novo_core::Page>(page, |p| p.title = "DOL motor starter".into())?;
        let three = |a: &str, b: &str, c: &str, d: &str, e: &str, f: &str| {
            vec![
                Pin::new(a, -8.0, -8.0),
                Pin::new(b, 0.0, -8.0),
                Pin::new(c, 8.0, -8.0),
                Pin::new(d, -8.0, 8.0),
                Pin::new(e, 0.0, 8.0),
                Pin::new(f, 8.0, 8.0),
            ]
        };
        let two = |a: &str, b: &str| vec![Pin::new(a, 0.0, -8.0), Pin::new(b, 0.0, 8.0)];
        let dev = |tx: &mut novo_core::Tx<'_>, tag: &str, what: &str| {
            tx.add_device(DeviceTag::parse(tag)?, what)
        };
        let role = |device, main| SymbolRole::Device { device, main };
        let at = |x: f64, y: f64| MPlacement::at(Point::new(x, y));

        let q1 = dev(tx, "-Q1", "Motor protection breaker")?;
        let k1 = dev(tx, "-K1", "Main contactor")?;
        let m1 = dev(tx, "-M1", "Motor 4 kW")?;
        let s1 = dev(tx, "-S1", "Stop")?;
        let s2 = dev(tx, "-S2", "Start")?;

        // Main circuit.
        let q = tx.place_symbol(
            page,
            "iec/mpcb3",
            at(80.0, 60.0),
            three("1", "3", "5", "2", "4", "6"),
            role(q1, true),
        )?;
        let k = tx.place_symbol(
            page,
            "iec/contactor3",
            at(80.0, 108.0),
            three("1", "3", "5", "2", "4", "6"),
            role(k1, false),
        )?;
        let m = tx.place_symbol(
            page,
            "iec/motor3",
            at(80.0, 160.0),
            vec![
                Pin::new("U1", -8.0, -8.0),
                Pin::new("V1", 0.0, -8.0),
                Pin::new("W1", 8.0, -8.0),
            ],
            role(m1, true),
        )?;
        for i in 0..3u16 {
            tx.connect(Endpoint::pin(q, 3 + i), Endpoint::pin(k, i))?;
            tx.connect(Endpoint::pin(k, 3 + i), Endpoint::pin(m, i))?;
        }

        // Control circuit with self-hold.
        let stop = tx.place_symbol(
            page,
            "iec/nc",
            at(200.0, 60.0),
            two("11", "12"),
            role(s1, true),
        )?;
        let start = tx.place_symbol(
            page,
            "iec/no",
            at(200.0, 108.0),
            two("13", "14"),
            role(s2, true),
        )?;
        let hold = tx.place_symbol(
            page,
            "iec/no",
            at(232.0, 108.0),
            two("13", "14"),
            role(k1, false),
        )?;
        let coil = tx.place_symbol(
            page,
            "iec/coil",
            at(200.0, 160.0),
            two("A1", "A2"),
            role(k1, true),
        )?;
        let j1 = tx.add_junction(page, Point::new(200.0, 84.0))?;
        let j2 = tx.add_junction(page, Point::new(200.0, 132.0))?;
        let w = tx.connect(Endpoint::pin(stop, 1), Endpoint::junction(j1))?;
        tx.update::<novo_core::Wire>(w, |w| w.number = Some("3".into()))?;
        tx.connect(Endpoint::junction(j1), Endpoint::pin(start, 0))?;
        let w = tx.connect(Endpoint::junction(j1), Endpoint::pin(hold, 0))?;
        tx.update::<novo_core::Wire>(w, |w| w.route = vec![Point::new(232.0, 84.0)])?;
        tx.connect(Endpoint::pin(start, 1), Endpoint::junction(j2))?;
        let w = tx.connect(Endpoint::pin(hold, 1), Endpoint::junction(j2))?;
        tx.update::<novo_core::Wire>(w, |w| w.route = vec![Point::new(232.0, 132.0)])?;
        let w = tx.connect(Endpoint::junction(j2), Endpoint::pin(coil, 0))?;
        tx.update::<novo_core::Wire>(w, |w| w.number = Some("4".into()))?;
        Ok(page)
    })?;
    Ok((doc, page))
}

/// A key for symbols whose artwork is not known, so callers can count them.
pub fn is_placeholder(art: &dyn SymbolArt, sym: &SymbolInstance) -> bool {
    art.artwork(sym).is_none()
}

/// Convenience for tests and demos: a symbol key from a string.
pub fn key(s: &str) -> SymbolKey {
    SymbolKey(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{DeviceTag, Document, Pin, Placement as MPlacement};

    fn doc() -> (Document, PageId, SymbolId, SymbolId, WireId) {
        let mut doc = Document::default();
        let ((page, k1, s1, w), _) = doc
            .transact("setup", |tx| {
                let page = tx.add_page("1")?;
                let k = tx.add_device(DeviceTag::parse("-K1")?, "Contactor")?;
                let s = tx.add_device(DeviceTag::parse("-S1")?, "Start")?;
                let k1 = tx.place_symbol(
                    page,
                    "iec/coil",
                    MPlacement::at(Point::new(100.0, 100.0)),
                    vec![Pin::new("A1", 0.0, -5.0), Pin::new("A2", 0.0, 5.0)],
                    SymbolRole::Device {
                        device: k,
                        main: true,
                    },
                )?;
                let s1 = tx.place_symbol(
                    page,
                    "iec/no",
                    MPlacement::at(Point::new(100.0, 60.0)),
                    vec![Pin::new("13", 0.0, -5.0), Pin::new("14", 0.0, 5.0)],
                    SymbolRole::Device {
                        device: s,
                        main: true,
                    },
                )?;
                let w = tx.connect(Endpoint::pin(s1, 1), Endpoint::pin(k1, 0))?;
                Ok((page, k1, s1, w))
            })
            .unwrap();
        (doc, page, k1, s1, w)
    }

    #[test]
    fn builds_page_items() {
        let (doc, page, k1, s1, w) = doc();
        let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let scene = ps.scene();
        // Frame + 2 symbols + 1 wire.
        assert_eq!(scene.len(), 4);
        let coil = scene.get(item_id(k1)).unwrap();
        assert_eq!(coil.labels[0].text, "-K1");
        let snaps: Vec<Vec2> = coil.snap_points.iter().map(|s| s.pos).collect();
        assert_eq!(snaps, vec![Vec2::new(100.0, 95.0), Vec2::new(100.0, 105.0)]);
        // The wire runs from S1:14 to K1:A1.
        let wire = scene.get(item_id(w)).unwrap();
        let Shape::Polyline { points, .. } = &wire.primitives[0].shape else {
            panic!()
        };
        assert_eq!(
            points,
            &vec![Vec2::new(100.0, 65.0), Vec2::new(100.0, 95.0)]
        );
        assert_eq!(ps.model_id(item_id(s1)), Some(AnyId::Symbol(s1)));
        // Picking the wire on the canvas gives back the model wire.
        let hit = scene.pick(Vec2::new(100.2, 80.0), 1.0, 2.0).unwrap();
        assert_eq!(ps.model_id(hit.id), Some(AnyId::Wire(w)));
    }

    #[test]
    fn notes_are_drawn_with_the_symbol_and_survive_edits() {
        let (mut doc, page, k1, s1, _w) = doc();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let note = |lines: &[&str], warning| SymbolNote {
            symbol: k1,
            lines: lines.iter().map(|l| l.to_string()).collect(),
            warning,
            anchor: NoteAnchor::TopRight,
        };
        let labels =
            |ps: &PageScene| -> Vec<Label> { ps.scene().get(item_id(k1)).unwrap().labels.clone() };
        assert!(ps.set_notes(
            doc.project(),
            [note(&["13-14 /1.2", "23-24 /1.3"], false)],
            &PlaceholderArt
        ));
        let l = labels(&ps);
        // The tag stays first; the note follows, above the ink's top right.
        assert_eq!(l[0].text, "-K1");
        let n = l.last().unwrap();
        assert_eq!(n.text, "13-14 /1.2\n23-24 /1.3");
        assert_eq!(n.color, ps.style.note_color);
        assert_eq!(n.anchor, Anchor::BottomLeft);
        assert_eq!(n.height, ps.style.tag_height);
        let ink = ps.scene().get(item_id(k1)).unwrap().primitives[0]
            .shape
            .bounds();
        assert!(
            n.pos.x > ink.max.x && n.pos.y < ink.min.y,
            "{:?} vs {ink:?}",
            n.pos
        );
        // The same notes again change nothing.
        assert!(!ps.set_notes(
            doc.project(),
            [note(&["13-14 /1.2", "23-24 /1.3"], false)],
            &PlaceholderArt
        ));
        // Moving the symbol carries its note along.
        let (_, changes) = doc
            .transact("move", |tx| {
                tx.move_symbol(k1, MPlacement::at(Point::new(150.0, 100.0)))
            })
            .unwrap();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        let moved = labels(&ps);
        assert_eq!(moved.last().unwrap().text, n.text);
        assert!((moved.last().unwrap().pos.x - n.pos.x - 50.0).abs() < 1e-3);
        // So does a full rebuild.
        ps.rebuild(doc.project(), &PlaceholderArt);
        assert_eq!(labels(&ps).len(), moved.len());
        // A warning switches colour; an empty note is dropped.
        ps.set_notes(
            doc.project(),
            [note(&["13-14 /1.2"], true)],
            &PlaceholderArt,
        );
        assert_eq!(
            labels(&ps).last().unwrap().color,
            ps.style.note_warning_color
        );
        assert!(ps.set_notes(doc.project(), [note(&[" "], false)], &PlaceholderArt));
        assert_eq!(labels(&ps).len(), 1);
        assert_eq!(ps.scene().get(item_id(s1)).unwrap().labels.len(), 1);
    }

    #[test]
    fn note_anchors_place_contact_images_below_and_references_under_the_tag() {
        let (doc, page, k1, s1, _w) = doc();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let note = |symbol, anchor, warning| SymbolNote {
            symbol,
            lines: vec!["13-14 /1.2".into()],
            warning,
            anchor,
        };
        ps.set_notes(
            doc.project(),
            [
                note(k1, NoteAnchor::Below, true),
                note(s1, NoteAnchor::Right, false),
            ],
            &PlaceholderArt,
        );
        let item = |id| ps.scene().get(item_id(id)).unwrap().clone();
        let ink = |it: &Item| {
            it.primitives
                .iter()
                .fold(crate::Aabb::EMPTY, |b, p| b.union(&p.shape.bounds()))
        };
        // A coil's contact image: under the ink, left-aligned with it.
        let coil = item(k1);
        let below = coil.labels.last().unwrap();
        assert_eq!(below.anchor, Anchor::TopLeft);
        assert_eq!(below.pos.x, ink(&coil).min.x);
        assert!(below.pos.y > ink(&coil).max.y);
        // A warning keeps its colour under selection; plain notes do not.
        assert!(below.keep_color);
        // A contact's reference: one line under its tag, aligned with it.
        let contact = item(s1);
        let (tag, reference) = (&contact.labels[0], contact.labels.last().unwrap());
        assert_eq!(reference.pos.x, tag.bounds().min.x);
        assert!(reference.pos.y >= tag.bounds().max.y);
        assert!(!reference.keep_color);
    }

    /// Two rows of one strip over the same x range (supply terminals at
    /// the top of a page, motor outputs at the bottom) are two runs, one
    /// name each, not one run per terminal.
    #[test]
    fn two_rows_of_a_strip_are_two_runs() {
        let aligned: &[(f64, f64)] = &[
            (100.0, 50.0),
            (105.0, 50.0),
            (110.0, 50.0),
            (100.0, 150.0),
            (105.0, 150.0),
            (110.0, 150.0),
        ];
        let staggered: &[(f64, f64)] = &[
            (100.0, 50.0),
            (102.5, 120.0),
            (110.0, 50.0),
            (112.5, 120.0),
            (120.0, 50.0),
            (122.5, 120.0),
        ];
        for (at, expect) in [(aligned, vec![0, 3]), (staggered, vec![0, 1])] {
            let mut doc = Document::default();
            let ((page, syms), _) = doc
                .transact("strip", |tx| {
                    let page = tx.add_page("1")?;
                    let x1 = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
                    let mut syms = Vec::new();
                    for (i, &(x, y)) in at.iter().enumerate() {
                        let t = tx.add_terminal(x1, (i + 1).to_string())?;
                        syms.push(tx.place_symbol(
                            page,
                            "iec/terminal",
                            MPlacement::at(Point::new(x, y)),
                            vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                            SymbolRole::Terminal { terminals: vec![t] },
                        )?);
                    }
                    Ok((page, syms))
                })
                .unwrap();
            let heads = strip_heads(doc.project(), page);
            let named: Vec<usize> = (0..syms.len())
                .filter(|&i| heads.contains_key(&syms[i]))
                .collect();
            assert_eq!(named, expect, "{at:?}");
        }
    }

    #[test]
    fn a_run_of_connected_terminals_gets_one_bar_first_lead_to_last() {
        let mut doc = Document::default();
        let ((page, last), _) = doc
            .transact("strip", |tx| {
                let page = tx.add_page("1")?;
                let x1 = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
                // Three connected in a row, one far away in the same group,
                // one connected alone in another group.
                let at = [(100.0, 50.0, 1), (108.0, 50.0, 1), (116.0, 50.0, 1)]
                    .into_iter()
                    .chain([(200.0, 50.0, 1), (100.0, 150.0, 2)]);
                let mut last = None;
                for (i, (x, y, group)) in at.enumerate() {
                    let t = tx.add_terminal(x1, (i + 1).to_string())?;
                    tx.update::<novo_core::Terminal>(t, |t| t.jumper = Some(group))?;
                    let sym = tx.place_symbol(
                        page,
                        BRIDGED_SYMBOL,
                        MPlacement::at(Point::new(x, y)),
                        vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                        SymbolRole::Terminal { terminals: vec![t] },
                    )?;
                    if i == 2 {
                        last = Some((t, sym));
                    }
                }
                Ok((page, last.unwrap()))
            })
            .unwrap();
        let bars = bridge_bars(doc.project(), page);
        assert_eq!(
            bars,
            vec![(Vec2::new(100.0, 51.4), Vec2::new(116.0, 51.4))],
            "one bar across the run; lone terminals draw none"
        );
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        assert!(ps.scene().contains(BARS_ITEM));
        // Taking the far terminal's neighbour into the run extends the bar.
        let (_, changes) = doc
            .transact("move", |tx| {
                tx.move_symbol(last.1, MPlacement::at(Point::new(124.0, 50.0)))
            })
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert_eq!(
            bridge_bars(doc.project(), page)[0].1,
            Vec2::new(124.0, 51.4)
        );
        // Unjumpering them removes the bars.
        let (_, changes) = doc
            .transact("free", |tx| {
                let ids: Vec<_> = tx.project().terminals().map(|t| t.id).collect();
                for t in ids {
                    tx.update::<novo_core::Terminal>(t, |t| t.jumper = None)?;
                }
                Ok(())
            })
            .unwrap();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        assert!(!ps.scene().contains(BARS_ITEM));
    }

    #[test]
    fn strip_label_clears_the_terminal_name_on_the_left() {
        // A library terminal that puts its own name left of the pins.
        struct LeftName;
        impl SymbolArt for LeftName {
            fn artwork(&self, _: &SymbolInstance) -> Option<Vec<Primitive>> {
                None
            }
            fn tag_anchor(&self, _: &SymbolInstance) -> Option<(Vec2, Anchor)> {
                Some((Vec2::new(-5.0, 0.0), Anchor::CenterRight))
            }
        }
        for rotation in [novo_core::Rotation::R0, novo_core::Rotation::R90] {
            let mut doc = Document::default();
            let ((page, sym), _) = doc
                .transact("strip", |tx| {
                    let page = tx.add_page("1")?;
                    let x1 = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
                    let t = tx.add_terminal(x1, "1")?;
                    let sym = tx.place_symbol(
                        page,
                        "iec/terminal",
                        MPlacement {
                            origin: Point::new(100.0, 50.0),
                            rotation,
                            mirrored: false,
                        },
                        vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                        SymbolRole::Terminal { terminals: vec![t] },
                    )?;
                    Ok((page, sym))
                })
                .unwrap();
            let ps = PageScene::new(doc.project(), page, &LeftName);
            let item = ps.scene().get(item_id(sym)).unwrap();
            let name = item.labels.iter().find(|l| l.text == "1").unwrap();
            let strip = item.labels.iter().find(|l| l.text == "-X1").unwrap();
            let (a, b) = (name.bounds(), strip.bounds());
            assert!(
                a.min.x >= b.max.x
                    || b.min.x >= a.max.x
                    || a.min.y >= b.max.y
                    || b.min.y >= a.max.y,
                "{rotation:?}: strip label {b:?} overlaps the terminal name {a:?}"
            );
        }
    }

    #[test]
    fn each_run_of_terminals_names_its_strip_once() {
        let mut doc = Document::default();
        let ((page, syms), _) = doc
            .transact("strip", |tx| {
                let page = tx.add_page("1")?;
                let x1 = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
                let mut syms = Vec::new();
                // Two runs: 1..3 at x 100..110, then 4 at x 200.
                for (i, x) in [100.0, 105.0, 110.0, 200.0].into_iter().enumerate() {
                    let t = tx.add_terminal(x1, (i + 1).to_string())?;
                    syms.push(tx.place_symbol(
                        page,
                        "iec/terminal",
                        MPlacement::at(Point::new(x, 50.0)),
                        vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                        SymbolRole::Terminal { terminals: vec![t] },
                    )?);
                }
                Ok((page, syms))
            })
            .unwrap();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let named = |ps: &PageScene| -> Vec<usize> {
            (0..syms.len())
                .filter(|&i| {
                    ps.scene()
                        .get(item_id(syms[i]))
                        .unwrap()
                        .labels
                        .iter()
                        .any(|l| l.text == "-X1")
                })
                .collect()
        };
        assert_eq!(named(&ps), vec![0, 3]);
        // The name sits left of the terminal, its own name stays first.
        let first = ps.scene().get(item_id(syms[0])).unwrap();
        assert_eq!(first.labels[0].text, "1");
        let strip = first.labels.iter().find(|l| l.text == "-X1").unwrap();
        assert_eq!(strip.anchor, Anchor::CenterRight);
        assert!(strip.pos.x < 100.0);
        // Moving the first terminal away hands the name to the next one.
        let (_, changes) = doc
            .transact("move", |tx| {
                tx.move_symbol(syms[0], MPlacement::at(Point::new(300.0, 50.0)))
            })
            .unwrap();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        assert_eq!(named(&ps), vec![0, 1, 3]);
        // Renaming the strip renames its runs.
        let x1 = doc
            .project()
            .terminal(match &doc.project().symbol(syms[1]).unwrap().role {
                SymbolRole::Terminal { terminals } => terminals[0],
                _ => unreachable!(),
            })
            .unwrap()
            .strip;
        let (_, changes) = doc
            .transact("retag", |tx| {
                tx.update::<novo_core::Device>(x1, |d| d.tag = DeviceTag::parse("-X2").unwrap())
            })
            .unwrap();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        let texts: Vec<String> = ps
            .scene()
            .get(item_id(syms[1]))
            .unwrap()
            .labels
            .iter()
            .map(|l| l.text.clone())
            .collect();
        assert!(texts.contains(&"-X2".to_string()), "{texts:?}");
    }

    #[test]
    fn moving_a_symbol_updates_its_wires() {
        let (mut doc, page, k1, _s1, w) = doc();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let (_, changes) = doc
            .transact("move", |tx| {
                tx.move_symbol(
                    k1,
                    MPlacement {
                        origin: Point::new(140.0, 100.0),
                        rotation: Rotation::R90,
                        mirrored: false,
                    },
                )
            })
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        let wire = ps.scene().get(item_id(w)).unwrap();
        let Shape::Polyline { points, .. } = &wire.primitives[0].shape else {
            panic!()
        };
        // A1 at (0, -5) rotated a quarter turn counter-clockwise is (-5, 0).
        assert_eq!(points.last().copied(), Some(Vec2::new(135.0, 100.0)));
        let coil = ps.scene().get(item_id(k1)).unwrap();
        assert!(
            coil.snap_points
                .iter()
                .any(|s| s.pos.distance(Vec2::new(135.0, 100.0)) < 1e-4)
        );
    }

    #[test]
    fn retagging_and_removal() {
        let (mut doc, page, k1, s1, w) = doc();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let device = doc.project().symbol(k1).unwrap().device().unwrap();
        let (_, changes) = doc
            .transact("retag", |tx| tx.set_tag(device, DeviceTag::parse("-K7")?))
            .unwrap();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        assert_eq!(ps.scene().get(item_id(k1)).unwrap().labels[0].text, "-K7");

        let (_, changes) = doc.transact("delete", |tx| tx.remove(s1)).unwrap();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        assert!(!ps.scene().contains(item_id(s1)));
        // Deleting a symbol deletes its wires in the model; both leave the canvas.
        assert!(doc.project().wire(w).is_none());
        assert!(!ps.scene().contains(item_id(w)));
        assert_eq!(ps.scene().len(), 2);
    }

    #[test]
    fn renaming_a_potential_relabels_its_feeds() {
        let (mut doc, page, ..) = doc();
        let ((pot, feed), _) = doc
            .transact("feed", |tx| {
                let pot = tx.add_potential("L+", novo_core::PotentialKind::default())?;
                let feed = tx.place_symbol(
                    page,
                    "iec/feed",
                    MPlacement::at(Point::new(40.0, 40.0)),
                    vec![Pin::new("1", 0.0, 5.0)],
                    SymbolRole::PotentialFeed { potential: pot },
                )?;
                Ok((pot, feed))
            })
            .unwrap();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        assert_eq!(ps.scene().get(item_id(feed)).unwrap().labels[0].text, "L+");
        let (_, changes) = doc
            .transact("rename", |tx| {
                tx.update::<novo_core::Potential>(pot, |p| p.name = "24V".into())
            })
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert_eq!(ps.scene().get(item_id(feed)).unwrap().labels[0].text, "24V");
    }

    #[test]
    fn terminals_show_their_name_and_never_steal_a_neighbours_click() {
        let (mut doc, page, ..) = doc();
        let ((t1, s1, s2), _) = doc
            .transact("strip", |tx| {
                let strip = tx.add_terminal_strip(DeviceTag::parse("=A1+CAB-X1")?)?;
                let mut syms = Vec::new();
                let mut first = None;
                for (i, name) in ["1", "2"].into_iter().enumerate() {
                    let t = tx.add_terminal(strip, name)?;
                    first.get_or_insert(t);
                    syms.push(tx.place_symbol(
                        page,
                        "iec/terminal",
                        // 8 mm apart, as on the Gate 1 strip.
                        MPlacement::at(Point::new(40.0 + 8.0 * i as f64, 140.0)),
                        vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                        SymbolRole::Terminal { terminals: vec![t] },
                    )?);
                }
                Ok((first.unwrap(), syms[0], syms[1]))
            })
            .unwrap();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        assert_eq!(ps.scene().get(item_id(s1)).unwrap().labels[0].text, "1");
        // Just inside -X1:2's outline, where -X1:1's tag box runs over it:
        // the body wins over the neighbour's text.
        let p = Vec2::new(48.0, 138.5);
        let tag = &ps.scene().get(item_id(s1)).unwrap().labels[0];
        assert!(
            tag.distance(p) < 0.5,
            "the repro needs the tag over the body"
        );
        let hit = ps.scene().pick(p, 1.0, 1.0).unwrap();
        assert_eq!(hit.id, item_id(s2));
        assert_eq!(hit.label, None);
        // Renaming the terminal relabels its symbol.
        let (_, changes) = doc
            .transact("rename", |tx| {
                tx.update::<novo_core::Terminal>(t1, |t| t.name = "PE".into())
            })
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert_eq!(ps.scene().get(item_id(s1)).unwrap().labels[0].text, "PE");
    }

    #[test]
    fn tag_stays_right_of_mirrored_symbols() {
        let (mut doc, page, k1, ..) = doc();
        let (_, _) = doc
            .transact("mirror", |tx| {
                tx.move_symbol(
                    k1,
                    MPlacement {
                        origin: Point::new(100.0, 100.0),
                        rotation: Rotation::R0,
                        mirrored: true,
                    },
                )
            })
            .unwrap();
        let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let label = &ps.scene().get(item_id(k1)).unwrap().labels[0];
        assert_eq!(label.anchor, Anchor::CenterLeft);
        assert!(label.pos.x > 100.0, "{label:?}");
    }

    #[test]
    fn junction_glyphs_point_where_they_say() {
        let style = ModelStyle::default();
        let ends = |g| -> Vec<Vec2> {
            junction_glyph(g, Vec2::ZERO, &style)
                .into_iter()
                .filter_map(|p| match p.shape {
                    Shape::Line { b, .. } => Some(b / JUNCTION_STUB),
                    _ => None,
                })
                .collect()
        };
        // A T with its stem down: stem down, through line left and right.
        let tee = ends(JunctionGlyph::Tee { stem: 0 });
        assert_eq!(tee.len(), 3);
        for d in [Vec2::Y, Vec2::X, -Vec2::X] {
            assert!(tee.iter().any(|e| e.distance(d) < 1e-6), "{tee:?}");
        }
        // One quarter turn clockwise: the stem points left.
        let turned = ends(JunctionGlyph::Tee { stem: 1 });
        assert!(turned.iter().any(|e| e.distance(-Vec2::X) < 1e-6));
        assert!(!turned.iter().any(|e| e.distance(Vec2::X) < 1e-6));
        // The default corner joins right and down.
        let corner = ends(JunctionGlyph::Corner { quarter_turns: 0 });
        assert_eq!(corner.len(), 2);
        assert!(corner.iter().any(|e| e.distance(Vec2::X) < 1e-6));
        assert!(corner.iter().any(|e| e.distance(Vec2::Y) < 1e-6));
    }

    #[test]
    fn wire_end_glyph_flag_clears_the_wire() {
        let style = ModelStyle::default();
        let prims = wire_end_glyph(Vec2::ZERO, Vec2::X, &style);
        assert_eq!(prims.len(), 3, "ring, pole and pennant");
        // The flag stands to the side of the line the wire ran along (the
        // x axis) and starts past the ring, never on the wire.
        let flag = prims[1].shape.bounds();
        assert!(flag.min.x >= style.wire_end_radius - 1e-4);
        assert!(
            flag.max.y.abs() > style.wire_end_radius || flag.min.y.abs() > style.wire_end_radius
        );
        // A zero direction still draws something sensible.
        assert_eq!(wire_end_glyph(Vec2::ZERO, Vec2::ZERO, &style).len(), 3);
    }

    #[test]
    fn junction_dots_use_the_accent() {
        let style = ModelStyle::default();
        assert_ne!(style.joint_color, style.wire.color);
        let dot = &junction_glyph(JunctionGlyph::Dot, Vec2::ZERO, &style)[0];
        assert_eq!(dot.fill, Some(style.joint_color));
        // A wire landing on a pin is the wire colour, not the copper.
        assert_eq!(style.pin_joint_color, style.wire.color);
        assert_ne!(style.pin_joint_color, style.joint_color);
    }

    #[test]
    fn junction_glyph_follows_topology() {
        use JunctionGlyph::*;
        let (x, y) = (Vec2::X, Vec2::Y);
        assert_eq!(
            JunctionGlyph::from_directions([x, -x, y * 3.0]),
            Tee { stem: 0 }
        );
        assert_eq!(JunctionGlyph::from_directions([-y, y, x]), Tee { stem: 3 });
        assert_eq!(
            JunctionGlyph::from_directions([x, y]),
            Corner { quarter_turns: 0 }
        );
        assert_eq!(
            JunctionGlyph::from_directions([-x, -y]),
            Corner { quarter_turns: 2 }
        );
        // Fewer than three conductors: no dot (DESIGN.md §2).
        assert_eq!(JunctionGlyph::from_directions([x, -x]), Plain);
        assert_eq!(JunctionGlyph::from_directions([x]), Plain);
        assert_eq!(JunctionGlyph::from_directions([]), Plain);
        assert!(junction_glyph(Plain, Vec2::ZERO, &ModelStyle::default()).is_empty());
        assert_eq!(JunctionGlyph::from_directions([x, -x, y, -y]), Dot);
        // Out-of-range quarter turns wrap instead of overflowing.
        assert_eq!(
            junction_glyph(Tee { stem: 255 }, Vec2::ZERO, &ModelStyle::default()).len(),
            4
        );

        // Wires are counted, not sides: three or more wires always get a
        // dot, even when they leave on only two sides.
        let dotted = |g: JunctionGlyph| {
            junction_glyph(g, Vec2::ZERO, &ModelStyle::default())
                .iter()
                .any(|p| matches!(p.shape, Shape::Circle { .. }))
        };
        assert_eq!(JunctionGlyph::from_directions([x, y, y]), Dot);
        assert_eq!(JunctionGlyph::from_directions([x, x, y, y]), Dot);
        assert_eq!(JunctionGlyph::from_directions([x, y, Vec2::ZERO]), Dot);
        assert_eq!(
            JunctionGlyph::from_directions([x, -x, y, y]),
            Tee { stem: 0 }
        );
        for dirs in [
            vec![x, y, y],
            vec![x, x, y, y],
            vec![x, -x, y],
            vec![x, -x, y, y],
            vec![x, -x, y, -y],
        ] {
            assert!(
                dotted(JunctionGlyph::from_directions(dirs.clone())),
                "{dirs:?}"
            );
        }
        assert!(!dotted(JunctionGlyph::from_directions([x, y])));

        // The demo's self-hold branch: both junctions are Ts, and removing
        // the branch wire leaves a straight run, drawn with no dot.
        let (mut doc, page) = demo_document().unwrap();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let lines = |ps: &PageScene, j: JunctionId| {
            ps.scene()
                .get(item_id(j))
                .unwrap()
                .primitives
                .iter()
                .filter(|p| matches!(p.shape, Shape::Line { .. }))
                .count()
        };
        let junctions = doc.project().page_contents(page).junctions;
        assert_eq!(junctions.len(), 2);
        for &j in &junctions {
            assert_eq!(lines(&ps, j), 3, "a T has three stubs");
        }
        let j = junctions[0];
        let pos = doc.project().junction(j).unwrap().position;
        let branch = doc
            .project()
            .wires_at(Endpoint::junction(j))
            .find(|&w| {
                let wire = doc.project().wire(w).unwrap();
                let other = if wire.from == Endpoint::junction(j) {
                    wire.to
                } else {
                    wire.from
                };
                let first = wire
                    .route
                    .first()
                    .copied()
                    .or_else(|| doc.project().endpoint_position(other))
                    .unwrap();
                (first.y - pos.y).abs() < 1e-6
            })
            .expect("a horizontal branch");
        let (_, changes) = doc.transact("cut", |tx| tx.remove(branch)).unwrap();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        assert!(
            ps.scene().get(item_id(j)).unwrap().primitives.is_empty(),
            "a straight run has no dot"
        );
    }

    #[test]
    fn three_wires_on_two_sides_keep_their_dot() {
        let mut doc = Document::default();
        let ((page, j), _) = doc
            .transact("node", |tx| {
                let page = tx.add_page("1")?;
                let j = tx.add_junction(page, Point::new(50.0, 50.0))?;
                // One wire leaves right, two leave down.
                let a = tx.place_symbol(
                    page,
                    "iec/x",
                    MPlacement::at(Point::new(60.0, 50.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Unassigned,
                )?;
                let b = tx.place_symbol(
                    page,
                    "iec/x",
                    MPlacement::at(Point::new(50.0, 60.0)),
                    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)],
                    SymbolRole::Unassigned,
                )?;
                tx.connect(Endpoint::junction(j), Endpoint::pin(a, 0))?;
                tx.connect(Endpoint::junction(j), Endpoint::pin(b, 0))?;
                tx.connect(Endpoint::junction(j), Endpoint::pin(b, 1))?;
                Ok((page, j))
            })
            .unwrap();
        let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let node = ps.scene().get(item_id(j)).unwrap();
        assert!(
            node.primitives
                .iter()
                .any(|p| matches!(p.shape, Shape::Circle { .. })),
            "a three-wire node must show its dot"
        );
    }

    #[test]
    fn text_records_are_drawn_and_follow_their_hosts() {
        use novo_core::{TextPlacement, Wire};
        let (mut doc, page, k1, _s1, w) = doc();
        let at = |x: f64, y: f64| TextPlacement {
            offset: Point::new(x, y),
            ..Default::default()
        };
        let ((note, tag, pot_text), _) = doc
            .transact("texts", |tx| {
                let note = tx.add_text(
                    page,
                    TextContent::Literal {
                        text: "Note".into(),
                    },
                    at(30.0, 40.0),
                    None,
                )?;
                let tag = tx.add_text(
                    page,
                    TextContent::DeviceTag,
                    TextPlacement {
                        offset: Point::new(-8.0, 0.0),
                        rotation: Rotation::R90,
                        anchor: TextAnchor::MiddleRight,
                    },
                    Some(k1.into()),
                )?;
                let pot_text =
                    tx.add_text(page, TextContent::Potential, at(2.0, 0.0), Some(w.into()))?;
                Ok((note, tag, pot_text))
            })
            .unwrap();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let label =
            |ps: &PageScene, t: TextId| ps.scene().get(item_id(t)).map(|i| i.labels[0].clone());
        let note_label = label(&ps, note).unwrap();
        assert_eq!(note_label.text, "Note");
        assert_eq!(note_label.pos, Vec2::new(30.0, 40.0));
        let tag_label = label(&ps, tag).unwrap();
        assert_eq!(tag_label.text, "-K1");
        assert_eq!(tag_label.pos, Vec2::new(92.0, 100.0));
        assert_eq!(tag_label.anchor, Anchor::CenterRight);
        assert_eq!(tag_label.rotation, -FRAC_PI_2);
        // The tag text replaces the symbol's own tag: never drawn twice.
        let coil = ps.scene().get(item_id(k1)).unwrap();
        assert!(
            coil.labels.iter().all(|l| l.text != "-K1"),
            "{:?}",
            coil.labels
        );
        // The wire defines no potential yet, so its label shows nothing.
        assert!(label(&ps, pot_text).is_none());
        // Picking a text gives back the text record.
        let hit = ps.scene().pick(Vec2::new(31.0, 39.0), 0.5, 2.0).unwrap();
        assert_eq!(ps.model_id(hit.id), Some(AnyId::Text(note)));

        // Moving the text, retagging the device and moving the symbol all
        // redraw the tag text.
        let device = doc.project().symbol(k1).unwrap().device().unwrap();
        let (_, changes) = doc
            .transact("edit", |tx| {
                tx.move_text(note, at(35.0, 45.0))?;
                tx.set_tag(device, DeviceTag::parse("-K9")?)?;
                tx.move_symbol(k1, MPlacement::at(Point::new(120.0, 100.0)))
            })
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert_eq!(label(&ps, note).unwrap().pos, Vec2::new(35.0, 45.0));
        let tag_label = label(&ps, tag).unwrap();
        assert_eq!(tag_label.text, "-K9");
        assert_eq!(tag_label.pos, Vec2::new(112.0, 100.0));

        // Assigning and renaming the wire's potential relabels it.
        let (pot, changes) = doc
            .transact("potential", |tx| {
                let pot = tx.add_potential("L1", novo_core::PotentialKind::Line)?;
                tx.set_wire_potential(w, Some(pot))?;
                Ok(pot)
            })
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert_eq!(label(&ps, pot_text).unwrap().text, "L1");
        let (_, changes) = doc
            .transact("rename", |tx| {
                tx.update::<novo_core::Potential>(pot, |p| p.name = "L2".into())
            })
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert_eq!(label(&ps, pot_text).unwrap().text, "L2");
        let (_, changes) = doc
            .transact("clear", |tx| tx.update::<Wire>(w, |w| w.potential = None))
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert!(label(&ps, pot_text).is_none());

        // Removing the tag text brings the symbol's own tag back.
        let (_, changes) = doc.transact("untag", |tx| tx.remove(tag)).unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert!(!ps.scene().contains(item_id(tag)));
        assert_eq!(ps.scene().get(item_id(k1)).unwrap().labels[0].text, "-K9");

        // Adding one back hides it again; deleting the symbol deletes both.
        let (tag, changes) = doc
            .transact("retag", |tx| {
                tx.add_text(page, TextContent::DeviceTag, at(5.0, 0.0), Some(k1.into()))
            })
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert!(
            ps.scene()
                .get(item_id(k1))
                .unwrap()
                .labels
                .iter()
                .all(|l| l.text != "-K9")
        );
        let (_, changes) = doc.transact("delete", |tx| tx.remove(k1)).unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert!(!ps.scene().contains(item_id(tag)));
        assert!(!ps.scene().contains(item_id(k1)));
        assert!(!ps.scene().contains(item_id(pot_text)));
        assert!(ps.scene().contains(item_id(note)));
        assert!(ps.model_id(item_id(tag)).is_none());

        // Undo restores the texts with their hosts.
        let changes = doc.undo().unwrap().unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert_eq!(label(&ps, tag).unwrap().text, "-K9");
    }

    #[test]
    fn artwork_text_and_tag_anchor() {
        struct Motor;
        impl SymbolArt for Motor {
            fn artwork(&self, _: &SymbolInstance) -> Option<Vec<Primitive>> {
                Some(vec![Primitive::stroked(
                    Shape::circle(Vec2::ZERO, 5.0),
                    Stroke::default(),
                )])
            }
            fn labels(&self, _: &SymbolInstance) -> Vec<Label> {
                vec![Label::new(Vec2::new(0.0, 1.0), "M", 3.5).anchored(Anchor::Center)]
            }
            fn tag_anchor(&self, _: &SymbolInstance) -> Option<(Vec2, Anchor)> {
                Some((Vec2::new(-7.0, 0.0), Anchor::CenterRight))
            }
        }
        let (doc, page, k1, ..) = doc();
        let ps = PageScene::new(doc.project(), page, &Motor);
        let item = ps.scene().get(item_id(k1)).unwrap();
        // The tag first, at the art's anchor; then the artwork text, placed.
        assert_eq!(item.labels[0].text, "-K1");
        assert_eq!(item.labels[0].pos, Vec2::new(93.0, 100.0));
        assert_eq!(item.labels[0].anchor, Anchor::CenterRight);
        assert_eq!(item.labels[1].text, "M");
        assert_eq!(item.labels[1].pos, Vec2::new(100.0, 101.0));
        assert_eq!(item.labels[1].color, ps.style.symbol.color);
        // Text is pickable on its own.
        let hit = ps.scene().pick(Vec2::new(100.0, 101.0), 0.5, 2.0).unwrap();
        assert_eq!(hit.label, Some(1));
    }

    #[test]
    fn an_open_wire_end_shows_a_ring_and_a_flag() {
        let mut doc = Document::default();
        let ((page, closed, open), _) = doc
            .transact("wires", |tx| {
                let page = tx.add_page("1")?;
                let x1 = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
                let mut pins = Vec::new();
                for x in [100.0, 140.0] {
                    let t = tx.add_terminal(x1, "1")?;
                    pins.push(tx.place_symbol(
                        page,
                        "iec/terminal",
                        MPlacement::at(Point::new(x, 50.0)),
                        vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                        SymbolRole::Terminal { terminals: vec![t] },
                    )?);
                }
                let closed = tx.connect(Endpoint::pin(pins[0], 1), Endpoint::pin(pins[1], 0))?;
                let open = tx.connect(
                    Endpoint::pin(pins[0], 0),
                    Endpoint::Free {
                        at: Point::new(100.0, 20.0),
                    },
                )?;
                Ok((page, closed, open))
            })
            .unwrap();
        let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let count = |w| ps.scene().get(item_id(w)).unwrap().primitives.len();
        // The open wire has the closed wire's line and dot plus ring, pole
        // and pennant.
        assert_eq!(count(open), 1 + 1 + 3);
        assert_eq!(count(closed), 1 + 2);
        // The ring sits on the open end.
        let ring = &ps.scene().get(item_id(open)).unwrap().primitives[2];
        let b = ring.shape.bounds();
        assert!((b.center() - Vec2::new(100.0, 20.0)).length() < 1e-3);
    }

    #[test]
    fn double_click_tells_text_component_and_wire_apart() {
        let mut doc = Document::default();
        let ((page, sym, wire), _) = doc
            .transact("dc", |tx| {
                let page = tx.add_page("1")?;
                let x1 = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
                let t = tx.add_terminal(x1, "1")?;
                let sym = tx.place_symbol(
                    page,
                    "iec/terminal",
                    MPlacement::at(Point::new(100.0, 50.0)),
                    vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                    SymbolRole::Terminal { terminals: vec![t] },
                )?;
                let wire = tx.connect(
                    Endpoint::pin(sym, 0),
                    Endpoint::Free {
                        at: Point::new(100.0, 20.0),
                    },
                )?;
                Ok((page, sym, wire))
            })
            .unwrap();
        let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let hit = |id: ItemId, label| Hit {
            id,
            distance: 0.0,
            label,
        };
        let t = |h| ps.double_click_target(Some(&h));
        assert_eq!(
            t(hit(item_id(sym), None)),
            DoubleClickTarget::Component(sym)
        );
        assert_eq!(t(hit(item_id(wire), None)), DoubleClickTarget::Wire(wire));
        assert_eq!(
            t(hit(item_id(sym), Some(0))),
            DoubleClickTarget::Text(AnyId::Symbol(sym))
        );
        assert_eq!(ps.double_click_target(None), DoubleClickTarget::Empty);
    }

    #[test]
    fn label_hits_name_the_owner_and_the_kind_of_text() {
        let mut doc = Document::default();
        let ((page, sym, wire, free), _) = doc
            .transact("labels", |tx| {
                let page = tx.add_page("1")?;
                let x1 = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
                let t = tx.add_terminal(x1, "1")?;
                let sym = tx.place_symbol(
                    page,
                    "iec/terminal",
                    MPlacement::at(Point::new(100.0, 50.0)),
                    vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                    SymbolRole::Terminal { terminals: vec![t] },
                )?;
                let wire = tx.connect(
                    Endpoint::pin(sym, 0),
                    Endpoint::Free {
                        at: Point::new(100.0, 20.0),
                    },
                )?;
                let free = tx.add_text(
                    page,
                    TextContent::Literal {
                        text: "Note".into(),
                    },
                    novo_core::TextPlacement {
                        offset: Point::new(60.0, 60.0),
                        ..Default::default()
                    },
                    None,
                )?;
                Ok((page, sym, wire, free))
            })
            .unwrap();
        let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let label = |id: ItemId, label| {
            ps.label_hit(
                doc.project(),
                &Hit {
                    id,
                    distance: 0.0,
                    label,
                },
            )
        };
        // The terminal's own name is its tag, the first label.
        let tag = label(item_id(sym), Some(0)).unwrap();
        assert_eq!(
            (tag.owner, tag.kind),
            (AnyId::Symbol(sym), LabelKind::TerminalName)
        );
        // Any later label of a symbol is the canvas's own text.
        assert_eq!(label(item_id(sym), Some(1)).unwrap().kind, LabelKind::Fixed);
        assert_eq!(
            label(item_id(wire), Some(0)).unwrap().kind,
            LabelKind::WireNumber
        );
        let lit = label(item_id(free), Some(0)).unwrap();
        assert_eq!((lit.kind, lit.text), (LabelKind::Literal, Some(free)));
        // A hit on a line, not on text, is no label hit.
        assert_eq!(label(item_id(sym), None), None);
    }

    /// A wire from a terminal's pin up and over to an open end, with two
    /// corners at (100, 30) and (140, 30).
    fn bent_wire() -> (Document, PageId, WireId) {
        let mut doc = Document::default();
        let ((page, wire), _) = doc
            .transact("bent", |tx| {
                let page = tx.add_page("1")?;
                let x1 = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
                let t = tx.add_terminal(x1, "1")?;
                let sym = tx.place_symbol(
                    page,
                    "iec/terminal",
                    MPlacement::at(Point::new(100.0, 50.0)),
                    vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                    SymbolRole::Terminal { terminals: vec![t] },
                )?;
                let wire = tx.connect(
                    Endpoint::pin(sym, 0),
                    Endpoint::Free {
                        at: Point::new(140.0, 47.5),
                    },
                )?;
                tx.update::<novo_core::Wire>(wire, |w| {
                    w.route = vec![Point::new(100.0, 30.0), Point::new(140.0, 30.0)];
                })?;
                Ok((page, wire))
            })
            .unwrap();
        (doc, page, wire)
    }

    #[test]
    fn a_click_on_a_wire_never_picks_it_whole() {
        let (doc, page, wire) = bent_wire();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        // By default a click picks the wire whole (until the editor picks
        // its parts)...
        assert!(ps.scene().pick(Vec2::new(120.0, 30.0), 2.0, 4.0).is_some());
        let style = ModelStyle {
            wires_pick_whole: false,
            ..ps.style
        };
        ps.set_style(style, doc.project(), &PlaceholderArt);
        // On the straight run, and right on a corner: no pick by click.
        assert!(ps.scene().pick(Vec2::new(120.0, 30.0), 2.0, 4.0).is_none());
        assert!(ps.scene().pick(Vec2::new(100.0, 30.0), 2.0, 4.0).is_none());
        // A box still selects it.
        let rect = crate::Aabb::from_corners(Vec2::new(90.0, 20.0), Vec2::new(150.0, 55.0));
        let ids = ps
            .scene()
            .box_select(&rect, crate::scene::BoxMode::Crossing, 4.0);
        assert!(ids.contains(&item_id(wire)));
    }

    #[test]
    fn wire_parts_are_corners_and_segments() {
        let (doc, page, wire) = bent_wire();
        let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let part = |x, y| ps.wire_part_at(doc.project(), Vec2::new(x, y), 1.5);
        let kind = |x, y| part(x, y).map(|p| (p.wire, p.kind));
        // A corner wins where it is within reach, even on a run.
        assert_eq!(kind(100.0, 30.5), Some((wire, WirePartKind::Corner(0))));
        assert_eq!(kind(140.0, 29.6), Some((wire, WirePartKind::Corner(1))));
        // Mid run: the segment (0 up, 1 across, 2 down).
        assert_eq!(kind(100.0, 40.0), Some((wire, WirePartKind::Segment(0))));
        assert_eq!(kind(120.0, 30.2), Some((wire, WirePartKind::Segment(1))));
        assert_eq!(kind(140.0, 40.0), Some((wire, WirePartKind::Segment(2))));
        assert_eq!(kind(120.0, 20.0), None);
    }

    #[test]
    fn corners_are_drawn_and_a_focused_run_lights_its_corners() {
        let (doc, page, wire) = bent_wire();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let handles = |ps: &PageScene| ps.scene().get(item_id(wire)).unwrap().handles.clone();
        let prims = |ps: &PageScene| ps.scene().get(item_id(wire)).unwrap().primitives.len();
        // Corners are screen-size handles; the line, the pin dot and the
        // open end (ring, pole, pennant) stay primitives.
        let plain = prims(&ps);
        assert_eq!(plain, 1 + 1 + 3);
        let base = handles(&ps);
        assert_eq!(base.len(), 2);
        assert!(base.iter().all(|h| h.half == ps.style.corner_half));
        let focus = ps.style.focus_color;
        let lit = |ps: &PageScene| {
            let item = ps.scene().get(item_id(wire)).unwrap();
            item.handles.iter().filter(|h| h.color == focus).count()
                + item
                    .primitives
                    .iter()
                    .filter(|p| p.stroke.is_some_and(|s| s.color == focus))
                    .count()
        };
        assert_eq!(lit(&ps), 0);
        // The across run: the run itself and both of its corners.
        let seg = WirePart {
            wire,
            kind: WirePartKind::Segment(1),
        };
        ps.set_wire_focus(doc.project(), Some(seg));
        assert_eq!(lit(&ps), 3);
        // A corner lights just itself, drawn larger than the plain ones.
        let corner = WirePart {
            wire,
            kind: WirePartKind::Corner(0),
        };
        ps.set_wire_focus(doc.project(), Some(corner));
        assert_eq!(lit(&ps), 1);
        assert!(handles(&ps).iter().any(|h| h.half > ps.style.corner_half));
        ps.set_wire_focus(doc.project(), None);
        assert_eq!(prims(&ps), plain);
        assert_eq!(handles(&ps), base);
    }

    #[test]
    fn label_kinds_follow_the_symbol_role() {
        let mut doc = Document::default();
        let ((page, feed, int), _) = doc
            .transact("roles", |tx| {
                let page = tx.add_page("1")?;
                let l1 = tx.add_potential("L1", novo_core::PotentialKind::Line)?;
                let feed = tx.place_symbol(
                    page,
                    "iec/potential",
                    MPlacement::at(Point::new(50.0, 50.0)),
                    vec![Pin::new("1", 0.0, 2.5)],
                    SymbolRole::PotentialFeed { potential: l1 },
                )?;
                let int = tx.place_symbol(
                    page,
                    "iec/interruption",
                    MPlacement::at(Point::new(80.0, 50.0)),
                    vec![Pin::new("1", 0.0, 2.5)],
                    SymbolRole::Interruption { name: "A1".into() },
                )?;
                Ok((page, feed, int))
            })
            .unwrap();
        let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let kind = |s: SymbolId| {
            ps.label_hit(
                doc.project(),
                &Hit {
                    id: item_id(s),
                    distance: 0.0,
                    label: Some(0),
                },
            )
            .map(|l| l.kind)
        };
        assert_eq!(kind(feed), Some(LabelKind::Potential));
        assert_eq!(kind(int), Some(LabelKind::InterruptionName));
    }

    #[test]
    fn page_scene_hands_the_sheet_to_the_view() {
        let mut doc = Document::default();
        let (page, _) = doc.transact("page", |tx| tx.add_page("1")).unwrap();
        let style = ModelStyle {
            sheet_fill: Some(Color::rgb(19, 19, 21)),
            ..ModelStyle::default()
        };
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        ps.set_style(style, doc.project(), &PlaceholderArt);
        let sheet = ps.scene().sheet().expect("a page has a sheet");
        assert_eq!(sheet.fill, Some(Color::rgb(19, 19, 21)));
        assert!(!sheet.lines.is_empty(), "frame lines keep dots clear");
        // The frame item no longer fills: the view paints the sheet under
        // the grid dots.
        let frame = ps.scene().get(item_id(page)).unwrap();
        assert!(frame.primitives.iter().all(|p| p.fill.is_none()));
    }

    #[test]
    fn frame_has_columns_and_title_block() {
        let (mut doc, page, ..) = doc();
        doc.transact("info", |tx| {
            tx.set_info(|i| {
                i.name = "Pump station".into();
                i.properties
                    .insert("revision".into(), novo_core::Value::Text("B".into()));
                i.properties
                    .insert("drawn_by".into(), novo_core::Value::Text("TT".into()));
            })?;
            tx.update::<novo_core::Page>(page, |p| {
                p.title = "Power".into();
                p.properties.insert(
                    "structure".into(),
                    novo_core::Value::Text("=A1+CAB1".into()),
                );
            })
        })
        .unwrap();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let texts = |ps: &PageScene| -> Vec<String> {
            ps.scene()
                .get(item_id(page))
                .unwrap()
                .labels
                .iter()
                .map(|l| l.text.clone())
                .collect()
        };
        let t = texts(&ps);
        let columns = doc.project().page(page).unwrap().format.columns as usize;
        // Each column number appears top and bottom.
        assert_eq!(t.iter().filter(|s| *s == "1").count(), 2);
        assert!(t.contains(&columns.to_string()));
        for want in [
            "Pump station",
            "1  Power",
            "1 of 1",
            "=A1+CAB1",
            "B",
            "TT / ",
        ] {
            assert!(t.iter().any(|s| s == want), "{want} missing from {t:?}");
        }
        // Column numbers sit on PageFormat::column_at's grid.
        let format = doc.project().page(page).unwrap().format;
        let frame = ps.scene().get(item_id(page)).unwrap();
        for l in frame
            .labels
            .iter()
            .filter(|l| l.text.parse::<u16>().is_ok())
        {
            assert_eq!(format.column_at(f64::from(l.pos.x)).to_string(), l.text);
        }

        // Adding a page changes "n of m"; info changes redraw too.
        let (_, changes) = doc.transact("page", |tx| tx.add_page("2")).unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert!(texts(&ps).iter().any(|s| s == "1 of 2"));
        let (_, changes) = doc
            .transact("rename", |tx| tx.set_info(|i| i.name = "Mill".into()))
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        assert!(texts(&ps).iter().any(|s| s == "Mill"));
    }

    #[test]
    fn generated_sheets_draw_inside_the_frame() {
        let (mut doc, page, ..) = doc();
        doc.transact("structure", |tx| {
            tx.update::<novo_core::Page>(page, |p| {
                p.title = "Bill of materials".into();
                p.structure = DeviceTag::parse("=A1+CAB1").unwrap();
            })
        })
        .unwrap();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let frame = |ps: &PageScene| ps.scene().get(item_id(page)).unwrap().clone();
        let before = frame(&ps);
        let texts = |ps: &PageScene| -> Vec<String> {
            frame(ps).labels.iter().map(|l| l.text.clone()).collect()
        };
        assert!(texts(&ps).iter().any(|s| s == "=A1+CAB1"));

        let cell = |x: f32, text: &str, align| SheetText {
            min: Vec2::new(x, 20.0),
            size: Vec2::new(30.0, 5.0),
            text: text.into(),
            height: 2.5,
            align,
            // Header cells; the long one must still fit when bold.
            bold: x != 45.0,
        };
        let sheet = SheetContent {
            name: "20b".into(),
            number: Some((2, 3)),
            texts: vec![
                cell(15.0, "-K1", Anchor::CenterLeft),
                cell(45.0, "12 pcs", Anchor::CenterRight),
                cell(
                    75.0,
                    "A very long article description that cannot fit",
                    Anchor::CenterLeft,
                ),
            ],
            lines: vec![SheetLine {
                from: Vec2::new(15.0, 25.0),
                to: Vec2::new(105.0, 25.0),
                heavy: true,
            }],
        };
        assert!(ps.set_sheet(doc.project(), Some(sheet.clone())));
        assert!(!ps.set_sheet(doc.project(), Some(sheet.clone())));
        let f = frame(&ps);
        assert_eq!(f.primitives.len(), before.primitives.len() + 1);
        let label = |text: &str| f.labels.iter().find(|l| l.text == text).cloned();
        let k1 = label("-K1").expect("left cell");
        assert_eq!(
            (k1.pos, k1.anchor),
            (Vec2::new(15.0, 22.5), Anchor::CenterLeft)
        );
        assert!(k1.bold);
        let qty = label("12 pcs").expect("right cell");
        assert!(!qty.bold);
        assert_eq!(
            (qty.pos, qty.anchor),
            (Vec2::new(75.0, 22.5), Anchor::CenterRight)
        );
        // Too long for its box: cut with an ellipsis, inside the box.
        let long = f
            .labels
            .iter()
            .find(|l| l.text.starts_with("A very"))
            .unwrap();
        assert!(long.text.ends_with('…'));
        assert!(long.size().x <= 30.0 + 1e-3, "{}", long.size().x);
        // The title block names the sheet and counts sheets.
        let t = texts(&ps);
        for want in ["20b  Bill of materials", "2 of 3"] {
            assert!(t.iter().any(|s| s == want), "{want} missing from {t:?}");
        }
        // Kept across a rebuild; None goes back to the page alone.
        ps.rebuild(doc.project(), &PlaceholderArt);
        assert!(texts(&ps).iter().any(|s| s == "-K1"));
        assert!(ps.set_sheet(doc.project(), None));
        assert_eq!(frame(&ps).primitives.len(), before.primitives.len());
        assert!(texts(&ps).iter().any(|s| s == "1  Bill of materials"));

        // Generated pages never count among the schematic pages.
        let (_, changes) = doc
            .transact("report", |tx| {
                let p = tx.add_page("20")?;
                tx.update::<novo_core::Page>(p, |p| {
                    p.kind = PageKind::Generated {
                        generator: "bom".into(),
                    }
                })
            })
            .unwrap();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        assert!(texts(&ps).iter().any(|s| s == "1 of 1"), "{:?}", texts(&ps));
    }

    #[test]
    fn identical_symbols_share_one_drawing() {
        use std::cell::Cell;
        struct Counting(Cell<usize>);
        impl SymbolArt for Counting {
            fn artwork(&self, _: &SymbolInstance) -> Option<Vec<Primitive>> {
                self.0.set(self.0.get() + 1);
                None
            }
        }
        let (doc, page) = demo_document().unwrap();
        let art = Counting(Cell::new(0));
        let ps = PageScene::new(doc.project(), page, &art);
        let symbols = doc.project().page_contents(page).symbols.len();
        let kinds: std::collections::HashSet<ArtKey> = doc
            .project()
            .page_contents(page)
            .symbols
            .iter()
            .map(|&s| ArtKey::of(doc.project().symbol(s).unwrap()))
            .collect();
        assert!(kinds.len() < symbols, "the demo repeats a symbol");
        assert_eq!(art.0.get(), kinds.len());
        assert_eq!(ps.scene().len(), 1 + symbols + 2 + 12);
    }

    #[test]
    fn demo_document_draws() {
        let (doc, page) = demo_document().unwrap();
        let ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        // Frame, 7 symbols, 2 junctions, 6 + 6 wires.
        assert_eq!(ps.scene().len(), 1 + 7 + 2 + 12);
        let b = ps.scene().bounds();
        assert!(b.max.x <= 420.5 && b.max.y <= 297.5, "{b:?}");
    }

    #[test]
    fn undo_restores_wires() {
        let (mut doc, page, k1, _s1, w) = doc();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let before = ps.scene().get(item_id(w)).cloned();
        let (_, changes) = doc
            .transact("move", |tx| {
                tx.move_symbol(k1, MPlacement::at(Point::new(150.0, 150.0)))
            })
            .unwrap();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        assert_ne!(ps.scene().get(item_id(w)).cloned(), before);
        let changes = doc.undo().unwrap().unwrap();
        ps.apply(doc.project(), &changes, &PlaceholderArt);
        assert_eq!(ps.scene().get(item_id(w)).cloned(), before);
    }

    #[test]
    fn mirrored_placement_matches_model() {
        let p = MPlacement {
            origin: Point::new(10.0, 20.0),
            rotation: Rotation::R270,
            mirrored: true,
        };
        let ours = placement(&p).affine();
        for local in [Point::new(2.0, 0.0), Point::new(-1.0, 3.0)] {
            let a = vec(p.apply(local));
            let b = ours.transform_point2(vec(local));
            assert!(a.distance(b) < 1e-4, "{a} vs {b}");
        }
    }

    #[test]
    fn custom_art_is_used() {
        struct Art;
        impl SymbolArt for Art {
            fn artwork(&self, s: &SymbolInstance) -> Option<Vec<Primitive>> {
                (s.symbol == key("iec/coil")).then(|| {
                    vec![Primitive::stroked(
                        Shape::circle(Vec2::ZERO, 3.0),
                        Stroke::default(),
                    )]
                })
            }
        }
        let (doc, page, k1, s1, _) = doc();
        let ps = PageScene::new(doc.project(), page, &Art);
        let coil = ps.scene().get(item_id(k1)).unwrap();
        assert!(
            matches!(coil.primitives[0].shape, Shape::Circle { center, .. } if center == Vec2::new(100.0, 100.0))
        );
        assert!(is_placeholder(&Art, doc.project().symbol(s1).unwrap()));
    }
}
