//! IEC 81346-2 letter codes: the one table every crate takes a new
//! device's letter from (DESIGN.md §6, "Letter codes").
//!
//! The letter comes from what a device is for, read from the symbol's
//! function type. Two classes differ between companies, so a project rule
//! can change them ([`LetterRules`]); a typed tag always wins over both.

/// How contactors are coded: K (the default most panel builders use) or Q
/// (IEC 81346-2:2019, a power contactor switches energy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContactorCode {
    #[default]
    K,
    Q,
}

/// How circuit breakers are coded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BreakerCode {
    /// A breaker that protects a circuit is F (`mcb`), one that switches a
    /// feeder is Q (`mpcb`).
    #[default]
    ByPurpose,
    AlwaysQ,
    AlwaysF,
}

/// The project rules that change letter codes ("Contactors are coded
/// [K | Q]", "Circuit breakers are coded [by purpose | always Q |
/// always F]").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LetterRules {
    pub contactors: ContactorCode,
    pub breakers: BreakerCode,
}

/// The letter code for a device of `function` under the default rules.
/// `None` for symbols that are not devices of their own (connectivity,
/// earth, terminal accessories).
///
/// ```
/// assert_eq!(novo_symbols::letter_code("coil.relay"), Some("K"));
/// assert_eq!(novo_symbols::letter_code("protection.mpcb"), Some("Q"));
/// assert_eq!(novo_symbols::letter_code("earth.pe"), None);
/// ```
pub fn letter_code(function: &str) -> Option<&'static str> {
    letter_code_with(function, LetterRules::default())
}

