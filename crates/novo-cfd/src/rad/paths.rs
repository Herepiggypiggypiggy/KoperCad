//! The heat of a body split by path.

/// Where a body's heat goes, watts. Convection is what is left once the
/// conduction into the plate, rails and walls and the net radiation are
/// taken off, so the three always add up to the total (the energy balance
/// of the flow solver decides how far the remainder can be trusted).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HeatPaths {
    pub total_w: f64,
    pub conduction_w: f64,
    pub radiation_w: f64,
}

impl HeatPaths {
    pub fn new(total_w: f64, conduction_w: f64, radiation_w: f64) -> HeatPaths {
        HeatPaths {
            total_w,
            conduction_w,
            radiation_w,
        }
    }

    /// Convection into the air, W.
    pub fn convection_w(&self) -> f64 {
        self.total_w - self.conduction_w - self.radiation_w
    }

    /// Shares `(convection, conduction, radiation)` of the total; zeros for
    /// a body that gives off no heat.
    pub fn shares(&self) -> (f64, f64, f64) {
        if self.total_w.abs() < 1e-12 {
            return (0.0, 0.0, 0.0);
        }
        (
            self.convection_w() / self.total_w,
            self.conduction_w / self.total_w,
            self.radiation_w / self.total_w,
        )
    }
}
