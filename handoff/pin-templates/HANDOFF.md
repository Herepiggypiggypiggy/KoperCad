# Pin templates per device class: handoff

Branch `claude/project-thread-szro2a`, first PR adds the engine and the check (see PROGRESS.md for the PR number).

State: engine, library conformance test and the `PinNames` check are in. Contacts are not templated on purpose: `ContactType` already checks their numbers.

Resume: `cargo test -p novo-symbols -p novo-checks`. Templates live in `crates/novo-symbols/src/pin_template.rs` (function `build`). Adding a class: add a `t(...)` entry, run the library test, add a case to `crates/novo-checks/tests/pin_names.rs`.

Decisions: pin names are strings; several sets per class; optional pins are allowed absent; severity is info so a house convention never blocks; no EPLAN data, names come from IEC 61666, 60034-8, 60947, 60076.
Trap: the check only compares part terminals when a device has exactly one part and the part carries terminal positions (`pin.<name>.x_mm`).
