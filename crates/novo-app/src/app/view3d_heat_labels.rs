//! Pinned labels on the hot devices of a finished heat result in the 3D
//! view: tag, surface temperature and the margin to the tightest limit, the
//! same reading as the badges of the 2D heat view. They sit on the front of
//! the device and follow the camera. Model values from the solver, not
//! measurements (the strip's Probe and the Heat panel say so).

use eframe::egui;
use glam::Vec3;
use novo_cfd::CfdSnapshot;
use novo_thermal::Status;
use novo_view3d::Scene;
use novo_view3d::scene::ItemKind;
use novo_view3d::widget::View3d;

use super::heat_spots::{self, Row};

/// Most labels pinned at once: the devices nearest or over their limit.
pub(super) const MOST: usize = 6;

/// What one label says.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Label {
    pub id: u64,
    pub text: String,
    pub status: Status,
}

fn text(r: &Row) -> String {
    match r.margin_k {
        Some(m) => format!("{}  {:.0} °C · {m:+.0} K", r.tag, r.surface_c),
        None => format!("{}  {:.0} °C", r.tag, r.surface_c),
    }
}

/// The labels of a result, worst first: over their limit, then marginal,
/// then the hottest. Devices that give off no heat get none.
pub(super) fn labels(snap: &CfdSnapshot, tag_of: &dyn Fn(u64) -> String) -> Vec<Label> {
    heat_spots::rows(snap, tag_of, MOST)
        .iter()
        .map(|r| Label {
            id: r.id,
            text: text(r),
            status: r.status,
        })
        .collect()
}

/// Draws the labels over the 3D view. `dim` greys them while the result is
/// out of date or a new run is under way.
pub(super) fn paint(
    ui: &egui::Ui,
    view: &View3d,
    rect: egui::Rect,
    scene: &Scene,
    labels: &[Label],
    dim: bool,
) {
    let pal = crate::theme::ui_palette();
    let font = egui::FontId::proportional(12.0);
    let painter = ui.painter().with_clip_rect(rect);
    let mut placed: Vec<egui::Rect> = Vec::new();
    for l in labels {
        let Some(item) = scene
            .items
            .iter()
            .find(|i| i.id.kind == ItemKind::Device && i.id.raw == l.id)
        else {
            continue;
        };
        // The front face (the side away from the plate), in the middle.
        let front = scene
            .world_matrix(item)
            .transform_point3(Vec3::new(0.0, 0.0, item.size.z));
        let Some(at) = view.project(rect, scene, front) else {
            continue;
        };
        let colour = if dim {
            pal.text_secondary
        } else {
            match l.status {
                Status::Over => pal.error,
                Status::Marginal => pal.warning,
                Status::Ok => pal.success,
                Status::NoLimit => pal.text_secondary,
            }
        };
        let galley = painter.layout_no_wrap(l.text.clone(), font.clone(), pal.text_primary);
        let size = galley.size() + egui::vec2(12.0, 6.0);
        let mut chip = egui::Rect::from_center_size(at, size);
        // Labels of devices that lie close together step down instead of
        // covering each other.
        while placed.iter().any(|p| p.intersects(chip)) && chip.max.y < rect.max.y {
            chip = chip.translate(egui::vec2(0.0, size.y + 2.0));
        }
        placed.push(chip);
        painter.rect(
            chip,
            4.0,
            pal.raised.gamma_multiply(0.92),
            egui::Stroke::new(1.5, colour),
            egui::StrokeKind::Inside,
        );
        painter.galley(chip.min + egui::vec2(6.0, 3.0), galley, pal.text_primary);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::heat_3d_tests::snapshot;
    use novo_layout::Rect;
    use novo_thermal::DeviceResult;

    fn dev(id: u64, surface: f64, loss: f64, status: Status, margin: Option<f64>) -> DeviceResult {
        DeviceResult {
            id,
            footprint: Rect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
            loss_w: loss,
            surface_c: surface,
            air_c: surface - 5.0,
            h_w_m2k: 8.0,
            plate_share: 0.2,
            surface_band_c: (surface, surface),
            air_band_c: (surface, surface),
            rated_max_ambient_c: None,
            rated_max_surface_c: None,
            margin_k: margin,
            margin_pessimistic_k: margin,
            status,
        }
    }

    fn tag(id: u64) -> String {
        format!("-F{id}")
    }

    #[test]
    fn a_label_reads_like_the_2d_badge_with_the_margin_signed() {
        let mut s = snapshot();
        s.devices = vec![
            dev(1, 57.0, 5.0, Status::Ok, Some(8.0)),
            dev(2, 91.0, 5.0, Status::Over, Some(-3.0)),
            dev(3, 40.0, 5.0, Status::NoLimit, None),
        ];
        let l = labels(&s, &tag);
        assert_eq!(l[0].text, "-F2  91 °C · -3 K");
        assert_eq!(l[0].status, Status::Over);
        assert_eq!(l[1].text, "-F1  57 °C · +8 K");
        assert_eq!(l[2].text, "-F3  40 °C");
    }

    #[test]
    fn only_the_worst_few_devices_are_pinned_and_idle_ones_never() {
        let mut s = snapshot();
        s.devices = (1..=10)
            .map(|i| dev(i, 40.0 + i as f64, 5.0, Status::Ok, Some(30.0)))
            .collect();
        s.devices.push(dev(99, 120.0, 0.0, Status::Ok, None));
        let l = labels(&s, &tag);
        assert_eq!(l.len(), MOST);
        assert!(l.iter().all(|x| x.id != 99));
        assert_eq!(l[0].id, 10, "the hottest comes first within a status");
    }
}
