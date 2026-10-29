//! Dock slots: a strip along the left, the right or the bottom of the canvas
//! that a panel can be dropped into. A panel that opts in with
//! [`super::panel::Panel::dockable`] shows two small buttons in its title bar
//! (dock left, dock right) and, while it is dragged, three snap zones light up
//! at the edges of the canvas; letting go over one docks it there.
//!
//! A docked panel is a pane of its slot: the slot is a resizable side or
//! bottom panel of the window (the canvas gets what is left), and the panels
//! in it share it equally. Its header has the buttons to float it again or
//! send it to another slot. Floated again, it comes back to where it was.
//!
//! egui has no docking; this is `panel.rs` and egui's own side panels, as
//! `docs/design/visual-redesign.md` section 8 describes. Where the panels are
//! docked is egui's persisted data, so it is kept between runs; View > Reset
//! panel layout floats them all again.

use eframe::egui;

/// How far from the canvas edge a drag has to go to be over a snap zone.
const ZONE: f32 = 72.0;
/// The size a slot opens at, and how small or large the person can drag it.
const SIDE_DEFAULT: f32 = 320.0;
const SIDE_MIN: f32 = 220.0;
const SIDE_MAX: f32 = 520.0;
const BOTTOM_DEFAULT: f32 = 240.0;
const BOTTOM_MIN: f32 = 140.0;
/// The header of a docked pane and the buttons in it.
/// The least a pane keeps when the divider between two panes is dragged: along
/// a side slot its height, along the bottom slot its width.
const PANE_MIN_HEIGHT: f32 = 120.0;
const PANE_MIN_WIDTH: f32 = 200.0;
/// How far the divider's grip reaches into a pane from the edge it shares.
const DIVIDER: f32 = 6.0;
const HEADER: f32 = 28.0;
const BUTTON: f32 = 22.0;

/// Where a panel is docked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Slot {
    Left,
    Right,
    Bottom,
}

impl Slot {
    pub(super) const ALL: [Slot; 3] = [Slot::Left, Slot::Right, Slot::Bottom];

    /// The name of the slot's side panel (its egui id, also in `EDGE_PANELS`).
    pub(super) fn name(self) -> &'static str {
        match self {
            Slot::Left => "dock_left",
            Slot::Right => "dock_right",
            Slot::Bottom => "dock_bottom",
        }
    }

    fn code(self) -> u8 {
        match self {
            Slot::Left => 1,
            Slot::Right => 2,
            Slot::Bottom => 3,
        }
    }

    fn from_code(code: u8) -> Option<Slot> {
        Slot::ALL.into_iter().find(|s| s.code() == code)
    }

    fn tip(self) -> &'static str {
        match self {
            Slot::Left => "Dock at the left edge",
            Slot::Right => "Dock at the right edge",
            Slot::Bottom => "Dock at the bottom edge",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Slot::Left => novo_theme::icons::ARROW_LINE_LEFT,
            Slot::Right => novo_theme::icons::ARROW_LINE_RIGHT,
            Slot::Bottom => novo_theme::icons::ARROW_LINE_DOWN,
        }
    }
}

/// The panels that are docked, in the order they were docked, with their slot
/// code. One list so that Reset panel layout can clear it and egui saves it.
fn list_id() -> egui::Id {
    egui::Id::new("dock_list")
}

fn list(ctx: &egui::Context) -> Vec<(String, u8)> {
    ctx.data_mut(|d| d.get_persisted(list_id()))
        .unwrap_or_default()
}

/// The slot the panel named `name` is docked in.
pub(super) fn slot_of(ctx: &egui::Context, name: &str) -> Option<Slot> {
    list(ctx)
        .into_iter()
        .find(|(n, _)| n == name)
        .and_then(|(_, code)| Slot::from_code(code))
}

/// Docks the panel in `slot` (at the end of it), or floats it with `None`.
pub(super) fn set(ctx: &egui::Context, name: &str, slot: Option<Slot>) {
    if slot_of(ctx, name) != slot {
        ctx.data_mut(|d| d.insert_temp(changed_id(), SETTLE_FRAMES));
    }
    let mut all = list(ctx);
    all.retain(|(n, _)| n != name);
    if let Some(slot) = slot {
        all.push((name.to_owned(), slot.code()));
    }
    ctx.data_mut(|d| d.insert_persisted(list_id(), all));
    ctx.request_repaint();
}

