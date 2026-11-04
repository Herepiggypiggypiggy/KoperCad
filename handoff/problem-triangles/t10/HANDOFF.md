# T10 handoff

State: PR open from branch `claude/project-thread-51hfmo`; 16 of the 89 rows of sites/sim.md closed; the rest listed in ROADMAP.md.

How it works
- `app/sim_flags.rs`: pure adapters (`note_drafts`, `finding_draft`, `failure_draft`, `pin_problem_draft`, `broken_binding_draft`, `motor_start_draft`, `motor_unmodelled_draft`) build `FlagDraft`s of source `Sim`; `set_flags`/`set_notes` store them per window in `wb.flags.sim` and rebuild the `Sim` source in the store (no churn when the drafts are equal); `strip` draws a list of flags as triangle + short title (click opens the shared popover); `locate_flag` jumps to a device anchor.
- Windows keep their own `notice` lists as data; only the drawing changed. Rule key = `<window>.<kind>.<hash of text>` so two reasons are two flags and the same note in two windows is two flags.
- Findings of ledger F10: novo-sim `PinProblem` was shown only as a red "!" with hover text; now typed flags (red, signal row plus device). novo-drive `ResistorFlag` has no consumer and no input data; not shown (see ROADMAP).

Traps
- The Issues pill and Flags list still read `wb.problems`, not the store, so sim flags are not in the top bar count yet (the count unification is outside this lane).
- Marks on the schematic draw only Symbol/Wire anchors, so the Device anchor of a sim flag is used by Locate only.
- Controller card flags clear while the Bindings fold is closed (drafts come from drawing the rows).

Tests: `cargo test -p novo-app --lib sim_flags`; QA scenarios 9500 (1366x768) and 9501 (1920x1080).
