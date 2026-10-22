//! The grilles, fans and filter fans of the Heat panel as geometry in the 3D
//! view: a coloured frame with slats or a rotor on the wall where the opening
//! is, an arrow through it for the way the air goes, and a label with its
//! size and flow. Pure geometry from the settings and the enclosure (see
//! `heat_settings`), so they show before any run; a finished result adds the
//! flow measured at each opening, from the solved air speed next to it.
//!
//! The solver's frame is the interior of the cabinet, y down and z from the
//! plate to the door; [`Place`] maps it into the scene.

use eframe::egui;
use glam::Vec3;
use novo_cfd::CfdSnapshot;
use novo_heatview::cfd::{self, Axis};
use novo_view3d::widget::View3d;
use novo_view3d::{Overlay, OverlayLines, OverlayMesh, Scene};

use super::heat_3d::Place;
use super::heat_settings::{Footprint, Kind, VentSpec, Wall};

/// Colours: air in, air out, not known yet. The label always says it in words
/// too.
pub(super) const IN_RGB: [u8; 3] = [70, 170, 255];
pub(super) const OUT_RGB: [u8; 3] = [255, 140, 50];
pub(super) const OPEN_RGB: [u8; 3] = [185, 195, 215];

/// How far the drawing floats off the wall, mm, so it is not lost in it.
const LIFT_MM: f32 = 1.2;
/// Less air than this through an opening counts as none, m3/h.
const STILL_M3H: f64 = 0.5;
/// Draw order: above the slice and the shell.
const MESH_ORDER: i32 = 8;
const LINE_ORDER: i32 = 9;

/// Which way the air goes through an opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    In,
    Out,
    /// A grille before a run: open either way.
    Open,
}

impl Role {
    pub(super) fn rgb(self) -> [u8; 3] {
        match self {
            Role::In => IN_RGB,
            Role::Out => OUT_RGB,
            Role::Open => OPEN_RGB,
        }
    }
}

/// One opening ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Marker {
    pub index: usize,
    pub spec: VentSpec,
    pub role: Role,
    /// Air through it, m3/h, positive into the cabinet: measured from a
    /// result, or `None` before one.
    pub measured_m3h: Option<f64>,
    pub selected: bool,
    pub centre: Vec3,
    /// Unit vectors along the width and the other way, on the wall.
    pub along: Vec3,
    pub across: Vec3,
    /// Half the width and the other side, mm, as drawn.
    pub half: [f32; 2],
    /// Out of the cabinet.
    pub outward: Vec3,
    /// The whole wall it sits in, in the scene (corners in order).
    pub wall: [Vec3; 4],
}

/// The direction of a solver-frame vector in the scene (y flips).
fn to_world(v: [f64; 3]) -> Vec3 {
    Vec3::new(v[0] as f32, -v[1] as f32, v[2] as f32)
}

/// The role of an opening: a fan is as set, a grille is as measured.
pub(super) fn role_of(spec: &VentSpec, measured_m3h: Option<f64>) -> Role {
    match (spec.set_direction(), measured_m3h) {
        (Some(true), _) => Role::In,
        (Some(false), _) => Role::Out,
        (None, Some(q)) if q > STILL_M3H => Role::In,
        (None, Some(q)) if q < -STILL_M3H => Role::Out,
        (None, _) => Role::Open,
    }
}

