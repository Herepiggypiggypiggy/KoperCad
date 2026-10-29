//! Pin templates per device class: the standard connection designations a
//! device of a function type normally has (coil `A1 A2`, motor `U1 V1 W1`,
//! motor brake `BD1 BD2`, breaker poles `1 2 3 4 5 6`).
//!
//! A template lists one or more [`PinSet`]s, each a valid way to name the
//! pins of that class (a motor with three terminals, with six, a
//! pole-changing motor). Names are strings, never numbers: `1U`, `BD1`,
//! `1L1` and `A1` all occur. The names follow the generic IEC conventions
//! (IEC 61666 / EN 50005 for contactors and relays, IEC 60034-8 for
//! machines, IEC 60947 for switching devices and IEC 60076 for
//! transformers). Contacts are not here: their numbering is checked per
//! contact (novo-checks `ContactType`).
//!
//! The templates are used to keep the built-in library consistent (a test
//! walks every symbol), to check pins on the sheet and the terminal names of
//! an assigned part (novo-checks `PinNames`) and to name the right pins when
//! a deviation is reported.

use std::sync::OnceLock;

/// Other pin names a set accepts besides its listed ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extra {
    /// Contact numbers: two digits, the first the position, the second 1 to 8
    /// (`13`, `24`, `41`).
    Contacts,
    /// A name that starts with this text and goes on with digits (`S11`,
    /// `Y2`).
    Series(&'static str),
}

impl Extra {
    fn accepts(self, name: &str) -> bool {
        match self {
            Extra::Contacts => {
                let b = name.as_bytes();
                b.len() == 2 && (b'1'..=b'9').contains(&b[0]) && (b'1'..=b'8').contains(&b[1])
            }
            Extra::Series(prefix) => name
                .strip_prefix(prefix)
                .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())),
        }
    }
}

/// One valid way to name the pins of a device class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinSet {
    /// Plain name of the variant, for messages (`six terminals`).
    pub name: String,
    /// Pins that must be there.
    pub required: Vec<String>,
    /// Pins that may be there.
    pub optional: Vec<String>,
    /// Further names accepted by pattern.
    pub extras: Vec<Extra>,
}

impl PinSet {
    fn new(name: &str, required: &[&str], optional: &[&str]) -> PinSet {
        PinSet {
            name: name.to_owned(),
            required: required.iter().map(|s| (*s).to_owned()).collect(),
            optional: optional.iter().map(|s| (*s).to_owned()).collect(),
            extras: Vec::new(),
        }
    }

    fn with_extras(mut self, extras: &[Extra]) -> PinSet {
        self.extras = extras.to_vec();
        self
    }

    fn knows(&self, name: &str) -> bool {
        self.required.iter().any(|r| r == name)
            || self.optional.iter().any(|o| o == name)
            || self.extras.iter().any(|e| e.accepts(name))
    }

    /// Whether `pins` is a valid naming under this set.
    pub fn accepts(&self, pins: &[&str]) -> bool {
        self.required.iter().all(|r| pins.contains(&r.as_str()))
            && pins.iter().all(|p| self.knows(p))
    }

    /// The pin names of the set in plain text: required, then optional in
    /// brackets (`U1 V1 W1 [PE]`).
    pub fn describe(&self) -> String {
        let mut parts: Vec<String> = self.required.clone();
        if !self.optional.is_empty() {
            parts.push(format!("[{}]", self.optional.join(" ")));
        }
        parts.join(" ")
    }
}

/// The pin names of one device class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinTemplate {
    /// Short id of the class (`coil`, `motor.ac3`).
    pub class: &'static str,
    /// Plain name for messages (`coil`, `three-phase motor`).
    pub label: &'static str,
    /// The symbol function types the template covers: the function itself
    /// and everything below it (`coil` covers `coil.relay`), unless a longer
    /// template matches.
    pub functions: Vec<&'static str>,
    /// Valid pin namings, preferred first.
    pub sets: Vec<PinSet>,
}

