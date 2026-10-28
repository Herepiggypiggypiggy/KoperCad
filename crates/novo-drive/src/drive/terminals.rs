//! Control terminals and macro configurations (generic names, no vendor tables).
//!
//! A *macro* is a named bundle of terminal functions. The profile (lane L3) will store macros
//! as data; the built-in ones here are generic and neutral.

/// What a digital input does when it is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalFunction {
    Unused,
    /// Run forward (2-wire: held = run, released = stop).
    Forward,
    /// Run reverse.
    Reverse,
    /// Run command without direction; the direction is taken from [`TerminalFunction::DirectionReverse`].
    Run,
    /// With [`TerminalFunction::Run`]: reverse direction when active.
    DirectionReverse,
    /// Binary bit 0 of the preset selection.
    PresetBit0,
    /// Binary bit 1 of the preset selection.
    PresetBit1,
    /// Jog frequency while active.
    Jog,
    /// Stop on the fast-stop ramp (usually wired normally closed: use `invert`).
    FastStop,
    /// Drive enable (a hardware enable such as a safe torque off; usually normally closed).
    Enable,
    /// Rising edge resets latched faults.
    FaultReset,
    /// External fault (normally closed chain: use `invert`).
    ExternalFault,
    /// Motor-potentiometer faster.
    PotUp,
    /// Motor-potentiometer slower.
    PotDown,
}

/// One digital input: a function and whether the wiring logic is inverted
/// (`invert` = the function is active when the terminal is **de-energised**, for NC contacts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DigitalInput {
    pub function: TerminalFunction,
    pub invert: bool,
}

impl DigitalInput {
    pub const fn new(function: TerminalFunction) -> Self {
        Self {
            function,
            invert: false,
        }
    }
    pub const fn nc(function: TerminalFunction) -> Self {
        Self {
            function,
            invert: true,
        }
    }
}

/// Where the main frequency reference comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReferenceSource {
    /// Fixed value from the panel / parameter (Hz).
    Panel(f64),
    /// Analog voltage input, `min_v..max_v` mapped to `min_hz..max_hz`.
    AnalogVoltage {
        min_v: f64,
        max_v: f64,
        min_hz: f64,
        max_hz: f64,
    },
    /// Analog current input in mA (typically 4..20) mapped to `min_hz..max_hz`; below
    /// `loss_ma` the signal counts as lost.
    AnalogCurrent {
        min_ma: f64,
        max_ma: f64,
        min_hz: f64,
        max_hz: f64,
        loss_ma: Option<f64>,
    },
    /// Frequency word from a PLC or fieldbus (taken from the inputs).
    Word,
    /// Motor potentiometer (up/down inputs change a stored value).
    MotorPot,
}

/// Named bundle of terminal functions for the digital inputs and the logic outputs.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalMacro {
    pub name: String,
    pub inputs: Vec<DigitalInput>,
}

impl TerminalMacro {
    /// 2-wire control with four presets. DI1 forward, DI2 reverse, DI3 and DI4 select the preset
    /// in binary (00 = main reference, 01 = preset 1, 10 = preset 2, 11 = preset 3).
    pub fn two_wire_presets() -> Self {
        use TerminalFunction as F;
        Self {
            name: "2-wire with presets".into(),
            inputs: vec![
                DigitalInput::new(F::Forward),
                DigitalInput::new(F::Reverse),
                DigitalInput::new(F::PresetBit0),
                DigitalInput::new(F::PresetBit1),
            ],
        }
    }

    /// Crane travel, two speeds: DI1 forward, DI2 reverse, DI3 = high speed (step 2).
    /// Step 1 (only the direction input) runs the main reference or preset 1 as configured by
    /// the caller; DI3 selects preset 2. This is the radio step 1 / step 2 wiring.
    pub fn two_wire_two_speed() -> Self {
        use TerminalFunction as F;
        Self {
            name: "2-wire, two speeds".into(),
            inputs: vec![
                DigitalInput::new(F::Forward),
                DigitalInput::new(F::Reverse),
                DigitalInput::new(F::PresetBit1),
                DigitalInput::new(F::Enable),
            ],
        }
    }

    /// Run plus direction: DI1 run, DI2 reverse when active, DI3 jog.
    pub fn run_and_direction() -> Self {
        use TerminalFunction as F;
        Self {
            name: "Run and direction".into(),
            inputs: vec![
                DigitalInput::new(F::Run),
                DigitalInput::new(F::DirectionReverse),
                DigitalInput::new(F::Jog),
            ],
        }
    }

    /// Motor potentiometer: DI1 forward, DI2 faster, DI3 slower.
    pub fn motor_pot() -> Self {
        use TerminalFunction as F;
        Self {
            name: "Motor potentiometer".into(),
            inputs: vec![
                DigitalInput::new(F::Forward),
                DigitalInput::new(F::PotUp),
                DigitalInput::new(F::PotDown),
            ],
        }
    }
}

/// Decoded commands from the digital inputs.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Commands {
    pub forward: bool,
    pub reverse: bool,
    pub run: bool,
    pub direction_reverse: bool,
    pub preset: usize,
    pub jog: bool,
    pub fast_stop: bool,
    /// False when an enable input is wired and not active.
    pub enable: bool,
    pub fault_reset: bool,
    pub external_fault: bool,
    pub pot_up: bool,
    pub pot_down: bool,
}

impl Commands {
    /// Decode energised-terminal states with the macro's functions. Missing inputs count as off.
    pub fn decode(mac: &TerminalMacro, di: &[bool]) -> Self {
        use TerminalFunction as F;
        let mut c = Commands {
            enable: true,
            ..Default::default()
        };
        let mut enable_wired = false;
        let mut enable_active = false;
        for (i, input) in mac.inputs.iter().enumerate() {
            let level = di.get(i).copied().unwrap_or(false);
            let active = level != input.invert;
            match input.function {
                F::Unused => {}
                F::Forward => c.forward |= active,
                F::Reverse => c.reverse |= active,
                F::Run => c.run |= active,
                F::DirectionReverse => c.direction_reverse |= active,
                F::PresetBit0 => c.preset |= usize::from(active),
                F::PresetBit1 => c.preset |= usize::from(active) << 1,
                F::Jog => c.jog |= active,
                F::FastStop => c.fast_stop |= active,
                F::Enable => {
                    enable_wired = true;
                    enable_active |= active;
                }
                F::FaultReset => c.fault_reset |= active,
                F::ExternalFault => c.external_fault |= active,
                F::PotUp => c.pot_up |= active,
                F::PotDown => c.pot_down |= active,
            }
        }
        if enable_wired {
            c.enable = enable_active;
        }
        c
    }
}