/// The markers of `specs` in `e`, placed in the scene. `flows` is the air
/// measured through each opening (same order), empty without a result.
pub(super) fn markers(
    specs: &[VentSpec],
    e: &novo_thermal::model::Enclosure,
    place: Place,
    flows: &[Option<f64>],
    selected: Option<usize>,
) -> Vec<Marker> {
    specs
        .iter()
        .enumerate()
        .map(|(index, spec)| {
            let fit = spec.fitted(e);
            let f = fit.footprint(e);
            let varying: Vec<usize> = (0..3).filter(|a| f.hi[*a] - f.lo[*a] > 1e-9).collect();
            let axis = |i: usize| {
                let mut d = [0.0; 3];
                d[varying[i]] = 1.0;
                to_world(d)
            };
            let (along, across) = (axis(0), axis(1));
            let half = [
                0.5 * (f.hi[varying[0]] - f.lo[varying[0]]) as f32,
                0.5 * (f.hi[varying[1]] - f.lo[varying[1]]) as f32,
            ];
            let measured = flows.get(index).copied().flatten();
            Marker {
                index,
                spec: fit,
                role: role_of(&fit, measured),
                measured_m3h: measured,
                selected: selected == Some(index),
                centre: place.world(f.centre()),
                along,
                across,
                half,
                outward: -to_world(f.inward()),
                wall: wall_corners(fit.wall, e).map(|p| place.world(p)),
            }
        })
        .collect()
}

/// The corners of a whole wall in the solver frame.
fn wall_corners(wall: Wall, e: &novo_thermal::model::Enclosure) -> [[f64; 3]; 4] {
    let (w, h, d) = (
        e.interior_width_mm(),
        e.interior_height_mm(),
        e.interior_depth_mm(),
    );
    match wall {
        Wall::Top => [[0.0, 0.0, 0.0], [w, 0.0, 0.0], [w, 0.0, d], [0.0, 0.0, d]],
        Wall::Bottom => [[0.0, h, 0.0], [w, h, 0.0], [w, h, d], [0.0, h, d]],
        Wall::Left => [[0.0, 0.0, 0.0], [0.0, h, 0.0], [0.0, h, d], [0.0, 0.0, d]],
        Wall::Right => [[w, 0.0, 0.0], [w, h, 0.0], [w, h, d], [w, 0.0, d]],
        Wall::Door => [[0.0, 0.0, d], [w, 0.0, d], [w, h, d], [0.0, h, d]],
    }
}

impl Marker {
    fn corners(&self, inset: f32) -> [Vec3; 4] {
        let (a, b) = (
            self.along * (self.half[0] - inset).max(1.0),
            self.across * (self.half[1] - inset).max(1.0),
        );
        let c = self.centre + self.outward * LIFT_MM;
        [c - a - b, c + a - b, c + a + b, c - a + b]
    }

    /// How long its arrow is, mm.
    fn arrow_len(&self) -> f32 {
        (1.2 * self.half[0].min(self.half[1])).clamp(45.0, 120.0)
    }

    fn colour(&self, alpha: u8) -> [u8; 4] {
        let c = self.role.rgb();
        [c[0], c[1], c[2], alpha]
    }
}

fn lines(width_px: f32) -> OverlayLines {
    OverlayLines {
        width_px,
        order: LINE_ORDER,
        ..Default::default()
    }
}

fn polyline(out: &mut OverlayLines, points: &[Vec3], colour: [u8; 4]) {
    out.starts.push(out.positions.len() as u32);
    for p in points {
        out.positions.push(*p);
        out.colors.push(colour);
    }
}

