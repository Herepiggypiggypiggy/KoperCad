//! The Measure tool of the 3D view (View > 3D > Measure, right-click > Measure
//! distance, or Ctrl+K "measure"): click two points on the cabinet and read
//! the straight distance, its X, Y and height parts, and, when both points
//! are on devices, the gap between the two devices and between their centres.
//! It reads the scene only; the project is never touched, so there is nothing
//! to undo and the heat result stays as it is.

use eframe::egui;
use glam::Vec3;
use novo_layout::Rect;
use novo_layout::state::CabinetLayout;
use novo_view3d::scene::{ItemId, ItemKind, PlateFrame};
use novo_view3d::{Overlay, OverlayLines};

/// Margin from the view's corner, in points.
const MARGIN: f32 = 12.0;
/// Half length of the cross that marks a picked point, mm.
const TICK_MM: f32 = 6.0;
const LINE: [u8; 4] = [255, 196, 64, 255];
const LEG: [u8; 4] = [255, 196, 64, 120];

/// A picked point and the item it was on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Pin {
    pub point: Vec3,
    pub item: Option<ItemId>,
}

/// The Measure tool: on or off, and the points picked so far (none, one or
/// two; a click with two set starts over).
#[derive(Debug, Default)]
pub(crate) struct Measure {
    pub(super) on: bool,
    pins: Vec<Pin>,
}

/// What two points and their devices come to, in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Reading {
    /// Straight line between the points.
    pub distance: f64,
    /// Parts along the plate: x to the right, y down, and the height above
    /// the plate (all absolute).
    pub dx: f64,
    pub dy: f64,
    pub dz: f64,
    /// Both points were on devices.
    pub devices: Option<Between>,
}

/// Two devices: the free space between their footprints and their centres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Between {
    /// Edge to edge; 0 when they touch or overlap.
    pub gap: f64,
    pub centres: f64,
}

impl Measure {
    pub(super) fn toggle(&mut self) {
        self.on = !self.on;
        self.pins.clear();
    }

    pub(super) fn clear(&mut self) {
        self.pins.clear();
    }

    pub(super) fn has_pins(&self) -> bool {
        !self.pins.is_empty()
    }

    /// A click on the view: the first and second click set the points, a
    /// third starts a new measurement.
    pub(super) fn click(&mut self, point: Vec3, item: Option<ItemId>) {
        if self.pins.len() >= 2 {
            self.pins.clear();
        }
        self.pins.push(Pin { point, item });
    }

    /// The picked points, rounded to a tenth of a millimetre, to tell when
    /// the overlays need drawing again.
    pub(super) fn key(&self) -> Vec<[i64; 3]> {
        self.pins
            .iter()
            .map(|p| p.point.to_array().map(|v| (v * 10.0).round() as i64))
            .collect()
    }

    /// The reading, once two points are set.
    pub(super) fn reading(
        &self,
        frame: &PlateFrame,
        layout: Option<&CabinetLayout>,
    ) -> Option<Reading> {
        let [a, b] = self.pins.as_slice() else {
            return None;
        };
        let (pa, pb) = (frame.plate(a.point), frame.plate(b.point));
        let (dx, dy, dz) = (
            (pb.0 - pa.0).abs(),
            (pb.1 - pa.1).abs(),
            f64::from((b.point.z - a.point.z).abs()),
        );
        let devices = layout.and_then(|l| {
            let rect = |pin: &Pin| {
                let item = pin.item.filter(|i| i.kind == ItemKind::Device)?;
                l.placements
                    .iter()
                    .find(|p| p.id == item.raw)
                    .map(|p| p.footprint())
            };
            let (ra, rb) = (rect(a)?, rect(b)?);
            if a.item == b.item {
                return None;
            }
            Some(between(&ra, &rb))
        });
        Some(Reading {
            distance: (a.point - b.point).length().into(),
            dx,
            dy,
            dz,
            devices,
        })
    }

    /// The line, the three legs along the axes, and a cross at each point.
    pub(super) fn overlays(&self) -> Vec<Overlay> {
        let mut out = Vec::new();
        let mut cross = OverlayLines {
            width_px: 2.0,
            order: 61,
            ..Default::default()
        };
        for pin in &self.pins {
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                cross.starts.push(cross.positions.len() as u32);
                cross.positions.push(pin.point - axis * TICK_MM);
                cross.positions.push(pin.point + axis * TICK_MM);
                cross.colors.extend([LINE; 2]);
            }
        }
        if let [a, b] = self.pins.as_slice() {
            let (a, b) = (a.point, b.point);
            let legs = [a, Vec3::new(b.x, a.y, a.z), Vec3::new(b.x, b.y, a.z), b];
            out.push(Overlay::Lines(OverlayLines {
                positions: legs.to_vec(),
                colors: vec![LEG; 4],
                starts: Vec::new(),
                width_px: 1.5,
                order: 59,
            }));
            out.push(Overlay::Lines(OverlayLines {
                positions: vec![a, b],
                colors: vec![LINE; 2],
                starts: Vec::new(),
                width_px: 2.5,
                order: 60,
            }));
        }
        if !cross.positions.is_empty() {
            out.push(Overlay::Lines(cross));
        }
        out
    }

    /// The midpoint of the line, where the distance is written.
    pub(super) fn midpoint(&self) -> Option<Vec3> {
        let [a, b] = self.pins.as_slice() else {
            return None;
        };
        Some((a.point + b.point) * 0.5)
    }
}

