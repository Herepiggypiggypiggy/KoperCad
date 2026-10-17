//! Align and spread a multi-selection in the 3D view: line devices up on
//! their left, right, top or bottom edge or on a shared centre line, or
//! spread them so the free space between neighbours is equal. The moves are
//! one group edit (one undo step) and the Cabinet checks flag what the new
//! positions break, like any other move.
//!
//! Where the user finds it: select two or more devices in the 3D view; the
//! Align strip appears at the bottom of the view. Also View > 3D > Align and
//! spread, and Ctrl+K "align".

use std::collections::BTreeSet;

use eframe::egui;
use novo_layout::Rect;
use novo_layout::state::CabinetLayout;
use novo_view3d::scene::{ItemId, ItemKind};

use super::NovoApp;
use super::view3d_move::Edit;

/// What to do with the selected devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Op {
    Left,
    CentreX,
    Right,
    Top,
    CentreY,
    Bottom,
    SpreadAcross,
    SpreadDown,
}

/// `(command id, button label, hint)` for every operation.
pub(super) const OPS: [(&str, &str, &str, Op); 8] = [
    (
        "view.3d_align_left",
        "Left",
        "Line the left edges up",
        Op::Left,
    ),
    (
        "view.3d_align_hcentre",
        "Centre",
        "Line the centres up on one vertical line",
        Op::CentreX,
    ),
    (
        "view.3d_align_right",
        "Right",
        "Line the right edges up",
        Op::Right,
    ),
    ("view.3d_align_top", "Top", "Line the top edges up", Op::Top),
    (
        "view.3d_align_vcentre",
        "Middle",
        "Line the centres up on one horizontal line",
        Op::CentreY,
    ),
    (
        "view.3d_align_bottom",
        "Bottom",
        "Line the bottom edges up",
        Op::Bottom,
    ),
    (
        "view.3d_spread_h",
        "Spread across",
        "Equal space between the devices from left to right (needs three)",
        Op::SpreadAcross,
    ),
    (
        "view.3d_spread_v",
        "Spread down",
        "Equal space between the devices from top to bottom (needs three)",
        Op::SpreadDown,
    ),
];

impl Op {
    pub(super) fn from_command(id: &str) -> Option<Op> {
        OPS.iter().find(|o| o.0 == id).map(|o| o.3)
    }

    fn needs(self) -> usize {
        match self {
            Op::SpreadAcross | Op::SpreadDown => 3,
            _ => 2,
        }
    }
}

/// The new top left of every selected device that has to move.
pub(super) fn arrange(
    layout: &CabinetLayout,
    selection: &BTreeSet<ItemId>,
    op: Op,
) -> Option<Edit> {
    let mut boxes: Vec<(u64, Rect)> = selection
        .iter()
        .filter(|i| i.kind == ItemKind::Device)
        .filter_map(|i| {
            layout
                .placements
                .iter()
                .find(|p| p.id == i.raw)
                .map(|p| (p.id, p.footprint()))
        })
        .collect();
    if boxes.len() < op.needs() {
        return None;
    }
    let min = |f: &dyn Fn(&Rect) -> f64| boxes.iter().map(|b| f(&b.1)).fold(f64::MAX, f64::min);
    let max = |f: &dyn Fn(&Rect) -> f64| boxes.iter().map(|b| f(&b.1)).fold(f64::MIN, f64::max);
    let mut to: Vec<(u64, f64, f64)> = Vec::new();
    match op {
        Op::Left => {
            let x = min(&|r| r.x);
            to.extend(boxes.iter().map(|(id, r)| (*id, x, r.y)));
        }
        Op::Right => {
            let right = max(&|r| r.right());
            to.extend(boxes.iter().map(|(id, r)| (*id, right - r.w, r.y)));
        }
        Op::CentreX => {
            let c = (min(&|r| r.x) + max(&|r| r.right())) / 2.0;
            to.extend(boxes.iter().map(|(id, r)| (*id, c - r.w / 2.0, r.y)));
        }
        Op::Top => {
            let y = min(&|r| r.y);
            to.extend(boxes.iter().map(|(id, r)| (*id, r.x, y)));
        }
        Op::Bottom => {
            let bottom = max(&|r| r.bottom());
            to.extend(boxes.iter().map(|(id, r)| (*id, r.x, bottom - r.h)));
        }
        Op::CentreY => {
            let c = (min(&|r| r.y) + max(&|r| r.bottom())) / 2.0;
            to.extend(boxes.iter().map(|(id, r)| (*id, r.x, c - r.h / 2.0)));
        }
        Op::SpreadAcross => {
            boxes.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
            let span = boxes.last()?.1.right() - boxes[0].1.x;
            let used: f64 = boxes.iter().map(|b| b.1.w).sum();
            let gap = (span - used) / (boxes.len() - 1) as f64;
            let mut x = boxes[0].1.x;
            for (id, r) in &boxes {
                to.push((*id, x, r.y));
                x += r.w + gap;
            }
        }
        Op::SpreadDown => {
            boxes.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
            let span = boxes.last()?.1.bottom() - boxes[0].1.y;
            let used: f64 = boxes.iter().map(|b| b.1.h).sum();
            let gap = (span - used) / (boxes.len() - 1) as f64;
            let mut y = boxes[0].1.y;
            for (id, r) in &boxes {
                to.push((*id, r.x, y));
                y += r.h + gap;
            }
        }
    }
    // Only what actually moves, so an already aligned selection is no edit.
    let devices: Vec<(u64, f64, f64)> = to
        .into_iter()
        .filter(|(id, x, y)| {
            boxes
                .iter()
                .find(|b| b.0 == *id)
                .is_some_and(|b| (b.1.x - x).abs() > 1e-9 || (b.1.y - y).abs() > 1e-9)
        })
        .collect();
    (!devices.is_empty()).then_some(Edit::Group {
        devices,
        rails: Vec::new(),
    })
}

