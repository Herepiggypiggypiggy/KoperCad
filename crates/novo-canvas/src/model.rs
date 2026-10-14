//! Adapter from the `novo-core` data model to a canvas [`Scene`] (feature
//! `model`). This is the only part of the crate that knows the model; the
//! rest of the canvas stays model-agnostic.
//!
//! A [`PageScene`] mirrors one page: every symbol, wire and junction becomes
//! an item whose [`ItemId`] is the model's raw id (ids are unique across the
//! project), so canvas picks and selections map straight back to model ids.
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
    AnyId, ChangeSet, Endpoint, JunctionId, PageId, Point, Project, Rotation, SymbolId,
    SymbolInstance, SymbolKey, SymbolRole, WireId,
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
    let rotation = match p.rotation {
        Rotation::R0 => 0.0,
        Rotation::R90 => -FRAC_PI_2,
        Rotation::R180 => -2.0 * FRAC_PI_2,
        Rotation::R270 => -3.0 * FRAC_PI_2,
    };
    Placement {
        offset: vec(p.origin),
        rotation,
        mirror: p.mirrored,
        scale: 1.0,
    }
}

/// Artwork for a symbol in symbol coordinates (mm, y down, unplaced).
pub trait SymbolArt {
    /// Primitives for the symbol, or `None` to fall back to the placeholder.
    fn artwork(&self, symbol: &SymbolInstance) -> Option<Vec<Primitive>>;
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
        }
    }
}

/// Layers a page scene uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelLayers {
    pub frame: LayerId,
    pub wires: LayerId,
    pub symbols: LayerId,
}

