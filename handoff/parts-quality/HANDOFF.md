# PARTS-Q handoff

Branch: claude/project-thread-7gkab5 (repo herepiggypiggypiggy/kopercad). Reviewer slot R2 (data and engines), lead A cse_01FiLvRBKEcNNHbQEGhYyL1S.

State: PR 1 written (see PROGRESS.md for SHA once pushed). Files: crates/novo-parts/src/quality.rs (+ 2 export lines in lib.rs), crates/novo-parts/tests/quality.rs, crates/novo-app/src/app/parts_quality.rs, hook in app/parts.rs (field, init, 5 lines at top of parts_ui), `mod parts_quality;` in app.rs.

Resume: `CARGO_PROFILE_DEV_DEBUG=0 cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
Rules: after READY do not push; follow-ups go in a new PR from main. Tests in new files. No catalog.rs/schema.rs edits.
Decisions: score counts only recorded data; Partial = half; unknown source of power loss is Partial; trust state will live in a sidecar file, not the catalog schema.
