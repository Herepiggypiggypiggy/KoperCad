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

use crate::scene::{Item, ItemId, Layer, LayerId, Scene, SnapPoint};
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
}

impl JunctionGlyph {
    /// The glyph for wires leaving a point in `dirs`, one entry per wire
    /// (any length; they are snapped to the nearest of the four axes).
    /// Wires are counted, not sides: three or more wires are an electrical
    /// node and always get a dot (a T when they use exactly three sides).
    /// Exactly two wires at a right angle make a corner; anything else (a
    /// straight run, a loose end) is a dot.
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
                .map_or(JunctionGlyph::Dot, |q| JunctionGlyph::Corner {
                    quarter_turns: q,
                }),
            _ => JunctionGlyph::Dot,
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
    let dot = Primitive::filled(Shape::circle(pos, style.junction_radius), style.wire.color);
    let stubs = |dirs: &[Vec2]| {
        dirs.iter()
            .map(|d| Primitive::stroked(Shape::line(pos, pos + *d * JUNCTION_STUB), style.wire))
            .collect::<Vec<_>>()
    };
    match glyph {
        JunctionGlyph::Dot => vec![dot],
        JunctionGlyph::Tee { stem } => {
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
            stubs(&[quarter_dir(quarter_turns + 3), quarter_dir(quarter_turns)])
        }
    }
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
}

/// One printed sheet of a generated page (a report): text boxes and rules
/// laid out by its generator (novo-reports' `layout_sheet`), in page
/// millimetres with the origin at the sheet's top left. The canvas stays
/// independent of the report crate; callers convert.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SheetContent {
    /// Printed where the page name goes: `20`, or `20.2` for the second
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
fn fit_text(text: &str, width: f32, height: f32) -> String {
    let advance = crate::text::ADVANCE * height;
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
    pub style: ModelStyle,
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
        let contents = project.page_contents(self.page);
        for id in contents.symbols {
            self.sync_symbol(project, id, art);
        }
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
        let mut sheet = Primitive::stroked(Shape::rect(Vec2::ZERO, size), thin);
        sheet.fill = self.style.sheet_fill;
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
                let text = fit_text(&t.text, t.size.x, t.height);
                if !text.is_empty() {
                    item.labels.push(
                        Label::new(pos, text, t.height)
                            .anchored(anchor)
                            .colored(self.style.tag_color),
                    );
                }
            }
        }
        self.title_block(project, page, inner_max, &mut item);
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
        if let Some(text) = symbol_text(project, sym).filter(|_| !tag_text) {
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
            item.labels.insert(
                0,
                Label::new(pos, text, self.style.tag_height)
                    .anchored(anchor)
                    .colored(self.style.tag_color),
            );
        }
        if let Some(note) = self.notes.get(&id) {
            item.labels.push(self.note_label(&item, note));
        }
        self.put(id.into(), item);
    }

    /// A note above the top right corner of the symbol's ink, clear of its
    /// tag, in the tag's size so both read alike.
    fn note_label(&self, item: &Item, note: &SymbolNote) -> Label {
        let ink = item
            .primitives
            .iter()
            .fold(crate::Aabb::EMPTY, |b, p| b.union(&p.shape.bounds()));
        let pos = if ink.is_empty() {
            item.snap_points
                .first()
                .map_or(Vec2::ZERO, |s| s.pos + Vec2::new(1.0, -1.0))
        } else {
            Vec2::new(ink.max.x + 1.0, ink.min.y - 0.5)
        };
        let color = if note.warning {
            self.style.note_warning_color
        } else {
            self.style.note_color
        };
        Label::new(pos, note.lines.join("\n"), self.style.tag_height)
            .anchored(Anchor::BottomLeft)
            .colored(color)
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
        Endpoint::Pin { .. } => None,
    })
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
        assert_eq!(JunctionGlyph::from_directions([x, -x]), Dot);
        assert_eq!(JunctionGlyph::from_directions([x, -x, y, -y]), Dot);

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
        // the branch wire leaves a straight run, drawn as a plain dot.
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
        assert_eq!(lines(&ps, j), 0, "a straight run is a dot");
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
        };
        let sheet = SheetContent {
            name: "20.2".into(),
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
        let qty = label("12 pcs").expect("right cell");
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
        for want in ["20.2  Bill of materials", "2 of 3"] {
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
