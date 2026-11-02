//! Batch select and drag on the Cabinet plate. Box select (drag on empty
//! plate) and Shift-click are the canvas's own; this adds what happens next:
//! a drag that starts on a selected device carries the whole selection, the
//! arrow keys nudge it, Ctrl+A selects every device on the plate, and with two
//! or more devices selected the Align row (left, centre, right, top, middle,
//! bottom, spread across, spread down) docks under the plate. Every device of
//! a group lands through the single-device rules (grid, DIN rail snap), and
//! the whole move is one undo step.
//!
//! Where the user finds it: Cabinet workspace. Drag a box round devices, or
//! Shift-click them, then drag one; or press Ctrl+A.

use eframe::egui;
use novo_layout::state::CabinetLayout;

use super::view3d_align::{OPS, Op};

/// The Align row: a docked bar under the plate (it takes its own room, so it
/// covers nothing and sits clear of the status bar), shown while two or more
/// devices are selected. A button that needs more devices is greyed and says
/// how many in its hover text. Returns the operation clicked.
pub(super) fn align_bar(ui: &mut egui::Ui, devices: usize) -> Option<Op> {
    let mut picked = None;
    egui::Panel::bottom("cabinet_align_bar")
        .resizable(false)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Align");
                ui.weak(format!("{devices} selected"));
                for (_, label, hint, op) in OPS {
                    let need = op.needs();
                    let button = ui.add_enabled(devices >= need, egui::Button::new(label));
                    let button = if devices >= need {
                        button.on_hover_text(hint)
                    } else {
                        button.on_disabled_hover_text(format!(
                            "{hint}. Needs {need} selected devices."
                        ))
                    };
                    if button.clicked() {
                        picked = Some(op);
                    }
                }
            });
        });
    picked
}
/// Where every device of a dragged group lands: the grabbed one at `lead_at`
/// (already snapped, rail landing included), the others by the same offset
/// from where they started, each snapped to the grid and onto a DIN rail like
/// a single device would be. `group` holds `(id, start x, start y)`.
pub(super) fn landings(
    layout: &CabinetLayout,
    group: &[(u64, f64, f64)],
    lead: u64,
    lead_at: (f64, f64),
) -> Vec<(u64, f64, f64)> {
    let Some(&(_, lx, ly)) = group.iter().find(|g| g.0 == lead) else {
        return Vec::new();
    };
    let (dx, dy) = clamp_offset(layout, group, (lead_at.0 - lx, lead_at.1 - ly));
    let lead_at = (lx + dx, ly + dy);
    group
        .iter()
        .map(|&(id, x, y)| {
            if id == lead {
                return (id, lead_at.0, lead_at.1);
            }
            let at = layout.snapped(x + dx, y + dy);
            let at = layout
                .placements
                .iter()
                .find(|p| p.id == id)
                .map_or(at, |p| {
                    let f = p.footprint();
                    super::cabinet::rail_landing(layout, id, (f.w, f.h), at)
                });
            (id, at.0, at.1)
        })
        .collect()
}

/// The offset cut back so the group does not leave the usable plate any
/// further than it already does (a device that stands outside stays
/// flagged by the checks; a drag never pushes one further out).
/// Collisions are not prevented: like a single drag, the boxes turn red
/// and the checks name them.
fn clamp_offset(layout: &CabinetLayout, group: &[(u64, f64, f64)], d: (f64, f64)) -> (f64, f64) {
    let u = layout.plate.usable();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(id, x, y) in group {
        let Some(p) = layout.placements.iter().find(|p| p.id == id) else {
            continue;
        };
        let f = p.footprint();
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + f.w);
        max_y = max_y.max(y + f.h);
    }
    if min_x > max_x {
        return d;
    }
    let lo = |edge: f64, bound: f64| (bound - edge).min(0.0);
    let hi = |edge: f64, bound: f64| (bound - edge).max(0.0);
    (
        d.0.clamp(lo(min_x, u.x), hi(max_x, u.right())),
        d.1.clamp(lo(min_y, u.y), hi(max_y, u.bottom())),
    )
}

