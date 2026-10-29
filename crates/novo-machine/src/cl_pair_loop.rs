//! The fixed-step loop over two closed-loop winches on one beam, driven by the drawn circuit
//! (lane L2c, second half).
//!
//! The same tick as [`crate::cl_winch_loop`], with the beam mechanics of [`crate::sync_winch`]
//! in place of two independent hooks:
//! 1. limit switches and each drive's brake relay contact are written from the previous tick,
//! 2. the circuit settles by one step,
//! 3. the common command (up, down, speed step 2) is read from the devices of winch A; every
//!    other input is read per winch: the two STO channels, the encoder signal, mains, the brake
//!    feed, the brake feedback and the reset,
//! 4. the pair is stepped: both drives, both brakes, both drums, the two ropes and the beam.
//!
//! So an STO channel opened on winch B, an encoder signal lost on B or a brake contactor of B
//! that does not pull in reach only that winch, and the pair does what the chosen
//! synchronisation and the partner action make of it. Winch B's own up, down and step 2 devices
//! are not read (the pair is commanded as one); `check_bindings` says so when they are drawn.
//!
//! Exact by definition: the order of the tick, the sign convention, the statics of the beam on
//! two ropes. Model: everything stated in [`crate::sync_winch`] and in
//! `novo_drive::closed_loop`. Not reported for a pair (the single winch reports them): start
//! rollback, stop travel and brake slip; read them off the snapshot.

use novo_drive::InputError;
use novo_drive::closed_loop::{ClFault, EncoderHealth};

use crate::cl_winch_loop::{ClWinchBinding, binding_findings, energised, energised_or_true};
use crate::cosim::StepConfig;
use crate::hoist_loop::BindingFinding;
use crate::port::CircuitPort;
use crate::sync_winch::{PairCommand, PairSnapshot, SyncFault, WinchLine, WinchPair};

