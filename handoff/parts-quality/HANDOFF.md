# PARTS-Q handoff

Repo herepiggypiggypiggy/kopercad, branch claude/project-thread-7gkab5 (restart from main after each merge). Reviewer R2 cse_01SEokxUZ1MKDwyQvLm4DPys, lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.

State: #568 (score + Quality tab), #979 (Duplicates tab), #1014 (Review tab) merged. Follow-up PR is a DRAFT: coordinator asked for no ready-mark until the release candidate is tagged. It adds trust tags (Unreviewed / Changed since review / Rejected) in the part picker rows, the Parts list description cell and the part detail. Nothing is blocked; tags are information.
Files: novo-parts/src/quality.rs (score, duplicates, ReviewLedger); novo-app/src/app/parts_quality.rs (tabs, plus trust_at/trust_badge with a ledger cache that rereads parts-review.json when its mtime changes, checked at most once a second); small touches in part_picker.rs (Candidate.trust) and parts.rs (list cell, detail line).
Open: the DESIGN CHECK is not built. The ledger lives in the user profile, the checks are pure functions of the project, so a check needs the trust stamped into the project part at assignment (property on the project Part) plus two ProblemKinds in novo-checks (checks lane). Stale after a later approval unless re-stamped. Waiting for a decision from the coordinator. Also open: a project-level "in use" listing in the Review tab; incoming_conflicts has no caller; CLI `kopercad-cli parts quality`.
Resume: CARGO_PROFILE_DEV_DEBUG=0 cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
Rules: draft until the coordinator says the candidate is tagged; no push after READY; no catalog.rs/schema.rs edits; tests in new files; send heads to R2 and lead A.
