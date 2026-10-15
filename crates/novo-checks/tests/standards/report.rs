//! The compliance report as lines, Markdown and CSV.

use super::*;
use novo_checks::standards::{NOTICE, StandardSet};

#[test]
fn the_printed_report_states_what_it_is_and_what_to_verify() {
    let g = golden();
    let lines = standards::lines(g.project(), &g.report());
    let text: Vec<&str> = lines.iter().map(|(_, l)| l.as_str()).collect();
    assert_eq!(text[0], "Standards compliance");
    assert_eq!(text[1], NOTICE);
    assert!(NOTICE.contains("does not certify"));
    assert!(NOTICE.contains("licensed copy"));
    assert!(text.iter().any(|l| l.contains("Edition: IEC 60204-1:2016 with Amendment 1:2021")));
    assert!(text.iter().any(|l| l.contains("[PASS] 5.3 Supply disconnecting device  (iec60204.supply-disconnect)")));
    assert!(text.iter().any(|l| l.starts_with("Short-circuit current rating")));
    assert!(text.iter().any(|l| l.contains("Panel rating: 65 kA")));
    assert!(text.iter().any(|l| l.contains("-K1 (contactor): 5 kA (marked), counts as 100 kA behind a current-limiting device")));
    assert!(lines.iter().any(|(bold, l)| *bold && l == "Values to confirm in your licensed copy"));
    assert!(text.iter().any(|l| l.starts_with("nfpa79.motor-branch-size 7.2: Percentages")));
}

#[test]
fn failures_unknowns_and_page_references_are_listed() {
    let mut g = golden();
    let (q1, w) = (g.q1_sym, g.power_wires[1]);
    g.rekey(q1, "iec/protection/fuse_3p");
    g.style(w, None, Some(2.5));
    let lines = standards::lines(g.project(), &g.report());
    let text: Vec<&str> = lines.iter().map(|(_, l)| l.as_str()).collect();
    assert!(text.iter().any(|l| l.contains("[FAIL] 5.3 Supply disconnecting device")));
    assert!(text.iter().any(|l| l.contains("=A1+FLD-M1 can be reached from the supply without passing a disconnecting device  (page 1)")));
    assert!(text.iter().any(|l| l.contains("Not evaluated: 1 wire has no colour set")));
}

#[test]
fn no_standard_means_no_report() {
    let mut g = golden();
    g.edit(|tx, _| standards::select(tx, &[]));
    assert!(standards::lines(g.project(), &g.report()).is_empty());
}

#[test]
fn markdown_has_a_table_per_standard_and_the_findings() {
    let mut g = golden();
    let q1 = g.q1_sym;
    g.rekey(q1, "iec/protection/fuse_3p");
    let md = standards::to_markdown(g.project(), &g.report());
    assert!(md.starts_with("# Standards compliance: Golden machine"));
    for set in StandardSet::ALL {
        assert!(md.contains(&format!("## {}:", set.short())));
    }
    assert!(md.contains("| FAIL | 5.3 | Supply disconnecting device | `iec60204.supply-disconnect` |"));
    assert!(md.contains("- **Fail:** =A1+FLD-M1 can be reached"));
    assert!(md.contains("| Component | Type | Rating | Source | Counts as |"));
}

#[test]
fn csv_has_a_row_per_finding_and_quotes_what_needs_it() {
    let mut g = golden();
    let q1 = g.q1_sym;
    g.rekey(q1, "iec/protection/fuse_3p");
    let csv = standards::to_csv(g.project(), &g.report());
    let mut lines = csv.lines();
    assert_eq!(lines.next(), Some("standard,rule,clause,title,result,kind,detail,page"));
    assert!(csv.contains("IEC 60204-1,iec60204.supply-disconnect,5.3,Supply disconnecting device,FAIL,finding,=A1+FLD-M1 can be reached from the supply without passing a disconnecting device,1"));
    // A passing rule has one row with no detail.
    assert!(csv.contains("IEC 60204-1,iec60204.pe-connection,8.2,Protective bonding of motors,PASS,,,"));
    // Commas in titles are quoted.
    let rows = csv.lines().count();
    assert!(rows > 40);
}

#[test]
fn every_set_is_described() {
    for set in StandardSet::ALL {
        assert!(set.needs_licensed_copy());
        assert!(!set.title().is_empty() && !set.edition().is_empty());
        assert_eq!(StandardSet::parse(set.id()), Some(set));
    }
    assert_eq!(StandardSet::parse("iso9001"), None);
}
