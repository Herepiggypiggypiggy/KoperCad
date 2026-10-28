//! DC bus: capacitor, rectifier, braking chopper and resistor, optional regeneration to mains.
//!
//! The bus keeps its state as stored energy `E = C V^2 / 2`, and every joule that enters or
//! leaves is booked in [`BusEnergy`], so the balance
//! `mains_in - rectifier_loss - inverter_loss - resistor - regen_to_mains - to_load == dE_bus`
//! holds by construction in every step (a property test checks it over random runs).
//! The rectifier is a stiff source with a small series resistance (model); the inverter has a
//! constant efficiency in both directions plus a standby loss (model).

/// Bus configuration (all entered by the user or taken from the drive profile).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BusConfig {
    pub capacitance_f: f64,
    /// Mains line-to-line voltage (rms).
    pub mains_v: f64,
    /// Rectifier source resistance in ohm (model of diode drop and line impedance).
    pub source_ohm: f64,
    pub inverter_efficiency: f64,
    /// Constant loss of the control electronics and fans, W.
    pub standby_w: f64,
    /// Chopper switches on above this bus voltage and off below `chopper_off_v`.
    pub chopper_on_v: f64,
    pub chopper_off_v: f64,
    /// Braking resistor in ohm; `None` = no resistor fitted.
    pub resistor_ohm: Option<f64>,
    /// Continuous rating of the resistor in W (for the thermal model).
    pub resistor_continuous_w: f64,
    /// Heat capacity above the continuous rating in J before the resistor model trips (0 = no model).
    pub resistor_thermal_j: f64,
    /// Regenerative front end returns energy above the rectified mains level.
    pub regen_to_mains: bool,
    pub over_voltage_v: f64,
    pub under_voltage_v: f64,
}

impl BusConfig {
    /// Generic 400 V class defaults (stated generic values, not a product).
    pub fn generic_400v() -> Self {
        Self {
            capacitance_f: 1.0e-3,
            mains_v: 400.0,
            source_ohm: 0.5,
            inverter_efficiency: 0.97,
            standby_w: 30.0,
            chopper_on_v: 780.0,
            chopper_off_v: 750.0,
            resistor_ohm: None,
            resistor_continuous_w: 0.0,
            resistor_thermal_j: 0.0,
            regen_to_mains: false,
            over_voltage_v: 820.0,
            under_voltage_v: 380.0,
        }
    }

    /// Rectified no-load bus voltage `sqrt(2) * V_LL`.
    pub fn rectified_v(&self) -> f64 {
        self.mains_v * std::f64::consts::SQRT_2
    }
}

/// Cumulative energy booking in J.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BusEnergy {
    pub mains_in: f64,
    pub rectifier_loss: f64,
    pub inverter_loss: f64,
    pub resistor: f64,
    pub regen_to_mains: f64,
    /// Energy delivered to the motor side (negative while the motor regenerates).
    pub to_load: f64,
}

/// Result of one bus step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BusStep {
    pub volts: f64,
    pub chopper_on: bool,
    pub resistor_w: f64,
    /// Power drawn from the mains (negative = returned).
    pub mains_w: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DcBus {
    pub cfg: BusConfig,
    energy_j: f64,
    pub chopper_on: bool,
    pub book: BusEnergy,
    /// Resistor heat above its continuous rating, J (thermal model).
    pub resistor_heat_j: f64,
}

impl DcBus {
    /// Bus charged to the rectified mains voltage.
    pub fn new(cfg: BusConfig) -> Self {
        let v = cfg.rectified_v();
        Self {
            cfg,
            energy_j: 0.5 * cfg.capacitance_f * v * v,
            chopper_on: false,
            book: BusEnergy::default(),
            resistor_heat_j: 0.0,
        }
    }

    /// Present bus voltage.
    pub fn volts(&self) -> f64 {
        (2.0 * self.energy_j / self.cfg.capacitance_f)
            .max(0.0)
            .sqrt()
    }

    /// Stored energy in J.
    pub fn stored_j(&self) -> f64 {
        self.energy_j
    }

    fn energy_at(&self, v: f64) -> f64 {
        0.5 * self.cfg.capacitance_f * v * v
    }

