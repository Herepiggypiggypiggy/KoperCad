//! Placing an unplaced device from a list into the 3D view (View > 3D >
//! Place a device, right-click on empty plate > Place a device, or Ctrl+K
//! "place"): the list shows the devices of this enclosure that are not on the
//! plate yet; pick one and a landing box follows the pointer over the plate,
//! snapped to the grid and onto a rail or duct line, red where the Cabinet
//! would flag the spot. A click drops it with the Cabinet's own `Ask::Drop`
//! (one undo step), Esc lets go of the device.

use eframe::egui;
use novo_layout::Rect;
use novo_layout::plan::Candidate;
use novo_layout::state::CabinetLayout;
use novo_view3d::scene::{ItemId, ItemKind};

use super::view3d_move::{Landing, rail_snap};

/// Margin from the view's corner and the height of the toolbar above the
/// list, in points.
const MARGIN: f32 = 12.0;
const BELOW_TOOLBAR: f32 = 52.0;

/// The list: open or not, the device picked from it and where the pointer
/// was over the plate last frame.
#[derive(Debug, Default)]
pub(crate) struct PlaceList {
    pub(super) open: bool,
    pub(super) armed: Option<u64>,
    pub(super) pointer: Option<(f64, f64)>,
}

impl PlaceList {
    pub(super) fn toggle(&mut self) {
        self.open = !self.open;
        self.armed = None;
    }

    /// Where the armed device would land with its centre at the pointer.
    pub(super) fn landing(&self, layout: &CabinetLayout, cands: &[Candidate]) -> Option<Landing> {
        let id = self.armed?;
        let at = self.pointer?;
        let c = cands.iter().find(|c| c.id == id)?;
        landing(layout, c, at)
    }
}

/// The landing of candidate `c` with its centre at plate point `at`; `None`
/// when it is on the plate already.
pub(super) fn landing(layout: &CabinetLayout, c: &Candidate, at: (f64, f64)) -> Option<Landing> {
    if layout.placements.iter().any(|p| p.id == c.id) {
        return None;
    }
    let raw = layout.snapped(at.0 - c.size.width / 2.0, at.1 - c.size.height / 2.0);
    let mut trial = layout.clone();
    trial.drop_device(c.id, c.size, raw.0, raw.1);
    let (to, rail) = rail_snap(&trial, c.id, raw);
    trial.drop_device(c.id, c.size, to.0, to.1);
    if let Some(p) = trial.placements.iter_mut().find(|p| p.id == c.id) {
        (p.x, p.y) = to;
    }
    let report = trial.report();
    let id = c.id;
    let clash = report.collisions.iter().any(|(a, b)| *a == id || *b == id)
        || report.outside.contains(&id)
        || report
            .too_close
            .iter()
            .any(|(a, b, _)| *a == id || *b == id);
    Some(Landing {
        item: ItemId::new(ItemKind::Device, id),
        at: to,
        rect: Rect {
            x: to.0,
            y: to.1,
            w: c.size.width,
            h: c.size.height,
        },
        clash,
        rail,
        follower: false,
        blockers: Vec::new(),
    })
}

/// What a frame of the list asked for.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Picked {
    pub close: bool,
}

/// The list, below the toolbar. `items` are `(id, tag, size text)`.
pub(super) fn panel(
    ctx: &egui::Context,
    view: egui::Rect,
    state: &mut PlaceList,
    items: &[(u64, String, String)],
) -> Picked {
    let mut out = Picked::default();
    egui::Area::new(egui::Id::new("view3d_place"))
        .order(egui::Order::Middle)
        .fixed_pos(view.left_top() + egui::vec2(MARGIN, BELOW_TOOLBAR + MARGIN))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(190.0);
                ui.horizontal(|ui| {
                    ui.strong("Place a device");
                    if ui
                        .small_button("Close")
                        .on_hover_text("Close the list")
                        .clicked()
                    {
                        out.close = true;
                    }
                });
                ui.weak("Pick one, then click on the plate");
                ui.add_space(2.0);
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for (id, tag, size) in items {
                            let on = state.armed == Some(*id);
                            let row = ui.selectable_label(on, format!("{tag}   {size}"));
                            if row.clicked() {
                                state.armed = if on { None } else { Some(*id) };
                            }
                        }
                        if items.is_empty() {
                            ui.weak("Every device is placed");
                        }
                    });
                if state.armed.is_some() {
                    ui.weak("Esc lets go of the device");
                }
            });
        });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::{Duct, Margins, Placement, Plate, Rail, Size};

    fn size() -> Size {
        Size {
            width: 45.0,
            height: 90.0,
            guessed: false,
        }
    }

    fn cand(id: u64) -> Candidate {
        Candidate {
            id,
            size: size(),
            sort_key: String::new(),
        }
    }

    fn layout() -> CabinetLayout {
        let mut l = CabinetLayout::new(Plate {
            width: 600.0,
            height: 800.0,
            margin: 10.0,
        });
        l.placements = vec![Placement {
            id: 1,
            x: 100.0,
            y: 100.0,
            rotated: false,
            size: size(),
            clearance: None,
            margins: Margins::default(),
            rail: None,
        }];
        l.rails = vec![Rail {
            id: 7,
            x: 50.0,
            y: 400.0,
            length: 300.0,
            vertical: false,
        }];
        l.ducts = Vec::<Duct>::new();
        l
    }

    #[test]
    fn the_landing_is_centred_on_the_pointer_and_snapped() {
        let l = layout();
        let land = landing(&l, &cand(2), (400.0, 200.0)).unwrap();
        assert!(!land.clash);
        let c = (
            land.rect.x + land.rect.w / 2.0,
            land.rect.y + land.rect.h / 2.0,
        );
        assert!(
            (c.0 - 400.0).abs() <= l.grid && (c.1 - 200.0).abs() <= l.grid,
            "{c:?}"
        );
        assert_eq!(land.at, (land.rect.x, land.rect.y));
    }

    #[test]
    fn a_spot_on_another_device_is_a_clash() {
        let l = layout();
        let land = landing(&l, &cand(2), (120.0, 140.0)).unwrap();
        assert!(land.clash);
    }

    #[test]
    fn a_spot_off_the_plate_is_a_clash() {
        let l = layout();
        assert!(landing(&l, &cand(2), (-50.0, 200.0)).unwrap().clash);
    }

    #[test]
    fn near_a_rail_the_top_edge_snaps_onto_its_line() {
        let l = layout();
        // Centre 45 mm above the rail line puts the top about 4 mm off it.
        let land = landing(&l, &cand(2), (200.0, 403.0 + 45.0 - 4.0)).unwrap();
        assert_eq!(land.rail, Some(7));
        assert_eq!(land.at.1, 400.0);
    }

    #[test]
    fn a_placed_device_has_no_landing() {
        let l = layout();
        assert!(landing(&l, &cand(1), (300.0, 300.0)).is_none());
    }

    #[test]
    fn arming_needs_a_pointer_and_toggling_lets_go() {
        let l = layout();
        let mut p = PlaceList {
            open: true,
            armed: Some(2),
            pointer: None,
        };
        assert!(p.landing(&l, &[cand(2)]).is_none());
        p.pointer = Some((300.0, 300.0));
        assert!(p.landing(&l, &[cand(2)]).is_some());
        p.toggle();
        assert!(!p.open && p.armed.is_none());
    }
}
