# Handoff: fault injection (lane F1)

Branch `claude/project-thread-hvek84` (repo kopercad). PR 1 contains engine, UI, tests, design doc. See ROADMAP.md.

Resume: `cargo test -p novo-tests --test walkthrough_fault_injection`, `cargo test -p novo-app --lib defects_card`.
Code: `crates/novo-sim/src/{inject,walkthrough}.rs`, hooks in `coupling.rs` (`settle_faulted`, `Faults`) and `lib.rs`
(`solve_with`, `Defects`), app `crates/novo-app/src/app/defects_card.rs` plus small hooks in `live.rs`
(`LiveView::defects`, `solved`, `leave`).

Traps: a cut wire uses the netlist of a copy of the project, so `Solved` skips `is_current` while defects are
active. Tags in the fixtures are full (`=A1+CAB-K1`); `inject::full_name` accepts the short form. Welded without
a pin only welds make contacts; use `mirror` for positively guided break contacts.
