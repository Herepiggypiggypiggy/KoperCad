//! Moving things in the 3D view (View > Workspace > 3D): a left drag that
//! starts on a device, rail or duct moves it across the plate, R turns it and
//! Delete takes it off. The rules are the Cabinet canvas's own: the same
//! snap (`CabinetLayout::snapped`), the same checks (`CabinetLayout::report`)
//! and the same edits (`Ask::Drop`, `Rotate`, `Remove`, `RailAsk::MoveTo`,
//! `Turn`, `Delete`), so one undo step per edit and the plate and the 3D view
//! always show the same project.

use std::collections::BTreeSet;

use novo_layout::Rect;
use novo_layout::state::CabinetLayout;
use novo_view3d::scene::{ItemId, ItemKind, PlateFrame};
use novo_view3d::widget::ItemDrag;
use novo_view3d::{Overlay, OverlayLines, OverlayMesh};

use super::cabinet::Ask;
use super::cabinet_rails::RailAsk;

/// A DIN rail's drawn width across its run, mm.
const RAIL_WIDTH_MM: f64 = 35.0;

/// An edit asked for in the 3D view, run through the Cabinet's own paths.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Edit {
    Device(DeviceEdit),
    Rail(RailEdit),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum DeviceEdit {
    Drop(u64, f64, f64),
    Rotate(Vec<u64>),
    Remove(Vec<u64>),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum RailEdit {
    MoveTo(u64, f64, f64),
    Turn(u64),
    Delete(u64),
}

impl Edit {
    pub(super) fn ask(self) -> (Option<Ask>, Option<RailAsk>) {
        match self {
            Edit::Device(DeviceEdit::Drop(d, x, y)) => (Some(Ask::Drop(d, x, y)), None),
            Edit::Device(DeviceEdit::Rotate(d)) => (Some(Ask::Rotate(d)), None),
            Edit::Device(DeviceEdit::Remove(d)) => (Some(Ask::Remove(d)), None),
            Edit::Rail(RailEdit::MoveTo(r, x, y)) => (None, Some(RailAsk::MoveTo(r, x, y))),
            Edit::Rail(RailEdit::Turn(r)) => (None, Some(RailAsk::Turn(r))),
            Edit::Rail(RailEdit::Delete(r)) => (None, Some(RailAsk::Delete(r))),
        }
    }
}

/// Where a dragged item would land.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Landing {
    pub item: ItemId,
    /// The snapped top left (devices) or start (rails and ducts).
    pub at: (f64, f64),
    /// The plate area it would cover.
    pub rect: Rect,
    /// The move breaks a rule the Cabinet also flags: it overlaps another
    /// device, leaves the plate or crowds a neighbour.
    pub clash: bool,
}

/// The drag in progress, kept from one frame to the next.
#[derive(Debug, Default)]
pub(super) struct MoveState {
    /// Dragging in the view moves things (the Move tool). On by default.
    pub(super) off: bool,
    active: Option<Active>,
}

#[derive(Debug, Clone, Copy)]
struct Active {
    item: ItemId,
    /// Top left or start when the drag began.
    from: (f64, f64),
    landing: Landing,
}

/// The top left of a device, or the start of a rail or duct.
fn origin(layout: &CabinetLayout, item: ItemId) -> Option<(f64, f64)> {
    match item.kind {
        ItemKind::Device => layout
            .placements
            .iter()
            .find(|p| p.id == item.raw)
            .map(|p| (p.x, p.y)),
        ItemKind::Rail => layout
            .rails
            .iter()
            .find(|r| r.id == item.raw)
            .map(|r| (r.x, r.y)),
        ItemKind::Duct => layout
            .ducts
            .iter()
            .find(|d| d.id == item.raw)
            .map(|d| (d.x, d.y)),
        _ => None,
    }
}

/// The plate area an item covers when its origin is at `at`.
fn area(layout: &CabinetLayout, item: ItemId, at: (f64, f64)) -> Option<Rect> {
    Some(match item.kind {
        ItemKind::Device => {
            let f = layout
                .placements
                .iter()
                .find(|p| p.id == item.raw)?
                .footprint();
            Rect {
                x: at.0,
                y: at.1,
                ..f
            }
        }
        ItemKind::Rail => {
            let r = layout.rails.iter().find(|r| r.id == item.raw)?;
            let w = RAIL_WIDTH_MM;
            if r.vertical {
                Rect {
                    x: at.0 - w / 2.0,
                    y: at.1,
                    w,
                    h: r.length,
                }
            } else {
                Rect {
                    x: at.0,
                    y: at.1 - w / 2.0,
                    w: r.length,
                    h: w,
                }
            }
        }
        ItemKind::Duct => {
            let mut d = layout.ducts.iter().find(|d| d.id == item.raw)?.clone();
            (d.x, d.y) = at;
            d.footprint()
        }
        _ => return None,
    })
}

/// Where `item` lands when the pointer has moved `delta` mm on the plate
/// since the press, snapped like the Cabinet's drag. `None` for something
/// that is not on the plate.
#[cfg(test)]
pub(super) fn landing(layout: &CabinetLayout, item: ItemId, delta: (f64, f64)) -> Option<Landing> {
    let from = origin(layout, item)?;
    landing_from(layout, item, from, delta)
}

