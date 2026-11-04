//! What goes on the coordination plot next to the trip curves: the damage
//! curve of the cable each device protects and the start curve of each
//! motor behind it, and whether the device's curve lies between them.
//!
//! Everything here is read from the project and the single-line diagram
//! ([`SingleLine`]) and is plain data for the view to paint.
//!
//! **Model or maker data.** The trip curves of the devices are the standard
//! band limits of the device class (IEC 60898-1, IEC 60269, IEC 60947-4-1),
//! not a maker's own curves. The cable damage curve is the adiabatic
//! `I²t = k²S²` of IEC 60364-4-43 and holds from about 5 s down to 5 ms. The
//! motor start curve is a block: the starting current for the starting time,
//! from `starting_current_ratio` and `starting_time_s` on the motor when
//! entered, else typical values (marked assumed). The first-cycle inrush peak
//! is not drawn. [`CoordinationPlot::basis`] says this in words.

use std::collections::HashMap;

use novo_core::{DeviceId, Project};

use super::curves::{Band, TripCurve};
use super::model::Study;
use crate::calc_checks::device_number;
use crate::single_line::{Kind, Origin, SingleLine};

/// Property on a motor: starting current as a multiple of rated current.
pub const START_RATIO: &str = "starting_current_ratio";
/// Alternative name of [`START_RATIO`] (locked rotor).
pub const START_RATIO_ALT: &str = "locked_rotor_ratio";
/// Property on a motor: starting time in seconds.
pub const START_TIME_S: &str = "starting_time_s";
/// Starting current multiple used when the motor does not state one.
pub const TYPICAL_START_RATIO: f64 = 7.0;
/// Starting time used when the motor does not state one, seconds.
pub const TYPICAL_START_TIME_S: f64 = 5.0;
/// The adiabatic formula is not used beyond this time, seconds.
pub const ADIABATIC_MAX_S: f64 = 5.0;
/// ... nor below this one, seconds (drawn down to here).
pub const ADIABATIC_MIN_S: f64 = 0.005;
/// Below this time (IEC 60364-4-43) the device's let-through energy `I\u{b2}t`
/// is compared with `k\u{b2}S\u{b2}`, not its trip time, so the plot's verdict
/// stops here.
pub const TIME_CHECK_MIN_S: f64 = 0.1;

/// How a device sits against a cable or motor curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The device curve keeps clear of it everywhere that matters.
    Pass,
    /// It may cross: the bands overlap, or data was assumed.
    Warning,
    /// It crosses.
    Fail,
}

/// The damage curve of a cable (or the thinnest cable behind a device).
#[derive(Debug, Clone, PartialEq)]
pub struct CableCurve {
    /// Index of the protecting device in [`Study::devices`].
    pub device: usize,
    /// The run, as the single-line diagram labels it.
    pub label: String,
    /// Cross-section, mm².
    pub section_mm2: f64,
    /// Where the cross-section comes from.
    pub section_origin: Origin,
    /// Material constant `k` (copper).
    pub k: f64,
    /// `k²S²`, A²s.
    pub i2t: f64,
    /// The curve as multiples of 1 A: `(amperes, seconds)` from 5 s to 5 ms.
    pub band: Band,
    /// Largest fault current the cable can see, A (the fault level at the
    /// protecting device), when known.
    pub ik_a: Option<f64>,
    /// The verdict.
    pub status: Status,
    /// One sentence for the verdict.
    pub text: String,
}

impl CableCurve {
    /// Seconds the cable withstands `i_a`, `None` outside 5 s to 5 ms.
    pub fn time_at(&self, i_a: f64) -> Option<f64> {
        if !(i_a.is_finite() && i_a > 0.0) {
            return None;
        }
        let (lo, hi) = self.range_a();
        (i_a >= lo * 0.9999 && i_a <= hi * 1.0001).then(|| self.i2t / (i_a * i_a))
    }

    /// Currents the curve covers, A: from 5 s down to 5 ms.
    pub fn range_a(&self) -> (f64, f64) {
        (
            (self.i2t / ADIABATIC_MAX_S).sqrt(),
            (self.i2t / ADIABATIC_MIN_S).sqrt(),
        )
    }
}

