//! Hoist families (lane L2d): conical-rotor and rectifier brakes, fine-lift motor, lost phase,
//! low voltage and brake wear. Expected values are worked out on paper in the comments.

use novo_drive::brake::{BrakeState, SpringBrake, Switching};
use novo_drive::brake_system::{
    BrakeSource, BrakeSystem, ConicalPull, Rectifier, RectifierFault, RectifierKind,
};
use novo_drive::hoist_unit::{
    BrakeFeed, Feeds, FineLift, HoistFaults, HoistSnapshot, TwoSpeedHoistUnit,
};
use novo_drive::mechanics::Hoist;
use novo_drive::motor::{Direction, PoleChangeMotor, Winding, WindingSel};
use novo_drive::sequencer::{Energised, Sequencer, SequencerConfig};
use novo_drive::supply::{Phase, Supply};

const DT: f64 = 0.002;

fn motor() -> PoleChangeMotor {
    // 12/2 hoist motor: fine 0.8 kW 480 of 500 rpm, coarse 6 kW 2880 of 3000 rpm.
    PoleChangeMotor::new(
        Winding::new(12, 50.0, 800.0, 480.0, 2.0).unwrap(),
        Winding::new(2, 50.0, 6000.0, 2880.0, 2.2).unwrap(),
        0.05,
    )
    .unwrap()
}

fn seq() -> Sequencer {
    Sequencer::new(SequencerConfig {
        changeover_dead_time_s: 0.1,
        fine_first_s: 0.5,
        reversal_dead_time_s: 0.2,
    })
    .unwrap()
}

/// 100 kg, r 0.1 m, i 40: 0.0025 m per rad, static torque 100 g 0.0025 = 2.4517 N m.
fn hoist() -> Hoist {
    Hoist::new(100.0, 0.1, 40.0, 1, 1.0).unwrap()
}

fn spring(apply_s: f64) -> SpringBrake {
    SpringBrake::new(60.0, 0.05, apply_s, apply_s, 0.0, Switching::AcSide).unwrap()
}

fn conical() -> TwoSpeedHoistUnit {
    TwoSpeedHoistUnit::with_system(
        motor(),
        hoist(),
        seq(),
        BrakeSystem::new(
            spring(0.05),
            BrakeSource::Conical(ConicalPull::new(0.5, 1.0).unwrap()),
        ),
        BrakeFeed::MotorTerminals,
    )
    .unwrap()
}

fn rectified(kind: RectifierKind, apply_s: f64) -> TwoSpeedHoistUnit {
    TwoSpeedHoistUnit::with_system(
        motor(),
        hoist(),
        seq(),
        BrakeSystem::new(
            spring(apply_s),
            BrakeSource::Rectifier(Rectifier::new(kind).unwrap()),
        ),
        BrakeFeed::MotorTerminals,
    )
    .unwrap()
}

fn fine_up() -> Option<Energised> {
    Some(Energised {
        winding: WindingSel::Fine,
        direction: Direction::Positive,
    })
}

fn run(
    u: &mut TwoSpeedHoistUnit,
    main: Option<Energised>,
    supply: Supply,
    seconds: f64,
) -> HoistSnapshot {
    let mut last = None;
    for _ in 0..(seconds / DT) as usize {
        let mut f = Feeds::simple(main, true, false);
        f.supply = supply;
        last = Some(u.step_feeds(f, DT));
    }
    last.unwrap()
}

fn height(s: &HoistSnapshot) -> f64 {
    s.hook_height_m
}

#[test]
fn conical_and_rectifier_hoists_reach_the_same_steady_speed() {
    // The brake family changes how the brake opens, not the motor: steady fine speed is the
    // closed-form slip speed for the load torque 2.4517 N m.
    let want = motor()
        .winding(WindingSel::Fine)
        .steady_omega_rad_s(Direction::Positive, 100.0 * novo_drive::GRAVITY * 0.0025)
        .unwrap()
        * 0.0025;
    for mut u in [conical(), rectified(RectifierKind::Bridge, 0.1)] {
        let s = run(&mut u, fine_up(), Supply::HEALTHY, 3.0);
        assert!((s.hook_speed_m_s - want).abs() < 0.01 * want, "{s:?}");
        assert_eq!(s.brake_state, BrakeState::Released);
        assert!(!s.conditions.any());
    }
}

