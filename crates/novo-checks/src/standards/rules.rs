//! The rule table: one entry per rule, with its id, standard, clause pointer,
//! plain-words requirement and what it needs to be evaluated.
//!
//! The wording of every requirement is our own paraphrase, short and
//! practical. It is not the text of the standard and is no substitute for it:
//! the engineer reads the clause in their own licensed copy. Clause numbers
//! are pointers to where the requirement lives in the edition named on the
//! set; numbering shifts between editions, so check it against your copy.

use crate::Severity;

use super::StandardSet;

/// One rule of a standard.
#[derive(Debug, PartialEq, Eq)]
pub struct Rule {
    /// Stable id, also the name its waivers are stored under:
    /// `<set>.<name>`, e.g. `iec60204.pe-colour`.
    pub id: &'static str,
    pub set: StandardSet,
    /// Where to read the requirement in the edition named by the set.
    pub clause: &'static str,
    /// A few words.
    pub title: &'static str,
    /// What the rule asks of the design, in our own words.
    pub requirement: &'static str,
    /// How serious a failure is.
    pub severity: Severity,
    /// What the rule reads from the project: shown when it cannot be
    /// evaluated, so the user knows what to fill in.
    pub inputs: &'static str,
    /// Limits or table values the rule uses that depend on the edition: the
    /// engineer confirms them in their licensed copy, and each can be
    /// overridden per project. `None` when the rule has no such number.
    pub verify: Option<&'static str>,
}

macro_rules! rules {
    ($($konst:ident = $id:literal, $set:ident, $clause:literal, $title:literal, $sev:ident,
        $req:literal, $inputs:literal, $verify:expr;)*) => {
        $(pub(crate) const $konst: &str = $id;)*
        /// Every rule, grouped by standard.
        pub static RULES: &[Rule] = &[
            $(Rule {
                id: $id,
                set: StandardSet::$set,
                clause: $clause,
                title: $title,
                requirement: $req,
                severity: Severity::$sev,
                inputs: $inputs,
                verify: $verify,
            },)*
        ];
    };
}

