//! IEC 60204-1 rules, one break at a time.

use super::*;
use novo_core::*;

const SET: &str = "iec60204";

fn id(name: &str) -> String {
    format!("{SET}.{name}")
}

#[test]
fn a_fuse_alone_is_not_a_disconnecting_device() {
    let mut g = golden();
    let q1 = g.q1_sym;
    g.rekey(q1, "iec/protection/fuse_3p");
    assert_fails(&g, &id("supply-disconnect"), "=A1+FLD-M1 can be reached from the supply without passing a disconnecting device");
    // Nothing isolates, so there is nothing to lock.
    assert_eq!(g.status(&id("disconnect-lockable")), RuleStatus::NotApplicable);
}

#[test]
fn a_bypass_of_the_disconnector_is_found() {
    // A second feed straight to the fuses.
    let mut g = golden();
    let (f1_sym, page) = (g.f1_sym, g.page);
    g.edit(|tx, _| {
        let potential = tx.add_potential("L1b", PotentialKind::Line)?;
        let feed = tx.place_symbol(
            page,
            "iec/connectivity/potential_feed",
            Placement::at(Point::new(5.0, 5.0)),
            vec![Pin::new("1", 0.0, 8.0)],
            SymbolRole::PotentialFeed { potential },
        )?;
        tx.connect(Endpoint::pin(feed, 0), Endpoint::pin(f1_sym, 0))?;
        Ok(())
    });
    assert_fails(&g, &id("supply-disconnect"), "=A1+FLD-M1");
}

#[test]
fn a_breaker_isolates_only_when_it_says_so() {
    let mut g = golden();
    let (q1_sym, q1) = (g.q1_sym, g.q1);
    g.rekey(q1_sym, "iec/protection/mcb_3p");
    g.clear(q1, "isolating");
    assert_unknown(&g, &id("supply-disconnect"), "circuit breaker only");
    g.prop(q1, "isolating", true);
    assert_passes(&g, &id("supply-disconnect"));
    g.prop(q1, "isolating", false);
    assert_fails(&g, &id("supply-disconnect"), "=A1+FLD-M1");
}

#[test]
fn an_unlockable_or_unstated_disconnector_is_reported() {
    let mut g = golden();
    let q1 = g.q1;
    g.prop(q1, "lockable", false);
    assert_fails(&g, &id("disconnect-lockable"), "=A1+CAB-Q1 cannot be locked");
    g.clear(q1, "lockable");
    assert_unknown(&g, &id("disconnect-lockable"), "state whether it can be locked");
    g.prop(q1, "lockable", "yes");
    assert_passes(&g, &id("disconnect-lockable"));
}

#[test]
fn no_supply_feed_means_the_path_cannot_be_followed() {
    // Feeds replaced by plain pins: nothing defines a potential.
    let mut g = golden();
    let wires = g.power_wires.clone();
    g.edit(|tx, _| {
        // Disconnect the L1..L3 feeds by removing their first wires.
        for w in [wires[0], wires[5], wires[10]] {
            tx.remove(w)?;
        }
        Ok(())
    });
    assert_unknown(&g, &id("supply-disconnect"), "not reached by any supply feed");
}

#[test]
fn a_motor_without_its_protective_conductor_fails() {
    let mut g = golden();
    let wire = g.pe_wire;
    g.edit(|tx, _| tx.remove(wire));
    assert_fails(&g, &id("pe-connection"), "has its PE connection unconnected");
}

#[test]
fn a_motor_pe_on_a_live_net_fails_and_on_an_unnamed_net_is_unknown() {
    // PE point wired to the L1 net.
    let mut g = golden();
    let (wire, m1_sym, f1_sym) = (g.pe_wire, g.m1_sym, g.f1_sym);
    g.edit(|tx, _| {
        tx.remove(wire)?;
        tx.connect(Endpoint::pin(m1_sym, 3), Endpoint::pin(f1_sym, 0))?;
        Ok(())
    });
    assert_fails(&g, &id("pe-connection"), "not a protective-earth net");

    // PE point wired to a terminal with no potential at all.
    let mut g = golden();
    let (wire, m1_sym, page, m1) = (g.pe_wire, g.m1_sym, g.page, g.m1);
    g.edit(|tx, _| {
        tx.remove(wire)?;
        let spare = tx.place_symbol(
            page,
            "iec/earth/earth",
            Placement::at(Point::new(5.0, 200.0)),
            vec![Pin::new("1", 0.0, 0.0)],
            SymbolRole::Device { device: m1, main: false },
        )?;
        tx.connect(Endpoint::pin(m1_sym, 3), Endpoint::pin(spare, 0))?;
        Ok(())
    });
    assert_unknown(&g, &id("pe-connection"), "has no PE potential");
}

