//! Motor current and heating of a contactor hoist (lane L2d): stator current from the fitted
//! equivalent circuit, current on two phases, a two-node winding and frame heat model with a
//! self-ventilated cooling law, and a thermal overload relay with a trip class.
//!
//! Exact by definition: a wye motor on two phases at standstill draws `sqrt(3)/2` of its
//! three-phase locked-rotor current (the two windings in series across one line voltage); the
//! steady-state temperature rise of the two nodes (`P / G`); the trip time of the relay at a
//! constant current (closed form of its first-order model). Model: the equivalent circuit
//! parameters fitted from the nameplate, the rule for two-phase running current, copper-loss
//! heating by the stator and rotor losses of the circuit, the thermal resistances and
//! capacitances (generic defaults, to be replaced by data), and the relay curve.

use crate::Fidelity;
use crate::drive::MotorNameplate;
use crate::error::{self, InputError};
use crate::induction::{FitOptions, FitReport, InductionMotor, Supply as LineSupply};
use crate::motor::Direction;
use crate::supply::Supply;

/// Fidelity of every current, loss and temperature in this module except the exact items above.
pub const THERMAL_FIDELITY: Fidelity = Fidelity::Model;

/// What the current and heat model leaves out, shown next to the numbers in the UI.
pub const THERMAL_MODEL_LEAVES_OUT: &[&str] = &[
    "A rated current left empty is derived from the power with an assumed efficiency (fine 0.6, coarse 0.85, fine-lift 0.7), not a measured one; type the nameplate value to replace it",
    "The overload relay protects the main motor only; the fine-lift motor has current and heat but no relay of its own",
    "Current and losses come from an equivalent circuit fitted to the nameplate; the hoist torque uses its own fit, so the two agree near the rated point only",
    "Current on two phases: the lower of 1.73 times the three-phase current and 0.87 times the three-phase locked-rotor current (exact at standstill for a wye motor); a rule of thumb, not a double-field solution",
    "Heat: two lumped nodes (winding, frame) with generic resistances and capacities; no fan curve, no end-winding detail, no temperature effect on resistance",
    "The winding rise at which a real motor is damaged depends on its insulation class, age and mounting; the limit is the number you enter",
    "Overload relay: a first-order thermal model with a trip class (time to trip at 7.2 times the setting); no phase-loss sensitivity, no ambient compensation",
];

/// Ratio of two-phase locked-rotor current to three-phase locked-rotor current of a wye
/// motor: `sqrt(3) / 2`. Exact for equal phase impedances.
pub const TWO_PHASE_LOCKED_RATIO: f64 = 0.866_025_403_784_438_6;

/// One winding's electrical model.
#[derive(Debug, Clone, PartialEq)]
pub struct WindingElectrics {
    motor: InductionMotor,
    frequency_hz: f64,
    volts_ll: f64,
    rated_current_a: f64,
    rated_losses_w: f64,
    rated_rpm: f64,
}

impl WindingElectrics {
    /// Fits the circuit to a nameplate with typical small-motor assumptions
    /// ([`FitOptions::typical`]).
    pub fn from_nameplate(
        np: &MotorNameplate,
        poles: u32,
    ) -> Result<(Self, FitReport), InputError> {
        let (motor, report) =
            InductionMotor::from_nameplate(np, Some(poles), &FitOptions::typical())?;
        let supply = LineSupply::new(np.frequency_hz, np.voltage_v, Direction::Positive)?;
        let rated = motor.evaluate(&supply, np.rpm * std::f64::consts::TAU / 60.0)?;
        Ok((
            Self {
                motor,
                frequency_hz: np.frequency_hz,
                volts_ll: np.voltage_v,
                rated_current_a: np.current_a,
                rated_losses_w: rated.losses_w(),
                rated_rpm: np.rpm,
            },
            report,
        ))
    }

    pub fn rated_current_a(&self) -> f64 {
        self.rated_current_a
    }

    /// Total losses at the rated point, W.
    pub fn rated_losses_w(&self) -> f64 {
        self.rated_losses_w
    }

    pub fn rated_rpm(&self) -> f64 {
        self.rated_rpm
    }