/// The group an arrow key moves: every selected device by `(dx, dy)` mm.
pub(super) fn nudged(
    layout: &CabinetLayout,
    selected: &[u64],
    dx: f64,
    dy: f64,
) -> Vec<(u64, f64, f64)> {
    layout
        .placements
        .iter()
        .filter(|p| selected.contains(&p.id))
        .map(|p| {
            // A device on a rail slides along it: the arrow across the rail
            // would take it off, which is a drag's job.
            let (dx, dy) = match p.rail.and_then(|r| layout.rails.iter().find(|q| q.id == r)) {
                Some(r) if r.vertical => (0.0, dy),
                Some(_) => (dx, 0.0),
                None => (dx, dy),
            };
            (p.id, p.x + dx, p.y + dy)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::{Margins, Placement, Plate, Size};

    fn placement(id: u64, x: f64, y: f64) -> Placement {
        Placement {
            id,
            x,
            y,
            rotated: false,
            size: Size {
                width: 40.0,
                height: 60.0,
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
        l.grid = 5.0;
        l.placements = vec![
            placement(1, 100.0, 100.0),
            placement(2, 200.0, 120.0),
            placement(3, 400.0, 400.0),
        ];
        l
    }

    #[test]
    fn the_group_keeps_its_shape_when_the_grabbed_device_moves() {
        let l = layout();
        let group = [(1, 100.0, 100.0), (2, 200.0, 120.0)];
        let out = landings(&l, &group, 1, (150.0, 70.0));
        assert_eq!(out, vec![(1, 150.0, 70.0), (2, 250.0, 90.0)]);
    }

    #[test]
    fn followers_snap_to_the_grid() {
        let l = layout();
        let group = [(1, 100.0, 100.0), (2, 203.0, 121.0)];
        let out = landings(&l, &group, 1, (150.0, 100.0));
        assert_eq!(out[1], (2, 255.0, 120.0));
    }

    #[test]
    fn an_unknown_lead_moves_nothing() {
        let l = layout();
        assert!(landings(&l, &[(1, 100.0, 100.0)], 9, (0.0, 0.0)).is_empty());
    }

    #[test]
    fn a_group_stops_at_the_plate_edge() {
        let l = layout();
        let group = [(1, 100.0, 100.0), (2, 200.0, 120.0)];
        // Far left and up: the left-most device stops at the margin (10).
        let out = landings(&l, &group, 1, (-500.0, -500.0));
        assert_eq!(out, vec![(1, 10.0, 10.0), (2, 110.0, 30.0)]);
        // Far right: the right edge of device 2 (200+40) stops at 590.
        let out = landings(&l, &group, 1, (900.0, 100.0));
        assert_eq!(out[1].1 + 40.0, 590.0);
    }

    #[test]
    fn a_device_already_outside_is_not_pulled_in_or_pushed_out() {
        let mut l = layout();
        l.placements[0].x = 0.0;
        let out = landings(&l, &[(1, 0.0, 100.0)], 1, (-50.0, 100.0));
        assert_eq!(out, vec![(1, 0.0, 100.0)]);
        let out = landings(&l, &[(1, 0.0, 100.0)], 1, (50.0, 100.0));
        assert_eq!(out, vec![(1, 50.0, 100.0)]);
    }

    #[test]
    fn a_device_on_a_rail_slides_along_it_only() {
        let mut l = layout();
        l.rails = vec![novo_layout::Rail {
            id: 7,
            x: 50.0,
            y: 130.0,
            length: 400.0,
            vertical: false,
        }];
        l.placements[0].rail = Some(7);
        let out = nudged(&l, &[1, 2], 5.0, 5.0);
        assert_eq!(out, vec![(1, 105.0, 100.0), (2, 205.0, 125.0)]);
    }

    #[test]
    fn nudge_moves_only_the_selected() {
        let l = layout();
        let out = nudged(&l, &[1, 3], 5.0, -5.0);
        assert_eq!(out, vec![(1, 105.0, 95.0), (3, 405.0, 395.0)]);
    }
}
