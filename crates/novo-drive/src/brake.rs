//! Spring-applied holding brake (lane L2).

/// Brake state seen by the mechanics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrakeState {
    Applied,
    Released,
}
