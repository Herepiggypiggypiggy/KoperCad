# Controller: handoff

State: skeleton only (`ControlKind`, `Step`, `two_step_contacts`), no dependencies. Lanes add their own dependencies in the crate's Cargo.toml.
Resume: `cargo test -p novo-controller`.
Rule: no vendor profiles or customer files in the repository; shipped profiles are generic.

2026-10-28 C4 import wizard: `novo_controller::import` (table parser, NL/EN word lists in `data/`, `propose` with warnings, TOML in the C1 profile format, device pins, generic library) and the wizard window `crates/novo-app/src/app/controller_import.rs` (File > Import > Controller profile from I/O table). Design: `docs/design/controller-import.md`. Resume: `cargo test -p novo-controller -p novo-app controller_import`. After C1 (PR #1558) merges: add a test that parses every library profile with `Profile::parse` and `validate` (checked by hand against head 0a7170a: 0 errors).
