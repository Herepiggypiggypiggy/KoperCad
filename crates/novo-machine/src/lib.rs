//! Machine and crane engine (skeleton, plan `docs/design/drive-crane-simulation.md` lane L0).
//!
//! Lanes: L4 kinematics (`crane`), L4/L5 schematic `bindings`, L5 co-simulation `cosim`.

pub mod bindings;
pub mod cosim;
pub mod crane;
pub mod hoist_loop;
pub mod port;

#[cfg(test)]
mod tests {
    #[test]
    fn pendulum_four_metres_is_about_four_seconds() {
        let t = super::crane::pendulum_period_s(4.0).unwrap();
        assert!((t - 4.01).abs() < 0.01, "{t}");
    }

    #[test]
    fn pendulum_rejects_bad_length() {
        assert!(super::crane::pendulum_period_s(-1.0).is_err());
        assert!(super::crane::pendulum_period_s(0.0).is_err());
        assert!(super::crane::pendulum_period_s(f64::NAN).is_err());
    }
}
