# Evidence for PR #1887 (heat scenarios)

Head: 973a8e44cc18660d0e8ed50f33a3666efef3a63c (merge of origin/main b3f974798 into claude/project-thread-stctor-heat). The binary was built from this head after `touch crates/novo-app/build.rs`; report.md says "source at git 973a8e44cc" and has no older-binary warning. Main contained in the binary: b3f974798 (Heat H8, #1861).

Scenarios: the 21 changed files in qa/scenarios (diff against origin/main) plus 112 and 16176, run in one folder at both sizes: 40 runs, 34 passed, 6 failed. Pictures: /mnt/project-files/qa/pr-heat-973a8e44/shots (report.md refers to them as shots/).

## Stays red and why (real app bugs, entries in /mnt/project-files/qa/app-bugs-main.md)
- 230-workpack-out-of-date-after-cooler-layout, 1366x768: docked Cooler layout window is clipped, Apply the moves is never drawn, so the layout is never applied.
- 380-3d-cooling-advice-2d-vs-3d-comparison, 1366x768 and 1920x1080: docked Cooling advice window is clipped, "Model comparison" never on screen.
- 16176-heat-refuses-20m-enclosure, both sizes: "Cannot simulate" notice is cleared every frame by clear_cannot_simulate.

## Flakes (scenario timing, not fixed)
- 270-3d-heat-strip-two-layer-rows-short-window at 1280x688: failed in 2 of 4 runs (first evidence run, and a rerun after this one); passed in the evidence run in this folder and at 1366x768 every time. Failure is "click_text Quick": the automatic Heat run restarts after the Cancel/Auto recipe (Auto still checked, Cancel visible, Quick radio disabled). Treat as an intermittent red; not a pass.
- 112-3d-heat-panel-row-and-advice at 1366x768: failed once in the evidence run at the same step (Quick disabled while the auto run starts); the rerun passed 3 of 3 (both sizes). The scenario is unchanged by this PR.

## Not tested
Windows; sizes other than those the scenarios declare; the other scenarios in the repository (only the heat package files were run); app code was not changed or tested by cargo test.
