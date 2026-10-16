//! IEC 60204-1 rules. The logic is shared (see `shared`); this file fixes the
//! rule ids, limits and colour scheme of the standard.

use super::rules::*;
use super::shared::{self, Scheme};
use super::{Env, Sink, StandardSet};

pub(super) fn run(e: &Env<'_>, s: &mut Sink) {
    let set = StandardSet::Iec60204;
    if !e.on(set) {
        return;
    }
    shared::supply_disconnect(e, s, set, IEC_SUPPLY_DISCONNECT, IEC_DISCONNECT_LOCKABLE);
    shared::earth_connection(e, s, IEC_PE_CONNECTION);
    shared::pe_size_metric(e, s, IEC_PE_SIZE);
    shared::ampacity_rule(e, s, set, IEC_CONDUCTOR_AMPACITY, shared::Tables::Iec);
    let scheme = Scheme::iec60204(e);
    shared::colours(
        e,
        s,
        &scheme,
        IEC_PE_COLOUR,
        IEC_NEUTRAL_COLOUR,
        IEC_CONDUCTOR_COLOUR,
    );
    // 277 V ac and 250 V dc as understood for the edition; both overridable.
    shared::control_voltage(e, s, set, IEC_CONTROL_VOLTAGE, 277.0, Some(250.0));
    shared::control_supply(e, s, IEC_CONTROL_SUPPLY);
    shared::protection(
        e,
        s,
        IEC_SHORT_CIRCUIT_PROTECTION,
        IEC_CONTROL_PROTECTION,
        IEC_MOTOR_OVERLOAD,
    );
    shared::estop(
        e,
        s,
        IEC_ESTOP_PRESENT,
        IEC_ESTOP_CONTACT,
        IEC_ESTOP_COLOUR,
        IEC_STOP_CATEGORY,
    );
    shared::pushbutton_colours(e, s, IEC_PUSHBUTTON_COLOUR);
    shared::indicator_colours(e, s, IEC_INDICATOR_COLOUR);
    shared::nameplate(e, s, IEC_NAMEPLATE, false);
}
