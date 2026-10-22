//! Moving things in the 3D view (View > Workspace > 3D): a left drag that
//! starts on a device, rail or duct moves it across the plate, R turns it and
//! Delete takes it off. The rules are the Cabinet canvas's own: the same
//! snap (`CabinetLayout::snapped`), the same checks (`CabinetLayout::report`)
//! and the same edits (`Ask::Drop`, `Rotate`, `Remove`, `RailAsk::MoveTo`,
//! `Turn`, `Delete`), so one undo step per edit and the plate and the 3D view
//! always show the same project.

use std::collections::BTreeSet;

use eframe::egui;
use novo_core::StructureId;
use novo_layout::Rect;
use novo_layout::state::CabinetLayout;
use novo_view3d::scene::{ItemId, ItemKind, PlateFrame};
use novo_view3d::widget::ItemDrag;
use novo_view3d::{Overlay, OverlayLines, OverlayMesh};

use super::NovoApp;
use super::cabinet::Ask;
use super::cabinet_rails::RailAsk;

/// A DIN rail's drawn width across its run, mm.
const RAIL_WIDTH_MM: f64 = 35.0;

/// An edit asked for in the 3D view, run through the Cabinet's own paths.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Edit {
    Device(DeviceEdit),
    Rail(RailEdit),
    /// Several devices, rails and ducts moved together: one undo step.
    Group {
        devices: Vec<(u64, f64, f64)>,
        rails: Vec<(u64, f64, f64)>,
    },
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
            Edit::Group { .. } => (None, None),
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
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Landing {
    pub item: ItemId,
    /// The snapped top left (devices) or start (rails and ducts).
    pub at: (f64, f64),
    /// The plate area it would cover.
    pub rect: Rect,
    /// The move breaks a rule the Cabinet also flags: it overlaps another
    /// device, leaves the plate or crowds a neighbour.
    pub clash: bool,
    /// The rail or duct the grabbed device lines up with (its edge snapped
    /// onto the rail's line).
    pub rail: Option<u64>,
    /// A device riding on a moved rail or duct: shown with it, but no edit of
    /// its own (the rail's move carries it).
    pub follower: bool,
    /// Devices a moved rail or duct would run into: they are not on it and
    /// their footprint overlaps its new place.
    pub blockers: Vec<u64>,
}

/// How close a device's top left gets to a rail's line before it snaps onto
/// it, mm.
pub(super) const RAIL_SNAP_MM: f64 = 8.0;

/// The drag in progress, kept from one frame to the next.
#[derive(Debug, Default)]
pub(super) struct MoveState {
    active: Option<Active>,
}

#[derive(Debug, Clone)]
struct Active {
    /// The item under the press.
    grabbed: ItemId,
    /// Everything that moves with it, with where each started.
    members: Vec<(ItemId, (f64, f64))>,
    landings: Vec<Landing>,
}

