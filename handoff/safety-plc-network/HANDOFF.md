# Safety, PLC and network symbols: handoff

Branch `claude/project-thread-brqpzh`. PR: see PROGRESS.md.

Where things are:
- Symbols: `crates/novo-symbols/symbols/iec/{safety,network}/`, `plc/cpu.toml`, `plc/remote_io.toml`. Format notes: `crates/novo-symbols/FORMAT.md` ("Safety, PLC and network symbols").
- Tests: `crates/novo-symbols/tests/safety_network.rs`, `novo-workflow/src/insert.rs` (`safety_plc_and_network_words_find_their_symbols`), `novo-parts/tests/macro_starters.rs` (17 starters).
- Letter table: `novo-symbols/src/letter.rs` (pinned table updated). Standards class: `novo-checks/src/standards/symbols.rs`. EPLAN words: `novo-interop/src/eplan/symbols/table.toml` (end of file).
- Macros: `novo-parts/src/macros/starter/safety.rs` (uses `Kit` from `more.rs`, made `pub(super)`).
- QA: `scripts/qa.sh --only safety-plc` (needs xvfb openbox mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0). Pictures: `/mnt/project-files/qa/safety-network/`.

Decisions: network connections are one pin per Ethernet port (the cable and cores belong to the cable plan). Relay pins ordered A1 S34 S11 S12 S21 S22 so a reset button sits left of the channels. The emergency_stop_2ch symbol has no "e-stop" keyword so the existing single-channel button and the Emergency stop macro keep their ranks in Ctrl+K. Symbol ids are stable once shipped.

Traps: adding a symbol needs a row in the EPLAN table.toml (a test enforces it), a line in the letter table test, and the starter macro count test (17).
Resume: `cargo test -p novo-symbols -p novo-workflow -p novo-parts -p novo-interop -p novo-checks`.
