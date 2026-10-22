//! Every shipped scenario, run with the scripted model: the tools, the draft,
//! the program's own checks, the one-step undo and the wording rules.
//! Not a measure of a real model (see `novo_ai::eval`).

use novo_ai::eval::{self, Mode};

#[test]
fn every_scenario_passes_with_the_scripted_model() {
    let mode = Mode::Scripted;
    let reports: Vec<_> = eval::builtin()
        .iter()
        .map(|s| eval::run(s, &mode))
        .collect();
    let text = eval::markdown(&mode, &reports);
    if let Some(dir) = std::env::var_os("ASK_CLAUDE_EVAL_REPORT") {
        std::fs::write(std::path::Path::new(&dir).join("scripted-report.md"), &text).unwrap();
    }
    let failed: Vec<&str> = reports
        .iter()
        .filter(|r| !r.passed())
        .map(|r| r.id.as_str())
        .collect();
    assert!(failed.is_empty(), "failed: {failed:?}\n{text}");
}