/// The top left of a device, or the start of a rail or duct.
pub(super) fn origin(layout: &CabinetLayout, item: ItemId) -> Option<(f64, f64)> {
    match item.kind {
        // A device on the door or another surface is not on the plate the
        // drag works on; move it where its surface is shown.
        ItemKind::Device => layout
            .placements
            .iter()
            .find(|p| p.id == item.raw && !layout.off_plate.contains_key(&p.id))
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
pub(super) fn area(layout: &CabinetLayout, item: ItemId, at: (f64, f64)) -> Option<Rect> {
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

/// A device's top left pulled onto a rail or duct line it is next to. A
/// device sits on a DIN rail with the rail through its middle, so a
/// horizontal rail sets the device's vertical centre and a vertical one its
/// horizontal centre. A duct has no devices on it and keeps the old rule: a
/// horizontal one sets the top, a vertical one the left. The snap only
/// applies along the line's run, so a device never jumps to a far rail.
pub(super) fn rail_snap(
    layout: &CabinetLayout,
    device: u64,
    at: (f64, f64),
) -> ((f64, f64), Option<u64>) {
    let Some(p) = layout.placements.iter().find(|p| p.id == device) else {
        return (at, None);
    };
    let f = p.footprint();
    // (id, x, y, length, vertical, share of the device's size that lies
    // before its centre: a half for a rail, none for a duct)
    let lines = layout
        .rails
        .iter()
        .map(|r| (r.id, r.x, r.y, r.length, r.vertical, 0.5))
        .chain(
            layout
                .ducts
                .iter()
                .map(|d| (d.id, d.x, d.y, d.length, d.vertical, 0.0)),
        );
    let mut best: Option<(f64, u64, (f64, f64))> = None;
    for (id, x, y, length, vertical, centred) in lines {
        let (gap, snapped) = if vertical {
            let along = at.1 + f.h > y && at.1 < y + length;
            let left = x - f.w * centred;
            ((at.0 - left).abs(), along.then_some((left, at.1)))
        } else {
            let along = at.0 + f.w > x && at.0 < x + length;
            let top = y - f.h * centred;
            ((at.1 - top).abs(), along.then_some((at.0, top)))
        };
        if gap <= RAIL_SNAP_MM
            && let Some(to) = snapped
            && best.is_none_or(|(g, ..)| gap < g)
        {
            best = Some((gap, id, to));
        }
    }
    best.map_or((at, None), |(_, id, to)| (to, Some(id)))
}

/// Where `members` land when the pointer has moved `delta` mm on the plate
/// since the press. The grabbed item is snapped (to the grid, and a device
/// onto a nearby rail); the others keep their distance from it, so a
/// selection moves as one piece. `None` when the grabbed item is not on the
/// plate.
fn landings(
    layout: &CabinetLayout,
    grabbed: ItemId,
    members: &[(ItemId, (f64, f64))],
    delta: (f64, f64),
) -> Option<Vec<Landing>> {
    let from = members.iter().find(|(i, _)| *i == grabbed)?.1;
    let raw = layout.snapped(from.0 + delta.0, from.1 + delta.1);
    let (target, rail) = match grabbed.kind {
        ItemKind::Device => rail_snap(layout, grabbed.raw, raw),
        _ => (raw, None),
    };
    let shift = (target.0 - from.0, target.1 - from.1);
    let mut moved = layout.clone();
    let mut out: Vec<(ItemId, (f64, f64), bool)> = Vec::new();
    for (item, start) in members {
        let at = (start.0 + shift.0, start.1 + shift.1);
        match item.kind {
            ItemKind::Device => {
                moved.move_to(item.raw, at.0, at.1);
                // `move_to` snaps to the grid; keep the exact (rail) position.
                if let Some(p) = moved.placements.iter_mut().find(|p| p.id == item.raw) {
                    (p.x, p.y) = at;
                }
            }
            ItemKind::Rail => {
                if let Some(r) = moved.rails.iter_mut().find(|r| r.id == item.raw) {
                    (r.x, r.y) = at;
                }
            }
            ItemKind::Duct => {
                if let Some(d) = moved.ducts.iter_mut().find(|d| d.id == item.raw) {
                    (d.x, d.y) = at;
                }
            }
            _ => {}
        }
        out.push((*item, at, false));
    }
    // Devices mounted on a moved rail or duct ride along (the model's
    // `MoveRail` carries them), unless they are moved on their own.
    let carried: Vec<(u64, (f64, f64))> = members
        .iter()
        .filter(|(i, _)| matches!(i.kind, ItemKind::Rail | ItemKind::Duct))
        .map(|(i, o)| (i.raw, shift_of(&out, *i, *o)))
        .collect();
    for p in &layout.placements {
        let Some(rail) = p.rail else { continue };
        let Some((_, d)) = carried.iter().find(|(r, _)| *r == rail) else {
            continue;
        };
        if members
            .iter()
            .any(|(i, _)| i.kind == ItemKind::Device && i.raw == p.id)
        {
            continue;
        }
        let at = (p.x + d.0, p.y + d.1);
        if let Some(m) = moved.placements.iter_mut().find(|m| m.id == p.id) {
            (m.x, m.y) = at;
        }
        out.push((
            ItemId {
                kind: ItemKind::Device,
                raw: p.id,
            },
            at,
            true,
        ));
    }
    let report = moved.report();
    let involved = |id: u64| {
        report.collisions.iter().any(|(a, b)| *a == id || *b == id)
            || report.outside.contains(&id)
            || report
                .too_close
                .iter()
                .any(|(a, b, _)| *a == id || *b == id)
    };
    let riding = |id: u64| {
        out.iter()
            .any(|(i, _, _)| i.kind == ItemKind::Device && i.raw == id)
    };
    let shaky_follower = out.iter().any(|(i, _, f)| *f && involved(i.raw));
    out.iter()
        .map(|(item, at, follower)| {
            let rect = area(layout, *item, *at)?;
            let mut blockers = Vec::new();
            let clash = match item.kind {
                ItemKind::Device => involved(item.raw),
                _ => {
                    // Things not moving with it that sit where it lands.
                    let on_it = |p: &novo_layout::Placement| p.rail == Some(item.raw);
                    blockers = moved
                        .placements
                        .iter()
                        .filter(|p| !on_it(p) && !riding(p.id))
                        .filter(|p| p.footprint().overlaps(&rect))
                        .map(|p| p.id)
                        .collect();
                    rect.x < 0.0
                        || rect.y < 0.0
                        || rect.right() > layout.plate.width
                        || rect.bottom() > layout.plate.height
                        || !blockers.is_empty()
                        || shaky_follower
                }
            };
            Some(Landing {
                item: *item,
                at: *at,
                rect,
                clash,
                rail: (*item == grabbed).then_some(rail).flatten(),
                follower: *follower,
                blockers,
            })
        })
        .collect()
}

/// How far `item` has moved from `start` in a finished landing list.
fn shift_of(out: &[(ItemId, (f64, f64), bool)], item: ItemId, start: (f64, f64)) -> (f64, f64) {
    out.iter()
        .find(|(i, _, _)| *i == item)
        .map_or((0.0, 0.0), |(_, at, _)| (at.0 - start.0, at.1 - start.1))
}

/// Where `item` lands when the pointer has moved `delta` mm on the plate
/// since the press. `None` for something that is not on the plate.
#[cfg(test)]
pub(super) fn landing(layout: &CabinetLayout, item: ItemId, delta: (f64, f64)) -> Option<Landing> {
    let from = origin(layout, item)?;
    landings(layout, item, &[(item, from)], delta)?.pop()
}

impl MoveState {
    /// One frame of the drag the view reported. `selection` is what is
    /// selected: pressing on a selected item moves the whole selection. Returns
    /// the landings to draw and, when the button went up on a new place, the
    /// edit to run.
    pub(super) fn step(
        &mut self,
        layout: &CabinetLayout,
        selection: &BTreeSet<ItemId>,
        drag: Option<ItemDrag>,
    ) -> (Vec<Landing>, Option<Edit>) {
        let Some(drag) = drag else {
            self.active = None;
            return (Vec::new(), None);
        };
        let members = match &self.active {
            Some(a) if a.grabbed == drag.id => a.members.clone(),
            _ => {
                let mut picked: Vec<ItemId> = if selection.contains(&drag.id) {
                    selection.iter().copied().collect()
                } else {
                    vec![drag.id]
                };
                if !picked.contains(&drag.id) {
                    picked.push(drag.id);
                }
                picked
                    .into_iter()
                    .filter_map(|i| origin(layout, i).map(|o| (i, o)))
                    .collect()
            }
        };
        let Some(land) = landings(layout, drag.id, &members, drag.delta) else {
            self.active = None;
            return (Vec::new(), None);
        };
        if !drag.done {
            self.active = Some(Active {
                grabbed: drag.id,
                members,
                landings: land.clone(),
            });
            return (land, None);
        }
        self.active = None;
        let moved: Vec<&Landing> = land
            .iter()
            .filter(|l| !l.follower)
            .filter(|l| members.iter().any(|(i, o)| *i == l.item && *o != l.at))
            .collect();
        let edit = match moved.as_slice() {
            [] => None,
            [one] if one.item.kind == ItemKind::Device => Some(Edit::Device(DeviceEdit::Drop(
                one.item.raw,
                one.at.0,
                one.at.1,
            ))),
            [one] => Some(Edit::Rail(RailEdit::MoveTo(
                one.item.raw,
                one.at.0,
                one.at.1,
            ))),
            many => Some(Edit::Group {
                devices: many
                    .iter()
                    .filter(|l| l.item.kind == ItemKind::Device)
                    .map(|l| (l.item.raw, l.at.0, l.at.1))
                    .collect(),
                rails: many
                    .iter()
                    .filter(|l| l.item.kind != ItemKind::Device)
                    .map(|l| (l.item.raw, l.at.0, l.at.1))
                    .collect(),
            }),
        };
        (Vec::new(), edit)
    }

    /// Whether something is being dragged now.
    pub(super) fn dragging(&self) -> bool {
        self.active.is_some()
    }

    /// The landings of the drag in progress.
    pub(super) fn landings(&self) -> Vec<Landing> {
        self.active
            .as_ref()
            .map_or_else(Vec::new, |a| a.landings.clone())
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

impl NovoApp {
    /// Runs a group move as one undo step: devices keep their turn, rails and
    /// ducts move by the Cabinet's own command. A pending Place all preview
    /// is accepted first, like any plate edit.
    pub(super) fn view3d_move_group(
        &mut self,
        structure: StructureId,
        devices: Vec<(u64, f64, f64)>,
        rails: Vec<(u64, f64, f64)>,
    ) {
        if self.cabinet.preview.is_some() {
            self.cabinet_ask(Ask::Accept, Some(structure), None, &[]);
        }
        let layout = self
            .backend
            .document()
            .and_then(|d| novo_layout::model::load(d.project(), structure));
        let Some(layout) = layout else {
            return;
        };
        let moves_devices = !devices.is_empty();
        let done = self.cabinet_edit("Move selection", move |tx| {
            // Rails first: the move carries what is mounted on them, then
            // the devices of the group are set to their exact places.
            for (id, x, y) in rails {
                tx.execute(novo_core::Command::MoveRail {
                    rail: novo_core::RailId::from(id),
                    x,
                    y,
                })?;
            }
            for (id, x, y) in devices {
                let rotated = layout
                    .placements
                    .iter()
                    .find(|p| p.id == id)
                    .is_some_and(|p| p.rotated);
                novo_layout::model::place_device(tx, structure, id, x, y, rotated)?;
            }
            Ok(())
        });
        // A dragged group counts as a drop for the lessons, like a single one.
        if done && moves_devices {
            self.learn_signal("cabinet_dropped");
        }
    }
}

/// What a dragged rail or duct would run into, in words: the devices in its
/// way by tag, or that it leaves the plate, or that a device riding on it
/// would collide. `None` when nothing is wrong with it. Devices dragged on
/// their own are only coloured red.
pub(super) fn warning(
    layout: &CabinetLayout,
    landings: &[Landing],
    tag_of: &dyn Fn(u64) -> String,
) -> Option<String> {
    let l = landings
        .iter()
        .find(|l| l.clash && matches!(l.item.kind, ItemKind::Rail | ItemKind::Duct))?;
    let name = if l.item.kind == ItemKind::Rail {
        "This rail"
    } else {
        "This duct"
    };
    if !l.blockers.is_empty() {
        let mut tags: Vec<String> = l.blockers.iter().map(|d| tag_of(*d)).collect();
        tags.sort();
        tags.dedup();
        let shown = tags
            .iter()
            .take(4)
            .map(|t| if t.is_empty() { "a device" } else { t.as_str() })
            .collect::<Vec<_>>()
            .join(", ");
        let more = tags.len().saturating_sub(4);
        return Some(if more > 0 {
            format!("{name} would run into {shown} and {more} more")
        } else {
            format!("{name} would run into {shown}")
        });
    }
    let r = l.rect;
    if r.x < 0.0 || r.y < 0.0 || r.right() > layout.plate.width || r.bottom() > layout.plate.height
    {
        return Some(format!("{name} would leave the plate"));
    }
    Some("A device on it would collide, leave the plate or crowd a neighbour".to_owned())
}

/// The warning as a small card by the pointer, red like the ghost.
pub(super) fn show_warning(ctx: &egui::Context, text: &str) {
    let Some(at) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    egui::Area::new(egui::Id::new("view3d.move_warning"))
        .order(egui::Order::Tooltip)
        .fixed_pos(at + egui::vec2(16.0, 16.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new(text).color(crate::theme::ui_palette().error));
            });
        });
}

/// The landing box as overlays: a translucent slab with an outline, blue
/// where the move is fine and red where the Cabinet would flag it.
pub(super) fn ghost(land: &Landing, frame: &PlateFrame, top_mm: f64) -> Vec<Overlay> {
    let (fill, edge): ([u8; 4], [u8; 4]) = if land.clash {
        ([230, 70, 70, 90], [240, 90, 90, 255])
    } else if land.rail.is_some() {
        // Lined up with a rail or duct.
        ([80, 200, 130, 80], [110, 225, 155, 255])
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
