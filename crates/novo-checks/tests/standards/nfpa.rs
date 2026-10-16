//! NFPA 79 rules, one break at a time. Shared logic is exercised in iec.rs;
//! here are the rules and numbers that differ.

use super::*;
use novo_core::*;

fn id(name: &str) -> String {
    format!("nfpa79.{name}")
}

#[test]
fn nfpa_neutral_is_white_or_grey_not_blue() {
    let mut g = golden();
    let w = g.add_neutral(Some("WH"));
    assert_passes(&g, &id("neutral-colour"));
    g.style(w, Some("GY"), Some(1.0));
    assert_passes(&g, &id("neutral-colour"));
    g.style(w, Some("BU"), Some(1.0));
    assert_fails(
        &g,
        &id("neutral-colour"),
        "neutral net N is blue, which should be white or grey or white with blue stripe",
    );
    // The same wire is right under IEC 60204-1.
    assert_passes(&g, "iec60204.neutral-colour");
}

#[test]
fn nfpa_grounding_conductor_may_be_green_and_dc_power_is_blue() {
    let mut g = golden();
    let pe_wire = g.pe_wire;
    g.style(pe_wire, Some("GN"), Some(2.5));
    assert_passes(&g, &id("pe-colour"));
    assert_fails(&g, "iec60204.pe-colour", "protective-earth net PE");
}

#[test]
fn the_grounding_conductor_follows_the_branch_device_rating() {
    let mut g = golden();
    let (pe_wire, f1) = (g.pe_wire, g.f1);
    // 10 A branch: 14 AWG (2.08 mm²); 1.5 mm² is thinner.
    g.style(pe_wire, Some("GNYE"), Some(1.5));
    assert_fails(&g, &id("ground-size"), "the grounding conductor is 1.5 mm²");
    assert_fails(&g, "ul508a.ground-size", "needs at least 14 AWG");
    g.style(pe_wire, Some("GNYE"), Some(2.5));
    assert_passes(&g, &id("ground-size"));
    // A 60 A device wants 10 AWG (5.26 mm²).
    g.prop(f1, RATED_CURRENT_A, 60.0);
    assert_fails(&g, &id("ground-size"), "needs at least 10 AWG (5.26 mm²)");
    g.style(pe_wire, Some("GNYE"), Some(6.0));
    assert_passes(&g, &id("ground-size"));
    // Without a rated current on the device the wire cannot be sized.
    g.clear(f1, RATED_CURRENT_A);
    assert_unknown(&g, &id("ground-size"), "could not be sized");
}

#[test]
fn conductors_carry_what_their_protective_device_allows() {
    let mut g = golden();
    let f1 = g.f1;
    // 1.5 mm² rates as 16 AWG, 10 A at 60 °C terminals; a 15 A device is too much.
    for w in g.power_wires.clone() {
        g.style(w, Some("BK"), Some(1.5));
    }
    g.prop(f1, RATED_CURRENT_A, 15.0);
    // F1 is the motor's branch protector: the motor rule sizes it, and the
    // wire is judged against 125 % of the motor current (6.25 A here).
    assert_passes(&g, &id("conductor-ampacity"));
}

#[test]
fn a_compliant_motor_branch_is_not_a_false_ampacity_error() {
    let mut g = golden();
    let (f1, m1) = (g.f1, g.m1);
    // 11 A motor, time-delay fuse 175 % = 19.25 A, up to the next standard
    // rating, 20 A. 14 AWG carries 15 A: the fuse is above the wire, which
    // the motor branch rule allows; the wire still carries 125 % of 11 A.
    g.prop(m1, "flc_a", 11.0);
    g.prop(f1, RATED_CURRENT_A, 20.0);
    for w in g.power_wires.clone() {
        g.style(w, Some("BK"), Some(2.5));
    }
    assert_passes(&g, &id("motor-branch-size"));
    assert_passes(&g, &id("conductor-ampacity"));
    assert_passes(&g, "ul508a.conductor-ampacity");
    // The wire is still held to the motor's current: 1.5 mm² (10 A) is below 13.75 A.
    for w in g.power_wires.clone() {
        g.style(w, Some("BK"), Some(1.5));
    }
    assert_fails(&g, &id("conductor-ampacity"), "needs 13.75 A (125 %)");
}

