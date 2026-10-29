//! What releases a hoist brake, by hoist family (lane L2d).
//!
//! Two ways to lift a spring brake are modelled on top of [`SpringBrake`]:
//!
//! * [`ConicalPull`]: a conical-rotor motor. The spring pushes the rotor against the brake ring;
//!   the magnetic field of the stator pulls the rotor axially into the stator and lifts it off.
//!   There is no rectifier and no brake coil: the brake is released exactly when the motor has a
//!   strong enough field, and drops when the field goes (power loss, a lost phase, low voltage).
//! * [`Rectifier`]: a separate brake coil on a rectifier, from the motor terminals or a separate
//!   supply, with the usual variants (half-wave, bridge, over-excited, externally controlled)
//!   and rectifier faults.
//!
//! Both reduce to one number per tick, the excitation (a fraction of rated), that
//! [`SpringBrake::step_excitation`] turns into release and apply with delays and hysteresis.
//!
//! Exact by definition: the mean output of a three-phase bridge is 1.35 of the line voltage and
//! of a single-phase bridge 0.9, so a lost phase leaves a three-phase rectifier at 2/3 of its
//! output. Model: everything else (thresholds, delays, the pull of a conical motor on two phases,
//! the effect of a faulty diode). No manufacturer's data, no compliance claim.

use crate::Fidelity;
use crate::brake::{BrakeState, SpringBrake};
use crate::error::{self, InputError};
use crate::motor::WindingSel;
use crate::supply::{Phase, Supply};

/// Fidelity of the excitation figures other than the lost-phase ratio of a three-phase bridge.
pub const BRAKE_SYSTEM_FIDELITY: Fidelity = Fidelity::Model;

/// What the hoist-family model leaves out, shown next to the numbers in the UI.
pub const FAMILY_MODEL_LEAVES_OUT: &[&str] = &[
    "Brake thresholds, delays and the pull on two phases are numbers you enter or defaults, not the measurements of one particular brake",
    "Conical-rotor motor: the axial rotor travel, the release current surge and lining wear are not modelled; the brake is open, closed or on its way with a delay",
    "Rectifier: mean output only, no ripple, coil inductance or free-wheel current; a fault is a stated outcome (an open diode halves a bridge)",
    "Two phases: a double-revolving-field estimate (no torque at standstill); the real torque depends on the winding connection and the load",
    "Fine-lift motor: a rigid speed ratio while it is fed and a free path while it is not; real gear arrangements differ",
    "A phase open in the drawing is taken to be L3 (the brake rectifier on L1 and L2 stays fed); pick another phase under Faults to try",
    "No manufacturer data is used; a family is a set of generic behaviours you can set to match a data sheet",
];

/// Output of a three-phase bridge with a lost phase over its healthy output: `0.9 / 1.35`.
pub const THREE_PHASE_LOST_PHASE_RATIO: f64 = 0.9 / 1.35;

/// Which rectifier feeds the coil.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RectifierKind {
    /// One diode in series with the coil, from two motor terminals. Nothing to switch: the
    /// coil is fed whenever the motor is.
    HalfWave,
    /// Single-phase bridge from two motor terminals (or a separate supply).
    Bridge,
    /// Three-phase bridge with over-excitation: the coil gets `boost` times its rated voltage
    /// for `boost_s` seconds, which shortens the release delay by the same factor.
    FastExcitation { boost: f64, boost_s: f64 },
    /// Fed from a separate controlled low-voltage supply, so the motor supply does not matter.
    Controlled,
}

/// A fault in the rectifier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RectifierFault {
    None,
    /// One diode open: a bridge degrades to half-wave (half the output), a half-wave rectifier
    /// loses all output, a three-phase bridge loses one sixth.
    OpenDiode,
    /// A shorted diode blows the fuse or trips the protection: no output (model).
    ShortedDiode,
    /// The coil or its wiring is open.
    OpenOutput,
    /// The over-excitation stage failed: normal output, no boost.
    NoBoost,
    /// Output reduced to this share for any other reason (ageing, a failed capacitor).
    LowOutput(f64),
}

/// Which two terminals a single-phase rectifier is wired to.
pub const SINGLE_PHASE_TAP: (Phase, Phase) = (Phase::L1, Phase::L2);

/// A brake rectifier with its state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rectifier {
    kind: RectifierKind,
    fault: RectifierFault,
    fed_s: f64,
}

