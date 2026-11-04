//! Census of the problem sentences left in the terminals, cables, PLC,
//! numbering and cross reference files (problem triangles, lane T8).
//!
//! The ceiling is this area's own number, in this file only, so two lane
//! pull requests never edit the same line. It only goes down; the closing
//! lane (T15) sets it to zero. Job failures ("Could not ...: {e}") are still
//! toasts here and are counted until the job chip exists (T12, T15).

use std::path::Path;

/// The files of the area (`handoff/problem-triangles/sites/terminals.md`).
const FILES: [&str; 13] = [
    "terminals.rs",
    "terminal_edit.rs",
    "cable_cores.rs",
    "cable_controls.rs",
    "plc.rs",
    "plc_card.rs",
    "plc_link.rs",
    "plc_list.rs",
    "plc_run.rs",
    "plc_style.rs",
    "numbering.rs",
    "device_renumber.rs",
    "xref.rs",
];

/// Error and warning toasts. Was 46 before lane T8.
const TOAST_CEILING: usize = 33;
/// Warning or error coloured sentences. Was 8 before lane T8.
const COLOURED_CEILING: usize = 0;

fn count(patterns: &[&str]) -> usize {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app");
    FILES
        .iter()
        .map(|f| {
            let text = std::fs::read_to_string(dir.join(f)).unwrap_or_default();
            // Tests and comments do not put sentences in front of anyone.
            let code = text.split("#[cfg(test)]").next().unwrap_or_default();
            code.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .map(|l| patterns.iter().filter(|p| l.contains(*p)).count())
                .sum::<usize>()
        })
        .sum()
}

#[test]
fn the_toast_count_of_the_area_only_goes_down() {
    let n = count(&["Level::Error", "Level::Warning"]);
    assert!(
        n <= TOAST_CEILING,
        "{n} error or warning toasts in the terminals area, the ceiling is {TOAST_CEILING}: \
         use a triangle (flags_inline) instead"
    );
}

#[test]
fn the_area_has_no_coloured_problem_sentences() {
    let n = count(&[
        "ui_palette().warning",
        "ui_palette().error",
        "colored_label",
    ]);
    assert!(
        n == COLOURED_CEILING,
        "{n} coloured problem sentences in the terminals area, the ceiling is {COLOURED_CEILING}"
    );
}