fn landing_from(
    layout: &CabinetLayout,
    item: ItemId,
    from: (f64, f64),
    delta: (f64, f64),
) -> Option<Landing> {
    let at = layout.snapped(from.0 + delta.0, from.1 + delta.1);
    let rect = area(layout, item, at)?;
    let clash = match item.kind {
        ItemKind::Device => {
            let mut moved = layout.clone();
            moved.move_to(item.raw, at.0, at.1);
            let r = moved.report();
            let id = item.raw;
            r.collisions.iter().any(|(a, b)| *a == id || *b == id)
                || r.outside.contains(&id)
                || r.too_close.iter().any(|(a, b, _)| *a == id || *b == id)
        }
        _ => {
            rect.x < 0.0
                || rect.y < 0.0
                || rect.right() > layout.plate.width
                || rect.bottom() > layout.plate.height
        }
    };
    Some(Landing {
        item,
        at,
        rect,
        clash,
    })
}

impl MoveState {
    /// One frame of the drag the view reported. Returns the landing to draw
    /// and, when the button went up on a new place, the edit to run.
    pub(super) fn step(
        &mut self,
        layout: &CabinetLayout,
        drag: Option<ItemDrag>,
    ) -> (Option<Landing>, Option<Edit>) {
        let Some(drag) = drag else {
            self.active = None;
            return (None, None);
        };
        let held = match self.active {
            Some(a) if a.item == drag.id => a.from,
            _ => match origin(layout, drag.id) {
                Some(from) => from,
                None => {
                    self.active = None;
                    return (None, None);
                }
            },
        };
        let Some(land) = landing_from(layout, drag.id, held, drag.delta) else {
            self.active = None;
            return (None, None);
        };
        if !drag.done {
            self.active = Some(Active {
                item: drag.id,
                from: held,
                landing: land,
            });
            return (Some(land), None);
        }
        self.active = None;
        let moved = land.at != held;
        let edit = moved.then_some(match drag.id.kind {
            ItemKind::Device => Edit::Device(DeviceEdit::Drop(drag.id.raw, land.at.0, land.at.1)),
            _ => Edit::Rail(RailEdit::MoveTo(drag.id.raw, land.at.0, land.at.1)),
        });
        (None, edit)
    }

    /// Whether the item is being dragged now.
    pub(super) fn dragging(&self) -> bool {
        self.active.is_some()
    }

    /// The landing of the drag in progress.
    pub(super) fn landing(&self) -> Option<Landing> {
        self.active.map(|a| a.landing)
    }
}

/// The keys the shell left for the view (`edit.rotate`, `edit.delete`) as
/// edits of what is selected: devices first, else the selected rails and
/// ducts. Ids that are not on the plate are left out.
pub(super) fn key_edits(
    keys: &[String],
    selection: &BTreeSet<ItemId>,
    layout: &CabinetLayout,
) -> Vec<Edit> {
    let devices: Vec<u64> = selection
        .iter()
        .filter(|i| i.kind == ItemKind::Device)
        .map(|i| i.raw)
        .filter(|d| layout.placements.iter().any(|p| p.id == *d))
        .collect();
    let rails: Vec<u64> = selection
        .iter()
        .filter(|i| matches!(i.kind, ItemKind::Rail | ItemKind::Duct))
        .map(|i| i.raw)
        .filter(|r| {
            layout.rails.iter().any(|x| x.id == *r) || layout.ducts.iter().any(|x| x.id == *r)
        })
        .collect();
    let mut out = Vec::new();
    for key in keys {
        match key.as_str() {
            "edit.rotate" if !devices.is_empty() => {
                out.push(Edit::Device(DeviceEdit::Rotate(devices.clone())));
            }
            "edit.rotate" => out.extend(rails.iter().map(|r| Edit::Rail(RailEdit::Turn(*r)))),
            "edit.delete" if !devices.is_empty() => {
                out.push(Edit::Device(DeviceEdit::Remove(devices.clone())));
            }
            "edit.delete" => out.extend(rails.iter().map(|r| Edit::Rail(RailEdit::Delete(*r)))),
            _ => {}
        }
    }
    out
}

/// The landing box as overlays: a translucent slab with an outline, blue
/// where the move is fine and red where the Cabinet would flag it.
pub(super) fn ghost(land: &Landing, frame: &PlateFrame, top_mm: f64) -> Vec<Overlay> {
    let (fill, edge): ([u8; 4], [u8; 4]) = if land.clash {
        ([230, 70, 70, 90], [240, 90, 90, 255])
    } else {
        ([90, 160, 255, 80], [120, 180, 255, 255])
    };
    let r = land.rect;
    let corners = [
        (r.x, r.y),
        (r.right(), r.y),
        (r.right(), r.bottom()),
        (r.x, r.bottom()),
    ];
    let z = top_mm.max(1.0);
    let top: Vec<_> = corners.iter().map(|c| frame.world(c.0, c.1, z)).collect();
    let mesh = OverlayMesh {
        positions: top.clone(),
        colors: vec![fill; 4],
        indices: vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2],
        translucent: true,
        order: 50,
    };
    let mut line = top;
    line.push(line[0]);
    let lines = OverlayLines {
        colors: vec![edge; line.len()],
        positions: line,
        starts: Vec::new(),
        width_px: 2.0,
        order: 51,
    };
    vec![Overlay::Mesh(mesh), Overlay::Lines(lines)]
}
