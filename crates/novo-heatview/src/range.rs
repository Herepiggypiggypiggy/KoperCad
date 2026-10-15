//! Legend range and ticks: round numbers, a range that only grows while a
//! run is going, and the level lists contours are drawn at.

/// Colour range of a map, degrees C.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub lo: f64,
    pub hi: f64,
}

impl Range {
    /// Position of `v` in the range, 0 to 1 (clamped).
    pub fn t(&self, v: f64) -> f32 {
        if self.hi <= self.lo {
            return 0.0;
        }
        (((v - self.lo) / (self.hi - self.lo)).clamp(0.0, 1.0)) as f32
    }
}

/// The map range: ambient at the bottom; at the top the larger of the hottest
/// value rounded up to the next 5 K and the highest device limit (so every
/// limit sits on the legend), at least 10 K above the bottom.
pub fn map_range(ambient: f64, hottest: f64, highest_limit: Option<f64>) -> Range {
    let top = (hottest / 5.0).ceil() * 5.0;
    let top = highest_limit.map_or(top, |l| top.max(l));
    Range {
        lo: ambient,
        hi: top.max(ambient + 10.0),
    }
}

/// Grow-only range for a running solve: the legend never shrinks, so the
/// picture does not flicker as the hot spot sharpens.
pub fn grow(previous: Option<Range>, next: Range) -> Range {
    match previous {
        None => next,
        Some(p) => Range {
            lo: p.lo.min(next.lo),
            hi: p.hi.max(next.hi),
        },
    }
}

/// Legend ticks: every 10 K, every 5 K when the span is under 30 K.
pub fn ticks(range: Range) -> Vec<f64> {
    let step = if range.hi - range.lo < 30.0 {
        5.0
    } else {
        10.0
    };
    levels(range, step)
}

/// Most levels one list may hold.
const MAX_LEVELS: usize = 10_000;

/// Multiples of `step` inside the range (inclusive).
pub fn levels(range: Range, step: f64) -> Vec<f64> {
    // A diverged solve can hand over infinite or absurd ends: no levels,
    // never an allocation of that size.
    if step <= 0.0 || !range.lo.is_finite() || !range.hi.is_finite() || range.hi < range.lo {
        return Vec::new();
    }
    if (range.hi - range.lo) / step > MAX_LEVELS as f64 {
        return Vec::new();
    }
    let first = (range.lo / step).ceil() as i64;
    let last = (range.hi / step).floor() as i64;
    (first..=last).map(|k| k as f64 * step).collect()
}

/// Contour levels: every 5 K, thinned to at most `max` by doubling the step.
/// Returns `(level, labelled)`; every second level is labelled.
pub fn contour_levels(range: Range, max: usize) -> Vec<(f64, bool)> {
    let mut step = 5.0;
    let mut all = levels(range, step);
    while all.len() > max && step < 1000.0 {
        step *= 2.0;
        all = levels(range, step);
    }
    all.into_iter()
        .map(|l| (l, ((l / step).round() as i64) % 2 == 0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_rounds_up_and_includes_limits() {
        let r = map_range(35.0, 71.2, Some(90.0));
        assert_eq!((r.lo, r.hi), (35.0, 90.0));
        let r = map_range(35.0, 71.2, Some(60.0));
        assert_eq!(r.hi, 75.0);
        assert_eq!(map_range(35.0, 36.0, None).hi, 45.0, "at least 10 K");
    }

    #[test]
    fn range_only_grows_while_running() {
        let a = Range { lo: 35.0, hi: 70.0 };
        let b = Range { lo: 35.0, hi: 60.0 };
        assert_eq!(grow(Some(a), b), a);
        assert_eq!(grow(Some(b), a), a);
        assert_eq!(grow(None, b), b);
    }

    #[test]
    fn ticks_are_round() {
        assert_eq!(
            ticks(Range { lo: 35.0, hi: 85.0 }),
            vec![40.0, 50.0, 60.0, 70.0, 80.0]
        );
        assert_eq!(
            ticks(Range { lo: 35.0, hi: 55.0 }),
            vec![35.0, 40.0, 45.0, 50.0, 55.0]
        );
    }

    #[test]
    fn contours_are_thinned_and_alternate_labels() {
        let c = contour_levels(Range { lo: 35.0, hi: 85.0 }, 8);
        let levels: Vec<f64> = c.iter().map(|l| l.0).collect();
        assert_eq!(
            levels,
            vec![40.0, 50.0, 60.0, 70.0, 80.0],
            "11 levels thinned to a 10 K step"
        );
        let labelled: Vec<f64> = c.iter().filter(|l| l.1).map(|l| l.0).collect();
        assert_eq!(labelled, vec![40.0, 60.0, 80.0]);
    }

    #[test]
    fn a_diverged_range_gives_no_levels() {
        assert!(
            levels(
                Range {
                    lo: 35.0,
                    hi: f64::INFINITY
                },
                5.0
            )
            .is_empty()
        );
        assert!(
            levels(
                Range {
                    lo: f64::NAN,
                    hi: 80.0
                },
                5.0
            )
            .is_empty()
        );
        assert!(levels(Range { lo: 0.0, hi: 1e15 }, 5.0).is_empty());
        assert!(
            ticks(Range {
                lo: 0.0,
                hi: f64::INFINITY
            })
            .is_empty()
        );
        assert!(
            contour_levels(
                Range {
                    lo: 0.0,
                    hi: f64::INFINITY
                },
                8
            )
            .is_empty()
        );
    }

    #[test]
    fn t_clamps() {
        let r = Range { lo: 0.0, hi: 10.0 };
        assert_eq!(r.t(-1.0), 0.0);
        assert_eq!(r.t(5.0), 0.5);
        assert_eq!(r.t(99.0), 1.0);
    }
}
