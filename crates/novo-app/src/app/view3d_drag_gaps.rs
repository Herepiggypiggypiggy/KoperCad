//! Live distances while a device is dragged in the 3D view: the clear gap to
//! the nearest neighbour on each side, with the gap the two parts ask for, so
//! the fit is judged while the part is still in the hand. The rule is the one
//! of `novo_layout::clearance_violations`: the larger of the two facing
//! sides' needs.

use eframe::egui;
use novo_layout::Side;
use novo_layout::state::CabinetLayout;
use novo_view3d::dimension::format_mm;
use novo_view3d::scene::ItemKind;

use super::view3d_move::Landing;

fn format_mm_text(mm: f64) -> String {
    format_mm(mm as f32)
}

/// Rounding slack when a gap is compared with the need, mm.
const EPS_MM: f64 = 0.05;

/// The clear distance from the landing box to what is beside it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SideGap {
    pub side: Side,
    pub mm: f64,
    /// What the two facing parts ask for.
    pub need: f64,
}

impl SideGap {
    pub fn ok(&self) -> bool {
        self.mm + EPS_MM >= self.need
    }
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Left => "left",
        Side::Right => "right",
        Side::Above => "above",
        Side::Below => "below",
    }
}

/// The nearest device on each side of a dragged device at `landing`, as far
/// as it stands wholly beside it and overlaps it across. Empty for anything
/// but a single device on the plate.
pub(super) fn gaps(layout: &CabinetLayout, landing: &Landing) -> Vec<SideGap> {
    if landing.item.kind != ItemKind::Device || landing.follower {
        return Vec::new();
    }
    let others = layout.on_plate();
    let Some(mut own) = others.iter().find(|p| p.id == landing.item.raw).cloned() else {
        return Vec::new();
    };
    own.x = landing.at.0;
    own.y = landing.at.1;
    let b = own.footprint();
    let mut out = Vec::new();
    for side in [Side::Left, Side::Right, Side::Above, Side::Below] {
        let near = others
            .iter()
            .filter(|p| p.id != own.id)
            .filter_map(|p| {
                let f = p.footprint();
                let across_y = f.y < b.bottom() && f.bottom() > b.y;
                let across_x = f.x < b.right() && f.right() > b.x;
                let (gap, facing) = match side {
                    Side::Left if across_y && f.right() <= b.x => (b.x - f.right(), Side::Right),
                    Side::Right if across_y && f.x >= b.right() => (f.x - b.right(), Side::Left),
                    Side::Above if across_x && f.bottom() <= b.y => (b.y - f.bottom(), Side::Below),
                    Side::Below if across_x && f.y >= b.bottom() => (f.y - b.bottom(), Side::Above),
                    _ => return None,
                };
                Some((gap, own.need(side).max(p.need(facing))))
            })
            .min_by(|a, c| a.0.total_cmp(&c.0));
        if let Some((mm, need)) = near {
            out.push(SideGap { side, mm, need });
        }
    }
    out
}

/// One line of the card: `left 45 mm (min 60 mm)`.
pub(super) fn line(g: &SideGap) -> String {
    format!(
        "{} {} (min {})",
        side_name(g.side),
        format_mm_text(g.mm),
        format_mm_text(g.need)
    )
}

/// The gaps as a small card by the pointer: green where the clearance is
/// kept, red where it is not.
pub(super) fn show(ctx: &egui::Context, landing: &Landing, gaps: &[SideGap]) {
    let Some(at) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let p = crate::theme::ui_palette();
    egui::Area::new(egui::Id::new("view3d.drag_gaps"))
        .order(egui::Order::Tooltip)
        .fixed_pos(at + egui::vec2(16.0, 16.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(format!(
                    "X {}  Y {}",
                    format_mm_text(landing.at.0),
                    format_mm_text(landing.at.1)
                ));
                for g in gaps {
                    let colour = if g.ok() { p.success } else { p.error };
                    ui.label(egui::RichText::new(line(g)).color(colour).monospace());
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::state::CabinetLayout;
    use novo_layout::{Margins, Placement, Plate, Rect, Size};
    use novo_view3d::scene::ItemId;

    fn dev(id: u64, x: f64) -> Placement {
        Placement {
            id,
            x,
            y: 100.0,
            rotated: false,
            size: Size {
                width: 50.0,
                height: 50.0,
                guessed: false,
            },
            clearance: Some(20.0),
            margins: Margins::default(),
            rail: None,
        }
    }

    fn layout() -> CabinetLayout {
        let mut l = CabinetLayout::new(Plate {
            width: 600.0,
            height: 400.0,
            margin: 0.0,
        });
        l.placements = vec![dev(1, 0.0), dev(2, 200.0), dev(3, 400.0)];
        l
    }

    fn landing(at: (f64, f64)) -> Landing {
        Landing {
            item: ItemId::new(ItemKind::Device, 2),
            at,
            rect: Rect {
                x: at.0,
                y: at.1,
                w: 50.0,
                h: 50.0,
            },
            clash: false,
            rail: None,
            follower: false,
            blockers: Vec::new(),
        }
    }

    #[test]
    fn nearest_neighbour_each_side_with_the_asked_clearance() {
        let l = layout();
        // Dragged to x 80: 30 mm from the left neighbour (ends at 50), 270 from the right.
        let g = gaps(&l, &landing((80.0, 100.0)));
        let left = g.iter().find(|g| g.side == Side::Left).unwrap();
        assert!((left.mm - 30.0).abs() < 1e-9 && left.need == 20.0 && left.ok());
        let right = g.iter().find(|g| g.side == Side::Right).unwrap();
        assert!((right.mm - 270.0).abs() < 1e-9);
        assert!(
            g.iter()
                .all(|g| g.side != Side::Above && g.side != Side::Below)
        );
        assert_eq!(line(left), "left 30 mm (min 20 mm)");
    }

    #[test]
    fn too_close_is_not_ok() {
        let l = layout();
        let g = gaps(&l, &landing((60.0, 100.0)));
        let left = g.iter().find(|g| g.side == Side::Left).unwrap();
        assert!((left.mm - 10.0).abs() < 1e-9 && !left.ok());
    }

    #[test]
    fn rails_and_unknown_devices_give_nothing() {
        let l = layout();
        let mut rail = landing((80.0, 100.0));
        rail.item = ItemId::new(ItemKind::Rail, 2);
        assert!(gaps(&l, &rail).is_empty());
        let mut unknown = landing((80.0, 100.0));
        unknown.item = ItemId::new(ItemKind::Device, 99);
        assert!(gaps(&l, &unknown).is_empty());
    }
}