/// Edge to edge and centre to centre of two footprints.
fn between(a: &Rect, b: &Rect) -> Between {
    let c = |r: &Rect| (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let (ca, cb) = (c(a), c(b));
    Between {
        gap: a.gap(b),
        centres: (ca.0 - cb.0).hypot(ca.1 - cb.1),
    }
}

/// `1234.5` as `1234.5 mm`; a metre and more as `1.235 m` is left to the
/// reader: cabinets are read in millimetres.
pub(super) fn mm(v: f64) -> String {
    format!("{v:.1} mm")
}

/// The readout panel, bottom left of the view. Returns true when Clear was
/// pressed.
pub(super) fn readout(
    ctx: &egui::Context,
    view: egui::Rect,
    m: &Measure,
    r: Option<Reading>,
) -> bool {
    let mut clear = false;
    egui::Area::new(egui::Id::new("view3d_measure"))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::LEFT_BOTTOM)
        .fixed_pos(view.left_bottom() + egui::vec2(MARGIN, -MARGIN))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.strong("Measure");
                match (m.pins.len(), r) {
                    (0, _) => {
                        ui.label("Click a point on the cabinet.");
                    }
                    (1, _) => {
                        ui.label("Click the second point.");
                    }
                    (_, Some(r)) => {
                        ui.label(format!("Distance   {}", mm(r.distance)));
                        ui.weak(format!(
                            "Across {}   Down {}   Height {}",
                            mm(r.dx),
                            mm(r.dy),
                            mm(r.dz)
                        ));
                        if let Some(d) = r.devices {
                            ui.separator();
                            ui.label(format!("Gap between devices   {}", mm(d.gap)));
                            ui.weak(format!("Centre to centre   {}", mm(d.centres)));
                        }
                    }
                    _ => {}
                }
                ui.horizontal(|ui| {
                    if m.has_pins() && ui.small_button("Clear").clicked() {
                        clear = true;
                    }
                    ui.weak("Esc clears, again to leave");
                });
            });
        });
    clear
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::{Margins, Placement, Plate, Size};

    fn frame() -> PlateFrame {
        PlateFrame {
            width: 600.0,
            height: 800.0,
        }
    }

    fn placement(id: u64, x: f64, y: f64) -> Placement {
        Placement {
            id,
            x,
            y,
            rotated: false,
            size: Size {
                width: 45.0,
                height: 90.0,
                guessed: false,
            },
            clearance: None,
            margins: Margins::default(),
            rail: None,
        }
    }

    fn layout() -> CabinetLayout {
        let mut l = CabinetLayout::new(Plate {
            width: 600.0,
            height: 800.0,
            margin: 10.0,
        });
        l.placements = vec![placement(1, 100.0, 100.0), placement(2, 300.0, 100.0)];
        l
    }

    fn dev(id: u64) -> Option<ItemId> {
        Some(ItemId::new(ItemKind::Device, id))
    }

    #[test]
    fn two_clicks_read_the_distance_and_its_parts() {
        let f = frame();
        let mut m = Measure::default();
        m.click(f.world(100.0, 100.0, 0.0), None);
        assert!(m.reading(&f, None).is_none(), "one point is not a reading");
        m.click(f.world(160.0, 180.0, 0.0), None);
        let r = m.reading(&f, None).unwrap();
        assert!((r.distance - 100.0).abs() < 1e-3, "{r:?}");
        assert!((r.dx - 60.0).abs() < 1e-3 && (r.dy - 80.0).abs() < 1e-3);
        assert_eq!(r.dz, 0.0);
        assert!(r.devices.is_none());
    }

    #[test]
    fn height_counts_in_the_distance() {
        let f = frame();
        let mut m = Measure::default();
        m.click(f.world(0.0, 0.0, 0.0), None);
        m.click(f.world(0.0, 0.0, 75.0), None);
        let r = m.reading(&f, None).unwrap();
        assert!((r.distance - 75.0).abs() < 1e-3 && (r.dz - 75.0).abs() < 1e-3);
    }

    #[test]
    fn two_devices_read_the_gap_between_their_edges() {
        let f = frame();
        let l = layout();
        let mut m = Measure::default();
        m.click(f.world(110.0, 110.0, 20.0), dev(1));
        m.click(f.world(310.0, 110.0, 20.0), dev(2));
        let r = m.reading(&f, Some(&l)).unwrap();
        let d = r.devices.expect("both points are on devices");
        // Device 1 ends at x 145, device 2 starts at 300.
        assert!((d.gap - 155.0).abs() < 1e-6, "{d:?}");
        assert!((d.centres - 200.0).abs() < 1e-6);
    }

    #[test]
    fn the_same_device_twice_is_not_a_pair() {
        let f = frame();
        let l = layout();
        let mut m = Measure::default();
        m.click(f.world(110.0, 110.0, 20.0), dev(1));
        m.click(f.world(120.0, 150.0, 20.0), dev(1));
        assert!(m.reading(&f, Some(&l)).unwrap().devices.is_none());
    }

    #[test]
    fn a_third_click_starts_over_and_toggling_forgets_the_points() {
        let f = frame();
        let mut m = Measure::default();
        for x in [0.0, 10.0, 20.0] {
            m.click(f.world(x, 0.0, 0.0), None);
        }
        assert_eq!(m.key().len(), 1, "the third click began a new measurement");
        m.toggle();
        assert!(m.on && !m.has_pins());
    }

    #[test]
    fn the_overlays_are_valid_and_follow_the_points() {
        let f = frame();
        let mut m = Measure::default();
        assert!(m.overlays().is_empty());
        m.click(f.world(0.0, 0.0, 0.0), None);
        assert_eq!(m.overlays().len(), 1, "a cross for the first point");
        m.click(f.world(50.0, 50.0, 10.0), None);
        let all = m.overlays();
        assert_eq!(all.len(), 3);
        for o in all {
            let Overlay::Lines(l) = o else {
                panic!("lines only")
            };
            assert!(l.is_valid());
        }
    }
}
