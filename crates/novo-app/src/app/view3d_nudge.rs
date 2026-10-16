//! Precise moves in the 3D view: the arrow keys nudge the selection (a grid
//! step, 5 mm with Ctrl, 1 mm with Shift) and a Position box types the
//! offset of the selected device from the plate's left and top edge. Both
//! go through the group move the drag uses, so they are one undo step, the
//! checks and the heat result follow, and the Cabinet shows the same project.
//!
//! Where the user finds it: select something in the 3D view and press the
//! arrow keys; the Position box (bottom right) shows when one device is
//! selected. View > 3D > Position box (Ctrl+K "position") shows or hides it.

use std::collections::BTreeSet;

use eframe::egui;
use novo_layout::state::CabinetLayout;
use novo_view3d::scene::{ItemId, ItemKind};

use super::view3d_move::{Edit, area, origin};

/// Fine and coarse nudge, mm.
pub(super) const FINE_MM: f64 = 1.0;
pub(super) const COARSE_MM: f64 = 5.0;
const MARGIN: f32 = 12.0;

/// How far one arrow press moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    Grid,
    Coarse,
    Fine,
}

impl Step {
    pub(super) fn from_modifiers(m: egui::Modifiers) -> Step {
        if m.shift {
            Step::Fine
        } else if m.command || m.ctrl {
            Step::Coarse
        } else {
            Step::Grid
        }
    }
}

/// The selection moved one step in `dir` (x right, y down; -1, 0 or 1 each)
/// as one group edit; `None` when nothing movable is selected.
pub(super) fn nudge(
    layout: &CabinetLayout,
    selection: &BTreeSet<ItemId>,
    dir: (i32, i32),
    step: Step,
) -> Option<Edit> {
    if dir == (0, 0) {
        return None;
    }
    let grid = layout.grid;
    let along = |v: f64, d: i32| -> f64 {
        if d == 0 {
            return v;
        }
        let d = f64::from(d);
        match step {
            Step::Fine => v + d * FINE_MM,
            Step::Coarse => v + d * COARSE_MM,
            // Onto the next grid line, not off by whatever the item had.
            Step::Grid if grid > 0.0 => {
                let n = v / grid;
                // A tolerance keeps an item that is on a line one step on.
                (if d > 0.0 {
                    (n + 1e-9).floor() + 1.0
                } else {
                    (n - 1e-9).ceil() - 1.0
                }) * grid
            }
            Step::Grid => v + d * FINE_MM,
        }
    };
    let mut devices = Vec::new();
    let mut rails = Vec::new();
    for item in selection {
        let Some((x, y)) = origin(layout, *item) else {
            continue;
        };
        let to = (along(x, dir.0), along(y, dir.1));
        match item.kind {
            ItemKind::Device => devices.push((item.raw, to.0, to.1)),
            ItemKind::Rail | ItemKind::Duct => rails.push((item.raw, to.0, to.1)),
            _ => {}
        }
    }
    (!devices.is_empty() || !rails.is_empty()).then_some(Edit::Group { devices, rails })
}

/// Where a device sits on the plate, mm: its top left from the plate's left
/// and top edge, and the free space to the right and bottom edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Position {
    pub x: f64,
    pub y: f64,
    pub to_right: f64,
    pub to_bottom: f64,
}

pub(super) fn position(layout: &CabinetLayout, device: u64) -> Option<Position> {
    let item = ItemId::new(ItemKind::Device, device);
    let (x, y) = origin(layout, item)?;
    let r = area(layout, item, (x, y))?;
    Some(Position {
        x,
        y,
        to_right: layout.plate.width - r.right(),
        to_bottom: layout.plate.height - r.bottom(),
    })
}

/// The device moved so its top left is at the typed offsets (either may be
/// left as it is).
pub(super) fn set_position(
    layout: &CabinetLayout,
    device: u64,
    x: Option<f64>,
    y: Option<f64>,
) -> Option<Edit> {
    let now = position(layout, device)?;
    let to = (x.unwrap_or(now.x), y.unwrap_or(now.y));
    ((to.0 - now.x).abs() > 1e-9 || (to.1 - now.y).abs() > 1e-9).then(|| Edit::Group {
        devices: vec![(device, to.0, to.1)],
        rails: Vec::new(),
    })
}