/// All of the markers as overlays: a translucent fill, the frames, the slats
/// or rotors and the arrows.
pub(super) fn overlays(markers: &[Marker]) -> Vec<Overlay> {
    let mut fill = OverlayMesh {
        translucent: true,
        order: MESH_ORDER,
        ..Default::default()
    };
    let mut frame = lines(3.2);
    let mut frame_selected = lines(5.5);
    let mut detail = lines(1.6);
    let mut arrows = lines(3.6);
    for m in markers {
        if m.selected {
            // The wall it is in, tinted, so "left wall" is not a guess.
            let base = fill.positions.len() as u32;
            fill.positions
                .extend(m.wall.map(|p| p + m.outward * (LIFT_MM * 0.5)));
            fill.colors.extend([m.colour(38); 4]);
            fill.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let alpha = if m.selected { 150 } else { 85 };
        let c = m.corners(0.0);
        let base = fill.positions.len() as u32;
        fill.positions.extend(c);
        fill.colors.extend([m.colour(alpha); 4]);
        fill.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);

        let target = if m.selected {
            &mut frame_selected
        } else {
            &mut frame
        };
        let solid = m.colour(255);
        polyline(target, &[c[0], c[1], c[2], c[3], c[0]], solid);
        if m.selected {
            // A white keyline inside a selected frame stands out on any wall.
            let w = m.corners(4.0);
            polyline(
                &mut frame,
                &[w[0], w[1], w[2], w[3], w[0]],
                [255, 255, 255, 235],
            );
        }
        let faint = m.colour(210);
        match m.spec.kind {
            Kind::Grille => slats(m, &mut detail, faint),
            Kind::Fan => rotor(m, &mut detail, faint),
            Kind::FilterFan => {
                rotor(m, &mut detail, faint);
                // The filter frame around the fan.
                let w = m.corners(5.0);
                polyline(&mut detail, &[w[0], w[1], w[2], w[3], w[0]], faint);
            }
        }
        arrow(m, &mut arrows);
    }
    let mut out = Vec::new();
    if !fill.indices.is_empty() {
        out.push(Overlay::Mesh(fill));
    }
    for l in [detail, frame, frame_selected, arrows] {
        if !l.positions.is_empty() {
            out.push(Overlay::Lines(l));
        }
    }
    out
}

/// Horizontal louvres across the opening.
fn slats(m: &Marker, out: &mut OverlayLines, colour: [u8; 4]) {
    let n = ((2.0 * m.half[1] / 14.0) as usize).clamp(2, 12);
    let c = m.centre + m.outward * LIFT_MM;
    let (a, b) = (m.along * (m.half[0] - 4.0).max(1.0), m.across);
    for i in 0..n {
        let t = (i as f32 + 0.5) / n as f32 * 2.0 - 1.0;
        let off = b * (t * (m.half[1] - 4.0).max(1.0));
        polyline(out, &[c - a + off, c + a + off], colour);
    }
}

/// A ring and three blades.
fn rotor(m: &Marker, out: &mut OverlayLines, colour: [u8; 4]) {
    const SEGMENTS: usize = 28;
    let short = m.half[0].min(m.half[1]);
    let r = (0.9 * short).min(short - 3.0).max(2.0);
    let c = m.centre + m.outward * LIFT_MM;
    let at = |ang: f32, rr: f32| c + m.along * (rr * ang.cos()) + m.across * (rr * ang.sin());
    let ring: Vec<Vec3> = (0..=SEGMENTS)
        .map(|i| at(i as f32 / SEGMENTS as f32 * std::f32::consts::TAU, r))
        .collect();
    polyline(out, &ring, colour);
    for k in 0..3 {
        let ang = k as f32 / 3.0 * std::f32::consts::TAU;
        // A blade curls a little, so the rotor reads as a rotor.
        let mid = at(ang + 0.35, 0.55 * r);
        polyline(out, &[c, mid, at(ang + 0.7, r)], colour);
    }
}

/// A shaft through the opening with a head that shows the way the air goes.
/// An opening not known to be either way gets none.
fn arrow(m: &Marker, out: &mut OverlayLines) {
    let len = m.arrow_len();
    let (start, end) = match m.role {
        Role::In => (m.centre + m.outward * len, m.centre - m.outward * 0.6 * len),
        Role::Out => (m.centre - m.outward * 0.6 * len, m.centre + m.outward * len),
        Role::Open => return,
    };
    let colour = m.colour(255);
    polyline(out, &[start, end], colour);
    let back = (start - end).normalize_or_zero() * (0.32 * len);
    let barb = 0.2 * len;
    for side in [m.along, -m.along, m.across, -m.across] {
        polyline(out, &[end + back + side * barb, end], colour);
    }
}

/// Where the label of a marker hangs: past the tip of the arrow, or just
/// outside the wall when it has none.
pub(super) fn label_anchor(m: &Marker) -> Vec3 {
    let reach = match m.role {
        Role::Open => 30.0,
        _ => m.arrow_len() + 14.0,
    };
    m.centre + m.outward * reach
}