    /// Three-phase locked-rotor current at `voltage_ratio` of the rated voltage, A.
    pub fn locked_rotor_current_a(&self, voltage_ratio: f64) -> Result<f64, InputError> {
        let s = LineSupply::new(
            self.frequency_hz,
            self.volts_ll * voltage_ratio,
            Direction::Positive,
        )?;
        Ok(self.motor.evaluate(&s, 0.0)?.stator_current_a)
    }

    /// Line current in A and total losses in W at shaft speed `omega_rad_s` (signed, positive
    /// in the rising sense) with the field turning in `direction`.
    pub fn current_and_losses(
        &self,
        direction: Direction,
        omega_rad_s: f64,
        supply: &Supply,
    ) -> Result<(f64, f64), InputError> {
        let ratio = supply.voltage_ratio();
        let s = LineSupply::new(self.frequency_hz, self.volts_ll * ratio, direction)?;
        let three = self.motor.evaluate(&s, omega_rad_s)?;
        if !supply.phase_lost() || three.stator_current_a <= 0.0 {
            return Ok((three.stator_current_a, three.losses_w()));
        }
        // Two phases: rule of thumb, see the module doc.
        let locked = self.locked_rotor_current_a(ratio)?;
        let i3 = three.stator_current_a;
        let i2 = (3f64.sqrt() * i3).min(TWO_PHASE_LOCKED_RATIO * locked);
        // Copper of two phases carrying i2 against three carrying i3.
        let losses = three.losses_w() * (2.0 * i2 * i2) / (3.0 * i3 * i3);
        Ok((i2, losses))
    }
}

/// Heat model parameters. The defaults are generic and labelled so in the UI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalParams {
    /// Winding rise over ambient at rated load and full cooling, K.
    pub winding_rise_rated_k: f64,
    /// Part of that rise between winding and frame, K.
    pub winding_to_frame_k: f64,
    /// Time constant of the winding node against the frame, s.
    pub tau_winding_s: f64,
    /// Time constant of the frame node against ambient at full cooling, s.
    pub tau_frame_s: f64,
    /// Share of the full cooling the motor has at standstill (self-ventilated, 0.3 to 0.5).
    pub standstill_cooling: f64,
    /// Winding rise at which the model calls it too hot, K.
    pub limit_rise_k: f64,
}

impl ThermalParams {
    pub const GENERIC: Self = Self {
        winding_rise_rated_k: 80.0,
        winding_to_frame_k: 30.0,
        tau_winding_s: 40.0,
        tau_frame_s: 1500.0,
        standstill_cooling: 0.4,
        limit_rise_k: 100.0,
    };

    pub fn validate(&self) -> Result<(), InputError> {
        error::positive("winding_rise_rated_k", self.winding_rise_rated_k)?;
        error::positive("winding_to_frame_k", self.winding_to_frame_k)?;
        if self.winding_to_frame_k >= self.winding_rise_rated_k {
            return Err(InputError::Inconsistent {
                what: "winding-to-frame rise is not below the total rated rise",
            });
        }
        error::positive("tau_winding_s", self.tau_winding_s)?;
        error::positive("tau_frame_s", self.tau_frame_s)?;
        error::fraction("standstill_cooling", self.standstill_cooling)?;
        error::positive("limit_rise_k", self.limit_rise_k)?;
        Ok(())
    }
}

/// Two-node heat model of one motor: winding to frame to ambient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotorThermal {
    params: ThermalParams,
    g_winding_frame: f64,
    g_frame_ambient_rated: f64,
    c_winding: f64,
    c_frame: f64,
    winding_rise_k: f64,
    frame_rise_k: f64,
}

impl MotorThermal {
    /// `rated_losses_w` is the loss at the rated point, which sets the conductances so that the
    /// rated rise comes out as entered.
    pub fn new(params: ThermalParams, rated_losses_w: f64) -> Result<Self, InputError> {
        params.validate()?;
        error::positive("rated_losses_w", rated_losses_w)?;
        let g_wf = rated_losses_w / params.winding_to_frame_k;
        let g_fa = rated_losses_w / (params.winding_rise_rated_k - params.winding_to_frame_k);
        Ok(Self {
            params,
            g_winding_frame: g_wf,
            g_frame_ambient_rated: g_fa,
            c_winding: params.tau_winding_s * g_wf,
            c_frame: params.tau_frame_s * g_fa,
            winding_rise_k: 0.0,
            frame_rise_k: 0.0,
        })
    }

