//! Rails and ducts by hand in the Cabinet: a list of every DIN rail and
//! duct on the plate where each can be added, moved, resized, turned and
//! deleted (every edit is one undo step), with the selected one outlined
//! on the plate. Place all adds rails and ducts by itself; this is for the
//! ones it does not know about and for tuning what it made.

use std::collections::HashMap;

use eframe::egui;
use novo_canvas::Viewport;
use novo_core::{Rail, RailId, RailKind, StructureId};
use novo_layout::rules::{DUCT_HEIGHT_MM, DUCT_WIDTH_MM};

/// A value of a rail the list edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Field {
    X,
    Y,
    Length,
    Width,
    Height,
}

/// What the list asked for.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum RailAsk {
    Add {
        kind: RailKind,
        vertical: bool,
    },
    /// New position and size, as shown after the edit.
    Set(RailEdit),
    /// Turn a rail or duct a quarter turn about its start.
    Turn(u64),
    /// Dragged on the plate: the rail's new start.
    MoveTo(u64, f64, f64),
    Delete(u64),
}

/// A rail with all the values the list can change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RailEdit {
    pub(super) id: u64,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) length: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

#[derive(Default)]
pub(super) struct RailsState {
    pub(super) open: bool,
    /// The rail or duct outlined on the plate; Delete removes it.
    pub(super) selected: Option<u64>,
    /// Values being dragged or typed, not yet committed.
    pending: HashMap<(u64, Field), f64>,
    /// A rail or duct being dragged across the plate.
    pub(super) drag: Option<RailDrag>,
    /// The rail or duct under the pointer, set by the canvas each frame.
    pub(super) hover: Option<u64>,
}

/// A rail or duct being dragged: where its start would land.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RailDrag {
    pub(super) id: u64,
    /// The pointer's offset from the start of the rail, in mm.
    pub(super) grab: (f64, f64),
    pub(super) at: (f64, f64),
}

/// The duct or rail under a point of the plate (mm), ducts first as they
/// lie over the rails. `slack` widens thin strips so they are easy to hit.
pub(super) fn pick(
    project: &novo_core::Project,
    structure: StructureId,
    x: f64,
    y: f64,
    slack: f64,
) -> Option<u64> {
    let hit = |r: &Rail| {
        let (across, along) = (across_of(r), r.length);
        let (w, h) = if r.vertical {
            (across, along)
        } else {
            (along, across)
        };
        let (left, top) = if r.vertical {
            (r.x - across / 2.0, r.y)
        } else {
            (r.x, r.y - across / 2.0)
        };
        x >= left - slack && x <= left + w + slack && y >= top - slack && y <= top + h + slack
    };
    let mut hits: Vec<&Rail> = project
        .all::<Rail>()
        .filter(|r| r.structure == structure && hit(r))
        .collect();
    hits.sort_by_key(|r| (r.kind != RailKind::Duct, std::cmp::Reverse(r.id.raw())));
    hits.first().map(|r| r.id.raw())
}

/// The grip at the start of a rail or duct, in mm: a tab outside its start
/// that stays grabbable when devices cover the rail. `(left, top, right,
/// bottom)`.
fn grip_of(r: &Rail, size: f64) -> (f64, f64, f64, f64) {
    let across = across_of(r).max(size);
    if r.vertical {
        (r.x - across / 2.0, r.y - size, r.x + across / 2.0, r.y)
    } else {
        (r.x - size, r.y - across / 2.0, r.x, r.y + across / 2.0)
    }
}

/// The rail or duct whose grip is under a point (mm); the grip wins over
/// devices on the rail.
pub(super) fn pick_grip(
    project: &novo_core::Project,
    structure: StructureId,
    x: f64,
    y: f64,
    slack: f64,
    size: f64,
) -> Option<u64> {
    project
        .all::<Rail>()
        .filter(|r| r.structure == structure)
        .filter(|r| {
            let (l, t, rt, b) = grip_of(r, size);
            x >= l - slack && x <= rt + slack && y >= t - slack && y <= b + slack
        })
        .max_by_key(|r| r.id.raw())
        .map(|r| r.id.raw())
}

