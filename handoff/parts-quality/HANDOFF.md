# PARTS-Q handoff

Repo herepiggypiggypiggy/kopercad, branch claude/project-thread-7gkab5 (restart from main after each merge). Reviewer R2 cse_01SEokxUZ1MKDwyQvLm4DPys, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.

State: #568 (score + Quality tab) and #979 (Duplicates tab) merged. PR 3 (import review queue) written; see PROGRESS.md for the PR number and SHA.
Files: crates/novo-parts/src/quality.rs (score, duplicates, ReviewLedger), tests quality.rs, quality_duplicates.rs, quality_review.rs; crates/novo-app/src/app/parts_quality.rs (tabs Parts / Quality / Duplicates / Review).
Review rules: source starting import: is untrusted until approved; authored is trusted; no provenance is Unrecorded (not queued). Decisions live in parts-review.json next to parts.json, keyed by maker+order, with a sha256 fingerprint that ignores assets and terminal positions. A broken file blocks decisions and is never overwritten.
Untrusted tags in the picker, Parts list and detail: rebased and carried as #1084 (was #1024).
Post-v0.1, design check (not built, deliberately): the ledger lives in the user profile while novo-checks is a pure function of the project. Idea: stamp the trust into the project Part at assignment (in the part_picker assign commands) and add two ProblemKinds in novo-checks (checks lane). Catch: the stamp goes stale when a part is approved or changed later unless re-stamped, which would mislead, so it was left out of v0.1 (coordinator decision 2026-10-17).
Open / later: incoming_conflicts has no caller (import dialog thread); CLI `kopercad-cli parts quality`; refresh is manual after in-place edits.
Resume: CARGO_PROFILE_DEV_DEBUG=0 cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
Rules: no push after READY; no catalog.rs/schema.rs edits; tests in new files; send heads to R2 and lead A.
