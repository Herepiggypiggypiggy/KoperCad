//! The one place that decides how a floating panel behaves: roomy by default,
//! dragged by its title bar, resized from its edges and corners, and put back
//! where the person left it after a restart.
//!
//! Two shapes:
//!
//! * [`dialog`] for panels the person opens (Parts, Standards, Bench output):
//!   a window that keeps clear of the screen edge and never starts smaller
//!   than its content needs.
//! * [`Panel`] for panels that sit on the canvas (the inspectors, the lesson
//!   card, the 3D toolbar): the same window, placed beside the canvas edge
//!   until the person moves it, then left where they put it.
//!
//! Position and size are egui's own area memory, which eframe saves with the
//! settings. "Reset panel layout" (`view.reset_panels`) clears it and the
//! moved marks below.

use std::collections::BTreeMap;

use eframe::egui;

/// Gap between a panel and the canvas edge.
pub(super) const MARGIN: f32 = 12.0;
/// Space kept above a panel for the top bar, and below it for the status bar.
const TOP: f32 = 48.0;
const BOTTOM: f32 = 60.0;
/// The narrowest a panel can be dragged to.
const MIN_WIDTH: f32 = 260.0;
/// The shortest a panel can be dragged to.
const MIN_HEIGHT: f32 = 120.0;
/// A panel the person opens starts at least this wide.
const DIALOG_MIN_WIDTH: f32 = 380.0;

/// Which end of the canvas a panel starts at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Side {
    Left,
    Right,
    BottomLeft,
    BottomRight,
}

/// The egui id of the panel named `name`, for code that asks where it is.
pub(super) fn id(name: &str) -> egui::Id {
    egui::Id::new(("panel", name))
}

/// A panel on the canvas: where it starts, how big, and what it is called.
/// The title bar is the handle for dragging; the id keeps the position
/// between runs.
pub(super) struct Panel {
    id: &'static str,
    title: String,
    canvas: egui::Rect,
    side: Side,
    width: f32,
    collapsible: bool,
    scroll_bar: bool,
    top: f32,
    bottom: f32,
    height: Option<f32>,
    fit: bool,
    title_bar: bool,
    above_card: bool,
}

