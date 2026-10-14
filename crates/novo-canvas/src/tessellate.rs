//! Turns primitives into triangles for the GPU.
//!
//! Strokes are emitted as quads along each run with mitred joins. Every
//! stroke vertex carries its width both in world units and in screen points
//! plus an extrusion direction, and the vertex shader does the final
//! extrusion. That keeps the mesh valid at every zoom level: panning and
//! zooming only change a uniform, never the mesh. The shader also adds a
//! one-pixel fringe and fades it out, which anti-aliases lines without MSAA.

use crate::scene::{Item, ItemId, LayerId, Scene};
use crate::shape::{Primitive, Shape};
use crate::style::{Color, Stroke, WidthUnit};
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use std::collections::HashMap;
use std::ops::Range;

/// GPU vertex. 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    /// World position of the stroke centreline (or of the fill vertex).
    pub pos: [f32; 2],
    /// Unit-ish extrusion direction (mitre-scaled), zero for fills.
    pub extrude: [f32; 2],
    /// Stroke width in world units.
    pub width_world: f32,
    /// Stroke width in screen points; negative marks a fill vertex.
    pub width_screen: f32,
    /// -1 or +1 across the stroke, used for anti-aliasing.
    pub edge: f32,
    /// Packed straight-alpha sRGB colour.
    pub color: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }
}

/// Mitre joins sharper than this (ratio of mitre length to half width) are
/// split into separate segments instead.
const MITER_LIMIT: f32 = 4.0;

/// World tolerance used to flatten curves (0.01 mm).
pub const DEFAULT_TOLERANCE: f32 = 0.01;

/// Overrides applied when drawing highlights (selection, hover).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Highlight {
    pub color: Color,
    /// Extra stroke width in screen points.
    pub extra_width: f32,
    /// Opacity applied to fills.
    pub fill_alpha: u8,
}

impl Highlight {
    pub fn selection() -> Self {
        Self {
            color: Color::SELECTION,
            extra_width: 3.0,
            fill_alpha: 70,
        }
    }

    pub fn hover() -> Self {
        Self {
            color: Color::HOVER,
            extra_width: 2.0,
            fill_alpha: 50,
        }
    }
}

pub struct Tessellator {
    pub tolerance: f32,
}

impl Default for Tessellator {
    fn default() -> Self {
        Self {
            tolerance: DEFAULT_TOLERANCE,
        }
    }
}

impl Tessellator {
    pub fn primitive(&self, mesh: &mut Mesh, p: &Primitive) {
        if let Some(fill) = p.fill
            && fill.0[3] > 0
            && let Some(poly) = p.shape.area_polygon(self.tolerance)
        {
            fill_polygon(
                mesh,
                &poly,
                fill,
                !matches!(p.shape, Shape::Polyline { .. }),
            );
        }
        if let Some(stroke) = p.stroke
            && stroke.color.0[3] > 0
        {
            for run in p.shape.outline(self.tolerance) {
                stroke_run(mesh, &run, &stroke);
            }
        }
    }

    pub fn highlighted(&self, mesh: &mut Mesh, p: &Primitive, h: &Highlight) {
        let stroke = p.stroke.unwrap_or(Stroke::screen(0.0, h.color));
        let (width_world, width_screen) = widths(&stroke);
        let s = HighlightStroke {
            width_world,
            width_screen: width_screen + h.extra_width,
            color: h.color,
        };
        if p.fill.is_some()
            && let Some(poly) = p.shape.area_polygon(self.tolerance)
        {
            fill_polygon(
                mesh,
                &poly,
                h.color.with_alpha(h.fill_alpha),
                !matches!(p.shape, Shape::Polyline { .. }),
            );
        }
        for run in p.shape.outline(self.tolerance) {
            stroke_run_raw(mesh, &run, s.width_world, s.width_screen, s.color);
        }
    }

    pub fn item(&self, mesh: &mut Mesh, item: &Item) {
        for p in &item.primitives {
            self.primitive(mesh, p);
        }
    }

    pub fn item_highlighted(&self, mesh: &mut Mesh, item: &Item, h: &Highlight) {
        for p in &item.primitives {
            self.highlighted(mesh, p, h);
        }
    }

    /// Tessellates the whole scene in draw order and records the index range
    /// of each layer so the renderer can hide layers without rebuilding.
    pub fn scene(&self, scene: &Scene) -> SceneMesh {
        self.scene_cached(scene, &mut ItemMeshCache::default())
    }

