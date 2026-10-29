//! Loop checks for the open-loop drive hoist: brake logic, flux build-up, a hanging load and a
//! scripted circuit. Expected values are worked out on paper in the comments.

use std::cell::RefCell;
use std::rc::Rc;

use novo_drive::brake::{SpringBrake, Switching};
use novo_drive::drive::terminals::DigitalInput;
use novo_drive::drive::{
    Drive, DriveConfig, DriveState, Fault, MotorNameplate, ReferenceSource, TerminalFunction,
    TerminalMacro,
};
use novo_drive::hoist_logic::{
    BrakeLogic, HoistLogic, HoistLogicConfig, LoadSense, LogicFault, LogicPhase, ReversalJump,
};
use novo_drive::induction::{FitOptions, InductionMotor};
use novo_drive::mechanics::Hoist;
use novo_drive::rope::RopeConfig;
use novo_machine::bindings::SensorBinding;
use novo_machine::cosim::StepConfig;
use novo_machine::drive_hoist_loop::{
    DriveHoistBinding, DriveHoistLoop, DriveHoistModule, DriveHoistParts, FindingKind,
};
use novo_machine::hoist_loop::{LimitKind, LimitSwitch};
use novo_machine::port::FakeCircuit;

const DT: f64 = 0.005;

/// What the operator and the supply do; the scripted circuit reads it every tick.
#[derive(Default, Clone, Copy)]
struct Cmd {
    up: bool,
    down: bool,
    step2: bool,
    step3: bool,
    mains: bool,
    /// The brake contactor is wired (a pulled-in relay contact closes it).
    contactor_wired: bool,
    /// An upper end limit stops the up command when this contact opens.
    limit_cuts_up: bool,
}

type Shared = Rc<RefCell<Cmd>>;

fn plate() -> MotorNameplate {
    MotorNameplate {
        voltage_v: 400.0,
        current_a: 11.2,
        frequency_hz: 50.0,
        rpm: 1440.0,
        power_kw: 5.5,
        power_factor: 0.82,
    }
}

/// 2000 kg on a 0.15 m drum, ratio 63, two falls: 0.15 / (2 x 63) = 0.00119 m per rad/s of
/// motor speed. Weight torque on the shaft: 2000 x 9.80665 x 0.00119 = 23.35 N m, which is 64 %
/// of the 36.5 N m rated torque of the motor.
fn hoist(mass: f64) -> Hoist {
    Hoist::new(mass, 0.15, 63.0, 2, 0.85).unwrap()
}

fn logic_cfg() -> HoistLogicConfig {
    let mut b = BrakeLogic::generic(11.2);
    b.release_time_s = 0.25;
    // The brake opens at about the rated current: flux is built and the torque carries the load.
    b.release_current_up_a = 10.0;
    b.release_current_down_a = 10.0;
    HoistLogicConfig {
        jump: None,
        brake: b,
        steps_hz: vec![10.0, 25.0, 50.0],
        sense: None,
        rated_torque_nm: 36.5,
    }
}

fn drive_cfg(resistor: Option<f64>) -> DriveConfig {
    let np = plate();
    let mut cfg = DriveConfig::generic_travel(np, 14.0);
    cfg.reference = ReferenceSource::Word;
    cfg.macro_ = TerminalMacro {
        name: "hoist".into(),
        inputs: vec![
            DigitalInput::new(TerminalFunction::Forward),
            DigitalInput::new(TerminalFunction::Reverse),
            DigitalInput::new(TerminalFunction::FaultReset),
        ],
    };
    cfg.accel_time_s = 2.0;
    cfg.decel_time_s = 2.0;
    cfg.max_hz = 90.0;
    // Boost covers the stator drop at low frequency: 20 V at 0 Hz fading out by 15 Hz.
    cfg.vf.boost_v = 20.0;
    cfg.vf.boost_end_hz = 15.0;
    cfg.bus.resistor_ohm = resistor;
    cfg
}

