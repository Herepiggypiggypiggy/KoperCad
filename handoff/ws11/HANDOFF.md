# WS-11 Terminals: handoff (rewrite whole file each time)

Updated: 2026-10-15 18:30 UTC

## State
- Everything WS-11 opened is merged: #30 and its follow-ups, #85, #95, #104, #122, #129, #135, #139, #147, #251, #299 and #330 (the Terminals drawer's Kind, Jumper and Cable columns). Nothing of WS-11 is open apart from the storage robustness PR (see PROGRESS.md).
- Repo is `kopercad` on GitHub; the old name `NovoElectric` still resolves. Shared branch `claude/project-thread-xqeqyw`. After a squash merge, restart the next PR from main and push it as a fast-forward: commit-tree the new tree with parents (old branch head, new work); never force-push.
- GitHub Actions: Linux runs per PR. Local fmt, clippy and tests are the signal I trust; build with `CARGO_PROFILE_DEV_DEBUG=0` or the container disk fills at about 40 GB.

## Code
- crates/novo-terminals/src: strip.rs (rows, edits, `connect_terminals` by #249), style.rs (`connected()` picks `terminal_bridged` for jumpered terminals), place.rs, geometry.rs (`redraw`, `symbol_pins`), screen.rs (connection screen incl. cables), suggest.rs (dismissals as `Dismissal` records), check.rs, naming.rs, occurrence.rs.
- The bridge bar: `bridge_bars` in crates/novo-canvas/src/model.rs draws one heavy bar per run of connected terminals from first to last lead; `terminal_bridged` and `terminal_end_bridged` carry only the dot.
- Drawer: crates/novo-app/src/app/terminals.rs `strip_table` (columns in `STRIP_HEADS`).

## Resume
```
git fetch origin && git checkout -B claude/project-thread-xqeqyw origin/main
CARGO_PROFILE_DEV_DEBUG=0 cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
After each push, send the head SHA to the main reviewer (session_01QSnXFBDRyvwwzhT1S1gFGN) and the lead (session_01FiLvRBKEcNNHbQEGhYyL1S). No pushes after READY.

## Half-done or waiting
- Storage robustness (lead's item): new tests in crates/novo-storage/tests/robustness.rs. They found no bug: a v1 file upgrades with a `.v1.bak`, a cut file never panics, a crash image mid-save rolls back, an older recovery file is still offered. A migration that fails midway is not tested because `migrate` is crate-private; that needs a unit test inside novo-storage (the storage owner's call).
- Parked: pitch of connected terminals in symbols is 8 mm (the library grid is 4 mm), the bar overhang is solved; nothing else outstanding for the bridge.

## People
Roster with session ids: /mnt/project-files/coordination/threads.md. Main reviewer: session_01QSnXFBDRyvwwzhT1S1gFGN. Lead: session_01FiLvRBKEcNNHbQEGhYyL1S. Layout: session_01UXbCeQW22y6tv74F2Xvbvm.

## Decisions and traps
- No novo-core API change; edits happen inside a `novo_core::Tx` so each is one undo step.
- A symbol with several terminals needs its bridges at insert time (`TerminalLevels` error otherwise).
- Do not edit `[workspace.dependencies]`; keep canvas free of a novo-terminals dependency (that is why `bridge_bars` lives in the canvas).
