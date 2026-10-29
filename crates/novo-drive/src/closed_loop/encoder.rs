//! Incremental encoder on the motor shaft: quadrature counts, speed from counts over a window.
//!
//! A TTL encoder with `ppr` pulses per revolution gives `4 ppr` counts per revolution after
//! quadrature decoding (exact by definition). The speed is `counts / (4 ppr) * 2 pi / window`,
//! held between windows, so its resolution is one count per window (exact arithmetic). The
//! failure modes are injected by the user (model): signal lost (counts stop) and direction
//! lost (one channel gone: counts only go up).

use std::f64::consts::TAU;

use crate::error::{self, InputError};

/// What the encoder does right now (fault injection for scenarios).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EncoderHealth {
    #[default]
    Healthy,
    /// Cable break or supply lost: no counts at all.
    SignalLost,
    /// One channel lost: counts arrive but only upward, whatever the direction.
    DirectionLost,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Encoder {
    ppr: u32,
    window_s: f64,
    pub health: EncoderHealth,
    counts: i64,
    true_prev: i64,
    window_start_counts: i64,
    elapsed_s: f64,
    speed_rad_s: f64,
}

impl Encoder {
    /// `window_s` is the speed measurement window (at least one drive step).
    pub fn new(ppr: u32, window_s: f64) -> Result<Self, InputError> {
        if ppr == 0 {
            return Err(InputError::NotPositive {
                field: "encoder ppr",
                value: 0.0,
            });
        }
        error::positive("encoder window_s", window_s)?;
        Ok(Self {
            ppr,
            window_s,
            health: EncoderHealth::Healthy,
            counts: 0,
            true_prev: 0,
            window_start_counts: 0,
            elapsed_s: 0.0,
            speed_rad_s: 0.0,
        })
    }

    pub fn ppr(&self) -> u32 {
        self.ppr
    }

    /// Counts per revolution after quadrature decoding: `4 ppr`.
    pub fn counts_per_rev(&self) -> f64 {
        4.0 * f64::from(self.ppr)
    }

    /// Smallest speed step the measurement can see, in rpm: one count per window.
    pub fn speed_resolution_rpm(&self) -> f64 {
        60.0 / (self.counts_per_rev() * self.window_s)
    }

    /// Position as the drive sees it, in rad, quantised to one count.
    pub fn position_rad(&self) -> f64 {
        self.counts as f64 * TAU / self.counts_per_rev()
    }

    pub fn counts(&self) -> i64 {
        self.counts
    }

    /// Speed as the drive sees it, in rad/s, held between windows.
    pub fn speed_rad_s(&self) -> f64 {
        self.speed_rad_s
    }

    /// Reads the true shaft angle (rad) and returns the measured speed (rad/s).
    pub fn update(&mut self, true_angle_rad: f64, dt_s: f64) -> f64 {
        let true_counts = (true_angle_rad * self.counts_per_rev() / TAU).floor() as i64;
        match self.health {
            EncoderHealth::Healthy => self.counts = true_counts,
            EncoderHealth::SignalLost => {}
            EncoderHealth::DirectionLost => {
                self.counts += (true_counts - self.true_prev).abs();
            }
        }
        self.true_prev = true_counts;
        self.elapsed_s += dt_s;
        if self.elapsed_s + 1e-12 >= self.window_s {
            let d = (self.counts - self.window_start_counts) as f64;
            self.speed_rad_s = d * TAU / self.counts_per_rev() / self.elapsed_s;
            self.window_start_counts = self.counts;
            self.elapsed_s = 0.0;
        }
        self.speed_rad_s
    }
}