fn brake(switching: Switching) -> SpringBrake {
    // 80 N m, opens 0.08 s after the coil is fed, builds torque 0.15 s (AC side) or 0.03 s
    // (DC side) after the coil is off, over 0.05 s.
    SpringBrake::new(80.0, 0.08, 0.15, 0.03, 0.05, switching).unwrap()
}

struct Rig {
    mass: f64,
    logic: HoistLogicConfig,
    resistor: Option<f64>,
    brake_t: SpringBrake,
    feedback: bool,
    limit_m: Option<f64>,
    rope: Option<RopeConfig>,
}

impl Rig {
    fn new() -> Self {
        Self {
            mass: 2000.0,
            logic: logic_cfg(),
            resistor: Some(60.0),
            brake_t: brake(Switching::AcSide),
            feedback: false,
            limit_m: None,
            rope: None,
        }
    }

    fn module(&self) -> DriveHoistModule {
        let np = plate();
        let (motor, _) = InductionMotor::from_nameplate(&np, None, &FitOptions::typical()).unwrap();
        let mut limits = Vec::new();
        if let Some(m) = self.limit_m {
            limits.push(LimitSwitch {
                binding: SensorBinding {
                    name: "upper".into(),
                    device_tag: "=S-TOP".into(),
                    actuate_at_mm: m * 1000.0,
                },
                kind: LimitKind::Upper,
            });
        }
        let module = DriveHoistModule::new(
            "=M-HOIST",
            DriveHoistParts {
                drive: Drive::new(drive_cfg(self.resistor)),
                logic: HoistLogic::new(self.logic.clone()).unwrap(),
                motor,
                hoist: hoist(self.mass),
                brake: self.brake_t,
                rotor_inertia_kg_m2: 0.03,
            },
            DriveHoistBinding {
                up: Some("=K-UP".into()),
                down: Some("=K-DOWN".into()),
                speed_inputs: vec![Some("=K-S2".into()), Some("=K-S3".into())],
                mains: Some("=K-MAIN".into()),
                brake_relay: Some("=A-R2".into()),
                brake_feed: Some("=K-BRAKE".into()),
                brake_feedback: self.feedback.then(|| "=K-BRAKE".to_owned()),
                reset: Some("=S-RESET".into()),
                limits,
            },
        )
        .unwrap();
        match self.rope {
            Some(cfg) => module.with_rope(cfg).unwrap(),
            None => module,
        }
    }

    fn start(&self) -> (DriveHoistLoop, FakeCircuit, Shared) {
        let cmd: Shared = Rc::new(RefCell::new(Cmd {
            mains: true,
            contactor_wired: true,
            ..Cmd::default()
        }));
        let shared = Rc::clone(&cmd);
        let circuit = FakeCircuit::new().with_rule(move |s| {
            let c = *shared.borrow();
            let top_ok = s.contacts.get("=S-TOP").copied().unwrap_or(true);
            let up = c.up && (top_ok || !c.limit_cuts_up);
            s.energised.clear();
            for (on, tag) in [
                (up, "=K-UP"),
                (c.down, "=K-DOWN"),
                (c.step2, "=K-S2"),
                (c.step3, "=K-S3"),
                (c.mains, "=K-MAIN"),
            ] {
                if on {
                    s.energised.insert(tag.to_owned());
                }
            }
            let relay = s.contacts.get("=A-R2").copied().unwrap_or(false);
            if relay && c.contactor_wired && c.mains {
                s.energised.insert("=K-BRAKE".to_owned());
            }
            s.energised.insert("=S-RESET-KNOWN".to_owned());
        });
        let mut circuit = circuit;
        circuit.known.insert("=S-RESET".into());
        let mut lp = DriveHoistLoop::new(StepConfig { dt_s: DT }).unwrap();
        lp.modules.push(self.module());
        (lp, circuit, cmd)
    }
}

