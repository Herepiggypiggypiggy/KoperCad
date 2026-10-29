//! Rails and ducts by their ends in the Cabinet: two handles on the selected
//! one to drag either end (the other end stays), a trim at a crossing or at
//! a point, a cut to a stock length, and the length and the bar it is cut
//! from shown beside it. Every action is one undo step. The geometry and the
//! rules (snapping, no overlap, devices on a rail) are `novo_layout::railedit`.

use eframe::egui;
use novo_canvas::Viewport;
use novo_core::{Rail, RailId, StructureId};
use novo_layout::railedit::{self, Edit, End, SnapTo, Stop};
use novo_layout::stock::Stock;

use super::cabinet_rails::{RailAsk, RailsState, across_of};

/// An end being dragged: the piece it would leave.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResizeDrag {
    pub(super) id: u64,
    pub(super) end: End,
    pub(super) edit: Option<Edit>,
}

/// The handle size in mm at this zoom: 12 screen pixels, never under 6 mm.
pub(super) fn handle_size(zoom: f64) -> f64 {
    (12.0 / zoom).max(6.0)
}

/// The handle of one end, inside the rail or duct: `(left, top, right,
/// bottom)` in mm.
pub(super) fn handle_of(r: &Rail, end: End, size: f64) -> (f64, f64, f64, f64) {
    let size = size.min(r.length / 3.0).max(1.0);
    let across = across_of(r).max(size);
    let (from, to) = match end {
        End::Start => (0.0, size),
        End::Finish => (r.length - size, r.length),
    };
    if r.vertical {
        (r.x - across / 2.0, r.y + from, r.x + across / 2.0, r.y + to)
    } else {
        (r.x + from, r.y - across / 2.0, r.x + to, r.y + across / 2.0)
    }
}

/// The end of `id` whose handle is under a point (mm).
pub(super) fn pick_end(
    project: &novo_core::Project,
    id: u64,
    x: f64,
    y: f64,
    slack: f64,
    size: f64,
) -> Option<End> {
    let r = project.get::<Rail>(RailId::from(id))?;
    [End::Finish, End::Start].into_iter().find(|&end| {
        let (l, t, rt, b) = handle_of(r, end, size);
        x >= l - slack && x <= rt + slack && y >= t - slack && y <= b + slack
    })
}

fn to_screen(rect: egui::Rect, view: &Viewport, x: f64, y: f64) -> egui::Pos2 {
    let s = view.world_to_screen(novo_canvas::vec2(x as f32, y as f32));
    egui::pos2(rect.min.x + s.x, rect.min.y + s.y)
}

/// Draws the two end handles of the selected rail or duct (at `edit`'s
/// shape while one is dragged).
pub(super) fn draw_handles(
    painter: &egui::Painter,
    rect: egui::Rect,
    view: &Viewport,
    project: &novo_core::Project,
    id: u64,
    (size, active): (f64, Option<End>),
    palette: &novo_theme::Palette,
) {
    let Some(r) = project.get::<Rail>(RailId::from(id)) else {
        return;
    };
    for end in [End::Start, End::Finish] {
        let (l, t, rt, b) = handle_of(r, end, size);
        let area =
            egui::Rect::from_two_pos(to_screen(rect, view, l, t), to_screen(rect, view, rt, b));
        let lit = active == Some(end);
        let fill = if lit {
            palette.accent_hover
        } else {
            palette.accent
        };
        painter.rect_filled(area, 2.0, fill);
        painter.rect_stroke(
            area,
            2.0,
            egui::Stroke::new(1.0, palette.on_accent),
            egui::StrokeKind::Inside,
        );
        let c = area.center();
        let step = if r.vertical {
            egui::vec2(3.5, 0.0)
        } else {
            egui::vec2(0.0, 3.5)
        };
        for k in [-1.0_f32, 0.0, 1.0] {
            painter.circle_filled(c + step * k, 1.0, palette.on_accent);
        }
    }
}

/// "1 360 mm", plus what the end snapped to or was stopped by.
pub(super) fn edit_text(project: &novo_core::Project, edit: &Edit) -> String {
    let name = |id: u64| match project.get::<Rail>(RailId::from(id)).map(|r| r.kind) {
        Some(novo_core::RailKind::Duct) => format!("duct {id}"),
        _ => format!("rail {id}"),
    };
    let mut text = format!("{:.0} mm", edit.length);
    match (edit.stop, edit.snap) {
        (Some(Stop::Part(id)), _) => text.push_str(&format!(" - stops at {}", name(id))),
        (Some(Stop::Device(_)), _) => text.push_str(" - stops at a device"),
        (Some(Stop::Plate), _) => text.push_str(" - plate edge"),
        (Some(Stop::Devices), _) => text.push_str(" - as short as its devices allow"),
        (Some(Stop::Shortest), _) => text.push_str(" - shortest cut"),
        (None, Some(SnapTo::Part(id))) => text.push_str(&format!(" - in line with {}", name(id))),
        (None, Some(SnapTo::Device(_))) => text.push_str(" - at a device edge"),
        (None, Some(SnapTo::Plate)) => text.push_str(" - plate edge"),
        _ => {}
    }
    text
}