    pub fn params(&self) -> &ThermalParams {
        &self.params
    }

    pub fn winding_rise_k(&self) -> f64 {
        self.winding_rise_k
    }

    pub fn frame_rise_k(&self) -> f64 {
        self.frame_rise_k
    }

    /// Everything back to ambient.
    pub fn cool_down(&mut self) {
        self.winding_rise_k = 0.0;
        self.frame_rise_k = 0.0;
    }

    pub fn too_hot(&self) -> bool {
        self.winding_rise_k >= self.params.limit_rise_k
    }

    /// Cooling factor at a shaft speed over the rated speed (clamped to 0..1): the standstill
    /// share plus the rest in proportion to speed.
    pub fn cooling_factor(&self, speed_ratio: f64) -> f64 {
        let f0 = self.params.standstill_cooling;
        f0 + (1.0 - f0) * speed_ratio.abs().clamp(0.0, 1.0)
    }

    /// Steady-state `(winding, frame)` rise in K for constant losses and cooling factor.
    pub fn steady_rise_k(&self, losses_w: f64, cooling_factor: f64) -> (f64, f64) {
        let frame = losses_w / (self.g_frame_ambient_rated * cooling_factor);
        (frame + losses_w / self.g_winding_frame, frame)
    }

    /// Advance by `dt_s` with the winding losses `losses_w` and the speed over rated speed.
    pub fn step(&mut self, losses_w: f64, speed_ratio: f64, dt_s: f64) {
        let g_fa = self.g_frame_ambient_rated * self.cooling_factor(speed_ratio);
        // Sub-steps keep the explicit scheme stable for any caller step.
        let h_max = 0.1 * self.params.tau_winding_s.min(self.params.tau_frame_s);
        let n = (dt_s / h_max).ceil().max(1.0);
        let h = dt_s / n;
        for _ in 0..n as usize {
            let flow = self.g_winding_frame * (self.winding_rise_k - self.frame_rise_k);
            self.winding_rise_k += (losses_w - flow) / self.c_winding * h;
            self.frame_rise_k += (flow - g_fa * self.frame_rise_k) / self.c_frame * h;
        }
    }
}

/// A thermal overload relay: a first-order model of the bimetal strip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverloadRelay {
    setting_a: f64,
    tau_s: f64,
    state: f64,
    tripped: bool,
}

/// Ratio at which the relay state trips: `1.05^2`, the usual 105 % of the setting.
pub const TRIP_STATE: f64 = 1.05 * 1.05;

/// Multiple of the setting at which the trip class is stated.
pub const CLASS_MULTIPLE: f64 = 7.2;

impl OverloadRelay {
    /// `class_s` is the trip time in seconds at 7.2 times the setting from a cold start
    /// (class 10 is 10 s, class 20 is 20 s). The time constant follows from it.
    pub fn new(setting_a: f64, class_s: f64) -> Result<Self, InputError> {
        error::positive("overload setting_a", setting_a)?;
        error::positive("overload class_s", class_s)?;
        let tau = -class_s / (1.0 - TRIP_STATE / (CLASS_MULTIPLE * CLASS_MULTIPLE)).ln();
        Ok(Self {
            setting_a,
            tau_s: tau,
            state: 0.0,
            tripped: false,
        })
    }

    pub fn setting_a(&self) -> f64 {
        self.setting_a
    }

    pub fn tau_s(&self) -> f64 {
        self.tau_s
    }

    pub fn tripped(&self) -> bool {
        self.tripped
    }

    /// Thermal state over the trip state (1 = at the trip point).
    pub fn load(&self) -> f64 {
        self.state / TRIP_STATE
    }

    /// Time to trip from cold at a constant multiple of the setting, in s (closed form);
    /// `None` when it never trips (the multiple squared does not exceed the trip state).
    pub fn trip_time_s(&self, multiple: f64) -> Option<f64> {
        let ss = multiple * multiple;
        if ss <= TRIP_STATE {
            None
        } else {
            Some(-self.tau_s * (1.0 - TRIP_STATE / ss).ln())
        }
    }

