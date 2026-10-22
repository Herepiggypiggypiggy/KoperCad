# Handoff: live view operate and explain

Branch claude/project-thread-z38d09 (repo kopercad). Files: crates/novo-sim/src/explain.rs, crates/novo-sim/tests/explain.rs, crates/novo-app/src/app/live.rs (Operate list, Why section), qa/scenarios/07-live-operate-explain.qa.

Resume: `CARGO_TARGET_DIR=/tmp/kc-target cargo test -p novo-sim`, `cargo test -p novo-app --lib live`, `scripts/qa.sh --only live-operate` (Linux needs xvfb openbox mesa-vulkan-drivers libxkbcommon-x11-0).

Decisions: routes minimise open poles then poles, so the named cause is the cheapest repair. Potentials, not currents: no flow direction, so no animated pulses. The shipped example has no control circuit (no pushbuttons), so the Operate list is empty there; unit tests use a latch and a reversing interlock.
Reviewer: R4 (live/sim area). Merge lead by PR parity.
No-supply (C-56): a drawing where no net defines a potential shows a banner in the Live card; Why? answers 'No supply is defined' for any wire or device. Scenario qa/scenarios/08-live-no-supply.qa.