    /// Like [`Tessellator::scene`], but reuses per-item meshes from `cache`
    /// for items that did not change, so editing one item on a big page only
    /// re-tessellates that item.
    pub fn scene_cached(&self, scene: &Scene, cache: &mut ItemMeshCache) -> SceneMesh {
        let order = scene.draw_order();
        let mut mesh = Mesh::default();
        let mut layers: Vec<(LayerId, Range<u32>)> = Vec::new();
        cache.generation = cache.generation.wrapping_add(1);
        let generation = cache.generation;
        for (id, item) in order {
            let rev = scene.item_revision(id).unwrap_or(0);
            let entry = cache.items.entry(id).or_insert_with(|| CachedItem {
                revision: u64::MAX,
                generation,
                mesh: Mesh::default(),
            });
            if entry.revision != rev {
                entry.mesh.clear();
                self.item(&mut entry.mesh, item);
                entry.revision = rev;
            }
            entry.generation = generation;
            let start = mesh.indices.len() as u32;
            let base = mesh.vertices.len() as u32;
            mesh.vertices.extend_from_slice(&entry.mesh.vertices);
            mesh.indices
                .extend(entry.mesh.indices.iter().map(|i| i + base));
            let end = mesh.indices.len() as u32;
            match layers.last_mut() {
                Some((l, r)) if *l == item.layer => r.end = end,
                _ => layers.push((item.layer, start..end)),
            }
        }
        cache.items.retain(|_, c| c.generation == generation);
        SceneMesh {
            revision: scene.revision(),
            mesh,
            layers,
        }
    }
}

/// Per-item meshes kept between scene rebuilds.
#[derive(Default)]
pub struct ItemMeshCache {
    items: HashMap<ItemId, CachedItem>,
    generation: u64,
}

impl ItemMeshCache {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

struct CachedItem {
    revision: u64,
    generation: u64,
    mesh: Mesh,
}

struct HighlightStroke {
    width_world: f32,
    width_screen: f32,
    color: Color,
}

/// Tessellated scene plus the index range of each layer, in draw order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneMesh {
    pub revision: u64,
    pub mesh: Mesh,
    pub layers: Vec<(LayerId, Range<u32>)>,
}

impl SceneMesh {
    /// Index ranges to draw given layer visibility, merged where adjacent.
    pub fn visible_ranges(&self, scene: &Scene) -> Vec<Range<u32>> {
        let mut out: Vec<Range<u32>> = Vec::new();
        for (l, r) in &self.layers {
            if r.is_empty() || !scene.layer(*l).is_some_and(|l| l.visible) {
                continue;
            }
            match out.last_mut() {
                Some(last) if last.end == r.start => last.end = r.end,
                _ => out.push(r.clone()),
            }
        }
        out
    }
}

fn widths(s: &Stroke) -> (f32, f32) {
    match s.unit {
        WidthUnit::World => (s.width.max(0.0), 0.0),
        WidthUnit::Screen => (0.0, s.width.max(0.0)),
    }
}

/// Strokes an open run of points (closed when the last equals the first).
pub fn stroke_run(mesh: &mut Mesh, run: &[Vec2], stroke: &Stroke) {
    let (ww, ws) = widths(stroke);
    stroke_run_raw(mesh, run, ww, ws, stroke.color);
}