#[test]
fn the_protective_conductor_follows_the_phase_conductor() {
    let mut g = golden();
    let (power, pe_wire) = (g.power_wires.clone(), g.pe_wire);
    // Up to 16 mm²: as large as the phase conductor.
    for w in &power {
        g.style(*w, Some("BK"), Some(10.0));
    }
    g.style(pe_wire, Some("GNYE"), Some(6.0));
    assert_fails(&g, &id("pe-size"), "protective conductor is 6 mm², phase conductors up to 10 mm² need at least 10 mm²");
    g.style(pe_wire, Some("GNYE"), Some(10.0));
    assert_passes(&g, &id("pe-size"));
    // 16 to 35 mm²: 16 mm².
    for w in &power {
        g.style(*w, Some("BK"), Some(25.0));
    }
    g.style(pe_wire, Some("GNYE"), Some(10.0));
    assert_fails(&g, &id("pe-size"), "need at least 16 mm²");
    g.style(pe_wire, Some("GNYE"), Some(16.0));
    assert_passes(&g, &id("pe-size"));
    // Above 35 mm²: half.
    for w in &power {
        g.style(*w, Some("BK"), Some(70.0));
    }
    assert_fails(&g, &id("pe-size"), "need at least 35 mm²");
    g.style(pe_wire, Some("GNYE"), Some(35.0));
    assert_passes(&g, &id("pe-size"));
}

#[test]
fn a_missing_cross_section_makes_the_pe_size_unknown_not_a_pass() {
    let mut g = golden();
    let pe_wire = g.pe_wire;
    g.style(pe_wire, Some("GNYE"), None);
    assert_unknown(&g, &id("pe-size"), "no cross-section set");
    assert_ne!(g.status(&id("pe-size")), RuleStatus::Pass);
}

#[test]
fn the_protective_conductor_is_green_yellow_and_only_that() {
    let mut g = golden();
    let (pe_wire, w0) = (g.pe_wire, g.power_wires[0]);
    g.style(pe_wire, Some("BU"), Some(2.5));
    assert_fails(&g, &id("pe-colour"), "protective-earth net PE");
    g.style(pe_wire, Some("GN/YE"), Some(2.5));
    assert_passes(&g, &id("pe-colour"));
    // Green-yellow on a power wire is misuse of the protective colour.
    g.style(w0, Some("GNYE"), Some(2.5));
    assert_fails(&g, &id("pe-colour"), "a colour reserved for the protective conductor");
    // No colour at all is unknown.
    g.style(w0, Some("BK"), Some(2.5));
    g.style(pe_wire, None, Some(2.5));
    assert_unknown(&g, &id("pe-colour"), "no colour set");
}

#[test]
fn a_colour_that_is_not_a_code_is_reported_as_unreadable() {
    let mut g = golden();
    let w = g.power_wires[1];
    g.style(w, Some("sparkly"), Some(2.5));
    assert_unknown(&g, &id("conductor-colour"), "not a known code or name (sparkly)");
}

#[test]
fn a_cable_core_name_gives_the_wire_its_colour() {
    let mut g = golden();
    let (w, pe_wire) = (g.power_wires[4], g.pe_wire);
    g.style(w, None, Some(2.5));
    g.style(pe_wire, None, Some(2.5));
    g.edit(|tx, h| {
        let mut cores = Vec::new();
        for name in ["BK", "GNYE"] {
            let mut core = CableCore::new(name);
            core.cross_section_mm2 = Some(2.5);
            cores.push(core);
        }
        let cable = tx.add_cable(tag("=A1-W1"), cores)?;
        tx.assign_core(cable, 0, Some(w))?;
        tx.assign_core(cable, 1, Some(h.pe_wire))?;
        Ok(())
    });
    assert_passes(&g, &id("pe-colour"));
    assert_passes(&g, &id("conductor-colour"));
}

