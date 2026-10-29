//! Scripted winch scenarios (lane L2c): the closed-loop winch and the pair of synchronous
//! winches under a script, giving time series, events and design findings. Pure: the
//! Simulate > Winch scenarios window only draws the result.
//!
//! The hardware is a generic one (30 kW, 1480 rpm motor, 31.5:1 gear, 0.2 m drum radius, a
//! 400 N m holding brake, an encoder of 1024 pulses per revolution) so the scenarios show
//! behaviour, not a product. Every number is meant to be edited; the window says so.

use novo_drive::InputError;
use novo_drive::brake::{BrakeState, SpringBrake, Switching};
use novo_drive::closed_loop::{
    ClInputs, ClState, ClosedLoopConfig, ClosedLoopWinch, EncoderHealth, Finding, PreTorque,
    check_winch,
};
use novo_drive::drive::MotorNameplate;
use novo_drive::mechanics::Hoist;

use crate::sync_winch::{
    Beam, PairCommand, PairSnapshot, PartnerAction, Rope, Supervision, SyncMode, WinchPair,
};

const G: f64 = novo_drive::GRAVITY;
const DRUM_R: f64 = 0.2;
const RATIO: f64 = 31.5;
const STEP_S: f64 = 0.002;
/// One sample in the trace per this many steps.
const KEEP_EVERY: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioId {
    /// Lift, stop, lower, stop: the torque-first brake sequence on one winch.
    Cycle,
    /// The encoder fails while lifting.
    EncoderLoss,
    /// Safe torque off while lifting.
    SafeTorqueOff,
    /// Two winches, same drums, same references.
    PairMatched,
    /// Two winches, one drum a little larger than entered.
    PairDrumError,
    /// Two winches, one drive trips while lifting.
    PairDriveTrip,
}

impl ScenarioId {
    pub const ALL: [ScenarioId; 6] = [
        Self::Cycle,
        Self::EncoderLoss,
        Self::SafeTorqueOff,
        Self::PairMatched,
        Self::PairDrumError,
        Self::PairDriveTrip,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Cycle => "One winch: lift and lower cycle",
            Self::EncoderLoss => "One winch: encoder lost",
            Self::SafeTorqueOff => "One winch: safe torque off while lifting",
            Self::PairMatched => "Two winches: matched",
            Self::PairDrumError => "Two winches: a drum is a little larger",
            Self::PairDriveTrip => "Two winches: one drive trips",
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            Self::Cycle => {
                "The drive builds torque against the closed brake, proves it, opens the brake, runs, holds zero speed and closes the brake under torque. Change the pre-torque or the brake hold time to see the load sag or drop."
            }
            Self::EncoderLoss => {
                "The encoder signal or its direction channel fails during the lift. With monitoring on the drive trips on the mismatch with the motor model (a lost direction channel only shows when the shaft reverses); with it off the drive loses the speed it controls."
            }
            Self::SafeTorqueOff => {
                "Both STO channels drop while lifting: torque goes at once, the brake relay drops, and the load travels on until the brake has applied."
            }
            Self::PairMatched => {
                "Both winches lift one beam at the same hook speed. The rope forces stay at their static shares and the beam stays level."
            }
            Self::PairDrumError => {
                "Winch B's drum is larger than the value the drives were set up with. A free beam tilts, a guided load loads one rope; a position lock between the encoders cannot see it, a torque follower shares the load by torque."
            }
            Self::PairDriveTrip => {
                "Drive A trips during the lift. With a link the partner stops too; without one the partner keeps lifting against the held winch."
            }
        }
    }

    pub fn is_pair(self) -> bool {
        matches!(
            self,
            Self::PairMatched | Self::PairDrumError | Self::PairDriveTrip
        )
    }
}

/// What the user can change; each scenario reads the ones it uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    /// Single winch: the hook load in kg.
    pub load_kg: f64,
    pub pre_torque: PreTorque,
    /// How long the drive keeps torque after the brake relay opens, s.
    pub brake_hold_s: f64,
    pub braking_resistor: bool,
    pub encoder_monitor: bool,
    /// Encoder loss: the direction channel only (counts keep coming) instead of all signal.
    pub direction_channel_only: bool,
    /// Pair: beam mass in kg.
    pub beam_kg: f64,
    pub sync: SyncChoice,
    /// The beam is guided (tilt locked).
    pub guided: bool,
    /// Winch B's drum is this many percent larger than the drives were set up with.
    pub drum_error_pct: f64,
    /// A link stops the partner when one drive trips.
    pub partner_link: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncChoice {
    Independent,
    PositionLock,
    TorqueFollower,
}