/// What the label says, in two lines.
pub(super) fn label_lines(m: &Marker) -> [String; 2] {
    let what = match (m.spec.kind, m.role) {
        (Kind::Grille, Role::In) => "Grille: air in",
        (Kind::Grille, Role::Out) => "Grille: air out",
        (Kind::Grille, Role::Open) => "Grille",
        (Kind::Fan, Role::In) => "Fan: blows in",
        (Kind::Fan, Role::Out) => "Fan: blows out",
        (Kind::FilterFan, Role::In) => "Filter fan: blows in",
        (Kind::FilterFan, Role::Out) => "Filter fan: blows out",
        (_, Role::Open) => "Opening",
    };
    let size = format!("{:.0} × {:.0} mm", m.spec.width_mm, m.spec.height_mm);
    let flow = match (m.measured_m3h, m.spec.kind.is_fan()) {
        (Some(q), _) => format!("  ·  {:.0} m³/h (model)", q.abs()),
        (None, true) => format!("  ·  up to {:.0} m³/h", m.spec.flow_m3h),
        (None, false) => String::new(),
    };
    [format!("{}  {what}", m.index + 1), format!("{size}{flow}")]
}

/// What the person did with the labels this frame.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) struct LabelAction {
    /// The label of this opening was clicked.
    pub clicked: Option<usize>,
    /// This opening's label is being dragged.
    pub drag: Option<DragPhase>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DragPhase {
    pub index: usize,
    pub started: bool,
    pub stopped: bool,
}

/// Paints the labels over the 3D view and reports a click on one, or a drag
/// of one (which slides its opening along the wall).
pub(super) fn paint_labels(
    ui: &egui::Ui,
    view: &View3d,
    rect: egui::Rect,
    scene: &Scene,
    markers: &[Marker],
) -> LabelAction {
    let pal = crate::theme::ui_palette();
    let painter = ui.painter().with_clip_rect(rect);
    let bold = egui::FontId::proportional(12.5);
    let small = egui::FontId::proportional(11.0);
    let mut placed: Vec<egui::Rect> = Vec::new();
    let mut action = LabelAction::default();
    for m in markers {
        let Some(at) = view.project(rect, scene, label_anchor(m)) else {
            continue;
        };
        let [l1, l2] = label_lines(m);
        let g1 = painter.layout_no_wrap(l1, bold.clone(), pal.text_primary);
        let g2 = painter.layout_no_wrap(l2, small.clone(), pal.text_secondary);
        let size = egui::vec2(
            g1.size().x.max(g2.size().x) + 14.0,
            g1.size().y + g2.size().y + 8.0,
        );
        let mut chip = egui::Rect::from_center_size(at, size);
        // Keep the chip inside the view and off the others.
        chip = chip.translate(egui::vec2(
            (rect.min.x + 4.0 - chip.min.x).max(0.0) + (rect.max.x - 4.0 - chip.max.x).min(0.0),
            (rect.min.y + 4.0 - chip.min.y).max(0.0) + (rect.max.y - 4.0 - chip.max.y).min(0.0),
        ));
        while placed.iter().any(|p| p.intersects(chip)) && chip.max.y + size.y < rect.max.y {
            chip = chip.translate(egui::vec2(0.0, size.y + 3.0));
        }
        placed.push(chip);
        let c = m.role.rgb();
        let colour = egui::Color32::from_rgb(c[0], c[1], c[2]);
        let response = ui.interact(
            chip,
            egui::Id::new(("heat_opening_label", m.index)),
            egui::Sense::click_and_drag(),
        );
        if response.clicked() {
            action.clicked = Some(m.index);
        }
        if response.drag_started() || response.dragged() || response.drag_stopped() {
            action.drag = Some(DragPhase {
                index: m.index,
                started: response.drag_started(),
                stopped: response.drag_stopped(),
            });
        }
        let stroke = if m.selected || response.hovered() {
            2.5
        } else {
            1.5
        };
        painter.rect(
            chip,
            5.0,
            pal.raised.gamma_multiply(0.94),
            egui::Stroke::new(stroke, colour),
            egui::StrokeKind::Inside,
        );
        painter.galley(
            chip.min + egui::vec2(7.0, 3.0),
            g1.clone(),
            pal.text_primary,
        );
        painter.galley(
            chip.min + egui::vec2(7.0, 3.0 + g1.size().y),
            g2,
            pal.text_secondary,
        );
        response.on_hover_text(
            "Click to open it in the Vents and fans panel; drag to slide it along its wall",
        );
    }
    action
}