/// How a pin list differs from the template of its class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deviation {
    pub class: &'static str,
    pub label: &'static str,
    /// The set that is closest to the pins.
    pub closest: String,
    /// What that set looks like (`A1 A2`).
    pub closest_pins: String,
    /// Pins of the closest set that are missing.
    pub missing: Vec<String>,
    /// Pins the closest set does not know.
    pub unexpected: Vec<String>,
}

impl Deviation {
    /// One plain sentence: what is wrong and what the class normally uses.
    pub fn message(&self) -> String {
        let mut what = Vec::new();
        if !self.unexpected.is_empty() {
            what.push(format!("pin {} is not usual", self.unexpected.join(", ")));
        }
        if !self.missing.is_empty() {
            what.push(format!("pin {} is missing", self.missing.join(", ")));
        }
        format!(
            "{}: {}; a {} is usually {} ({})",
            self.label,
            what.join(" and "),
            self.label,
            self.closest_pins,
            self.closest
        )
    }
}

fn poles(max: usize, optional: &[&str]) -> Vec<PinSet> {
    let mut sets = Vec::new();
    for n in 1..=max {
        let plain: Vec<String> = (1..=2 * n).map(|i| i.to_string()).collect();
        let plain: Vec<&str> = plain.iter().map(String::as_str).collect();
        sets.push(PinSet::new(&format!("{n} pole"), &plain, optional));
    }
    for n in 1..=max {
        let marked: Vec<String> = (1..=n)
            .flat_map(|k| [format!("{}L{k}", 2 * k - 1), format!("{}T{k}", 2 * k)])
            .collect();
        let marked: Vec<&str> = marked.iter().map(String::as_str).collect();
        sets.push(PinSet::new(
            &format!("{n} pole, marked L and T"),
            &marked,
            optional,
        ));
    }
    sets
}

