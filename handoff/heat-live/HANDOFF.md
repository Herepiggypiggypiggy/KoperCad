# Handoff: heat follows the cabinet

Code: `crates/novo-app/src/app/heat_live.rs` (`HeatState::sync_live`, `start_current`, `controls`), tests in `heat_live_tests.rs`.
A result stores the `Scenario` it solved; each frame the Cabinet and 3D views call `sync_live`, which rebuilds the scenario when the project revision changes and compares. Different scenario = out of date. Auto (default on up to 40 devices) waits 0.4 s of quiet, cancels a superseded run, then starts a Coarse to Fine run. Edits that do not change the scenario do not mark anything.
Snapshots now get their own id (before, every snapshot of a run shared one, so caches did not refresh as levels improved).
View3d.rs: one block before `heat_controls` (poll, sync, Simulate request, strip row). PR #714 adds a panel there; merge by moving the strip row into the panel.
