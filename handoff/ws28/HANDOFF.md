# WS-28 Python API and CLI: handoff

## State (2026-10-14 20:00 UTC)
- PR #56 (phase 1) merged as 67c3591. Branch `claude/ws28-python-cli` restarted from main for the review follow-ups PR; number and head SHA are on the last line of PROGRESS.md.
- Nothing half-done in the working tree.

## Layout
- `crates/novo-py/src/lib.rs`: `ScriptProject` (Document + optional storage `Session`). Each edit method builds one `novo_core::Command` and runs it as one undo step; `session.record` after every edit/undo/redo. JSON views (`info_json`, `pages_json`, `devices_json`, `wires_json`, `nets_json`, `endpoint_name`) are shared with the CLI.
- `crates/novo-py/src/python.rs`: PyO3 shim, `#[cfg(feature = "python")]`. `Project` pyclass is `unsendable` (rusqlite connection). Queries go Value -> `json.loads`. Errors -> `novoelectric.NovoError`.
- `crates/novo-py/pyproject.toml`: maturin, module `novoelectric`, features `python`. pyo3 0.29 `abi3-py39` in `[workspace.dependencies]`.
- `crates/novo-py/python/tests/test_motor_circuit.py`: the DOL done-criterion test (BOM test skipped until WS-13).
- `crates/novo-cli`: `novo` bin; `run(args, out, err) -> exit code` in lib.rs for tests. Read-only (`ProjectFile::read`), refuses `-o` equal to the input.

## Resume
```
git fetch origin && git checkout claude/ws28-python-cli
export CARGO_TARGET_DIR=$HOME/target CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo clippy -p novo-py --features python --all-targets -- -D warnings
pip install --user maturin pytest
(cd crates/novo-py && CARGO_TARGET_DIR=$HOME/target-py maturin build --release -o $HOME/wheels)
pip install --user --force-reinstall $HOME/wheels/novoelectric-*.whl
python -m pytest -p no:cacheprovider crates/novo-py/python/tests
```

## Decisions and traps
- Python is behind a cargo feature so CI (no Python) stays green; maturin turns it on. Use a separate CARGO_TARGET_DIR for maturin (it builds with other flags).
- `ScriptProject::open`, `create` and `save_as` refuse a file with recovered autosave work (save_as onto the session's own file is allowed) and leave the recovery file, so scripts never save over crash work.
- `create`/`save_as` refuse an existing file unless `overwrite` (Python keyword `overwrite=False`).
- Reviewers: any CHANGES NEEDED blocks a merge; send every head to B and C and the coordinator.
- Opening a file that is locked waits ~5 s (storage's SQLite busy timeout) before `Locked`; the CLI test for that takes 5 s.
- Netlist names: device/terminal pins by designation (`-K1:13`); feeds `feed L1:1`, interruption points `ip NAME:1`, unlinked `symbol 12:1`.
- `novo check` runs `novo_checks::check`; exit 1 when the summary is not clean (infos such as missing parts do not count). `--json` gives severity, snake_case kind, label, message, page name, ids.
- `novo export pdf` uses `novo_pdf::export_pdf_with_art` with `novo_editor::art::LibraryArt` (needs novo-editor's `egui` feature, so the CLI links egui; see ROADMAP item 3). `--page NAME` repeats.
- Reports: `crates/novo-py/src/export.rs` (`report_bytes`, `report_json`, `write_export`, `is_project_path`) is shared by Python and the CLI; `novo export bom|devices|wires|toc|reports`.
- Exports go through `novo_py::write_export`: refused onto the input, any `*.novo`, `*.novo.autosave` or any existing SQLite file; written to `<out>.novo-tmp` then renamed.
- No novo-core API changes were needed.

## People
- Coordinator: session_01DZ68tTULUv7Uhe8ecKbC47. Reviewer for new-crate PRs: main reviewer B session_01QSnXFBDRyvwwzhT1S1gFGN (message it with every new head SHA); reviewer A session_01XEdxpvQFh4HWmurh72tKwp was pinged first.
- PR #51 pre-registers novo-cli/novo-py rows in Cargo.toml and CLAUDE.md: after it merges, merge main (scripts/merge-main.sh if present), keep one row each. #50 novo-checks, #49 novo-pdf, #55 WS-13 reports (Excel BOM): use them once merged.
- Roster: /mnt/project-files/coordination/threads.md.
