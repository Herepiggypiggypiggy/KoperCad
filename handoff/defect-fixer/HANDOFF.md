# Audit defect fixer: handoff

* Workflow: `scripts/qa.sh run --only <text> --out /mnt/project-files/qa/<name>`; scenarios in `qa/scenarios`. Disk is small: `export CARGO_PROFILE_DEV_DEBUG=0`, test only the crates you changed, `scripts/free-disk.sh` when full.
* Before blaming data, probe the model: the auditor's "25 A" and "Trip curve C" were greyed hints, not stored values.
* A READY PR gets no more pushes except a lead's merge-of-main request; follow-ups go in a new PR from main. Force pushes are blocked: new branch names.
* `add_generated_pages` also runs when a page kind is turned back on mid-session (`project_rules.rs`); it resets history only when the history was empty (the open path).
* Opening the same path in the same window does not reload history: use a fresh `Drive` in tests that need a real reopen.
* Adding a PLC card uses the `-A` tag letter like an imported I/O list; the PLC I/O lesson fixture keeps its hand-made analog card as -K1.