/// A number typed in mm: a comma works as the decimal point; nothing else
/// than a finite number is accepted.
pub(super) fn parse_mm(text: &str) -> Option<f64> {
    let t = text.trim().trim_end_matches("mm").trim().replace(',', ".");
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The text fields of the Position box.
#[derive(Debug, Default)]
pub(crate) struct Fields {
    pub(super) hidden: bool,
    for_device: Option<(u64, Position)>,
    x: String,
    y: String,
}

impl Fields {
    pub(super) fn toggle(&mut self) {
        self.hidden = !self.hidden;
    }

    /// The box for `device`, bottom right of `view`; returns the edit typed
    /// in, if any.
    pub(super) fn show(
        &mut self,
        ctx: &egui::Context,
        view: egui::Rect,
        layout: &CabinetLayout,
        device: u64,
    ) -> Option<Edit> {
        let pos = position(layout, device)?;
        let editing = ctx.memory(|m| m.focused()).is_some_and(|f| {
            f == egui::Id::new("view3d_pos_x") || f == egui::Id::new("view3d_pos_y")
        });
        if !editing && self.for_device != Some((device, pos)) {
            self.x = format!("{:.1}", pos.x);
            self.y = format!("{:.1}", pos.y);
            self.for_device = Some((device, pos));
        }
        let mut typed: (Option<f64>, Option<f64>) = (None, None);
        egui::Area::new(egui::Id::new("view3d_position"))
            .order(egui::Order::Middle)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(view.right_bottom() + egui::vec2(-MARGIN, -MARGIN))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.strong("Position");
                    ui.weak("Top left from the plate's left and top edge");
                    for (label, id, text, slot) in [
                        ("X", "view3d_pos_x", &mut self.x, &mut typed.0),
                        ("Y", "view3d_pos_y", &mut self.y, &mut typed.1),
                    ] {
                        ui.horizontal(|ui| {
                            ui.label(label);
                            let r = ui.add(
                                egui::TextEdit::singleline(text)
                                    .id(egui::Id::new(id))
                                    .desired_width(70.0),
                            );
                            ui.label("mm");
                            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                *slot = parse_mm(text);
                            }
                        });
                    }
                    ui.weak(format!(
                        "Free to the right {:.1} mm, to the bottom {:.1} mm",
                        pos.to_right, pos.to_bottom
                    ));
                    ui.weak("Arrows nudge: grid, Ctrl 5 mm, Shift 1 mm");
                });
            });
        if typed == (None, None) {
            return None;
        }
        set_position(layout, device, typed.0, typed.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::{Margins, Placement, Plate, Rail, Size};

    fn layout() -> CabinetLayout {
        let mut l = CabinetLayout::new(Plate {
            width: 600.0,
            height: 800.0,
            margin: 10.0,
        });
        l.placements = vec![Placement {
            id: 1,
            x: 103.0,
            y: 100.0,
            rotated: false,
            size: Size {
                width: 45.0,
                height: 90.0,
                guessed: false,
            },
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
        l
    }

    fn dev(id: u64) -> ItemId {
        ItemId::new(ItemKind::Device, id)
    }

    type Moves = Vec<(u64, f64, f64)>;

    fn group(e: Option<Edit>) -> (Moves, Moves) {
        match e {
            Some(Edit::Group { devices, rails }) => (devices, rails),
            other => panic!("not a group edit: {other:?}"),
        }
    }

    #[test]
    fn fine_and_coarse_steps_are_exact_millimetres() {
        let l = layout();
        let sel = BTreeSet::from([dev(1)]);
        let (d, _) = group(nudge(&l, &sel, (1, 0), Step::Fine));
        assert_eq!(d, vec![(1, 104.0, 100.0)]);
        let (d, _) = group(nudge(&l, &sel, (0, -1), Step::Coarse));
        assert_eq!(d, vec![(1, 103.0, 95.0)]);
    }

    #[test]
    fn a_grid_step_lands_on_the_next_grid_line() {
        let l = layout();
        let g = l.grid;
        let sel = BTreeSet::from([dev(1)]);
        assert_eq!(g, 5.0);
        let (d, _) = group(nudge(&l, &sel, (1, 0), Step::Grid));
        assert_eq!(d[0].1, 105.0, "off the grid: the next line");
        let (d, _) = group(nudge(&l, &sel, (-1, 0), Step::Grid));
        assert_eq!(d[0].1, 100.0);
        let (d, _) = group(nudge(&l, &sel, (0, 1), Step::Grid));
        assert_eq!(d[0].2, 105.0, "on the grid: one step");
    }

    #[test]
    fn a_selection_moves_together_devices_and_rails() {
        let l = layout();
        let sel = BTreeSet::from([dev(1), ItemId::new(ItemKind::Rail, 7)]);
        let (d, r) = group(nudge(&l, &sel, (0, 1), Step::Fine));
        assert_eq!(d, vec![(1, 103.0, 101.0)]);
        assert_eq!(r, vec![(7, 50.0, 401.0)]);
    }

    #[test]
    fn nothing_selected_or_no_direction_is_no_edit() {
        let l = layout();
        assert!(nudge(&l, &BTreeSet::new(), (1, 0), Step::Fine).is_none());
        assert!(nudge(&l, &BTreeSet::from([dev(1)]), (0, 0), Step::Fine).is_none());
        assert!(nudge(&l, &BTreeSet::from([dev(99)]), (1, 0), Step::Fine).is_none());
    }

    #[test]
    fn the_position_reads_offsets_and_free_space() {
        let p = position(&layout(), 1).unwrap();
        assert_eq!((p.x, p.y), (103.0, 100.0));
        assert!((p.to_right - (600.0 - 148.0)).abs() < 1e-9);
        assert!((p.to_bottom - (800.0 - 190.0)).abs() < 1e-9);
    }

    #[test]
    fn typing_one_axis_keeps_the_other_and_no_change_is_no_edit() {
        let l = layout();
        let (d, _) = group(set_position(&l, 1, Some(200.0), None));
        assert_eq!(d, vec![(1, 200.0, 100.0)]);
        assert!(set_position(&l, 1, Some(103.0), Some(100.0)).is_none());
        assert!(set_position(&l, 1, None, None).is_none());
    }

    #[test]
    fn typed_numbers_accept_a_comma_and_a_unit_and_refuse_the_rest() {
        assert_eq!(parse_mm("12,5"), Some(12.5));
        assert_eq!(parse_mm(" 40 mm "), Some(40.0));
        assert_eq!(parse_mm("-3"), Some(-3.0));
        assert_eq!(parse_mm("abc"), None);
        assert_eq!(parse_mm("NaN"), None);
        assert_eq!(parse_mm("inf"), None);
    }

    #[test]
    fn the_modifiers_pick_the_step() {
        let none = egui::Modifiers::NONE;
        assert_eq!(Step::from_modifiers(none), Step::Grid);
        assert_eq!(Step::from_modifiers(egui::Modifiers::SHIFT), Step::Fine);
        assert_eq!(Step::from_modifiers(egui::Modifiers::CTRL), Step::Coarse);
    }
}