/// Draws a grip on every rail and duct of the plate, brighter under the
/// pointer or when selected, and outlines the one under the pointer.
pub(super) fn draw_grips(
    painter: &egui::Painter,
    rect: egui::Rect,
    view: &Viewport,
    project: &novo_core::Project,
    structure: StructureId,
    (hover, selected): (Option<u64>, Option<u64>),
    size: f64,
) {
    let to_screen = |x: f64, y: f64| {
        let s = view.world_to_screen(novo_canvas::vec2(x as f32, y as f32));
        egui::pos2(rect.min.x + s.x, rect.min.y + s.y)
    };
    // Copper means "here": a grip at rest is a neutral handle, and turns
    // copper only under the pointer or when selected.
    let palette = novo_theme::Palette::of_ctx(painter.ctx());
    let accent = palette.accent;
    for r in project.all::<Rail>().filter(|r| r.structure == structure) {
        let id = r.id.raw();
        let (l, t, rt, b) = grip_of(r, size);
        let area = egui::Rect::from_two_pos(to_screen(l, t), to_screen(rt, b));
        let lit = hover == Some(id) || selected == Some(id);
        let fill = if lit {
            accent
        } else {
            palette.text_secondary.gamma_multiply(0.55)
        };
        painter.rect_filled(area, 2.0, fill);
        if hover == Some(id) && selected != Some(id) {
            let across = across_of(r);
            let (w, h) = if r.vertical {
                (across, r.length)
            } else {
                (r.length, across)
            };
            let (x, y) = if r.vertical {
                (r.x - across / 2.0, r.y)
            } else {
                (r.x, r.y - across / 2.0)
            };
            painter.rect_stroke(
                egui::Rect::from_two_pos(to_screen(x, y), to_screen(x + w, y + h)).expand(1.0),
                0.0,
                egui::Stroke::new(1.0, accent),
                egui::StrokeKind::Outside,
            );
        }
    }
}

/// How wide a rail or duct lies on the plate, across its run.
fn across_of(r: &Rail) -> f64 {
    match r.kind {
        RailKind::Duct if r.width > 0.0 => r.width,
        RailKind::Duct => novo_layout::DEFAULT_DUCT_MM,
        _ => 35.0,
    }
}

impl RailsState {
    /// The selected rail or duct, if it still exists; Delete removes it.
    pub(super) fn deletable(&self, project: &novo_core::Project) -> Option<u64> {
        self.selected
            .filter(|&id| project.get::<Rail>(RailId::from(id)).is_some())
    }
}

/// Where a new rail or duct goes: below everything on the plate for a
/// horizontal one, along the left edge for a vertical duct, kept on the
/// usable area. Rounded to 5 mm.
pub(super) fn new_rail_at(
    project: &novo_core::Project,
    structure: StructureId,
    kind: RailKind,
    vertical: bool,
) -> Option<(f64, f64, f64, f64, f64)> {
    let layout = novo_layout::model::load(project, structure)?;
    let usable = layout.plate.usable();
    let round = |v: f64| (v / 5.0).round() * 5.0;
    let (width, height) = match kind {
        RailKind::Duct => (DUCT_WIDTH_MM, DUCT_HEIGHT_MM),
        _ => (0.0, 0.0),
    };
    if vertical {
        let x = round(usable.x + width.max(35.0) / 2.0);
        return Some((x, usable.y, usable.h, width, height));
    }
    // Under the lowest device and rail.
    let mut bottom = usable.y;
    for p in &layout.placements {
        bottom = bottom.max(p.footprint().bottom());
    }
    for r in &layout.rails {
        if !r.vertical {
            bottom = bottom.max(r.y + 17.5);
        }
    }
    for d in &layout.ducts {
        if !d.vertical {
            bottom = bottom.max(d.y + d.width / 2.0);
        }
    }
    let half = if kind == RailKind::Duct { width } else { 35.0 } / 2.0;
    let y = round(bottom + 10.0 + half).min(usable.y + usable.h - half);
    Some((usable.x, y.max(usable.y + half), usable.w, width, height))
}

fn value(
    ui: &mut egui::Ui,
    state: &mut RailsState,
    id: u64,
    field: Field,
    current: f64,
    min: f64,
) -> Option<f64> {
    let mut v = state.pending.get(&(id, field)).copied().unwrap_or(current);
    let r = ui.add(
        egui::DragValue::new(&mut v)
            .speed(1.0)
            .range(min..=5000.0)
            .max_decimals(1)
            .suffix(" mm"),
    );
    if r.changed() {
        state.pending.insert((id, field), v);
    }
    // A drag commits when it ends; typing when the field loses focus.
    if r.drag_stopped() || r.lost_focus() {
        state.pending.remove(&(id, field));
        if (v - current).abs() > 1e-9 {
            return Some(v);
        }
    }
    None
}

