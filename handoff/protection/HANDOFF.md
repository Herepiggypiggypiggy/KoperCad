# Protection and cable engineering: handoff

Thread: "Protection and cable engineering" (session cse_01CQXymGH5kY1GfABR1JHePK).
Branch: claude/project-thread-utbmbn in kopercad. Stacked on PR #454 (adapter), which is stacked on #412
(calc library). Both are draft and unmerged; this branch contains their commits. PR for this work: see PROGRESS.md.

Layout (crates/novo-checks/src/calc/): tables.rs, network.rs, curves.rs, selectivity.rs, circuit.rs, spec.rs,
sccr.rs, starter.rs; adapter in calc_checks.rs (new kind CableRating); CLI in crates/novo-cli (`calc`).
Docs: docs/PROTECTION.md. Example: docs/examples/protection-sample.json.

Resume: `cargo test -p novo-checks -p novo-cli`; `cargo clippy --workspace --all-targets -- -D warnings`;
`./target/debug/kopercad-cli calc docs/examples/protection-sample.json`.

Traps and decisions:
- Tables are unverified; never print a number without `tables.disclosure()`. Missing table cells return None.
- Unknown is a status: do not default missing Icu, I²t or PE sizes.
- Upper-bound let-through (Ik² × guaranteed time) can prove a pass but never a fail: current-limiting devices let less through.
- Selectivity from generic bands is conservative; back-up needs a maker table.
- Overlaps: trip curves with "Simulation: faults, trip curves, waveforms" (cse_01NQC5mMueppdFcRDgtrhhwd), SCCR rule set and report with "Standards compliance checks" (cse_016Ck8MiWnPc2Jymjbqtu9u3), part fields with "Parts library at scale" (cse_01KA869tMQuWufRom7xSf4hF). All were messaged.
Not done: `kopercad-cli starter` subcommand (engine `calc::starter` exists, no CLI yet).
