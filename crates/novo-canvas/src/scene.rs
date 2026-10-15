//! The retained scene: items made of primitives, grouped into layers, with a
//! spatial index for fast picking, box selection and snapping.
//!
//! The canvas knows nothing about the electrical model. A model adapter turns
//! model objects (symbols, wires, frames) into [`Item`]s keyed by an
//! [`ItemId`] it chooses, typically the model's own entity id, and keeps the
//! scene in sync with `insert` and `remove`.

use crate::geom::Aabb;
use crate::shape::Primitive;
use crate::snap::SnapKind;
use crate::style::Color;
use crate::text::Label;
use glam::Vec2;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Caller-chosen identity of an item, stable across edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub u64);

/// Index of a layer in [`Scene::layers`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LayerId(pub u16);

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    /// Hidden layers are neither drawn nor picked.
    pub visible: bool,
    /// Locked layers are drawn but cannot be picked, selected or snapped to.
    pub locked: bool,
    /// Higher layers draw on top of lower ones.
    pub order: i32,
}

impl Layer {
    pub fn new(name: impl Into<String>, order: i32) -> Self {
        Self {
            name: name.into(),
            visible: true,
            locked: false,
            order,
        }
    }

    pub fn is_pickable(&self) -> bool {
        self.visible && !self.locked
    }
}

/// An explicit snap target on an item, such as a symbol's connection point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapPoint {
    pub pos: Vec2,
    pub kind: SnapKind,
}

impl SnapPoint {
    pub fn connection(pos: impl Into<Vec2>) -> Self {
        Self {
            pos: pos.into(),
            kind: SnapKind::Connection,
        }
    }
}

/// A segment the cursor snaps onto anywhere along its length, such as a
/// mounting rail or a cable duct in a layout drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapLine {
    pub a: Vec2,
    pub b: Vec2,
    /// When set, positions along the line are multiples of this many mm
    /// from `a` (a rail's pitch).
    pub step: Option<f32>,
}

impl SnapLine {
    pub fn new(a: impl Into<Vec2>, b: impl Into<Vec2>) -> Self {
        Self {
            a: a.into(),
            b: b.into(),
            step: None,
        }
    }

    pub fn stepped(mut self, step: f32) -> Self {
        self.step = (step > 0.0 && step.is_finite()).then_some(step);
        self
    }

    /// The point of the line nearest `p`, on its step when it has one
    /// (never past either end).
    pub fn nearest(&self, p: Vec2) -> Vec2 {
        let d = self.b - self.a;
        let len = d.length();
        if len < 1e-6 {
            return self.a;
        }
        let dir = d / len;
        let mut t = (p - self.a).dot(dir).clamp(0.0, len);
        if let Some(step) = self.step {
            t = ((t / step).round() * step).min((len / step).floor() * step);
        }
        self.a + dir * t
    }
}

/// One selectable thing on the canvas: a symbol, a wire, a frame.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Item {
    pub layer: LayerId,
    pub primitives: Vec<Primitive>,
    /// Extra snap targets beyond the geometric ones (connection points).
    pub snap_points: Vec<SnapPoint>,
    /// Segments to snap onto anywhere along their length.
    pub snap_lines: Vec<SnapLine>,
    /// Text drawn with the item (tags, pin numbers).
    pub labels: Vec<Label>,
    /// Non-selectable items (page frames, title blocks) are drawn and can be
    /// snapped to, but clicks go through them.
    pub selectable: bool,
    /// Whether a click on the item's lines picks it. `false` leaves only a
    /// box to select it (and a click on its text): a wire is picked by its
    /// corners and segments instead, never as a whole by a single click.
    pub click_select: bool,
    /// Small markers that keep their size on screen at every zoom (wire
    /// corners, pin dots), painted by the view over the drawing.
    pub handles: Vec<Handle>,
}

/// A marker with a fixed size in screen points, centred on a world point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Handle {
    pub at: Vec2,
    /// Half the side of the square (or the radius of the dot), in points.
    pub half: f32,
    pub color: Color,
    pub round: bool,
}