impl SyncChoice {
    pub const ALL: [SyncChoice; 3] = [Self::Independent, Self::PositionLock, Self::TorqueFollower];

    pub fn label(self) -> &'static str {
        match self {
            Self::Independent => "Same reference, no correction",
            Self::PositionLock => "Position lock between encoders",
            Self::TorqueFollower => "Torque follower",
        }
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            load_kg: 2500.0,
            pre_torque: PreTorque::LoadCell,
            brake_hold_s: 0.5,
            braking_resistor: true,
            encoder_monitor: true,
            direction_channel_only: false,
            beam_kg: 5000.0,
            sync: SyncChoice::Independent,
            guided: true,
            drum_error_pct: 1.0,
            partner_link: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub name: &'static str,
    pub unit: &'static str,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub t_s: f64,
    pub text: String,
}

/// The result of one scenario run.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    pub dt_s: f64,
    pub channels: Vec<Channel>,
    /// Channel indices drawn on one chart, one list per chart.
    pub charts: Vec<Vec<usize>>,
    pub events: Vec<Event>,
    pub findings: Vec<Finding>,
    /// Numbers worth reading after the run, label and value.
    pub summary: Vec<(String, String)>,
}

impl Trace {
    pub fn channel(&self, name: &str) -> Option<&Channel> {
        self.channels.iter().find(|c| c.name == name)
    }

    pub fn duration_s(&self) -> f64 {
        self.channels
            .first()
            .map_or(0.0, |c| c.values.len() as f64 * self.dt_s)
    }
}

fn motor() -> MotorNameplate {
    MotorNameplate {
        voltage_v: 400.0,
        current_a: 56.0,
        frequency_hz: 50.0,
        rpm: 1480.0,
        power_kw: 30.0,
        power_factor: 0.85,
    }
}

fn drive_cfg(p: &Params, pre: PreTorque) -> ClosedLoopConfig {
    let mut c = ClosedLoopConfig::generic_hoist(motor());
    c.pre_torque = pre;
    c.brake_close_wait_s = p.brake_hold_s;
    c.encoder_monitor = p.encoder_monitor;
    if p.braking_resistor {
        c.bus.resistor_ohm = Some(40.0);
    }
    c
}

fn brake() -> Result<SpringBrake, InputError> {
    SpringBrake::new(400.0, 0.15, 0.2, 0.2, 0.0, Switching::DcSide)
}

fn mpr() -> f64 {
    DRUM_R / RATIO
}

fn push(ch: &mut [Channel], i: usize, v: f64) {
    ch[i].values.push(v);
}

fn channel(name: &'static str, unit: &'static str) -> Channel {
    Channel {
        name,
        unit,
        values: Vec::new(),
    }
}

pub fn run(id: ScenarioId, p: &Params) -> Result<Trace, InputError> {
    match id {
        ScenarioId::Cycle | ScenarioId::EncoderLoss | ScenarioId::SafeTorqueOff => single(id, p),
        _ => pair(id, p),
    }
}

fn state_name(s: ClState) -> &'static str {
    match s {
        ClState::Off => "stopped, brake closed",
        ClState::Magnetising => "building flux",
        ClState::TorqueProof => "building torque against the closed brake",
        ClState::Releasing => "brake relay on, holding zero speed",
        ClState::Running => "running",
        ClState::Stopping => "ramping to zero speed",
        ClState::BrakeClosing => "brake relay off, still holding torque",
        ClState::StoActive => "safe torque off active",
        ClState::Faulted => "tripped",
    }
}