fn t(
    class: &'static str,
    label: &'static str,
    functions: &[&'static str],
    sets: Vec<PinSet>,
) -> PinTemplate {
    PinTemplate {
        class,
        label,
        functions: functions.to_vec(),
        sets,
    }
}

fn build() -> Vec<PinTemplate> {
    let motor_extra = ["PE", "T1", "T2", "BD1", "BD2"];
    vec![
        t(
            "coil.brake",
            "brake coil",
            &["coil.brake"],
            vec![
                PinSet::new("coil", &["A1", "A2"], &[]),
                PinSet::new("motor brake", &["BD1", "BD2"], &[]),
            ],
        ),
        t(
            "coil",
            "coil",
            &["coil"],
            vec![PinSet::new("coil", &["A1", "A2"], &["A3", "PE"])],
        ),
        t(
            "motor.ac3",
            "three-phase motor",
            &["motor.ac3"],
            vec![
                PinSet::new("three terminals", &["U1", "V1", "W1"], &motor_extra),
                PinSet::new(
                    "six terminals, star or delta",
                    &["U1", "V1", "W1", "U2", "V2", "W2"],
                    &motor_extra,
                ),
                PinSet::new(
                    "pole-changing, two windings",
                    &["1U", "1V", "1W", "2U", "2V", "2W"],
                    &motor_extra,
                ),
            ],
        ),
        t(
            "motor.ac1",
            "single-phase motor",
            &["motor.ac1"],
            vec![
                PinSet::new("main winding", &["U1", "U2"], &["PE"]),
                PinSet::new(
                    "main and auxiliary winding",
                    &["U1", "U2", "Z1", "Z2"],
                    &["PE"],
                ),
            ],
        ),
        t(
            "motor.dc",
            "DC motor",
            &["motor.dc"],
            vec![PinSet::new(
                "armature",
                &["A1", "A2"],
                &[
                    "B1", "B2", "C1", "C2", "D1", "D2", "E1", "E2", "F1", "F2", "PE",
                ],
            )],
        ),
        t(
            "contact.main",
            "main contact",
            &["contact.main"],
            poles(4, &[]),
        ),
        t(
            "protection.neutral",
            "breaker with neutral",
            &["protection.mcb", "protection.rcd", "protection.rcbo"],
            poles(4, &["N", "N'"]),
        ),
        t(
            "protection.overload",
            "overload relay",
            &["protection.overload"],
            poles(3, &["95", "96", "97", "98"]),
        ),
        t(
            "protection.poles",
            "protective device",
            &[
                "protection.fuse",
                "protection.fuse_switch",
                "protection.mpcb",
                "protection.mcp",
            ],
            poles(4, &[]),
        ),
        t(
            "switch.poles",
            "switch",
            &[
                "switch.disconnector",
                "switch.switch_disconnector",
                "switch.main",
            ],
            poles(4, &[]),
        ),
        t(
            "converter.vfd",
            "frequency drive",
            &["converter.vfd"],
            vec![PinSet::new(
                "mains and motor",
                &["L1", "L2", "L3", "U", "V", "W"],
                &["PE", "PE2", "DC+", "DC-"],
            )],
        ),
        t(
            "converter.soft_starter",
            "soft starter",
            &["converter.soft_starter"],
            vec![PinSet::new(
                "marked L and T",
                &["1L1", "3L2", "5L3", "2T1", "4T2", "6T3"],
                &["PE"],
            )],
        ),
        t(
            "converter.ac_dc",
            "power supply",
            &["converter.ac_dc"],
            vec![
                PinSet::new("single phase in, DC out", &["L", "N", "L+", "M"], &["PE"]),
                PinSet::new(
                    "three phase in, DC out",
                    &["L1", "L2", "L3", "L+", "M"],
                    &["N", "PE"],
                ),
            ],
        ),
        t(
            "transformer.1ph",
            "single-phase transformer",
            &["transformer.1ph"],
            vec![
                PinSet::new(
                    "primary 1U, secondary 2U",
                    &["1U1", "1U2", "2U1", "2U2"],
                    &["PE"],
                ),
                PinSet::new("winding 1 and 2", &["1.1", "1.2", "2.1", "2.2"], &["PE"]),
            ],
        ),
        t(
            "transformer.3ph",
            "three-phase transformer",
            &["transformer.3ph"],
            vec![PinSet::new(
                "primary 1, secondary 2",
                &["1U", "1V", "1W", "2U", "2V", "2W"],
                &["1N", "2N", "PE"],
            )],
        ),
        t(
            "transformer.current",
            "current transformer",
            &["transformer.current"],
            vec![PinSet::new(
                "primary P, secondary S",
                &["P1", "P2", "S1", "S2"],
                &[],
            )],
        ),
        t(
            "passive.diode",
            "diode",
            &["passive.diode"],
            vec![PinSet::new("anode, cathode", &["A", "K"], &[])],
        ),
        t(
            "passive.thyristor",
            "thyristor",
            &["passive.thyristor"],
            vec![PinSet::new("anode, cathode, gate", &["A", "K", "G"], &[])],
        ),
        t(
            "passive.rectifier",
            "rectifier",
            &["passive.rectifier"],
            vec![PinSet::new("bridge", &["~1", "~2", "+", "-"], &[])],
        ),
        t(
            "sensor.proximity",
            "three-wire sensor",
            &[
                "sensor.inductive",
                "sensor.capacitive",
                "sensor.photoelectric",
            ],
            vec![PinSet::new("supply and output", &["1", "3", "4"], &["2"])],
        ),
        t(
            "signal.lamp",
            "lamp",
            &["signal.lamp", "signal.beacon"],
            vec![PinSet::new("lamp", &["X1", "X2"], &[])],
        ),
        t(
            "safety.relay",
            "safety relay",
            &["safety.relay"],
            vec![
                PinSet::new("supply and contacts", &["A1", "A2"], &[]).with_extras(&[
                    Extra::Contacts,
                    Extra::Series("S"),
                    Extra::Series("Y"),
                    Extra::Series("X"),
                ]),
            ],
        ),
    ]
}

/// Every template, in the order they are listed here.
pub fn templates() -> &'static [PinTemplate] {
    static ALL: OnceLock<Vec<PinTemplate>> = OnceLock::new();
    ALL.get_or_init(build)
}

