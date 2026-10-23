//! Small shared helpers for interface motion: easing and the fade of a
//! short-lived element. Everything here is pure, so it is tested without a
//! window. With Reduce motion on, `toast_alpha` is always fully opaque and
//! `toast_settled` is always true: nothing eases and nothing asks for repaints.

use eframe::egui;
use std::time::Duration;

/// How long a new toast takes to fade in.
pub(super) const FADE_IN: Duration = Duration::from_millis(160);
/// How long the end of a toast's life is spent fading out.
pub(super) const FADE_OUT: Duration = Duration::from_millis(300);

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
}