#[test]
fn power_and_control_wires_take_their_circuit_colours() {
    let mut g = golden();
    let (w, c) = (g.power_wires[7], g.control_wires[2]);
    g.style(w, Some("BN"), Some(2.5));
    assert_fails(&g, &id("conductor-colour"), "wire on power net L2 is brown, which should be black");
    // Orange is for circuits that stay live with the supply off.
    g.style(w, Some("OG"), Some(2.5));
    assert_passes(&g, &id("conductor-colour"));
    g.style(w, Some("BK"), Some(2.5));
    // Red is ac control; this control circuit is dc.
    g.style(c, Some("RD"), Some(1.0));
    assert_fails(&g, &id("conductor-colour"), "control net");
    g.style(c, Some("BU"), Some(1.0));
    assert_passes(&g, &id("conductor-colour"));
}

#[test]
fn the_colour_table_can_be_overridden_per_project() {
    let mut g = golden();
    let (w1, w2) = (g.power_wires[7], g.power_wires[12]);
    g.style(w1, Some("BN"), Some(2.5));
    g.style(w2, Some("GY"), Some(2.5));
    assert_fails(&g, &id("conductor-colour"), "should be black");
    g.project_prop("standards.iec60204.colour.ac_power", Some("BK, BN, GY".into()));
    assert_passes(&g, &id("conductor-colour"));
}

#[test]
fn a_neutral_conductor_is_blue() {
    let mut g = golden();
    let w = g.add_neutral(Some("BU"));
    assert_passes(&g, &id("neutral-colour"));
    g.style(w, Some("WH"), Some(1.0));
    assert_fails(&g, &id("neutral-colour"), "neutral net N is white, which should be blue");
    g.style(w, None, Some(1.0));
    assert_unknown(&g, &id("neutral-colour"), "no colour set");
}

#[test]
fn control_voltage_is_held_to_the_limits() {
    let mut g = golden();
    let l_plus = g.l_plus;
    g.edit(|tx, _| tx.update::<Potential>(l_plus, |p| p.voltage = Some(400.0)));
    assert_fails(&g, &id("control-voltage"), "Control circuit L+ is 400 V dc, above the limit of 250 V");
    // The limit is a parameter, so a different edition can be entered.
    g.project_prop("standards.iec60204.max_control_dc_v", Some(500.0.into()));
    assert_passes(&g, &id("control-voltage"));
    g.project_prop("standards.iec60204.max_control_dc_v", None);
    g.edit(|tx, _| tx.update::<Potential>(l_plus, |p| p.voltage = None));
    assert_unknown(&g, &id("control-voltage"), "without a voltage");
}

/// A lamp fed from L1 through its own breaker, returning to N: an ac
/// control circuit tapped from the power supply potential.
fn tap_control_from_l1(g: &mut Golden) {
    let page = g.page;
    g.edit(|tx, h| {
        let f4 = tx.add_device(tag("=A1+CAB-F4"), "Lamp breaker")?;
        let h2 = tx.add_device(tag("=A1+CAB-H2"), "Mains lamp")?;
        let n = tx.add_potential("N", PotentialKind::Neutral)?;
        let mut d = Draw::new(tx, page);
        let feed = d.feed(h.l1);
        let feed_n = d.feed(n);
        let breaker = d.two_pole("iec/protection/mcb_1p", f4, true, "1", "2");
        let lamp = d.sym("iec/signalling/lamp", h2, true, &["X1", "X2"], &[], &[]);
        d.wire(feed, 0, breaker, 0);
        d.wire(breaker, 1, lamp, 0);
        d.wire(lamp, 1, feed_n, 0);
        Ok(())
    });
}

#[test]
fn ac_control_voltage_has_its_own_limit() {
    let mut g = golden();
    tap_control_from_l1(&mut g);
    // L1 is 400 V in the golden machine.
    assert_fails(&g, &id("control-voltage"), "Control circuit L1 is 400 V ac, above the limit of 277 V");
    let l1 = g.l1;
    g.edit(|tx, _| tx.update::<Potential>(l1, |p| p.voltage = Some(230.0)));
    assert_passes(&g, &id("control-voltage"));
    // NFPA 79 holds ac control to a lower limit.
    assert_fails(&g, "nfpa79.control-voltage", "230 V ac, above the limit of 120 V");
    g.project_prop("standards.nfpa79.max_control_ac_v", Some(240.0.into()));
    assert_passes(&g, "nfpa79.control-voltage");
}

