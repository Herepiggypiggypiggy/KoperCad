//! Motor starter basics: does the overload relay, contactor and
//! short-circuit protective device suit the motor and each other
//! (IEC 60947-4-1, direct-on-line start).
//!
//! These are the rules a panel builder checks first, not a coordination
//! study: manufacturers' type 1 and type 2 tables decide the last word and
//! are entered here as the one number they give, the largest short-circuit
//! protective device the starter may be fused with.

use super::{Finding, Rule, num, within};

/// Rated full-load current in amperes of a three-phase motor from its
/// shaft power: `I = P / (√3·U·η·cos φ)`. `None` for values that are not
/// positive, or an efficiency or power factor above 1.
pub fn full_load_current(
    power_kw: f64,
    voltage: f64,
    efficiency: f64,
    power_factor: f64,
) -> Option<f64> {
    let valid = |v: f64, top: f64| v.is_finite() && v > 0.0 && v <= top;
    if !(valid(power_kw, f64::MAX)
        && valid(voltage, f64::MAX)
        && valid(efficiency, 1.0)
        && valid(power_factor, 1.0))
    {
        return None;
    }
    Some(power_kw * 1000.0 / (3.0_f64.sqrt() * voltage * efficiency * power_factor))
}

/// What protects the motor circuit against short circuit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShortCircuitDevice {
    /// A gG/gL fuse (or a fuse-switch) of the given rated current, A.
    Fuse { rated_a: f64 },
    /// A motor protection breaker or circuit breaker with an adjustable or
    /// fixed instantaneous trip: its rated (top) current and the current
    /// at which it trips at once, both in A.
    Breaker { rated_a: f64, instantaneous_a: f64 },
}

impl ShortCircuitDevice {
    fn rated(self) -> f64 {
        match self {
            ShortCircuitDevice::Fuse { rated_a } | ShortCircuitDevice::Breaker { rated_a, .. } => {
                rated_a
            }
        }
    }
}

/// A direct-on-line starter and the motor it drives. Fields left `None`
/// are not judged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Starter {
    /// Rated full-load current of the motor from its plate, A.
    pub motor_current: f64,
    /// Setting range of the overload relay (or motor protection breaker's
    /// thermal release), A: lowest and highest.
    pub overload_range: Option<(f64, f64)>,
    /// Contactor's rated operational current for AC-3, A.
    pub contactor_ac3_a: Option<f64>,
    pub short_circuit: Option<ShortCircuitDevice>,
    /// Largest short-circuit protective device the manufacturer allows in
    /// front of this contactor and relay (their coordination table), A.
    pub max_short_circuit_a: Option<f64>,
}

/// A direct start draws about six to eight times the rated current; the
/// peak of the first half cycle roughly doubles that. A fuse that rides
/// this out is at least 1.6 times the motor current for starts up to five
/// seconds (IEC 60269-1 gG time-current curve at 8×); an instantaneous
/// trip should sit above twelve times.
const FUSE_MIN_FACTOR: f64 = 1.6;
const INSTANTANEOUS_MIN_FACTOR: f64 = 12.0;

