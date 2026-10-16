//! Typing a rail or duct's place and size in the 3D view: select one and a
//! card by the corner shows its Left, Top, Length (and for a duct Across and
//! Stands) in millimetres. A value commits when the field loses focus, as one
//! undo step through the Cabinet's own `RailAsk::Set` (the list's edit); a
//! rail that moves carries the devices mounted on it. While a value is being
//! typed the card says what the result would run into, the way a drag does.

use eframe::egui;
use novo_core::{Rail, RailId, RailKind};
use novo_layout::state::CabinetLayout;
use novo_layout::{DEFAULT_DUCT_MM, rail_used};

use super::cabinet_rails::{Field, RailAsk, RailEdit, RailsState, value};

/// A DIN rail's drawn width across its run, mm.
const RAIL_ACROSS_MM: f64 = 35.0;

/// What a rail or duct is now, from the project.
pub(super) fn current(project: &novo_core::Project, id: u64) -> Option<(RailEdit, RailKind, bool)> {
    let r = project.get::<Rail>(RailId::from(id))?;
    Some((
        RailEdit {
            id,
            x: r.x,
            y: r.y,
            length: r.length,
            width: r.width,
            height: r.height,
        },
        r.kind,
        r.vertical,
    ))
}

/// What `edit` would run into, in words; empty when it is fine. Devices
/// mounted on the rail move with it, the others must stay clear.
pub(super) fn problems(
    layout: &CabinetLayout,
    edit: &RailEdit,
    kind: RailKind,
    vertical: bool,
    tag_of: &dyn Fn(u64) -> String,
) -> Vec<String> {
    let duct = kind == RailKind::Duct;
    let (old_x, old_y) = layout
        .rails
        .iter()
        .find(|r| r.id == edit.id)
        .map(|r| (r.x, r.y))
        .or_else(|| {
            layout
                .ducts
                .iter()
                .find(|d| d.id == edit.id)
                .map(|d| (d.x, d.y))
        })
        .unwrap_or((edit.x, edit.y));
    let across = match (duct, edit.width) {
        (true, w) if w > 0.0 => w,
        (true, _) => DEFAULT_DUCT_MM,
        _ => RAIL_ACROSS_MM,
    };
    let (w, h) = if vertical {
        (across, edit.length)
    } else {
        (edit.length, across)
    };
    let (left, top) = if vertical {
        (edit.x - across / 2.0, edit.y)
    } else {
        (edit.x, edit.y - across / 2.0)
    };
    let rect = novo_layout::Rect {
        x: left,
        y: top,
        w,
        h,
    };
    let (dx, dy) = (edit.x - old_x, edit.y - old_y);
    let mut moved = layout.clone();
    if let Some(r) = moved.rails.iter_mut().find(|r| r.id == edit.id) {
        (r.x, r.y, r.length) = (edit.x, edit.y, edit.length);
    }
    if let Some(d) = moved.ducts.iter_mut().find(|d| d.id == edit.id) {
        (d.x, d.y, d.length, d.width) = (edit.x, edit.y, edit.length, across);
    }
    let mut riders = Vec::new();
    for p in &mut moved.placements {
        if p.rail == Some(edit.id) {
            riders.push(p.id);
            p.x += dx;
            p.y += dy;
        }
    }
    let mut out = Vec::new();
    if rect.x < 0.0
        || rect.y < 0.0
        || rect.right() > layout.plate.width
        || rect.bottom() > layout.plate.height
    {
        out.push("It would leave the plate".to_owned());
    }
    if !duct && let Some(rail) = moved.rails.iter().find(|r| r.id == edit.id) {
        let on: Vec<_> = moved
            .placements
            .iter()
            .filter(|p| riders.contains(&p.id))
            .cloned()
            .collect();
        let used = rail_used(rail, &on);
        if used > edit.length + 1e-6 {
            out.push(format!(
                "Its devices need {used:.0} mm, the rail would be {:.0} mm",
                edit.length
            ));
        }
    }
    let mut tags: Vec<String> = moved
        .placements
        .iter()
        .filter(|p| !riders.contains(&p.id) && p.footprint().overlaps(&rect))
        .map(|p| tag_of(p.id))
        .map(|t| if t.is_empty() { "a device".into() } else { t })
        .collect();
    tags.sort();
    tags.dedup();
    if !tags.is_empty() {
        out.push(format!("It would run into {}", tags.join(", ")));
    }
    let report = moved.report();
    let hit = |id: u64| riders.contains(&id);
    if report.collisions.iter().any(|(a, b)| hit(*a) || hit(*b))
        || report.outside.iter().any(|d| hit(*d))
        || report.too_close.iter().any(|(a, b, _)| hit(*a) || hit(*b))
    {
        out.push("A device on it would collide, leave the plate or crowd a neighbour".to_owned());
    }
    out
}

