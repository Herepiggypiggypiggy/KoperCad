# Learning and example coverage: handoff

## State (2026-10-25)
- Branch `claude/learning-backlog-2` (from main bbc6316): three new lessons (`emc-net-classes`, `emc-on-the-plate`, `build-order-3d`), a new step in `live-view`, anchors, QA walks `qa/scenarios/200` to `203`. PR number is in PROGRESS.md.
- App changes made so the lessons can tick: the EMC panels scroll the step's target into view beside the lesson card (`emc_schematic.rs`, `emc_view.rs`, reveal fields); the plate EMC panel stays clear of the card (`above_card`) and starts at the top on the plate; `SelectOp.pick` (`{ select = "a~b", pick = true }`) picks a wire like a click; Build order strip buttons raise anchors; Live card has `live.current_paths`.
- Sample projects: `emc-net-classes/project.json` is the EMC example (test in `emc_demo.rs`, `UPDATE_LESSONS=1`), `emc-on-the-plate/project.json` is the shields fixture (test in `crates/novo-tests/tests/lessons/emc.rs`). `build-order-3d` runs on `@example`.

## Resume
```
git fetch origin && git checkout -B claude/learning-backlog-2 origin/main
export CARGO_TARGET_DIR=/tmp/ctarget CARGO_INCREMENTAL=0
cargo test -p novo-app --lib every_shipped_lesson_replays   # about 2 minutes
cargo test -p novo-tests --test lessons
scripts/qa.sh run qa/scenarios/200-learn-emc-net-classes.qa --out /tmp/q   # set KOPERCAD_BIN_DIR=/tmp/ctarget/debug
```
Generated docs: `UPDATE_ANCHORS_DOC=1 UPDATE_UI_NAMES=1 cargo test -p novo-app --lib -- learn`.

## Notes for the next person
- A step that points into a side panel needs the panel to be tall enough beside the lesson card, or a `reveal` that scrolls the target into view; the replay gate fails with "the check did not tick" when the control is under the card.
- A wire selected by a solve opens the Inspector over the EMC panel; use `pick = true`.
- QA numbers: 200 to 209 are this thread's. Check open pull requests before adding.


## 2026-10-26 additions
- Lessons `work-cards` and `emc-fix-it`; QA 204 and 205 (block 200 to 209 is this lane's). Anchors: `build_order.work_cards`, `build_order.print_step`, `emc.check.preview` (first repair the plate can draw), `emc.check.apply`, `emc.check.show_plate`, `emc.whatif.before`, `emc.whatif.after`, `emc.whatif.apply`.
- "Do it for me" in the live app only runs commands and selects, not `ui_click`: QA walks must click chips and buttons for real.
- The shield repair card is drawn under two findings (EMC-04 and EMC-05); in QA use `click_text "On the plate" Button #2`.
- A lesson step over 500 characters makes the catalog reject the whole lesson without failing the replay gates: run `cargo test -p novo-learn`.
- Open: the full 57 lesson walk for existing-lesson drift was cut short (a container restart after 32 runs). Not redone.
- Lesson `cooling-what-if` (Cabinet track): Cabinet > Heat strip > Cooling what-if opens the wizard in 3D. Anchors `heat.whatif.go`, `heat.whatif.apply`. QA 206. The two run-dependent steps are `manual` in the replay gate. Not walked: Cooling advice, Cooler layout (named in one step), the 3D Simulate run.
- Lesson `cooling-advice` (Cabinet track, order 97): Cabinet > Heat strip > Cooling advice opens the window over the plate and starts the sweep. Anchor `heat.advice.add`. QA 207. "Cooling advice" and "Add" steps are `manual` in the replay gate. Still to do from the coordinator: Cooler layout, 3D Simulate run, Test tab (#1526 merged), Kitting (not merged).