fn changed_id() -> egui::Id {
    egui::Id::new("dock_changed")
}

/// Frames between a dock change and the refit: the strip appears a pass or two
/// after the panel asks for it, and the fit must see the canvas at its new size.
const SETTLE_FRAMES: u8 = 4;

/// True once, a few frames after a panel was docked, moved to another slot or
/// floated: the canvas has a different size by then, so the app fits the view
/// again (the sheet would otherwise sit partly under the strip).
pub(super) fn take_changed(ctx: &egui::Context) -> bool {
    let left = ctx.data(|d| d.get_temp::<u8>(changed_id())).unwrap_or(0);
    if left == 0 {
        return false;
    }
    ctx.data_mut(|d| d.insert_temp(changed_id(), left - 1));
    ctx.request_repaint();
    left == 1
}

/// Floats every panel (Reset panel layout).
pub(super) fn reset(ctx: &egui::Context) {
    if !list(ctx).is_empty() {
        ctx.data_mut(|d| d.insert_temp(changed_id(), SETTLE_FRAMES));
    }
    ctx.data_mut(|d| d.insert_persisted(list_id(), Vec::<(String, u8)>::new()));
}

/// Where each slot was drawn this frame: the pane areas are laid out in it.
fn rect_id(slot: Slot) -> egui::Id {
    egui::Id::new(("dock_rect", slot.code()))
}

fn slot_rect(ctx: &egui::Context, slot: Slot) -> Option<egui::Rect> {
    ctx.data(|d| d.get_temp(rect_id(slot)))
}

/// The panels that drew a pane in a slot, this pass and the one before: the
/// slot is only there (and the canvas only made smaller) while one did.
fn shown_id(slot: Slot, previous: bool) -> egui::Id {
    egui::Id::new(("dock_shown", slot.code(), previous))
}

fn register(ctx: &egui::Context, slot: Slot, name: &str) {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<String>>(shown_id(slot, false))
            .push(name.to_owned());
    });
}

fn shown_before(ctx: &egui::Context, slot: Slot) -> Vec<String> {
    ctx.data(|d| d.get_temp(shown_id(slot, true)))
        .unwrap_or_default()
}

/// Which pane of how many a panel is: the panels of a slot in the order they
/// were docked, those that drew last pass and this panel.
fn pane(ctx: &egui::Context, slot: Slot, name: &str) -> (usize, usize) {
    let before = shown_before(ctx, slot);
    let order: Vec<String> = list(ctx)
        .into_iter()
        .filter(|(n, code)| *code == slot.code() && (n == name || before.contains(n)))
        .map(|(n, _)| n)
        .collect();
    let at = order.iter().position(|n| n == name).unwrap_or(0);
    (at, order.len().max(1))
}

fn weights_id(slot: Slot) -> egui::Id {
    egui::Id::new(("dock_weights", slot.code()))
}

/// The share of its slot each of `n` panes has (they add up to one): equal
/// until a divider is dragged, and equal again when the number of panes
/// changes.
fn weights(ctx: &egui::Context, slot: Slot, n: usize) -> Vec<f32> {
    let stored: Option<Vec<f32>> = ctx.data_mut(|d| d.get_persisted(weights_id(slot)));
    match stored {
        Some(w) if w.len() == n && n > 0 => w,
        _ => vec![1.0 / n.max(1) as f32; n.max(1)],
    }
}

fn set_weights(ctx: &egui::Context, slot: Slot, w: Vec<f32>) {
    ctx.data_mut(|d| d.insert_persisted(weights_id(slot), w));
}

/// The share of a slot a pane gets: its weight of the slot, stacked down a
/// side slot and laid side by side along the bottom one.
fn split(rect: egui::Rect, slot: Slot, w: &[f32], at: usize) -> egui::Rect {
    let before: f32 = w[..at].iter().sum();
    let part = w[at];
    match slot {
        Slot::Bottom => egui::Rect::from_min_size(
            egui::pos2(rect.left() + rect.width() * before, rect.top()),
            egui::vec2(rect.width() * part, rect.height()),
        ),
        Slot::Left | Slot::Right => egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.top() + rect.height() * before),
            egui::vec2(rect.width(), rect.height() * part),
        ),
    }
}