/// The card for the one selected rail or duct; returns the edit to run when a
/// value was committed, or a turn.
pub(super) fn card(
    ctx: &egui::Context,
    (view_rect, bottom_room): (egui::Rect, f32),
    state: &mut RailsState,
    project: &novo_core::Project,
    layout: &CabinetLayout,
    id: u64,
    tag_of: &dyn Fn(u64) -> String,
) -> Option<RailAsk> {
    let (now, kind, vertical) = current(project, id)?;
    let duct = kind == RailKind::Duct;
    let mut ask = None;
    // Left, under the toolbar, like the Inspector it stands in for (a rail
    // has none): the right edge is the view cube and the Heat panel, the
    // bottom left the statistics.
    let title = format!(
        "{} {}{}",
        if duct { "Duct" } else { "DIN rail" },
        id,
        if vertical { " (down)" } else { "" }
    );
    super::panel::Panel::new(
        "view3d_rail",
        title,
        view_rect,
        super::panel::Side::Left,
        240.0,
    )
    .fit()
    .fixed_open()
    .top(super::view3d_controls::BELOW)
    .bottom_room(bottom_room)
    .show(ctx, |ui| {
        let on = layout
            .placements
            .iter()
            .filter(|p| p.rail == Some(id))
            .count();
        let mut edit = now;
        let mut changed = false;
        // Two values to a row keep the card short enough to sit above the
        // statistics on a small window.
        let mut fields = vec![
            ("Left", Field::X, now.x, -5000.0),
            ("Top", Field::Y, now.y, -5000.0),
            ("Length", Field::Length, now.length, 1.0),
        ];
        if duct {
            fields.push(("Across", Field::Width, now.width, 5.0));
            fields.push(("Stands", Field::Height, now.height, 5.0));
        }
        egui::Grid::new("view3d_rail_grid")
            .num_columns(4)
            .show(ui, |ui| {
                for (i, (label, field, current, min)) in fields.into_iter().enumerate() {
                    ui.label(label);
                    if let Some(v) = value(ui, state, id, field, current, min) {
                        changed = true;
                        match field {
                            Field::X => edit.x = v,
                            Field::Y => edit.y = v,
                            Field::Length => edit.length = v,
                            Field::Width => edit.width = v,
                            Field::Height => edit.height = v,
                        }
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
                if ui
                    .small_button("Turn")
                    .on_hover_text("Swap between along and down the plate (R)")
                    .clicked()
                {
                    ask = Some(RailAsk::Turn(id));
                }
            });
        if on > 0 {
            ui.weak(format!(
                "Carries {on} device{}; they move with it",
                if on == 1 { "" } else { "s" }
            ));
        }
        // What the typed (not yet committed) values would do.
        let typed = RailEdit {
            id,
            x: state.typed(id, Field::X).unwrap_or(now.x),
            y: state.typed(id, Field::Y).unwrap_or(now.y),
            length: state.typed(id, Field::Length).unwrap_or(now.length),
            width: state.typed(id, Field::Width).unwrap_or(now.width),
            height: state.typed(id, Field::Height).unwrap_or(now.height),
        };
        for line in problems(layout, &typed, kind, vertical, tag_of) {
            ui.colored_label(crate::theme::ui_palette().error, line);
        }
        if changed {
            ask = Some(RailAsk::Set(edit));
        }
    });
    ask
}
