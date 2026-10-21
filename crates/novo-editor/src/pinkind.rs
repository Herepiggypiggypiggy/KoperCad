//! What a pin is for, read from the library symbol, so auto-wiring joins
//! pins that belong together and not just pins that happen to face each
//! other (DESIGN.md §2).
//!
//! This is a classification by heuristics over the symbol's function type,
//! behavior kind, pole groups and connection roles; the library has no
//! explicit "pin kind" field. It only ever judges the *automatic* wires: a
//! person can still draw any wire by hand. A pin whose symbol is unknown, or
//! whose kind the heuristics cannot tell, is never the reason for a refusal.

use novo_symbols::Symbol;

/// The circuit a pin belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinKind {
    /// Main circuit: poles of power contacts, breakers with several poles,
    /// overload relays, motor windings.
    Power,
    /// Control circuit: coils, auxiliary contacts, push buttons, lamps, PLC
    /// channels.
    Control,
    /// Protective earth.
    Earth,
    /// Neutral conductor.
    Neutral,
    /// A terminal, plug or bridge: it passes whatever is connected to it.
    Terminal,
    /// A single pole of a breaker, fuse or switch: it can sit in the main
    /// circuit or feed a control circuit, but is never earth.
    Either,
}

impl PinKind {
    /// Short wording for refusal messages.
    pub fn label(self) -> &'static str {
        match self {
            PinKind::Power => "power",
            PinKind::Control => "control",
            PinKind::Earth => "earth",
            PinKind::Neutral => "neutral",
            PinKind::Terminal => "terminal",
            PinKind::Either => "single pole",
        }
    }
}

/// What the automatic wiring does with two facing pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// They belong together: draw the wire.
    Wire,
    /// Plausible but unusual: propose, never draw on its own.
    Ask(String),
    /// Different circuits: no automatic wire.
    Refuse(String),
}

/// The kind of connection `pin` (a designation such as `A2` or `U1`) of
/// `def`.
pub fn pin_kind(def: &Symbol, pin: &str) -> PinKind {
    let role = def
        .connections
        .iter()
        .find(|c| c.id == pin)
        .and_then(|c| c.role.as_deref());
    match role {
        Some("pe") => return PinKind::Earth,
        Some("n") => return PinKind::Neutral,
        _ => {}
    }
    let f = def.function.as_str();
    let part = |p: &str| f == p || f.starts_with(&format!("{p}."));
    if part("earth") || f == "terminal.pe" {
        return PinKind::Earth;
    }
    if f == "terminal.n" {
        return PinKind::Neutral;
    }
    if part("terminal") || part("connector") || part("connectivity") {
        return PinKind::Terminal;
    }
    let poles = def.behavior.poles().len();
    match def.behavior.kind.as_str() {
        "coil" | "lamp" | "signal" | "meter" | "plc_input" | "plc_output" => PinKind::Control,
        "contact" if part("contact.main") => PinKind::Power,
        "contact" if part("switch") => several_or_either(poles),
        "contact" => PinKind::Control,
        "overload" | "motor" => PinKind::Power,
        "breaker" | "fuse" => several_or_either(poles),
        _ if part("switch") || part("protection") => several_or_either(poles),
        // Transformers, converters, sources, loads, sensors, passives: they
        // can sit in either circuit.
        _ => PinKind::Either,
    }
}

fn several_or_either(poles: usize) -> PinKind {
    if poles >= 2 {
        PinKind::Power
    } else {
        PinKind::Either
    }
}

/// Whether two facing pins may be wired automatically.
pub fn judge(a: (&Symbol, &str), b: (&Symbol, &str)) -> Verdict {
    let (ka, kb) = (pin_kind(a.0, a.1), pin_kind(b.0, b.1));
    let clash = |x: PinKind, y: PinKind| {
        let pair = |p: PinKind, q: PinKind| (x == p && y == q) || (x == q && y == p);
        pair(PinKind::Power, PinKind::Control)
            || pair(PinKind::Earth, PinKind::Power)
            || pair(PinKind::Earth, PinKind::Control)
            || pair(PinKind::Earth, PinKind::Neutral)
            || pair(PinKind::Earth, PinKind::Either)
    };
    if clash(ka, kb) {
        return Verdict::Refuse(format!("{} to {}", ka.label(), kb.label()));
    }
    // Designations in a series chain run odd (supply side) to even (load
    // side); the next device starts again at an odd number. Two odd or two
    // even numbers facing each other mean a part is turned over.
    let series = |k: PinKind| matches!(k, PinKind::Power | PinKind::Control | PinKind::Either);
    if series(ka)
        && series(kb)
        && let (Ok(x), Ok(y)) = (a.1.parse::<u32>(), b.1.parse::<u32>())
    {
        if x % 2 == y % 2 {
            let side = if x % 2 == 1 { "supply" } else { "load" };
            return Verdict::Ask(format!(
                "both on the {side} side, a part may be turned over"
            ));
        }
        // Pole n of the upstream device (even, load side) belongs to pole n
        // of the next (the odd number just below it).
        if ka == PinKind::Power && kb == PinKind::Power {
            let (even, odd) = if x % 2 == 0 { (x, y) } else { (y, x) };
            if odd + 1 != even {
                return Verdict::Ask("crosses poles".to_string());
            }
        }
    }
    Verdict::Wire
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_symbols::Library;

    fn lib() -> Library {
        Library::builtin().unwrap()
    }

    fn kind(id: &str, pin: &str) -> PinKind {
        pin_kind(lib().get(id).unwrap_or_else(|| panic!("{id}")), pin)
    }

    #[test]
    fn pins_are_sorted_into_their_circuits() {
        assert_eq!(kind("iec/coils/contactor", "A1"), PinKind::Control);
        assert_eq!(kind("iec/contactors/main_3p", "2"), PinKind::Power);
        assert_eq!(kind("iec/protection/mcb_3p", "1"), PinKind::Power);
        assert_eq!(kind("iec/protection/mcb_1p", "1"), PinKind::Either);
        assert_eq!(kind("iec/protection/overload_relay", "3"), PinKind::Power);
        assert_eq!(kind("iec/motors/motor_3ph", "U1"), PinKind::Power);
        assert_eq!(kind("iec/motors/motor_3ph", "PE"), PinKind::Earth);
        assert_eq!(kind("iec/contacts/no", "13"), PinKind::Control);
        assert_eq!(kind("iec/operators/pushbutton_no", "13"), PinKind::Control);
    }
}