/// The start of a motor behind one or more devices.
#[derive(Debug, Clone, PartialEq)]
pub struct MotorStart {
    /// The motor's tag.
    pub tag: String,
    /// Rated current, A.
    pub rated_a: f64,
    /// Starting current, A.
    pub start_a: f64,
    /// Starting time, s.
    pub time_s: f64,
    /// The current multiple was not entered on the motor.
    pub ratio_assumed: bool,
    /// The starting time was not entered on the motor.
    pub time_assumed: bool,
    /// Indices of the devices in series with the motor, nearest first.
    pub devices: Vec<usize>,
}

impl MotorStart {
    /// The outline of the start, `(amperes, seconds)`: up at the starting
    /// current, along for the starting time, back to rated current.
    pub fn outline(&self) -> Vec<(f64, f64)> {
        vec![
            (self.start_a, 0.01),
            (self.start_a, self.time_s),
            (self.rated_a, self.time_s),
        ]
    }
}

/// A device against a cable or a motor.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotCheck {
    /// Index of the device in [`Study::devices`].
    pub device: usize,
    /// Cable or motor it was checked against (`-M1`, `-W3`).
    pub subject: String,
    /// What was checked.
    pub kind: CheckKind,
    /// Result.
    pub status: Status,
    /// One sentence.
    pub text: String,
}

/// What a [`PlotCheck`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// Does the device clear a fault before the cable is damaged.
    Cable,
    /// Does the device keep out of the motor's start.
    MotorStart,
}

/// The overlays and checks of a project.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoordinationPlot {
    /// Cable damage curves, at most one per device.
    pub cables: Vec<CableCurve>,
    /// Motor starts.
    pub motors: Vec<MotorStart>,
    /// Every check, cable and motor.
    pub checks: Vec<PlotCheck>,
}