impl Panel {
    pub(super) fn new(
        id: &'static str,
        title: impl Into<String>,
        canvas: egui::Rect,
        side: Side,
        width: f32,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            canvas,
            side,
            width,
            collapsible: true,
            scroll_bar: false,
            top: TOP,
            bottom: 0.0,
            height: None,
            fit: false,
            title_bar: true,
            above_card: false,
        }
    }

    /// Starts this far below the canvas top instead of under the top bar.
    pub(super) fn top(mut self, offset: f32) -> Self {
        self.top = offset;
        self
    }

    /// Keeps this much of the canvas bottom free (another panel lives there),
    /// so the panel starts no taller than the rest.
    pub(super) fn bottom_room(mut self, room: f32) -> Self {
        self.bottom = room;
        self
    }

    /// Starts this tall (capped to the canvas). egui's own default is 420 for
    /// any window, however little it holds.
    pub(super) fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// As tall and wide as its content (up to the canvas) instead of
    /// resizable: for toolbars and cards. It can still be dragged.
    pub(super) fn fit(mut self) -> Self {
        self.fit = true;
        self
    }

    /// No title bar: the panel has its own header and is dragged by its body.
    pub(super) fn bare(mut self) -> Self {
        self.title_bar = false;
        self
    }

    /// Starts clear of a card in the bottom right corner (the lesson card)
    /// instead of under it: for the panels that open on every selection.
    pub(super) fn above_card(mut self) -> Self {
        self.above_card = true;
        self
    }

    /// A panel that cannot be rolled up to its title.
    pub(super) fn fixed_open(mut self) -> Self {
        self.collapsible = false;
        self
    }

    /// The scroll bar is always drawn, so a form that continues below the
    /// fold shows that there is more.
    pub(super) fn scroll_bar_always(mut self) -> Self {
        self.scroll_bar = true;
        self
    }

    /// The corner the panel hangs from and where that corner starts: against
    /// its side of the canvas, so the panel's own size never matters.
    fn start(&self) -> (egui::Align2, egui::Pos2) {
        let top = self.canvas.top() + self.top;
        let bottom = self.canvas.bottom() - BOTTOM - MARGIN;
        let left = self.canvas.left() + MARGIN;
        let right = self.canvas.right() - MARGIN;
        match self.side {
            Side::Left => (egui::Align2::LEFT_TOP, egui::pos2(left, top)),
            Side::Right => (egui::Align2::RIGHT_TOP, egui::pos2(right, top)),
            Side::BottomLeft => (egui::Align2::LEFT_BOTTOM, egui::pos2(left, bottom)),
            Side::BottomRight => (egui::Align2::RIGHT_BOTTOM, egui::pos2(right, bottom)),
        }
    }

    /// Shows the panel; the body scrolls only when the panel is shorter than
    /// its content. Returns what `add` returned, and None while rolled up.
    pub(super) fn show<R>(
        self,
        ctx: &egui::Context,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<egui::InnerResponse<Option<R>>> {
        let id = id(self.id);
        note_seen(ctx, self.id);
        // A layout preset that was just opened puts the panel where that
        // layout left it, or back at its side when the layout never moved it.
        let taken = take_place(ctx, id);
        let placed = match taken {
            Some(Some(at)) => {
                mark_moved(ctx, id);
                Some(egui::pos2(at[0] as f32, at[1] as f32))
            }
            Some(None) => {
                clear_moved(ctx, id);
                None
            }
            None => None,
        };
        let moved = is_moved(ctx, id);
        let (pivot, start) = self.start();
        // A panel hanging from the right edge stays clear of a card in the
        // bottom right corner, unless the person has moved it.
        let corner = if self.above_card && self.side == Side::Right && !moved {
            reserved_bottom_right(ctx)
        } else {
            0.0
        };
        let max_height =
            (self.canvas.height() - self.top - BOTTOM - self.bottom - corner).max(MIN_HEIGHT);
        let mut window = egui::Window::new(&self.title)
            .id(id)
            .order(egui::Order::Middle)
            .resizable(!self.fit)
            .collapsible(self.collapsible)
            .title_bar(self.title_bar)
            .default_width(self.width)
            .min_width(MIN_WIDTH.min(self.width))
            .min_height(MIN_HEIGHT.min(max_height))
            .max_height(max_height)
            .scroll([false, !self.fit]);
        if self.fit {
            // Without resizing the width is the content's; wrapped rows wrap here.
            window = window.max_width(self.width);
        }
        if let Some(h) = self.height {
            window = window.default_height(h.min(max_height));
        }
        // Placed by us until the first drag (egui would shift a tall window
        // up while it is still sizing); kept on the canvas once it is dragged.
        window = if moved {
            let window = window.constrain_to(self.canvas);
            match placed {
                Some(at) => window.current_pos(at),
                None => window,
            }
        } else {
            window.constrain(false).pivot(pivot).current_pos(start)
        };
        // A window dragged by its title bar keeps the position it had before
        // the frame and ignores the one given here. For the one frame that a
        // layout places it, or that its side of the canvas has moved (a strip
        // appeared above the view), it may be dragged from anywhere, which
        // makes the position given here count (not while a button is down: that may
        // be the person dragging it).
        let drifted = !moved
            && !ctx.input(|i| i.pointer.any_down())
            && ctx
                .memory(|m| m.area_rect(id))
                .is_some_and(|r| corner_of(r, pivot).distance(start) > 1.0);
        if taken.is_some() || drifted {
            window = window.drag_area(egui::containers::WindowDrag::Anywhere);
        }
        // The window's own scroll area has no bar setting, so a solid
        // (non-floating) bar is asked for through the style while it is built.
        let before = self.scroll_bar.then(|| ctx.global_style().spacing.scroll);
        if self.scroll_bar {
            ctx.global_style_mut(|st| st.spacing.scroll = egui::style::ScrollStyle::solid());
        }
        let shown = window.show(ctx, add);
        if let Some(scroll) = before {
            ctx.global_style_mut(|st| st.spacing.scroll = scroll);
        }
        // Only a drag in which the pointer moved counts: a click on a button
        // inside it presses on the window too, and that must not pin it
        // where it was (it would stop following its side of the canvas).
        // With a title bar, egui moves a window by the response of its title
        // (the id `__title_click` under the window's id, an egui internal that
        // 0.36 does not export), not by the area's own response; the
        // `a_dragged_panel_stays_until_the_layout_is_reset` test fails if an
        // egui update renames it.
        let grabbed = shown.as_ref().is_some_and(|r| r.response.dragged())
            || ctx
                .read_response(id.with("__title_click"))
                .is_some_and(|r| r.dragged());
        if shown.is_some()
            && grabbed
            && ctx.input(|i| {
                i.pointer
                    .press_origin()
                    .zip(i.pointer.latest_pos())
                    .is_some_and(|(from, to)| from.distance(to) > 1.0)
            })
        {
            mark_moved(ctx, id);
        }
        shown
    }
}

