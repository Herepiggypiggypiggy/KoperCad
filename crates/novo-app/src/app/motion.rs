//! The clock for ambient motion (spinning fans, air flowing through an
//! opening, the drill). One place decides whether things move, so the setting
//! "Reduce motion" and the test harness both give the same still picture.

use eframe::egui;

/// Frame spacing while something moves: 30 pictures a second is smooth for a
/// slow spin and half the cost of a full redraw rate.
pub(super) const FRAME: std::time::Duration = std::time::Duration::from_millis(33);

/// Seconds on the animation clock, or `None` for a still picture.
pub(super) fn clock(ctx: &egui::Context, reduce_motion: bool) -> Option<f64> {
    // A window nobody is looking at stops redrawing.
    if reduce_motion || crate::control::still_pictures() || !ctx.input(|i| i.focused) {
        return None;
    }
    Some(ctx.input(|i| i.time))
}
