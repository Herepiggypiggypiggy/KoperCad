//! The EMC picture as overlays (`crate::overlay`) and as a recoloured scene.
//! Plain geometry in the scene frame; the app hands the result to the view
//! when the picture or the layers change.
//!
//! Draw order, low first: the exposure map, the keep-clear halos, the glow of
//! the sources, loop sheets, then lines and markers on top.

use glam::Vec3;

use super::field;
use super::geom::{self, Mesh, rgba};
use super::picture::{Hotspot, Picture, Quality, Role, ShieldEnd};
use super::style::{EmcLayers, HALO_ALPHA, Style};
use crate::overlay::{Overlay, OverlayLines, OverlayMesh};
use crate::scene::{ItemKind, PlateFrame, Scene};

/// Height above the plate at which flat sheets are drawn, mm. The exposure map
/// sits at the height most wires lie at, the halos just under it.
pub const MAP_HEIGHT_MM: f32 = 12.0;

fn world(frame: &PlateFrame, p: [f32; 3]) -> Vec3 {
    frame.world(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]))
}

fn world_xy(frame: &PlateFrame, x: f32, y: f32, z: f32) -> Vec3 {
    frame.world(f64::from(x), f64::from(y), f64::from(z))
}

fn mesh_overlay(m: Mesh, order: i32) -> Option<Overlay> {
    (!m.is_empty()).then_some({
        Overlay::Mesh(OverlayMesh {
            positions: m.positions,
            colors: m.colors,
            indices: m.indices,
            translucent: true,
            order,
        })
    })
}

fn opaque_overlay(m: Mesh, order: i32) -> Option<Overlay> {
    (!m.is_empty()).then_some({
        Overlay::Mesh(OverlayMesh {
            positions: m.positions,
            colors: m.colors,
            indices: m.indices,
            translucent: false,
            order,
        })
    })
}

struct Lines {
    positions: Vec<Vec3>,
    colors: Vec<[u8; 4]>,
    starts: Vec<u32>,
}

impl Lines {
    fn new() -> Self {
        Lines {
            positions: Vec::new(),
            colors: Vec::new(),
            starts: Vec::new(),
        }
    }

    fn polyline(&mut self, pts: &[Vec3], c: [u8; 4]) {
        if pts.len() < 2 {
            return;
        }
        self.starts.push(self.positions.len() as u32);
        for p in pts {
            self.positions.push(*p);
            self.colors.push(c);
        }
    }

    fn finish(self, width_px: f32, order: i32) -> Option<Overlay> {
        (!self.positions.is_empty()).then_some({
            Overlay::Lines(OverlayLines {
                positions: self.positions,
                colors: self.colors,
                starts: self.starts,
                width_px,
                order,
            })
        })
    }
}

/// The exposure map as a sheet over the plate: one square per cell, coloured
/// by the risk ramp, clear where it is calm.
fn map_sheet(pic: &Picture, frame: &PlateFrame, style: &Style) -> Option<Overlay> {
    let f = field::exposure(pic);
    let mut m = Mesh::default();
    let z = MAP_HEIGHT_MM;
    // Corners shared by neighbouring cells: one vertex per grid corner, the
    // colour of the cell it belongs to is averaged by the renderer's
    // interpolation, so the sheet reads as a smooth map and not as tiles.
    let corner = |i: usize, j: usize| -> f32 {
        let mut sum = 0.0;
        let mut n = 0.0;
        for (di, dj) in [(0usize, 0usize), (1, 0), (0, 1), (1, 1)] {
            if i >= di && j >= dj && i - di < f.nx && j - dj < f.ny {
                sum += f.values[(j - dj) * f.nx + (i - di)];
                n += 1.0;
            }
        }
        if n > 0.0 { sum / n } else { 0.0 }
    };
    let mut ids = vec![0u32; (f.nx + 1) * (f.ny + 1)];
    for j in 0..=f.ny {
        for i in 0..=f.nx {
            let v = corner(i, j);
            // Calm areas fade out so the cabinet shows through.
            let a = (((v - 0.04) / 0.5).clamp(0.0, 1.0) * 190.0).round() as u8;
            let x = (f.origin[0] + i as f32 * f.cell).min(pic.plate[0]);
            let y = (f.origin[1] + j as f32 * f.cell).min(pic.plate[1]);
            ids[j * (f.nx + 1) + i] = m.vertex(world_xy(frame, x, y, z), rgba(style.risk(v), a));
        }
    }
    for j in 0..f.ny {
        for i in 0..f.nx {
            let a = ids[j * (f.nx + 1) + i];
            let b = ids[j * (f.nx + 1) + i + 1];
            let c = ids[(j + 1) * (f.nx + 1) + i + 1];
            let d = ids[(j + 1) * (f.nx + 1) + i];
            m.quad(a, b, c, d);
        }
    }
    mesh_overlay(m, -2)
}

