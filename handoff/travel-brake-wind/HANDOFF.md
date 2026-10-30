# Travel brake, inertia and wind: handoff

Branch `claude/project-thread-3dvbdi` in kopercad. See ROADMAP.md for scope.

Where the user finds it: select a travel motor (one with "Moving mass" set), property section "Travel drive": brake, wind, gust and grip rows. Simulate > Drive run shows brake open/closed, wind force, last stop distance and a findings list; Simulate > Hoist in 3D shows a status line for the picked axes.

Code: `crates/novo-machine/src/travel_loop.rs` (Wind, Gust, Grip, brake in `step_module`, findings), `travel_spec.rs` (keys `trav_wind_*`, `trav_gust_*`, `trav_brake_*`, `trav_driven_share`, `trav_adhesion`), `crates/novo-app/src/app/drive_run.rs` (reading, findings, `travel_status`), `hoist_3d.rs` (status line). Fixture: `novo_tests::travel::travel_with_brake_and_wind`, `qa/fixtures/travel-brake-wind.novo`, QA 1790 and 1790b.

Tests: `cargo test -p novo-machine --test travel_loop` has the closed-form stop distance (delay, then brake plus rolling), the skid cap, the wind formula, the gust trapezoid, a brake holding against wind, determinism.

Traps:
- A 60 N m brake on a 5 t crane exceeds the rail grip (3.7 kN with the defaults): the run says so and caps the stop. Tests of the brake use 15 N m.
- Brake torque is on the motor shaft; force at the wheel = T / (r/i) x efficiency.
- Cutting the mains (not the enable) makes the drive output vanish at once; the enable input still ramps down.
- The brake without `trav_brake_relay` and `trav_brake_feed` follows R2 directly (ideal wiring).
Model, not measurement: wind is a uniform speed with one Cd and area, grip is one coefficient, the brake delays are the numbers you enter. Not a safety function.