/// The canvas scene of one model page, kept in sync with the project.
pub struct PageScene {
    page: PageId,
    scene: Scene,
    layers: ModelLayers,
    ids: HashMap<ItemId, AnyId>,
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
        };
        let mut s = Self {
            page,
            scene,
            layers,
            ids: HashMap::new(),
            style: ModelStyle::default(),
        };
        s.rebuild(project, art);
        s
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

    /// Rebuilds every item, for example after undo or loading a file.
    pub fn rebuild(&mut self, project: &Project, art: &dyn SymbolArt) {
        self.scene.clear();
        self.ids.clear();
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
    }

    /// Updates the scene after an edit. Returns whether anything changed.
    pub fn apply(&mut self, project: &Project, changes: &ChangeSet, art: &dyn SymbolArt) -> bool {
        if project.page(self.page).is_none() {
            let changed = !self.scene.is_empty();
            self.scene.clear();
            self.ids.clear();
            return changed;
        }
        let mut dirty: BTreeSet<AnyId> = BTreeSet::new();
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
                // Feed symbols show the potential's name.
                AnyId::Potential(_) => dirty.extend(
                    project
                        .referrers(id)
                        .filter(|r| matches!(r, AnyId::Symbol(_))),
                ),
                AnyId::Terminal(t) => dirty.extend(
                    project
                        .symbols_of_terminal(t)
                        .into_iter()
                        .map(|s| AnyId::Symbol(s.id)),
                ),
                AnyId::Page(p) if p == self.page => {
                    self.frame(project);
                }
                _ => {}
            }
        }
        let rev = self.scene.revision();
        for id in dirty {
            match id {
                AnyId::Symbol(s) => self.sync_symbol(project, s, art),
                AnyId::Junction(j) => self.sync_junction(project, j),
                AnyId::Wire(w) => self.sync_wire(project, w),
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
        let ink = Stroke::world(0.5, Color::rgb(90, 90, 90));
        let thin = Stroke::world(0.25, Color::rgb(90, 90, 90));
        let mut item = Item::new(self.layers.frame)
            .with(Primitive::stroked(Shape::rect(Vec2::ZERO, size), thin))
            .with(Primitive::stroked(
                Shape::rect(Vec2::splat(10.0), size - Vec2::splat(10.0)),
                ink,
            ))
            .unselectable();
        let title = if page.title.is_empty() {
            page.name.clone()
        } else {
            format!("{}  {}", page.name, page.title)
        };
        item.labels.push(
            Label::new(size - Vec2::new(14.0, 14.0), title, 3.5)
                .anchored(Anchor::BottomRight)
                .colored(Color::rgb(90, 90, 90)),
        );
        self.scene.insert(item_id(self.page), item);
    }

    fn sync_symbol(&mut self, project: &Project, id: SymbolId, art: &dyn SymbolArt) {
        let Some(sym) = project.symbol(id).filter(|s| s.page == self.page) else {
            self.remove(id.into());
            return;
        };
        let place = placement(&sym.placement);
        let local = art
            .artwork(sym)
            .unwrap_or_else(|| PlaceholderArt::draw(sym, self.style.symbol));
        let mut item = Item::new(self.layers.symbols);
        item.primitives = local.iter().map(|p| p.transformed(&place)).collect();
        item.snap_points = (0..sym.pins.len() as u16)
            .filter_map(|i| sym.pin_position(i))
            .map(|p| SnapPoint::connection(vec(p)))
            .collect();
        if let Some(text) = symbol_text(project, sym) {
            // To the right of the placed symbol, vertically centred on its
            // pins, whatever its rotation or mirroring.
            let origin = vec(sym.placement.origin);
            let b = crate::Aabb::from_points(
                (0..sym.pins.len() as u16)
                    .filter_map(|i| sym.pin_position(i))
                    .map(vec),
            );
            let anchor = if b.is_empty() {
                origin + Vec2::new(5.0, 0.0)
            } else {
                Vec2::new(b.max.x.max(origin.x + 2.5) + 5.0, b.center().y)
            };
            item.labels.push(
                Label::new(anchor, text, self.style.tag_height)
                    .anchored(Anchor::CenterLeft)
                    .colored(self.style.tag_color),
            );
        }
        self.put(id.into(), item);
    }

    fn sync_junction(&mut self, project: &Project, id: JunctionId) {
        let Some(j) = project.junction(id).filter(|j| j.page == self.page) else {
            self.remove(id.into());
            return;
        };
        let p = vec(j.position);
        let item = Item::new(self.layers.wires)
            .with(Primitive::filled(
                Shape::circle(p, self.style.junction_radius),
                self.style.wire.color,
            ))
            .with_snap(SnapPoint::connection(p));
        self.put(id.into(), item);
    }

    fn sync_wire(&mut self, project: &Project, id: WireId) {
        let Some(w) = project.wire(id).filter(|w| w.page == self.page) else {
            self.remove(id.into());
            return;
        };
        let (Some(a), Some(b)) = (
            project.endpoint_position(w.from),
            project.endpoint_position(w.to),
        ) else {
            self.remove(id.into());
            return;
        };
        let mut pts = vec![vec(a)];
        pts.extend(w.route.iter().map(|p| vec(*p)));
        pts.push(vec(b));
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

/// The text shown next to a symbol: its device tag, terminal designation,
/// interruption point name or potential name.
pub fn symbol_text(project: &Project, sym: &SymbolInstance) -> Option<String> {
    match &sym.role {
        SymbolRole::Device { device, .. } => project.device(*device).map(|d| d.tag.to_string()),
        SymbolRole::Terminal { terminal } => project.terminal_designation(*terminal),
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
    fn retagging_a_strip_relabels_its_terminals() {
        let (mut doc, page, ..) = doc();
        let ((strip, sym), _) = doc
            .transact("strip", |tx| {
                let strip = tx.add_terminal_strip(DeviceTag::parse("-X1")?)?;
                let t = tx.add_terminal(strip, "3")?;
                let sym = tx.place_symbol(
                    page,
                    "iec/terminal",
                    MPlacement::at(Point::new(60.0, 40.0)),
                    vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                    SymbolRole::Terminal { terminal: t },
                )?;
                Ok((strip, sym))
            })
            .unwrap();
        let mut ps = PageScene::new(doc.project(), page, &PlaceholderArt);
        let before = ps.scene().get(item_id(sym)).unwrap().labels[0].text.clone();
        assert!(before.contains("X1"), "{before}");
        let (_, changes) = doc
            .transact("retag", |tx| tx.set_tag(strip, DeviceTag::parse("-X9")?))
            .unwrap();
        assert!(ps.apply(doc.project(), &changes, &PlaceholderArt));
        let after = &ps.scene().get(item_id(sym)).unwrap().labels[0].text;
        assert!(after.contains("X9") && !after.contains("X1"), "{after}");
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