fn run_for(lp: &mut DriveHoistLoop, c: &mut FakeCircuit, seconds: f64) {
    lp.run(c, (seconds / DT).round() as usize);
}

fn m(lp: &DriveHoistLoop) -> &DriveHoistModule {
    &lp.modules[0]
}

fn kinds(lp: &DriveHoistLoop) -> Vec<FindingKind> {
    m(lp).findings().iter().map(|f| f.kind.clone()).collect()
}

fn rolled_back(lp: &DriveHoistLoop) -> Option<f64> {
    kinds(lp).into_iter().find_map(|k| match k {
        FindingKind::StartRollback { metres } => Some(metres),
        _ => None,
    })
}

#[test]
fn a_normal_start_opens_the_brake_only_with_torque_and_the_hook_does_not_drop() {
    let (mut lp, mut c, cmd) = Rig::new().start();
    cmd.borrow_mut().up = true;
    run_for(&mut lp, &mut c, 6.0);
    let s = m(&lp).snapshot().unwrap();
    assert_eq!(s.logic.phase, LogicPhase::Running);
    assert!(m(&lp).height_m() > 0.12, "{}", m(&lp).height_m());
    // Step 1 is 10 Hz: sync speed 300 rpm = 31.4 rad/s, hook 0.0375 m/s; the slip of a 64 %
    // loaded motor takes a few percent off.
    assert!((0.030..0.0376).contains(&s.speed_m_s), "{}", s.speed_m_s);
    assert!(
        rolled_back(&lp).is_none(),
        "no rollback expected: {:?}",
        kinds(&lp)
    );
}

#[test]
fn the_brake_relay_is_a_contact_in_the_circuit() {
    let (mut lp, mut c, cmd) = Rig::new().start();
    cmd.borrow_mut().up = true;
    let mut saw_closed = false;
    for _ in 0..1200 {
        lp.step(&mut c);
        if c.contacts.get("=A-R2") == Some(&true) {
            saw_closed = true;
            // Closed contact only after the motor had its current (release logic).
            assert!(m(&lp).snapshot().unwrap().motor.stator_current_a > 4.0);
            break;
        }
    }
    assert!(saw_closed);
}

#[test]
fn a_normal_stop_closes_the_brake_before_the_output_goes_off_and_the_hook_stays() {
    let (mut lp, mut c, cmd) = Rig::new().start();
    cmd.borrow_mut().up = true;
    run_for(&mut lp, &mut c, 6.0);
    let h = m(&lp).height_m();
    cmd.borrow_mut().up = false;
    run_for(&mut lp, &mut c, 6.0);
    let s = m(&lp).snapshot().unwrap();
    assert_eq!(s.logic.phase, LogicPhase::Idle);
    assert_eq!(s.speed_m_s, 0.0);
    assert_eq!(s.drive.state, DriveState::Stopped);
    let stop = kinds(&lp).into_iter().find_map(|k| match k {
        FindingKind::StopTravel { metres } => Some(metres),
        _ => None,
    });
    // The hook coasts a few centimetres in its direction while the brake closes, and does
    // not sink.
    let travelled = m(&lp).height_m() - h;
    assert!((-0.002..0.05).contains(&travelled), "{travelled}");
    assert!(stop.is_none_or(|v| v > -0.01), "{stop:?}");
    // And holds for a long time afterwards.
    let rest = m(&lp).height_m();
    run_for(&mut lp, &mut c, 10.0);
    assert_eq!(m(&lp).height_m(), rest);
}

