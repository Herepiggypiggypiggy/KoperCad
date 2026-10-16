//! The compliance document: verdicts, evidence, fix hints, planning values.

use super::*;
use novo_checks::standards::document::{self, Verdict};

#[test]
fn passes_carry_evidence_and_the_golden_machine_has_no_fail() {
    let g = golden();
    let doc = document::build(g.project(), &g.report());
    assert_eq!(doc.sections.len(), 3);
    for s in &doc.sections {
        assert_eq!(s.count(Verdict::Fail), 0, "{}", s.set.short());
        assert_eq!(s.count(Verdict::NotCheckable), 0);
    }
    let disc = doc.sections[0]
        .requirements
        .iter()
        .find(|r| r.id == "iec60204.supply-disconnect")
        .unwrap();
    assert_eq!(disc.verdict, Verdict::Pass);
    assert!(disc.checked > 0);
    assert!(
        disc.evidence
            .iter()
            .any(|e| e.contains("disconnecting device")),
        "{:?}",
        disc.evidence
    );
}

#[test]
fn a_pass_on_a_planning_value_is_marked_not_silent() {
    let g = golden();
    let doc = document::build(g.project(), &g.report());
    let amp = doc
        .sections
        .iter()
        .flat_map(|s| &s.requirements)
        .find(|r| r.id == "nfpa79.conductor-ampacity")
        .unwrap();
    assert_eq!(amp.verdict, Verdict::Pass);
    assert!(amp.rests_on_planning_values());
    let html = document::to_html(&doc, "");
    assert!(html.contains("planning value"));
    assert!(html.contains("awg.ampacity"));
    // A structural rule is not marked.
    let colour = doc.sections[0]
        .requirements
        .iter()
        .find(|r| r.id == "iec60204.pe-colour")
        .unwrap();
    assert!(!colour.rests_on_planning_values());
}

#[test]
fn failures_say_where_to_fix_and_unchecked_things_say_so() {
    let mut g = golden();
    let (q1, w) = (g.q1_sym, g.power_wires[1]);
    g.rekey(q1, "iec/protection/fuse_3p");
    g.style(w, None, Some(2.5));
    let doc = document::build(g.project(), &g.report());
    let failing: Vec<_> = doc
        .sections
        .iter()
        .flat_map(|s| &s.requirements)
        .filter(|r| r.verdict == Verdict::Fail)
        .collect();
    assert!(!failing.is_empty());
    let html = document::to_html(&doc, "2026-10-16");
    assert!(html.contains("Where to fix"));
    assert!(html.contains("2026-10-16"));
    assert!(html.contains("does not certify"));
    assert!(!html.to_lowercase().contains("is compliant"));
    for r in failing {
        assert!(!r.fix.is_empty());
        assert!(!r.findings.is_empty());
    }
}

#[test]
fn html_is_escaped_and_lists_what_is_not_covered() {
    let g = golden();
    let mut doc = document::build(g.project(), &g.report());
    doc.project_name = "A <b>&</b> B".into();
    let html = document::to_html(&doc, "");
    assert!(html.contains("A &lt;b&gt;&amp;&lt;/b&gt; B"));
    assert!(!html.contains("<b>&</b> B"));
    assert!(html.contains("Not covered"));
    assert!(html.contains("IEC 60364-5-52"));
}

#[test]
fn project_overrides_are_listed_as_the_engineers_own() {
    let mut g = golden();
    g.project_prop(
        "standards.iec60204.max_control_ac_v",
        Some(novo_core::Value::Float(250.0)),
    );
    let doc = document::build(g.project(), &g.report());
    assert_eq!(
        doc.sections[0].overrides,
        vec!["standards.iec60204.max_control_ac_v".to_owned()]
    );
}
