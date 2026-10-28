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
    /// Frequency converter (variable frequency drive).
    Drive,
    SoftStarter,
    /// Power supply or converter that makes a control voltage.
    PowerSupply,
    /// PLC central unit.
    PlcCpu,
    /// PLC input, output or remote I/O module.
    PlcIo,
    /// Ethernet switch.
    NetSwitch,
    /// A network or fieldbus port of a device. Never a part of its own.
    NetPort,
    /// Axial or radial fan.
    Fan,
    /// Cabinet vent, louvre or filter.
    Vent,
    /// Switchgear cabinet or enclosure.
    Enclosure,
    /// Main switch, switch-disconnector or isolator.
    Disconnector,
    /// Pilot lamp or indicator light.
    Lamp,
}

impl Role {
    /// The label with its article: "an overload relay", "a fan".
    pub fn with_article(self) -> String {
        let label = self.label();
        let an = label.starts_with(['a', 'e', 'i', 'o', 'u']);
        format!("{} {label}", if an { "an" } else { "a" })
    }

    /// Words for the kind, for reasons shown to people.
    pub fn label(self) -> &'static str {
        match self {
            Role::Contactor => "contactor",
            Role::Relay => "relay",
            Role::Mcb => "miniature circuit breaker",
            Role::Mpcb => "motor protection breaker",
            Role::Overload => "overload relay",
            Role::FuseHolder => "fuse holder",
            Role::PushNo | Role::PushNc | Role::PushBoth => "push button",
            Role::Motor => "motor",
            Role::Drive => "frequency converter",
            Role::SoftStarter => "soft starter",
            Role::PowerSupply => "power supply",
            Role::PlcCpu => "PLC CPU",
            Role::PlcIo => "PLC I/O module",
            Role::NetSwitch => "Ethernet switch",
            Role::NetPort => "network port",
            Role::Fan => "fan",
            Role::Vent => "vent",
            Role::Enclosure => "enclosure",
            Role::Disconnector => "main switch",
            Role::Lamp => "pilot lamp",
        }
    }

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
            // A soft starter also starts a motor, but is not a converter.
            (Drive, SoftStarter) | (SoftStarter, Drive) => 10,
            _ => 0,
        }
    }
}

/// The kind of a catalog part, from its stated function, keywords and
/// description. `None` when nothing says (the part then neither wins nor
/// loses on kind).
pub fn part_role(part: &CatalogPart) -> Option<Role> {
    // The explicit fields win over description prose: "Circuit breaker, overload
    // and short-circuit protection" is not an overload relay.
    role_of_text(&part.function)
        .or_else(|| role_of_text(&part.keywords.join(" ")))
        .or_else(|| role_of_text(&part.description))
}

