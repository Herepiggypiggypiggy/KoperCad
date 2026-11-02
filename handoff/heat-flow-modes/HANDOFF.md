# Heat flow modes and the moving cloud: handoff

Code
- `crates/novo-heatview/src/advect.rs`: the transport model (`Advector`): frozen solved velocity, solids held at their solved temperature, first-order upwind advection, molecular plus turbulent diffusion, a film term (8 W/m2K) to solid faces, grid-boundary faces with an inward speed over 0.03 m/s bring ambient air. `match_rmse_k` says how far the model's air is from the solved air.
- `crates/novo-app/src/app/heat_modes.rs`: `HeatFlowMode`, `flux_map`, `path_rows` / `paint_paths` / `paths_table`, `MovingCloud` (time-boxed stepping, 9 ms a frame) and `MoveControls`.
- Hooks: `Heat3d` (`heat_flow`, `paths`, `cloud_moves`, `set_heat_flow`), `heat_carry::Carry::field`, `heat_cloud::mesh_scaled` (the moving cloud is drawn on the solved cloud's colour scale), `view3d.rs` (`heat_controls`, the animating branch), `view3d_heat.rs` (the Paths fold), `heat_legend.rs` (notes).

Honesty
- Flux modes: derived from solved T and u, no surface radiation. Paths: the solver's own split when present; otherwise plate share only.
- Moving cloud: not a second solve. The flow does not react to the warming, first-order upwind smears fronts, walls and devices are held at solved temperatures. It settles near, not on, the solver's picture; the strip prints the distance in K.

Tests: `advect.rs` (front speed, conservation, settling, stability), `heat_modes_tests.rs`. QA scenario 18150.