/// The divider between pane `at` and the next one: dragged, it moves the
/// share of one pane to the other (each keeps a minimum); a double click gives
/// both equal shares again. `sub` is the pane `at`, `slot_area` the whole slot.
fn divider(
    ui: &mut egui::Ui,
    id: egui::Id,
    slot: Slot,
    slot_area: egui::Rect,
    sub: egui::Rect,
    at: usize,
    w: &[f32],
) {
    let ctx = ui.ctx().clone();
    let (grip, length, min, icon) = match slot {
        Slot::Bottom => (
            egui::Rect::from_x_y_ranges(sub.right() - DIVIDER..=sub.right(), sub.y_range()),
            slot_area.width(),
            PANE_MIN_WIDTH,
            egui::CursorIcon::ResizeHorizontal,
        ),
        Slot::Left | Slot::Right => (
            egui::Rect::from_x_y_ranges(sub.x_range(), sub.bottom() - DIVIDER..=sub.bottom()),
            slot_area.height(),
            PANE_MIN_HEIGHT,
            egui::CursorIcon::ResizeVertical,
        ),
    };
    let response = ui.interact(grip, id.with(("divider", at)), egui::Sense::click_and_drag());
    if response.hovered() || response.dragged() {
        ctx.set_cursor_icon(icon);
        let pal = crate::theme::ui_palette();
        ui.painter().rect_filled(grip, 0.0, pal.accent);
    }
    let mut w = w.to_vec();
    if response.double_clicked() {
        let pair = w[at] + w[at + 1];
        w[at] = pair / 2.0;
        w[at + 1] = pair / 2.0;
        set_weights(&ctx, slot, w);
    } else if response.dragged() {
        let delta = match slot {
            Slot::Bottom => response.drag_delta().x,
            _ => response.drag_delta().y,
        } / length.max(1.0);
        let pair = w[at] + w[at + 1];
        let floor = (min / length.max(1.0)).min(pair / 2.0);
        w[at] = (w[at] + delta).clamp(floor, pair - floor);
        w[at + 1] = pair - w[at];
        set_weights(&ctx, slot, w);
    }
}

/// The slots that have a panel in them: each is a side or bottom panel of the
/// window, resizable by its edge (a double click on the edge resets it).
/// Call it once per frame, after the other side panels and drawers and
/// before the canvas, so the canvas is what is left.
pub(super) fn spacers(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let pal = crate::theme::ui_palette();
    for slot in Slot::ALL {
        // This pass's registrations are the next pass's "before".
        let now: Vec<String> = ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Vec<String>>(shown_id(slot, false))
                .split_off(0)
        });
        ctx.data_mut(|d| d.insert_temp(shown_id(slot, true), now));
        if shown_before(&ctx, slot).is_empty() {
            ctx.data_mut(|d| d.remove::<egui::Rect>(rect_id(slot)));
            continue;
        }
        let screen = ctx.content_rect();
        let panel = match slot {
            Slot::Left => egui::Panel::left(slot.name())
                .default_size(SIDE_DEFAULT)
                .size_range(SIDE_MIN..=SIDE_MAX.min(screen.width() * 0.5)),
            Slot::Right => egui::Panel::right(slot.name())
                .default_size(SIDE_DEFAULT)
                .size_range(SIDE_MIN..=SIDE_MAX.min(screen.width() * 0.5)),
            Slot::Bottom => egui::Panel::bottom(slot.name())
                .default_size(BOTTOM_DEFAULT)
                .size_range(BOTTOM_MIN..=(screen.height() * 0.5).max(BOTTOM_MIN)),
        };
        let shown = panel
            .resizable(true)
            .frame(egui::Frame::NONE.fill(pal.panel))
            .show(ui, |ui| ui.set_min_size(ui.available_size()));
        let rect = shown.response.rect;
        ctx.data_mut(|d| d.insert_temp(rect_id(slot), rect));
    }
}

/// What a header button asked for.
enum Ask {
    Float,
    Move(Slot),
}