impl CoordinationPlot {
    /// The sentences that say what is model and what is data, for the "?"
    /// help of the plot.
    pub fn basis() -> Vec<&'static str> {
        vec![
            "Device curves are the standard band limits of the device type (IEC 60898-1, IEC 60269, IEC 60947-4-1), not a maker's own curves. The band runs from the fastest to the slowest trip.",
            "The cable curve is the adiabatic k\u{b2}S\u{b2} of IEC 60364-4-43 for copper, drawn from 5 s to 5 ms. The verdict covers 5 s to 0.1 s; faster than that the device's let-through energy decides, not its trip time; below it, the overload check of the single-line diagram applies.",
            "The motor start is a block: starting current for the starting time. The first-cycle inrush peak is not drawn. Values not entered on the motor are typical ones and marked assumed.",
        ]
    }

    /// Reads the overlays and runs the checks for the devices of `study`.
    pub fn read(project: &Project, study: &Study, line: &SingleLine) -> CoordinationPlot {
        let mut out = CoordinationPlot::default();
        let index: HashMap<DeviceId, usize> = study
            .devices
            .iter()
            .enumerate()
            .map(|(i, d)| (d.id, i))
            .collect();
        // Box tag -> device index, for "protected by".
        let by_tag: HashMap<&str, usize> = line
            .nodes
            .iter()
            .filter_map(|n| Some((n.tag.as_str(), *index.get(&n.device?)?)))
            .collect();
        out.read_cables(study, line, &by_tag);
        out.read_motors(project, study, line, &index);
        out
    }

    fn read_cables(&mut self, study: &Study, line: &SingleLine, by_tag: &HashMap<&str, usize>) {
        // The thinnest run behind each device is the one to protect.
        let mut thinnest: HashMap<usize, (f64, Origin, &crate::single_line::Link, String)> =
            HashMap::new();
        for node in &line.nodes {
            let Some(link) = &node.link else { continue };
            let Some((tag, _)) = &link.protected_by else {
                continue;
            };
            let Some(&dev) = by_tag.get(tag.as_str()) else {
                continue;
            };
            let Some(section) = link.section_mm2.value.filter(|s| *s > 0.0) else {
                continue;
            };
            let label = link
                .segments
                .first()
                .map(|s| s.cable.clone().unwrap_or_else(|| s.label.clone()))
                .unwrap_or_default();
            let label = if label.is_empty() {
                format!("to {}", node.tag)
            } else {
                label
            };
            let better = thinnest.get(&dev).is_none_or(|(s, ..)| section < *s);
            if better {
                thinnest.insert(dev, (section, link.section_mm2.origin, link, label));
            }
        }
        let mut devices: Vec<usize> = thinnest.keys().copied().collect();
        devices.sort_unstable();
        for dev in devices {
            let (section, origin, link, label) = &thinnest[&dev];
            let k = link.insulation.k_copper();
            let pd = &study.devices[dev];
            // The fault level at the device's own terminals, A.
            let ik_a = line
                .nodes
                .iter()
                .find(|n| n.device == Some(pd.id))
                .and_then(|n| n.ik_ka.value)
                .or(study.supply_ka)
                .map(|ka| ka * 1000.0)
                .filter(|a| *a > 0.0);
            let Ok(band) = TripCurve::cable_withstand(*section, k, 1.0) else {
                continue;
            };
            let mut curve = CableCurve {
                device: dev,
                label: label.clone(),
                section_mm2: *section,
                section_origin: *origin,
                k,
                i2t: k * k * section * section,
                band,
                ik_a,
                status: Status::Pass,
                text: String::new(),
            };
            let (status, text) = judge_cable(&pd.device.curve, &curve);
            curve.status = status;
            curve.text = text.clone();
            self.checks.push(PlotCheck {
                device: dev,
                subject: curve.label.clone(),
                kind: CheckKind::Cable,
                status,
                text,
            });
            self.cables.push(curve);
        }
    }

    fn read_motors(
        &mut self,
        project: &Project,
        study: &Study,
        line: &SingleLine,
        index: &HashMap<DeviceId, usize>,
    ) {
        for node in &line.nodes {
            if node.kind != Kind::Motor {
                continue;
            }
            let Some(rated_a) = node
                .current_a
                .value
                .or(node.rated_a.value)
                .filter(|a| *a > 0.0)
            else {
                continue;
            };
            // Devices on the way up, nearest first, up to a converter that
            // hides the motor's start from what is above it.
            let mut devices = Vec::new();
            let mut at = node.parent;
            while let Some(i) = at {
                let up = &line.nodes[i];
                if matches!(
                    up.kind,
                    Kind::Drive | Kind::SoftStarter | Kind::Transformer | Kind::PowerSupply
                ) {
                    devices.clear();
                    break;
                }
                if let Some(&d) = up.device.as_ref().and_then(|id| index.get(id)) {
                    devices.push(d);
                }
                at = up.parent;
            }
            if devices.is_empty() {
                continue;
            }
            let Some(id) = node.device else { continue };
            let num = |key: &str| device_number(project, id, key).filter(|v| *v > 0.0);
            let ratio = num(START_RATIO).or_else(|| num(START_RATIO_ALT));
            let time = num(START_TIME_S);
            let start = MotorStart {
                tag: node.tag.clone(),
                rated_a,
                start_a: rated_a * ratio.unwrap_or(TYPICAL_START_RATIO),
                time_s: time.unwrap_or(TYPICAL_START_TIME_S),
                ratio_assumed: ratio.is_none(),
                time_assumed: time.is_none(),
                devices,
            };
            for &d in &start.devices {
                let (status, text) = judge_motor(&study.devices[d].device.curve, &start);
                self.checks.push(PlotCheck {
                    device: d,
                    subject: start.tag.clone(),
                    kind: CheckKind::MotorStart,
                    status,
                    text,
                });
            }
            self.motors.push(start);
        }
    }

    /// The worst status over the checks of one device, `None` when it has
    /// none.
    pub fn status_of(&self, device: usize) -> Option<Status> {
        self.checks
            .iter()
            .filter(|c| c.device == device)
            .map(|c| c.status)
            .max_by_key(|s| match s {
                Status::Pass => 0,
                Status::Warning => 1,
                Status::Fail => 2,
            })
    }
}

