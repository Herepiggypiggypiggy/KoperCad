# Controller gamepad: handoff

Branch `claude/controller-c5-gamepad`, one PR from main (number and head in PROGRESS.md). Written by a cloud session without a physical pad.

State: engine, host service, UI, tests, QA scenario and docs are in the PR. Verified by tests: the pure module (`crates/novo-controller/tests/gamepad.rs`),
the host with a fake device (`crates/novo-app/src/app/controller_pad_tests.rs`, `input_devices.rs` tests). The Windows reader `input_gilrs.rs` was checked
to compile for `x86_64-pc-windows-msvc` only, and has never run against hardware.

Files:
- `crates/novo-controller/src/gamepad/{mod,input,map,run,calibrate}.rs`: pure, wasm clean. `stick_axis` in `panel.rs` became `pub`. `ProjectControllers.gamepads` (new, optional).
- `crates/novo-app/src/app/{input_devices,input_gilrs,controller_pad,controller_pad_tests}.rs`; hooks in `controller_card.rs` (section, windows, `pad_frame`), `controller_live.rs` (`HoldBy::Pad`, maps in snapshot and sync).
- `crates/novo-app/src/control.rs` (`pad` command), `crates/novo-cli/src/qa.rs` (`pad` step), `qa/scenarios/1590*`, `docs/testing/qa-suite.md`.
- `gilrs` is a `cfg(windows)` dependency of `novo-app` only: on Linux it needs libudev, which CI lacks.

Resume: `cargo test -p novo-controller --test gamepad`, `cargo test -p novo-app --lib controller_pad input_devices`, `scripts/qa.sh --only gamepad`
(needs xvfb openbox mesa-vulkan-drivers). Windows reader check: `rustup target add x86_64-pc-windows-msvc`, then check a small crate that includes `input_gilrs.rs`.

Traps: a new live card needs a registered lesson anchor (`learn.rs`, `learn_labels.rs`, `UPDATE_ANCHORS_DOC=1 cargo test -p novo-app anchors_doc`); floating windows
must use `panel::dialog`; the pad lets go when the card closes, the window loses focus or calibration runs; the map edit is written to the project only
when no pointer button is down (one undo step per drag). Under `--control` the app reads the shared fake pad, never hardware.
Honest limits: simulates wiring and logic, no certification claim; the dead-man gate and E-stop button are logic, not a safety function.