/// The point of `rect` that `pivot` names: the corner a panel hangs from.
fn corner_of(rect: egui::Rect, pivot: egui::Align2) -> egui::Pos2 {
    egui::pos2(
        rect.min.x + pivot.x().to_factor() * rect.width(),
        rect.min.y + pivot.y().to_factor() * rect.height(),
    )
}

/// How much a dialog's default size grows on a screen wider than the 1366
/// points its sizes were drawn for: in proportion, up to 1.5 times, so a
/// large screen shows more of a table or form instead of scrolling it.
pub(super) fn grow(screen_width: f32) -> f32 {
    (screen_width / 1366.0).clamp(1.0, 1.5)
}

/// A window the person opens from a command: movable by its title bar,
/// resizable, kept on screen and never shorter than a usable minimum.
/// `size` is the default (a height of 0 fits the content); once the person
/// resizes it that is remembered.
pub(super) fn dialog<'a>(
    ctx: &egui::Context,
    title: impl Into<egui::WidgetText>,
    size: [f32; 2],
) -> egui::Window<'a> {
    let title = title.into();
    // egui derives the window's id from `Some(title text)`; `esc` reads it
    // back. The test `escape_closes_the_dialog_on_top` catches a change.
    ctx.data_mut(|d| d.insert_temp(pending_id(), egui::Id::new(Some(title.text()))));
    let screen = ctx.content_rect().shrink(MARGIN);
    let size = [
        size[0] * grow(screen.width()),
        size[1] * grow(screen.width()),
    ];
    let width = size[0].max(DIALOG_MIN_WIDTH).min(screen.width());
    let window = egui::Window::new(title)
        .resizable(true)
        .collapsible(true)
        .min_width(MIN_WIDTH.min(width))
        .max_height((screen.height() - TOP - BOTTOM).max(MIN_HEIGHT))
        .constrain_to(screen)
        .scroll([false, true]);
    // A height of 0 means "as tall as the content", up to the screen.
    if size[1] > 0.0 {
        window.default_size([width, size[1].min((screen.height() - TOP).max(MIN_HEIGHT))])
    } else {
        window.default_width(width)
    }
}

/// For a dialog that sets its own window id: pass the id through here so
/// [`esc`] finds the window.
pub(super) fn keep_id(ctx: &egui::Context, id: egui::Id) -> egui::Id {
    ctx.data_mut(|d| d.insert_temp(pending_id(), id));
    id
}

fn esc_off_id() -> egui::Id {
    egui::Id::new("panel_dialog_esc_off")
}

/// Turns [`esc`] off or on. The lesson replay presses Escape between
/// lessons to clear the canvas and must not close the dialogs the lessons
/// leave open (the enclosure lesson's ring check depends on one of them).
#[cfg(test)]
pub(super) fn set_esc_closes(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(esc_off_id(), !on));
}

fn pending_id() -> egui::Id {
    egui::Id::new("panel_dialog_pending")
}

