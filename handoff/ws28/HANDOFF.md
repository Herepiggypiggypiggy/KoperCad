# WS-28 Python API and CLI: handoff

## State (2026-10-14 20:00 UTC)
- Branch `claude/ws28-python-cli`, PR #56. Head SHA: last line of PROGRESS.md.
- Phase 1 (headless) is complete and locally green; waiting on the main reviewer.
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
- `ScriptProject::open` refuses a file with recovered autosave work and leaves the recovery file (closing a pending session keeps it), so scripts never save over crash work.
- Opening a file that is locked waits ~5 s (storage's SQLite busy timeout) before `Locked`; the CLI test for that takes 5 s.
- Netlist names: device/terminal pins by designation (`-K1:13`); feeds `feed L1:1`, interruption points `ip NAME:1`, unlinked `symbol 12:1`.
- `novo check` has two built-in checks until a checks engine is on main; `export pdf` exits 5 until novo-pdf (#49) merges.
- No novo-core API changes were needed.

## People
- Coordinator: session_01DZ68tTULUv7Uhe8ecKbC47. Main reviewer: session_01XEdxpvQFh4HWmurh72tKwp (message it with every new head SHA).
- Roster: /mnt/project-files/coordination/threads.md.