rules! {
    // ----- IEC 60204-1 ---------------------------------------------------
    IEC_SUPPLY_DISCONNECT = "iec60204.supply-disconnect", Iec60204, "5.3",
        "Supply disconnecting device", Error,
        "Every incoming supply is switched through a disconnecting device that isolates it, before any power load can be reached.",
        "A supply feed (Line potential) and the devices between it and the loads. A circuit breaker counts when its part or device states `isolating = true`.",
        None;
    IEC_DISCONNECT_LOCKABLE = "iec60204.disconnect-lockable", Iec60204, "5.3.3",
        "Disconnecting device lockable in OFF", Warning,
        "The supply disconnecting device can be locked in the OFF position.",
        "The device or part property `lockable` (true/false) on the disconnecting device.",
        None;
    IEC_PE_CONNECTION = "iec60204.pe-connection", Iec60204, "8.2",
        "Protective bonding of motors", Error,
        "Every motor has its protective conductor connected, and to a protective-earth net.",
        "Motor symbols with a PE connection point and the potentials on the nets.",
        None;
    IEC_PE_SIZE = "iec60204.pe-size", Iec60204, "8.2.2",
        "Protective conductor cross-section", Error,
        "A protective conductor is at least as large as the phase conductors it accompanies, up to 16 mm², then 16 mm², then half the phase size above 35 mm².",
        "Cross-sections on the wires (or cable cores) at a device's phase and PE connections.",
        Some("The size steps (16 mm² and 35 mm²) are the usual ones for copper; confirm them for your edition and conductor material.");
    IEC_PE_COLOUR = "iec60204.pe-colour", Iec60204, "13.2.2",
        "Protective conductor identification", Error,
        "A protective conductor is green-yellow, and green-yellow is used for nothing else.",
        "Wire colours (or cable core names) on protective-earth nets.",
        None;
    IEC_NEUTRAL_COLOUR = "iec60204.neutral-colour", Iec60204, "13.2.3",
        "Neutral conductor identification", Warning,
        "A neutral conductor is blue.",
        "Wire colours on neutral nets.",
        None;
    IEC_CONDUCTOR_COLOUR = "iec60204.conductor-colour", Iec60204, "13.2.4",
        "Conductor colours by circuit", Warning,
        "Power circuits are black, ac control circuits red, dc control circuits blue; circuits that stay live with the supply off are orange.",
        "Wire colours and which circuit each wire is in (power or control, from the devices on its net).",
        Some("Colour choices as the edition names them; the table can be overridden per project with `standards.iec60204.colour.*`.");
    IEC_CONTROL_VOLTAGE = "iec60204.control-voltage", Iec60204, "9.1.2",
        "Control circuit voltage", Error,
        "Control circuit voltage stays below the limit for ac and for dc.",
        "The voltage on the potentials of control circuits.",
        Some("Limits: 277 V ac and 250 V dc as understood for the edition; override with `standards.iec60204.max_control_ac_v` and `max_control_dc_v`.");
    IEC_CONTROL_SUPPLY = "iec60204.control-supply", Iec60204, "9.1.1",
        "Control circuit supply", Warning,
        "AC control circuits are fed from their own source (a control transformer), not straight from the power supply potentials.",
        "Potentials of control circuits and where else they are used.",
        None;
    IEC_CONTROL_PROTECTION = "iec60204.control-protection", Iec60204, "7.2.3",
        "Control circuit overcurrent protection", Warning,
        "Control circuits are protected against overcurrent by a fuse or breaker between the source and the control devices.",
        "A supply feed and the devices between it and the control devices.",
        None;
    IEC_SHORT_CIRCUIT_PROTECTION = "iec60204.short-circuit-protection", Iec60204, "7.2",
        "Short-circuit protection of power circuits", Error,
        "Every power load is reached from the supply only through a fuse or breaker.",
        "A supply feed and the devices between it and each motor, drive or heater.",
        None;
    IEC_MOTOR_OVERLOAD = "iec60204.motor-overload", Iec60204, "7.3",
        "Motor overload protection", Error,
        "Every motor is protected against overload: an overload relay, a motor protection breaker, a drive, or a thermal sensor in the motor.",
        "A supply feed, the devices between it and each motor, or the motor property `thermal_protection = true`.",
        None;
    IEC_ESTOP_PRESENT = "iec60204.estop-present", Iec60204, "9.2.3.4",
        "Emergency stop device present", Error,
        "The machine has an emergency stop device, unless the project states that none is required.",
        "An emergency stop or pull-cord symbol, or the project property `machine.estop_required = false`.",
        None;
    IEC_ESTOP_CONTACT = "iec60204.estop-contact", Iec60204, "10.7",
        "Emergency stop has a break contact", Error,
        "An emergency stop device opens a normally closed contact, so a broken wire stops the machine.",
        "The library symbol of each emergency stop device.",
        None;
    IEC_ESTOP_COLOUR = "iec60204.estop-colour", Iec60204, "10.7.3",
        "Emergency stop colours", Warning,
        "The actuator is red on a yellow background.",
        "The device or part properties `actuator_colour` and `background_colour`.",
        None;
    IEC_STOP_CATEGORY = "iec60204.stop-category", Iec60204, "9.2.2",
        "Emergency stop category", Error,
        "An emergency stop works as stop category 0 or 1, never 2.",
        "The device or part property `stop_category` (0, 1 or 2).",
        None;
    IEC_PUSHBUTTON_COLOUR = "iec60204.pushbutton-colour", Iec60204, "10.2.1",
        "Push-button colours", Warning,
        "A push-button's colour fits its function: start/on green, stop/off red, reset blue, with black, grey and white for neutral use.",
        "The device property `function` and the device or part property `actuator_colour`.",
        None;
    IEC_INDICATOR_COLOUR = "iec60204.indicator-colour", Iec60204, "10.3",
        "Indicator light colours", Warning,
        "A pilot lamp's colour fits what it says: red for danger, yellow for abnormal, green for normal, blue for action required, white for neutral.",
        "The device property `function` on the lamp, and its symbol (`lamp_red` ...) or `lens_colour`.",
        None;
    IEC_NAMEPLATE = "iec60204.nameplate", Iec60204, "16.4",
        "Equipment marking data", Warning,
        "The data the equipment nameplate carries is stated: rated voltage, phases, frequency and full-load current.",
        "The project properties `machine.rated_voltage_v`, `machine.phases`, `machine.frequency_hz`, `machine.full_load_current_a`.",
        None;

    // ----- NFPA 79 -------------------------------------------------------
    NFPA_SUPPLY_DISCONNECT = "nfpa79.supply-disconnect", Nfpa79, "5.3",
        "Supply disconnecting means", Error,
        "Every incoming supply is switched through a disconnecting means before any power load can be reached.",
        "A supply feed (Line potential) and the devices between it and the loads. A circuit breaker counts when its part or device states `isolating = true`.",
        None;
    NFPA_DISCONNECT_LOCKABLE = "nfpa79.disconnect-lockable", Nfpa79, "5.3.3",
        "Disconnecting means lockable in OFF", Warning,
        "The disconnecting means can be locked in the OFF position.",
        "The device or part property `lockable` (true/false) on the disconnecting means.",
        None;
    NFPA_PE_CONNECTION = "nfpa79.pe-connection", Nfpa79, "8.2",
        "Equipment grounding of motors", Error,
        "Every motor has its equipment grounding conductor connected, and to a protective-earth net.",
        "Motor symbols with a PE connection point and the potentials on the nets.",
        None;
    NFPA_GROUND_SIZE = "nfpa79.ground-size", Nfpa79, "8.2.2",
        "Equipment grounding conductor size", Error,
        "A motor's grounding conductor is sized for the rating of the overcurrent device that protects its branch.",
        "The rated current of the nearest overcurrent device upstream of the motor and the cross-section of its PE wire.",
        Some("Sizes follow the usual copper table for equipment grounding conductors (NEC 250.122 values); confirm them in your copy of NFPA 79.");
    NFPA_PE_COLOUR = "nfpa79.pe-colour", Nfpa79, "13.2",
        "Grounding conductor identification", Error,
        "A grounding conductor is green or green with a yellow stripe.",
        "Wire colours on protective-earth nets.",
        None;
    NFPA_NEUTRAL_COLOUR = "nfpa79.neutral-colour", Nfpa79, "13.2",
        "Grounded conductor identification", Warning,
        "A grounded (neutral) conductor is white or natural grey.",
        "Wire colours on neutral nets.",
        None;
    NFPA_CONDUCTOR_COLOUR = "nfpa79.conductor-colour", Nfpa79, "13.2",
        "Conductor colours by circuit", Warning,
        "Ungrounded power conductors are black (dc power blue), ac control red, dc control blue; interlock circuits fed from another source are yellow.",
        "Wire colours and which circuit each wire is in.",
        Some("Colour choices as understood for the edition; override per project with `standards.nfpa79.colour.*`.");
    NFPA_CONDUCTOR_AMPACITY = "nfpa79.conductor-ampacity", Nfpa79, "12.5",
        "Conductor ampacity", Error,
        "A conductor can carry the current of the device that protects it, and a motor's conductors 125 % of its full-load current.",
        "Cross-sections on wires, rated currents of protective devices, motor `flc_a`.",
        Some("Ampacities are the usual copper table values at 60 °C terminations (75 °C with `standards.nfpa79.terminal_temp_c = 75`); confirm them in your copy.");
    NFPA_CONTROL_VOLTAGE = "nfpa79.control-voltage", Nfpa79, "9.1.1",
        "Control circuit voltage", Error,
        "AC control circuit voltage stays below the limit.",
        "The voltage on the potentials of control circuits.",
        Some("Limit: 120 V ac as understood for the edition; override with `standards.nfpa79.max_control_ac_v`.");
    NFPA_CONTROL_PROTECTION = "nfpa79.control-protection", Nfpa79, "7.2",
        "Control circuit overcurrent protection", Warning,
        "Control circuits are protected against overcurrent between the source and the control devices.",
        "A supply feed and the devices between it and the control devices.",
        None;
    NFPA_SHORT_CIRCUIT_PROTECTION = "nfpa79.short-circuit-protection", Nfpa79, "7.2",
        "Short-circuit protection of power circuits", Error,
        "Every power load is reached from the supply only through an overcurrent protective device.",
        "A supply feed and the devices between it and each motor, drive or heater.",
        None;
    NFPA_MOTOR_OVERLOAD = "nfpa79.motor-overload", Nfpa79, "7.3",
        "Motor overload protection", Error,
        "Every motor is protected against overload.",
        "A supply feed, the devices between it and each motor, or the motor property `thermal_protection = true`.",
        None;
    NFPA_MOTOR_BRANCH_SIZE = "nfpa79.motor-branch-size", Nfpa79, "7.2",
        "Motor branch-circuit protective device size", Error,
        "A motor's branch protective device is no larger than its type allows as a percentage of the motor's full-load current.",
        "Motor `flc_a`, and the protective device's rated current and its `protection_kind`.",
        Some("Percentages (300 % non-time-delay fuse, 175 % time-delay fuse, 250 % inverse-time breaker, 800 % instantaneous-trip breaker) follow the usual NEC 430.52 values; confirm them in your copy.");
    NFPA_ESTOP_PRESENT = "nfpa79.estop-present", Nfpa79, "9.2.5.4",
        "Emergency stop device present", Error,
        "The machine has an emergency stop device, unless the project states that none is required.",
        "An emergency stop or pull-cord symbol, or the project property `machine.estop_required = false`.",
        None;
    NFPA_ESTOP_CONTACT = "nfpa79.estop-contact", Nfpa79, "10.7",
        "Emergency stop has a break contact", Error,
        "An emergency stop device opens a normally closed contact.",
        "The library symbol of each emergency stop device.",
        None;
    NFPA_ESTOP_COLOUR = "nfpa79.estop-colour", Nfpa79, "10.7",
        "Emergency stop colours", Warning,
        "The actuator is red on a yellow background.",
        "The device or part properties `actuator_colour` and `background_colour`.",
        None;
    NFPA_STOP_CATEGORY = "nfpa79.stop-category", Nfpa79, "9.2.2",
        "Emergency stop category", Error,
        "An emergency stop works as stop category 0 or 1, never 2.",
        "The device or part property `stop_category`.",
        None;
    NFPA_PUSHBUTTON_COLOUR = "nfpa79.pushbutton-colour", Nfpa79, "10.2",
        "Push-button colours", Warning,
        "A push-button's colour fits its function.",
        "The device property `function` and the device or part property `actuator_colour`.",
        None;
    NFPA_INDICATOR_COLOUR = "nfpa79.indicator-colour", Nfpa79, "10.3",
        "Indicator light colours", Warning,
        "A pilot lamp's colour fits what it says.",
        "The device property `function` on the lamp, and its symbol or `lens_colour`.",
        None;
    NFPA_SCCR = "nfpa79.sccr", Nfpa79, "7.2.10",
        "Short-circuit current rating", Error,
        "The short-circuit current rating of the control panel is determined and is not below the available fault current at the supply.",
        "The project property `supply.short_circuit_ka` and the ratings of the power components (see UL 508A).",
        None;
    NFPA_NAMEPLATE = "nfpa79.nameplate", Nfpa79, "16.5",
        "Machine nameplate data", Warning,
        "The machine nameplate data is stated: supply voltage, phases, frequency, full-load current and short-circuit current rating.",
        "The project properties `machine.rated_voltage_v`, `machine.phases`, `machine.frequency_hz`, `machine.full_load_current_a`, `panel.sccr_ka`.",
        None;

    // ----- UL 508A -------------------------------------------------------
    UL_SCCR_COMPONENTS = "ul508a.sccr-components", Ul508a, "SB",
        "Component short-circuit ratings known", Warning,
        "Every power-circuit component has a short-circuit rating: marked on the part, or taken from the unmarked-component table of your copy.",
        "Device or part property `sccr_ka` (or `breaking_capacity_ka` for protective devices), or a default entered under `standards.ul508a.default_sccr_ka.<type>`.",
        Some("Unmarked-component defaults are not built in: enter the values from your own copy of UL 508A Supplement SB.");
    UL_SCCR_ADEQUATE = "ul508a.sccr-adequate", Ul508a, "SB",
        "Panel short-circuit rating covers the available fault current", Error,
        "The panel's short-circuit current rating, the lowest rating in the power circuit after any current-limiting protection, is at least the available fault current.",
        "`supply.short_circuit_ka` and the ratings of all power-circuit components.",
        Some("The calculation is the simplified Supplement SB method (lowest rating, raised by an upstream current-limiting device whose let-through is within the component's rating); combination ratings and transformer secondaries are not modelled.");
    UL_SCCR_MARKING = "ul508a.sccr-marking", Ul508a, "SB",
        "Marked short-circuit rating matches the calculation", Warning,
        "The rating marked on the panel (`panel.sccr_ka`) is stated and does not exceed the calculated rating.",
        "The project property `panel.sccr_ka`.",
        None;
    UL_MOTOR_BRANCH_SIZE = "ul508a.motor-branch-size", Ul508a, "31",
        "Motor branch-circuit protective device size", Error,
        "A motor's branch protective device is no larger than its type allows as a percentage of the motor's full-load current.",
        "Motor `flc_a`, and the protective device's rated current and its `protection_kind`.",
        Some("Percentages as for NFPA 79 (usual NEC 430.52 values); confirm them in your copy.");
    UL_OVERLOAD_SETTING = "ul508a.overload-setting", Ul508a, "30",
        "Motor overload setting", Error,
        "An overload relay is set no higher than 125 % of the motor's full-load current (115 % for motors without a 1.15 service factor or 40 °C rise marking).",
        "Motor `flc_a`, motor `service_factor`, and the overload device's `setting_a`.",
        Some("Percentages follow the usual NEC 430.32 values; confirm them in your copy.");
    UL_CONDUCTOR_AMPACITY = "ul508a.conductor-ampacity", Ul508a, "28",
        "Conductor ampacity", Error,
        "A conductor can carry the current of the device that protects it, and a motor's conductors 125 % of its full-load current.",
        "Cross-sections on wires, rated currents of protective devices, motor `flc_a`.",
        Some("Ampacities are the usual copper table values at 60 °C terminations (75 °C with `standards.ul508a.terminal_temp_c = 75`); confirm them in your copy.");
    UL_GROUND_SIZE = "ul508a.ground-size", Ul508a, "29",
        "Bonding conductor size", Error,
        "A motor's grounding conductor is sized for the rating of the overcurrent device that protects its branch.",
        "The rated current of the nearest overcurrent device upstream of the motor and the cross-section of its PE wire.",
        Some("Sizes follow the usual copper table for equipment grounding conductors (NEC 250.122 values); confirm them in your copy.");
    UL_NAMEPLATE = "ul508a.nameplate", Ul508a, "Marking",
        "Panel nameplate data", Warning,
        "The panel nameplate data is stated: supply voltage, phases, frequency, full-load current and short-circuit current rating.",
        "The project properties `machine.rated_voltage_v`, `machine.phases`, `machine.frequency_hz`, `machine.full_load_current_a`, `panel.sccr_ka`.",
        None;
}

/// The rule with this id.
pub fn rule(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

/// The rules of one standard, in table order.
pub fn rules_of(set: StandardSet) -> impl Iterator<Item = &'static Rule> {
    RULES.iter().filter(move |r| r.set == set)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_prefixed_by_their_set() {
        let mut ids: Vec<&str> = RULES.iter().map(|r| r.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate rule id");
        for r in RULES {
            assert!(
                r.id.starts_with(&format!("{}.", r.set.id())),
                "{} is not in set {}",
                r.id,
                r.set.id()
            );
            assert!(!r.clause.is_empty() && !r.title.is_empty());
            assert!(r.requirement.ends_with('.'), "{}", r.id);
        }
    }

    #[test]
    fn every_set_has_rules() {
        for set in StandardSet::ALL {
            assert!(rules_of(set).count() >= 7, "{set:?}");
        }
    }
}
