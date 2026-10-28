//! Machine and crane engine (skeleton, plan `docs/design/drive-crane-simulation.md` lane L0).
//!
//! Lanes: L4 kinematics (`crane`), L4/L5 schematic `bindings`, L5 co-simulation `cosim`.

pub mod bindings;
pub mod cosim;
pub mod crane;

#[cfg(test)]
mod tests {
    #[test]
    fn pendulum_four_metres_is_about_four_seconds() {
        let t = super::crane::pendulum_period_s(4.0);
        assert!((t - 4.01).abs() < 0.01, "{t}");
    }
}
