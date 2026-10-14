# WS-28 Python API and CLI: handoff

## State (2026-10-14 20:20 UTC, written at cloud wrap-up)
- PR #56 (phase 1: `novo-cli`, `novo-py`) **merged** as 67c3591.
- PR #65 (follow-ups + reports) **merged** at 20:22 UTC (READY from reviewers B
  and C on 3354d4e). Branch `claude/ws28-python-cli` is spent; restart it from main
  for new work.
- This handoff PR (`claude/handoff-ws28`) only touches `handoff/ws28/`. If it
  conflicts with #65's copy of these files, keep this PR's version.
- Nothing half-done in code. Next work starts as a **new PR from main**
  (ROADMAP "Next").

## What exists on main
- `crates/novo-py/src/lib.rs`: `ScriptProject` = `novo_core::Document` + an optional
  `novo_storage::Session`. Each edit method builds one `Command` (one undo step) and
  `session.record`s the change set (also after undo/redo). Queries return
  `serde_json::Value` (`info/pages/devices/symbols/wires/potentials/terminals/nets`),
  shared with the CLI. `endpoint_name`: `-K1:13`, `feed L1:1`, `ip X:1`, `symbol 12:1`.
- `crates/novo-py/src/export.rs`: `report_bytes` (novo-reports, .xlsx or one-report .csv),
  `report_json`, `write_export` (the only way exports hit disk: refuses the source,
  `*.novo`, `*.novo.autosave`, any SQLite file; writes `<out>.novo-tmp` then renames).
- `crates/novo-py/src/python.rs`: PyO3 0.29 shim behind cargo feature `python`
  (abi3-py39, module `novoelectric`, pyclass `Project` is `unsendable`). Results go
  Value -> `json.loads`. Errors -> `novoelectric.NovoError`.
- `crates/novo-cli`: `novo info | check | export json|pdf|bom|devices|wires|toc|reports`.
  Read-only on projects (`ProjectFile::read`). Exit codes 0 ok, 1 check problems,
  2 usage, 3 unreadable project, 4 output not writable/refused, 5 unavailable.
- Tests: `crates/novo-py/tests/script.rs`, `crates/novo-cli/tests/cli.rs`,
  `crates/novo-py/python/tests/test_motor_circuit.py` (6 pytest tests; the
  done-criterion `test_dol_bom_to_excel` runs and passes).

## Resume (no admin rights; Linux shown, Windows the same per user)
```
git fetch origin && git checkout -B claude/ws28-python-cli origin/main
export CARGO_TARGET_DIR=$HOME/target CARGO_INCREMENTAL=0
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p novo-py --features python --all-targets -- -D warnings
cargo test --workspace
# Python, venv (works on PEP 668 distros; Windows: .venv\Scripts\activate)
python -m venv .venv && . .venv/bin/activate && pip install maturin pytest
(cd crates/novo-py && maturin develop --release)
python -m pytest -p no:cacheprovider crates/novo-py/python/tests
# or without venv: pip install --user maturin pytest;
# (cd crates/novo-py && CARGO_TARGET_DIR=$HOME/target-py maturin build --release -o $HOME/wheels)
# pip install --user --force-reinstall $HOME/wheels/novoelectric-*.whl
```
Use a separate CARGO_TARGET_DIR for maturin (different flags). Expect a 5 s pause
in tests that open a locked file (storage's SQLite busy timeout).

## Decisions and traps
- Python is behind a cargo feature so the workspace (and CI) build without Python.
- Scripts never save over crash work: `open`, `create`, `save_as` refuse a file
  with a `.autosave` (even with `overwrite=True`); `create`/`save_as` refuse any
  existing file unless `overwrite=True`; saving onto the session's own file is fine.
- `novo check` uses `novo_checks::check`; exit 1 iff `Summary::is_clean()` is false
  (infos like "no part" never fail it).
- `novo export pdf` uses `novo_editor::art::LibraryArt`, which needs novo-editor's
  `egui` feature, so `novo` links egui/egui-wgpu. Fix pending: #70 (fixer session
  session_017LndnL4fiT5rRPkqy3R36S) gates `art` behind a light feature.
- Process: once a PR has a READY, never push to it; new scope = new PR from main.
  Every push: message reviewers B and C with the head SHA. Two READYs needed.
- No novo-core API changes so far; ask WS-02 for any.

## People
- Coordinator: session_01DZ68tTULUv7Uhe8ecKbC47.
- Reviewers for WS-28: B session_01QSnXFBDRyvwwzhT1S1gFGN, C session_01Q6QWwh6uhq8Z2C1Ysih3UA.
- WS-13 reports: session_01834PmEwNeusq2TnHgA5ZyG (#68 adds `terminals`, `cables`).
- Roster: /mnt/project-files/coordination/threads.md, handoff/coordinator/cloud-roster.md.
