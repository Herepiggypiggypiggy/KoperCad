# Scale pass two (follow-up to #1414)

Branch `claude/project-thread-d7x16s`. Works the ranked leftovers of RESULTS.md.

1. Wire jobs cache: `wire_cut_jobs_with` goes through `Project::derived`, so routing every wire happens once per revision and ferrule standard. The bench packet, standards report, handover and the reports that quote the cut list share it. Test: `wire_jobs_are_routed_once_per_revision` (novo-tests/tests/scale_budget.rs), a ratio of second ask to first, no clock limit.
2. Reports list counts: `reports_view.rs` `list_counts` works out the 12 row counts on a worker thread from a project clone; the list shows "-" or the last counts until the answer arrives and repaints. The selected report is still built on the UI thread (needed to draw it).
3. `Project::pages_in_order` keeps the sorted ids in `derived` per revision (internal only; no public API change). Every op clears derived caches, so a transaction never sees a stale order.

Resume: `cargo test -p novo-tests --test scale_budget`, `cargo test -p novo-app --lib scale_tests` (release for real numbers).
Not done: heat and CFD at large cabinets (unchanged from SCALE-1).

## Review round 1 (R5, Reviewer A)
* Counts shown only for the exact project state; "-" otherwise. A worker that dies is replaced. One worker at a time.
* Only the project's own ferrule standard is cached.
* Test `list_counts_show_only_the_current_state_and_equal_the_synchronous_ones` (reports_view.rs): equals sync `headline_count`, nothing shown after an edit or undo until the worker answers.
* QA scenario `qa/scenarios/200-reports-list-counts.qa` added, NOT yet run: this cloud box has no openbox. Needs a run on a box with the QA harness, evidence to /mnt/project-files/qa/ with HEAD.txt.
* Speed: release timings are NOT measured. "Routed once" and "off the UI thread" are by construction and test; any "faster" is unmeasured.