#[test]
fn opening_the_brake_without_a_current_check_drops_the_hook() {
    // No release current check, 1 Hz of magnetising and a brake that opens 20 ms after its
    // command: it opens while the flux is still building, the torque (proportional to flux
    // squared) is below the weight, and the hook sinks. At 1 to 3 Hz an open-loop drive cannot
    // lift this load once it is moving down, so the run reports a runaway drop.
    let mut rig = Rig::new();
    rig.brake_t = SpringBrake::new(80.0, 0.02, 0.15, 0.03, 0.05, Switching::AcSide).unwrap();
    rig.logic.brake.release_current_up_a = 0.0;
    rig.logic.brake.release_hz = 1.0;
    let (mut lp, mut c, cmd) = rig.start();
    cmd.borrow_mut().up = true;
    run_for(&mut lp, &mut c, 8.0);
    let drop = rolled_back(&lp).expect("a rollback is expected");
    assert!(drop > 0.005, "{drop}");
    // The proper setting (same load, same brake): the brake waits for the current.
    let (mut lp2, mut c2, cmd2) = Rig::new().start();
    cmd2.borrow_mut().up = true;
    run_for(&mut lp2, &mut c2, 8.0);
    assert!(rolled_back(&lp2).is_none(), "{:?}", kinds(&lp2));
    assert!(m(&lp).height_m() < 0.0 && m(&lp2).height_m() > 0.0);
}

#[test]
fn a_brake_that_cannot_hold_the_load_slips_with_the_drive_off() {
    // 5000 kg gives 58.4 N m at the shaft; the brake holds 40 N m.
    let mut rig = Rig::new();
    rig.mass = 5000.0;
    rig.brake_t = SpringBrake::new(40.0, 0.08, 0.15, 0.03, 0.05, Switching::AcSide).unwrap();
    let (mut lp, mut c, _cmd) = rig.start();
    run_for(&mut lp, &mut c, 3.0);
    assert!(m(&lp).height_m() < -0.05, "{}", m(&lp).height_m());
    assert!(
        kinds(&lp)
            .iter()
            .any(|k| matches!(k, FindingKind::BrakeSlip { .. })),
        "{:?}",
        kinds(&lp)
    );
}

#[test]
fn a_brake_contactor_that_never_pulls_in_is_caught_by_the_feedback() {
    let mut rig = Rig::new();
    rig.feedback = true;
    let (mut lp, mut c, cmd) = rig.start();
    cmd.borrow_mut().contactor_wired = false;
    cmd.borrow_mut().up = true;
    run_for(&mut lp, &mut c, 8.0);
    let s = m(&lp).snapshot().unwrap();
    assert_eq!(
        s.logic.phase,
        LogicPhase::Fault(LogicFault::BrakeFeedback),
        "{:?}",
        kinds(&lp)
    );
    assert!(m(&lp).height_m().abs() < 0.001);
    assert!(!s.logic.brake_relay);
}

#[test]
fn without_feedback_the_motor_pulls_against_a_closed_brake() {
    let (mut lp, mut c, cmd) = Rig::new().start();
    cmd.borrow_mut().contactor_wired = false;
    cmd.borrow_mut().up = true;
    run_for(&mut lp, &mut c, 4.0);
    // The brake holds the shaft; the motor current stays up (the lifting torque is not enough
    // to turn against 80 N m).
    assert_eq!(m(&lp).rpm(), 0.0);
    assert!(m(&lp).snapshot().unwrap().motor.stator_current_a > 5.0);
}