fn role_of_text(text: &str) -> Option<Role> {
    let text = text.to_lowercase();
    let has = |w: &str| text.contains(w);
    let word = |w: &str| words(&text).any(|t| t == w);
    // The kinds a letter code cannot tell apart come first, and the
    // narrow phrases before the broad ones: a part is named by what it is
    // ("Ethernet switch for PLC panels" is a switch, "PLC I/O module" a
    // module), so the PLC words are tried after the others.
    if has("frequency converter")
        || has("frequency inverter")
        || has("variable frequency")
        || has("variable speed")
        || has("ac drive")
        || word("vfd")
        || word("vsd")
    {
        Some(Role::Drive)
    } else if has("soft starter") || has("soft-starter") || has("softstarter") {
        Some(Role::SoftStarter)
    } else if has("power supply") || has("switch mode") || has("switched-mode") || word("psu") {
        Some(Role::PowerSupply)
    } else if has("ethernet switch") || has("network switch") || has("managed switch") {
        Some(Role::NetSwitch)
    } else if has("pilot lamp")
        || has("pilot light")
        || has("indicator light")
        || has("signal lamp")
        || has("signalling lamp")
    {
        Some(Role::Lamp)
    } else if has("i/o module")
        || has("io module")
        || has("input module")
        || has("output module")
        || has("signal module")
        || has("remote i/o")
    {
        Some(Role::PlcIo)
    } else if word("plc") || has("programmable logic") || has("plc cpu") || word("cpu") {
        Some(Role::PlcCpu)
    } else if has("enclosure") || has("control cabinet") || has("switchgear cabinet") {
        Some(Role::Enclosure)
    } else if has("louvre")
        || has("louver")
        || has("ventilation grille")
        || has("exhaust filter")
        || has("filter mat")
        || word("vent")
    {
        Some(Role::Vent)
    } else if word("fan") && !word("motor") {
        Some(Role::Fan)
    } else if has("motor protection") || has("motor circuit breaker") || has("mpcb") {
        Some(Role::Mpcb)
    } else if has("miniature circuit") || has("mcb") {
        Some(Role::Mcb)
    } else if has("overload") {
        Some(Role::Overload)
    } else if has("fuse holder") || has("fuse-holder") || has("fuse switch") {
        Some(Role::FuseHolder)
    } else if has("main switch")
        || has("switch-disconnector")
        || has("switch disconnector")
        || has("disconnector")
        || has("isolator")
    {
        Some(Role::Disconnector)
    } else if has("contactor relay") || has("relay") {
        Some(Role::Relay)
    } else if has("contactor") {
        Some(Role::Contactor)
    } else if has("push button") || has("pushbutton") {
        let no = has("1 no") || has("1no") || has(" no,") || text.ends_with(" no");
        let nc = has("1 nc") || has("1nc") || has(" nc,") || text.ends_with(" nc");
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

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
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
        "converter" => match parts.get(1).copied() {
            Some("vfd") => Some(Role::Drive),
            Some("soft_starter") => Some(Role::SoftStarter),
            Some("ac_dc" | "dc_dc") => Some(Role::PowerSupply),
            _ => None,
        },
        "plc" => match parts.get(1).copied() {
            Some("cpu") => Some(Role::PlcCpu),
            Some("supply") => Some(Role::PowerSupply),
            Some("remote_io" | "input" | "output") => Some(Role::PlcIo),
            _ => None,
        },
        "network" => match parts.get(1).copied() {
            Some("switch") => Some(Role::NetSwitch),
            Some("port") => Some(Role::NetPort),
            _ => None,
        },
        "switch" if has("main") || has("disconnector") || has("switch_disconnector") => {
            Some(Role::Disconnector)
        }
        "signal" if has("lamp") => Some(Role::Lamp),
        "motor" if has("fan") => Some(Role::Fan),
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
    /// A part's coil can take both (`24 V AC/DC`); a circuit has one kind.
    pub dc: bool,
}

impl Supply {
    fn ac(volts: f64) -> Self {
        Supply {
            volts,
            ac: true,
            dc: false,
        }
    }

    fn dc(volts: f64) -> Self {
        Supply {
            volts,
            ac: false,
            dc: true,
        }
    }

    /// Whether a coil rated `self` runs on a circuit of `circuit`.
    fn runs_on(self, circuit: Supply) -> bool {
        (self.volts - circuit.volts).abs() < 0.5
            && ((circuit.ac && self.ac) || (circuit.dc && self.dc))
    }
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
    // Whole words only ("24 V DC black" must not read as AC), allowing the
    // unit fused to the number ("24VDC", "50Hz").
    let word = |w: &str| {
        lower.split(|c: char| !c.is_ascii_alphanumeric()).any(|t| {
            t.strip_suffix(w)
                .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit() || c == 'v'))
        })
    };
    let dc = word("dc") || lower.contains('=');
    let ac = word("ac") || word("hz") || lower.contains('~');
    if !ac && !dc {
        return None;
    }
    Some(Supply { volts, ac, dc })
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
        (Line, Neutral | Pen) => va.map(|v| Supply::ac(phase(a, v))),
        (Neutral | Pen, Line) => vb.map(|v| Supply::ac(phase(b, v))),
        (Line, Line) if a.id != b.id => va.zip(vb).map(|(x, y)| Supply::ac(x.max(y))),
        (DcPositive, DcNegative) => va.map(Supply::dc),
        (DcNegative, DcPositive) => vb.map(Supply::dc),
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
    /// How many checks the percentage rests on (1 to 4).
    pub checks: u8,
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
    /// How many poles the device draws (switched paths of its symbol), for
    /// the kinds where a part's pole count must agree: breakers and fuse
    /// holders. `None` when the device is another kind or has no symbol.
    pub poles: Option<u32>,
}

/// The pole count of a breaker or fuse holder: the most poles any of its
/// symbols draws, from the library symbol's behaviour (the instance's
/// switched paths when the library does not know the symbol).
fn device_poles(project: &Project, device: DeviceId, role: Option<Role>) -> Option<u32> {
    if !matches!(role, Some(Role::Mcb | Role::Mpcb | Role::FuseHolder)) {
        return None;
    }
    project
        .symbols_of_device(device)
        .into_iter()
        .map(|s| {
            [Pack::Iec, Pack::Nfpa]
                .into_iter()
                .find_map(|pack| Library::shared(pack).get(&s.symbol.0))
                .map_or(s.pass_through.len(), |lib| lib.behavior.poles().len()) as u32
        })
        .filter(|&n| n > 0)
        .max()
}