#[test]
fn a_conical_brake_drops_sooner_than_a_slow_rectifier_brake_when_the_power_goes() {
    let stop_height = |mut u: TwoSpeedHoistUnit| {
        run(&mut u, fine_up(), Supply::HEALTHY, 2.0);
        let s = run(&mut u, None, Supply::HEALTHY, 2.0);
        assert_eq!(s.brake_state, BrakeState::Applied);
        height(&s)
    };
    // Same spring, but the rectifier is switched on the AC side with a 0.4 s apply delay,
    // against 0.05 s for the field-released brake.
    let conical_h = stop_height(conical());
    let slow_h = stop_height(rectified(RectifierKind::Bridge, 0.4));
    // At the moment of the power loss both are at the same height; the rectifier brake coasts
    // for longer, so the load has fallen back further (or risen less) before it bites.
    let mut a = conical();
    let mut b = rectified(RectifierKind::Bridge, 0.4);
    let h0 = height(&run(&mut a, fine_up(), Supply::HEALTHY, 2.0));
    let h0b = height(&run(&mut b, fine_up(), Supply::HEALTHY, 2.0));
    assert!((h0 - h0b).abs() < 1e-3);
    assert!(
        (conical_h - h0).abs() < (slow_h - h0).abs(),
        "conical moved {} m, slow rectifier {} m",
        conical_h - h0,
        slow_h - h0
    );
}

#[test]
fn a_conical_brake_does_not_lift_on_two_phases_from_standstill() {
    // Excitation on two phases: sqrt(0.5) = 0.707 of rated, below the 0.8 pull-in.
    let mut u = conical();
    let s = run(
        &mut u,
        fine_up(),
        Supply::HEALTHY.with_phase_lost(Phase::L2),
        3.0,
    );
    assert_eq!(s.brake_state, BrakeState::Applied);
    assert!(height(&s).abs() < 1e-6);
    assert!(s.conditions.fed_brake_closed);
    assert!(s.conditions.phase_lost);
    // Restore the phase: it opens and lifts.
    let s = run(&mut u, fine_up(), Supply::HEALTHY, 3.0);
    assert_eq!(s.brake_state, BrakeState::Released);
    assert!(height(&s) > 0.05);
}

#[test]
fn a_phase_lost_while_running_keeps_the_brake_open_but_weakens_the_motor() {
    let mut u = conical();
    let healthy = run(&mut u, fine_up(), Supply::HEALTHY, 3.0);
    // Drop-out is 0.4, the two-phase excitation 0.707: the brake stays open. The fine winding
    // loses about two thirds of its torque near rated speed, the load stays the same, so the
    // slip grows and the hook slows down but keeps rising.
    let s = run(
        &mut u,
        fine_up(),
        Supply::HEALTHY.with_phase_lost(Phase::L1),
        3.0,
    );
    assert_eq!(s.brake_state, BrakeState::Released);
    assert!(s.hook_speed_m_s > 0.0);
    assert!(
        s.hook_speed_m_s < healthy.hook_speed_m_s,
        "{} vs {}",
        s.hook_speed_m_s,
        healthy.hook_speed_m_s
    );
    assert!(s.conditions.phase_lost && !s.conditions.fed_brake_closed);
}

#[test]
fn low_voltage_holds_both_brake_families_shut() {
    // 70 % of rated is below the 80 % pull-in of either brake, and the motor could not lift
    // against the brake anyway.
    let dip = Supply::new(0.7, None).unwrap();
    for mut u in [conical(), rectified(RectifierKind::Bridge, 0.1)] {
        let s = run(&mut u, fine_up(), dip, 2.0);
        assert_eq!(s.brake_state, BrakeState::Applied);
        assert!(height(&s).abs() < 1e-6);
        assert!(s.conditions.low_voltage && s.conditions.fed_brake_closed);
    }
}

#[test]
fn rectifier_faults_decide_whether_the_brake_opens() {
    let opens = |kind, fault| {
        let mut u = rectified(kind, 0.1);
        u.set_faults(HoistFaults {
            rectifier: fault,
            ..HoistFaults::NONE
        })
        .unwrap();
        let s = run(&mut u, fine_up(), Supply::HEALTHY, 1.5);
        s.brake_state == BrakeState::Released
    };
    use RectifierFault::*;
    use RectifierKind::*;
    assert!(opens(Bridge, None));
    // One open diode halves the bridge output: 0.5 < 0.8 pull-in.
    assert!(!opens(Bridge, OpenDiode));
    // A half-wave rectifier with its single diode open has no output at all.
    assert!(!opens(HalfWave, OpenDiode));
    assert!(!opens(Bridge, OpenOutput));
    assert!(!opens(Bridge, ShortedDiode));
    // Three-phase bridge: one diode of six open leaves 5/6 = 0.83, still above pull-in.
    let fast = FastExcitation {
        boost: 2.0,
        boost_s: 0.2,
    };
    assert!(opens(fast, OpenDiode));
    // A weak output of 70 % does not.
    assert!(!opens(Bridge, LowOutput(0.7)));
    assert!(opens(Bridge, LowOutput(0.9)));
}