/// One tick of rectifier output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Output {
    /// Coil voltage as a fraction of the rated coil voltage.
    pub excitation: f64,
    /// Factor on the release delay (1 = none).
    pub release_speedup: f64,
}

impl Rectifier {
    pub fn new(kind: RectifierKind) -> Result<Self, InputError> {
        if let RectifierKind::FastExcitation { boost, boost_s } = kind {
            error::finite("boost", boost)?;
            if !(1.0..=10.0).contains(&boost) {
                return Err(InputError::OutOfRange {
                    field: "boost",
                    value: boost,
                    min: 1.0,
                    max: 10.0,
                });
            }
            error::non_negative("boost_s", boost_s)?;
        }
        Ok(Self {
            kind,
            fault: RectifierFault::None,
            fed_s: 0.0,
        })
    }

    pub fn kind(&self) -> RectifierKind {
        self.kind
    }

    pub fn fault(&self) -> RectifierFault {
        self.fault
    }

    pub fn set_fault(&mut self, fault: RectifierFault) -> Result<(), InputError> {
        if let RectifierFault::LowOutput(f) = fault {
            error::fraction("rectifier output", f)?;
        }
        self.fault = fault;
        Ok(())
    }

    /// Output for one tick of `dt_s`. `fed` is whether the rectifier input is switched on.
    pub fn output(&mut self, fed: bool, supply: &Supply, dt_s: f64) -> Output {
        if !fed {
            self.fed_s = 0.0;
            return Output {
                excitation: 0.0,
                release_speedup: 1.0,
            };
        }
        let before = self.fed_s;
        self.fed_s += dt_s;

        let v = if matches!(self.kind, RectifierKind::Controlled) {
            1.0
        } else {
            supply.voltage_ratio()
        };
        let mut e = match (self.kind, supply.lost_phase()) {
            (RectifierKind::FastExcitation { .. }, Some(_)) => v * THREE_PHASE_LOST_PHASE_RATIO,
            (RectifierKind::HalfWave | RectifierKind::Bridge, Some(p))
                if p == SINGLE_PHASE_TAP.0 || p == SINGLE_PHASE_TAP.1 =>
            {
                0.0
            }
            _ => v,
        };
        e *= match (self.fault, self.kind) {
            (RectifierFault::None | RectifierFault::NoBoost, _) => 1.0,
            (RectifierFault::OpenDiode, RectifierKind::Bridge | RectifierKind::Controlled) => 0.5,
            (RectifierFault::OpenDiode, RectifierKind::HalfWave) => 0.0,
            (RectifierFault::OpenDiode, RectifierKind::FastExcitation { .. }) => 5.0 / 6.0,
            (RectifierFault::ShortedDiode | RectifierFault::OpenOutput, _) => 0.0,
            (RectifierFault::LowOutput(f), _) => f,
        };
        let speedup = match self.kind {
            RectifierKind::FastExcitation { boost, boost_s }
                if before < boost_s && self.fault != RectifierFault::NoBoost && e > 0.0 =>
            {
                boost
            }
            _ => 1.0,
        };
        Output {
            excitation: e,
            release_speedup: speedup,
        }
    }
}

/// The field pull of a conical-rotor motor, as an excitation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConicalPull {
    /// Share of the three-phase pull that remains with a phase lost (the field becomes a
    /// pulsating one). Model; the pull follows the flux squared, so the excitation scales
    /// with its square root.
    phase_lost_pull: f64,
    /// Pull of the fine (high pole count) winding relative to the coarse one at rated voltage.
    fine_pull: f64,
}

impl ConicalPull {
    /// Typical-looking defaults, to be replaced by data sheet values: `phase_lost_pull` 0.5,
    /// equal pull on both windings.
    pub fn new(phase_lost_pull: f64, fine_pull: f64) -> Result<Self, InputError> {
        error::fraction("conical phase_lost_pull", phase_lost_pull)?;
        error::positive("conical fine_pull", fine_pull)?;
        Ok(Self {
            phase_lost_pull,
            fine_pull,
        })
    }

    pub fn phase_lost_pull(&self) -> f64 {
        self.phase_lost_pull
    }

    pub fn fine_pull(&self) -> f64 {
        self.fine_pull
    }

    /// Excitation for the winding fed. The pull is `(V/Vn)^2` times the winding factor, so the
    /// excitation (a voltage-like number) is `V/Vn` times the root of the winding factor.
    pub fn excitation(&self, winding: Option<WindingSel>, supply: &Supply) -> f64 {
        let Some(winding) = winding else { return 0.0 };
        let mut pull = match winding {
            WindingSel::Fine => self.fine_pull,
            WindingSel::Coarse => 1.0,
        };
        if supply.phase_lost() {
            pull *= self.phase_lost_pull;
        }
        supply.voltage_ratio() * pull.sqrt()
    }
}