/// Pulls every vertex of a band back onto the plate rectangle, so a band
/// ends at the plate edge instead of running over what lies beside it.
fn clamp_to_plate(m: &mut Mesh, pic: &Picture, frame: &PlateFrame) {
    let [w, h] = pic.plate;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    for v in &mut m.positions {
        let (x, y) = frame.plate(*v);
        let (cx, cy) = (x.clamp(0.0, f64::from(w)), y.clamp(0.0, f64::from(h)));
        if (cx, cy) != (x, y) {
            let moved = frame.world(cx, cy, 0.0) - frame.world(x, y, 0.0);
            *v += moved;
        }
    }
}

fn halos(pic: &Picture, frame: &PlateFrame, style: &Style) -> Option<Overlay> {
    let mut m = Mesh::default();
    for r in pic.runs.iter().filter(|r| r.halo_mm > 0.0) {
        let colour = style.colour(r.class);
        let path: Vec<Vec3> = r
            .path
            .iter()
            .map(|p| world(frame, [p[0], p[1], MAP_HEIGHT_MM - 1.0]))
            .collect();
        // Three nested strips give a soft zone: strongest at the run, edge
        // at the distance the engine asked to keep.
        for (k, alpha) in HALO_ALPHA.iter().enumerate() {
            let share = 1.0 - k as f32 * 0.3;
            geom::strip(&mut m, &path, r.halo_mm * share, colour, *alpha, *alpha);
        }
    }
    clamp_to_plate(&mut m, pic, frame);
    mesh_overlay(m, -1)
}

/// The glow around each noisy part: a disc on the plate that fades with
/// distance and reaches as far as the part's loudness does.
fn glows(pic: &Picture, frame: &PlateFrame, style: &Style) -> Option<Overlay> {
    let mut m = Mesh::default();
    for n in pic.nodes.iter().filter(|n| n.role == Role::Source) {
        let c = n.centre();
        let reach = field::reach_mm(n.level) * 1.6;
        let colour = style.colour(super::picture::Class::Motor);
        let at = world_xy(frame, c[0], c[1], MAP_HEIGHT_MM - 2.0);
        geom::glow_disc(&mut m, at, reach, colour, (60.0 + 90.0 * n.level) as u8);
    }
    mesh_overlay(m, 0)
}

/// How a marker looks: the colour of its risk, bigger for a worse one.
fn marker_size(risk: f32) -> f32 {
    7.0 + 7.0 * risk
}

fn hotspot_overlays(
    pic: &Picture,
    frame: &PlateFrame,
    style: &Style,
    selected: Option<u32>,
    out: &mut Vec<Overlay>,
) {
    let mut gems = Mesh::default();
    let mut rings = Lines::new();
    let mut poles = Lines::new();
    let mut spans = Lines::new();
    let mut span_glow = Mesh::default();
    for h in &pic.hotspots {
        let colour = style.risk(0.55 + 0.45 * h.risk);
        let on = selected == Some(h.id);
        let base = world(frame, [h.at[0], h.at[1], 0.0]);
        let top = world(frame, h.at);
        let size = marker_size(h.risk) * if on { 1.4 } else { 1.0 };
        geom::gem(&mut gems, top, size, colour);
        poles.polyline(&[base, top - Vec3::Z * size], rgba(colour, 200));
        // Rings on the plate; the selected one is double.
        rings.polyline(
            &geom::ring(base + Vec3::Z * 1.0, 18.0 + 22.0 * h.risk),
            rgba(colour, 230),
        );
        if on {
            rings.polyline(
                &geom::ring(base + Vec3::Z * 1.0, 40.0 + 22.0 * h.risk),
                rgba(colour, 160),
            );
        }
        if h.span.len() >= 2 {
            let pts: Vec<Vec3> = h.span.iter().map(|p| world(frame, *p)).collect();
            spans.polyline(&pts, rgba(colour, if on { 255 } else { 190 }));
            let flat: Vec<Vec3> = pts
                .iter()
                .map(|p| Vec3::new(p.x, p.y, MAP_HEIGHT_MM))
                .collect();
            geom::strip(&mut span_glow, &flat, 14.0, colour, 90, 0);
        }
    }
    out.extend(mesh_overlay(span_glow, 1));
    out.extend(spans.finish(4.0, 3));
    out.extend(poles.finish(1.5, 3));
    out.extend(rings.finish(2.5, 3));
    out.extend(opaque_overlay(gems, 4));
}