#[test]
fn control_circuits_are_not_fed_straight_from_the_power_potentials() {
    let mut g = golden();
    tap_control_from_l1(&mut g);
    assert_fails(&g, &id("control-supply"), "also feeds power circuits");
}

#[test]
fn control_circuits_need_a_fuse_or_breaker() {
    let mut g = golden();
    let f3_sym = g.f3_sym;
    g.rekey(f3_sym, "iec/contacts/no");
    assert_fails(&g, &id("control-protection"), "Control circuit L+ is fed without a fuse or circuit breaker");
}

#[test]
fn power_loads_are_reached_only_through_a_protective_device() {
    let mut g = golden();
    let f1_sym = g.f1_sym;
    g.rekey(f1_sym, "iec/switches/disconnector_3p");
    assert_fails(&g, &id("short-circuit-protection"), "=A1+FLD-M1 can be reached from the supply without passing a fuse or circuit breaker");
    // A disconnector with fuses protects as well as isolates.
    g.rekey(f1_sym, "iec/protection/fuse_switch_3p");
    assert_passes(&g, &id("short-circuit-protection"));
}

#[test]
fn a_motor_needs_overload_protection() {
    let mut g = golden();
    let (f2_sym, m1) = (g.f2_sym, g.m1);
    g.rekey(f2_sym, "iec/contacts/no");
    assert_fails(&g, &id("motor-overload"), "=A1+FLD-M1 has no overload relay");
    // A thermal sensor in the motor is protection too.
    g.prop(m1, "thermal_protection", true);
    assert_passes(&g, &id("motor-overload"));
    g.prop(m1, "thermal_protection", false);
    // So is a motor protection breaker in the supply path.
    g.rekey(f2_sym, "iec/protection/motor_protection_breaker");
    assert_passes(&g, &id("motor-overload"));
}

#[test]
fn a_motor_behind_a_drive_counts_as_protected_by_it() {
    let mut g = golden();
    let (page, m1_sym) = (g.page, g.m1_sym);
    // Replace the motor feed by a drive: its output net is not fed through poles.
    g.edit(|tx, h| {
        tx.remove(h.f2_sym)?;
        let drive = tx.add_device(tag("=A1+CAB-U1"), "Frequency converter")?;
        let mut d = Draw::new(tx, page);
        let u1 = d.sym("iec/drives/frequency_converter", drive, true, &["L1", "L2", "L3", "U", "V", "W"], &[], &[]);
        for (i, pin) in [3u16, 4, 5].into_iter().enumerate() {
            d.wire(u1, pin, m1_sym, i as u16);
        }
        for (i, w) in [h.power_wires[2], h.power_wires[7], h.power_wires[12]].iter().enumerate() {
            let _ = (i, w);
        }
        Ok(())
    });
    assert_passes(&g, &id("motor-overload"));
}

#[test]
fn an_unfed_motor_is_unknown_not_unprotected() {
    let mut g = golden();
    let wires = g.power_wires.clone();
    g.edit(|tx, _| {
        for w in [wires[4], wires[9], wires[14]] {
            tx.remove(w)?;
        }
        Ok(())
    });
    assert_unknown(&g, &id("motor-overload"), "is not reached by any supply feed");
}

#[test]
fn an_emergency_stop_is_required_unless_declared_otherwise() {
    let mut g = golden();
    let s0_sym = g.s0_sym;
    g.rekey(s0_sym, "iec/operators/pushbutton_nc");
    g.clear(g.s0, "function");
    assert_fails(&g, &id("estop-present"), "No emergency stop device is drawn");
    g.project_prop("machine.estop_required", Some(false.into()));
    assert_eq!(g.status(&id("estop-present")), RuleStatus::NotApplicable);
}