#[test]
fn mains_loss_while_lowering_applies_the_brake_late_and_the_load_drops_some() {
    let (mut lp, mut c, cmd) = Rig::new().start();
    {
        let mut cm = cmd.borrow_mut();
        cm.down = true;
        cm.step2 = true;
    }
    run_for(&mut lp, &mut c, 12.0);
    assert!(m(&lp).snapshot().unwrap().speed_m_s < -0.05);
    let h = m(&lp).height_m();
    cmd.borrow_mut().mains = false;
    run_for(&mut lp, &mut c, 4.0);
    assert_eq!(m(&lp).snapshot().unwrap().speed_m_s, 0.0);
    let drop = h - m(&lp).height_m();
    // AC-side switching: 0.15 s delay + 0.05 s rise at 0.1 m/s or more: centimetres, not zero.
    assert!(drop > 0.01 && drop < 0.8, "{drop}");
    // The same event with the DC-side apply delay (0.03 s) drops the hook less.
    let mut rig = Rig::new();
    rig.brake_t = brake(Switching::DcSide);
    let (mut lp2, mut c2, cmd2) = rig.start();
    {
        let mut cm = cmd2.borrow_mut();
        cm.down = true;
        cm.step2 = true;
    }
    run_for(&mut lp2, &mut c2, 12.0);
    let h2 = m(&lp2).height_m();
    cmd2.borrow_mut().mains = false;
    run_for(&mut lp2, &mut c2, 4.0);
    let drop2 = h2 - m(&lp2).height_m();
    assert!(drop2 < drop, "{drop2} !< {drop}");
}

#[test]
fn lowering_with_no_braking_resistor_trips_on_over_voltage_and_the_brake_catches_the_load() {
    let mut rig = Rig::new();
    rig.resistor = None;
    let (mut lp, mut c, cmd) = rig.start();
    {
        let mut cm = cmd.borrow_mut();
        cm.down = true;
        cm.step3 = true;
    }
    run_for(&mut lp, &mut c, 20.0);
    let k = kinds(&lp);
    assert!(
        k.iter()
            .any(|k| matches!(k, FindingKind::DriveTrip(Fault::BusOverVoltage))),
        "{k:?}"
    );
    assert_eq!(m(&lp).snapshot().unwrap().speed_m_s, 0.0);
    // With a resistor the same lowering runs on.
    let (mut lp2, mut c2, cmd2) = Rig::new().start();
    {
        let mut cm = cmd2.borrow_mut();
        cm.down = true;
        cm.step3 = true;
    }
    run_for(&mut lp2, &mut c2, 20.0);
    assert!(
        !kinds(&lp2)
            .iter()
            .any(|k| matches!(k, FindingKind::DriveTrip(_))),
        "{:?}",
        kinds(&lp2)
    );
    assert!(m(&lp2).snapshot().unwrap().speed_m_s < -0.1);
}

#[test]
fn reversing_goes_through_a_full_brake_cycle() {
    let (mut lp, mut c, cmd) = Rig::new().start();
    cmd.borrow_mut().up = true;
    run_for(&mut lp, &mut c, 5.0);
    {
        let mut cm = cmd.borrow_mut();
        cm.up = false;
        cm.down = true;
    }
    let mut closed_between = false;
    let mut went_down = false;
    for _ in 0..(12.0 / DT) as usize {
        lp.step(&mut c);
        if c.contacts.get("=A-R2") == Some(&false) {
            closed_between = true;
        }
        if closed_between && m(&lp).snapshot().unwrap().speed_m_s < -0.01 {
            went_down = true;
        }
    }
    assert!(closed_between && went_down);
}

#[test]
fn speed_steps_select_the_frequencies() {
    let (mut lp, mut c, cmd) = Rig::new().start();
    cmd.borrow_mut().up = true;
    run_for(&mut lp, &mut c, 8.0);
    let v1 = m(&lp).snapshot().unwrap().speed_m_s;
    cmd.borrow_mut().step2 = true;
    run_for(&mut lp, &mut c, 8.0);
    let v2 = m(&lp).snapshot().unwrap().speed_m_s;
    cmd.borrow_mut().step3 = true;
    run_for(&mut lp, &mut c, 12.0);
    let v3 = m(&lp).snapshot().unwrap().speed_m_s;
    // Speeds follow 10, 25 and 50 Hz: 1 : 2.5 : 5 plus the effect of the slip, which is a
    // larger share of the speed at 10 Hz (about 240 of 300 rpm there against 1465 of 1500).
    assert!((2.4..3.0).contains(&(v2 / v1)), "{}", v2 / v1);
    assert!((5.0..6.8).contains(&(v3 / v1)), "{}", v3 / v1);
}