fn loop_overlays(pic: &Picture, frame: &PlateFrame, style: &Style, out: &mut Vec<Overlay>) {
    let mut sheets = Mesh::default();
    let mut edges = Lines::new();
    for l in &pic.loops {
        let pts: Vec<Vec3> = l.outline.iter().map(|p| world(frame, *p)).collect();
        let colour = style.risk(0.45 + 0.5 * l.risk);
        geom::fan(&mut sheets, &pts, colour, (70.0 + 80.0 * l.risk) as u8);
        edges.polyline(&pts, rgba(colour, 235));
    }
    out.extend(mesh_overlay(sheets, 2));
    out.extend(edges.finish(2.0, 3));
}

fn bonding_overlays(pic: &Picture, frame: &PlateFrame, style: &Style, out: &mut Vec<Overlay>) {
    let mut lines_a = Lines::new();
    let mut lines_b = Lines::new();
    let mut flags = Mesh::default();
    for b in &pic.bonds {
        let pts: Vec<Vec3> = b.path.iter().map(|p| world(frame, *p)).collect();
        for (dash, k) in geom::dashes(&pts, 9.0) {
            let (c, set) = if k % 2 == 0 {
                (style.earth.0, &mut lines_a)
            } else {
                (style.earth.1, &mut lines_b)
            };
            set.polyline(&dash, rgba(c, 255));
        }
        if b.quality != Quality::Good
            && let Some(mid) = pts.get(pts.len() / 2)
        {
            let risk = if b.quality == Quality::Bad { 0.95 } else { 0.7 };
            geom::glow_disc(
                &mut flags,
                *mid + Vec3::Z * 2.0,
                38.0,
                style.risk(risk),
                150,
            );
        }
    }
    out.extend(mesh_overlay(flags, 1));
    out.extend(lines_a.finish(4.0, 3));
    out.extend(lines_b.finish(4.0, 3));
}

fn shield_overlays(pic: &Picture, frame: &PlateFrame, style: &Style, out: &mut Vec<Overlay>) {
    let mut lines = Lines::new();
    let mut zig = Lines::new();
    for s in &pic.shields {
        let at = world(frame, s.at);
        match s.end {
            ShieldEnd::Full => {
                lines.polyline(&geom::ring(at, 12.0), rgba(style.good, 255));
            }
            ShieldEnd::Pigtail(mm) => {
                let colour = style.risk((0.55 + mm / 150.0).min(1.0));
                lines.polyline(&geom::ring(at, 12.0), rgba(colour, 255));
                // The pigtail itself, a short wavy line up from the ring.
                let n = 8;
                let pts: Vec<Vec3> = (0..=n)
                    .map(|i| {
                        let t = i as f32 / n as f32;
                        at + Vec3::new((i % 2) as f32 * 6.0 - 3.0, 12.0 + t * mm.min(120.0), 0.0)
                    })
                    .collect();
                zig.polyline(&pts, rgba(colour, 255));
            }
            ShieldEnd::Open => {
                let colour = style.risk(0.95);
                let ring = geom::ring(at, 12.0);
                for (d, k) in geom::dashes(&ring, 6.0) {
                    if k % 2 == 0 {
                        lines.polyline(&d, rgba(colour, 255));
                    }
                }
            }
        }
    }
    out.extend(lines.finish(3.0, 3));
    out.extend(zig.finish(2.5, 3));
}

/// Everything the layers ask for, in draw order.
pub fn overlays(
    pic: &Picture,
    frame: &PlateFrame,
    style: &Style,
    layers: &EmcLayers,
    selected: Option<u32>,
) -> Vec<Overlay> {
    let mut out = Vec::new();
    if layers.risk {
        out.extend(map_sheet(pic, frame, style));
    }
    if layers.halos {
        out.extend(halos(pic, frame, style));
    }
    if layers.sources {
        out.extend(glows(pic, frame, style));
    }
    if layers.loops {
        loop_overlays(pic, frame, style, &mut out);
    }
    if layers.bonding {
        bonding_overlays(pic, frame, style, &mut out);
    }
    if layers.shields {
        shield_overlays(pic, frame, style, &mut out);
    }
    if layers.spots {
        hotspot_overlays(pic, frame, style, selected, &mut out);
    }
    out
}