/// Checks a starter. Findings, in this order:
///
/// 1. overload relay whose range does not contain the motor current
///    (error: it cannot be set to protect the motor);
/// 2. the motor current in the bottom tenth of the range (warning: hard to
///    set exactly);
/// 3. contactor below the motor current (error);
/// 4. short-circuit device below the motor current (error), a fuse under
///    1.6 times it or an instantaneous trip under 12 times it (warnings:
///    trips on starting);
/// 5. short-circuit device above the coordination limit (error).
pub fn check_starter(starter: &Starter) -> Vec<Finding> {
    let mut out = Vec::new();
    let fla = starter.motor_current;
    if !(fla.is_finite() && fla > 0.0) {
        return out;
    }
    if let Some((low, high)) = starter.overload_range
        && low.is_finite()
        && high.is_finite()
        && low <= high
    {
        if !within(low, fla) || !within(fla, high) {
            out.push(Finding::error(
                Rule::MotorStarter,
                format!(
                    "the overload relay sets from {} to {} A, which cannot be set to the motor's {} A",
                    num(low),
                    num(high),
                    num(fla)
                ),
            ));
        } else if fla < low + 0.1 * (high - low) {
            out.push(Finding::warning(
                Rule::MotorStarter,
                format!(
                    "the motor's {} A is at the bottom of the overload relay's {} to {} A range; a smaller relay sets it more exactly",
                    num(fla),
                    num(low),
                    num(high)
                ),
            ));
        }
    }
    if let Some(ac3) = starter.contactor_ac3_a
        && ac3.is_finite()
        && ac3 > 0.0
        && !within(fla, ac3)
    {
        out.push(Finding::error(
            Rule::MotorStarter,
            format!(
                "the contactor is rated {} A (AC-3) but the motor draws {} A",
                num(ac3),
                num(fla)
            ),
        ));
    }
    if let Some(device) = starter.short_circuit {
        let rated = device.rated();
        if rated.is_finite() && rated > 0.0 {
            if !within(fla, rated) {
                out.push(Finding::error(
                    Rule::MotorStarter,
                    format!(
                        "the short-circuit device is rated {} A but the motor draws {} A",
                        num(rated),
                        num(fla)
                    ),
                ));
            } else {
                match device {
                    ShortCircuitDevice::Fuse { .. } if rated < FUSE_MIN_FACTOR * fla => {
                        out.push(Finding::warning(
                            Rule::MotorStarter,
                            format!(
                                "a {} A fuse may blow when the {} A motor starts; use at least {} A",
                                num(rated),
                                num(fla),
                                num((FUSE_MIN_FACTOR * fla).ceil())
                            ),
                        ));
                    }
                    ShortCircuitDevice::Breaker {
                        instantaneous_a, ..
                    } if instantaneous_a.is_finite()
                        && instantaneous_a > 0.0
                        && instantaneous_a < INSTANTANEOUS_MIN_FACTOR * fla =>
                    {
                        out.push(Finding::warning(
                            Rule::MotorStarter,
                            format!(
                                "the breaker trips at once at {} A, which a {} A motor's starting current can reach; set it above {} A",
                                num(instantaneous_a),
                                num(fla),
                                num(INSTANTANEOUS_MIN_FACTOR * fla)
                            ),
                        ));
                    }
                    _ => {}
                }
            }
            if let Some(limit) = starter.max_short_circuit_a
                && limit.is_finite()
                && limit > 0.0
                && !within(rated, limit)
            {
                out.push(Finding::error(
                    Rule::MotorStarter,
                    format!(
                        "the short-circuit device is {} A but the contactor and relay are only coordinated up to {} A",
                        num(rated),
                        num(limit)
                    ),
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    fn starter() -> Starter {
        Starter {
            motor_current: 9.0,
            overload_range: Some((7.0, 10.0)),
            contactor_ac3_a: Some(12.0),
            short_circuit: Some(ShortCircuitDevice::Fuse { rated_a: 16.0 }),
            max_short_circuit_a: Some(25.0),
        }
    }

    #[test]
    fn current_from_power() {
        // 4 kW, 400 V, η 0.86, cos φ 0.82: about 8.2 A.
        let i = full_load_current(4.0, 400.0, 0.86, 0.82).unwrap();
        assert!((i - 8.19).abs() < 0.02, "{i}");
        assert!(full_load_current(0.0, 400.0, 0.9, 0.8).is_none());
        assert!(full_load_current(4.0, 400.0, 1.1, 0.8).is_none());
        assert!(full_load_current(4.0, f64::NAN, 0.9, 0.8).is_none());
    }

    #[test]
    fn a_matching_starter_is_clean() {
        assert!(check_starter(&starter()).is_empty());
    }

    #[test]
    fn relay_range_must_contain_the_motor() {
        let mut s = starter();
        s.overload_range = Some((4.0, 6.3));
        let f = check_starter(&s);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Error);
        assert!(f[0].message.contains("4 to 6.3 A"));
        // Exactly the top of the range is allowed.
        s.overload_range = Some((6.0, 9.0));
        assert!(check_starter(&s).is_empty());
    }

    #[test]
    fn motor_at_the_bottom_of_the_range_warns() {
        let mut s = starter();
        s.overload_range = Some((9.0, 15.0));
        let f = check_starter(&s);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Warning);
    }

    #[test]
    fn contactor_too_small() {
        let mut s = starter();
        s.contactor_ac3_a = Some(7.0);
        let f = check_starter(&s);
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("contactor"));
    }

    #[test]
    fn fuse_sizes() {
        let mut s = starter();
        s.short_circuit = Some(ShortCircuitDevice::Fuse { rated_a: 10.0 });
        let f = check_starter(&s);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Warning);
        assert!(f[0].message.contains("at least 15 A"), "{}", f[0].message);

        s.short_circuit = Some(ShortCircuitDevice::Fuse { rated_a: 6.0 });
        assert_eq!(check_starter(&s)[0].severity, Severity::Error);

        s.short_circuit = Some(ShortCircuitDevice::Fuse { rated_a: 32.0 });
        let f = check_starter(&s);
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("coordinated up to 25 A"));
    }

    #[test]
    fn breaker_instantaneous_setting() {
        let mut s = starter();
        s.max_short_circuit_a = None;
        s.short_circuit = Some(ShortCircuitDevice::Breaker {
            rated_a: 10.0,
            instantaneous_a: 100.0,
        });
        let f = check_starter(&s);
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("above 108 A"), "{}", f[0].message);
        s.short_circuit = Some(ShortCircuitDevice::Breaker {
            rated_a: 10.0,
            instantaneous_a: 130.0,
        });
        assert!(check_starter(&s).is_empty());
    }

    #[test]
    fn nothing_known_says_nothing() {
        let s = Starter {
            motor_current: 9.0,
            overload_range: None,
            contactor_ac3_a: None,
            short_circuit: None,
            max_short_circuit_a: None,
        };
        assert!(check_starter(&s).is_empty());
        let bad = Starter {
            motor_current: 0.0,
            ..starter()
        };
        assert!(check_starter(&bad).is_empty());
    }
}