/// The name of a winch in the texts.
pub fn winch_name(i: usize) -> &'static str {
    if i == 0 { "A" } else { "B" }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClPairFinding {
    pub time_s: f64,
    pub kind: ClPairFindingKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClPairFindingKind {
    Trip {
        winch: usize,
        fault: ClFault,
    },
    Reset {
        winch: usize,
    },
    StoRequested {
        winch: usize,
    },
    StoReleased {
        winch: usize,
    },
    EncoderSignalLost {
        winch: usize,
    },
    EncoderSignalBack {
        winch: usize,
    },
    /// The pair stopped itself.
    Sync(SyncFault),
    SyncCleared,
    /// A rope carries more than the rated load.
    Overload {
        winch: usize,
        force_n: f64,
    },
    /// A rope went slack.
    Slack {
        winch: usize,
    },
    /// The tilt left the range the vertical-rope model covers.
    BeyondModel {
        tilt_deg: f64,
    },
}

impl ClPairFindingKind {
    /// One line for the window.
    pub fn text(&self) -> String {
        let n = winch_name;
        match self {
            Self::Trip { winch, fault } => format!(
                "Winch {} tripped ({}): {}.",
                n(*winch),
                fault.code(),
                fault.description()
            ),
            Self::Reset { winch } => format!("The trip of winch {} was reset.", n(*winch)),
            Self::StoRequested { winch } => format!(
                "Safe torque off on winch {}: both channels low, torque removed and the brake \
                 closed.",
                n(*winch)
            ),
            Self::StoReleased { winch } => {
                format!(
                    "Winch {}: both STO channels are energised again.",
                    n(*winch)
                )
            }
            Self::EncoderSignalLost { winch } => format!(
                "Winch {}: the encoder signal is gone, no speed or position feedback.",
                n(*winch)
            ),
            Self::EncoderSignalBack { winch } => {
                format!("Winch {}: the encoder signal is back.", n(*winch))
            }
            Self::Sync(SyncFault::PositionError) => {
                "The pair stopped: the encoder positions differ by more than the limit.".to_owned()
            }
            Self::Sync(SyncFault::PartnerFault) => {
                "The pair stopped: one drive faulted and the link stops the other.".to_owned()
            }
            Self::SyncCleared => "The pair is released again.".to_owned(),
            Self::Overload { winch, force_n } => format!(
                "The rope of winch {} carries {:.1} kN, more than its rating.",
                n(*winch),
                force_n / 1000.0
            ),
            Self::Slack { winch } => format!("The rope of winch {} went slack.", n(*winch)),
            Self::BeyondModel { tilt_deg } => format!(
                "The beam tilts {tilt_deg:.1} degrees, beyond what the vertical-rope model covers."
            ),
        }
    }
}

/// Findings kept per pair; the rest is counted.
pub const MAX_FINDINGS: usize = 200;

/// Two closed-loop winches on one beam and their bindings.
pub struct ClPairModule {
    /// The tag of winch A (the pair is named after it).
    pub id: String,
    pub pair: WinchPair,
    /// Winch A's `up`, `down` and `fast` command the pair; B's are not read.
    pub bindings: [ClWinchBinding; 2],
    /// Hook speed of step 1 and step 2, m/s.
    pub speeds_m_s: [f64; 2],
    /// The tags of the two motors (A, B).
    pub names: [String; 2],
    last_reset: [bool; 2],
    relay: [bool; 2],
    snapshot: Option<PairSnapshot>,
    findings: Vec<ClPairFinding>,
    findings_dropped: usize,
    last_fault: [Option<ClFault>; 2],
    last_sto: [bool; 2],
    last_encoder_ok: [bool; 2],
    last_sync: Option<SyncFault>,
    overloaded: [bool; 2],
    slack: [bool; 2],
    beyond: bool,
}

impl ClPairModule {
    pub fn new(
        names: [String; 2],
        pair: WinchPair,
        speeds_m_s: [f64; 2],
        bindings: [ClWinchBinding; 2],
    ) -> Result<Self, InputError> {
        if !speeds_m_s.iter().all(|v| v.is_finite() && *v > 0.0) {
            return Err(InputError::NotPositive {
                field: "hook speed",
                value: speeds_m_s[0].min(speeds_m_s[1]),
            });
        }
        Ok(Self {
            id: names[0].clone(),
            pair,
            bindings,
            speeds_m_s,
            names,
            last_reset: [false; 2],
            relay: [false; 2],
            snapshot: None,
            findings: Vec::new(),
            findings_dropped: 0,
            last_fault: [None; 2],
            last_sto: [false; 2],
            last_encoder_ok: [true; 2],
            last_sync: None,
            overloaded: [false; 2],
            slack: [false; 2],
            beyond: false,
        })
    }

    pub fn snapshot(&self) -> Option<&PairSnapshot> {
        self.snapshot.as_ref()
    }

    /// Height of the beam centre, m.
    pub fn height_m(&self) -> f64 {
        self.snapshot.as_ref().map_or(0.0, |s| s.height_m)
    }

    /// What the run found so far, oldest first (at most [`MAX_FINDINGS`]).
    pub fn findings(&self) -> &[ClPairFinding] {
        &self.findings
    }

    pub fn findings_dropped(&self) -> usize {
        self.findings_dropped
    }

    fn note(&mut self, time_s: f64, kind: ClPairFindingKind) {
        if self.findings.len() >= MAX_FINDINGS {
            self.findings_dropped += 1;
            return;
        }
        self.findings.push(ClPairFinding { time_s, kind });
    }
}

/// The loop over any number of pairs.
pub struct ClPairLoop {
    pub config: StepConfig,
    pub modules: Vec<ClPairModule>,
    time_s: f64,
}

impl ClPairLoop {
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

    pub fn module(&self, id: &str) -> Option<&ClPairModule> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// Binding problems to show before the run.
    pub fn check_bindings(&self, circuit: &impl CircuitPort) -> Vec<BindingFinding> {
        let mut out = Vec::new();
        for m in &self.modules {
            for i in 0..2 {
                let who = format!("{} (winch {})", m.names[i], winch_name(i));
                out.extend(binding_findings(&who, &m.bindings[i], i == 0, circuit));
                let b = &m.bindings[i];
                if i == 1 && (b.up.is_some() || b.down.is_some() || b.fast.is_some()) {
                    out.push(BindingFinding {
                        module: who,
                        message: "up, down and step 2 are read from winch A only: the pair is \
                                  commanded as one, so these devices are not used"
                            .into(),
                    });
                }
            }
        }
        out
    }

    /// One tick.
    pub fn step(&mut self, circuit: &mut impl CircuitPort) {
        let dt = self.config.dt_s;
        for m in &self.modules {
            let heights = m.snapshot.as_ref().map_or([0.0; 2], |s| s.true_position_m);
            for (i, height) in heights.into_iter().enumerate() {
                for limit in &m.bindings[i].limits {
                    circuit.set_contact(&limit.binding.device_tag, limit.closed_at(height));
                }
                if let Some(tag) = &m.bindings[i].brake_relay {
                    circuit.set_contact(tag, m.relay[i]);
                }
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

fn step_module(m: &mut ClPairModule, circuit: &impl CircuitPort, dt: f64, time_s: f64) {
    let up = energised(circuit, &m.bindings[0].up);
    let down = energised(circuit, &m.bindings[0].down);
    let fast = energised(circuit, &m.bindings[0].fast);
    let mut sto = [(true, true); 2];
    let mut line = [WinchLine::default(); 2];
    let mut reset_edge = false;
    for i in 0..2 {
        let b = &m.bindings[i];
        sto[i] = (
            energised_or_true(circuit, &b.sto[0]),
            energised_or_true(circuit, &b.sto[1]),
        );
        let encoder_ok = energised_or_true(circuit, &b.encoder);
        let health = if encoder_ok {
            EncoderHealth::Healthy
        } else {
            EncoderHealth::SignalLost
        };
        let drive = if i == 0 {
            &mut m.pair.a.drive
        } else {
            &mut m.pair.b.drive
        };
        drive.encoder.health = health;
        line[i] = WinchLine {
            brake_coil: b.brake_feed.as_deref().map(|t| circuit.is_energised(t)),
            brake_feedback: b.brake_feedback.as_deref().map(|t| circuit.is_energised(t)),
            mains_ok: energised_or_true(circuit, &b.mains),
        };
        let reset = energised(circuit, &b.reset);
        reset_edge |= reset && !m.last_reset[i];
        m.last_reset[i] = reset;
        if encoder_ok != m.last_encoder_ok[i] {
            let kind = if encoder_ok {
                ClPairFindingKind::EncoderSignalBack { winch: i }
            } else {
                ClPairFindingKind::EncoderSignalLost { winch: i }
            };
            m.note(time_s, kind);
            m.last_encoder_ok[i] = encoder_ok;
        }
    }
    let run = up != down;
    let speed = if fast {
        m.speeds_m_s[1]
    } else {
        m.speeds_m_s[0]
    };
    let cmd = PairCommand {
        run,
        hook_speed_m_s: if down { -speed } else { speed },
        sto,
        external_fault: [false; 2],
        fault_reset: reset_edge,
        line,
    };
    let snap = m.pair.step(&cmd, dt);
    for i in 0..2 {
        let out = snap.winches[i].drive;
        m.relay[i] = out.brake_release_cmd;
        if out.fault != m.last_fault[i] {
            match (m.last_fault[i], out.fault) {
                (_, Some(fault)) => m.note(time_s, ClPairFindingKind::Trip { winch: i, fault }),
                (Some(_), None) => m.note(time_s, ClPairFindingKind::Reset { winch: i }),
                (None, None) => {}
            }
            m.last_fault[i] = out.fault;
        }
        if out.sto_requested != m.last_sto[i] {
            let kind = if out.sto_requested {
                ClPairFindingKind::StoRequested { winch: i }
            } else {
                ClPairFindingKind::StoReleased { winch: i }
            };
            m.note(time_s, kind);
            m.last_sto[i] = out.sto_requested;
        }
        if snap.overload[i] != m.overloaded[i] {
            m.overloaded[i] = snap.overload[i];
            if snap.overload[i] {
                m.note(
                    time_s,
                    ClPairFindingKind::Overload {
                        winch: i,
                        force_n: snap.force_n[i],
                    },
                );
            }
        }
        if snap.slack[i] != m.slack[i] {
            m.slack[i] = snap.slack[i];
            if snap.slack[i] {
                m.note(time_s, ClPairFindingKind::Slack { winch: i });
            }
        }
    }
    if snap.sync_fault != m.last_sync {
        match snap.sync_fault {
            Some(f) => m.note(time_s, ClPairFindingKind::Sync(f)),
            None => m.note(time_s, ClPairFindingKind::SyncCleared),
        }
        m.last_sync = snap.sync_fault;
    }
    if snap.beyond_model != m.beyond {
        m.beyond = snap.beyond_model;
        if snap.beyond_model {
            m.note(
                time_s,
                ClPairFindingKind::BeyondModel {
                    tilt_deg: snap.tilt_deg,
                },
            );
        }
    }
    m.snapshot = Some(snap);
}