/// Where the opening goes when the pointer has moved from `start` to `now` on
/// its wall (scene points, mm): it moves with the pointer, snaps to 5 mm and
/// to the middle of the wall, and is kept inside the wall by `fitted`.
pub(super) fn dragged_to(
    from: &VentSpec,
    start: Vec3,
    now: Vec3,
    e: &novo_thermal::model::Enclosure,
) -> VentSpec {
    let d = now - start;
    // Scene y runs up, the solver's down.
    let (dx, dy) = (f64::from(d.x), -f64::from(d.y));
    let snap = |v: f64| (v / 5.0).round() * 5.0;
    let mut out = *from;
    let (w, h) = (e.interior_width_mm(), e.interior_height_mm());
    let mid = |v: f64, middle: f64| if (v - middle).abs() < 8.0 { middle } else { v };
    match from.wall {
        Wall::Top | Wall::Bottom => out.u_mm = mid(snap(from.u_mm + dx), 0.5 * w),
        Wall::Left | Wall::Right => out.u_mm = mid(snap(from.u_mm + dy), 0.5 * h),
        Wall::Door => {
            out.u_mm = mid(snap(from.u_mm + dx), 0.5 * w);
            out.v_mm = snap(from.v_mm + dy);
        }
    }
    out.fitted(e)
}

/// The air through the opening in m3/h, positive into the cabinet, from the
/// solved speed one cell inside the face. `None` when the result is not
/// usable. A model value: it follows the solver's openings, not a
/// measurement.
pub(super) fn measured_m3h(snap: &CfdSnapshot, f: &Footprint) -> Option<f64> {
    if !snap.is_consistent() {
        return None;
    }
    let inward = f.inward();
    let normal = (0..3).find(|a| inward[*a] != 0.0)?;
    let axis = [Axis::X, Axis::Y, Axis::Z][normal];
    let edges = super::heat_3d::slice_edges(snap, axis);
    let (first, last) = (edges[0], edges[edges.len() - 1]);
    let (cell, face) = if inward[normal] > 0.0 {
        (edges[1] - edges[0], first)
    } else {
        (edges[edges.len() - 1] - edges[edges.len() - 2], last)
    };
    let depth = face + inward[normal] * 0.5 * cell;
    let varying: Vec<usize> = (0..3).filter(|a| f.hi[*a] - f.lo[*a] > 1e-9).collect();
    if varying.len() != 2 {
        return None;
    }
    const N: usize = 6;
    let mut sum = 0.0;
    for i in 0..N {
        for j in 0..N {
            let mut p = [0.0; 3];
            p[normal] = depth;
            for (k, a) in varying.iter().enumerate() {
                let t = ([i, j][k] as f64 + 0.5) / N as f64;
                p[*a] = f.lo[*a] + t * (f.hi[*a] - f.lo[*a]);
            }
            let v = cfd::velocity_at(snap, p);
            sum += f64::from(v[normal]) * inward[normal];
        }
    }
    let area_m2 =
        (f.hi[varying[0]] - f.lo[varying[0]]) * (f.hi[varying[1]] - f.lo[varying[1]]) / 1.0e6;
    Some(sum / (N * N) as f64 * area_m2 * 3600.0)
}