fn fmt_a(a: f64) -> String {
    if a >= 1000.0 {
        format!("{:.1} kA", a / 1000.0)
    } else {
        format!("{a:.0} A")
    }
}

fn fmt_s(s: f64) -> String {
    if s < 1.0 {
        format!("{:.0} ms", s * 1000.0)
    } else {
        format!("{s:.1} s")
    }
}

/// Scans the currents where the cable curve applies (up to the fault level
/// there) and compares the device's slowest trip with the cable's time.
fn judge_cable(device: &TripCurve, cable: &CableCurve) -> (Status, String) {
    let lo = cable.range_a().0;
    let hi = (cable.i2t / TIME_CHECK_MIN_S).sqrt();
    let top = cable.ik_a.map_or(hi, |ik| ik.min(hi));
    let assumed = cable.section_origin != Origin::Stated;
    let origin_note = if assumed {
        " (cross-section not stated, taken from the diagram)"
    } else {
        ""
    };
    if top <= lo {
        return (
            Status::Pass,
            format!(
                "A fault here is at most {}, below the {} where the cable curve starts{origin_note}.",
                fmt_a(top),
                fmt_a(lo)
            ),
        );
    }
    let n = 240;
    let ratio = (top / lo).powf(1.0 / f64::from(n - 1));
    let mut worst: Option<(f64, bool)> = None; // (current, certain)
    let mut i = lo;
    for _ in 0..n {
        let t_cable = cable.i2t / (i * i);
        match device.trip_time_range(i) {
            None => {
                worst = worst.or(Some((i, true)));
                break;
            }
            Some((t_min, t_max)) => {
                if t_max >= t_cable {
                    let certain = t_min >= t_cable;
                    match worst {
                        Some((_, true)) => {}
                        _ if certain => {
                            worst = Some((i, true));
                            break;
                        }
                        None => worst = Some((i, false)),
                        _ => {}
                    }
                }
            }
        }
        i *= ratio;
    }
    match worst {
        Some((i, true)) => (
            Status::Fail,
            format!(
                "The device is too slow at {}: the cable is damaged after {}{origin_note}.",
                fmt_a(i),
                fmt_s(cable.i2t / (i * i))
            ),
        ),
        Some((i, false)) => (
            Status::Warning,
            format!(
                "At {} the slowest trip of the device is later than the cable allows ({}): not guaranteed{origin_note}.",
                fmt_a(i),
                fmt_s(cable.i2t / (i * i))
            ),
        ),
        None => (
            Status::Pass,
            format!(
                "The device clears before the cable is damaged from {} to {}{origin_note}. Above that the let-through energy decides (see the single-line and coordination checks).",
                fmt_a(lo),
                fmt_a(top)
            ),
        ),
    }
}

