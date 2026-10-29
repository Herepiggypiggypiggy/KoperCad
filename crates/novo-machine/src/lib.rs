//! Machine and crane engine (skeleton, plan `docs/design/drive-crane-simulation.md` lane L0).
//!
//! Lanes: L2c `sync_winch` (two closed-loop winches on one load), `cl_winch_spec` and `cl_winch_loop` (a closed-loop winch driven by the drawn circuit), L4 kinematics (`crane`), L4/L5 schematic `bindings`, L5 co-simulation `cosim`.

pub mod bindings;
pub mod cl_winch_loop;
pub mod cl_winch_spec;
pub mod cosim;
pub mod crane;
pub mod drive_hoist_loop;
pub mod drive_hoist_spec;
pub mod hoist_loop;
pub mod port;
pub mod spec;
pub mod swing;
pub mod sync_winch;
pub mod travel_loop;
pub mod travel_spec;
pub mod winch_scenarios;

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