#[test]
fn light_load_runs_faster_than_a_heavy_one_on_the_top_step() {
    let top_speed = |mass: f64| {
        let mut rig = Rig::new();
        rig.mass = mass;
        rig.logic.sense = Some(LoadSense {
            probe_hz: 20.0,
            probe_time_s: 0.5,
            light_fraction: 0.25,
            boost_hz: 80.0,
            margin: 1.5,
        });
        let (mut lp, mut c, cmd) = rig.start();
        {
            let mut cm = cmd.borrow_mut();
            cm.up = true;
            cm.step2 = true;
            cm.step3 = true;
        }
        run_for(&mut lp, &mut c, 25.0);
        (
            m(&lp).snapshot().unwrap().speed_m_s,
            m(&lp).snapshot().unwrap().top_step_hz,
        )
    };
    let (v_light, f_light) = top_speed(200.0);
    let (v_heavy, f_heavy) = top_speed(3600.0);
    assert!(f_light > 70.0, "{f_light}");
    assert_eq!(f_heavy, 50.0);
    assert!(v_light > 1.4 * v_heavy, "{v_light} vs {v_heavy}");
}

#[test]
fn the_upper_limit_drops_the_command_and_the_brake_logic_stops_the_hook() {
    let mut rig = Rig::new();
    rig.limit_m = Some(0.25);
    let (mut lp, mut c, cmd) = rig.start();
    {
        let mut cm = cmd.borrow_mut();
        cm.up = true;
        cm.limit_cuts_up = true;
    }
    run_for(&mut lp, &mut c, 15.0);
    let h = m(&lp).height_m();
    assert!((0.25..0.40).contains(&h), "{h}");
    assert_eq!(m(&lp).snapshot().unwrap().speed_m_s, 0.0);
}

#[test]
fn a_run_is_deterministic() {
    let go = || {
        let (mut lp, mut c, cmd) = Rig::new().start();
        cmd.borrow_mut().up = true;
        run_for(&mut lp, &mut c, 4.0);
        cmd.borrow_mut().step2 = true;
        run_for(&mut lp, &mut c, 4.0);
        cmd.borrow_mut().up = false;
        run_for(&mut lp, &mut c, 4.0);
        (m(&lp).height_m(), kinds(&lp))
    };
    assert_eq!(go(), go());
}

#[test]
fn binding_problems_are_reported() {
    let rig = Rig::new();
    let (lp, _, _) = rig.start();
    let empty = FakeCircuit::new();
    let found = lp.check_bindings(&empty);
    assert!(found.iter().any(|f| f.message.contains("=K-UP")));
    assert!(found.iter().any(|f| f.message.contains("=A-R2")));
    let mut lp = lp;
    lp.modules[0].binding.brake_feed = None;
    let found = lp.check_bindings(&empty);
    assert!(found.iter().any(|f| f.message.contains("ideal wiring")));
}

/// Runs `first` for 8 s, then flips to `second` and logs the brake relay for 8 s.
fn reversal(rig: &Rig, first_up: bool) -> (DriveHoistLoop, bool, f64) {
    let (mut lp, mut c, cmd) = rig.start();
    {
        let mut cm = cmd.borrow_mut();
        cm.up = first_up;
        cm.down = !first_up;
    }
    run_for(&mut lp, &mut c, 8.0);
    {
        let mut cm = cmd.borrow_mut();
        cm.up = !first_up;
        cm.down = first_up;
    }
    let mut relay_opened = false;
    let mut turned_at = None;
    for k in 0..(8.0 / DT) as usize {
        lp.step(&mut c);
        if c.contacts.get("=A-R2") == Some(&false) {
            relay_opened = true;
        }
        let v = m(&lp).snapshot().unwrap().speed_m_s;
        if turned_at.is_none() && (v > 0.005) == !first_up && v.abs() > 0.005 {
            turned_at = Some(k as f64 * DT);
        }
    }
    (lp, relay_opened, turned_at.unwrap_or(f64::INFINITY))
}

