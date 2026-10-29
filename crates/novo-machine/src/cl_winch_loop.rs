//! The fixed-step loop over closed-loop (encoder) winches driven by the drawn circuit (lane L2c).
//!
//! One tick, in this order, always the same, so a run is deterministic:
//! 1. limit switches and the drive's brake relay contact are written from the previous tick,
//! 2. the circuit settles by one step,
//! 3. each module reads the operator inputs (up, down, speed step 2), the two STO channels, the
//!    encoder signal, its mains, the brake feed and the brake feedback from the circuit,
//! 4. the closed-loop drive (`novo_drive::closed_loop`) turns them and the shaft as its encoder
//!    sees it into a torque and a brake relay, the brake follows its coil (fed by the circuit
//!    through the relay, the brake contactor and the rectifier) and the shaft advances.
//!
//! What the circuit can do to the winch, all through ordinary drawn parts, so the defects of
//! the fault injection lane reach it without a machine specific hook: a broken wire or a
//! contact stuck open on one STO channel (the channels disagree, then the drive latches the
//! discrepancy), a lost encoder signal device (the drive loses speed feedback and trips), a
//! brake contactor that does not pull in (the brake never opens) or is welded (the brake opens
//! before the drive has torque), a lost main contactor.
//!
//! Exact by definition: the order of the tick, the sign convention (positive shaft speed raises
//! the hook), the arithmetic of the hanging load, the STO gating. Model: everything inside
//! `novo_drive::closed_loop` (see its `FIDELITY` list).

use std::f64::consts::TAU;

use novo_drive::InputError;
use novo_drive::closed_loop::{
    ClFault, ClInputs, ClState, ClosedLoopWinch, EncoderHealth, WinchSnapshot,
};

use crate::cosim::StepConfig;
use crate::hoist_loop::{BindingFinding, LimitSwitch};
use crate::port::CircuitPort;

/// Hook movement below this (m) is not reported as a rollback, a stop travel or a slip.
const REPORT_BELOW_M: f64 = 0.005;

/// A hook that moved this far against the command at the start is reported at once.
const RUNAWAY_M: f64 = 0.05;

/// Where a closed-loop winch is attached to the schematic.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClWinchBinding {
    /// Device tag whose energised state is the "up" command.
    pub up: Option<String>,
    /// Device tag whose energised state is the "down" command.
    pub down: Option<String>,
    /// Device tag whose energised state selects speed step 2 (otherwise step 1).
    pub fast: Option<String>,
    /// Device tags of the two STO channels, energised = torque allowed. A channel that is not
    /// bound is always energised.
    pub sto: [Option<String>; 2],
    /// Device tag whose energised state means the encoder signal is present. `None`: always.
    pub encoder: Option<String>,
    /// Device tag of the main contactor coil that feeds the drive. `None`: mains is always on.
    pub mains: Option<String>,
    /// Device tag of the drive's brake relay contact. The module writes its state; the drawing
    /// connects it to the brake contactor.
    pub brake_relay: Option<String>,
    /// Device tag whose energised state feeds the brake coil (brake contactor, rectifier).
    /// `None`: the brake follows the relay directly (an ideal wiring).
    pub brake_feed: Option<String>,
    /// Device tag whose energised state confirms an open brake to the drive. `None`: no check.
    pub brake_feedback: Option<String>,
    /// Device tag whose energised state resets a tripped drive (rising edge).
    pub reset: Option<String>,
    /// Upper limit protects the top, lower limit the bottom, by hook height.
    pub limits: Vec<LimitSwitch>,
}