#[test]
fn a_rectifier_on_a_healthy_phase_opens_the_brake_while_the_motor_cannot_hold_the_load() {
    // The coil rectifier sits on L1-L2. Losing L3 leaves it fed, so the brake opens, but the
    // motor on two phases has exactly zero torque at standstill: the hook drops. This is the
    // dangerous case. Losing L1 kills the coil as well and the brake holds.
    let mut u = rectified(RectifierKind::Bridge, 0.1);
    let s = run(
        &mut u,
        fine_up(),
        Supply::HEALTHY.with_phase_lost(Phase::L3),
        2.0,
    );
    assert_eq!(s.brake_state, BrakeState::Released);
    assert!(height(&s) < -0.05, "{}", height(&s));
    assert!(s.conditions.phase_lost);
    let mut u = rectified(RectifierKind::Bridge, 0.1);
    let s = run(
        &mut u,
        fine_up(),
        Supply::HEALTHY.with_phase_lost(Phase::L1),
        2.0,
    );
    assert_eq!(s.brake_state, BrakeState::Applied);
    assert!(height(&s).abs() < 1e-6);
}

#[test]
fn a_slow_brake_lets_the_load_run_further() {
    let coast = |scale| {
        let mut u = conical();
        u.set_faults(HoistFaults {
            brake_delay_scale: scale,
            ..HoistFaults::NONE
        })
        .unwrap();
        let h0 = height(&run(&mut u, fine_up(), Supply::HEALTHY, 3.0));
        // Power off: the brake applies after 0.05 s x scale. The hook keeps rising on its
        // inertia meanwhile (about 0.1 m/s, slowed by the weight at 48 rad/s^2 on the shaft).
        let s = run(&mut u, None, Supply::HEALTHY, 3.0);
        assert_eq!(s.brake_state, BrakeState::Applied);
        height(&s) - h0
    };
    let normal = coast(1.0);
    let worn = coast(8.0);
    assert!(normal > 0.0 && normal < 0.02, "{normal}");
    // 8 times the delay: about 0.4 s of coasting at 0.1 m/s, several times the distance.
    assert!(worn > 3.0 * normal && worn > 0.02, "{normal} vs {worn}");
}

#[test]
fn a_worn_lining_below_the_load_torque_lets_the_load_slide() {
    let mut u = conical();
    run(&mut u, fine_up(), Supply::HEALTHY, 2.0);
    // 2 % of 60 N m = 1.2 N m, against 2.45 N m of weight.
    u.set_faults(HoistFaults {
        brake_torque_scale: 0.02,
        ..HoistFaults::NONE
    })
    .unwrap();
    let s = run(&mut u, None, Supply::HEALTHY, 3.0);
    assert_eq!(s.brake_state, BrakeState::Applied);
    assert!(s.hook_speed_m_s < -0.001, "{}", s.hook_speed_m_s);
    assert!(s.conditions.brake_slipping);
    // A healthy lining holds.
    let mut u = conical();
    run(&mut u, fine_up(), Supply::HEALTHY, 2.0);
    let s = run(&mut u, None, Supply::HEALTHY, 3.0);
    assert!(s.hook_speed_m_s.abs() < 1e-3);
    assert!(!s.conditions.brake_slipping);
}

#[test]
fn over_excitation_opens_the_brake_sooner() {
    let open_time = |kind| {
        let mut u = rectified(kind, 0.1);
        for i in 1..=2000 {
            let s = u.step_feeds(Feeds::simple(fine_up(), true, false), 0.001);
            if s.brake_state == BrakeState::Released {
                return f64::from(i) * 0.001;
            }
        }
        panic!("never opened");
    };
    let plain = open_time(RectifierKind::Bridge);
    let boosted = open_time(RectifierKind::FastExcitation {
        boost: 2.5,
        boost_s: 0.5,
    });
    // Release delay 0.05 s: 0.05 s plain, 0.02 s with 2.5 times over-excitation.
    assert!((plain - 0.05).abs() < 0.002, "{plain}");
    assert!((boosted - 0.02).abs() < 0.002, "{boosted}");
}

fn fine_lift_unit(releases_main_brake: bool) -> TwoSpeedHoistUnit {
    // 0.37 kW, 4 pole, 1380 of 1500 rpm: rated torque 0.37 kW / 144.5 rad/s = 2.56 N m. Turns 10
    // times faster than the main shaft at the hook.
    let w = Winding::new(4, 50.0, 370.0, 1380.0, 2.2).unwrap();
    let brake = BrakeSystem::new(
        SpringBrake::new(8.0, 0.05, 0.05, 0.05, 0.0, Switching::AcSide).unwrap(),
        BrakeSource::Conical(ConicalPull::new(0.5, 1.0).unwrap()),
    );
    conical().with_fine_lift(FineLift::new(w, 10.0, 0.002, brake, releases_main_brake).unwrap())
}

