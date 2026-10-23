//! A short cue when holes appear on the mounting plate: a placed device or
//! rail drills its screw holes, and each new hole is marked for under a
//! second: a target ring closes on the centre, the bore opens to its true
//! diameter, then the mark fades. It is information (these holes are the ones
//! your edit made, at their real size and place), not decoration, and it is
//! off with Reduce motion and under the test harness (see `motion`).
//!
//! The frame maths is pure; the painter only strokes what it is given.

use std::collections::HashSet;

use eframe::egui;
use novo_canvas::Viewport;
use novo_core::Project;
use novo_layout::drill::{self, Surface};

/// Seconds a cue lasts.
pub(super) const SECONDS: f64 = 0.9;
/// Age a held cue (`KOPERCAD_MOTION=hold`, tests only) is drawn at.
const HOLD_AGE: f64 = 0.5;
/// Most cues shown at once: a whole-plate import would otherwise flash.
pub(super) const MAX_CUES: usize = 24;

/// One hole by its place and size, to a tenth of a millimetre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct HoleKey(i64, i64, i64);

impl HoleKey {
    pub(super) fn of(x: f64, y: f64, dia: f64) -> Self {
        let t = |v: f64| (v * 10.0).round() as i64;
        Self(t(x), t(y), t(dia))
    }

    fn xyd(self) -> (f64, f64, f64) {
        (
            self.0 as f64 / 10.0,
            self.1 as f64 / 10.0,
            self.2 as f64 / 10.0,
        )
    }
}

/// A hole being marked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Cue {
    pub key: HoleKey,
    pub start: f64,
}

/// What to draw for a cue at `age` seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Frame {
    /// Target ring radius as a multiple of the hole radius; 0 when gone.
    pub ring: f32,
    /// Bore radius as a share of the real radius, 0..1.
    pub bore: f32,
    /// Overall opacity, 0..1.
    pub alpha: f32,
}

/// The cue at `age`, or `None` once it is over.
pub(super) fn frame_at(age: f64) -> Option<Frame> {
    if !(0.0..SECONDS).contains(&age) {
        return None;
    }
    let ease = |t: f64| 1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3);
    // 0 to 0.3 s: the ring closes from 3x to 1x. 0.2 to 0.5 s: the bore opens.
    let ring = 3.0 - 2.0 * ease(age / 0.3);
    let bore = ease((age - 0.2) / 0.3);
    // Full strength until 0.55 s, then fades out.
    let alpha = if age < 0.55 {
        1.0
    } else {
        1.0 - (age - 0.55) / (SECONDS - 0.55)
    };
    Some(Frame {
        ring: ring as f32,
        bore: bore as f32,
        alpha: alpha.clamp(0.0, 1.0) as f32,
    })
}

#[derive(Default)]
pub(super) struct DrillCue {
    /// The plate and revision `known` was read at.
    read: Option<(u64, u64)>,
    known: HashSet<HoleKey>,
    pub(super) cues: Vec<Cue>,
}

impl DrillCue {
    /// Whether the plate at this revision still has to be read.
    pub(super) fn stale(&self, plate: u64, revision: u64) -> bool {
        self.read != Some((plate, revision))
    }

    /// Notes the plate's holes at this revision. The first read of a plate
    /// (opening a project, changing panel) only learns them; later reads
    /// start a cue for each hole that is new.
    pub(super) fn observe(&mut self, plate: u64, revision: u64, holes: &[HoleKey], now: f64) {
        if self.read == Some((plate, revision)) {
            return;
        }
        let same_plate = self.read.is_some_and(|(p, _)| p == plate);
        let now_set: HashSet<HoleKey> = holes.iter().copied().collect();
        if same_plate {
            for k in holes {
                if !self.known.contains(k) && self.cues.len() < MAX_CUES {
                    self.cues.push(Cue {
                        key: *k,
                        start: now,
                    });
                }
            }
        } else {
            self.cues.clear();
        }
        self.known = now_set;
        self.read = Some((plate, revision));
    }

