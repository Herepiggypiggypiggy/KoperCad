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
const BOTTOM: f32 = 28.0;
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
        }
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

    /// Where the panel's top left corner starts: against its side, with the
    /// width it had last frame (a right-hand panel is as wide as its
    /// content, so the corner depends on it).
    fn start(&self, ctx: &egui::Context, id: egui::Id) -> egui::Pos2 {
        let width = ctx
            .memory(|m| m.area_rect(id))
            .map_or(self.width, |r| r.width());
        let y = self.canvas.top() + TOP;
        match self.side {
            Side::Left => egui::pos2(self.canvas.left() + MARGIN, y),
            Side::Right => egui::pos2(self.canvas.right() - MARGIN - width, y),
        }
    }

    /// Shows the panel; the body scrolls only when the panel is shorter than
    /// its content. Returns what `add` returned, and None while rolled up.
    pub(super) fn show<R>(
        self,
        ctx: &egui::Context,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<egui::InnerResponse<Option<R>>> {
        let id = egui::Id::new(("panel", self.id));
        let moved = is_moved(ctx, id);
        let start = self.start(ctx, id);
        let max_height = (self.canvas.height() - TOP - BOTTOM).max(MIN_HEIGHT);
        let mut window = egui::Window::new(&self.title)
            .id(id)
            .order(egui::Order::Middle)
            .resizable(true)
            .collapsible(self.collapsible)
            .default_width(self.width)
            .min_width(MIN_WIDTH.min(self.width))
            .min_height(MIN_HEIGHT.min(max_height))
            .max_height(max_height)
            .constrain_to(self.canvas)
            .scroll([false, true]);
        window = if moved {
            window
        } else {
            window.current_pos(start)
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
fn mark_moved(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|d| {
        let epoch: u32 = d.get_persisted(epoch_id()).unwrap_or(0);
        d.insert_persisted(id.with("moved"), epoch);
    });
}

fn is_moved(ctx: &egui::Context, id: egui::Id) -> bool {
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