fn single(id: ScenarioId, p: &Params) -> Result<Trace, InputError> {
    let hoist = Hoist::new(p.load_kg, DRUM_R, RATIO, 1, 0.9)?;
    let cfg = drive_cfg(p, p.pre_torque);
    let findings = check_winch(&cfg, &hoist, &brake()?);
    let mut w = ClosedLoopWinch::new(cfg, hoist, brake()?, 0.5)?;
    let mut ch = vec![
        channel("Hook height", "m"),
        channel("Hook speed", "m/s"),
        channel("Motor torque", "N m"),
        channel("Brake torque", "N m"),
        channel("Speed reference", "rpm"),
        channel("Measured speed", "rpm"),
        channel("Brake relay", "on"),
        channel("DC bus", "V"),
    ];
    let charts = vec![vec![0], vec![1, 4, 5], vec![2, 3], vec![7]];
    let mut events = Vec::new();
    let mut last_state = ClState::Off;
    let mut last_relay = false;
    let mut last_brake = BrakeState::Applied;
    let mut last_fault = None;
    let total = 16.0;
    let n = (total / STEP_S) as usize;
    let mut max_bus: f64 = 0.0;
    let mut peak_torque: f64 = 0.0;
    let mut sag: f64 = 0.0;
    let mut release_height = None;
    for k in 0..n {
        let t = k as f64 * STEP_S;
        let mut inp = ClInputs::default();
        // Script: lift 0 to 5 s, rest, lower 8 to 12 s.
        let (run, rpm) = if t < 5.0 {
            (true, 700.0)
        } else if (8.0..12.0).contains(&t) {
            (true, -700.0)
        } else {
            (false, 0.0)
        };
        inp.run = run;
        inp.speed_ref_rpm = rpm;
        match id {
            ScenarioId::EncoderLoss if t >= 3.0 => {
                w.drive.encoder.health = if p.direction_channel_only {
                    EncoderHealth::DirectionLost
                } else {
                    EncoderHealth::SignalLost
                };
            }
            ScenarioId::SafeTorqueOff if (3.0..6.0).contains(&t) => {
                inp.run = true;
                inp.speed_ref_rpm = 700.0;
                inp.sto = (false, false);
            }
            _ => {}
        }
        if id == ScenarioId::EncoderLoss && p.direction_channel_only && t < 8.0 {
            // Lift only: the lost direction channel reads right when raising.
            inp.run = t < 5.0;
        }
        let s = w.step(&inp, STEP_S);
        let d = s.drive;
        if d.state != last_state {
            events.push(Event {
                t_s: t,
                text: format!("Drive: {}", state_name(d.state)),
            });
            last_state = d.state;
        }
        if d.brake_release_cmd != last_relay {
            events.push(Event {
                t_s: t,
                text: format!(
                    "Brake relay {} (torque {:.0} N m)",
                    if d.brake_release_cmd { "on" } else { "off" },
                    d.torque_nm
                ),
            });
            last_relay = d.brake_release_cmd;
        }
        if s.brake_state != last_brake {
            events.push(Event {
                t_s: t,
                text: format!(
                    "Brake {}",
                    if s.brake_state == BrakeState::Released {
                        "is open"
                    } else {
                        "holds"
                    }
                ),
            });
            if s.brake_state == BrakeState::Released && release_height.is_none() {
                release_height = Some(s.hook_height_m);
            }
            last_brake = s.brake_state;
        }
        if d.fault != last_fault {
            if let Some(f) = d.fault {
                events.push(Event {
                    t_s: t,
                    text: format!("Fault {}: {}", f.code(), f.description()),
                });
            }
            last_fault = d.fault;
        }
        if let Some(h0) = release_height
            && t < 3.0
        {
            sag = sag.max(h0 - s.hook_height_m);
        }
        max_bus = max_bus.max(d.bus_v);
        peak_torque = peak_torque.max(d.torque_nm.abs());
        if k % KEEP_EVERY == 0 {
            push(&mut ch, 0, s.hook_height_m);
            push(&mut ch, 1, s.hook_speed_m_s);
            push(&mut ch, 2, d.torque_nm);
            push(&mut ch, 3, s.brake_torque_nm);
            push(&mut ch, 4, d.ref_rpm);
            push(&mut ch, 5, d.measured_rpm);
            push(&mut ch, 6, f64::from(u8::from(d.brake_release_cmd)));
            push(&mut ch, 7, d.bus_v);
        }
    }
    let end_h = ch[0].values.last().copied().unwrap_or(0.0);
    let mut summary = vec![
        (
            "Weight torque on the motor shaft".to_owned(),
            format!("{:.0} N m (exact: m g r / i)", p.load_kg * G * mpr()),
        ),
        (
            "Sag after brake release (first 3 s)".to_owned(),
            format!("{:.1} mm", sag * 1000.0),
        ),
        (
            "Peak motor torque".to_owned(),
            format!("{peak_torque:.0} N m (model)"),
        ),
        (
            "Highest DC bus voltage".to_owned(),
            format!("{max_bus:.0} V (model)"),
        ),
        ("Hook height at the end".to_owned(), format!("{end_h:.3} m")),
    ];
    if id == ScenarioId::SafeTorqueOff {
        let at_sto = ch[0].values[(3.0 / (STEP_S * KEEP_EVERY as f64)) as usize];
        let hi = ch[0].values.iter().copied().fold(f64::MIN, f64::max);
        summary.push((
            "Extra rise after STO".to_owned(),
            format!(
                "{:.0} mm (model: brake delay entered as 0.2 s)",
                (hi - at_sto) * 1000.0
            ),
        ));
    }
    Ok(Trace {
        dt_s: STEP_S * KEEP_EVERY as f64,
        channels: ch,
        charts,
        events,
        findings,
        summary,
    })
}

