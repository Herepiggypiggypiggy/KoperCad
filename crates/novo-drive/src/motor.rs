//! Induction motor at variable frequency (lane L2).

use crate::Fidelity;

/// Synchronous speed in rpm for a supply frequency and pole count (`120 f / p`).
pub fn sync_rpm(frequency_hz: f64, poles: u32) -> f64 {
    120.0 * frequency_hz / f64::from(poles)
}

/// Fidelity of [`sync_rpm`].
pub const SYNC_RPM_FIDELITY: Fidelity = Fidelity::Exact;
