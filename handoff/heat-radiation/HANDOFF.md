# Heat H2 radiation: handoff

Branch `claude/project-thread-86kxne` (repo kopercad). State: step 1 only, engine, not wired anywhere, so no UI entry yet (stated in the PR).

Run: `cargo test -p novo-thermal radiation`; clippy `-D warnings` clean.

Tests (9): unit squares parallel 0.19982 and perpendicular 0.20004 against closed forms, off-size boxes, cube 1/5 each face, closed and reciprocal box, small body in box vs two-surface formula (2%), close parallel plates vs infinite-plate formula (4%), black adiabatic sides at the fourth-power mean, low emissivity walls reflect.

Traps: polygon normal is the radiating side (use `facing`). No occlusion. Edge-sharing faces converge slowest (use div 6 to 8). Wait for H1 contract before touching `thermal.rs`.
Contacts: heat researcher thread cmsg_01TV2QyH4AYw9X2aTmgUsuhCL7FH6aNrvjWWRxkLjoj7gk; H3 PR #1857 (`outside.rs`), H7 design #1856.
