//! Small shared helpers for interface motion: easing and the fade of a
//! short-lived element. Everything here is pure, so it is tested without a
//! window. The ambient clock (`clock`, `FRAME`) is the one place that decides
//! whether fans, air and the drill cue move. With Reduce motion on, `toast_alpha` is always fully opaque and
//! `toast_settled` is always true: nothing eases and nothing asks for repaints.

use eframe::egui;
use std::time::Duration;

/// How long a new toast takes to fade in.
pub(super) const FADE_IN: Duration = Duration::from_millis(160);
/// How long the end of a toast's life is spent fading out.
pub(super) const FADE_OUT: Duration = Duration::from_millis(300);

/// Frame spacing while something moves: 30 pictures a second is smooth for a
/// slow spin and half the cost of a full redraw rate.
pub(super) const FRAME: Duration = Duration::from_millis(33);

/// Seconds on the animation clock, or `None` for a still picture: Reduce
/// motion, the test harness (see `control::still_pictures`), or a window
/// nobody is looking at.
pub(super) fn clock(ctx: &egui::Context, reduce_motion: bool) -> Option<f64> {
    if reduce_motion || crate::control::still_pictures() || !ctx.input(|i| i.focused) {
        return None;
    }
    Some(ctx.input(|i| i.time))
}

/// How long a jump on a 2D canvas (an outline row, a problem, a cross
/// reference) glides to its target; none with Reduce motion or under the
/// test harness, so a jump lands at once. Unit tests run with the egui clock
/// stopped, so a glide there would never arrive: they get none either (the
/// canvas has its own glide test).
pub(super) fn glide_secs(reduce_motion: bool) -> f32 {
    if reduce_motion || crate::control::still_pictures() || cfg!(test) {
        0.0
    } else {
        GLIDE_SECS
    }
}

/// The glide time of a jump.
pub(super) const GLIDE_SECS: f32 = 0.22;

/// Ease-out: fast at the start, settling gently. `t` is clamped to 0..=1.
pub(super) fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}

/// Opacity of a toast `age` after it was raised, with `left` of its life to
/// go. `None` for `left` is a toast that stays until clicked.
pub(super) fn toast_alpha(age: Duration, left: Option<Duration>, reduced: bool) -> f32 {
    if reduced {
        return 1.0;
    }
    let fade_in = ease_out(age.as_secs_f32() / FADE_IN.as_secs_f32());
    let fade_out = left.map_or(1.0, |l| (l.as_secs_f32() / FADE_OUT.as_secs_f32()).min(1.0));
    fade_in.min(fade_out).clamp(0.0, 1.0)
}

/// Whether the toast is between its fades, so no repaint is needed for it.
pub(super) fn toast_settled(age: Duration, left: Option<Duration>, reduced: bool) -> bool {
    reduced || (age >= FADE_IN && left.is_none_or(|l| l >= FADE_OUT))
}

/// The toast's box at opacity `alpha`: fill, border and shadow all fade with
/// the content (`Ui::set_opacity` alone leaves the box opaque).
pub(super) fn toast_frame(style: &egui::Style, alpha: f32) -> egui::Frame {
    egui::Frame::popup(style).multiply_with_opacity(alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_jump_glides_for_about_a_fifth_of_a_second_and_never_with_reduce_motion() {
        assert_eq!(glide_secs(true), 0.0);
        // Under `cfg(test)` the app glide is off, so read the constant's intent
        // from the source: 0.22 s.
        assert_eq!(GLIDE_SECS, 0.22);
    }

    #[test]
    fn the_toast_box_fades_with_its_text() {
        let style = egui::Style::default();
        let full = toast_frame(&style, 1.0);
        let half = toast_frame(&style, 0.5);
        let none = toast_frame(&style, 0.0);
        assert_eq!(full.fill, egui::Frame::popup(&style).fill);
        assert!(half.fill.a() < full.fill.a() && half.fill.a() > 0);
        assert!(half.stroke.color.a() < full.stroke.color.a());
        assert!(half.shadow.color.a() < full.shadow.color.a());
        assert_eq!(none.fill.a(), 0);
        assert_eq!(none.stroke.color.a(), 0);
        assert_eq!(none.shadow.color.a(), 0);
    }

    const MS: fn(u64) -> Duration = Duration::from_millis;

    #[test]
    fn a_new_toast_fades_in_and_then_stays_opaque() {
        let long = Some(Duration::from_secs(4));
        assert_eq!(toast_alpha(MS(0), long, false), 0.0);
        let mid = toast_alpha(MS(60), long, false);
        assert!(mid > 0.3 && mid < 1.0, "{mid}");
        assert_eq!(toast_alpha(MS(500), long, false), 1.0);
    }

    #[test]
    fn a_toast_fades_out_over_its_last_moments() {
        let end = toast_alpha(MS(5000), Some(MS(150)), false);
        assert!((end - 0.5).abs() < 0.01, "{end}");
        assert_eq!(toast_alpha(MS(5000), Some(MS(0)), false), 0.0);
    }

    #[test]
    fn a_toast_that_stays_until_clicked_never_fades_out() {
        assert_eq!(toast_alpha(MS(10_000), None, false), 1.0);
        assert!(toast_settled(MS(10_000), None, false));
    }

    #[test]
    fn reduced_motion_is_always_opaque_and_settled() {
        assert_eq!(toast_alpha(MS(0), Some(MS(0)), true), 1.0);
        assert!(toast_settled(MS(0), Some(MS(0)), true));
    }

    #[test]
    fn settled_is_false_only_inside_a_fade() {
        assert!(!toast_settled(MS(10), Some(MS(4000)), false));
        assert!(!toast_settled(MS(4000), Some(MS(100)), false));
        assert!(toast_settled(MS(1000), Some(MS(4000)), false));
    }

    #[test]
    fn the_clock_is_a_still_picture_with_reduce_motion_or_without_focus() {
        let ctx = egui::Context::default();
        assert_eq!(clock(&ctx, true), None);
        // A context nobody has focused yet is not a window being watched.
        assert_eq!(clock(&ctx, false), None);
    }
}