/// What lifts the spring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrakeSource {
    Rectifier(Rectifier),
    Conical(ConicalPull),
}

/// What a brake system reads each tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrakeInput {
    /// For a rectifier brake: the rectifier input is switched on. For a conical brake: ignored
    /// (the winding says it).
    pub rectifier_fed: bool,
    /// The winding fed on the motor that carries this brake.
    pub winding: Option<WindingSel>,
    pub supply: Supply,
}

/// A spring brake and the source that lifts it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrakeSystem {
    pub brake: SpringBrake,
    source: BrakeSource,
    excitation: f64,
}

impl BrakeSystem {
    pub fn new(brake: SpringBrake, source: BrakeSource) -> Self {
        Self {
            brake,
            source,
            excitation: 0.0,
        }
    }

    pub fn source(&self) -> &BrakeSource {
        &self.source
    }

    pub fn source_mut(&mut self) -> &mut BrakeSource {
        &mut self.source
    }

    /// Excitation of the last tick, as a share of rated.
    pub fn excitation(&self) -> f64 {
        self.excitation
    }

    pub fn holding_torque_nm(&self) -> f64 {
        self.brake.holding_torque_nm()
    }

    pub fn state(&self) -> BrakeState {
        self.brake.state()
    }

    /// Advance one tick.
    pub fn step(&mut self, input: BrakeInput, dt_s: f64) {
        let (e, speedup) = match &mut self.source {
            BrakeSource::Rectifier(r) => {
                let o = r.output(input.rectifier_fed, &input.supply, dt_s);
                (o.excitation, o.release_speedup)
            }
            BrakeSource::Conical(c) => (c.excitation(input.winding, &input.supply), 1.0),
        };
        self.excitation = e;
        self.brake.step_excitation(e, speedup, dt_s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brake::Switching;

    fn brake() -> SpringBrake {
        SpringBrake::new(60.0, 0.10, 0.10, 0.02, 0.0, Switching::AcSide).unwrap()
    }

    fn fed(rect: &mut Rectifier, supply: Supply) -> Output {
        rect.output(true, &supply, 0.01)
    }

    #[test]
    fn a_lost_phase_leaves_a_three_phase_bridge_at_two_thirds() {
        let mut r = Rectifier::new(RectifierKind::FastExcitation {
            boost: 2.0,
            boost_s: 0.2,
        })
        .unwrap();
        let o = fed(&mut r, Supply::HEALTHY.with_phase_lost(Phase::L3));
        assert!((o.excitation - 2.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn a_single_phase_rectifier_survives_only_the_phase_it_does_not_use() {
        for kind in [RectifierKind::HalfWave, RectifierKind::Bridge] {
            let mut r = Rectifier::new(kind).unwrap();
            assert_eq!(
                fed(&mut r, Supply::HEALTHY.with_phase_lost(Phase::L3)).excitation,
                1.0
            );
            assert_eq!(
                fed(&mut r, Supply::HEALTHY.with_phase_lost(Phase::L1)).excitation,
                0.0
            );
            assert_eq!(
                fed(&mut r, Supply::HEALTHY.with_phase_lost(Phase::L2)).excitation,
                0.0
            );
        }
    }

    #[test]
    fn a_controlled_rectifier_ignores_the_motor_supply() {
        let mut r = Rectifier::new(RectifierKind::Controlled).unwrap();
        let s = Supply::new(0.3, Some(Phase::L1)).unwrap();
        assert_eq!(fed(&mut r, s).excitation, 1.0);
    }

    #[test]
    fn open_diode_by_kind() {
        let one = |kind| {
            let mut r = Rectifier::new(kind).unwrap();
            r.set_fault(RectifierFault::OpenDiode).unwrap();
            fed(&mut r, Supply::HEALTHY).excitation
        };
        assert_eq!(one(RectifierKind::Bridge), 0.5);
        assert_eq!(one(RectifierKind::HalfWave), 0.0);
        assert!(
            (one(RectifierKind::FastExcitation {
                boost: 2.0,
                boost_s: 0.1
            }) - 5.0 / 6.0)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn over_excitation_lasts_only_for_the_boost_time() {
        let mut r = Rectifier::new(RectifierKind::FastExcitation {
            boost: 3.0,
            boost_s: 0.05,
        })
        .unwrap();
        let mut sped = 0;
        for _ in 0..20 {
            if r.output(true, &Supply::HEALTHY, 0.01).release_speedup > 1.0 {
                sped += 1;
            }
        }
        assert_eq!(sped, 5);
        // Off resets it.
        r.output(false, &Supply::HEALTHY, 0.01);
        assert_eq!(r.output(true, &Supply::HEALTHY, 0.01).release_speedup, 3.0);
        r.set_fault(RectifierFault::NoBoost).unwrap();
        r.output(false, &Supply::HEALTHY, 0.01);
        assert_eq!(r.output(true, &Supply::HEALTHY, 0.01).release_speedup, 1.0);
    }

    #[test]
    fn the_boost_shortens_the_release_time_by_its_factor() {
        let time_to_release = |kind| -> f64 {
            let mut sys = BrakeSystem::new(
                brake(),
                BrakeSource::Rectifier(Rectifier::new(kind).unwrap()),
            );
            let mut t = 0.0_f64;
            while sys.state() == BrakeState::Applied || sys.holding_torque_nm() > 0.0 {
                sys.step(
                    BrakeInput {
                        rectifier_fed: true,
                        winding: None,
                        supply: Supply::HEALTHY,
                    },
                    0.001,
                );
                t += 0.001;
                assert!(t < 1.0);
            }
            t
        };
        let plain = time_to_release(RectifierKind::Bridge);
        let fast = time_to_release(RectifierKind::FastExcitation {
            boost: 2.0,
            boost_s: 1.0,
        });
        assert!((plain - 0.10).abs() < 0.003, "{plain}");
        assert!((fast - 0.05).abs() < 0.003, "{fast}");
    }

    #[test]
    fn a_conical_brake_lifts_on_the_motor_field_and_drops_when_it_goes() {
        let mut sys = BrakeSystem::new(
            brake(),
            BrakeSource::Conical(ConicalPull::new(0.5, 1.0).unwrap()),
        );
        let input = |w, s| BrakeInput {
            rectifier_fed: false,
            winding: w,
            supply: s,
        };
        for _ in 0..200 {
            sys.step(input(Some(WindingSel::Coarse), Supply::HEALTHY), 0.001);
        }
        assert_eq!(sys.holding_torque_nm(), 0.0);
        for _ in 0..400 {
            sys.step(input(None, Supply::HEALTHY), 0.001);
        }
        assert_eq!(sys.holding_torque_nm(), 60.0);
    }

    #[test]
    fn a_conical_brake_stays_shut_on_a_lost_phase_from_standstill_but_keeps_open_if_running() {
        let two = Supply::HEALTHY.with_phase_lost(Phase::L2);
        let run = |first: Supply, then: Supply| {
            let mut sys = BrakeSystem::new(
                brake(),
                BrakeSource::Conical(ConicalPull::new(0.5, 1.0).unwrap()),
            );
            for _ in 0..300 {
                sys.step(
                    BrakeInput {
                        rectifier_fed: false,
                        winding: Some(WindingSel::Coarse),
                        supply: first,
                    },
                    0.001,
                );
            }
            let opened = sys.holding_torque_nm() == 0.0;
            for _ in 0..300 {
                sys.step(
                    BrakeInput {
                        rectifier_fed: false,
                        winding: Some(WindingSel::Coarse),
                        supply: then,
                    },
                    0.001,
                );
            }
            (opened, sys.holding_torque_nm() == 0.0)
        };
        // Excitation on two phases: sqrt(0.5) = 0.707, below pull-in 0.8, above drop-out 0.4.
        assert_eq!(run(two, two), (false, false));
        assert_eq!(run(Supply::HEALTHY, two), (true, true));
        // A voltage dip to 0.3 drops it either way.
        let dip = Supply::new(0.3, None).unwrap();
        assert_eq!(run(Supply::HEALTHY, dip), (true, false));
    }

    #[test]
    fn refuses_bad_numbers() {
        assert!(
            Rectifier::new(RectifierKind::FastExcitation {
                boost: 0.5,
                boost_s: 0.1
            })
            .is_err()
        );
        assert!(ConicalPull::new(0.0, 1.0).is_err());
        let mut r = Rectifier::new(RectifierKind::Bridge).unwrap();
        assert!(r.set_fault(RectifierFault::LowOutput(0.0)).is_err());
    }
}