#[test]
fn a_pull_cord_or_a_stated_function_counts_as_an_emergency_stop() {
    let mut g = golden();
    let (s0_sym, s0) = (g.s0_sym, g.s0);
    g.rekey(s0_sym, "iec/operators/pullcord_nc");
    assert_passes(&g, &id("estop-present"));
    g.rekey(s0_sym, "iec/operators/pushbutton_nc");
    // The function is stated as an emergency stop on the device.
    assert_passes(&g, &id("estop-present"));
    g.clear(s0, "function");
    assert_fails(&g, &id("estop-present"), "No emergency stop");
}

#[test]
fn an_emergency_stop_opens_a_break_contact() {
    let mut g = golden();
    let s0_sym = g.s0_sym;
    g.rekey(s0_sym, "iec/operators/pushbutton_mushroom_no");
    assert_fails(&g, &id("estop-contact"), "make contact");
    g.rekey(s0_sym, "iec/operators/pushbutton_mushroom_nc");
    assert_passes(&g, &id("estop-contact"));
}

#[test]
fn an_emergency_stop_is_red_on_yellow() {
    let mut g = golden();
    let s0 = g.s0;
    g.prop(s0, "actuator_colour", "GN");
    assert_fails(&g, &id("estop-colour"), "green on yellow");
    g.prop(s0, "actuator_colour", "RD");
    g.prop(s0, "background_colour", "WH");
    assert_fails(&g, &id("estop-colour"), "red on white");
    g.clear(s0, "background_colour");
    assert_unknown(&g, &id("estop-colour"), "without `actuator_colour` and `background_colour`");
}

#[test]
fn an_emergency_stop_is_category_0_or_1() {
    let mut g = golden();
    let s0 = g.s0;
    g.prop(s0, "stop_category", 2i64);
    assert_fails(&g, &id("stop-category"), "stop category 2");
    g.prop(s0, "stop_category", 1i64);
    assert_passes(&g, &id("stop-category"));
    g.clear(s0, "stop_category");
    assert_unknown(&g, &id("stop-category"), "without a `stop_category`");
}

#[test]
fn push_button_colours_follow_their_function() {
    let mut g = golden();
    let (s1, s2) = (g.s1, g.s2);
    g.prop(s1, "actuator_colour", "RD");
    assert_fails(&g, &id("pushbutton-colour"), "=A1+CAB-S1 (start) is red; that function takes green or black or grey or white");
    g.prop(s1, "actuator_colour", "GN");
    g.prop(s2, "actuator_colour", "GN");
    assert_fails(&g, &id("pushbutton-colour"), "=A1+CAB-S2 (stop) is green");
    g.prop(s2, "actuator_colour", "BK");
    assert_passes(&g, &id("pushbutton-colour"));
    g.prop(s2, "function", "dance");
    assert_unknown(&g, &id("pushbutton-colour"), "\"dance\" is not one the colour table knows");
    g.clear(s2, "function");
    assert_unknown(&g, &id("pushbutton-colour"), "without a `function`");
}

#[test]
fn lamp_colours_follow_what_they_say() {
    let mut g = golden();
    let h1 = g.h1;
    g.prop(h1, "function", "abnormal");
    assert_fails(&g, &id("indicator-colour"), "(abnormal) is green; that meaning takes yellow");
    g.prop(h1, "function", "emergency");
    assert_fails(&g, &id("indicator-colour"), "takes red");
    g.prop(h1, "function", "fault");
    assert_unknown(&g, &id("indicator-colour"), "\"fault\" is not one the colour table knows");
    // A colourless lamp states its lens colour.
    let h1_sym = g.h1_sym;
    g.rekey(h1_sym, "iec/signalling/lamp");
    g.prop(h1, "function", "normal");
    assert_unknown(&g, &id("indicator-colour"), "Lamps without a `function` or a colour");
    g.prop(h1, "lens_colour", "GN");
    assert_passes(&g, &id("indicator-colour"));
}

#[test]
fn the_nameplate_lists_what_is_missing() {
    let mut g = golden();
    g.project_prop("machine.frequency_hz", None);
    g.project_prop("machine.full_load_current_a", None);
    assert_fails(&g, &id("nameplate"), "not stated: frequency, full-load current");
    // IEC 60204-1 does not ask for the short-circuit rating on the nameplate
    // data this rule reads.
    g.project_prop("panel.sccr_ka", None);
    let r = g.result(&id("nameplate"));
    assert!(!r.open[0].message.contains("short-circuit"));
}
