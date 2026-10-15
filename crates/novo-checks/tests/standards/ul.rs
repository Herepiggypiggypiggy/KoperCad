//! UL 508A rules that are not the SCCR calculation.

use super::*;
use novo_core::*;

fn id(name: &str) -> String {
    format!("ul508a.{name}")
}

#[test]
fn overload_relays_are_set_within_the_motor_rating() {
    let mut g = golden();
    let f2 = g.f2;
    assert_passes(&g, &id("overload-setting"));
    // 125 % of 5 A is 6.25 A.
    g.prop(f2, "setting_a", 6.5);
    assert_fails(
        &g,
        &id("overload-setting"),
        "=A1+CAB-F2 is set to 6.5 A, above 125 % of =A1+FLD-M1's 5 A (6.25 A)",
    );
    g.prop(f2, "setting_a", 6.25);
    assert_passes(&g, &id("overload-setting"));
    g.clear(f2, "setting_a");
    assert_unknown(
        &g,
        &id("overload-setting"),
        "state its setting (`setting_a`)",
    );
}

#[test]
fn the_lower_limit_applies_to_motors_without_the_service_factor() {
    let mut g = golden();
    let (f2, m1) = (g.f2, g.m1);
    g.prop(f2, "setting_a", 6.0); // between 115 % (5.75) and 125 %
    assert_passes(&g, &id("overload-setting")); // service factor 1.15 stated
    g.clear(m1, "service_factor");
    assert_unknown(
        &g,
        &id("overload-setting"),
        "state the motor's `service_factor` or `temp_rise_c`",
    );
    g.prop(m1, "service_factor", 1.0);
    assert_fails(
        &g,
        &id("overload-setting"),
        "above 115 % of =A1+FLD-M1's 5 A (5.75 A)",
    );
    // A marked temperature rise of 40 °C or less allows 125 %.
    g.prop(m1, "temp_rise_c", 40.0);
    assert_passes(&g, &id("overload-setting"));
}

#[test]
fn ul_motor_branch_rule_matches_nfpa() {
    let mut g = golden();
    let f1 = g.f1;
    assert_passes(&g, &id("motor-branch-size"));
    g.prop(f1, RATED_CURRENT_A, 20.0);
    assert_fails(&g, &id("motor-branch-size"), "above the 10 A allowed");
}

#[test]
fn ul_nameplate_includes_the_rating() {
    let mut g = golden();
    g.project_prop("machine.phases", None);
    assert_fails(&g, &id("nameplate"), "number of phases");
}

#[test]
fn a_set_not_selected_is_not_evaluated() {
    let mut g = golden();
    g.edit(|tx, _| standards::select(tx, &[standards::StandardSet::Iec60204]));
    let report = g.report();
    assert!(
        report
            .results
            .iter()
            .all(|r| r.rule.set == standards::StandardSet::Iec60204)
    );
    assert!(report.sccr.is_none());
}