fn fine_only(u: &mut TwoSpeedHoistUnit, direction: Direction, seconds: f64) -> HoistSnapshot {
    let mut last = None;
    for _ in 0..(seconds / DT) as usize {
        let mut f = Feeds::simple(None, true, false);
        f.fine_lift = Some(direction);
        last = Some(u.step_feeds(f, DT));
    }
    last.unwrap()
}

#[test]
fn the_fine_lift_motor_raises_the_hook_slowly_at_the_closed_form_speed() {
    // Weight torque 2.4517 N m on the main shaft is 0.2452 N m on the fine motor. The Kloss
    // closed form gives its speed; the hook moves at that speed / 10 * 0.0025 m per rad.
    let w = Winding::new(4, 50.0, 370.0, 1380.0, 2.2).unwrap();
    let want = w
        .steady_omega_rad_s(
            Direction::Positive,
            100.0 * novo_drive::GRAVITY * 0.0025 / 10.0,
        )
        .unwrap()
        / 10.0
        * 0.0025;
    let mut u = fine_lift_unit(true);
    let s = fine_only(&mut u, Direction::Positive, 6.0);
    assert!(
        (s.hook_speed_m_s - want).abs() < 0.02 * want,
        "{} vs {want}",
        s.hook_speed_m_s
    );
    assert!(s.fine_engaged);
    assert_eq!(s.fine_brake_state, Some(BrakeState::Released));
    // About 4 cm/s: far slower than the main winding on fine (about 12 cm/s).
    assert!(s.hook_speed_m_s < 0.05);
}

#[test]
fn the_fine_lift_motor_lowers_and_holds_the_load_back() {
    let mut u = fine_lift_unit(true);
    fine_only(&mut u, Direction::Positive, 4.0);
    let s = fine_only(&mut u, Direction::Negative, 8.0);
    // Lowering with the field in the lowering sense: the weight aids the motor, which runs
    // slightly above its synchronous speed as a generator (157.08 rad/s / 10 * 0.0025 =
    // 0.03927 m/s at synchronous speed) and holds the load back there.
    let sync = 157.08 / 10.0 * 0.0025;
    let v = -s.hook_speed_m_s;
    assert!(v > sync && v < 1.05 * sync, "{v} vs {sync}");
}

#[test]
fn a_fine_lift_motor_cannot_move_a_drum_whose_main_brake_it_does_not_release() {
    let mut u = fine_lift_unit(false);
    let s = fine_only(&mut u, Direction::Positive, 3.0);
    assert_eq!(s.brake_state, BrakeState::Applied);
    assert!(height(&s).abs() < 1e-6);
    assert!(s.conditions.fed_brake_closed);
}

#[test]
fn both_motors_fed_together_is_reported_and_the_fine_motor_brakes() {
    let mut u = fine_lift_unit(true);
    let mut last = None;
    for _ in 0..(3.0 / DT) as usize {
        let mut f = Feeds::simple(
            Some(Energised {
                winding: WindingSel::Coarse,
                direction: Direction::Positive,
            }),
            true,
            false,
        );
        f.fine_lift = Some(Direction::Positive);
        last = Some(u.step_feeds(f, DT));
    }
    let s = last.unwrap();
    assert!(s.conditions.both_motors_fed);
    // The fine motor is dragged far above its synchronous speed: it brakes (negative torque).
    assert!(s.fine_torque_nm < 0.0, "{}", s.fine_torque_nm);
}

#[test]
fn a_hum_from_the_drawing_keeps_the_winding_and_has_no_starting_torque() {
    let mut u = conical();
    // Humming from standstill: two phases, no torque at zero speed, and the conical brake
    // stays shut at 0.707 excitation.
    let mut last = None;
    for _ in 0..(2.0 / DT) as usize {
        let mut f = Feeds::simple(None, true, false);
        f.humming = true;
        f.supply = Supply::HEALTHY.with_phase_lost(Phase::L2);
        last = Some(u.step_feeds(f, DT));
    }
    let s = last.unwrap();
    assert!(s.energised.is_some());
    assert!(s.conditions.phase_lost);
    assert!(height(&s).abs() < 1e-6);
}

#[test]
fn faults_refuse_silly_numbers_and_leave_the_unit_unchanged() {
    let mut u = conical();
    let before = *u.faults();
    assert!(
        u.set_faults(HoistFaults {
            brake_torque_scale: 1.5,
            ..HoistFaults::NONE
        })
        .is_err()
    );
    assert!(
        u.set_faults(HoistFaults {
            voltage_ratio: f64::NAN,
            ..HoistFaults::NONE
        })
        .is_err()
    );
    assert!(
        u.set_faults(HoistFaults {
            brake_delay_scale: 0.0,
            ..HoistFaults::NONE
        })
        .is_err()
    );
    assert_eq!(*u.faults(), before);
    assert!(u.faults().is_none());
}