/// The floating "Rails and ducts" list.
pub(super) fn show_window(
    ctx: &egui::Context,
    state: &mut RailsState,
    project: &novo_core::Project,
    structure: Option<StructureId>,
) -> Option<RailAsk> {
    if !state.open {
        return None;
    }
    let mut open = true;
    let mut ask = None;
    let window = super::panel::dialog(ctx, "Rails and ducts", [560.0, 0.0])
        .open(&mut open)
        .show(ctx, |ui| {
            let Some(structure) = structure else {
                ui.weak("This panel has no cabinet yet.");
                return;
            };
            if novo_layout::model::load(project, structure).is_none() {
                ui.weak("Set the mounting plate first.");
                return;
            }
            ui.horizontal(|ui| {
                if ui.button("Add DIN rail").clicked() {
                    ask = Some(RailAsk::Add {
                        kind: RailKind::DinRail,
                        vertical: false,
                    });
                }
                if ui.button("Add duct").clicked() {
                    ask = Some(RailAsk::Add {
                        kind: RailKind::Duct,
                        vertical: false,
                    });
                }
                if ui
                    .button("Add vertical duct")
                    .on_hover_text("A trunk that joins the ducts between the rows")
                    .clicked()
                {
                    ask = Some(RailAsk::Add {
                        kind: RailKind::Duct,
                        vertical: true,
                    });
                }
            });
            ui.weak(
                "Edit a value to move or resize. Delete removes the selected row. \
                 Ctrl+Z undoes each step.",
            );
            let mut rails: Vec<&Rail> = project
                .all::<Rail>()
                .filter(|r| r.structure == structure)
                .collect();
            rails.sort_by_key(|r| (r.kind != RailKind::DinRail, r.id.raw()));
            if rails.is_empty() {
                ui.add_space(6.0);
                ui.weak("No rails or ducts yet. Place all adds them, or add one above.");
                return;
            }
            egui::Grid::new("cabinet_rails_grid")
                .striped(true)
                .num_columns(8)
                .show(ui, |ui| {
                    for h in ["", "Left", "Top", "Length", "Across", "Stands", "", ""] {
                        ui.weak(h);
                    }
                    ui.end_row();
                    for r in rails {
                        let id = r.id.raw();
                        let duct = r.kind == RailKind::Duct;
                        let label = format!(
                            "{} {}{}",
                            if duct { "Duct" } else { "Rail" },
                            id,
                            if r.vertical { " |" } else { "" }
                        );
                        if ui
                            .selectable_label(state.selected == Some(id), label)
                            .clicked()
                        {
                            state.selected = (state.selected != Some(id)).then_some(id);
                        }
                        let mut edit = RailEdit {
                            id,
                            x: r.x,
                            y: r.y,
                            length: r.length,
                            width: r.width,
                            height: r.height,
                        };
                        let mut changed = false;
                        for (field, current, min) in [
                            (Field::X, r.x, -5000.0),
                            (Field::Y, r.y, -5000.0),
                            (Field::Length, r.length, 1.0),
                        ] {
                            if let Some(v) = value(ui, state, id, field, current, min) {
                                changed = true;
                                match field {
                                    Field::X => edit.x = v,
                                    Field::Y => edit.y = v,
                                    _ => edit.length = v,
                                }
                            }
                        }
                        if duct {
                            for (field, current) in
                                [(Field::Width, r.width), (Field::Height, r.height)]
                            {
                                if let Some(v) = value(ui, state, id, field, current, 5.0) {
                                    changed = true;
                                    if field == Field::Width {
                                        edit.width = v;
                                    } else {
                                        edit.height = v;
                                    }
                                }
                            }
                        } else {
                            ui.weak("35 mm");
                            ui.label("");
                        }
                        if ui
                            .small_button("Turn")
                            .on_hover_text("Swap between along and down the plate")
                            .clicked()
                        {
                            ask = Some(RailAsk::Turn(id));
                        }
                        if ui.small_button("Delete").clicked() {
                            ask = Some(RailAsk::Delete(id));
                        }
                        if changed {
                            ask = Some(RailAsk::Set(edit));
                        }
                        ui.end_row();
                    }
                });
        });
    if let Some(w) = window {
        super::cabinet_bar::note_rect(
            ctx,
            "cabinet.rails",
            "Rails and ducts",
            w.response.rect,
            false,
        );
    }
    state.open = open;
    ask
}

/// Outlines the selected rail or duct over the canvas.
pub(super) fn draw_selected(
    painter: &egui::Painter,
    rect: egui::Rect,
    view: &Viewport,
    project: &novo_core::Project,
    selected: u64,
    start: Option<(f64, f64)>,
) {
    let Some(r) = project.get::<Rail>(RailId::from(selected)) else {
        return;
    };
    let across = across_of(r);
    let (w, h) = if r.vertical {
        (across, r.length)
    } else {
        (r.length, across)
    };
    let (sx, sy) = start.unwrap_or((r.x, r.y));
    let (x, y) = if r.vertical {
        (sx - across / 2.0, sy)
    } else {
        (sx, sy - across / 2.0)
    };
    let to_screen = |x: f64, y: f64| {
        let s = view.world_to_screen(novo_canvas::vec2(x as f32, y as f32));
        egui::pos2(rect.min.x + s.x, rect.min.y + s.y)
    };
    let area = egui::Rect::from_two_pos(to_screen(x, y), to_screen(x + w, y + h)).expand(2.0);
    // The selected rail is "here": copper.
    let colour = novo_theme::Palette::of_ctx(painter.ctx()).accent;
    painter.rect_stroke(
        area,
        0.0,
        egui::Stroke::new(2.0, colour),
        egui::StrokeKind::Outside,
    );
}

