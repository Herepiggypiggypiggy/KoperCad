//! Frame governor (3d-look.md section 4): steps the quality tier down when
//! the 95th percentile frame time stays above 20 ms for two seconds, and
//! never steps up by itself.

use crate::palette::QualityTier;

pub const BUDGET_MS: f32 = 20.0;
pub const SUSTAIN_SECS: f32 = 2.0;
const WINDOW: usize = 120;

#[derive(Debug, Clone)]
pub struct FrameGovernor {
    pub tier: QualityTier,
    /// Fixed by the user: the governor only watches.
    pub locked: bool,
    frames: Vec<f32>,
    over_secs: f32,
}

impl FrameGovernor {
    pub fn new(tier: QualityTier, locked: bool) -> Self {
        Self {
            tier,
            locked,
            frames: Vec::with_capacity(WINDOW),
            over_secs: 0.0,
        }
    }

    pub fn p95_ms(&self) -> f32 {
        if self.frames.is_empty() {
            return 0.0;
        }
        let mut v = self.frames.clone();
        v.sort_by(f32::total_cmp);
        v[((v.len() as f32 * 0.95).ceil() as usize).clamp(1, v.len()) - 1]
    }

    /// Record one frame. Returns the new tier when it stepped down.
    pub fn frame(&mut self, ms: f32) -> Option<QualityTier> {
        if self.frames.len() == WINDOW {
            self.frames.remove(0);
        }
        self.frames.push(ms);
        if self.p95_ms() > BUDGET_MS {
            self.over_secs += ms / 1000.0;
        } else {
            self.over_secs = 0.0;
        }
        if self.locked || self.over_secs < SUSTAIN_SECS {
            return None;
        }
        let next = self.tier.step_down()?;
        self.tier = next;
        self.frames.clear();
        self.over_secs = 0.0;
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut FrameGovernor, ms: f32, secs: f32) -> Option<QualityTier> {
        let mut out = None;
        for _ in 0..((secs * 1000.0 / ms) as usize) {
            out = out.or(g.frame(ms));
        }
        out
    }

    #[test]
    fn fast_frames_never_step_down() {
        let mut g = FrameGovernor::new(QualityTier::High, false);
        assert_eq!(run(&mut g, 8.0, 10.0), None);
    }

    #[test]
    fn two_seconds_of_slow_frames_step_down_one_tier() {
        let mut g = FrameGovernor::new(QualityTier::High, false);
        assert_eq!(run(&mut g, 30.0, 3.0), Some(QualityTier::Standard));
        assert_eq!(g.tier, QualityTier::Standard);
    }

    #[test]
    fn a_short_hitch_does_not() {
        let mut g = FrameGovernor::new(QualityTier::High, false);
        assert_eq!(run(&mut g, 30.0, 1.0), None);
        assert_eq!(run(&mut g, 8.0, 5.0), None);
    }

    #[test]
    fn low_and_locked_tiers_stay_put() {
        let mut g = FrameGovernor::new(QualityTier::Low, false);
        assert_eq!(run(&mut g, 40.0, 5.0), None);
        let mut l = FrameGovernor::new(QualityTier::High, true);
        assert_eq!(run(&mut l, 40.0, 5.0), None);
    }

    #[test]
    fn it_never_steps_up() {
        let mut g = FrameGovernor::new(QualityTier::Standard, false);
        run(&mut g, 4.0, 10.0);
        assert_eq!(g.tier, QualityTier::Standard);
    }
}