/// A docked panel: a pane of its slot with a header and a body that scrolls.
/// `None` for the one frame before the slot exists.
pub(super) fn show_docked<R>(
    ctx: &egui::Context,
    id: egui::Id,
    name: &str,
    title: &str,
    slot: Slot,
    closable: bool,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<egui::InnerResponse<Option<R>>> {
    register(ctx, slot, name);
    let Some(slot_area) = slot_rect(ctx, slot) else {
        // The slot appears with the next pass.
        ctx.request_repaint();
        return None;
    };
    let (at, of) = pane(ctx, slot, name);
    let w = weights(ctx, slot, of);
    let sub = split(slot_area, slot, &w, at);
    let pal = crate::theme::ui_palette();
    let mut ask = None;
    let mut closed = false;
    let mut out = egui::Area::new(id.with("docked"))
        .order(egui::Order::Background)
        .fixed_pos(sub.min)
        .constrain(false)
        .show(ctx, |ui| {
            ui.scope_builder(egui::UiBuilder::new().max_rect(sub), |ui| {
                ui.set_clip_rect(sub);
                ui.painter().rect_filled(sub, 0.0, pal.panel);
                let header = egui::Rect::from_min_size(sub.min, egui::vec2(sub.width(), HEADER));
                ui.painter().rect_filled(header, 0.0, pal.raised);
                ui.painter().hline(
                    sub.x_range(),
                    header.bottom(),
                    egui::Stroke::new(1.0, pal.border_subtle),
                );
                ui.painter().vline(
                    if slot == Slot::Right {
                        sub.left()
                    } else {
                        sub.right()
                    },
                    sub.y_range(),
                    egui::Stroke::new(1.0, pal.border_subtle),
                );
                ui.painter().text(
                    header.left_center() + egui::vec2(8.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    title,
                    egui::FontId::proportional(13.0),
                    pal.text_strong,
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(header.shrink2(egui::vec2(4.0, 3.0)))
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        if closable
                            && ui
                                .add(
                                    egui::Button::new(novo_theme::icons::X)
                                        .frame(false)
                                        .min_size(egui::vec2(BUTTON, BUTTON)),
                                )
                                .on_hover_text("Close")
                                .clicked()
                        {
                            closed = true;
                        }
                        if ui
                            .add(
                                egui::Button::new(novo_theme::icons::CORNERS_OUT)
                                    .frame(false)
                                    .min_size(egui::vec2(BUTTON, BUTTON)),
                            )
                            .on_hover_text("Float this panel")
                            .clicked()
                        {
                            ask = Some(Ask::Float);
                        }
                        for to in Slot::ALL.into_iter().rev().filter(|s| *s != slot) {
                            if ui
                                .add(
                                    egui::Button::new(to.icon())
                                        .frame(false)
                                        .min_size(egui::vec2(BUTTON, BUTTON)),
                                )
                                .on_hover_text(to.tip())
                                .clicked()
                            {
                                ask = Some(Ask::Move(to));
                            }
                        }
                    },
                );
                let body = egui::Rect::from_min_max(
                    egui::pos2(sub.left(), header.bottom() + 1.0),
                    sub.max,
                )
                .shrink(6.0);
                let inner = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                        egui::ScrollArea::both()
                            .auto_shrink([false, false])
                            .show(ui, add)
                            .inner
                    })
                    .inner;
                if at + 1 < of {
                    divider(ui, id, slot, slot_area, sub, at, &w);
                }
                inner
            })
            .inner
        });
    match ask {
        Some(Ask::Float) => set(ctx, name, None),
        Some(Ask::Move(to)) => set(ctx, name, Some(to)),
        None => {}
    }
    if closed {
        ctx.data_mut(|d| d.insert_temp(id.with("closed"), true));
    }
    // The pane fills its share of the slot whatever its content used.
    out.response.rect = sub;
    Some(egui::InnerResponse::new(Some(out.inner), out.response))
}

/// The dock buttons in the title bar of a floating panel: painted on the
/// window's own layer and hit-tested by hand, since egui's title bar has no
/// room for more buttons of ours. Returns the slot that was clicked.
pub(super) fn title_buttons(ctx: &egui::Context, id: egui::Id) -> Option<Slot> {
    let title = ctx.read_response(id.with("__title_click"))?;
    let layer = egui::LayerId::new(egui::Order::Middle, id);
    let pal = crate::theme::ui_palette();
    let painter = ctx.layer_painter(layer);
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let clicked = ctx.input(|i| i.pointer.primary_clicked());
    let right = title.rect.right() - 4.0;
    let mut chosen = None;
    for (k, slot) in [Slot::Right, Slot::Left].into_iter().enumerate() {
        let rect = egui::Rect::from_center_size(
            egui::pos2(
                right - BUTTON / 2.0 - k as f32 * BUTTON,
                title.rect.center().y,
            ),
            egui::vec2(BUTTON, BUTTON),
        );
        let hovered = pointer.is_some_and(|p| rect.contains(p))
            && pointer.and_then(|p| ctx.layer_id_at(p)) == Some(layer);
        if hovered {
            painter.rect_filled(rect, 4.0, pal.hover);
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            egui::Tooltip::always_open(
                ctx.clone(),
                layer,
                id.with(("dock_button", slot.code())),
                egui::PopupAnchor::Pointer,
            )
            .show(|ui| ui.label(slot.tip()));
            if clicked {
                chosen = Some(slot);
            }
        }
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            slot.icon(),
            egui::FontId::proportional(14.0),
            if hovered {
                pal.text_strong
            } else {
                pal.text_secondary
            },
        );
    }
    chosen
}

