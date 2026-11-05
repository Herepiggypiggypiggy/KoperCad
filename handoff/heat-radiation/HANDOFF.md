# Heat H2 radiation: handoff

Branch `claude/project-thread-86kxne` (repo kopercad). State: engine (`radiation.rs`) plus its wiring into the 2D films and the 3D device films behind `Params::radiation_geometry`, which is off by default (no existing result moves). No UI entry yet: the Heat layer radiation split is lane H7.

Run: `cargo test -p novo-thermal radiation`; clippy `-D warnings` clean.

Tests (12 unit tests in `radiation.rs` plus 5 in `tests/radiation_geometry.rs` for the wiring): unit squares parallel 0.19982 and perpendicular 0.20004 against closed forms, off-size boxes, cube 1/5 each face, closed and reciprocal box, small body in box vs two-surface formula (2%), close parallel plates vs infinite-plate formula (4%), black adiabatic sides at the fourth-power mean, low emissivity walls reflect.

Traps: polygon normal is the radiating side (use `facing`). Occlusion by box blockers is approximate (64 sight lines per emitting point, 2 x 2 cells). Edge-sharing faces converge slowest (use div 6 to 8). Wait for H1 contract before touching `thermal.rs`.
Contacts: heat researcher thread cmsg_01TV2QyH4AYw9X2aTmgUsuhCL7FH6aNrvjWWRxkLjoj7gk; H3 PR #1857 (`outside.rs`), H7 design #1856.