fn stroke_run_raw(
    mesh: &mut Mesh,
    run: &[Vec2],
    width_world: f32,
    width_screen: f32,
    color: Color,
) {
    // Drop repeated points.
    let mut pts: Vec<Vec2> = Vec::with_capacity(run.len());
    for &p in run {
        if !p.is_finite() {
            return;
        }
        if pts
            .last()
            .is_none_or(|q: &Vec2| q.distance_squared(p) > 1e-12)
        {
            pts.push(p);
        }
    }
    if pts.len() < 2 {
        return;
    }
    let closed = pts.len() > 2 && pts[0].distance_squared(pts[pts.len() - 1]) <= 1e-12;
    if closed {
        pts.pop();
    }
    let n = pts.len();
    let seg_count = if closed { n } else { n - 1 };
    let normal = |i: usize| -> Vec2 {
        let a = pts[i % n];
        let b = pts[(i + 1) % n];
        (b - a).normalize().perp()
    };
    let color = color.to_u32();
    let push_pair = |mesh: &mut Mesh, p: Vec2, ext: Vec2| -> u32 {
        let base = mesh.vertices.len() as u32;
        for edge in [-1.0f32, 1.0] {
            mesh.vertices.push(Vertex {
                pos: p.to_array(),
                extrude: (ext * edge).to_array(),
                width_world,
                width_screen,
                edge,
                color,
            });
        }
        base
    };
    let quad = |mesh: &mut Mesh, a: u32, b: u32| {
        mesh.indices
            .extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
    };

    // Mitre at vertex i between segment i-1 and segment i, or None if too
    // sharp (the run is then split there).
    let miter = |i: usize| -> Option<Vec2> {
        let n0 = normal((i + seg_count - 1) % seg_count.max(1));
        let n1 = normal(i % n);
        let m = n0 + n1;
        if m.length_squared() < 1e-8 {
            return None;
        }
        let m = m.normalize();
        let scale = 1.0 / m.dot(n1).max(1e-6);
        (scale <= MITER_LIMIT).then_some(m * scale)
    };

    let mut prev: Option<u32> = None;
    for s in 0..seg_count {
        let i0 = s;
        let i1 = (s + 1) % n;
        let nrm = normal(s);
        let start_ext = if s == 0 && !closed { None } else { miter(i0) };
        let end_ext = if s + 1 == seg_count && !closed {
            None
        } else {
            miter(i1)
        };
        let a = match (prev, start_ext) {
            (Some(p), Some(_)) => p,
            _ => push_pair(mesh, pts[i0], start_ext.unwrap_or(nrm)),
        };
        let b = push_pair(mesh, pts[i1], end_ext.unwrap_or(nrm));
        quad(mesh, a, b);
        prev = end_ext.map(|_| b);
    }
    // A closed run ends on a fresh pair at the first point with the same
    // mitre as its start pair, so the loop closes without a seam.
}

/// Fills a polygon. Convex shapes use a fan; others use ear clipping.
pub fn fill_polygon(mesh: &mut Mesh, poly: &[Vec2], color: Color, convex: bool) {
    if poly.len() < 3 {
        return;
    }
    let base = mesh.vertices.len() as u32;
    let c = color.to_u32();
    mesh.vertices.extend(poly.iter().map(|p| Vertex {
        pos: p.to_array(),
        extrude: [0.0; 2],
        width_world: 0.0,
        width_screen: -1.0,
        edge: 0.0,
        color: c,
    }));
    if convex {
        for i in 1..poly.len() as u32 - 1 {
            mesh.indices
                .extend_from_slice(&[base, base + i, base + i + 1]);
        }
    } else {
        for [a, b, c] in ear_clip(poly) {
            mesh.indices
                .extend_from_slice(&[base + a, base + b, base + c]);
        }
    }
}

fn signed_area(poly: &[Vec2]) -> f32 {
    let n = poly.len();
    (0..n)
        .map(|i| poly[i].perp_dot(poly[(i + 1) % n]))
        .sum::<f32>()
        * 0.5
}

