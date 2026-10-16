//! How well a catalog part suits a device: its kind, the coil voltage of the
//! circuit it sits in and the device's own sizing values.
//!
//! A letter code is too coarse for this: `F` is an MCB, an overload relay
//! and a fuse holder alike, `S` a green NO and a red NC button. The kind of
//! the device comes from the function of its symbol (`protection.mcb`,
//! `contact.nc.push`), the kind of the part from its `function`, keywords
//! and description. The score is a planning aid, not a verdict: it says how
//! many of the things that can be checked from the data agree.

use novo_core::{Device, DeviceId, Endpoint, Potential, PotentialKind, Project, Value};
use novo_symbols::{Library, Pack};

use crate::catalog::CatalogPart;

/// What a device or a part is, at the granularity that decides whether the
/// part can stand in for the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Contactor,
    Relay,
    /// Miniature circuit breaker.
    Mcb,
    /// Motor protection circuit breaker.
    Mpcb,
    Overload,
    FuseHolder,
    /// Push button with a make contact.
    PushNo,
    /// Push button with a break contact.
    PushNc,
    /// Push button with both kinds of contact.
    PushBoth,
    Motor,
}

impl Role {
    /// How well a part of this role fills a device that needs `need`, from
    /// 0 (wrong kind) to 60.
    fn score_for(self, need: Role) -> u32 {
        use Role::*;
        if self == need {
            return 60;
        }
        match (need, self) {
            (PushNo | PushNc, PushBoth) => 35,
            // A contactor relay can switch a coil's load, but not a motor.
            (Contactor, Relay) | (Relay, Contactor) => 15,
            // A motor protection breaker also protects, but is not an MCB.
            (Mcb, Mpcb) | (Mpcb, Mcb) => 10,
            _ => 0,
        }
    }
}

/// The kind of a catalog part, from its stated function, keywords and
/// description. `None` when nothing says (the part then neither wins nor
/// loses on kind).
pub fn part_role(part: &CatalogPart) -> Option<Role> {
    let text = format!(
        "{} {} {}",
        part.function,
        part.keywords.join(" "),
        part.description
    )
    .to_lowercase();
    let has = |w: &str| text.contains(w);
    if has("motor protection") || has("motor circuit breaker") || has("mpcb") {
        Some(Role::Mpcb)
    } else if has("miniature circuit") || has("mcb") {
        Some(Role::Mcb)
    } else if has("overload") {
        Some(Role::Overload)
    } else if has("fuse holder") || has("fuse-holder") || has("fuse switch") {
        Some(Role::FuseHolder)
    } else if has("contactor relay") || has("relay") {
        Some(Role::Relay)
    } else if has("contactor") {
        Some(Role::Contactor)
    } else if has("push button") || has("pushbutton") {
        let no = has("1 no") || has("1no") || text.contains(" no,") || text.ends_with(" no");
        let nc = has("1 nc") || has("1nc") || text.contains(" nc,") || text.ends_with(" nc");
        Some(match (no, nc) {
            (true, false) => Role::PushNo,
            (false, true) => Role::PushNc,
            _ => Role::PushBoth,
        })
    } else if has("induction motor") || has("three phase motor") {
        Some(Role::Motor)
    } else {
        None
    }
}

/// The kind of a device, from the function of the symbol that draws it.
/// `None` for devices whose symbols say nothing the catalog can use
/// (terminals, sensors, signals), which are then ranked by letter code only.
pub fn device_role(project: &Project, device: DeviceId) -> Option<Role> {
    let mut roles = project
        .symbols_of_device(device)
        .into_iter()
        .filter_map(|symbol| {
            let function = [Pack::Iec, Pack::Nfpa]
                .into_iter()
                .find_map(|pack| Library::shared(pack).get(&symbol.symbol.0))?
                .function
                .clone();
            role_of_function(&function)
        });
    // Main contacts make a contactor of a device whose coil is drawn as a
    // plain relay coil; the first role stands otherwise.
    let first = roles.next()?;
    Some(
        if first == Role::Relay && roles.any(|r| r == Role::Contactor) {
            Role::Contactor
        } else {
            first
        },
    )
}

