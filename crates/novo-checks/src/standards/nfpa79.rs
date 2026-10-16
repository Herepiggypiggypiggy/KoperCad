//! NFPA 79 rules. Same shared logic as IEC 60204-1 with NFPA's ids, colour
//! scheme and limits, plus the AWG-based conductor rules and the SCCR rule.

use super::rules::*;
use super::shared::{self, Scheme};
use super::{Env, Sink, StandardSet, keys};

pub(super) fn run(e: &Env<'_>, s: &mut Sink) {
    let set = StandardSet::Nfpa79;
    if !e.on(set) {
        return;
    }
    shared::supply_disconnect(e, s, set, NFPA_SUPPLY_DISCONNECT, NFPA_DISCONNECT_LOCKABLE);
    shared::earth_connection(e, s, NFPA_PE_CONNECTION);
    shared::ground_size_awg(e, s, NFPA_GROUND_SIZE);
    let scheme = Scheme::nfpa79(e);
    shared::colours(
        e,
        s,
        &scheme,
        NFPA_PE_COLOUR,
        NFPA_NEUTRAL_COLOUR,
        NFPA_CONDUCTOR_COLOUR,
    );
    shared::ampacity_rule(e, s, set, NFPA_CONDUCTOR_AMPACITY, shared::Tables::Awg);
    // 120 V ac as understood for the edition; overridable. No dc limit is
    // built in: the engineer enters it from their copy.
    shared::control_voltage(e, s, set, NFPA_CONTROL_VOLTAGE, 120.0, None);
    shared::protection(
        e,
        s,
        NFPA_SHORT_CIRCUIT_PROTECTION,
        NFPA_CONTROL_PROTECTION,
        NFPA_MOTOR_OVERLOAD,
    );
    shared::motor_branch_size(e, s, NFPA_MOTOR_BRANCH_SIZE);
    shared::estop(
        e,
        s,
        NFPA_ESTOP_PRESENT,
        NFPA_ESTOP_CONTACT,
        NFPA_ESTOP_COLOUR,
        NFPA_STOP_CATEGORY,
    );
    shared::pushbutton_colours(e, s, NFPA_PUSHBUTTON_COLOUR);
    shared::indicator_colours(e, s, NFPA_INDICATOR_COLOUR);
    sccr_rule(e, s);
    shared::nameplate(e, s, NFPA_NAMEPLATE, true);
}

/// The panel's short-circuit rating is determined (marked, or calculated from
/// every component) and not below the available fault current.
fn sccr_rule(e: &Env<'_>, s: &mut Sink) {
    let Some(report) = s.sccr.clone() else {
        return;
    };
    let Some(available) = report.available_ka else {
        s.unknown(
            NFPA_SCCR,
            e.anchor(),
            format!(
                "State the available fault current at the supply (`{}`) to compare it with the panel's rating",
                keys::SUPPLY_SHORT_CIRCUIT_KA
            ),
        );
        return;
    };
    let marked = e.params.number(keys::PANEL_SCCR_KA);
    let rating = marked.or(if report.complete {
        report.panel_ka
    } else {
        None
    });
    match rating {
        Some(r) if r + 1e-9 >= available => s.pass(NFPA_SCCR),
        Some(r) => s.fail(
            NFPA_SCCR,
            e.anchor(),
            format!(
                "The panel's short-circuit current rating is {} kA, below the {} kA available at the supply",
                super::report::kilo(r),
                super::report::kilo(available)
            ),
        ),
        None => match report.panel_ka {
            Some(upper) if upper + 1e-9 < available => s.fail(
                NFPA_SCCR,
                e.anchor(),
                format!(
                    "A component rated {} kA is already below the {} kA available at the supply",
                    super::report::kilo(upper),
                    super::report::kilo(available)
                ),
            ),
            _ => s.unknown(
                NFPA_SCCR,
                e.anchor(),
                "The panel's rating is not determined: some power components have no short-circuit rating (see UL 508A: component ratings) and none is marked on the panel".to_owned(),
            ),
        },
    }
}
