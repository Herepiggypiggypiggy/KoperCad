//! The quoted accuracy statement must not claim more than the suite measured.

use novo_cfd::accuracy::flow_core_statement;

#[test]
fn statement_is_filled_and_honest() {
    let s = flow_core_statement();
    assert!(!s.headline.is_empty() && !s.limits.is_empty());
    assert!(
        s.benchmarks
            .iter()
            .all(|b| b.error.is_finite() && b.error >= 0.0)
    );
    // the headline says "about 3%" for velocity: the worst listed velocity error is 3.06%
    let worst_velocity = s
        .benchmarks
        .iter()
        .filter(|b| b.name.contains("velocity"))
        .map(|b| b.error)
        .fold(0.0, f64::max);
    assert!(worst_velocity < 0.035, "{worst_velocity}");
    let worst_nusselt = s
        .benchmarks
        .iter()
        .filter(|b| b.name.contains("Nusselt"))
        .map(|b| b.error)
        .fold(0.0, f64::max);
    assert!(worst_nusselt < 0.01, "{worst_nusselt}");
}