/// Whether a dialog window is on top, which is then the one Escape belongs
/// to ([`esc`] closes it on the same key press).
pub(super) fn dialog_on_top(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<egui::Id>(pending_id()))
        .is_some_and(|id| {
            let layer = egui::LayerId::new(egui::Order::Middle, id);
            top_middle_layer(ctx) == Some(layer)
        })
}

/// The floating window on top that was drawn last frame. A window that was
/// closed keeps its remembered layer (and its place on top of the others),
/// so the area order alone would leave Escape
/// with nothing to close once the top one is gone.
fn top_middle_layer(ctx: &egui::Context) -> Option<egui::LayerId> {
    ctx.memory(|m| {
        let layers: Vec<_> = m.layer_ids().collect();
        layers
            .into_iter()
            .rev()
            .find(|layer| layer.order == egui::Order::Middle && m.areas().visible_last_frame(layer))
    })
}

/// Escape closes the dialog on top: wrap the `open` flag handed to
/// `.open(..)` right after [`dialog`]. Does nothing while a text field has
/// the keyboard (Escape is theirs then) or when another floating window is on
/// top of the dialog.
pub(super) fn esc<'a>(ctx: &egui::Context, open: &'a mut bool) -> &'a mut bool {
    let id = ctx.data(|d| d.get_temp::<egui::Id>(pending_id()));
    if let Some(id) = id
        && !ctx.data(|d| d.get_temp::<bool>(esc_off_id()).unwrap_or(false))
        && ctx.input(|i| i.key_pressed(egui::Key::Escape))
        && !ctx.egui_wants_keyboard_input()
        && top_middle_layer(ctx) == Some(egui::LayerId::new(egui::Order::Middle, id))
    {
        *open = false;
    }
    open
}

/// "Reset panel layout": every panel goes back to its default place and
/// size.
pub(super) fn reset_layout(ctx: &egui::Context) {
    ctx.memory_mut(|m| m.reset_areas());
    ctx.data_mut(|d| {
        let epoch: u32 = d.get_persisted(epoch_id()).unwrap_or(0);
        d.insert_persisted(epoch_id(), epoch.wrapping_add(1));
    });
}

fn corner_id() -> egui::Id {
    egui::Id::new("panel_corner_reserved")
}

/// The bottom right corner is taken by a card (the lesson card) this tall:
/// panels that start at the right side stay above it instead of drawing
/// under it. 0 gives the corner back. Read on the next frame.
pub(super) fn reserve_bottom_right(ctx: &egui::Context, height: f32) {
    ctx.data_mut(|d| d.insert_temp(corner_id(), height));
}

pub(super) fn reserved_bottom_right(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(corner_id())).unwrap_or(0.0)
}

fn epoch_id() -> egui::Id {
    egui::Id::new("panel_layout_epoch")
}

/// The person dragged the panel: from now on it stays where they left it
/// instead of following its side of the canvas.
pub(super) fn mark_moved(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|d| {
        let epoch: u32 = d.get_persisted(epoch_id()).unwrap_or(0);
        d.insert_persisted(id.with("moved"), epoch);
    });
}

fn clear_moved(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|d| d.remove::<u32>(id.with("moved")));
}

fn seen_id() -> egui::Id {
    egui::Id::new("panel_seen")
}

/// The panels shown so far, so a layout knows which ones to put back.
fn note_seen(ctx: &egui::Context, name: &'static str) {
    ctx.data_mut(|d| {
        let seen = d.get_temp_mut_or_default::<Vec<&'static str>>(seen_id());
        if !seen.contains(&name) {
            seen.push(name);
        }
    });
}

fn seen(ctx: &egui::Context) -> Vec<&'static str> {
    ctx.data(|d| d.get_temp::<Vec<&'static str>>(seen_id()))
        .unwrap_or_default()
}

/// Where a layout preset wants a panel: `Some(top left)` or `None` for its
/// own side. Taken by the panel's next frame.
type Place = Option<[i32; 2]>;

fn place_id(id: egui::Id) -> egui::Id {
    id.with("place")
}

