# PARTS-Q handoff

Repo herepiggypiggypiggy/kopercad, branch claude/project-thread-7gkab5 (restart from main after each merge). Reviewer R2 cse_01SEokxUZ1MKDwyQvLm4DPys, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.

State: PR 1 (#568, score + Quality tab) merged. PR 2 (duplicates and conflicts) written locally, see PROGRESS.md for the PR number and SHA.
Files: crates/novo-parts/src/quality.rs (score, report, duplicates), tests/quality.rs and tests/quality_duplicates.rs, crates/novo-app/src/app/parts_quality.rs (Parts window tabs Parts / Quality / Duplicates).
Next: PR 3 review queue (sidecar parts-review.json next to the catalog, fingerprint via sha2; import:* parts untrusted until approved; authored trusted), Approve/Reject in the Quality tab.
Resume: CARGO_PROFILE_DEV_DEBUG=0 cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
Rules: no push after READY; no catalog.rs/schema.rs edits; tests in new files; send heads to R2 and lead A.
Decisions: score counts only recorded data; unknown is never a conflict; same-order-number across unrelated makers is only a candidate.