/// The pole count a part states in its data (`poles`), as a number or text.
fn part_poles(part: &CatalogPart) -> Option<u32> {
    match part.data.get("poles")? {
        Value::Int(n) => u32::try_from(*n).ok(),
        Value::Float(f) if *f >= 0.0 && f.fract() == 0.0 => Some(*f as u32),
        Value::Text(t) => t.trim().parse().ok(),
        _ => None,
    }
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
            poles: device_poles(project, device.id, role),
        }
    }
}

/// The match of `part` for a device, as the share of points earned over
/// the checks that could be made: kind (60), letter code (10), coil voltage
/// (20, only for a device with a coil whose circuit voltage is known), poles
/// (20, only for a breaker or fuse holder whose symbol draws them) and
/// rating (10, only when the device states a sizing value). A check with
/// nothing to compare is not counted, and the tooltip says which. A part
/// whose data does not say what it is earns nothing for kind, and half for
/// a coil voltage it does not state, so a part that cannot be confirmed
/// never reaches the score of one that matches.
pub fn fit(part: &CatalogPart, need: &DeviceNeed, device: &Device) -> Fit {
    let mut reasons = Vec::new();
    let (mut earned, mut possible) = (0u32, 0u32);
    let mut checks = 0u8;

    match (need.role, part_role(part)) {
        (Some(want), Some(have)) => {
            let s = have.score_for(want);
            possible += 60;
            checks += 1;
            earned += s;
            reasons.push(
                match s {
                    60 => "kind: right",
                    0 => "kind: a different kind of device",
                    _ => "kind: similar",
                }
                .to_owned(),
            );
        }
        (Some(_), None) => {
            possible += 60;
            checks += 1;
            reasons.push("kind: the part does not say what it is".to_owned());
        }
        (None, _) => reasons.push("kind: not checked, the device's kind is not known".to_owned()),
    }

    possible += 10;
    checks += 1;
    match part
        .letter_codes
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&need.letter_code))
    {
        Some(0) => {
            earned += 10;
            reasons.push("letter code: main use".to_owned());
        }
        Some(_) => {
            earned += 6;
            reasons.push("letter code: secondary use".to_owned());
        }
        None if part.letter_codes.is_empty() => {
            earned += 3;
            reasons.push("letter code: not stated".to_owned());
        }
        None => reasons.push("letter code: does not fit".to_owned()),
    }

    if let Some(want) = need.coil {
        possible += 20;
        checks += 1;
        let name = |s: Supply| if s.ac { "AC" } else { "DC" };
        let have = part.data.get("coil_voltage").and_then(|v| match v {
            Value::Text(t) => parse_supply(t),
            _ => None,
        });
        match have {
            Some(h) if h.runs_on(want) => {
                earned += 20;
                reasons.push(format!(
                    "coil: {} V {} matches the circuit",
                    want.volts,
                    name(want)
                ));
            }
            Some(h) => reasons.push(format!(
                "coil: {} V, the circuit has {} V {}",
                h.volts,
                want.volts,
                name(want)
            )),
            None => {
                earned += 10;
                reasons.push("coil: voltage not stated".to_owned());
            }
        }
    } else {
        reasons.push("coil: not checked, no coil voltage known".to_owned());
    }

    if let Some(want) = need.poles {
        possible += 20;
        checks += 1;
        match part_poles(part) {
            Some(have) if have == want => {
                earned += 20;
                reasons.push(format!("poles: {want} match the device"));
            }
            Some(have) => reasons.push(format!(
                "poles: the part has {have}, the device draws {want}"
            )),
            None => {
                earned += 10;
                reasons.push("poles: not stated".to_owned());
            }
        }
    } else {
        reasons.push("poles: not checked, the device's pole count is not known".to_owned());
    }

    let sized = device
        .properties
        .iter()
        .any(|(_, v)| matches!(v, Value::Int(_) | Value::Float(_)));
    if sized {
        possible += 10;
        checks += 1;
        if crate::assign::sized_for(part, device) {
            earned += 10;
            reasons.push("rating: matches".to_owned());
        } else {
            reasons.push("rating: does not match the device".to_owned());
        }
    } else {
        reasons.push("rating: not checked, the device states none".to_owned());
    }

    Fit {
        percent: (earned * 100 / possible) as u8,
        checks,
        reasons,
    }
}

/// What the kinds of a device and a part say about each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindCheck {
    /// Both state a kind and it is the same.
    Right,
    /// Both state a kind; the kinds are related but not the same.
    Similar { part: Role, device: Role },
    /// Both state a kind and they cannot stand in for each other.
    Wrong { part: Role, device: Role },
    /// The device states a kind, the part does not say what it is.
    PartSilent { device: Role },
    /// The part states a kind, the device's symbol does not.
    DeviceSilent { part: Role },
    /// Neither says; only the letter code and the sizing can be compared.
    BothSilent,
}

