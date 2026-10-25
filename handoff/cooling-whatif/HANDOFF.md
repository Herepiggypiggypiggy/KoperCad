# Cooling what-if: handoff

Branch `claude/project-thread-qk7tim` (kopercad). Engine `crates/novo-thermal/src/whatif.rs`; app `crates/novo-app/src/app/heat_whatif.rs` (window, Apply, 3D preview helpers), hooks in `view3d.rs` (window, preview markers and ghost boxes, `whatif_apply` at the top of `view3d_show`), `heat.rs` (button, command), `cabinet.rs` (hand-off to 3D), `workbench.rs`, `cabinet_menu.rs`, `novo-workflow` commands and language.

Where the user finds it: Cabinet or 3D Heat strip "Cooling what-if", plate right-click "Cooling what-if…", palette "Cooling what-if wizard".

How it works: the window asks ambient and duty; "Try the fixes" starts `whatif::run` on a worker thread (stage name and run count shown, Stop keeps what was found). Result cards are ranked; the picked card is previewed in 3D (no storing). Apply: moves go through `apply_moves` as one undo step, openings through `add_pair` (undo in the Vents and fans list). A bigger enclosure is shown but cannot be applied (decision for Size the enclosure).

Traps: the window grows over several frames in tests (render 12 frames). `Wizard::start` uses `key_of` from heat_sweep for staleness. The wizard only lives in the 3D view; opening from the Cabinet switches workspace. Quick settings for tests: `whatif::Settings::quick()`. QA scenarios 400-402 (block 400+; 190-193 and up to 396 are taken).

Honesty: 2D model with planning values, ENCLOSURE_VALIDATED stays false; the window carries "MODEL, NOT MEASURED" and the model note; the first-ranked fix is the easiest one that clears every limit, not necessarily the best possible.