/// The snap zones of a canvas: a strip along its left and right edges and
/// along the bottom. Painted while the title bar of a dockable panel is
/// dragged; the slot under the pointer when the drag ends is where the panel
/// docks. The sides win in the corners.
pub(super) fn snap_zones(
    ctx: &egui::Context,
    canvas: egui::Rect,
    dragging: bool,
    stopped: bool,
) -> Option<Slot> {
    if !dragging && !stopped {
        return None;
    }
    let pointer = ctx.input(|i| i.pointer.latest_pos())?;
    let zones = [
        (
            Slot::Left,
            egui::Rect::from_min_size(canvas.min, egui::vec2(ZONE, canvas.height())),
        ),
        (
            Slot::Right,
            egui::Rect::from_min_size(
                egui::pos2(canvas.right() - ZONE, canvas.top()),
                egui::vec2(ZONE, canvas.height()),
            ),
        ),
        (
            Slot::Bottom,
            egui::Rect::from_min_size(
                egui::pos2(canvas.left(), canvas.bottom() - ZONE),
                egui::vec2(canvas.width(), ZONE),
            ),
        ),
    ];
    let over = zones
        .iter()
        .find(|(_, rect)| rect.contains(pointer))
        .map(|(slot, _)| *slot);
    if dragging {
        let pal = crate::theme::ui_palette();
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("dock_zones"),
        ));
        for (slot, rect) in zones {
            let on = over == Some(slot);
            painter.rect_filled(
                rect,
                4.0,
                pal.accent.gamma_multiply(if on { 0.35 } else { 0.12 }),
            );
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(if on { 2.0 } else { 1.0 }, pal.accent),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                slot.tip(),
                egui::FontId::proportional(13.0),
                pal.text_strong,
            );
        }
    }
    if stopped { over } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::panel::{self, Panel, Side};

    const SCREEN: egui::Vec2 = egui::vec2(1366.0, 768.0);

    /// The window of the tests: dock slots, then a canvas, then dockable
    /// panels on it, as `app.rs` lays them out.
    struct Harness {
        ctx: egui::Context,
        t: f64,
    }

    struct Out {
        canvas: egui::Rect,
        panels: Vec<Option<egui::Rect>>,
    }

    impl Harness {
        fn new() -> Self {
            let ctx = egui::Context::default();
            Self { ctx, t: 100.0 }
        }

        fn frame(&mut self, names: &[&'static str], events: Vec<egui::Event>) -> Out {
            self.t += 0.05;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
                time: Some(self.t),
                events,
                ..Default::default()
            };
            let mut out = Out {
                canvas: egui::Rect::NOTHING,
                panels: Vec::new(),
            };
            let mut full = self.ctx.run_ui(input, |ui| {
                spacers(ui);
                let canvas = egui::CentralPanel::no_frame()
                    .show(ui, |_| {})
                    .response
                    .rect;
                let ctx = ui.ctx().clone();
                out.canvas = canvas;
                for name in names {
                    let shown = Panel::new(name, *name, canvas, Side::Right, 300.0)
                        .dockable()
                        .show(&ctx, |ui| {
                            ui.label("body");
                        });
                    out.panels.push(shown.map(|r| r.response.rect));
                }
            });
            full.textures_delta.clear();
            out
        }

        fn settle(&mut self, names: &[&'static str]) -> Out {
            let mut out = self.frame(names, vec![]);
            for _ in 0..4 {
                out = self.frame(names, vec![]);
            }
            out
        }

        fn click(&mut self, names: &[&'static str], at: egui::Pos2) {
            let press = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            self.frame(names, vec![egui::Event::PointerMoved(at)]);
            self.frame(names, vec![press(true)]);
            self.frame(names, vec![press(false)]);
        }

        fn drag(&mut self, names: &[&'static str], from: egui::Pos2, to: egui::Pos2) {
            let press = |at, pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            self.frame(names, vec![egui::Event::PointerMoved(from)]);
            self.frame(names, vec![press(from, true)]);
            for step in 1..=6 {
                let at = from + (to - from) * (step as f32 / 6.0);
                self.frame(names, vec![egui::Event::PointerMoved(at)]);
            }
            self.frame(names, vec![press(to, false)]);
            self.settle(names);
        }
    }

    const ONE: &[&str] = &["dock_test_a"];
    const TWO: &[&str] = &["dock_test_a", "dock_test_b"];

    #[test]
    fn a_floating_panel_takes_nothing_from_the_canvas() {
        let mut h = Harness::new();
        let out = h.settle(ONE);
        assert_eq!(out.canvas.left(), 0.0);
        assert!(out.canvas.right() > SCREEN.x - 1.0);
        assert!(out.panels[0].is_some());
    }

    #[test]
    fn a_panel_docked_left_fills_a_strip_and_the_canvas_is_what_is_left() {
        let mut h = Harness::new();
        h.settle(ONE);
        set(&h.ctx, "dock_test_a", Some(Slot::Left));
        let out = h.settle(ONE);
        let pane = out.panels[0].expect("the docked panel is shown");
        assert!(
            (out.canvas.left() - SIDE_DEFAULT).abs() < 2.0,
            "{:?}",
            out.canvas
        );
        assert!(
            pane.left() < 1.0 && (pane.width() - SIDE_DEFAULT).abs() < 2.0,
            "{pane:?}"
        );
        assert!(
            (pane.height() - out.canvas.height()).abs() < 2.0,
            "{pane:?} {:?}",
            out.canvas
        );
    }

    #[test]
    fn right_and_bottom_slots_take_their_own_edge() {
        let mut h = Harness::new();
        h.settle(ONE);
        set(&h.ctx, "dock_test_a", Some(Slot::Right));
        let out = h.settle(ONE);
        assert!(
            (SCREEN.x - out.canvas.right() - SIDE_DEFAULT).abs() < 2.0,
            "{:?}",
            out.canvas
        );
        set(&h.ctx, "dock_test_a", Some(Slot::Bottom));
        let out = h.settle(ONE);
        let pane = out.panels[0].unwrap();
        assert!(out.canvas.right() > SCREEN.x - 1.0, "{:?}", out.canvas);
        assert!(pane.width() > out.canvas.width() - 2.0, "{pane:?}");
        assert!(pane.bottom() <= SCREEN.y + 0.5, "{pane:?}");
        assert!(
            out.canvas.bottom() <= pane.top() + 1.0,
            "{:?} {pane:?}",
            out.canvas
        );
    }

    #[test]
    fn floating_a_docked_panel_gives_the_canvas_back() {
        let mut h = Harness::new();
        let home = h.settle(ONE).panels[0].unwrap();
        set(&h.ctx, "dock_test_a", Some(Slot::Left));
        h.settle(ONE);
        set(&h.ctx, "dock_test_a", None);
        let out = h.settle(ONE);
        assert_eq!(out.canvas.left(), 0.0, "{:?}", out.canvas);
        let back = out.panels[0].unwrap();
        assert!((back.min - home.min).length() < 3.0, "{home:?} {back:?}");
    }

    #[test]
    fn two_panels_in_a_slot_share_it_equally() {
        let mut h = Harness::new();
        h.settle(TWO);
        set(&h.ctx, "dock_test_a", Some(Slot::Left));
        set(&h.ctx, "dock_test_b", Some(Slot::Left));
        let out = h.settle(TWO);
        let (a, b) = (out.panels[0].unwrap(), out.panels[1].unwrap());
        assert!((a.height() - b.height()).abs() < 2.0, "{a:?} {b:?}");
        assert!(a.bottom() <= b.top() + 1.0, "{a:?} {b:?}");
        assert!((a.height() + b.height() - out.canvas.height()).abs() < 3.0);
        // Along the bottom they sit side by side.
        set(&h.ctx, "dock_test_a", Some(Slot::Bottom));
        set(&h.ctx, "dock_test_b", Some(Slot::Bottom));
        let out = h.settle(TWO);
        let (a, b) = (out.panels[0].unwrap(), out.panels[1].unwrap());
        assert!((a.width() - b.width()).abs() < 2.0, "{a:?} {b:?}");
        assert!(a.right() <= b.left() + 1.0, "{a:?} {b:?}");
    }

    #[test]
    fn dragging_a_title_bar_to_a_snap_zone_docks_the_panel() {
        let mut h = Harness::new();
        let home = h.settle(ONE).panels[0].unwrap();
        // The title bar, away from its buttons.
        let grab = home.center_top() + egui::vec2(-60.0, 10.0);
        h.drag(ONE, grab, egui::pos2(24.0, 300.0));
        assert_eq!(slot_of(&h.ctx, "dock_test_a"), Some(Slot::Left));
        let out = h.settle(ONE);
        assert!(
            (out.canvas.left() - SIDE_DEFAULT).abs() < 2.0,
            "{:?}",
            out.canvas
        );
    }

    #[test]
    fn a_drag_that_ends_off_the_zones_only_moves_the_panel() {
        let mut h = Harness::new();
        let home = h.settle(ONE).panels[0].unwrap();
        let grab = home.center_top() + egui::vec2(-60.0, 10.0);
        h.drag(ONE, grab, egui::pos2(500.0, 300.0));
        assert_eq!(slot_of(&h.ctx, "dock_test_a"), None);
        let moved = h.settle(ONE).panels[0].unwrap();
        assert!(
            (moved.min - home.min).length() > 100.0,
            "{home:?} {moved:?}"
        );
    }

    #[test]
    fn a_change_of_slot_asks_for_one_refit_a_few_frames_later() {
        let mut h = Harness::new();
        let ctx = h.ctx.clone();
        assert!(!take_changed(&ctx), "nothing changed yet");
        set(&ctx, "dock_test_a", Some(Slot::Left));
        let mut asked = Vec::new();
        for _ in 0..8 {
            asked.push(take_changed(&ctx));
        }
        assert_eq!(asked.iter().filter(|a| **a).count(), 1, "{asked:?}");
        assert!(
            !asked[0],
            "not on the same frame: the strip is not there yet"
        );
        // The same slot again changes nothing; floating does.
        set(&ctx, "dock_test_a", Some(Slot::Left));
        assert!((0..8).all(|_| !take_changed(&ctx)));
        set(&ctx, "dock_test_a", None);
        assert!((0..8).any(|_| take_changed(&ctx)));
        // Reset panel layout with nothing docked asks for nothing.
        reset(&ctx);
        assert!((0..8).all(|_| !take_changed(&ctx)));
        let _ = &mut h;
    }

    #[test]
    fn the_title_bar_buttons_dock_left_and_right() {
        let mut h = Harness::new();
        h.settle(ONE);
        for (k, slot) in [(1.0, Slot::Left), (0.0, Slot::Right)] {
            set(&h.ctx, "dock_test_a", None);
            h.settle(ONE);
            let title = h
                .ctx
                .read_response(panel::id("dock_test_a").with("__title_click"))
                .expect("a title bar")
                .rect;
            let at = egui::pos2(
                title.right() - 4.0 - BUTTON / 2.0 - k * BUTTON,
                title.center().y,
            );
            h.click(ONE, at);
            assert_eq!(slot_of(&h.ctx, "dock_test_a"), Some(slot), "{slot:?}");
        }
    }

    #[test]
    fn reset_panel_layout_floats_every_panel() {
        let mut h = Harness::new();
        h.settle(TWO);
        set(&h.ctx, "dock_test_a", Some(Slot::Left));
        set(&h.ctx, "dock_test_b", Some(Slot::Bottom));
        h.settle(TWO);
        panel::reset_layout(&h.ctx);
        let out = h.settle(TWO);
        assert_eq!(slot_of(&h.ctx, "dock_test_a"), None);
        assert_eq!(slot_of(&h.ctx, "dock_test_b"), None);
        assert_eq!(out.canvas.left(), 0.0, "{:?}", out.canvas);
        assert!(out.canvas.right() > SCREEN.x - 1.0);
    }

    #[test]
    fn a_slot_is_dragged_wider_and_a_double_click_on_its_edge_resets_it() {
        let mut h = Harness::new();
        h.settle(ONE);
        set(&h.ctx, "dock_test_a", Some(Slot::Left));
        let out = h.settle(ONE);
        let edge = egui::pos2(out.canvas.left() - 2.0, 300.0);
        h.drag(ONE, edge, edge + egui::vec2(100.0, 0.0));
        let wide = h.settle(ONE).canvas.left();
        assert!(wide > SIDE_DEFAULT + 60.0, "{wide}");
        assert!(wide <= SIDE_MAX + 1.0, "{wide}");
        // The app calls this once a frame; the slot's edge is in EDGE_PANELS.
        let edge = egui::pos2(wide - 2.0, 300.0);
        let press = |pressed| egui::Event::PointerButton {
            pos: edge,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        h.frame(ONE, vec![egui::Event::PointerMoved(edge)]);
        for (i, _) in [0, 1, 2, 3].iter().enumerate() {
            h.frame(ONE, vec![press(i % 2 == 0)]);
        }
        panel::splitters_reset(&h.ctx);
        let back = h.settle(ONE).canvas.left();
        assert!((back - SIDE_DEFAULT).abs() < 2.0, "{back}");
    }

    /// Two panes stacked in the right slot and the point on the divider.
    fn stacked(h: &mut Harness) -> (Out, egui::Pos2) {
        h.settle(TWO);
        set(&h.ctx, "dock_test_a", Some(Slot::Right));
        set(&h.ctx, "dock_test_b", Some(Slot::Right));
        let out = h.settle(TWO);
        let top = out.panels[0].expect("first pane");
        let at = egui::pos2(top.center().x, top.bottom() - DIVIDER / 2.0);
        (out, at)
    }

    #[test]
    fn dragging_the_divider_moves_height_from_one_pane_to_the_other() {
        let mut h = Harness::new();
        let (before, at) = stacked(&mut h);
        h.drag(TWO, at, at + egui::vec2(0.0, 80.0));
        let after = h.settle(TWO);
        let (a0, b0) = (before.panels[0].unwrap(), before.panels[1].unwrap());
        let (a1, b1) = (after.panels[0].unwrap(), after.panels[1].unwrap());
        assert!((a1.height() - a0.height() - 80.0).abs() < 3.0, "{a0:?} {a1:?}");
        assert!((b0.height() - b1.height() - 80.0).abs() < 3.0, "{b0:?} {b1:?}");
    }

    #[test]
    fn a_pane_keeps_a_minimum_when_the_divider_is_dragged_far() {
        let mut h = Harness::new();
        let (_, at) = stacked(&mut h);
        h.drag(TWO, at, at + egui::vec2(0.0, 2000.0));
        let after = h.settle(TWO);
        let b = after.panels[1].unwrap();
        assert!(b.height() >= PANE_MIN_HEIGHT - 2.0, "{b:?}");
        h.drag(TWO, egui::pos2(at.x, after.panels[0].unwrap().bottom() - 3.0), egui::pos2(at.x, 0.0));
        let after = h.settle(TWO);
        assert!(after.panels[0].unwrap().height() >= PANE_MIN_HEIGHT - 2.0);
    }

    #[test]
    fn a_double_click_on_the_divider_makes_the_panes_equal_again() {
        let mut h = Harness::new();
        let (before, at) = stacked(&mut h);
        h.drag(TWO, at, at + egui::vec2(0.0, 80.0));
        let moved = h.settle(TWO).panels[0].unwrap();
        let at = egui::pos2(at.x, moved.bottom() - DIVIDER / 2.0);
        h.click(TWO, at);
        h.click(TWO, at);
        let after = h.settle(TWO);
        let (a0, a1) = (before.panels[0].unwrap(), after.panels[0].unwrap());
        assert!((a0.height() - a1.height()).abs() < 3.0, "{a0:?} {a1:?}");
    }

    #[test]
    fn the_shares_are_equal_again_when_the_number_of_panes_changes() {
        let ctx = egui::Context::default();
        set_weights(&ctx, Slot::Left, vec![0.7, 0.3]);
        assert_eq!(weights(&ctx, Slot::Left, 2), vec![0.7, 0.3]);
        let three = weights(&ctx, Slot::Left, 3);
        assert!(three.iter().all(|w| (w - 1.0 / 3.0).abs() < 1e-6), "{three:?}");
    }
}