impl super::NovoApp {
    /// Opens or closes the rails and ducts list in the Cabinet.
    pub(super) fn toggle_cabinet_rails(&mut self) {
        if self.backend.document().is_none() {
            return;
        }
        if self.workspace() != super::workspace::Workspace::Cabinet {
            self.switch_workspace(super::workspace::Workspace::Cabinet);
            self.cabinet.table = false;
            self.cabinet.rails.open = true;
        } else {
            self.cabinet.rails.open ^= true;
        }
    }

    /// Opens the plate card with its "Inside and door" section in view (the
    /// command `cabinet.door`; the only other way is Plate setup and a scroll).
    pub(super) fn open_cabinet_door(&mut self) {
        if self.backend.document().is_none() {
            return;
        }
        if self.workspace() != super::workspace::Workspace::Cabinet {
            self.switch_workspace(super::workspace::Workspace::Cabinet);
        }
        self.cabinet.table = false;
        self.cabinet.plate_card = true;
        self.cabinet.view.selection.clear();
        self.cabinet.door.reveal = Some("cabinet.door");
    }

    /// Applies what the list asked for as one undo step.
    pub(super) fn cabinet_rail_ask(&mut self, structure: Option<StructureId>, ask: RailAsk) {
        let Some(structure) = structure else {
            return;
        };
        match ask {
            RailAsk::Add { kind, vertical } => {
                let Some(doc) = self.backend.document() else {
                    return;
                };
                let Some((x, y, length, width, height)) =
                    new_rail_at(doc.project(), structure, kind, vertical)
                else {
                    return;
                };
                let label = if kind == RailKind::Duct {
                    "Add duct"
                } else {
                    "Add DIN rail"
                };
                let mut created = None;
                self.cabinet_edit(label, |tx| {
                    let out = tx.execute(novo_core::Command::AddRail {
                        structure,
                        x,
                        y,
                        length,
                        kind,
                        vertical,
                        width,
                        height,
                    })?;
                    if let Some(novo_core::AnyId::Rail(id)) = out.first() {
                        created = Some(id.raw());
                    }
                    Ok(())
                });
                if let Some(id) = created {
                    self.cabinet.rails.selected = Some(id);
                }
            }
            RailAsk::Set(e) => {
                let rail = RailId::from(e.id);
                self.cabinet_edit("Edit rail", move |tx| {
                    let Some(r) = tx.project().get::<Rail>(rail) else {
                        return Ok(());
                    };
                    let moved = (r.x, r.y) != (e.x, e.y);
                    let sized = (r.length, r.width, r.height) != (e.length, e.width, e.height);
                    if moved {
                        tx.execute(novo_core::Command::MoveRail {
                            rail,
                            x: e.x,
                            y: e.y,
                        })?;
                    }
                    if sized {
                        tx.execute(novo_core::Command::SetRailSize {
                            rail,
                            length: e.length,
                            width: e.width,
                            height: e.height,
                        })?;
                    }
                    Ok(())
                });
            }
            RailAsk::MoveTo(id, x, y) => {
                let rail = RailId::from(id);
                let unchanged = self
                    .backend
                    .document()
                    .and_then(|d| d.project().get::<Rail>(rail).map(|r| (r.x, r.y) == (x, y)))
                    .unwrap_or(true);
                if !unchanged {
                    self.cabinet_edit("Move rail", move |tx| {
                        tx.execute(novo_core::Command::MoveRail { rail, x, y })
                            .map(|_| ())
                    });
                }
            }
            RailAsk::Turn(id) => {
                let rail = RailId::from(id);
                self.cabinet_edit("Turn rail", move |tx| {
                    tx.update::<Rail>(rail, |r| r.vertical = !r.vertical)
                        .map(|_| ())
                });
            }
            RailAsk::Delete(id) => {
                let rail = RailId::from(id);
                self.cabinet_edit("Delete rail", move |tx| {
                    tx.execute(novo_core::Command::DeleteRail { rail })
                        .map(|_| ())
                });
                if self.cabinet.rails.selected == Some(id) {
                    self.cabinet.rails.selected = None;
                }
            }
        }
    }
}