/// The scene with wires in their class colour and devices tinted by role.
/// The revision changes with `salt` so the view redraws.
pub fn tint_scene(
    scene: &Scene,
    pic: &Picture,
    style: &Style,
    layers: &EmcLayers,
    salt: u64,
) -> Scene {
    let mut out = scene.clone();
    out.revision = scene
        .revision
        .wrapping_add(salt.wrapping_mul(0x9E37_79B9) | 1);
    if !layers.classes {
        return out;
    }
    for w in &mut out.wires {
        match pic.run(w.id.raw) {
            Some(r) => {
                w.tint = Some(style.colour(r.class));
                w.stripe = if r.class == super::picture::Class::Earth {
                    Some(style.earth.1)
                } else {
                    None
                };
                w.radius = (w.radius * 1.7).max(1.6);
            }
            None => {
                // A wire the engine did not classify fades to grey.
                w.tint = Some([0x6B, 0x6B, 0x6B]);
                w.stripe = None;
            }
        }
    }
    for item in &mut out.items {
        if item.id.kind != ItemKind::Device {
            continue;
        }
        if let Some(n) = pic.node(item.id.raw) {
            match n.role {
                Role::Source => item.tint = Some(style.risk(0.45 + 0.4 * n.level)),
                Role::Victim => item.tint = Some(style.colour(super::picture::Class::Analog)),
                Role::Neutral => {}
            }
        }
    }
    out
}

/// A hot spot's screen label: `H1`, `H2` ...
pub fn label(h: &Hotspot) -> String {
    format!("H{}", h.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emc::{Class, Fix, Kind, Level, Provenance, Run};

    fn style() -> Style {
        Style {
            class: [[200, 100, 20]; 7],
            ramp: |t| [(t * 255.0) as u8, 40, 40],
            earth: ([200, 200, 0], [20, 120, 30]),
            good: [30, 160, 60],
        }
    }

    fn picture() -> Picture {
        let mut pic = Picture::empty([400.0, 500.0], Provenance::Synthetic);
        pic.runs.push(Run {
            id: 1,
            label: "motor".into(),
            class: Class::Motor,
            shielded: false,
            path: vec![[50.0, 50.0, 5.0], [50.0, 300.0, 5.0]],
            halo_mm: 80.0,
            class_reason: String::new(),
            overridden: false,
        });
        pic.hotspots.push(Hotspot {
            id: 1,
            kind: Kind::Parallel,
            level: Level::Index,
            at: [50.0, 150.0, 5.0],
            span: vec![[50.0, 100.0, 5.0], [50.0, 200.0, 5.0]],
            risk: 0.7,
            title: "t".into(),
            why: String::new(),
            numbers: Vec::new(),
            runs: vec![1],
            devices: Vec::new(),
            rule: "EMC-02".into(),
            source: String::new(),
            band: None,
            assumptions: Vec::new(),
            fixes: vec![Fix {
                key: "x".into(),
                label: "x".into(),
                detail: String::new(),
                applicable: false,
            }],
        });
        pic
    }

    fn frame() -> PlateFrame {
        PlateFrame {
            width: 400.0,
            height: 500.0,
        }
    }

    #[test]
    fn the_layers_decide_what_is_drawn() {
        let pic = picture();
        let none = EmcLayers {
            classes: false,
            halos: false,
            sources: false,
            spots: false,
            risk: false,
            loops: false,
            bonding: false,
            shields: false,
        };
        assert!(overlays(&pic, &frame(), &style(), &none, None).is_empty());
        let halos = EmcLayers {
            halos: true,
            ..none
        };
        assert!(!overlays(&pic, &frame(), &style(), &halos, None).is_empty());
    }

    #[test]
    fn every_vertex_is_finite_and_every_mesh_index_in_range() {
        let pic = picture();
        let all = EmcLayers {
            classes: true,
            halos: true,
            sources: true,
            spots: true,
            risk: true,
            loops: true,
            bonding: true,
            shields: true,
        };
        for o in overlays(&pic, &frame(), &style(), &all, Some(1)) {
            match o {
                Overlay::Mesh(m) => {
                    assert_eq!(m.positions.len(), m.colors.len());
                    assert!(m.positions.iter().all(|p| p.is_finite()));
                    assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
                }
                Overlay::Lines(l) => {
                    assert_eq!(l.positions.len(), l.colors.len());
                    assert!(l.positions.iter().all(|p| p.is_finite()));
                }
            }
        }
    }

    #[test]
    fn a_band_ends_at_the_plate_edge() {
        let mut pic = picture();
        // 80 mm either side of a run at x = 50 would reach x = -30.
        pic.runs[0].path = vec![[50.0, 50.0, 5.0], [50.0, 460.0, 5.0]];
        let Some(Overlay::Mesh(m)) = halos(&pic, &frame(), &style()) else {
            panic!("a band is drawn");
        };
        let f = frame();
        for p in &m.positions {
            let (x, y) = f.plate(*p);
            assert!(
                (0.0..=400.0).contains(&x) && (0.0..=500.0).contains(&y),
                "{x} {y}"
            );
        }
        // The part on the plate is still there.
        assert!(m.positions.iter().any(|p| f.plate(*p).0 > 100.0));
    }

    #[test]
    fn a_hot_spot_is_labelled_by_its_number() {
        assert_eq!(label(&picture().hotspots[0]), "H1");
    }
}