    /// Advance one step. `load_w` is the electrical power at the motor terminals (positive
    /// motoring, negative regenerating); `mains_ok` false opens the rectifier.
    pub fn step(&mut self, load_w: f64, mains_ok: bool, dt_s: f64) -> BusStep {
        if (dt_s.is_nan() || dt_s <= 0.0) || !dt_s.is_finite() {
            return BusStep {
                volts: self.volts(),
                chopper_on: self.chopper_on,
                resistor_w: 0.0,
                mains_w: 0.0,
            };
        }
        let c = self.cfg;
        let load_w = if load_w.is_finite() { load_w } else { 0.0 };
        let eta = c.inverter_efficiency.clamp(0.01, 1.0);
        // Inverter: draw from the bus is larger than the motor power when motoring, smaller in
        // magnitude when regenerating; standby loss always.
        let (draw_w, inv_loss_w) = if load_w >= 0.0 {
            (load_w / eta, load_w / eta - load_w)
        } else {
            (load_w * eta, -load_w * (1.0 - eta))
        };
        let draw_w = draw_w + c.standby_w;
        let inv_loss_w = inv_loss_w + c.standby_w;
        let mut e = self.energy_j - draw_w * dt_s;
        self.book.to_load += load_w * dt_s;
        self.book.inverter_loss += inv_loss_w * dt_s;
        // Chopper with hysteresis.
        let v_now = (2.0 * e.max(0.0) / c.capacitance_f).sqrt();
        if c.resistor_ohm.is_some() {
            if v_now >= c.chopper_on_v {
                self.chopper_on = true;
            } else if v_now <= c.chopper_off_v {
                self.chopper_on = false;
            }
        }
        let mut resistor_j = 0.0;
        if let (true, Some(r)) = (self.chopper_on, c.resistor_ohm) {
            let want = v_now * v_now / r * dt_s;
            let room = (e - self.energy_at(c.chopper_off_v)).max(0.0);
            resistor_j = want.min(room);
            e -= resistor_j;
        }
        // Regeneration to mains: everything above the rectified level goes back.
        let mut regen_j = 0.0;
        if c.regen_to_mains {
            regen_j = (e - self.energy_at(c.rectified_v())).max(0.0);
            e -= regen_j;
        }
        // Rectifier conducts below the rectified level.
        let mut mains_j = 0.0;
        let mut rect_loss_j = 0.0;
        if mains_ok && c.source_ohm > 0.0 {
            let v = (2.0 * e.max(0.0) / c.capacitance_f).sqrt().max(1.0);
            let vr = c.rectified_v();
            if v < vr {
                let i = (vr - v) / c.source_ohm;
                let gap = (self.energy_at(vr) - e).max(0.0);
                let deliver = (i * v * dt_s).min(gap);
                let i_eff = deliver / (v * dt_s);
                rect_loss_j = i_eff * i_eff * c.source_ohm * dt_s;
                mains_j = deliver + rect_loss_j;
                e += deliver;
            }
        }
        // A load drawing more than is stored cannot take the bus below zero.
        if e < 0.0 {
            // Book the shortfall as not delivered so the balance still closes.
            let short = -e;
            self.book.to_load -= short;
            self.book.inverter_loss += 0.0;
            e = 0.0;
        }
        self.energy_j = e;
        self.book.mains_in += mains_j;
        self.book.rectifier_loss += rect_loss_j;
        self.book.resistor += resistor_j;
        self.book.regen_to_mains += regen_j;
        let resistor_w = resistor_j / dt_s;
        if c.resistor_thermal_j > 0.0 {
            self.resistor_heat_j =
                (self.resistor_heat_j + (resistor_w - c.resistor_continuous_w) * dt_s).max(0.0);
        }
        BusStep {
            volts: self.volts(),
            chopper_on: self.chopper_on,
            resistor_w,
            mains_w: (mains_j - regen_j) / dt_s,
        }
    }

    /// True once the resistor thermal model has used up its capacity.
    pub fn resistor_overloaded(&self) -> bool {
        self.cfg.resistor_thermal_j > 0.0 && self.resistor_heat_j >= self.cfg.resistor_thermal_j
    }

    /// Energy balance residual in J: `inflow - outflow - dE_bus` since the start, given the
    /// initial stored energy. Zero up to rounding.
    pub fn balance_residual_j(&self, initial_stored_j: f64) -> f64 {
        let b = &self.book;
        let inflow = b.mains_in;
        let outflow =
            b.rectifier_loss + b.inverter_loss + b.resistor + b.regen_to_mains + b.to_load;
        inflow - outflow - (self.energy_j - initial_stored_j)
    }
}