/// Outlines a piece `(x, y, length)` of `id` over the canvas and writes
/// `text` beside it. `stopped` draws it in the warning colour.
pub(super) fn draw_piece(
    painter: &egui::Painter,
    (rect, view): (egui::Rect, &Viewport),
    project: &novo_core::Project,
    id: u64,
    (x, y, length): (f64, f64, f64),
    (text, stopped): (&str, bool),
    palette: &novo_theme::Palette,
) {
    let Some(r) = project.get::<Rail>(RailId::from(id)) else {
        return;
    };
    let across = across_of(r);
    let (w, h, left, top) = if r.vertical {
        (across, length, x - across / 2.0, y)
    } else {
        (length, across, x, y - across / 2.0)
    };
    let area = egui::Rect::from_two_pos(
        to_screen(rect, view, left, top),
        to_screen(rect, view, left + w, top + h),
    )
    .expand(2.0);
    let colour = if stopped {
        palette.warning
    } else {
        palette.accent
    };
    painter.rect_stroke(
        area,
        0.0,
        egui::Stroke::new(2.0, colour),
        egui::StrokeKind::Outside,
    );
    // The length, beside the middle: above a row, right of a column.
    let (anchor, align) = if r.vertical {
        (
            egui::pos2(area.max.x + 6.0, area.center().y),
            egui::Align2::LEFT_CENTER,
        )
    } else {
        (
            egui::pos2(area.center().x, area.min.y - 6.0),
            egui::Align2::CENTER_BOTTOM,
        )
    };
    let font = egui::FontId::proportional(12.0);
    let galley = painter.layout_no_wrap(text.to_owned(), font, palette.text_primary);
    let label = align.anchor_size(anchor, galley.size()).expand(3.0);
    painter.rect_filled(label, 3.0, palette.raised);
    painter.rect_stroke(
        label,
        3.0,
        egui::Stroke::new(1.0, colour),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        label.min + egui::vec2(3.0, 3.0),
        galley,
        palette.text_primary,
    );
}

/// The bar a length is cut from, as one workshop line.
pub(super) fn bar_line(length: f64) -> String {
    let stock = Stock::default();
    let n = railedit::bar_note(length, &stock);
    if n.bars == 0 {
        return String::new();
    }
    let bar = stock.length;
    let left = if n.offcut <= 0.0 {
        "no offcut".to_string()
    } else if n.kept {
        format!("{:.0} mm offcut, keep", n.offcut)
    } else {
        format!("{:.0} mm offcut, scrap", n.offcut)
    };
    if n.bars == 1 {
        format!("Cut from one {bar:.0} mm bar: {left}.")
    } else {
        format!(
            "Needs {} bars of {bar:.0} mm, spliced: {left} on the last.",
            n.bars
        )
    }
}

/// The card for the selected rail or duct under the list: its length and
/// bar, the trim buttons and the stock lengths.
pub(super) fn selected_card(
    ui: &mut egui::Ui,
    state: &mut RailsState,
    project: &novo_core::Project,
    structure: StructureId,
) -> Option<RailAsk> {
    let id = state.deletable(project)?;
    let r = project.get::<Rail>(RailId::from(id))?;
    let layout = novo_layout::model::load(project, structure)?;
    let mut ask = None;
    let duct = r.kind == novo_core::RailKind::Duct;
    ui.separator();
    ui.strong(format!(
        "{} {} - {:.0} mm",
        if duct { "Duct" } else { "Rail" },
        id,
        r.length
    ));
    ui.weak(bar_line(r.length));
    ui.horizontal_wrapped(|ui| {
        for (end, label, tip) in [
            (
                End::Start,
                "Trim start to crossing",
                "Cuts the start back to the nearest rail or duct that crosses it",
            ),
            (
                End::Finish,
                "Trim end to crossing",
                "Cuts the end back to the nearest rail or duct that crosses it",
            ),
        ] {
            let found = railedit::trim_to_crossing(&layout, id, end);
            let button = ui.add_enabled(found.is_some(), egui::Button::new(label));
            let button = match found {
                Some(t) => button.on_hover_text(format!("{tip}: {:.0} mm come off.", t.cut_off)),
                None => {
                    button.on_disabled_hover_text(railedit::no_crossing_reason(&layout, id, end))
                }
            };
            if button.clicked() {
                ask = Some(RailAsk::Trim { id, end });
            }
        }
        if ui
            .selectable_label(state.trim, "Trim at point")
            .on_hover_text(
                "Click on the rail or duct to cut there; the shorter side comes off. \
                 Esc ends it.",
            )
            .clicked()
        {
            state.trim ^= true;
        }
        ui.menu_button("Cut to length", |ui| {
            let stock = Stock::default();
            for len in railedit::stock_lengths(&stock) {
                let enabled = railedit::set_length(&layout, id, len).is_some();
                if ui
                    .add_enabled(
                        enabled,
                        egui::Button::new(format!("{len:.0} mm  ({})", short_bar(len, &stock))),
                    )
                    .clicked()
                {
                    ask = Some(RailAsk::SetLength(id, len));
                    ui.close();
                }
            }
        });
    });
    ask
}

