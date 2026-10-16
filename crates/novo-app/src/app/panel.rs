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
        let moved = is_moved(ctx, id);
        let (pivot, start) = self.start();
        let max_height = (self.canvas.height() - self.top - BOTTOM - self.bottom).max(MIN_HEIGHT);
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
            window.constrain_to(self.canvas)
        } else {
            window.constrain(false).pivot(pivot).current_pos(start)
        };
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
        if let Some(r) = &shown
            && r.response.drag_started()
        {
            mark_moved(ctx, id);
        }
        shown
    }
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
    let screen = ctx.content_rect().shrink(MARGIN);
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

/// "Reset panel layout": every panel goes back to its default place and
/// size.
pub(super) fn reset_layout(ctx: &egui::Context) {
    ctx.memory_mut(|m| m.reset_areas());
    ctx.data_mut(|d| {
        let epoch: u32 = d.get_persisted(epoch_id()).unwrap_or(0);
        d.insert_persisted(epoch_id(), epoch.wrapping_add(1));
    });
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

pub(super) fn is_moved(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.data_mut(|d| {
        let epoch: u32 = d.get_persisted(epoch_id()).unwrap_or(0);
        d.get_persisted::<u32>(id.with("moved")) == Some(epoch)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