#[test]
fn a_motors_conductors_carry_125_percent_of_its_full_load_current() {
    let mut g = golden();
    let m1 = g.m1;
    g.prop(m1, "flc_a", 14.0);
    // 125 % of 14 A is 17.5 A; 14 AWG carries 15 A at 60 °C.
    assert_fails(
        &g,
        &id("conductor-ampacity"),
        "a motor of 14 A full-load current needs 17.5 A (125 %)",
    );
    g.project_prop("standards.nfpa79.terminal_temp_c", Some(75.0.into()));
    assert_passes(&g, &id("conductor-ampacity"));
}

#[test]
fn ampacity_cannot_be_judged_for_unrated_devices_or_unsized_wires() {
    let mut g = golden();
    let (f1, w) = (g.f1, g.power_wires[2]);
    g.clear(f1, RATED_CURRENT_A);
    assert_unknown(&g, &id("conductor-ampacity"), "without a rated current");
    g.prop(f1, RATED_CURRENT_A, 10.0);
    g.style(w, Some("BK"), None);
    assert_unknown(&g, &id("conductor-ampacity"), "no cross-section");
    // A control wire thinner than the smallest AWG size is not guessed at.
    g.style(w, Some("BK"), Some(2.5));
    let c = g.control_wires[0];
    g.style(c, Some("BU"), Some(0.5));
    assert_unknown(
        &g,
        &id("conductor-ampacity"),
        "thinner than the smallest AWG size",
    );
}

#[test]
fn motor_branch_devices_are_sized_by_type_against_full_load_current() {
    let mut g = golden();
    let f1 = g.f1;
    // Time-delay fuse: 175 % of 5 A = 8.75 A, up to the next standard rating, 10 A.
    assert_passes(&g, &id("motor-branch-size"));
    g.prop(f1, RATED_CURRENT_A, 15.0);
    assert_fails(
        &g,
        &id("motor-branch-size"),
        "=A1+CAB-F1 is rated 15 A, above the 10 A allowed for =A1+FLD-M1 (175 % of 5 A",
    );
    // A non-time-delay fuse may be 300 %: 15 A.
    g.prop(f1, "protection_kind", "non_time_delay_fuse");
    assert_passes(&g, &id("motor-branch-size"));
    // A fuse that does not say what it is cannot be sized.
    g.clear(f1, "protection_kind");
    assert_unknown(
        &g,
        &id("motor-branch-size"),
        "state its type (`protection_kind`",
    );
    // A breaker is an inverse-time breaker unless it says otherwise: 250 %.
    let f1_sym = g.f1_sym;
    g.rekey(f1_sym, "iec/protection/mcb_3p");
    g.prop(f1, RATED_CURRENT_A, 12.0);
    assert_passes(&g, &id("motor-branch-size"));
    g.prop(f1, RATED_CURRENT_A, 20.0);
    assert_fails(&g, &id("motor-branch-size"), "above the 15 A allowed");
}

#[test]
fn motor_branch_size_needs_the_motors_full_load_current() {
    let mut g = golden();
    let m1 = g.m1;
    g.clear(m1, "flc_a");
    assert_unknown(
        &g,
        &id("motor-branch-size"),
        "state the motor's full-load current",
    );
}

#[test]
fn the_panel_rating_must_cover_the_available_fault_current() {
    let mut g = golden();
    g.project_prop("supply.short_circuit_ka", Some(100.0.into()));
    assert_fails(
        &g,
        &id("sccr"),
        "rating is 65 kA, below the 100 kA available",
    );
    g.project_prop("supply.short_circuit_ka", None);
    assert_unknown(&g, &id("sccr"), "available fault current");
    g.project_prop("supply.short_circuit_ka", Some(10.0.into()));
    assert_passes(&g, &id("sccr"));
}

#[test]
fn an_undetermined_panel_rating_is_not_a_pass() {
    let mut g = golden();
    let k1 = g.k1;
    g.clear(k1, "sccr_ka");
    g.project_prop("panel.sccr_ka", None);
    assert_unknown(&g, &id("sccr"), "not determined");
    // Unless a known component already falls short of the supply.
    g.project_prop("supply.short_circuit_ka", Some(100.0.into()));
    assert_fails(
        &g,
        &id("sccr"),
        "A component rated 65 kA is already below the 100 kA",
    );
}

#[test]
fn nfpa_nameplate_includes_the_short_circuit_rating() {
    let mut g = golden();
    g.project_prop("panel.sccr_ka", None);
    assert_fails(&g, &id("nameplate"), "short-circuit current rating");
}