    /// Forgets finished cues; true while any still runs.
    pub(super) fn running(&mut self, now: f64) -> bool {
        if crate::control::hold_cues() {
            return !self.cues.is_empty();
        }
        self.cues.retain(|c| now - c.start < SECONDS);
        !self.cues.is_empty()
    }

    pub(super) fn paint(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        view: &Viewport,
        now: f64,
        colour: egui::Color32,
    ) {
        for c in &self.cues {
            let age = if crate::control::hold_cues() {
                HOLD_AGE
            } else {
                now - c.start
            };
            let Some(f) = frame_at(age) else {
                continue;
            };
            let (x, y, dia) = c.key.xyd();
            let s = view.world_to_screen(novo_canvas::vec2(x as f32, y as f32));
            let centre = egui::pos2(rect.min.x + s.x, rect.min.y + s.y);
            // The real radius on screen, never smaller than a legible dot.
            let r = (0.5 * dia as f32 * view.zoom).max(2.5);
            let col = |a: f32| colour.gamma_multiply((a * f.alpha).clamp(0.0, 1.0));
            if f.bore > 0.0 {
                painter.circle_filled(centre, r * f.bore, col(0.35));
                painter.circle_stroke(centre, r * f.bore, egui::Stroke::new(1.2, col(0.9)));
            }
            if f.ring > 1.02 {
                painter.circle_stroke(centre, r * f.ring, egui::Stroke::new(1.0, col(0.7)));
            }
        }
    }
}

impl DrillCue {
    /// The cues as lines on the plate of the 3D view: for each hole a ring
    /// that closes and a bore that opens, fading out. The bore is the hole's
    /// true size; only the ring is kept at 6 mm or more so a small hole shows.
    pub(super) fn lines(
        &self,
        frame: &novo_view3d::PlateFrame,
        now: f64,
        rgb: [u8; 3],
    ) -> Option<novo_view3d::OverlayLines> {
        const SEGMENTS: usize = 24;
        // Clears a DIN rail (7.5 mm), so a screw under a rail still shows.
        const LIFT: f64 = 9.0;
        let mut out = novo_view3d::OverlayLines {
            width_px: 2.0,
            order: 60,
            ..Default::default()
        };
        let mut circle = |centre: (f64, f64), r: f64, alpha: f32| {
            let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
            if a < 6 {
                return;
            }
            out.starts.push(out.positions.len() as u32);
            for i in 0..=SEGMENTS {
                let t = i as f64 / SEGMENTS as f64 * std::f64::consts::TAU;
                out.positions.push(frame.world(
                    centre.0 + r * t.cos(),
                    centre.1 + r * t.sin(),
                    LIFT,
                ));
                out.colors.push([rgb[0], rgb[1], rgb[2], a]);
            }
        };
        for c in &self.cues {
            let age = if crate::control::hold_cues() {
                HOLD_AGE
            } else {
                now - c.start
            };
            let Some(f) = frame_at(age) else { continue };
            let (x, y, dia) = c.key.xyd();
            // The bore is the hole's true radius; only the closing ring has a
            // legibility floor, so a 4 mm hole still shows from across the cabinet.
            let bore_r = (0.5 * dia).max(0.5);
            let ring_r = bore_r.max(6.0);
            if f.bore > 0.0 {
                circle((x, y), bore_r * f64::from(f.bore), 0.9 * f.alpha);
            }
            if f.ring > 1.02 {
                circle((x, y), ring_r * f64::from(f.ring), 0.7 * f.alpha);
            }
        }
        (!out.positions.is_empty()).then_some(out)
    }
}