fn take_place(ctx: &egui::Context, id: egui::Id) -> Option<Place> {
    ctx.data_mut(|d| d.remove_temp::<Place>(place_id(id)))
}

/// The panels the person has dragged, with their top left corner in points:
/// what a layout preset keeps of the floating panels. A panel with a
/// placement still waiting is left out; its rectangle is not the layout's yet.
pub(super) fn places(ctx: &egui::Context) -> BTreeMap<String, [i32; 2]> {
    seen(ctx)
        .into_iter()
        .filter(|name| {
            let id = id(name);
            is_moved(ctx, id) && ctx.data(|d| d.get_temp::<Place>(place_id(id))).is_none()
        })
        .filter_map(|name| {
            let rect = ctx.memory(|m| m.area_rect(id(name)))?;
            Some((
                name.to_owned(),
                [rect.min.x.round() as i32, rect.min.y.round() as i32],
            ))
        })
        .collect()
}

/// Puts every panel the app has shown where `layout` keeps it, and those it
/// does not name back at their own side. Each panel takes its placement the
/// next time it is shown, so a panel that is closed now picks it up when it
/// opens.
pub(super) fn place_all(ctx: &egui::Context, layout: &BTreeMap<String, [i32; 2]>) {
    let mut names: Vec<String> = seen(ctx).into_iter().map(str::to_owned).collect();
    names.extend(layout.keys().cloned());
    names.sort();
    names.dedup();
    ctx.data_mut(|d| {
        for name in names {
            let at: Place = layout.get(&name).copied();
            d.insert_temp(place_id(id(&name)), at);
        }
    });
}