/// The letter code for a device of `function` under the project's rules.
pub fn letter_code_with(function: &str, rules: LetterRules) -> Option<&'static str> {
    let parts: Vec<&str> = function.split('.').collect();
    let has = |part: &str| parts.contains(&part);
    let contactor = match rules.contactors {
        ContactorCode::K => "K",
        ContactorCode::Q => "Q",
    };
    let breaker = |by_purpose: &'static str| match rules.breakers {
        BreakerCode::ByPurpose => by_purpose,
        BreakerCode::AlwaysQ => "Q",
        BreakerCode::AlwaysF => "F",
    };
    Some(match parts[0] {
        "connectivity" | "earth" | "accessory" => return None,
        "terminal" | "connector" => "X",
        "coil" if has("valve") => "M",
        "coil" if has("timer") => "K",
        "coil" => contactor,
        "contact" if ["push", "key", "turn", "estop"].iter().any(|p| has(p)) => "S",
        "contact"
            if ["position", "proximity", "pressure", "float", "temperature"]
                .iter()
                .any(|p| has(p)) =>
        {
            "B"
        }
        "contact" if has("thermal") => "F",
        "contact" if has("delay_on") || has("delay_off") => "K",
        "contact" => contactor,
        "protection" => match parts.get(1).copied() {
            Some("mpcb" | "mcp") => breaker("Q"),
            Some("mcb" | "rcbo") => breaker("F"),
            _ => "F",
        },
        "switch" => "Q",
        "motor" => "M",
        "source" => "G",
        "converter" | "transformer" => "T",
        "meter" | "signal" => "P",
        "sensor" => "B",
        "load" => "E",
        "plc" => "A",
        "passive" => match parts.get(1).copied() {
            Some("capacitor") => "C",
            Some("inductor" | "filter") => "L",
            Some("diode" | "rectifier" | "transistor" | "thyristor" | "triac") => "V",
            _ => "R",
        },
        _ => "A",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Library;

    /// Pins the whole table: every function type in the built-in library
    /// and the letter it gets under the default rules.
    #[test]
    fn table_is_pinned() {
        let expected: &[(&str, Option<&str>)] = &[
            ("accessory.terminal.end_bracket", None),
            ("accessory.terminal.end_plate", None),
            ("coil.relay", Some("K")),
            ("coil.timer.off_delay", Some("K")),
            ("coil.timer.on_delay", Some("K")),
            ("coil.valve", Some("M")),
            ("connectivity.bridge", None),
            ("connectivity.interruption", None),
            ("connectivity.potential", None),
            ("connector.female", Some("X")),
            ("connector.male", Some("X")),
            ("connector.outlet", Some("X")),
            ("connector.pair", Some("X")),
            ("contact.co", Some("K")),
            ("contact.co.push", Some("S")),
            ("contact.main", Some("K")),
            ("contact.nc", Some("K")),
            ("contact.nc.delay_off", Some("K")),
            ("contact.nc.delay_on", Some("K")),
            ("contact.nc.estop", Some("S")),
            ("contact.nc.float", Some("B")),
            ("contact.nc.key", Some("S")),
            ("contact.nc.position", Some("B")),
            ("contact.nc.pressure", Some("B")),
            ("contact.nc.proximity", Some("B")),
            ("contact.nc.push", Some("S")),
            ("contact.nc.temperature", Some("B")),
            ("contact.nc.thermal", Some("F")),
            ("contact.nc.turn", Some("S")),
            ("contact.no", Some("K")),
            ("contact.no.delay_off", Some("K")),
            ("contact.no.delay_on", Some("K")),
            ("contact.no.float", Some("B")),
            ("contact.no.key", Some("S")),
            ("contact.no.position", Some("B")),
            ("contact.no.pressure", Some("B")),
            ("contact.no.proximity", Some("B")),
            ("contact.no.push", Some("S")),
            ("contact.no.temperature", Some("B")),
            ("contact.no.thermal", Some("F")),
            ("contact.no.turn", Some("S")),
            ("converter.ac_dc", Some("T")),
            ("converter.dc_dc", Some("T")),
            ("converter.soft_starter", Some("T")),
            ("converter.vfd", Some("T")),
            ("earth", None),
            ("earth.chassis", None),
            ("earth.pe", None),
            ("load.heater", Some("E")),
            ("meter.current", Some("P")),
            ("meter.energy", Some("P")),
            ("meter.frequency", Some("P")),
            ("meter.hours", Some("P")),
            ("meter.power", Some("P")),
            ("meter.power_factor", Some("P")),
            ("meter.resistance", Some("P")),
            ("meter.temperature", Some("P")),
            ("meter.voltage", Some("P")),
            ("motor.ac1", Some("M")),
            ("motor.ac3", Some("M")),
            ("motor.dc", Some("M")),
            ("passive.capacitor", Some("C")),
            ("passive.capacitor.polarized", Some("C")),
            ("passive.capacitor.variable", Some("C")),
            ("passive.diode", Some("V")),
            ("passive.diode.led", Some("V")),
            ("passive.diode.photo", Some("V")),
            ("passive.diode.zener", Some("V")),
            ("passive.filter", Some("L")),
            ("passive.inductor", Some("L")),
            ("passive.inductor.reactor", Some("L")),
            ("passive.potentiometer", Some("R")),
            ("passive.rectifier", Some("V")),
            ("passive.resistor", Some("R")),
            ("passive.resistor.ldr", Some("R")),
            ("passive.resistor.ntc", Some("R")),
            ("passive.resistor.ptc", Some("R")),
            ("passive.resistor.variable", Some("R")),
            ("passive.resistor.varistor", Some("R")),
            ("passive.thyristor", Some("V")),
            ("passive.transistor.npn", Some("V")),
            ("passive.transistor.pnp", Some("V")),
            ("passive.triac", Some("V")),
            ("plc.input.analog", Some("A")),
            ("plc.input.digital", Some("A")),
            ("plc.output.analog", Some("A")),
            ("plc.output.digital", Some("A")),
            ("plc.output.relay", Some("A")),
            ("plc.supply", Some("A")),
            ("protection.fuse", Some("F")),
            ("protection.fuse_switch", Some("F")),
            ("protection.mcb", Some("F")),
            ("protection.mcp", Some("Q")),
            ("protection.mpcb", Some("Q")),
            ("protection.overload", Some("F")),
            ("protection.rcbo", Some("F")),
            ("protection.rcd", Some("F")),
            ("protection.surge", Some("F")),
            ("sensor.temperature.rtd", Some("B")),
            ("sensor.transmitter", Some("B")),
            ("signal.bell", Some("P")),
            ("signal.buzzer", Some("P")),
            ("signal.horn", Some("P")),
            ("signal.lamp", Some("P")),
            ("source.ac", Some("G")),
            ("source.current", Some("G")),
            ("source.dc", Some("G")),
            ("source.generator", Some("G")),
            ("switch.changeover", Some("Q")),
            ("switch.disconnector", Some("Q")),
            ("switch.main", Some("Q")),
            ("switch.switch_disconnector", Some("Q")),
            ("terminal", Some("X")),
            ("terminal.disconnect", Some("X")),
            ("terminal.fuse", Some("X")),
            ("terminal.multilevel", Some("X")),
            ("terminal.n", Some("X")),
            ("terminal.pe", Some("X")),
            ("transformer.1ph", Some("T")),
            ("transformer.3ph", Some("T")),
            ("transformer.current", Some("T")),
        ];
        for (function, code) in expected {
            assert_eq!(letter_code(function), *code, "{function}");
        }
        // A new function type in the library must be added above.
        let library = Library::builtin().unwrap();
        for symbol in library.iter() {
            let f = symbol.function.as_str();
            assert!(
                expected.iter().any(|(e, _)| *e == f),
                "{f} ({}) is missing from the pinned table",
                symbol.id
            );
        }
    }

    #[test]
    fn project_rules_change_contactors_and_breakers() {
        let q = LetterRules {
            contactors: ContactorCode::Q,
            ..LetterRules::default()
        };
        assert_eq!(letter_code_with("coil.relay", q), Some("Q"));
        assert_eq!(letter_code_with("contact.main", q), Some("Q"));
        assert_eq!(letter_code_with("contact.no", q), Some("Q"));
        assert_eq!(letter_code_with("coil.timer.on_delay", q), Some("K"));
        assert_eq!(letter_code_with("contact.no.push", q), Some("S"));

        let rule = |breakers| LetterRules {
            breakers,
            ..LetterRules::default()
        };
        let always_q = rule(BreakerCode::AlwaysQ);
        assert_eq!(letter_code_with("protection.mcb", always_q), Some("Q"));
        assert_eq!(letter_code_with("protection.rcbo", always_q), Some("Q"));
        assert_eq!(letter_code_with("protection.fuse", always_q), Some("F"));
        let always_f = rule(BreakerCode::AlwaysF);
        assert_eq!(letter_code_with("protection.mpcb", always_f), Some("F"));
        assert_eq!(letter_code_with("protection.mcp", always_f), Some("F"));
        assert_eq!(letter_code_with("protection.overload", always_f), Some("F"));
    }
}