/// Something the run found, with the time it happened.
#[derive(Debug, Clone, PartialEq)]
pub struct ClWinchFinding {
    pub time_s: f64,
    pub kind: ClFindingKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClFindingKind {
    /// The hook went against the command (`metres`, positive) after the brake relay closed.
    StartRollback {
        metres: f64,
    },
    /// Net hook movement from the brake command to standstill (positive = up).
    StopTravel {
        metres: f64,
    },
    /// The hook moved with the drive off and the brake commanded closed.
    BrakeSlip {
        metres: f64,
    },
    /// The drive tripped.
    Trip(ClFault),
    /// A trip was cleared by a reset.
    Reset,
    /// Both STO channels went low: torque removed, brake closed.
    StoRequested,
    /// The STO channels came back.
    StoReleased,
    /// The encoder signal device lost its supply or its wire.
    EncoderSignalLost,
    EncoderSignalBack,
}

impl ClFindingKind {
    /// One line for the window.
    pub fn text(&self) -> String {
        match self {
            Self::StartRollback { metres } => format!(
                "The hook went {:.0} mm against the command after the brake relay closed.",
                metres * 1000.0
            ),
            Self::StopTravel { metres } => format!(
                "The hook moved {:.0} mm {} between the brake command and standstill.",
                metres.abs() * 1000.0,
                if *metres >= 0.0 { "up" } else { "down" }
            ),
            Self::BrakeSlip { metres } => format!(
                "The brake does not hold the load: the hook moved {:.0} mm with the drive off.",
                metres * 1000.0
            ),
            Self::Trip(f) => format!("The drive tripped ({}): {}.", f.code(), f.description()),
            Self::Reset => "The trip was reset.".to_owned(),
            Self::StoRequested => {
                "Safe torque off: both channels low, torque removed and the brake closed."
                    .to_owned()
            }
            Self::StoReleased => "Both STO channels are energised again.".to_owned(),
            Self::EncoderSignalLost => {
                "The encoder signal is gone: no speed or position feedback.".to_owned()
            }
            Self::EncoderSignalBack => "The encoder signal is back.".to_owned(),
        }
    }
}

/// A closed-loop winch: drive, brake, hoist load and bindings.
#[derive(Debug)]
pub struct ClWinchModule {
    pub id: String,
    pub winch: ClosedLoopWinch,
    pub binding: ClWinchBinding,
    /// Hook speed of step 1 and step 2, m/s.
    pub speeds_m_s: [f64; 2],
    last_reset: bool,
    relay: bool,
    last_fault: Option<ClFault>,
    last_sto_requested: bool,
    last_encoder_ok: bool,
    snapshot: Option<WinchSnapshot>,
    findings: Vec<ClWinchFinding>,
    // Trackers for the findings.
    start_mark: Option<(f64, bool)>,
    start_worst_m: f64,
    start_reported: bool,
    stop_mark: Option<f64>,
    idle_baseline: Option<f64>,
    slip_reported: bool,
}

impl ClWinchModule {
    pub fn new(
        id: impl Into<String>,
        winch: ClosedLoopWinch,
        speeds_m_s: [f64; 2],
        binding: ClWinchBinding,
    ) -> Result<Self, InputError> {
        if !speeds_m_s.iter().all(|v| v.is_finite() && *v > 0.0) {
            return Err(InputError::NotPositive {
                field: "hook speed",
                value: speeds_m_s[0].min(speeds_m_s[1]),
            });
        }
        Ok(Self {
            id: id.into(),
            winch,
            binding,
            speeds_m_s,
            last_reset: false,
            relay: false,
            last_fault: None,
            last_sto_requested: false,
            last_encoder_ok: true,
            snapshot: None,
            findings: Vec::new(),
            start_mark: None,
            start_worst_m: 0.0,
            start_reported: false,
            stop_mark: None,
            idle_baseline: Some(0.0),
            slip_reported: false,
        })
    }

    /// Hook height in metres, 0 at the start (positive is up).
    pub fn height_m(&self) -> f64 {
        self.snapshot.as_ref().map_or(0.0, |s| s.hook_height_m)
    }

    pub fn snapshot(&self) -> Option<&WinchSnapshot> {
        self.snapshot.as_ref()
    }

    /// What the run found so far, oldest first.
    pub fn findings(&self) -> &[ClWinchFinding] {
        &self.findings
    }

    fn note(&mut self, time_s: f64, kind: ClFindingKind) {
        self.findings.push(ClWinchFinding { time_s, kind });
    }
}

/// The loop over any number of closed-loop winches.
pub struct ClWinchLoop {
    pub config: StepConfig,
    pub modules: Vec<ClWinchModule>,
    time_s: f64,
}

impl ClWinchLoop {
    pub fn new(config: StepConfig) -> Result<Self, InputError> {
        config.validate()?;
        Ok(Self {
            config,
            modules: Vec::new(),
            time_s: 0.0,
        })
    }

    pub fn time_s(&self) -> f64 {
        self.time_s
    }

    pub fn module(&self, id: &str) -> Option<&ClWinchModule> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// Binding problems to show before the run.
    pub fn check_bindings(&self, circuit: &impl CircuitPort) -> Vec<BindingFinding> {
        self.modules
            .iter()
            .flat_map(|m| binding_findings(&m.id, &m.binding, true, circuit))
            .collect()
    }