fn jump_rig(ramp_s: f64) -> Rig {
    let mut rig = Rig::new();
    rig.logic.jump = Some(ReversalJump {
        jump_hz: 8.0,
        ramp_time_s: ramp_s,
    });
    rig
}

#[test]
fn a_reversal_with_the_jump_keeps_the_brake_open_and_turns_much_sooner() {
    // Up at step 1 (10 Hz), then down. Full cycle: down to 2 Hz (about 0.4 s), brake closes,
    // 0.5 s restart, magnetise and release again. With the jump nothing of that happens.
    let (lp_full, opened_full, t_full) = reversal(&Rig::new(), true);
    assert!(opened_full);
    let (lp_jump, opened_jump, t_jump) = reversal(&jump_rig(0.3), true);
    assert!(
        !opened_jump,
        "the brake relay stays closed through the jump"
    );
    assert!(t_jump + 0.8 < t_full, "{t_jump} vs {t_full}");
    assert!(m(&lp_jump).snapshot().unwrap().speed_m_s < -0.02);
    assert!(m(&lp_full).snapshot().unwrap().speed_m_s < -0.02);
}

#[test]
fn a_slow_pass_through_zero_lets_the_load_sink_more_than_a_fast_one() {
    // Down at step 1, then up with the brake open: the weight is not held while the field
    // passes zero. A fast band of 0.3 s gives the motor torque back sooner than the normal ramp.
    let (fast, _, _) = reversal(&jump_rig(0.3), false);
    let (slow, _, _) = reversal(&jump_rig(2.0), false);
    let sag = |lp: &DriveHoistLoop| {
        kinds(lp).iter().find_map(|k| match k {
            FindingKind::ReversalSag { metres } => Some(*metres),
            _ => None,
        })
    };
    let (s_fast, s_slow) = (sag(&fast).unwrap_or(0.0), sag(&slow).unwrap_or(0.0));
    assert!(s_slow > s_fast + 0.005, "{s_slow} vs {s_fast}");
    // The brake never closed in either run: the hook ends up rising.
    assert!(m(&fast).snapshot().unwrap().speed_m_s > 0.02);
}

fn floor_rig(floor_m: f64) -> Rig {
    let mut rig = Rig::new();
    rig.rope = Some(RopeConfig {
        floor_m,
        stiffness_n_per_m: 1.0e6,
        damping_ratio: 0.1,
    });
    rig
}

fn rope_finding<T>(lp: &DriveHoistLoop, pick: impl Fn(&FindingKind) -> Option<T>) -> Option<T> {
    kinds(lp).iter().find_map(pick)
}

#[test]
fn lowering_onto_the_floor_leaves_slack_rope_and_the_drum_carries_nothing() {
    // Floor 0.4 m below the start. The hook comes down at step 3 (about 0.19 m/s), the drum keeps
    // paying out rope until the command is dropped: the rope goes slack and the hook rests.
    let (mut lp, mut c, cmd) = floor_rig(-0.4).start();
    {
        let mut cm = cmd.borrow_mut();
        cm.down = true;
        cm.step2 = true;
        cm.step3 = true;
    }
    run_for(&mut lp, &mut c, 9.0);
    cmd.borrow_mut().down = false;
    run_for(&mut lp, &mut c, 4.0);
    let s = m(&lp).snapshot().unwrap();
    let rope = s.rope.unwrap();
    assert!(rope.on_floor);
    assert!(rope.tension_n < 1.0, "{}", rope.tension_n);
    assert!((rope.hook_m + 0.4).abs() < 1e-6);
    assert!(s.height_m < -0.6, "drum at {}", s.height_m);
    assert!((rope.slack_m - (-0.4 - s.height_m)).abs() < 0.01);
    let landed = rope_finding(&lp, |k| match k {
        FindingKind::FloorLanding { speed_m_s } => Some(*speed_m_s),
        _ => None,
    });
    assert!(landed.unwrap() > 0.1, "{landed:?}");
}