/// The Align strip, bottom centre of the view; the operation clicked.
pub(super) fn strip(ctx: &egui::Context, view: egui::Rect, devices: usize) -> Option<Op> {
    let mut picked = None;
    egui::Area::new(egui::Id::new("view3d_align"))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(egui::pos2(view.center().x, view.bottom() - 12.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Align");
                    for (_, label, hint, op) in OPS {
                        let enough = devices >= op.needs();
                        if ui
                            .add_enabled(enough, egui::Button::new(label))
                            .on_hover_text(hint)
                            .clicked()
                        {
                            picked = Some(op);
                        }
                        if op == Op::Bottom {
                            ui.separator();
                        }
                    }
                });
            });
        });
    picked
}

impl NovoApp {
    /// Runs `op` on the devices selected in the 3D view.
    pub(super) fn view3d_arrange(&mut self, op: Op) {
        let Some(document) = self.backend.document() else {
            return;
        };
        let project = document.project();
        let Some(id) = super::view3d::enclosure(project, self.cabinet.location.as_ref()) else {
            return;
        };
        let Some(layout) = novo_layout::model::load(project, id) else {
            return;
        };
        let selection = self.view3d.view.state.selection.clone();
        match arrange(&layout, &selection, op) {
            Some(Edit::Group { devices, rails }) => self.view3d_move_group(id, devices, rails),
            _ => self.message(
                super::Level::Info,
                "Select two or more devices that are not lined up yet (three to spread).",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::{Margins, Placement, Plate, Size};

    fn placement(id: u64, x: f64, y: f64, w: f64, h: f64) -> Placement {
        Placement {
            id,
            x,
            y,
            rotated: false,
            size: Size {
                width: w,
                height: h,
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
        l.placements = vec![
            placement(1, 100.0, 100.0, 40.0, 80.0),
            placement(2, 150.0, 130.0, 60.0, 60.0),
            placement(3, 400.0, 110.0, 20.0, 100.0),
        ];
        l
    }

    fn all() -> BTreeSet<ItemId> {
        (1..=3).map(|i| ItemId::new(ItemKind::Device, i)).collect()
    }

    fn moves(e: Option<Edit>) -> Vec<(u64, f64, f64)> {
        match e {
            Some(Edit::Group { mut devices, .. }) => {
                devices.sort_by_key(|d| d.0);
                devices
            }
            other => panic!("not a group edit: {other:?}"),
        }
    }

    #[test]
    fn left_and_right_use_the_outermost_edges() {
        let l = layout();
        // Device 1 already starts at the left-most x, so only 2 and 3 move.
        assert_eq!(
            moves(arrange(&l, &all(), Op::Left)),
            vec![(2, 100.0, 130.0), (3, 100.0, 110.0)]
        );
        // Right-most edge is 420.
        assert_eq!(
            moves(arrange(&l, &all(), Op::Right)),
            vec![(1, 380.0, 100.0), (2, 360.0, 130.0)]
        );
    }

    #[test]
    fn centres_share_the_middle_of_the_whole_selection() {
        let l = layout();
        // x from 100 to 420: centre 260.
        let m = moves(arrange(&l, &all(), Op::CentreX));
        assert_eq!(m[0], (1, 240.0, 100.0));
        assert_eq!(m[2], (3, 250.0, 110.0));
        // y from 100 to 210: centre 155.
        let m = moves(arrange(&l, &all(), Op::CentreY));
        assert_eq!(m[0], (1, 100.0, 115.0));
    }

    #[test]
    fn top_and_bottom_line_the_edges_up() {
        let l = layout();
        let m = moves(arrange(&l, &all(), Op::Top));
        assert!(m.iter().all(|d| d.2 == 100.0));
        let m = moves(arrange(&l, &all(), Op::Bottom));
        // Lowest bottom is 210; each device ends there.
        assert!(m.iter().all(|(id, _, y)| {
            (y + [80.0, 60.0, 100.0][*id as usize - 1] - 210.0).abs() < 1e-9
        }));
    }

    #[test]
    fn spreading_leaves_equal_gaps_and_keeps_the_ends() {
        let l = layout();
        let m = moves(arrange(&l, &all(), Op::SpreadAcross));
        // Span 100..420 = 320, widths 120, free 200, two gaps of 100.
        // Device 1 keeps x 100 and device 3 ends at 420: only 2 moves.
        assert_eq!(m, vec![(2, 240.0, 130.0)]);
    }

    #[test]
    fn too_few_devices_or_nothing_to_move_is_no_edit() {
        let l = layout();
        let one = BTreeSet::from([ItemId::new(ItemKind::Device, 1)]);
        assert!(arrange(&l, &one, Op::Left).is_none());
        let two = BTreeSet::from([
            ItemId::new(ItemKind::Device, 1),
            ItemId::new(ItemKind::Device, 2),
        ]);
        assert!(arrange(&l, &two, Op::SpreadAcross).is_none(), "needs three");
        let mut aligned = layout();
        aligned.placements[1].x = 100.0;
        assert!(arrange(&aligned, &two, Op::Left).is_none());
    }

    #[test]
    fn every_operation_has_a_unique_command() {
        let ids: BTreeSet<_> = OPS.iter().map(|o| o.0).collect();
        assert_eq!(ids.len(), OPS.len());
        assert_eq!(Op::from_command("view.3d_align_top"), Some(Op::Top));
        assert_eq!(Op::from_command("view.3d_nope"), None);
    }
}