fn role_of_function(function: &str) -> Option<Role> {
    let parts: Vec<&str> = function.split('.').collect();
    let has = |p: &str| parts.contains(&p);
    match parts[0] {
        "protection" => match parts.get(1).copied() {
            Some("mcb" | "rcbo") => Some(Role::Mcb),
            Some("mpcb" | "mcp") => Some(Role::Mpcb),
            Some("overload") => Some(Role::Overload),
            Some("fuse" | "fuse_switch") => Some(Role::FuseHolder),
            _ => None,
        },
        "contact" if has("thermal") => Some(Role::Overload),
        "contact" if has("main") => Some(Role::Contactor),
        "contact"
            if ["push", "key", "turn", "estop", "foot", "pull"]
                .iter()
                .any(|p| has(p)) =>
        {
            if has("nc") || has("estop") {
                Some(Role::PushNc)
            } else if has("no") {
                Some(Role::PushNo)
            } else {
                None
            }
        }
        "coil" if has("contactor") => Some(Role::Contactor),
        "coil" if has("valve") || has("brake") || has("solenoid") => None,
        "coil" => Some(Role::Relay),
        "motor" => Some(Role::Motor),
        _ => None,
    }
}

/// A control voltage: volts and whether it is alternating.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Supply {
    pub volts: f64,
    pub ac: bool,
}

/// `24 V DC` or `230 V AC` (also `24VDC`, `230 V 50 Hz`) as a supply.
pub fn parse_supply(text: &str) -> Option<Supply> {
    let lower = text.to_lowercase();
    let digits: String = lower
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let volts: f64 = digits.parse().ok()?;
    let ac = if lower.contains("dc") || lower.contains('=') {
        false
    } else if lower.contains("ac") || lower.contains("hz") || lower.contains('~') {
        true
    } else {
        return None;
    };
    Some(Supply { volts, ac })
}

/// The voltage a relay or contactor coil of `device` sees, when it can be
/// told: the device's own `coil_voltage` value, otherwise the potentials on
/// the coil's two pins (line and neutral give the phase voltage, two lines
/// the line voltage, a DC pair its voltage).
pub fn coil_supply(project: &Project, device: &Device) -> Option<Supply> {
    if let Some(Value::Text(text)) = device.properties.get("coil_voltage") {
        return parse_supply(text);
    }
    let symbol = project
        .symbols_of_device(device.id)
        .into_iter()
        .find(|s| s.pins.iter().any(|p| p.name == "A1"))?;
    let netlist = project.netlist();
    let mut found: Vec<(&Potential, Option<f64>)> = Vec::new();
    for pin in 0..symbol.pins.len().min(2) {
        let end = Endpoint::Pin {
            symbol: symbol.id,
            pin: pin as u16,
        };
        let net = netlist.net_of(end)?;
        let potential = netlist
            .net(net)
            .potentials
            .iter()
            .filter_map(|p| project.potential(*p))
            .find(|p| p.kind.is_supply())?;
        found.push((potential, potential.voltage));
    }
    let [(a, va), (b, vb)] = found[..] else {
        return None;
    };
    use PotentialKind::*;
    // `L1`..`L3` carry the line-to-line voltage of their system; a single
    // `L` carries its own.
    let phase = |p: &Potential, v: f64| {
        if matches!(p.name.as_str(), "L1" | "L2" | "L3") {
            nominal(v / 3f64.sqrt())
        } else {
            v
        }
    };
    match (a.kind, b.kind) {
        (Line, Neutral | Pen) => va.map(|v| Supply {
            volts: phase(a, v),
            ac: true,
        }),
        (Neutral | Pen, Line) => vb.map(|v| Supply {
            volts: phase(b, v),
            ac: true,
        }),
        (Line, Line) if a.id != b.id => va.zip(vb).map(|(x, y)| Supply {
            volts: x.max(y),
            ac: true,
        }),
        (DcPositive, DcNegative) => va.map(|v| Supply {
            volts: v,
            ac: false,
        }),
        (DcNegative, DcPositive) => vb.map(|v| Supply {
            volts: v,
            ac: false,
        }),
        _ => None,
    }
}

/// The nearest common mains voltage (400 V line-to-line gives 230 V phase,
/// not 230.94).
fn nominal(volts: f64) -> f64 {
    const COMMON: [f64; 6] = [24.0, 48.0, 110.0, 120.0, 230.0, 240.0];
    COMMON
        .into_iter()
        .find(|c| (c - volts).abs() / c < 0.02)
        .unwrap_or(volts.round())
}

