//! Batch select and drag on the Cabinet plate. Box select (drag on empty
//! plate) and Shift-click are the canvas's own; this adds what happens next:
//! a drag that starts on a selected device carries the whole selection, the
//! arrow keys nudge it, Ctrl+A selects every device on the plate, and with two
//! or more devices selected the Align strip (left, centre, right, top, middle,
//! bottom, spread across, spread down) shows over the plate. Every device of
//! a group lands through the single-device rules (grid, DIN rail snap), and
//! the whole move is one undo step.
//!
//! Where the user finds it: Cabinet workspace. Drag a box round devices, or
//! Shift-click them, then drag one; or press Ctrl+A.

use novo_layout::state::CabinetLayout;

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
    let (dx, dy) = (lead_at.0 - lx, lead_at.1 - ly);
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
        .map(|p| (p.id, p.x + dx, p.y + dy))
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
    fn nudge_moves_only_the_selected() {
        let l = layout();
        let out = nudged(&l, &[1, 3], 5.0, -5.0);
        assert_eq!(out, vec![(1, 105.0, 95.0), (3, 405.0, 395.0)]);
    }
}
