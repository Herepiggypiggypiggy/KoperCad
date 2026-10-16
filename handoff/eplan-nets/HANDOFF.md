# EPLAN-NETS handoff

Branch `claude/eplan-nets-pca7os` (kopercad). PR number and head SHA: see PROGRESS.md.

Resume: `cargo test -p novo-interop -p novo-checks`, then `cargo clippy --workspace --all-targets -- -D warnings`.

People: PXF reader thread cse_01SaP8ogQQJ4nBhYv3qCTcBm (fills ConnGraph); findings thread cse_01VQS5EbJ8bqSb6DPZ72Kuyw; reviewer R2 cse_01SEokxUZ1MKDwyQvLm4DPys; lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.

Decisions
- nets.rs is pure and neutral: no PXF types, no novo-core types except Endpoint/Netlist in `compare`.
- Only axis-aligned runs are reconstructed.
- DanglingJunction flags junctions with no wire only; one-wire junctions are legitimate wire ends.
- New ProblemKinds are listed in `tests/motor_starter.rs` `needs_more` (own fixtures in `tests/eplan_style.rs`).

Traps: the crane repo is private and outside this session's repo scope; rules come from the findings file only. Never copy EPLAN or customer content.
