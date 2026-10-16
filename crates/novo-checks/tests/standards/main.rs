//! Standards compliance rule sets (IEC 60204-1, NFPA 79, UL 508A): the golden
//! machine meets every rule, and each test breaks one thing.

mod golden;
mod iec;
mod nfpa;
mod report;
mod sccr;
mod ul;
mod waivers;

use golden::*;
use novo_checks::standards::{self, RuleStatus};

#[test]
fn the_golden_machine_meets_every_rule_with_nothing_unknown() {
    let g = golden();
    let report = g.report();
    let mut bad = Vec::new();
    for r in &report.results {
        let fine = matches!(r.status, RuleStatus::Pass | RuleStatus::NotApplicable);
        if !fine || !r.unknown.is_empty() || !r.open.is_empty() {
            bad.push(format!(
                "{} {:?} {:?} {:?}",
                r.rule.id,
                r.status,
                r.open.iter().map(|p| &p.message).collect::<Vec<_>>(),
                r.unknown.iter().map(|u| &u.why).collect::<Vec<_>>()
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    // Rules that apply to this machine are evaluated, not skipped.
    let evaluated = report
        .results
        .iter()
        .filter(|r| r.status == RuleStatus::Pass)
        .count();
    assert!(evaluated >= 30, "only {evaluated} rules evaluated");
    let _ = standards::RULES.len();
}

/// Asserts a rule has an open finding whose message contains `needle`.
pub fn assert_fails(g: &Golden, id: &str, needle: &str) {
    let r = g.result(id);
    assert_eq!(r.status, RuleStatus::Fail, "{id}: {:?}", r.unknown);
    assert!(
        r.open.iter().any(|p| p.message.contains(needle)),
        "{id}: no finding with {needle:?} in {:#?}",
        r.open.iter().map(|p| &p.message).collect::<Vec<_>>()
    );
}

/// Asserts a rule could not judge something and says why.
pub fn assert_unknown(g: &Golden, id: &str, needle: &str) {
    let r = g.result(id);
    assert!(
        matches!(r.status, RuleStatus::Partial | RuleStatus::NotEvaluated),
        "{id}: {:?} {:?}",
        r.status,
        r.open.iter().map(|p| &p.message).collect::<Vec<_>>()
    );
    assert!(
        r.unknown.iter().any(|u| u.why.contains(needle)),
        "{id}: no unknown with {needle:?} in {:#?}",
        r.unknown.iter().map(|u| &u.why).collect::<Vec<_>>()
    );
}

pub fn assert_passes(g: &Golden, id: &str) {
    let r = g.result(id);
    assert_eq!(
        r.status,
        RuleStatus::Pass,
        "{id}: {:?} {:?}",
        r.open.iter().map(|p| &p.message).collect::<Vec<_>>(),
        r.unknown.iter().map(|u| &u.why).collect::<Vec<_>>()
    );
}
