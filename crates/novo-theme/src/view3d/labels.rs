//! Label layout and highlight state (3d-look.md section 5), GPU free.
//!
//! The renderer projects each device to a screen rectangle and hands the
//! candidates here; this decides which labels show and how opaque they are.

/// Labels show only when the device is at least this wide on screen.
pub const MIN_DEVICE_PX: f32 = 28.0;
pub const LABEL_FADE_SECS: f32 = 0.12;
pub const HOVER_FADE_SECS: f32 = 0.08;

/// A device as seen on screen this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelCandidate {
    pub id: u64,
    /// Screen rectangle of the label chip: x, y, width, height in pixels.
    pub rect: [f32; 4],
    /// Width of the device on screen, pixels.
    pub device_px: f32,
    pub selected: bool,
    pub hovered: bool,
}

impl LabelCandidate {
    fn priority(&self) -> (u8, u32) {
        let class = if self.selected {
            2
        } else if self.hovered {
            1
        } else {
            0
        };
        // Larger devices win within a class.
        (class, self.device_px as u32)
    }

    fn overlaps(&self, other: &Self) -> bool {
        let (a, b) = (self.rect, other.rect);
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    }
}

/// Ids of the labels to show, best first. Selected and hovered devices
/// always show (hover also overrides the size rule); others need
/// [`MIN_DEVICE_PX`] and must not overlap a label that already won.
pub fn visible_labels(candidates: &[LabelCandidate]) -> Vec<u64> {
    let mut order: Vec<&LabelCandidate> = candidates
        .iter()
        .filter(|c| c.selected || c.hovered || c.device_px >= MIN_DEVICE_PX)
        .collect();
    order.sort_by(|a, b| b.priority().cmp(&a.priority()).then(a.id.cmp(&b.id)));
    let mut shown: Vec<&LabelCandidate> = Vec::new();
    for c in order {
        let forced = c.selected || c.hovered;
        if forced || !shown.iter().any(|s| s.overlaps(c)) {
            shown.push(c);
        }
    }
    shown.iter().map(|c| c.id).collect()
}

/// A value easing towards 0 or 1 over a fixed time, for label opacity and
/// the hover outline. Reduced motion makes it jump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fade {
    pub value: f32,
    secs: f32,
}

impl Fade {
    pub fn label() -> Self {
        Self {
            value: 0.0,
            secs: LABEL_FADE_SECS,
        }
    }

    pub fn hover() -> Self {
        Self {
            value: 0.0,
            secs: HOVER_FADE_SECS,
        }
    }

    pub fn step(&mut self, on: bool, dt: f32, reduced_motion: bool) {
        let target = if on { 1.0 } else { 0.0 };
        if reduced_motion {
            self.value = target;
        } else {
            let d = dt / self.secs;
            self.value = if on {
                (self.value + d).min(1.0)
            } else {
                (self.value - d).max(0.0)
            };
        }
    }
}

/// What the overlay pass draws around one object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Highlight {
    /// Outline width in pixels and colour (straight sRGB).
    pub outline_px: f32,
    pub outline: [u8; 4],
    /// Face tint opacity (copper 12% when selected).
    pub tint: f32,
}

/// The look of an object's state, `hover_fade` scaling the hover outline.
/// Selected wins over hover.
pub fn highlight(selected: bool, hover_fade: f32) -> Option<Highlight> {
    use super::palette::{HOVER, SELECTION};
    if selected {
        Some(Highlight {
            outline_px: 2.0,
            outline: SELECTION,
            tint: 0.12,
        })
    } else if hover_fade > 0.0 {
        let mut c = HOVER;
        c[3] = (255.0 * hover_fade.clamp(0.0, 1.0)).round() as u8;
        Some(Highlight {
            outline_px: 1.5,
            outline: c,
            tint: 0.0,
        })
    } else {
        None
    }
}

/// Opacity of everything not selected in focus mode.
pub const FOCUS_DIM: f32 = 0.70;

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: u64, x: f32, px: f32) -> LabelCandidate {
        LabelCandidate {
            id,
            rect: [x, 0.0, 40.0, 20.0],
            device_px: px,
            selected: false,
            hovered: false,
        }
    }

    #[test]
    fn small_devices_get_no_label_unless_hovered() {
        let mut small = cand(1, 0.0, 10.0);
        assert!(visible_labels(&[small.clone()]).is_empty());
        small.hovered = true;
        assert_eq!(visible_labels(&[small]), vec![1]);
    }

    #[test]
    fn overlaps_resolve_by_selected_then_hovered_then_size() {
        let mut a = cand(1, 0.0, 90.0);
        let b = cand(2, 10.0, 60.0);
        assert_eq!(visible_labels(&[a.clone(), b.clone()]), vec![1]);
        let mut b2 = b.clone();
        b2.hovered = true;
        assert_eq!(visible_labels(&[a.clone(), b2.clone()]), vec![2]);
        a.selected = true;
        let both = visible_labels(&[a, b2]);
        assert_eq!(both[0], 1);
    }

    #[test]
    fn separate_labels_all_show() {
        let v = visible_labels(&[
            cand(1, 0.0, 50.0),
            cand(2, 100.0, 50.0),
            cand(3, 200.0, 30.0),
        ]);
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn the_result_is_stable_for_equal_priority() {
        let v1 = visible_labels(&[cand(2, 0.0, 50.0), cand(1, 10.0, 50.0)]);
        let v2 = visible_labels(&[cand(1, 10.0, 50.0), cand(2, 0.0, 50.0)]);
        assert_eq!(v1, v2);
    }

    #[test]
    fn fades_take_their_time_and_jump_with_reduced_motion() {
        let mut f = Fade::label();
        f.step(true, 0.06, false);
        assert!((f.value - 0.5).abs() < 1e-6);
        f.step(true, 1.0, false);
        assert_eq!(f.value, 1.0);
        f.step(false, 0.0, true);
        assert_eq!(f.value, 0.0);
    }

    #[test]
    fn selection_is_copper_and_wins_over_hover() {
        let h = highlight(true, 1.0).unwrap();
        assert_eq!(h.outline_px, 2.0);
        assert_eq!(h.outline, super::super::palette::SELECTION);
        assert!((h.tint - 0.12).abs() < 1e-6);
    }

    #[test]
    fn hover_is_a_thinner_blue_outline_that_fades_in() {
        let h = highlight(false, 0.5).unwrap();
        assert_eq!(h.outline_px, 1.5);
        assert_eq!(h.outline[..3], super::super::palette::HOVER[..3]);
        assert!(h.outline[3] > 100 && h.outline[3] < 160);
        assert!(highlight(false, 0.0).is_none());
    }
}