#[test]
fn raising_again_takes_the_slack_and_snatches_the_load_off_the_floor() {
    let (mut lp, mut c, cmd) = floor_rig(-0.4).start();
    {
        let mut cm = cmd.borrow_mut();
        cm.down = true;
        cm.step2 = true;
        cm.step3 = true;
    }
    run_for(&mut lp, &mut c, 9.0);
    {
        let mut cm = cmd.borrow_mut();
        cm.down = false;
    }
    run_for(&mut lp, &mut c, 4.0);
    let slack = m(&lp).snapshot().unwrap().rope.unwrap().slack_m;
    {
        let mut cm = cmd.borrow_mut();
        cm.up = true;
    }
    run_for(&mut lp, &mut c, 14.0);
    let taken = rope_finding(&lp, |k| match k {
        FindingKind::SlackRope { metres } => Some(*metres),
        _ => None,
    });
    assert!(
        (taken.unwrap() - slack).abs() < 0.02,
        "{taken:?} vs {slack}"
    );
    let snatch = rope_finding(&lp, |k| match k {
        FindingKind::LoadSnatch { factor, .. } => Some(*factor),
        _ => None,
    });
    assert!(snatch.unwrap() > 1.0, "{snatch:?}");
    let rope = m(&lp).snapshot().unwrap().rope.unwrap();
    assert!(!rope.on_floor);
    assert!(m(&lp).snapshot().unwrap().speed_m_s > 0.0);
}

#[test]
fn a_load_picked_up_through_slack_rope_is_sensed_lighter_than_it_is() {
    // The probe runs while the drum still takes up slack rope, so the torque it averages is far
    // below the real load: load sensing lets a heavy pick-up run on a boosted top step.
    let sensed = |rig: Rig, lower_first: bool| {
        let mut rig = rig;
        rig.mass = 3000.0;
        rig.logic.sense = Some(LoadSense {
            probe_hz: 20.0,
            probe_time_s: 0.5,
            light_fraction: 0.25,
            boost_hz: 80.0,
            margin: 1.5,
        });
        let (mut lp, mut c, cmd) = rig.start();
        if lower_first {
            {
                let mut cm = cmd.borrow_mut();
                cm.down = true;
                cm.step2 = true;
                cm.step3 = true;
            }
            run_for(&mut lp, &mut c, 9.0);
            cmd.borrow_mut().down = false;
            run_for(&mut lp, &mut c, 4.0);
            assert!(m(&lp).snapshot().unwrap().rope.unwrap().slack_m > 0.3);
        }
        {
            let mut cm = cmd.borrow_mut();
            cm.up = true;
            cm.step2 = true;
            cm.step3 = true;
        }
        run_for(&mut lp, &mut c, 4.0);
        kinds(&lp)
            .iter()
            .rev()
            .find_map(|k| match k {
                FindingKind::Logic(novo_drive::hoist_logic::LogicEvent::LoadSensed {
                    fraction,
                    ..
                }) => Some(*fraction),
                _ => None,
            })
            .unwrap()
    };
    let hanging = sensed(Rig::new(), false);
    let through_slack = sensed(floor_rig(-0.3), true);
    assert!(
        through_slack < 0.6 * hanging,
        "{through_slack} vs {hanging}"
    );
}

#[test]
fn without_a_floor_nothing_changes_and_there_is_no_rope_reading() {
    let (mut lp, mut c, cmd) = Rig::new().start();
    cmd.borrow_mut().up = true;
    run_for(&mut lp, &mut c, 3.0);
    assert!(m(&lp).snapshot().unwrap().rope.is_none());
    assert!(m(&lp).rope().is_none());
}
