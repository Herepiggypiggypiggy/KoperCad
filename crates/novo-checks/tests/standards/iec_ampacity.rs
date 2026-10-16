//! IEC 60204-1 conductor current-carrying capacity: the installation method
//! is read, never guessed, and the table values are planning values.

use super::*;
use novo_core::*;

const ID: &str = "iec60204.conductor-ampacity";

#[test]
fn the_golden_machine_passes_with_evidence_naming_the_conditions() {
    let g = golden();
    assert_passes(&g, ID);
    let r = g.result(ID);
    assert!(
        r.evidence
            .iter()
            .any(|e| e.contains("PVC, method C") && e.contains("40 °C ambient")),
        "{:?}",
        r.evidence
    );
}

#[test]
fn without_an_installation_method_the_wires_are_not_rated() {
    let mut g = golden();
    g.project_prop("machine.installation_method", None);
    assert_unknown(&g, ID, "no installation method");
    // A method on the wire's own property is enough for that wire only.
    let w = g.power_wires[4];
    g.prop(w, "installation_method", "C");
    assert_unknown(&g, ID, "no installation method");
}

#[test]
fn the_method_ambient_and_insulation_change_the_verdict() {
    let mut g = golden();
    let (m1, w) = (g.m1, g.power_wires[4]);
    // 2.5 mm², three loaded conductors, PVC, 40 °C: method C carries about
    // 20.9 A, method B2 about 15.2 A.
    g.prop(m1, "flc_a", 20.0);
    assert_passes(&g, ID);
    g.prop(w, "installation_method", "B2");
    assert_fails(&g, ID, "PVC, method B2");
    g.prop(w, "installation_method", "C");
    g.project_prop("machine.ambient_c", Some(Value::Float(30.0)));
    g.prop(m1, "flc_a", 23.0);
    assert_passes(&g, ID);
    g.project_prop("machine.ambient_c", Some(Value::Float(50.0)));
    assert_fails(&g, ID, "at 50 °C ambient");
    g.project_prop("machine.ambient_c", Some(Value::Float(30.0)));
    g.prop(w, "grouped_circuits", 6i64);
    assert_fails(&g, ID, "6 grouped circuits");
    g.clear(w, "grouped_circuits");
    for i in [4, 9, 14] {
        let wire = g.power_wires[i];
        g.prop(wire, "insulation", "XLPE");
    }
    g.prop(m1, "flc_a", 29.0);
    assert_passes(&g, ID);
}

#[test]
fn unlisted_sections_and_methods_are_not_checkable() {
    let mut g = golden();
    let w = g.power_wires[4];
    g.prop(w, "installation_method", "D");
    assert_unknown(&g, ID, "not one of B1, B2 or C");
    g.prop(w, "installation_method", "C");
    g.style(w, Some("BK"), Some(2.0));
    assert_unknown(&g, ID, "the tables do not list");
}

#[test]
fn the_report_marks_the_verdict_as_resting_on_planning_values() {
    let g = golden();
    let doc = standards::document::build(g.project(), &g.report());
    let r = doc.sections[0]
        .requirements
        .iter()
        .find(|r| r.id == ID)
        .unwrap();
    assert!(r.rests_on_planning_values());
    assert!(r.basis.iter().any(|b| b.id == "iec.ampacity-tables"));
    let html = standards::document::to_html(&doc, "");
    assert!(html.contains("iec.ampacity-tables"));
}
