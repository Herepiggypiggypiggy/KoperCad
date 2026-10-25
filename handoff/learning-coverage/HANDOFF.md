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