impl Handle {
    pub fn square(at: Vec2, half: f32, color: Color) -> Self {
        Self {
            at,
            half,
            color,
            round: false,
        }
    }

    pub fn dot(at: Vec2, radius: f32, color: Color) -> Self {
        Self {
            at,
            half: radius,
            color,
            round: true,
        }
    }
}

impl Item {
    pub fn new(layer: LayerId) -> Self {
        Self {
            layer,
            primitives: Vec::new(),
            snap_points: Vec::new(),
            snap_lines: Vec::new(),
            labels: Vec::new(),
            selectable: true,
            click_select: true,
            handles: Vec::new(),
        }
    }

    pub fn with(mut self, p: Primitive) -> Self {
        self.primitives.push(p);
        self
    }

    pub fn with_snap(mut self, s: SnapPoint) -> Self {
        self.snap_points.push(s);
        self
    }

    pub fn with_snap_line(mut self, l: SnapLine) -> Self {
        self.snap_lines.push(l);
        self
    }

    pub fn with_label(mut self, l: Label) -> Self {
        self.labels.push(l);
        self
    }

    /// Only a box (or a click on its text) selects the item, not a click on
    /// its lines.
    pub fn box_only(mut self) -> Self {
        self.click_select = false;
        self
    }

    pub fn unselectable(mut self) -> Self {
        self.selectable = false;
        self
    }

    /// Bounds ignoring screen-width strokes (those depend on zoom).
    fn index_bounds(&self) -> Aabb {
        let mut b = Aabb::EMPTY;
        for p in &self.primitives {
            b = b.union(&p.bounds(f32::INFINITY));
        }
        for s in &self.snap_points {
            b.include_point(s.pos);
        }
        for l in &self.snap_lines {
            b.include_point(l.a);
            b.include_point(l.b);
        }
        for l in &self.labels {
            b = b.union(&l.bounds());
        }
        b
    }

    /// Largest half width of a screen-width stroke, in screen points.
    fn screen_half_width(&self) -> f32 {
        self.primitives
            .iter()
            .filter_map(|p| p.stroke)
            .filter(|s| s.unit == crate::style::WidthUnit::Screen)
            .map(|s| s.width * 0.5)
            .fold(0.0, f32::max)
    }

    /// Distance from `p` to the item's ink.
    pub fn distance(&self, p: Vec2, zoom: f32) -> f32 {
        self.nearest_part(p, zoom).0
    }

    /// Distance from `p` to the item's ink or text, and the index of the
    /// label when text is nearest.
    pub fn nearest_part(&self, p: Vec2, zoom: f32) -> (f32, Option<usize>) {
        let (ink, text) = self.parts_distance(p, zoom);
        match text {
            Some((d, i)) if d < ink => (d, Some(i)),
            _ => (ink, None),
        }
    }

    /// Distance to the ink, and to the nearest label with its index.
    /// An item with no ink and no text (a junction drawn plain) is picked
    /// at its snap points instead, so it can still be selected and deleted.
    fn parts_distance(&self, p: Vec2, zoom: f32) -> (f32, Option<(f32, usize)>) {
        let ink = if self.is_invisible() {
            self.snap_points
                .iter()
                .map(|s| s.pos.distance(p))
                .fold(f32::INFINITY, f32::min)
        } else {
            self.primitives
                .iter()
                .map(|prim| prim.distance(p, zoom))
                .fold(f32::INFINITY, f32::min)
        };
        let text = self
            .labels
            .iter()
            .enumerate()
            .map(|(i, l)| (l.distance(p), i))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        (ink, text)
    }

    /// True when any ink or text touches `rect`.
    /// An item with neither (see [`Self::distance`]) touches `rect` when a
    /// snap point is inside it.
    pub fn intersects(&self, rect: &Aabb) -> bool {
        if self.is_invisible() {
            return self.snap_points.iter().any(|s| rect.contains_point(s.pos));
        }
        self.primitives.iter().any(|p| p.intersects(rect))
            || self.labels.iter().any(|l| l.intersects(rect))
    }