/// Ear clipping triangulation of a simple polygon (O(n^2), fine for the
/// small outlines symbols use).
pub fn ear_clip(poly: &[Vec2]) -> Vec<[u32; 3]> {
    let n = poly.len();
    let mut out = Vec::with_capacity(n.saturating_sub(2));
    if n < 3 {
        return out;
    }
    let mut idx: Vec<u32> = (0..n as u32).collect();
    let sign = signed_area(poly).signum();
    let p = |i: u32| poly[i as usize];
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for k in 0..m {
            let (a, b, c) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let cross = (p(b) - p(a)).perp_dot(p(c) - p(b));
            if cross * sign <= 0.0 {
                continue; // reflex or degenerate
            }
            let inside = idx
                .iter()
                .any(|&q| q != a && q != b && q != c && point_in_triangle(p(q), p(a), p(b), p(c)));
            if inside {
                continue;
            }
            out.push([a, b, c]);
            idx.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            break; // not simple; give up on the remainder
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = (b - a).perp_dot(p - a);
    let d2 = (c - b).perp_dot(p - b);
    let d3 = (a - c).perp_dot(p - c);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Layer;

    #[test]
    fn line_is_one_quad() {
        let mut m = Mesh::default();
        stroke_run(
            &mut m,
            &[Vec2::ZERO, Vec2::new(10.0, 0.0)],
            &Stroke::world(1.0, Color::BLACK),
        );
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.indices.len(), 6);
        // Extrusion is perpendicular to the line.
        assert_eq!(m.vertices[0].extrude, [0.0, -1.0]);
        assert_eq!(m.vertices[1].extrude, [0.0, 1.0]);
        assert_eq!(m.vertices[0].width_world, 1.0);
    }

    #[test]
    fn right_angle_mitre() {
        let mut m = Mesh::default();
        stroke_run(
            &mut m,
            &[Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0)],
            &Stroke::screen(2.0, Color::BLACK),
        );
        // Shared joint: 3 pairs.
        assert_eq!(m.vertices.len(), 6);
        let joint = Vec2::from(m.vertices[2].extrude);
        assert!((joint.length() - 2f32.sqrt()).abs() < 1e-5);
    }

    #[test]
    fn hairpin_splits_instead_of_spiking() {
        let mut m = Mesh::default();
        stroke_run(
            &mut m,
            &[Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(0.0, 0.1)],
            &Stroke::world(1.0, Color::BLACK),
        );
        for v in &m.vertices {
            assert!(Vec2::from(v.extrude).length() <= MITER_LIMIT + 1e-3);
        }
        assert_eq!(m.vertices.len(), 8);
    }

    #[test]
    fn closed_rect_outline() {
        let mut m = Mesh::default();
        let t = Tessellator::default();
        t.primitive(
            &mut m,
            &Primitive::stroked(
                Shape::rect(Vec2::ZERO, Vec2::splat(10.0)),
                Stroke::default(),
            )
            .with_fill(Color::WHITE),
        );
        // 4 fill vertices + 4 segments with mitred joints (4 pairs + 1 for the
        // first segment's end, reusing pairs where mitred).
        let fill = m.vertices.iter().filter(|v| v.width_screen < 0.0).count();
        assert_eq!(fill, 4);
        assert_eq!(m.indices.len(), 6 + 4 * 6);
        for &i in &m.indices {
            assert!((i as usize) < m.vertices.len());
        }
    }

    #[test]
    fn degenerate_input_is_ignored() {
        let mut m = Mesh::default();
        stroke_run(&mut m, &[Vec2::ONE, Vec2::ONE], &Stroke::default());
        stroke_run(
            &mut m,
            &[Vec2::ONE, Vec2::new(f32::NAN, 0.0)],
            &Stroke::default(),
        );
        fill_polygon(&mut m, &[Vec2::ZERO, Vec2::ONE], Color::BLACK, true);
        assert!(m.is_empty());
    }

    #[test]
    fn ear_clip_concave() {
        // An L shape (6 vertices -> 4 triangles).
        let l = [
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ];
        let tris = ear_clip(&l);
        assert_eq!(tris.len(), 4);
        let area: f32 = tris
            .iter()
            .map(|t| {
                let (a, b, c) = (l[t[0] as usize], l[t[1] as usize], l[t[2] as usize]);
                ((b - a).perp_dot(c - a) * 0.5).abs()
            })
            .sum();
        assert!((area - 3.0).abs() < 1e-5);
    }

    #[test]
    fn scene_mesh_layer_ranges() {
        let mut s = Scene::new();
        let top = s.add_layer(Layer::new("Top", 5));
        let line = |layer| {
            Item::new(layer).with(Primitive::stroked(
                Shape::line(Vec2::ZERO, Vec2::X),
                Stroke::default(),
            ))
        };
        s.insert(ItemId(1), line(top));
        s.insert(ItemId(2), line(LayerId(0)));
        s.insert(ItemId(3), line(LayerId(0)));
        let sm = Tessellator::default().scene(&s);
        assert_eq!(sm.layers.len(), 2);
        assert_eq!(sm.layers[0].0, LayerId(0), "lower layer draws first");
        assert_eq!(sm.layers[0].1, 0..12);
        assert_eq!(sm.layers[1].1, 12..18);
        assert_eq!(sm.visible_ranges(&s), vec![0..18]);
        s.layer_mut(LayerId(0)).unwrap().visible = false;
        assert_eq!(sm.visible_ranges(&s), vec![12..18]);
    }

    #[test]
    fn cached_rebuild_matches_full_rebuild() {
        let mut s = crate::demo::stress_scene(500);
        let t = Tessellator::default();
        let mut cache = ItemMeshCache::default();
        assert_eq!(t.scene_cached(&s, &mut cache).mesh, t.scene(&s).mesh);
        // Replace one item, remove another, add a third.
        s.insert(
            ItemId(3),
            Item::new(LayerId(0)).with(Primitive::stroked(
                Shape::circle(Vec2::ZERO, 3.0),
                Stroke::default(),
            )),
        );
        s.remove(ItemId(4));
        s.insert(
            ItemId(99_999),
            Item::new(LayerId(0)).with(Primitive::stroked(
                Shape::line(Vec2::ZERO, Vec2::ONE),
                Stroke::default(),
            )),
        );
        let cached = t.scene_cached(&s, &mut cache);
        let full = t.scene(&s);
        assert_eq!(cached.mesh, full.mesh);
        assert_eq!(cached.layers, full.layers);
        assert_eq!(cache.len(), s.len());
    }
}
