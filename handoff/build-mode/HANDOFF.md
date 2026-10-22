# Build mode handoff

Branch `claude/project-thread-naa4la` (kopercad). One PR, see PROGRESS.md for the head SHA.

State: engine, screen, tests and QA scenario done; fmt, clippy -D warnings and
`cargo test -p novo-commissioning -p novo-workflow -p novo-app` pass.

Resume: read docs/BUILD-MODE.md, then `crates/novo-commissioning/src/build.rs` and
`crates/novo-app/src/app/build_mode.rs`. QA: `scripts/qa.sh run qa/scenarios/71-build-mode.qa`
(needs xvfb, openbox, mesa-vulkan-drivers, libxkbcommon-x11-0).

Decisions and traps:
- The plan reuses `novo_layout::assembly::steps` for rails/ducts/devices/labels order and `wire_cut_jobs` for wires; Drill and Prepare steps are added in front. Do not change novo-layout's order from here.
- Tick keys are (cabinet, phase, subject), never step numbers. A wire used by two jobs gets `first_wire | n << 40`.
- Space is handled by `build_space` from `space_released` (the keymap eats Space on press); Enter ticks in the card.
- Ticks sit in `CommissioningStore.build` (additive JSON key `build`, absent when empty), so old files load and the `.kcfield` carries them.
- Build mode shares `wb.commissioning` for the store; it closes itself when the Commissioning workspace opens, so two copies never diverge.
- A test that loops on `next()` hangs if two steps share a key (a tick then flips the other to Changed). Keep the key-uniqueness test.