fn winch(p: &Params, weight_nm: f64) -> Result<ClosedLoopWinch, InputError> {
    let hoist = Hoist::new(1.0, DRUM_R, RATIO, 1, 0.9)?;
    let cfg = drive_cfg(p, PreTorque::Fixed(weight_nm));
    ClosedLoopWinch::with_shaft_inertia(cfg, hoist, brake()?, 0.5, 1.0)
}

fn pair(id: ScenarioId, p: &Params) -> Result<Trace, InputError> {
    let weight_nm = p.beam_kg * G / 2.0 * mpr();
    let beam = Beam::new(p.beam_kg, 4.0, 0.0, p.guided)?;
    let rope = Rope::with_stiffness(2.0e6, 0.3)?;
    let mode = match p.sync {
        SyncChoice::Independent => SyncMode::Independent,
        SyncChoice::PositionLock => SyncMode::PositionLock {
            gain_per_s: 20.0,
            limit_m_s: 0.05,
        },
        SyncChoice::TorqueFollower => SyncMode::TorqueFollower {
            share: 1.0,
            margin: 0.15,
        },
    };
    let mut sup = Supervision::new(0.6 * p.beam_kg * G);
    sup.on_partner_fault = if p.partner_link {
        PartnerAction::StopBoth
    } else {
        PartnerAction::KeepRunning
    };
    let scale = match id {
        ScenarioId::PairDrumError => 1.0 + p.drum_error_pct / 100.0,
        _ => 1.0,
    };
    let mut pr = WinchPair::new(
        winch(p, weight_nm)?,
        winch(p, weight_nm)?,
        [rope; 2],
        beam,
        mode,
        sup,
    )
    .with_drum_error([1.0, scale]);
    let mut ch = vec![
        channel("Beam height", "m"),
        channel("Tilt", "deg"),
        channel("Rope force A", "kN"),
        channel("Rope force B", "kN"),
        channel("Rope rating", "kN"),
        channel("Position error A-B (encoders)", "mm"),
        channel("Speed A", "m/s"),
        channel("Speed B", "m/s"),
        channel("Torque A", "N m"),
        channel("Torque B", "N m"),
    ];
    let charts = vec![
        vec![0],
        vec![2, 3, 4],
        vec![1],
        vec![5],
        vec![6, 7],
        vec![8, 9],
    ];
    let mut events = Vec::new();
    let total = 12.0;
    let n = (total / STEP_S) as usize;
    let mut last: Option<PairSnapshot> = None;
    let mut last_states = [ClState::Off; 2];
    let mut last_fault = [None, None];
    let mut worst_diff: f64 = 0.0;
    let mut max_tilt: f64 = 0.0;
    for k in 0..n {
        let t = k as f64 * STEP_S;
        let mut cmd = if t < 8.0 {
            PairCommand::run(0.3)
        } else {
            PairCommand::default()
        };
        if id == ScenarioId::PairDriveTrip && t >= 4.0 {
            cmd.external_fault[0] = true;
        }
        let s = pr.step(&cmd, STEP_S);
        for i in 0..2 {
            let name = if i == 0 { "A" } else { "B" };
            let st = s.winches[i].drive.state;
            if st != last_states[i] {
                events.push(Event {
                    t_s: t,
                    text: format!("Winch {name}: {}", state_name(st)),
                });
                last_states[i] = st;
            }
            let f = s.winches[i].drive.fault;
            if f != last_fault[i] {
                if let Some(f) = f {
                    events.push(Event {
                        t_s: t,
                        text: format!("Winch {name} fault {}: {}", f.code(), f.description()),
                    });
                }
                last_fault[i] = f;
            }
        }
        if let Some(prev) = last {
            if let (Some(fault), None) = (s.sync_fault, prev.sync_fault) {
                events.push(Event {
                    t_s: t,
                    text: format!("Pair stops itself: {fault:?}"),
                });
            }
            for i in 0..2 {
                let name = if i == 0 { "A" } else { "B" };
                if s.overload[i] && !prev.overload[i] {
                    events.push(Event {
                        t_s: t,
                        text: format!(
                            "Rope {name} above its rating ({:.1} kN)",
                            s.force_n[i] / 1000.0
                        ),
                    });
                }
                if s.slack[i] && !prev.slack[i] {
                    events.push(Event {
                        t_s: t,
                        text: format!("Rope {name} goes slack"),
                    });
                }
            }
            if s.beyond_model && !prev.beyond_model {
                events.push(Event {
                    t_s: t,
                    text: "Tilt beyond what the vertical-rope model covers".into(),
                });
            }
        }
        worst_diff = worst_diff.max((s.force_n[0] - s.force_n[1]).abs());
        max_tilt = max_tilt.max(s.tilt_deg.abs());
        last = Some(s);
        if k % KEEP_EVERY == 0 {
            push(&mut ch, 0, s.height_m);
            push(&mut ch, 1, s.tilt_deg);
            push(&mut ch, 2, s.force_n[0] / 1000.0);
            push(&mut ch, 3, s.force_n[1] / 1000.0);
            push(&mut ch, 4, pr.supervision.rope_swl_n / 1000.0);
            push(&mut ch, 5, s.position_error_m * 1000.0);
            push(&mut ch, 6, s.winches[0].hook_speed_m_s);
            push(&mut ch, 7, s.winches[1].hook_speed_m_s);
            push(&mut ch, 8, s.winches[0].drive.torque_nm);
            push(&mut ch, 9, s.winches[1].drive.torque_nm);
        }
    }
    let kn = |v: f64| format!("{:.1} kN", v / 1000.0);
    let summary = vec![
        (
            "Static rope force each (exact: M g / 2)".to_owned(),
            kn(p.beam_kg * G / 2.0),
        ),
        (
            "Peak rope force A / B".to_owned(),
            format!("{} / {}", kn(pr.peak_force_n[0]), kn(pr.peak_force_n[1])),
        ),
        ("Largest force difference".to_owned(), kn(worst_diff)),
        ("Largest tilt".to_owned(), format!("{max_tilt:.2} deg")),
        ("Rope rating used".to_owned(), kn(pr.supervision.rope_swl_n)),
    ];
    let mut findings = Vec::new();
    if p.guided && p.sync == SyncChoice::PositionLock && id == ScenarioId::PairDrumError {
        findings.push(Finding {
            severity: novo_drive::closed_loop::Severity::Warning,
            text: "A position lock compares encoder counts: it cannot see a drum or gear that differs from the entered value. Share the load by torque or measure the rope forces.".into(),
        });
    }
    if p.guided && p.sync == SyncChoice::Independent && id == ScenarioId::PairDrumError {
        findings.push(Finding {
            severity: novo_drive::closed_loop::Severity::Problem,
            text: "Both drives get the same reference with nothing to correct a drift: on a guided load the difference is carried by rope stretch (k times the difference, split between the ropes).".into(),
        });
    }
    if id == ScenarioId::PairDriveTrip && !p.partner_link {
        findings.push(Finding {
            severity: novo_drive::closed_loop::Severity::Problem,
            text: "No link between the drives: when one trips the other keeps lifting against the held winch. Wire the fault relay of each drive into the other's stop.".into(),
        });
    }
    Ok(Trace {
        dt_s: STEP_S * KEEP_EVERY as f64,
        channels: ch,
        charts,
        events,
        findings,
        summary,
    })
}