pub(super) fn is_moved(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.data_mut(|d| {
        let epoch: u32 = d.get_persisted(epoch_id()).unwrap_or(0);
        d.get_persisted::<u32>(id.with("moved")) == Some(epoch)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialogs_grow_on_big_screens_and_not_on_small_ones() {
        assert_eq!(grow(1366.0), 1.0);
        assert_eq!(grow(1100.0), 1.0);
        assert!((grow(1920.0) - 1.405).abs() < 0.01);
        assert_eq!(grow(3840.0), 1.5);
    }

    #[test]
    fn a_dialog_is_larger_at_1920_than_at_1366_and_fits_both() {
        let width_at = |w: f32| {
            let ctx = egui::Context::default();
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, 1080.0));
            let mut width = 0.0;
            for _ in 0..2 {
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        ..Default::default()
                    },
                    |ctx| {
                        if let Some(r) = dialog(ctx, "Report designer", [1040.0, 640.0])
                            .show(ctx, |ui| ui.label("content"))
                        {
                            width = r.response.rect.width();
                        }
                    },
                );
                out.textures_delta.clear();
            }
            (width, screen.width())
        };
        let (small, small_screen) = width_at(1366.0);
        let (big, _) = width_at(1920.0);
        assert!(small <= small_screen, "{small}");
        assert!(big > small * 1.3, "{small} -> {big}");
    }

    fn canvas() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0))
    }

    fn frame(ctx: &egui::Context, side: Side) -> egui::Rect {
        let mut rect = egui::Rect::NOTHING;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(canvas()),
                ..Default::default()
            },
            |ctx| {
                let panel = Panel::new("test", "Test", canvas(), side, 300.0);
                if let Some(r) = panel.show(ctx, |ui| {
                    ui.label("content");
                }) {
                    rect = r.response.rect;
                }
            },
        );
        out.textures_delta.clear();
        rect
    }

    #[test]
    fn an_unmoved_panel_follows_its_canvas_when_the_canvas_moves_down() {
        let ctx = egui::Context::default();
        let top_with = |ctx: &egui::Context, canvas_top: f32| {
            let area = egui::Rect::from_min_max(egui::pos2(0.0, canvas_top), canvas().max);
            let mut top = 0.0;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(canvas()),
                    ..Default::default()
                },
                |ctx| {
                    let panel = Panel::new("test", "Test", area, Side::Right, 300.0);
                    if let Some(r) = panel.show(ctx, |ui| {
                        ui.label("content");
                    }) {
                        top = r.response.rect.top();
                    }
                },
            );
            out.textures_delta.clear();
            top
        };
        for _ in 0..3 {
            top_with(&ctx, 0.0);
        }
        let first = top_with(&ctx, 0.0);
        let mut later = first;
        for _ in 0..3 {
            later = top_with(&ctx, 66.0);
        }
        assert!((later - first - 66.0).abs() < 1.5, "{first} -> {later}");
    }

    #[test]
    fn a_panel_starts_against_its_side_inside_the_canvas() {
        let ctx = egui::Context::default();
        let left = frame(&ctx, Side::Left);
        assert!(canvas().contains_rect(left), "{left:?}");
        assert!(left.left() <= MARGIN + 1.0);
        let ctx = egui::Context::default();
        let _ = frame(&ctx, Side::Right);
        let right = frame(&ctx, Side::Right);
        assert!(canvas().contains_rect(right), "{right:?}");
        assert!(right.right() >= canvas().right() - MARGIN - 1.0);
    }

    #[test]
    fn bottom_panels_sit_in_the_bottom_corners_of_the_canvas() {
        for side in [Side::BottomLeft, Side::BottomRight] {
            let ctx = egui::Context::default();
            let _ = frame(&ctx, side);
            let rect = frame(&ctx, side);
            assert!(canvas().contains_rect(rect), "{side:?} {rect:?}");
            assert!(
                rect.bottom() > canvas().bottom() - MARGIN - BOTTOM - 2.0,
                "{rect:?}"
            );
            let against_left = rect.left() <= MARGIN + 1.0;
            assert_eq!(against_left, side == Side::BottomLeft, "{rect:?}");
        }
    }

    #[test]
    fn reset_clears_the_moved_mark() {
        let ctx = egui::Context::default();
        let id = egui::Id::new(("panel", "x"));
        assert!(!is_moved(&ctx, id));
        mark_moved(&ctx, id);
        assert!(is_moved(&ctx, id));
        reset_layout(&ctx);
        assert!(!is_moved(&ctx, id));
    }

    /// One frame with the given pointer events; returns the panel's rect.
    fn step(ctx: &egui::Context, events: Vec<egui::Event>) -> egui::Rect {
        let mut rect = egui::Rect::NOTHING;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(canvas()),
                events,
                ..Default::default()
            },
            |ctx| {
                let panel = Panel::new("drag", "Drag", canvas(), Side::Right, 300.0);
                if let Some(r) = panel.show(ctx, |ui| {
                    ui.label("content");
                }) {
                    rect = r.response.rect;
                }
            },
        );
        out.textures_delta.clear();
        rect
    }

    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// One frame of a dialog opened with `open`; `escape` presses Escape.
    fn dialog_frame(ctx: &egui::Context, open: &mut bool, escape: bool) {
        dialog_frame_with(ctx, open, escape, false);
    }

    /// As `dialog_frame`; `own_id` gives the window an id of its own.
    fn dialog_frame_with(ctx: &egui::Context, open: &mut bool, escape: bool, own_id: bool) {
        let events = if escape {
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]
        } else {
            Vec::new()
        };
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(canvas()),
                events,
                ..Default::default()
            },
            |ctx| {
                if *open {
                    let window = dialog(ctx, "Escape test", [400.0, 0.0]);
                    let window = if own_id {
                        window.id(keep_id(ctx, egui::Id::new("own_id_dialog")))
                    } else {
                        window
                    };
                    window.open(esc(ctx, open)).show(ctx, |ui| {
                        ui.label("content");
                    });
                }
            },
        );
        out.textures_delta.clear();
    }

    #[test]
    fn escape_closes_the_dialog_on_top() {
        let ctx = egui::Context::default();
        let mut open = true;
        dialog_frame(&ctx, &mut open, false);
        dialog_frame(&ctx, &mut open, false);
        assert!(open, "stays open without a key");
        dialog_frame(&ctx, &mut open, true);
        assert!(!open, "Escape closes it");
    }

    #[test]
    fn escape_closes_a_dialog_with_its_own_window_id() {
        let ctx = egui::Context::default();
        let mut open = true;
        dialog_frame_with(&ctx, &mut open, false, true);
        dialog_frame_with(&ctx, &mut open, false, true);
        dialog_frame_with(&ctx, &mut open, true, true);
        assert!(!open);
    }

    #[test]
    fn a_dragged_panel_stays_until_the_layout_is_reset() {
        let ctx = egui::Context::default();
        let _ = step(&ctx, vec![]);
        let start = step(&ctx, vec![]);
        let grab = start.center_top() + egui::vec2(0.0, 10.0);
        let _ = step(
            &ctx,
            vec![egui::Event::PointerMoved(grab), button(grab, true)],
        );
        let to = grab + egui::vec2(-300.0, 150.0);
        let _ = step(&ctx, vec![egui::Event::PointerMoved(to)]);
        let _ = step(
            &ctx,
            vec![egui::Event::PointerMoved(to + egui::vec2(-2.0, 2.0))],
        );
        let _ = step(&ctx, vec![button(to, false)]);
        let moved = step(&ctx, vec![]);
        assert!(
            (moved.min - start.min).length() > 100.0,
            "dragged: {start:?} -> {moved:?}"
        );
        assert_eq!(step(&ctx, vec![]).min, moved.min, "it stays after the drag");
        reset_layout(&ctx);
        let _ = step(&ctx, vec![]);
        let back = step(&ctx, vec![]);
        assert!((back.min - start.min).length() < 1.0, "{start:?} {back:?}");
    }

    #[test]
    fn the_command_resets_the_layout() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = super::super::gate1_tests::Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        let id = egui::Id::new(("panel", "x"));
        mark_moved(&ctx, id);
        assert!(is_moved(&ctx, id));
        d.app.run_command(&ctx, "view.reset_panels", None);
        assert!(!is_moved(&ctx, id));
    }

    /// A frame of the panel named `name`; returns its rectangle.
    fn named(ctx: &egui::Context, name: &'static str) -> egui::Rect {
        let mut rect = egui::Rect::NOTHING;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(canvas()),
                ..Default::default()
            },
            |ctx| {
                let panel = Panel::new(name, name, canvas(), Side::Right, 300.0);
                if let Some(r) = panel.show(ctx, |ui| {
                    ui.label("content");
                }) {
                    rect = r.response.rect;
                }
            },
        );
        out.textures_delta.clear();
        rect
    }

    #[test]
    fn a_layout_puts_a_panel_where_it_was_left_and_back_at_its_side() {
        let ctx = egui::Context::default();
        let _ = named(&ctx, "card");
        let home = named(&ctx, "card");
        assert!(places(&ctx).is_empty(), "nothing is dragged yet");

        let layout = BTreeMap::from([("card".to_owned(), [200, 300])]);
        place_all(&ctx, &layout);
        let _ = named(&ctx, "card");
        let moved = named(&ctx, "card");
        assert!(
            (moved.min - egui::pos2(200.0, 300.0)).length() < 1.5,
            "{moved:?}"
        );
        assert_eq!(named(&ctx, "card").min, moved.min, "it stays");
        assert_eq!(
            places(&ctx),
            BTreeMap::from([("card".to_owned(), [200, 300])])
        );

        // A layout that never moved it: back at its own side.
        place_all(&ctx, &BTreeMap::new());
        let _ = named(&ctx, "card");
        let back = named(&ctx, "card");
        assert!((back.min - home.min).length() < 1.5, "{home:?} {back:?}");
        assert!(places(&ctx).is_empty());
    }

    #[test]
    fn a_placement_waits_for_a_panel_that_is_not_open_yet() {
        let ctx = egui::Context::default();
        place_all(&ctx, &BTreeMap::from([("later".to_owned(), [120, 140])]));
        let _ = named(&ctx, "later");
        let r = named(&ctx, "later");
        assert!((r.min - egui::pos2(120.0, 140.0)).length() < 1.5, "{r:?}");
    }
}
