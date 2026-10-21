# Project data safety: handoff

Branch `claude/project-thread-7iuj1r` (kopercad). Code: `crates/novo-storage/src/salvage.rs`, `error.rs` (`damaged_at`), `file.rs`; app: `NovoApp::open_salvaged` in `crates/novo-app/src/app.rs`; root `Cargo.toml` (serde_json feature).
Tests: `crates/novo-storage/tests/damaged.rs`, `crates/novo-tests/tests/data_safety.rs`, app test `a_damaged_file_opens_as_a_recovered_copy_and_the_original_is_left_alone`. QA: `qa/scenarios/36-damaged-file-recovery.qa` with `qa/fixtures/damaged-cut-short.novo` (golden file cut at 60%).
Resume: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p novo-storage -p novo-tests --test data_safety -p novo-app`.
Traps: salvage reads the header by hand (SQLite refuses even pragmas on a file shorter than its header claims) and sets `writable_schema` on a read-only connection to read it; never write to the damaged file. Do not push to a READY PR.
Honest limits: see ROADMAP "Next".