    /// One tick.
    pub fn step(&mut self, circuit: &mut impl CircuitPort) {
        let dt = self.config.dt_s;
        for m in &self.modules {
            let height = m.height_m();
            for limit in &m.binding.limits {
                circuit.set_contact(&limit.binding.device_tag, limit.closed_at(height));
            }
            if let Some(tag) = &m.binding.brake_relay {
                circuit.set_contact(tag, m.relay);
            }
        }
        circuit.settle(dt);
        let time_s = self.time_s;
        for m in &mut self.modules {
            step_module(m, circuit, dt, time_s);
        }
        self.time_s += dt;
    }

    /// Runs `ticks` ticks.
    pub fn run(&mut self, circuit: &mut impl CircuitPort, ticks: usize) {
        for _ in 0..ticks {
            self.step(circuit);
        }
    }
}

/// Binding problems of one winch. `commanded`: this winch's own up and down inputs are what
/// commands it (the second winch of a pair is commanded through the first one's).
pub(crate) fn binding_findings(
    module: &str,
    b: &ClWinchBinding,
    commanded: bool,
    circuit: &impl CircuitPort,
) -> Vec<BindingFinding> {
    let mut out = Vec::new();
    let finding = |message: String| BindingFinding {
        module: module.to_owned(),
        message,
    };
    let named = [
        ("up input", &b.up),
        ("down input", &b.down),
        ("speed step 2 input", &b.fast),
        ("STO channel 1", &b.sto[0]),
        ("STO channel 2", &b.sto[1]),
        ("encoder signal", &b.encoder),
        ("main contactor", &b.mains),
        ("brake relay contact", &b.brake_relay),
        ("brake feed", &b.brake_feed),
        ("brake feedback", &b.brake_feedback),
        ("reset input", &b.reset),
    ];
    for (what, tag) in named {
        if let Some(tag) = tag
            && !circuit.knows_device(tag)
        {
            out.push(finding(format!("{what} {tag} is not in the circuit")));
        }
    }
    for limit in &b.limits {
        if !circuit.knows_device(&limit.binding.device_tag) {
            out.push(finding(format!(
                "limit switch {} is not in the circuit",
                limit.binding.device_tag
            )));
        }
    }
    if commanded && b.up.is_none() && b.down.is_none() {
        out.push(finding(
            "no up or down input is bound: the winch cannot be commanded".into(),
        ));
    }
    if b.brake_relay.is_none() && b.brake_feed.is_some() {
        out.push(finding(
            "the brake is fed from the circuit but the drive's brake relay contact is \
             not bound, so the brake is never commanded open"
                .into(),
        ));
    }
    if b.brake_feed.is_none() {
        out.push(finding(
            "no brake feed device is bound: the brake follows the drive's relay \
             directly (ideal wiring, the brake contactor and rectifier are not checked)"
                .into(),
        ));
    }
    if b.sto[0].is_none() || b.sto[1].is_none() {
        out.push(finding(
            "an STO channel is not bound: it counts as always energised, so STO can \
             never be requested through it"
                .into(),
        ));
    }
    if b.encoder.is_none() {
        out.push(finding(
            "no encoder signal device is bound: the encoder is always present in the \
             drawing (a loss can still be injected in the engine)"
                .into(),
        ));
    }
    out
}

pub(crate) fn energised(circuit: &impl CircuitPort, tag: &Option<String>) -> bool {
    tag.as_deref().is_some_and(|t| circuit.is_energised(t))
}

pub(crate) fn energised_or_true(circuit: &impl CircuitPort, tag: &Option<String>) -> bool {
    tag.as_deref().is_none_or(|t| circuit.is_energised(t))
}

fn step_module(m: &mut ClWinchModule, circuit: &impl CircuitPort, dt: f64, time_s: f64) {
    let up = energised(circuit, &m.binding.up);
    let down = energised(circuit, &m.binding.down);
    let fast = energised(circuit, &m.binding.fast);
    let sto = (
        energised_or_true(circuit, &m.binding.sto[0]),
        energised_or_true(circuit, &m.binding.sto[1]),
    );
    let encoder_ok = energised_or_true(circuit, &m.binding.encoder);
    let mains_ok = energised_or_true(circuit, &m.binding.mains);
    let reset = energised(circuit, &m.binding.reset);
    let reset_edge = reset && !m.last_reset;
    m.last_reset = reset;

    // A lost encoder signal is a device of the drawing: it keeps the counts from moving. Losing
    // it and getting it back are separate findings.
    m.winch.drive.encoder.health = if encoder_ok {
        EncoderHealth::Healthy
    } else {
        EncoderHealth::SignalLost
    };
    if encoder_ok != m.last_encoder_ok {
        let kind = if encoder_ok {
            ClFindingKind::EncoderSignalBack
        } else {
            ClFindingKind::EncoderSignalLost
        };
        m.note(time_s, kind);
        m.last_encoder_ok = encoder_ok;
    }

    let run = up != down;
    let metres_per_rad = m.winch.hoist().metres_per_rad();
    let speed = if fast {
        m.speeds_m_s[1]
    } else {
        m.speeds_m_s[0]
    };
    let rpm = speed / metres_per_rad * 60.0 / TAU * if down { -1.0 } else { 1.0 };
    let inp = ClInputs {
        run,
        speed_ref_rpm: rpm,
        sto,
        brake_feedback: m
            .binding
            .brake_feedback
            .as_deref()
            .map(|tag| circuit.is_energised(tag)),
        mains_ok,
        fault_reset: reset_edge,
        brake_coil: m
            .binding
            .brake_feed
            .as_deref()
            .map(|tag| circuit.is_energised(tag)),
        ..ClInputs::default()
    };
    let snap = m.winch.step(&inp, dt);
    let out = snap.drive;
    m.relay = out.brake_release_cmd;
    if out.fault != m.last_fault {
        match (m.last_fault, out.fault) {
            (_, Some(f)) => m.note(time_s, ClFindingKind::Trip(f)),
            (Some(_), None) => m.note(time_s, ClFindingKind::Reset),
            (None, None) => {}
        }
        m.last_fault = out.fault;
    }
    if out.sto_requested != m.last_sto_requested {
        let kind = if out.sto_requested {
            ClFindingKind::StoRequested
        } else {
            ClFindingKind::StoReleased
        };
        m.note(time_s, kind);
        m.last_sto_requested = out.sto_requested;
    }
    track_findings(m, &snap, time_s, run, up);
    m.snapshot = Some(snap);
}

fn track_findings(m: &mut ClWinchModule, snap: &WinchSnapshot, time_s: f64, run: bool, up: bool) {
    let relay = snap.drive.brake_release_cmd;
    let height = snap.hook_height_m;
    let prev_relay = m
        .snapshot
        .as_ref()
        .is_some_and(|s| s.drive.brake_release_cmd);
    // Start: from the moment the relay closes until the hook moves with the command.
    if relay && !prev_relay {
        m.start_mark = Some((height, up));
        m.start_worst_m = 0.0;
        m.start_reported = false;
        m.idle_baseline = None;
        m.slip_reported = false;
        m.stop_mark = None;
    }
    if let Some((h0, was_up)) = m.start_mark {
        let against = if was_up { h0 - height } else { height - h0 };
        m.start_worst_m = m.start_worst_m.max(against);
        if m.start_worst_m > RUNAWAY_M && !m.start_reported {
            let metres = m.start_worst_m;
            m.start_reported = true;
            m.note(time_s, ClFindingKind::StartRollback { metres });
        }
        if -against > 0.02 || !relay {
            if m.start_worst_m > REPORT_BELOW_M && !m.start_reported {
                let metres = m.start_worst_m;
                m.note(time_s, ClFindingKind::StartRollback { metres });
            }
            m.start_mark = None;
        }
    }
    // Stop: from the relay opening until the shaft is at rest and the drive is off.
    if !relay && prev_relay {
        m.stop_mark = Some(height);
    }
    if let Some(h1) = m.stop_mark
        && snap.shaft_rpm == 0.0
        && snap.drive.state == ClState::Off
        && !run
    {
        let metres = height - h1;
        if metres.abs() > REPORT_BELOW_M {
            m.note(time_s, ClFindingKind::StopTravel { metres });
        }
        m.stop_mark = None;
        m.idle_baseline = Some(height);
    }
    // Slip with the drive off and the brake commanded closed.
    let idle = matches!(snap.drive.state, ClState::Off | ClState::StoActive);
    if idle
        && let Some(base) = m.idle_baseline
        && !m.slip_reported
        && (height - base).abs() > 0.02
        && m.stop_mark.is_none()
    {
        m.slip_reported = true;
        m.note(
            time_s,
            ClFindingKind::BrakeSlip {
                metres: (height - base).abs(),
            },
        );
    }
    if !idle {
        m.slip_reported = false;
    }
}
