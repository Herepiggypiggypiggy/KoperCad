# CFD-F results and app: handoff

Where the user finds it: Cabinet, Heat chip, Simulate button (or command `heat.simulate`, Ctrl+K "heat").
Code: `crates/novo-heatview/src/cfd.rs` (pure helpers, tests in `crates/novo-heatview/tests/cfd.rs`), `crates/novo-app/src/app/heat_run.rs` (worker), `heat.rs` (cache from a solution, badges, legend, chips).
Flow: cabinet.rs builds `heat::Input`; on `take_request()` calls `heat::start_run` (bridge `scenario_from_project`, then `Runner` with `default_solver()`). `poll` in `chips` takes snapshots; `build_solved` slices at `front_plane_z` over the plate rectangle.
A result is "out of date" when `layout_key` of the Input differs from the run's.
Solver: `heat_run::default_solver` runs `novo_cfd::CabinetSolver` through the scheduler (`Plan::up_to(Fine)`); tests use the zero-delay `DemoLadder`. Progress events feed `heat_progress.rs` (bar and sparkline).
3D: `heat_3d.rs` builds overlays (slice, shell, streamlines, tracers) from the solution in the scene frame (`Place::world`); `view3d.rs` has the Heat strip and calls `View3d::set_overlays`, rebuilding only when the result or settings change, and the tracers every frame (not with reduced motion).