/// What a part is worth for a device, in whole percent, and what the
/// number rests on.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    pub percent: u8,
    /// One short phrase per thing that was checked, for the tooltip.
    pub reasons: Vec<String>,
}

impl Fit {
    /// A short word for the number.
    pub fn label(&self) -> &'static str {
        match self.percent {
            80.. => "Strong",
            55..=79 => "Fair",
            _ => "Weak",
        }
    }
}

/// Everything about a device that decides which parts suit it.
#[derive(Debug, Clone, Default)]
pub struct DeviceNeed {
    pub letter_code: String,
    pub role: Option<Role>,
    pub coil: Option<Supply>,
}

impl DeviceNeed {
    pub fn of(project: &Project, device: &Device) -> Self {
        let role = device_role(project, device.id);
        let coil = matches!(role, Some(Role::Contactor | Role::Relay))
            .then(|| coil_supply(project, device))
            .flatten();
        DeviceNeed {
            letter_code: device.tag.letter_code().to_owned(),
            role,
            coil,
        }
    }
}

/// The match of `part` for a device: kind (60), letter code (10), coil
/// voltage (20) and the device's own sizing values (10). A check that
/// cannot be made because the data is missing gives half its points, so a
/// part is never rewarded for silence over a match nor punished like a
/// mismatch.
pub fn fit(part: &CatalogPart, need: &DeviceNeed, device: &Device) -> Fit {
    let mut reasons = Vec::new();
    let mut points = 0u32;

    match (need.role, part_role(part)) {
        (Some(want), Some(have)) => {
            let s = have.score_for(want);
            points += s;
            reasons.push(if s == 60 {
                "right kind of device".to_owned()
            } else if s > 0 {
                "similar kind of device".to_owned()
            } else {
                "a different kind of device".to_owned()
            });
        }
        _ => {
            points += 30;
            reasons.push("kind of device not stated".to_owned());
        }
    }

    match part
        .letter_codes
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&need.letter_code))
    {
        Some(0) => points += 10,
        Some(_) => points += 6,
        None if part.letter_codes.is_empty() => points += 3,
        None => {}
    }

    if let Some(want) = need.coil {
        let have = part.data.get("coil_voltage").and_then(|v| match v {
            Value::Text(t) => parse_supply(t),
            _ => None,
        });
        match have {
            Some(h) if h.ac == want.ac && (h.volts - want.volts).abs() < 0.5 => {
                points += 20;
                reasons.push(format!(
                    "coil {} V {} matches the circuit",
                    want.volts,
                    if want.ac { "AC" } else { "DC" }
                ));
            }
            Some(h) => reasons.push(format!(
                "coil {} V {}, the circuit has {} V {}",
                h.volts,
                if h.ac { "AC" } else { "DC" },
                want.volts,
                if want.ac { "AC" } else { "DC" }
            )),
            None => {
                points += 10;
                reasons.push("coil voltage not stated".to_owned());
            }
        }
    } else {
        points += 20;
    }

    if crate::assign::sized_for(part, device) {
        points += 10;
    } else {
        reasons.push("does not match the device's rating".to_owned());
    }

    Fit {
        percent: points.min(100) as u8,
        reasons,
    }
}

/// Whether the part is of a plainly different kind than the device. Used to
/// hide such parts when something better exists.
pub fn is_wrong_kind(part: &CatalogPart, need: &DeviceNeed) -> bool {
    matches!(
        (need.role, part_role(part)),
        (Some(want), Some(have)) if have.score_for(want) == 0
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supply_text_is_read() {
        assert_eq!(
            parse_supply("24 V DC"),
            Some(Supply {
                volts: 24.0,
                ac: false
            })
        );
        assert_eq!(
            parse_supply("230 V AC"),
            Some(Supply {
                volts: 230.0,
                ac: true
            })
        );
        assert_eq!(parse_supply("24"), None);
    }

    #[test]
    fn phase_voltage_snaps_to_the_common_value() {
        assert_eq!(nominal(400.0 / 3f64.sqrt()), 230.0);
        assert_eq!(nominal(133.0), 133.0);
    }
}
