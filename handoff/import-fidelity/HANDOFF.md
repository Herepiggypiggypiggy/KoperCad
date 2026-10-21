# Import fidelity: handoff

Thread: Import fidelity (project thread cmsg_01TV2QyH4AYw9X2aTmgUsuhCKscLXXAN8M846wwvJ4C8GG). Lead: project lead thread.

State
- #1069 merged. Step 1 (the `guessed` report section) is in a PR from branch `import-fidelity-report`; see PROGRESS.md for the head.
- `Report` (crates/novo-interop/src/report.rs): moved / lossy / guessed / notes. `Report::guess(what, count, why)`, `is_exact()`. Summary line ends `Guessed: ...`.
- AML: `AmlPlan::report()` adds boxes (`aml::draw::is_box`), wire routes when the file has nets but no runs, new devices without any footprint.
- EPLAN: `pxf/plan.rs` adds boxes, loosely matched symbols, wire routes, per-cabinet footprints and rails/ducts.

Resume
- `cargo test -p novo-interop -p novo-cli`; clippy with `-D warnings`.
- The private parts file is in the project mount, `interop/private/crane/part-sizes.csv`; never commit it or real project designations (Reviewer A blocks on invented-names hygiene).

Traps
- Never push to a PR marked READY; follow-ups go in a new PR from main.
- Test strings must be invented (`=A1+CAB1-...`).