    fn is_invisible(&self) -> bool {
        self.primitives.is_empty() && self.labels.is_empty()
    }
}

static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);

fn next_revision() -> u64 {
    NEXT_REVISION.fetch_add(1, Ordering::Relaxed)
}

struct Slot {
    id: ItemId,
    item: Item,
    bounds: Aabb,
    revision: u64,
}

/// How box selection decides membership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxMode {
    /// Only items completely inside the box.
    Window,
    /// Every item the box touches.
    Crossing,
}

impl BoxMode {
    /// CAD convention: dragging left to right selects by window, right to
    /// left by crossing.
    pub fn from_drag(start: Vec2, end: Vec2) -> Self {
        if end.x >= start.x {
            BoxMode::Window
        } else {
            BoxMode::Crossing
        }
    }
}

/// Result of a pick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub id: ItemId,
    /// Distance from the pick point to the nearest of the item's lines
    /// or label boxes, in world units.
    pub distance: f32,
    /// The label that was hit, when text was nearer than any line.
    pub label: Option<usize>,
}

pub struct Scene {
    layers: Vec<Layer>,
    slots: Vec<Option<Slot>>,
    free: Vec<u32>,
    by_id: HashMap<ItemId, u32>,
    index: GridIndex,
    max_screen_half_width: f32,
    /// Tallest label ever inserted (not lowered on removal), so a view can
    /// skip text entirely when even that would be too small to read.
    max_label_height: f32,
    revision: u64,
    sheet: Option<Sheet>,
}

