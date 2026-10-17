# REL-COMPAT handoff

Branch `claude/project-thread-qq7uo7`, one PR per slice from main. Slice 1 = tests + storage fixes + docs (see PROGRESS.md for the PR number).

Code: `crates/novo-tests/tests/compat_v0_1.rs`, `crates/novo-tests/tests/golden/`, `crates/novo-storage/src/{file,error}.rs` (`UnknownRecord`, no overwrite of a newer file in `lock_existing`), `crates/novo-app/src/app.rs` (`describe_error`), `docs/FILE-COMPATIBILITY.md`.
Resume: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p novo-tests --test compat_v0_1 -p novo-storage -p novo-app`.
Traps: the child-process tests re-run the test binary by name (`child_save_loop`, `child_autosave_then_die`); keep names in sync. Session::save only writes recorded changes; the kill test uses ProjectFile::save on purpose. The golden file must never change for a released version.
Headless Linux only; Windows not run.