    pub fn step(&mut self, current_a: f64, dt_s: f64) {
        let ratio = current_a / self.setting_a;
        // Exact solution of the first-order step over `dt_s` for a constant current.
        let target = ratio * ratio;
        self.state = target + (self.state - target) * (-dt_s / self.tau_s).exp();
        if self.state >= TRIP_STATE {
            self.tripped = true;
        }
    }

    /// Manual reset: clears the trip, and the strip keeps its heat.
    pub fn reset(&mut self) {
        self.tripped = false;
    }

    pub fn cool_down(&mut self) {
        self.state = 0.0;
        self.tripped = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nameplate() -> MotorNameplate {
        // 4 kW, 400 V, 50 Hz, 1440 rpm, 8.5 A, power factor 0.82: a typical 4-pole motor.
        MotorNameplate {
            voltage_v: 400.0,
            current_a: 8.5,
            frequency_hz: 50.0,
            rpm: 1440.0,
            power_kw: 4.0,
            power_factor: 0.82,
        }
    }

    fn electrics() -> WindingElectrics {
        WindingElectrics::from_nameplate(&nameplate(), 4).unwrap().0
    }

    #[test]
    fn three_phase_current_matches_the_nameplate_at_the_rated_point() {
        let e = electrics();
        let omega = 1440.0 * std::f64::consts::TAU / 60.0;
        let (i, p) = e
            .current_and_losses(Direction::Positive, omega, &Supply::HEALTHY)
            .unwrap();
        assert!((i - 8.5).abs() < 0.05 * 8.5, "{i}");
        assert!((p - e.rated_losses_w()).abs() < 1e-6 * p);
    }

    #[test]
    fn two_phases_at_standstill_draw_sqrt3_over_2_of_the_locked_current() {
        let e = electrics();
        let locked = e.locked_rotor_current_a(1.0).unwrap();
        assert!(locked > 4.0 * 8.5, "{locked}");
        let two = Supply::HEALTHY.with_phase_lost(crate::supply::Phase::L2);
        let (i2, _) = e
            .current_and_losses(Direction::Positive, 0.0, &two)
            .unwrap();
        assert!((i2 - TWO_PHASE_LOCKED_RATIO * locked).abs() < 1e-9 * locked);
    }

    #[test]
    fn two_phases_running_at_rated_speed_draw_more_than_three() {
        let e = electrics();
        let omega = 1440.0 * std::f64::consts::TAU / 60.0;
        let (i3, p3) = e
            .current_and_losses(Direction::Positive, omega, &Supply::HEALTHY)
            .unwrap();
        let two = Supply::HEALTHY.with_phase_lost(crate::supply::Phase::L1);
        let (i2, p2) = e
            .current_and_losses(Direction::Positive, omega, &two)
            .unwrap();
        assert!((i2 / i3 - 3f64.sqrt()).abs() < 1e-9, "{}", i2 / i3);
        // Two phases carrying sqrt(3) times the current: 2 * 3 / 3 = 2 times the copper loss.
        assert!((p2 / p3 - 2.0).abs() < 1e-9, "{}", p2 / p3);
    }

    #[test]
    fn low_voltage_lowers_the_locked_rotor_current_in_proportion() {
        let e = electrics();
        let full = e.locked_rotor_current_a(1.0).unwrap();
        let low = e.locked_rotor_current_a(0.8).unwrap();
        assert!((low / full - 0.8).abs() < 0.01, "{}", low / full);
    }

    #[test]
    fn steady_rise_is_the_rated_rise_at_rated_losses_and_full_cooling() {
        let e = electrics();
        let t = MotorThermal::new(ThermalParams::GENERIC, e.rated_losses_w()).unwrap();
        let (w, f) = t.steady_rise_k(e.rated_losses_w(), 1.0);
        assert!((w - 80.0).abs() < 1e-9);
        assert!((f - 50.0).abs() < 1e-9);
        // Standstill cooling of 40 %: the frame rise is 2.5 times higher.
        let (_, f0) = t.steady_rise_k(e.rated_losses_w(), 0.4);
        assert!((f0 - 125.0).abs() < 1e-9);
    }

    #[test]
    fn the_model_settles_at_the_steady_rise() {
        let e = electrics();
        let mut t = MotorThermal::new(ThermalParams::GENERIC, e.rated_losses_w()).unwrap();
        for _ in 0..(20_000.0 / 0.5) as usize {
            t.step(e.rated_losses_w(), 1.0, 0.5);
        }
        assert!(
            (t.winding_rise_k() - 80.0).abs() < 0.5,
            "{}",
            t.winding_rise_k()
        );
        assert!((t.frame_rise_k() - 50.0).abs() < 0.5);
        assert!(!t.too_hot());
        t.cool_down();
        assert_eq!(t.winding_rise_k(), 0.0);
    }

    #[test]
    fn a_locked_rotor_heats_the_winding_at_p_over_c_and_hits_the_limit_in_seconds() {
        let e = electrics();
        let p_locked = e
            .current_and_losses(Direction::Positive, 0.0, &Supply::HEALTHY)
            .unwrap()
            .1;
        let mut t = MotorThermal::new(ThermalParams::GENERIC, e.rated_losses_w()).unwrap();
        // First tick: the whole loss goes into the winding capacity.
        let c_w = ThermalParams::GENERIC.tau_winding_s * e.rated_losses_w() / 30.0;
        t.step(p_locked, 0.0, 0.01);
        assert!((t.winding_rise_k() - p_locked * 0.01 / c_w).abs() < 1e-6 * t.winding_rise_k());
        let mut s = 0.0_f64;
        while !t.too_hot() && s < 600.0 {
            t.step(p_locked, 0.0, 0.01);
            s += 0.01;
        }
        // Far shorter than the frame time constant; of the order of ten seconds, not minutes.
        assert!(s > 1.0 && s < 60.0, "{s}");
    }

    #[test]
    fn two_phase_standstill_heats_slower_than_three_phase_locked_but_still_to_the_limit() {
        let e = electrics();
        let hit = |supply: Supply| {
            let p = e
                .current_and_losses(Direction::Positive, 0.0, &supply)
                .unwrap()
                .1;
            let mut t = MotorThermal::new(ThermalParams::GENERIC, e.rated_losses_w()).unwrap();
            let mut s = 0.0_f64;
            while !t.too_hot() && s < 3600.0 {
                t.step(p, 0.0, 0.05);
                s += 0.05;
            }
            s
        };
        let three = hit(Supply::HEALTHY);
        let two = hit(Supply::HEALTHY.with_phase_lost(crate::supply::Phase::L3));
        // (sqrt(3)/2)^2 * 2/3 = 0.5 of the locked copper loss: about twice as slow.
        assert!(two > 1.5 * three && two < 4.0 * three, "{two} vs {three}");
        assert!(two < 3600.0);
    }

    #[test]
    fn relay_trips_at_the_class_time_at_7_2_times_the_setting() {
        let mut r = OverloadRelay::new(8.5, 10.0).unwrap();
        let t = r.trip_time_s(7.2).unwrap();
        assert!((t - 10.0).abs() < 1e-9);
        let mut s = 0.0_f64;
        while !r.tripped() {
            r.step(7.2 * 8.5, 0.001);
            s += 0.001;
            assert!(s < 20.0);
        }
        assert!((s - 10.0).abs() < 0.005, "{s}");
        assert!(r.load() >= 1.0);
        r.reset();
        assert!(!r.tripped());
    }

    #[test]
    fn relay_never_trips_at_the_setting_and_trips_slowly_above_it() {
        let r = OverloadRelay::new(8.5, 10.0).unwrap();
        assert!(r.trip_time_s(1.0).is_none());
        assert!(r.trip_time_s(1.05).is_none());
        let slow = r.trip_time_s(1.2).unwrap();
        let fast = r.trip_time_s(3.0).unwrap();
        assert!(slow > 60.0 && fast < slow / 10.0, "{slow} {fast}");
        let mut c = OverloadRelay::new(8.5, 10.0).unwrap();
        for _ in 0..100_000 {
            c.step(8.5, 0.1);
        }
        assert!(!c.tripped());
    }

    #[test]
    fn refuses_bad_numbers() {
        let mut p = ThermalParams::GENERIC;
        p.winding_to_frame_k = 90.0;
        assert!(MotorThermal::new(p, 100.0).is_err());
        assert!(MotorThermal::new(ThermalParams::GENERIC, 0.0).is_err());
        assert!(OverloadRelay::new(0.0, 10.0).is_err());
        assert!(OverloadRelay::new(8.5, 0.0).is_err());
    }
}