/// The device must not trip during the start: its fastest trip at the
/// starting current has to be later than the starting time.
fn judge_motor(device: &TripCurve, start: &MotorStart) -> (Status, String) {
    let assumed = match (start.ratio_assumed, start.time_assumed) {
        (true, true) => " (starting current and time assumed)",
        (true, false) => " (starting current assumed)",
        (false, true) => " (starting time assumed)",
        (false, false) => "",
    };
    let (status, mut text) = match device.trip_time_range(start.start_a) {
        None => (
            Status::Pass,
            format!(
                "The device does not trip at the starting current of {}{assumed}.",
                fmt_a(start.start_a)
            ),
        ),
        Some((_, t_max)) if t_max < start.time_s => (
            Status::Fail,
            format!(
                "The device trips within {} at {}, before a start of {} is over{assumed}.",
                fmt_s(t_max),
                fmt_a(start.start_a),
                fmt_s(start.time_s)
            ),
        ),
        Some((t_min, _)) if t_min <= start.time_s => (
            Status::Warning,
            format!(
                "The device may trip after {} at {}, during a start of {}{assumed}.",
                fmt_s(t_min),
                fmt_a(start.start_a),
                fmt_s(start.time_s)
            ),
        ),
        Some((t_min, _)) => (
            Status::Pass,
            format!(
                "The device holds for at least {} at {}, longer than the {} start{assumed}.",
                fmt_s(t_min),
                fmt_a(start.start_a),
                fmt_s(start.time_s)
            ),
        ),
    };
    if status == Status::Pass && (start.ratio_assumed || start.time_assumed) {
        // A pass on assumed numbers is still a pass, but say so.
        text.push_str(" Enter the motor's own values to confirm.");
    }
    (status, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protection::curves::{McbCurve, RelayClass};

    fn cable(section: f64, k: f64, ik: Option<f64>) -> CableCurve {
        CableCurve {
            device: 0,
            label: "W1".into(),
            section_mm2: section,
            section_origin: Origin::Stated,
            k,
            i2t: k * k * section * section,
            band: TripCurve::cable_withstand(section, k, 1.0).unwrap(),
            ik_a: ik,
            status: Status::Pass,
            text: String::new(),
        }
    }

    fn motor(rated: f64, ratio: f64, time: f64) -> MotorStart {
        MotorStart {
            tag: "-M1".into(),
            rated_a: rated,
            start_a: rated * ratio,
            time_s: time,
            ratio_assumed: false,
            time_assumed: false,
            devices: vec![0],
        }
    }

    #[test]
    fn cable_curve_is_k2s2_over_i2() {
        let c = cable(2.5, 143.0, None);
        let t = c.time_at(1000.0).unwrap();
        assert!((t - 143.0f64.powi(2) * 6.25 / 1.0e6).abs() < 1e-9);
        // Outside the adiabatic range there is no value.
        assert!(c.time_at(10.0).is_none());
        assert!(c.time_at(1.0e6).is_none());
        let (lo, hi) = c.range_a();
        assert!(lo < hi);
    }

    #[test]
    fn a_c16_protects_2_5_mm2_but_a_gg63_does_not() {
        let c16 = TripCurve::mcb(16.0, McbCurve::C).unwrap();
        let (status, _) = judge_cable(&c16, &cable(2.5, 143.0, Some(10_000.0)));
        assert_eq!(status, Status::Pass);
        let gg63 = TripCurve::fuse_gg(63.0, None).unwrap();
        let (status, text) = judge_cable(&gg63, &cable(1.5, 115.0, Some(10_000.0)));
        assert_ne!(status, Status::Pass, "{text}");
    }

    #[test]
    fn a_small_fault_level_stays_below_the_cable_curve() {
        let c16 = TripCurve::mcb(16.0, McbCurve::C).unwrap();
        // 1.5 mm2 PVC starts at about 77 A; a 50 A fault level never gets there.
        let (status, text) = judge_cable(&c16, &cable(1.5, 115.0, Some(50.0)));
        assert_eq!(status, Status::Pass);
        assert!(text.contains("below"));
    }

    #[test]
    fn a_motor_breaker_holds_through_a_normal_start() {
        let mpcb = TripCurve::mpcb(10.0, RelayClass::C10, 13.0).unwrap();
        let (status, _) = judge_motor(&mpcb, &motor(10.0, 7.0, 3.0));
        assert_eq!(status, Status::Pass);
    }

    #[test]
    fn a_class_10_relay_trips_in_a_long_start() {
        let relay = TripCurve::overload_relay(10.0, RelayClass::C10).unwrap();
        // Seven times for 30 s: far longer than a class 10 relay allows.
        let (status, text) = judge_motor(&relay, &motor(10.0, 7.0, 30.0));
        assert_eq!(status, Status::Fail, "{text}");
        let (short, _) = judge_motor(&relay, &motor(10.0, 7.0, 1.0));
        assert_eq!(short, Status::Pass);
    }

    #[test]
    fn assumed_start_values_are_said() {
        let relay = TripCurve::overload_relay(10.0, RelayClass::C10).unwrap();
        let mut m = motor(10.0, 7.0, 1.0);
        m.ratio_assumed = true;
        let (_, text) = judge_motor(&relay, &m);
        assert!(text.contains("assumed"), "{text}");
    }

    #[test]
    fn start_outline_is_a_block() {
        let o = motor(10.0, 7.0, 4.0).outline();
        assert_eq!(o[0].0, 70.0);
        assert_eq!(o[1], (70.0, 4.0));
        assert_eq!(o[2], (10.0, 4.0));
    }
}