/// The holes on the mounting plate of `structure`: rail screws and the
/// screw holes of the placed devices.
pub(super) fn plate_holes(project: &Project, structure: novo_core::StructureId) -> Vec<HoleKey> {
    let Some(layout) = novo_layout::model::load(project, structure) else {
        return Vec::new();
    };
    let features: Vec<_> = layout
        .placements
        .iter()
        .filter_map(|p| project.get::<novo_core::Device>(novo_core::DeviceId::from(p.id)))
        .map(|d| novo_layout::model::features(project, d))
        .collect();
    drill::drill_table(&layout, &features)
        .holes
        .iter()
        .filter(|h| h.surface == Surface::Plate)
        .map(|h| HoleKey::of(h.x, h.y, h.dia))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(n: i64) -> Vec<HoleKey> {
        (0..n)
            .map(|i| HoleKey::of(i as f64 * 10.0, 5.0, 4.0))
            .collect()
    }

    #[test]
    fn the_first_read_of_a_plate_only_learns_its_holes() {
        let mut d = DrillCue::default();
        d.observe(1, 1, &keys(4), 0.0);
        assert!(d.cues.is_empty());
        assert!(!d.running(0.1));
    }

    #[test]
    fn only_new_holes_get_a_cue_and_a_second_read_is_free() {
        let mut d = DrillCue::default();
        d.observe(1, 1, &keys(2), 0.0);
        d.observe(1, 2, &keys(4), 5.0);
        assert_eq!(d.cues.len(), 2);
        d.observe(1, 2, &keys(6), 5.1);
        assert_eq!(d.cues.len(), 2, "same revision: nothing read again");
        assert!(d.running(5.5));
        assert!(!d.running(7.0));
    }

    #[test]
    fn another_plate_does_not_flash_its_holes() {
        let mut d = DrillCue::default();
        d.observe(1, 1, &keys(2), 0.0);
        d.observe(2, 2, &keys(8), 1.0);
        assert!(d.cues.is_empty());
    }

    #[test]
    fn a_big_import_is_capped() {
        let mut d = DrillCue::default();
        d.observe(1, 1, &[], 0.0);
        d.observe(1, 2, &keys(200), 1.0);
        assert_eq!(d.cues.len(), MAX_CUES);
    }

    #[test]
    fn the_frame_closes_opens_and_fades() {
        let a = frame_at(0.0).unwrap();
        assert_eq!((a.ring, a.bore, a.alpha), (3.0, 0.0, 1.0));
        let b = frame_at(0.5).unwrap();
        assert!((b.ring - 1.0).abs() < 1e-3 && (b.bore - 1.0).abs() < 1e-3);
        let c = frame_at(0.8).unwrap();
        assert!(c.alpha < 1.0 && c.alpha > 0.0);
        assert!(frame_at(SECONDS).is_none() && frame_at(-0.1).is_none());
    }

    #[test]
    fn the_3d_bore_keeps_the_true_hole_size_and_only_the_ring_is_enlarged() {
        let frame = novo_view3d::PlateFrame {
            width: 800.0,
            height: 600.0,
        };
        let mut d = DrillCue::default();
        d.observe(1, 1, &keys(1), 0.0);
        d.observe(1, 2, &keys(2), 5.0);
        let (x, y, _) = d.cues[0].key.xyd();
        let l = d.lines(&frame, 5.0 + HOLD_AGE, [1, 2, 3]).expect("cue");
        let c = frame.world(x, y, 0.0);
        let dist = |p: &glam::Vec3| ((p.x - c.x).powi(2) + (p.y - c.y).powi(2)).sqrt();
        // The first polyline is the bore: a 4 mm hole stays within 2 mm.
        let first = l.starts.get(1).map_or(l.positions.len(), |s| *s as usize);
        assert!(l.positions[..first].iter().all(|p| dist(p) <= 2.0 + 1e-3));
    }

    #[test]
    fn plate_lines_sit_above_a_rail_and_are_empty_when_nothing_runs() {
        let frame = novo_view3d::PlateFrame {
            width: 800.0,
            height: 600.0,
        };
        let mut d = DrillCue::default();
        assert!(d.lines(&frame, 0.0, [1, 2, 3]).is_none());
        d.observe(1, 1, &keys(1), 0.0);
        d.observe(1, 2, &keys(3), 5.0);
        let l = d.lines(&frame, 5.3, [1, 2, 3]).expect("cue running");
        assert!(l.positions.iter().all(|p| p.z > 7.5));
        assert!(d.lines(&frame, 9.0, [1, 2, 3]).is_none());
    }
}