/// What a proposal reads from a part: its kind, pole count and coil
/// voltage. Each takes a text scan, so a batch of proposals computes them
/// once per part ([`PartFacts::of`]), not once per device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartFacts {
    pub role: Option<Role>,
    pub poles: Option<u32>,
    pub coil: Option<Supply>,
}

impl PartFacts {
    pub fn of(part: &CatalogPart) -> Self {
        PartFacts {
            role: part_role(part),
            poles: part_poles(part),
            coil: part.data.get("coil_voltage").and_then(|v| match v {
                Value::Text(t) => parse_supply(t),
                _ => None,
            }),
        }
    }
}

/// Compares the kinds of a part and a device.
pub fn check_kind(part: &CatalogPart, need: &DeviceNeed) -> KindCheck {
    check_kind_with(&PartFacts::of(part), need)
}

/// [`check_kind`] with the part's facts already read.
pub fn check_kind_with(part: &PartFacts, need: &DeviceNeed) -> KindCheck {
    match (need.role, part.role) {
        (Some(device), Some(have)) => match have.score_for(device) {
            60 => KindCheck::Right,
            0 => KindCheck::Wrong { part: have, device },
            _ => KindCheck::Similar { part: have, device },
        },
        (Some(device), None) => KindCheck::PartSilent { device },
        (None, Some(part)) => KindCheck::DeviceSilent { part },
        (None, None) => KindCheck::BothSilent,
    }
}

/// The first thing that rules `part` out for a device, in words: a
/// different kind of device, a coil for another voltage or another number
/// of poles. A value the part does not state never rules it out.
pub fn conflict(part: &CatalogPart, need: &DeviceNeed) -> Option<String> {
    conflict_with(&PartFacts::of(part), need)
}

/// [`conflict`] with the part's facts already read.
pub fn conflict_with(part: &PartFacts, need: &DeviceNeed) -> Option<String> {
    if let KindCheck::Wrong { part: have, device } = check_kind_with(part, need) {
        return Some(format!(
            "the part is {}, the device is {}",
            have.with_article(),
            device.with_article()
        ));
    }
    if let (Some(want), Some(have)) = (need.coil, part.coil)
        && !have.runs_on(want)
    {
        return Some(format!(
            "its coil takes {} V, the circuit has {} V",
            have.volts, want.volts
        ));
    }
    if let (Some(want), Some(have)) = (need.poles, part.poles)
        && want != have
    {
        return Some(format!("it has {have} poles, the device draws {want}"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supply_text_is_read() {
        assert_eq!(parse_supply("24 V DC"), Some(Supply::dc(24.0)));
        assert_eq!(parse_supply("230 V AC"), Some(Supply::ac(230.0)));
        assert_eq!(parse_supply("24"), None);
        for text in ["24 V DC black", "24 V DC contactor", "24VDC, face"] {
            let s = parse_supply(text).unwrap();
            assert!(s.dc && !s.ac, "{text}");
        }
        assert_eq!(parse_supply("230 V 50Hz"), Some(Supply::ac(230.0)));
    }

    #[test]
    fn an_ac_dc_coil_runs_on_either() {
        let both = parse_supply("24 V AC/DC").unwrap();
        assert!(both.runs_on(Supply::ac(24.0)));
        assert!(both.runs_on(Supply::dc(24.0)));
        assert!(!both.runs_on(Supply::dc(48.0)));
        assert!(!parse_supply("24 V DC").unwrap().runs_on(Supply::ac(24.0)));
    }

    fn part(description: &str, keywords: &[&str]) -> CatalogPart {
        let mut p = CatalogPart::new("Acme", "1");
        p.description = description.to_owned();
        p.keywords = keywords.iter().map(|k| k.to_string()).collect();
        p
    }

    #[test]
    fn keywords_win_over_description_prose() {
        let p = part(
            "Circuit breaker 1P C16, overload and short-circuit protection",
            &["mcb"],
        );
        assert_eq!(part_role(&p), Some(Role::Mcb));
        let p = part(
            "Circuit breaker 1P C16, overload and short-circuit protection",
            &[],
        );
        assert_eq!(part_role(&p), Some(Role::Overload), "prose only: a guess");
    }

    #[test]
    fn phase_voltage_snaps_to_the_common_value() {
        assert_eq!(nominal(400.0 / 3f64.sqrt()), 230.0);
        assert_eq!(nominal(133.0), 133.0);
    }
}