/// The sheet of paper a page scene is drawn on. The view paints the fill
/// first, then the grid dots inside the sheet, then the scene on top, so a
/// stroke or text always covers a dot.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub rect: Aabb,
    pub fill: Option<Color>,
    /// The frame and title-block lines, which keep clear of grid dots.
    pub lines: Vec<(Vec2, Vec2)>,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    /// Default spatial index cell, in world units. 20 mm suits schematic
    /// pages where symbols are roughly 10 to 30 mm.
    pub const DEFAULT_CELL: f32 = 20.0;

    pub fn new() -> Self {
        Self::with_cell_size(Self::DEFAULT_CELL)
    }

    pub fn with_cell_size(cell: f32) -> Self {
        Self {
            layers: vec![Layer::new("Default", 0)],
            slots: Vec::new(),
            free: Vec::new(),
            by_id: HashMap::new(),
            index: GridIndex::new(cell),
            max_screen_half_width: 0.0,
            max_label_height: 0.0,
            revision: next_revision(),
            sheet: None,
        }
    }

    /// The sheet the scene is drawn on, when it is a page.
    pub fn sheet(&self) -> Option<&Sheet> {
        self.sheet.as_ref()
    }

    pub fn set_sheet(&mut self, sheet: Option<Sheet>) {
        self.sheet = sheet;
    }

    /// Changes whenever the geometry changes. Revisions are unique across all
    /// scenes in the process, so a renderer cache keyed by revision never
    /// confuses two scenes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(id.0 as usize)
    }

    /// Mutable access for toggling visibility and locks. Changing `order`
    /// changes draw order, so it bumps the revision.
    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.revision = next_revision();
        self.layers.get_mut(id.0 as usize)
    }

    pub fn add_layer(&mut self, layer: Layer) -> LayerId {
        self.layers.push(layer);
        self.revision = next_revision();
        LayerId((self.layers.len() - 1) as u16)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn contains(&self, id: ItemId) -> bool {
        self.by_id.contains_key(&id)
    }

    pub fn get(&self, id: ItemId) -> Option<&Item> {
        let slot = *self.by_id.get(&id)?;
        self.slots[slot as usize].as_ref().map(|s| &s.item)
    }

    /// Inserts or replaces the item with this id.
    pub fn insert(&mut self, id: ItemId, item: Item) {
        assert!(
            (item.layer.0 as usize) < self.layers.len(),
            "item {id:?} uses unknown layer {:?}",
            item.layer
        );
        self.remove(id);
        let bounds = item.index_bounds();
        self.max_screen_half_width = self.max_screen_half_width.max(item.screen_half_width());
        for l in &item.labels {
            self.max_label_height = self.max_label_height.max(l.height);
        }
        let slot = match self.free.pop() {
            Some(s) => s,
            None => {
                self.slots.push(None);
                (self.slots.len() - 1) as u32
            }
        };
        self.index.insert(slot, &bounds);
        let revision = next_revision();
        self.slots[slot as usize] = Some(Slot {
            id,
            item,
            bounds,
            revision,
        });
        self.by_id.insert(id, slot);
        self.revision = revision;
    }

    pub fn remove(&mut self, id: ItemId) -> Option<Item> {
        let slot = self.by_id.remove(&id)?;
        let s = self.slots[slot as usize].take()?;
        self.index.remove(slot, &s.bounds);
        self.free.push(slot);
        self.revision = next_revision();
        Some(s.item)
    }

    pub fn clear(&mut self) {
        self.slots.clear();
        self.free.clear();
        self.by_id.clear();
        self.index = GridIndex::new(self.index.cell);
        self.max_screen_half_width = 0.0;
        self.max_label_height = 0.0;
        self.revision = next_revision();
    }

    /// At least the height of the tallest label in the scene, in world
    /// units (it may overestimate after removals, never underestimate).
    pub fn max_label_height(&self) -> f32 {
        self.max_label_height
    }

    /// All items in storage order.
    pub fn iter(&self) -> impl Iterator<Item = (ItemId, &Item)> {
        self.slots.iter().flatten().map(|s| (s.id, &s.item))
    }

    /// Items in draw order: by layer order, then storage order.
    pub fn draw_order(&self) -> Vec<(ItemId, &Item)> {
        let mut v: Vec<_> = self.iter().collect();
        v.sort_by_key(|(_, it)| (self.layer_order(it.layer), it.layer));
        v
    }

    /// Changes whenever this item is inserted or replaced.
    pub fn item_revision(&self, id: ItemId) -> Option<u64> {
        let slot = *self.by_id.get(&id)?;
        self.slots[slot as usize].as_ref().map(|s| s.revision)
    }

    fn layer_order(&self, l: LayerId) -> i32 {
        self.layers.get(l.0 as usize).map_or(0, |l| l.order)
    }

    /// Bounds of everything in the scene.
    pub fn bounds(&self) -> Aabb {
        self.slots
            .iter()
            .flatten()
            .fold(Aabb::EMPTY, |b, s| b.union(&s.bounds))
    }

    /// Bounds of one item.
    pub fn item_bounds(&self, id: ItemId) -> Option<Aabb> {
        let slot = *self.by_id.get(&id)?;
        self.slots[slot as usize].as_ref().map(|s| s.bounds)
    }

    /// Items whose bounds touch `rect` (a broad phase, not exact).
    pub fn query(&self, rect: &Aabb) -> Vec<ItemId> {
        self.query_slots(rect)
            .into_iter()
            .filter_map(|s| self.slots[s as usize].as_ref().map(|s| s.id))
            .collect()
    }

    fn query_slots(&self, rect: &Aabb) -> Vec<u32> {
        let mut out = Vec::new();
        self.index.query(rect, &mut out);
        out.retain(|&s| {
            self.slots[s as usize]
                .as_ref()
                .is_some_and(|slot| slot.bounds.intersects(rect))
        });
        out
    }

    fn pickable(&self, slot: &Slot) -> bool {
        slot.item.selectable && self.layer(slot.item.layer).is_some_and(Layer::is_pickable)
    }

    /// The topmost selectable item within `tolerance` world units of `p`.
    /// Among items within tolerance the closest wins; ties go to the item
    /// drawn on top.
    pub fn pick(&self, p: Vec2, tolerance: f32, zoom: f32) -> Option<Hit> {
        self.pick_all(p, tolerance, zoom).into_iter().next()
    }

    /// Every selectable item within tolerance, closest (then topmost) first.
    pub fn pick_all(&self, p: Vec2, tolerance: f32, zoom: f32) -> Vec<Hit> {
        let reach = tolerance + self.max_screen_half_width / zoom.max(f32::EPSILON);
        let rect = Aabb::from_center(p, Vec2::splat(reach));
        let mut hits: Vec<(Hit, f32, i32, u32)> = self
            .query_slots(&rect)
            .into_iter()
            .filter_map(|s| {
                let slot = self.slots[s as usize].as_ref()?;
                if !self.pickable(slot) {
                    return None;
                }
                // Ink beats text: a label counts as `tolerance` further away
                // than it is, so a tag running over a neighbour's body never
                // steals the click from that body. Text still wins where no
                // line is near, and a pointer inside the label box always
                // takes the label, even where the symbol's own ink runs
                // under it.
                let (mut ink, text) = slot.item.parts_distance(p, zoom);
                if !slot.item.click_select {
                    ink = f32::INFINITY;
                }
                let (score, d, label) = match text {
                    Some((t, i))
                        if t <= tolerance
                            && (ink > tolerance || t + tolerance < ink || t <= 1e-4) =>
                    {
                        (t + tolerance, t, Some(i))
                    }
                    _ => (ink, ink, None),
                };
                (d <= tolerance).then(|| {
                    (
                        Hit {
                            id: slot.id,
                            distance: d,
                            label,
                        },
                        score,
                        self.layer_order(slot.item.layer),
                        s,
                    )
                })
            })
            .collect();
        hits.sort_by(|a, b| a.1.total_cmp(&b.1).then(b.2.cmp(&a.2)).then(b.3.cmp(&a.3)));
        hits.into_iter().map(|h| h.0).collect()
    }

    /// Selectable items selected by a box.
    pub fn box_select(&self, rect: &Aabb, mode: BoxMode, zoom: f32) -> Vec<ItemId> {
        let reach = self.max_screen_half_width / zoom.max(f32::EPSILON);
        self.query_slots(&rect.expand(reach))
            .into_iter()
            .filter_map(|s| {
                let slot = self.slots[s as usize].as_ref()?;
                if !self.pickable(slot) {
                    return None;
                }
                let hit = match mode {
                    BoxMode::Window => rect.contains(&slot.bounds),
                    BoxMode::Crossing => slot.item.intersects(rect),
                };
                hit.then_some(slot.id)
            })
            .collect()
    }

    /// Visits every snap candidate within `rect` on pickable layers,
    /// including unselectable items such as frames.
    pub(crate) fn for_each_snap_candidate(&self, rect: &Aabb, mut f: impl FnMut(ItemId, &Item)) {
        for s in self.query_slots(rect) {
            if let Some(slot) = self.slots[s as usize].as_ref()
                && self.layer(slot.item.layer).is_some_and(Layer::is_pickable)
            {
                f(slot.id, &slot.item);
            }
        }
    }
}