fn covers(prefix: &str, function: &str) -> bool {
    function == prefix
        || function
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

/// The template of a symbol function type: the one with the longest
/// matching function, so `coil.brake` wins over `coil` for a brake coil.
pub fn template_for(function: &str) -> Option<&'static PinTemplate> {
    templates()
        .iter()
        .filter_map(|tpl| {
            tpl.functions
                .iter()
                .filter(|f| covers(f, function))
                .map(|f| f.len())
                .max()
                .map(|len| (len, tpl))
        })
        .max_by_key(|(len, _)| *len)
        .map(|(_, tpl)| tpl)
}

/// How `pins` differs from the template of `function`, or `None` when it
/// follows the template (any of its sets) or the function has no template.
pub fn check_pins(function: &str, pins: &[&str]) -> Option<Deviation> {
    let tpl = template_for(function)?;
    if tpl.sets.iter().any(|s| s.accepts(pins)) {
        return None;
    }
    let (set, missing, unexpected) = tpl
        .sets
        .iter()
        .map(|s| {
            let missing: Vec<String> = s
                .required
                .iter()
                .filter(|r| !pins.contains(&r.as_str()))
                .cloned()
                .collect();
            let unexpected: Vec<String> = pins
                .iter()
                .filter(|p| !s.knows(p))
                .map(|p| (*p).to_owned())
                .collect();
            (s, missing, unexpected)
        })
        .min_by_key(|(_, m, u)| m.len() + u.len())?;
    Some(Deviation {
        class: tpl.class,
        label: tpl.label,
        closest: set.name.clone(),
        closest_pins: set.describe(),
        missing,
        unexpected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coil_and_brake() {
        assert!(check_pins("coil.contactor", &["A1", "A2"]).is_none());
        assert!(check_pins("coil.brake", &["BD1", "BD2"]).is_none());
        let d = check_pins("coil.relay", &["A1", "B2"]).unwrap();
        assert_eq!(d.missing, ["A2"]);
        assert_eq!(d.unexpected, ["B2"]);
        assert!(d.message().contains("coil"));
    }

    #[test]
    fn motor_sets_and_optional_pins() {
        assert!(check_pins("motor.ac3", &["U1", "V1", "W1"]).is_none());
        assert!(check_pins("motor.ac3", &["U1", "V1", "W1", "PE", "BD1", "BD2"]).is_none());
        assert!(check_pins("motor.ac3", &["1U", "1V", "1W", "2U", "2V", "2W", "PE"]).is_none());
        let d = check_pins("motor.ac3", &["U", "V", "W"]).unwrap();
        assert_eq!(d.missing, ["U1", "V1", "W1"]);
    }

    #[test]
    fn poles_either_numbering() {
        assert!(check_pins("protection.mcb", &["1", "2", "3", "4", "5", "6"]).is_none());
        assert!(check_pins("protection.mcb", &["1L1", "2T1", "3L2", "4T2"]).is_none());
        assert!(check_pins("protection.mcb", &["1", "2", "3"]).is_some());
    }

    #[test]
    fn contacts_have_no_template_and_unknown_functions_pass() {
        assert!(template_for("contact.no").is_none());
        assert!(check_pins("misc.thing", &["Q"]).is_none());
    }

    #[test]
    fn longest_function_wins() {
        assert_eq!(template_for("coil.brake").unwrap().class, "coil.brake");
        assert_eq!(template_for("coil.timer.on_delay").unwrap().class, "coil");
    }

    #[test]
    fn safety_relay_takes_contacts_and_inputs() {
        let pins = ["A1", "S11", "S12", "A2", "13", "14", "23", "24", "41", "42"];
        assert!(check_pins("safety.relay", &pins).is_none());
        assert!(check_pins("safety.relay", &["A1", "S11"]).is_some());
    }
}