/// "full bar", "half a bar", ... for a stock length.
fn short_bar(len: f64, stock: &Stock) -> String {
    let f = len / stock.length;
    if (f - 1.0).abs() < 1e-9 {
        "full bar".into()
    } else if (f - 0.5).abs() < 1e-9 {
        "half a bar".into()
    } else if (f - 0.25).abs() < 1e-9 {
        "a quarter".into()
    } else {
        "a fifth".into()
    }
}

impl super::NovoApp {
    /// Applies a dragged end, a trim or a typed or stock length as one undo
    /// step. Only the rail or duct changes: the devices on a rail stay where
    /// they are.
    pub(super) fn cabinet_rail_resize(&mut self, structure: StructureId, id: u64, edit: Edit) {
        let rail = RailId::from(id);
        let Some(doc) = self.backend.document() else {
            return;
        };
        let Some(r) = doc.project().get::<Rail>(rail) else {
            return;
        };
        if (r.x, r.y, r.length) == (edit.x, edit.y, edit.length) {
            return;
        }
        let _ = structure;
        let (dx, dy) = (edit.x - r.x, edit.y - r.y);
        let along = if r.vertical { dy } else { dx };
        let label = if edit.length < r.length {
            "Shorten rail"
        } else {
            "Lengthen rail"
        };
        self.cabinet_edit(label, move |tx| {
            tx.update::<Rail>(rail, |r| {
                r.x = edit.x;
                r.y = edit.y;
                r.length = edit.length;
            })?;
            // A device's offset along its rail counts from the rail's start.
            let on: Vec<novo_core::CabinetPlacementId> = tx
                .project()
                .all::<novo_core::CabinetPlacement>()
                .filter(|p| p.rail == Some(rail) && p.rail_offset.is_some())
                .map(|p| p.id)
                .collect();
            if along != 0.0 {
                for p in on {
                    tx.update::<novo_core::CabinetPlacement>(p, |p| {
                        p.rail_offset = p.rail_offset.map(|o| o - along);
                    })?;
                }
            }
            Ok(())
        });
    }

    /// A trim at a crossing, a typed length or a stock length.
    pub(super) fn cabinet_rail_cut(&mut self, structure: StructureId, ask: &RailAsk) {
        let Some(layout) = self
            .backend
            .document()
            .and_then(|d| novo_layout::model::load(d.project(), structure))
        else {
            return;
        };
        let (id, edit, what) = match *ask {
            RailAsk::Trim { id, end } => match railedit::trim_to_crossing(&layout, id, end) {
                Some(t) => (id, t.edit, format!("Trimmed {:.0} mm off", t.cut_off)),
                None => {
                    let why = match *ask {
                        RailAsk::Trim { id, end } => railedit::no_crossing_reason(&layout, id, end),
                        _ => String::new(),
                    };
                    self.message(super::Level::Info, why);
                    return;
                }
            },
            RailAsk::TrimAt { id, end, at } => match railedit::trim_at(&layout, id, end, at) {
                Some(t) => (id, t.edit, format!("Trimmed {:.0} mm off", t.cut_off)),
                None => {
                    self.message(
                        super::Level::Info,
                        "That cut would leave less than 10 mm or cut into the devices on the rail"
                            .to_string(),
                    );
                    return;
                }
            },
            RailAsk::SetLength(id, length) => match railedit::set_length(&layout, id, length) {
                Some(e) => {
                    let text = match e.stop {
                        Some(Stop::Part(o)) => format!(" (stops at {o})"),
                        Some(Stop::Device(_)) => " (stops at a device)".to_string(),
                        Some(Stop::Devices) => " (as short as its devices allow)".to_string(),
                        Some(Stop::Plate) => " (plate edge)".to_string(),
                        _ => String::new(),
                    };
                    (id, e, format!("Length {:.0} mm{text}", e.length))
                }
                None => return,
            },
            _ => return,
        };
        self.cabinet_rail_resize(structure, id, edit);
        self.message(super::Level::Info, what);
    }
}