/// Uniform grid spatial hash. Items covering many cells go to a separate list
/// that every query scans, so huge items (page frames) do not flood the grid.
struct GridIndex {
    cell: f32,
    cells: HashMap<(i32, i32), Vec<u32>>,
    large: Vec<u32>,
}

impl GridIndex {
    const MAX_CELLS_PER_ITEM: i64 = 64;

    fn new(cell: f32) -> Self {
        assert!(cell > 0.0, "cell size must be positive");
        Self {
            cell,
            cells: HashMap::new(),
            large: Vec::new(),
        }
    }

    fn range(&self, b: &Aabb) -> Option<(i32, i32, i32, i32)> {
        if b.is_empty() || !b.min.is_finite() || !b.max.is_finite() {
            return None;
        }
        let f = |v: f32| {
            (v / self.cell)
                .floor()
                .clamp(i32::MIN as f32, i32::MAX as f32) as i32
        };
        Some((f(b.min.x), f(b.min.y), f(b.max.x), f(b.max.y)))
    }

    fn is_large(x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        (x1 as i64 - x0 as i64 + 1) * (y1 as i64 - y0 as i64 + 1) > Self::MAX_CELLS_PER_ITEM
    }

    fn insert(&mut self, slot: u32, b: &Aabb) {
        let Some((x0, y0, x1, y1)) = self.range(b) else {
            self.large.push(slot);
            return;
        };
        if Self::is_large(x0, y0, x1, y1) {
            self.large.push(slot);
            return;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.cells.entry((x, y)).or_default().push(slot);
            }
        }
    }

    fn remove(&mut self, slot: u32, b: &Aabb) {
        match self.range(b) {
            Some((x0, y0, x1, y1)) if !Self::is_large(x0, y0, x1, y1) => {
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        if let Some(v) = self.cells.get_mut(&(x, y)) {
                            v.retain(|&s| s != slot);
                            if v.is_empty() {
                                self.cells.remove(&(x, y));
                            }
                        }
                    }
                }
            }
            _ => self.large.retain(|&s| s != slot),
        }
    }

    fn query(&self, rect: &Aabb, out: &mut Vec<u32>) {
        out.extend_from_slice(&self.large);
        if let Some((x0, y0, x1, y1)) = self.range(rect) {
            let cells = (x1 as i64 - x0 as i64 + 1) * (y1 as i64 - y0 as i64 + 1);
            if cells as usize > self.cells.len() {
                // Query bigger than the populated area: walk the map instead.
                for (&(x, y), v) in &self.cells {
                    if x >= x0 && x <= x1 && y >= y0 && y <= y1 {
                        out.extend_from_slice(v);
                    }
                }
            } else {
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        if let Some(v) = self.cells.get(&(x, y)) {
                            out.extend_from_slice(v);
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::Shape;
    use crate::style::{Color, Stroke};

    fn line_item(a: Vec2, b: Vec2) -> Item {
        Item::new(LayerId(0)).with(Primitive::stroked(
            Shape::line(a, b),
            Stroke::world(0.5, Color::BLACK),
        ))
    }

    #[test]
    fn insert_replace_remove() {
        let mut s = Scene::new();
        let r0 = s.revision();
        s.insert(ItemId(1), line_item(Vec2::ZERO, Vec2::new(10.0, 0.0)));
        assert!(s.revision() > r0);
        assert_eq!(s.len(), 1);
        s.insert(
            ItemId(1),
            line_item(Vec2::new(100.0, 0.0), Vec2::new(110.0, 0.0)),
        );
        assert_eq!(s.len(), 1);
        assert!(s.pick(Vec2::new(5.0, 0.0), 1.0, 1.0).is_none());
        assert_eq!(
            s.pick(Vec2::new(105.0, 0.2), 1.0, 1.0).unwrap().id,
            ItemId(1)
        );
        assert!(s.remove(ItemId(1)).is_some());
        assert!(s.is_empty());
        assert!(s.pick(Vec2::new(105.0, 0.0), 1.0, 1.0).is_none());
    }

    #[test]
    fn tallest_label_is_tracked_until_cleared() {
        let mut s = Scene::new();
        assert_eq!(s.max_label_height(), 0.0);
        s.insert(ItemId(1), line_item(Vec2::ZERO, Vec2::X));
        assert_eq!(s.max_label_height(), 0.0);
        let labelled =
            |h| line_item(Vec2::ZERO, Vec2::X).with_label(crate::Label::new(Vec2::ZERO, "-K1", h));
        s.insert(ItemId(2), labelled(2.5));
        s.insert(ItemId(3), labelled(5.0));
        assert_eq!(s.max_label_height(), 5.0);
        // Never underestimates: removal keeps the old maximum.
        s.remove(ItemId(3));
        assert_eq!(s.max_label_height(), 5.0);
        s.clear();
        assert_eq!(s.max_label_height(), 0.0);
    }

    #[test]
    fn pick_prefers_closest_then_topmost() {
        let mut s = Scene::new();
        let top = s.add_layer(Layer::new("Top", 10));
        s.insert(ItemId(1), line_item(Vec2::ZERO, Vec2::new(10.0, 0.0)));
        s.insert(
            ItemId(2),
            line_item(Vec2::new(0.0, 1.0), Vec2::new(10.0, 1.0)),
        );
        assert_eq!(s.pick(Vec2::new(5.0, 0.9), 2.0, 1.0).unwrap().id, ItemId(2));
        let mut it = line_item(Vec2::ZERO, Vec2::new(10.0, 0.0));
        it.layer = top;
        s.insert(ItemId(3), it);
        assert_eq!(s.pick(Vec2::new(5.0, 0.0), 2.0, 1.0).unwrap().id, ItemId(3));
        assert_eq!(s.pick_all(Vec2::new(5.0, 0.5), 2.0, 1.0).len(), 3);
    }

    #[test]
    fn hidden_and_locked_layers_are_not_pickable() {
        let mut s = Scene::new();
        let l = s.add_layer(Layer::new("Wires", 1));
        let mut it = line_item(Vec2::ZERO, Vec2::new(10.0, 0.0));
        it.layer = l;
        s.insert(ItemId(1), it);
        assert!(s.pick(Vec2::new(5.0, 0.0), 1.0, 1.0).is_some());
        s.layer_mut(l).unwrap().locked = true;
        assert!(s.pick(Vec2::new(5.0, 0.0), 1.0, 1.0).is_none());
        s.layer_mut(l).unwrap().locked = false;
        s.layer_mut(l).unwrap().visible = false;
        assert!(s.pick(Vec2::new(5.0, 0.0), 1.0, 1.0).is_none());
    }

    #[test]
    fn box_modes() {
        let mut s = Scene::new();
        s.insert(
            ItemId(1),
            line_item(Vec2::new(1.0, 1.0), Vec2::new(4.0, 1.0)),
        );
        s.insert(
            ItemId(2),
            line_item(Vec2::new(8.0, 1.0), Vec2::new(30.0, 1.0)),
        );
        let r = Aabb::new(Vec2::ZERO, Vec2::splat(10.0));
        assert_eq!(s.box_select(&r, BoxMode::Window, 1.0), vec![ItemId(1)]);
        let mut c = s.box_select(&r, BoxMode::Crossing, 1.0);
        c.sort();
        assert_eq!(c, vec![ItemId(1), ItemId(2)]);
        assert_eq!(BoxMode::from_drag(Vec2::ZERO, Vec2::ONE), BoxMode::Window);
        assert_eq!(BoxMode::from_drag(Vec2::ONE, Vec2::ZERO), BoxMode::Crossing);
    }

    #[test]
    fn large_items_and_index_consistency() {
        let mut s = Scene::with_cell_size(1.0);
        // Spans far more than MAX_CELLS_PER_ITEM cells.
        s.insert(
            ItemId(1000),
            Item::new(LayerId(0)).with(Primitive::stroked(
                Shape::rect(Vec2::ZERO, Vec2::splat(500.0)),
                Stroke::default(),
            )),
        );
        for i in 0..200 {
            let x = i as f32 * 2.0;
            s.insert(
                ItemId(i),
                line_item(Vec2::new(x, 50.0), Vec2::new(x + 1.0, 50.0)),
            );
        }
        for i in (0..200).step_by(2) {
            s.remove(ItemId(i));
        }
        assert_eq!(s.len(), 101);
        assert!(s.pick(Vec2::new(0.5, 50.0), 0.1, 1.0).is_none());
        assert_eq!(
            s.pick(Vec2::new(2.5, 50.0), 0.1, 1.0).unwrap().id,
            ItemId(1)
        );
        assert_eq!(
            s.pick(Vec2::new(250.0, 0.0), 0.5, 1.0).unwrap().id,
            ItemId(1000)
        );
        let all = s.query(&Aabb::new(Vec2::splat(-1e6), Vec2::splat(1e6)));
        assert_eq!(all.len(), 101);
    }

    #[test]
    fn screen_width_strokes_are_found_when_zoomed_out() {
        let mut s = Scene::new();
        s.insert(
            ItemId(1),
            Item::new(LayerId(0)).with(Primitive::stroked(
                Shape::line(Vec2::ZERO, Vec2::new(10.0, 0.0)),
                Stroke::screen(10.0, Color::BLACK),
            )),
        );
        // At zoom 0.1 a 10 pt stroke is 100 world units wide.
        assert!(s.pick(Vec2::new(5.0, 40.0), 0.0, 0.1).is_some());
        assert!(s.pick(Vec2::new(5.0, 40.0), 0.0, 1.0).is_none());
    }

    #[test]
    fn unselectable_items_are_skipped() {
        let mut s = Scene::new();
        s.insert(
            ItemId(1),
            line_item(Vec2::ZERO, Vec2::X * 10.0).unselectable(),
        );
        assert!(s.pick(Vec2::new(5.0, 0.0), 1.0, 1.0).is_none());
        assert!(
            s.box_select(
                &Aabb::new(Vec2::splat(-1.0), Vec2::splat(20.0)),
                BoxMode::Window,
                1.0
            )
            .is_empty()
        );
    }
}

#[cfg(test)]
mod label_pick_tests {
    use super::*;
    use crate::shape::{Primitive, Shape};
    use crate::style::Stroke;
    use crate::text::Label;

    #[test]
    fn picking_reports_the_label() {
        let mut scene = Scene::new();
        scene.insert(
            ItemId(1),
            Item::new(LayerId(0))
                .with(Primitive::stroked(
                    Shape::line(Vec2::ZERO, Vec2::new(0.0, 10.0)),
                    Stroke::default(),
                ))
                .with_label(Label::new(Vec2::new(5.0, 5.0), "-K1", 2.5)),
        );
        let on_text = scene.pick(Vec2::new(6.0, 4.0), 0.5, 2.0).unwrap();
        assert_eq!((on_text.id, on_text.label), (ItemId(1), Some(0)));
        let on_line = scene.pick(Vec2::new(0.1, 8.0), 0.5, 2.0).unwrap();
        assert_eq!(on_line.label, None);
    }

    #[test]
    fn a_label_wins_over_ink_under_it() {
        let mut scene = Scene::new();
        scene.insert(
            ItemId(1),
            Item::new(LayerId(0))
                .with(Primitive::stroked(
                    Shape::line(Vec2::new(0.0, 5.0), Vec2::new(20.0, 5.0)),
                    Stroke::default(),
                ))
                .with_label(Label::new(Vec2::new(5.0, 5.0), "-K1", 2.5)),
        );
        // Inside the label box, right on the line.
        let inside = scene.pick(Vec2::new(6.0, 5.0), 0.5, 2.0).unwrap();
        assert_eq!(inside.label, Some(0));
        // Past the box the line still wins.
        let past = scene.pick(Vec2::new(18.0, 5.0), 0.5, 2.0).unwrap();
        assert_eq!(past.label, None);
    }

    /// A junction drawn plain has no ink or text, but is still picked and
    /// box-selected at its connection point.
    #[test]
    fn an_item_without_ink_is_picked_at_its_snap_points() {
        let mut s = Scene::new();
        let at = Vec2::new(10.0, 20.0);
        s.insert(
            ItemId(7),
            Item::new(LayerId(0)).with_snap(SnapPoint::connection(at)),
        );
        assert_eq!(
            s.pick(at + Vec2::new(0.5, 0.0), 1.0, 1.0).unwrap().id,
            ItemId(7)
        );
        assert!(s.pick(at + Vec2::new(3.0, 0.0), 1.0, 1.0).is_none());
        let around = Aabb::new(at - Vec2::ONE, at + Vec2::ONE);
        let beside = Aabb::new(at + Vec2::splat(2.0), at + Vec2::splat(4.0));
        for mode in [BoxMode::Window, BoxMode::Crossing] {
            assert_eq!(s.box_select(&around, mode, 1.0), vec![ItemId(7)]);
            assert!(s.box_select(&beside, mode, 1.0).is_empty());
        }
    }
}