/// Seeds for streamlines that start at the air entering: a small lattice just
/// inside each opening that takes air in (a fan blowing in, or a grille the
/// result shows air entering), in the solver frame.
pub(super) fn inlet_seeds(
    markers: &[Marker],
    specs: &[VentSpec],
    e: &novo_thermal::model::Enclosure,
) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for m in markers.iter().filter(|m| m.role == Role::In) {
        let Some(spec) = specs.get(m.index) else {
            continue;
        };
        let f = spec.footprint(e);
        let inward = f.inward();
        let varying: Vec<usize> = (0..3).filter(|a| f.hi[*a] - f.lo[*a] > 1e-9).collect();
        if varying.len() != 2 {
            continue;
        }
        const N: usize = 3;
        for i in 0..N {
            for j in 0..N {
                let mut p = f.centre();
                for (k, a) in varying.iter().enumerate() {
                    let t = ([i, j][k] as f64 + 0.5) / N as f64;
                    p[*a] = f.lo[*a] + t * (f.hi[*a] - f.lo[*a]);
                }
                for a in 0..3 {
                    p[a] += inward[a] * 12.0;
                }
                out.push(p);
            }
        }
    }
    out
}

/// The wall as a word, for the panel and the hover.
pub(super) fn wall_phrase(w: Wall) -> &'static str {
    match w {
        Wall::Top => "in the top",
        Wall::Bottom => "in the bottom",
        Wall::Left => "in the left wall",
        Wall::Right => "in the right wall",
        Wall::Door => "in the door",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::Rect;
    use novo_layout::sizing::{Installation, Material};
    use novo_thermal::model::Enclosure;
    use novo_view3d::PlateFrame;

    fn enclosure() -> Enclosure {
        Enclosure {
            width_mm: 800.0,
            height_mm: 1000.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 20.0,
                y: 30.0,
                w: 700.0,
                h: 900.0,
            },
        }
    }

    fn place() -> Place {
        Place {
            plate_x: 20.0,
            plate_y: 30.0,
            frame: PlateFrame {
                width: 700.0,
                height: 900.0,
            },
        }
    }

    fn marker(spec: VentSpec) -> Marker {
        markers(&[spec], &enclosure(), place(), &[], None).remove(0)
    }

    #[test]
    fn a_left_wall_opening_sits_on_the_left_wall_facing_out_of_the_cabinet() {
        let mut s = VentSpec::new(Wall::Left);
        s.u_mm = 500.0;
        let m = marker(s);
        // Interior x 0 is 20 mm left of the plate origin: world x = -20.
        assert!((m.centre.x + 20.0).abs() < 1e-4, "{:?}", m.centre);
        assert_eq!(m.outward, Vec3::new(-1.0, 0.0, 0.0));
        // Height: solver y 500 is world 900 - (500 - 30).
        assert!((m.centre.y - 430.0).abs() < 1e-3, "{:?}", m.centre);
        assert_eq!(m.half, [75.0, 50.0]);
    }

    #[test]
    fn a_door_opening_faces_the_front_and_a_top_opening_faces_up() {
        let mut d = VentSpec::new(Wall::Door);
        d.u_mm = 400.0;
        d.v_mm = 200.0;
        let m = marker(d);
        assert_eq!(m.outward, Vec3::new(0.0, 0.0, 1.0));
        assert!((m.centre.z - 296.0).abs() < 1e-4);
        let mut t = VentSpec::new(Wall::Top);
        t.u_mm = 400.0;
        let m = marker(t);
        assert_eq!(m.outward, Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn the_drawing_never_leaves_the_wall_it_is_on() {
        let mut s = VentSpec::new(Wall::Right);
        s.u_mm = -400.0;
        s.width_mm = 9000.0;
        let m = marker(s);
        for c in m.corners(0.0) {
            // The right wall is at solver x 796, plus the lift.
            assert!((c.x - (796.0 - 20.0 + LIFT_MM)).abs() < 1e-3);
            // Solver y 0 to 996 is world 930 down to -66.
            assert!(c.y >= -66.0 - 1e-3 && c.y <= 930.0 + 1e-3, "{c:?}");
        }
    }

    #[test]
    fn a_fan_has_its_direction_from_the_setting_and_a_grille_from_the_flow() {
        let mut fan = VentSpec::new(Wall::Door);
        fan.kind = Kind::FilterFan;
        assert_eq!(role_of(&fan, None), Role::In);
        assert_eq!(role_of(&fan, Some(-50.0)), Role::In, "the setting wins");
        fan.exhaust = true;
        assert_eq!(role_of(&fan, None), Role::Out);
        let g = VentSpec::new(Wall::Left);
        assert_eq!(role_of(&g, None), Role::Open);
        assert_eq!(role_of(&g, Some(12.0)), Role::In);
        assert_eq!(role_of(&g, Some(-12.0)), Role::Out);
        assert_eq!(role_of(&g, Some(0.2)), Role::Open);
    }

    #[test]
    fn each_kind_draws_its_own_detail_and_a_known_direction_draws_an_arrow() {
        let mut g = VentSpec::new(Wall::Left);
        g.u_mm = 500.0;
        let mut f = g;
        f.kind = Kind::FilterFan;
        let count = |m: Marker| -> (usize, usize) {
            let o = overlays(&[m]);
            let lines: Vec<&OverlayLines> = o
                .iter()
                .filter_map(|o| match o {
                    Overlay::Lines(l) => Some(l),
                    _ => None,
                })
                .collect();
            (o.len(), lines.iter().map(|l| l.starts.len()).sum::<usize>())
        };
        let (_, grille_lines) = count(marker(g));
        let (_, fan_lines) = count(marker(f));
        // The fan has a ring, three blades, a filter frame and an arrow (5
        // polylines): more than a grille with no known direction.
        assert!(fan_lines > grille_lines, "{fan_lines} {grille_lines}");
        for o in overlays(&[marker(f)]) {
            match o {
                Overlay::Mesh(m) => assert!(m.is_valid()),
                Overlay::Lines(l) => assert!(l.is_valid()),
            }
        }
    }

    #[test]
    fn an_arrow_into_the_cabinet_ends_inside_it_and_one_out_ends_outside() {
        let mut fan = VentSpec::new(Wall::Left);
        fan.kind = Kind::Fan;
        fan.u_mm = 500.0;
        let into = marker(fan);
        let arrows = |m: &Marker| {
            overlays(std::slice::from_ref(m))
                .into_iter()
                .filter_map(|o| match o {
                    Overlay::Lines(l) if l.width_px == 3.6 => Some(l),
                    _ => None,
                })
                .next()
                .unwrap()
        };
        let a = arrows(&into);
        // First polyline is the shaft: from outside (x < wall) to inside.
        assert!(a.positions[0].x < into.centre.x);
        assert!(a.positions[1].x > into.centre.x);
        fan.exhaust = true;
        let out = marker(fan);
        let a = arrows(&out);
        assert!(a.positions[0].x > out.centre.x);
        assert!(a.positions[1].x < out.centre.x);
    }

    #[test]
    fn dragging_slides_the_opening_along_its_wall_snapped_and_inside_it() {
        let e = enclosure();
        let mut left = VentSpec::new(Wall::Left);
        left.u_mm = 300.0;
        // Pointer 52 mm up in the scene = 52 mm up the wall = u down by 52.
        let up = dragged_to(&left, Vec3::ZERO, Vec3::new(0.0, 52.0, 0.0), &e);
        assert_eq!(up.u_mm, 250.0, "300 - 52 snapped to 5 mm");
        // Along the wall only: sideways pointer movement changes nothing.
        let side = dragged_to(&left, Vec3::ZERO, Vec3::new(30.0, 0.0, 40.0), &e);
        assert_eq!(side.u_mm, 300.0);
        // Far past the end: kept inside the wall.
        let far = dragged_to(&left, Vec3::ZERO, Vec3::new(0.0, -9000.0, 0.0), &e);
        assert_eq!(far.u_mm, 996.0 - 50.0 - 25.0);
        // Near the middle it snaps to the middle.
        let mut top = VentSpec::new(Wall::Top);
        top.u_mm = 100.0;
        let mid = dragged_to(&top, Vec3::ZERO, Vec3::new(295.0, 0.0, 0.0), &e);
        assert_eq!(mid.u_mm, 398.0);
        // The door moves both ways.
        let mut door = VentSpec::new(Wall::Door);
        door.u_mm = 200.0;
        door.v_mm = 200.0;
        let m = dragged_to(&door, Vec3::ZERO, Vec3::new(100.0, -50.0, 0.0), &e);
        assert_eq!((m.u_mm, m.v_mm), (300.0, 250.0));
    }

    #[test]
    fn the_selected_opening_tints_its_whole_wall() {
        let e = enclosure();
        let spec = VentSpec::presets(&e)[0].spec;
        let mesh_len = |sel| {
            let ms = markers(&[spec], &e, place(), &[], sel);
            match &overlays(&ms)[0] {
                Overlay::Mesh(m) => m.indices.len(),
                _ => panic!(),
            }
        };
        assert_eq!(mesh_len(Some(0)), mesh_len(None) + 6);
        let ms = markers(&[spec], &e, place(), &[], Some(0));
        // The wall quad lies on the left wall, solver x 0 is scene x -20.
        assert!(ms[0].wall.iter().all(|p| (p.x + 20.0).abs() < 1e-3));
    }

    #[test]
    fn labels_say_what_it_is_its_size_and_its_flow() {
        let mut fan = VentSpec::new(Wall::Door);
        fan.kind = Kind::FilterFan;
        fan.width_mm = 204.0;
        fan.height_mm = 204.0;
        fan.flow_m3h = 230.0;
        let m = marker(fan);
        let [a, b] = label_lines(&m);
        assert_eq!(a, "1  Filter fan: blows in");
        assert_eq!(b, "204 × 204 mm  ·  up to 230 m³/h");
        let mut g = VentSpec::new(Wall::Left);
        g.u_mm = 300.0;
        let mut m = marker(g);
        assert_eq!(label_lines(&m)[0], "1  Grille");
        m.measured_m3h = Some(-37.6);
        m.role = role_of(&m.spec, m.measured_m3h);
        let [a, b] = label_lines(&m);
        assert_eq!(a, "1  Grille: air out");
        assert_eq!(b, "150 × 100 mm  ·  38 m³/h (model)");
    }

    #[test]
    fn markers_follow_the_order_and_selection_of_the_settings() {
        let e = enclosure();
        let specs = [VentSpec::presets(&e)[0].spec, VentSpec::presets(&e)[1].spec];
        let ms = markers(&specs, &e, place(), &[Some(10.0), None], Some(1));
        assert_eq!(ms.len(), 2);
        assert!(!ms[0].selected && ms[1].selected);
        assert_eq!(ms[0].role, Role::In);
        assert_eq!(ms[1].role, Role::Open);
        assert_eq!((ms[0].index, ms[1].index), (0, 1));
    }

    #[test]
    fn streamlines_start_inside_each_air_in_opening_only() {
        let e = enclosure();
        let mut fan = VentSpec::presets(&e)[2].spec;
        fan.exhaust = false;
        let mut out = fan;
        out.exhaust = true;
        out.wall = Wall::Top;
        let specs = [fan, out];
        let ms = markers(&specs, &e, place(), &[], None);
        let seeds = inlet_seeds(&ms, &specs, &e);
        assert_eq!(seeds.len(), 9);
        // In front of the door's inside face, not outside it.
        assert!(
            seeds.iter().all(|p| p[2] < 296.0 && p[2] > 250.0),
            "{seeds:?}"
        );
    }
}
