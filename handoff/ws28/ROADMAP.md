# WS-28 Python API and CLI: roadmap

Roadmap item: embedded Python console and script runner in the app, a Python
package that opens and edits project files headless, and a command-line tool
for batch export, all through the same command API as the UI. Done when a
Python script places a motor circuit, renames its tags and exports a BOM to
Excel.

## Done (phase 1, headless)
- `crates/novo-cli`: `novo info | check | export json | export pdf` (pdf stubbed, exit 5), read-only via `ProjectFile::read`, stable exit codes 0-5.
- `crates/novo-py`: `ScriptProject` (pure Rust, always built) + PyO3 shim behind feature `python`; abi3 wheel via maturin, `pip install --user`.
- pytest: DOL motor circuit placed, tags renamed, saved, reopened, netlist asserted. BOM test present but skipped (WS-13).

## Next, in order
1. `novo export pdf` on top of novo-pdf once PR #49 merges (replace the stub; keep exit codes).
2. `novo check` delegates to the checks engine when one lands on main (built-in duplicate-tag and short checks meanwhile).
3. BOM export to Excel from Python and `novo export bom` once WS-13 (reports) merges; un-skip `test_dol_bom_to_excel`.
4. Python: `batch()` context manager (several calls as one undo step) on top of `Command::Batch`; terminals/cables/parts helpers; type stubs (`.pyi`).
5. CI job that builds the wheel and runs pytest (ask the release/build thread, which owns `.github/workflows`).
6. Embedded console and script runner in the app, after the Workbench (#46) lands: same `ScriptProject` over the app's Document.

## Open questions
- Wheel distribution for Windows users without admin: ship the wheel with the installer, or a per-user `pip install --user` from a release asset?
- Should `novo` gain write commands (rename, renumber) or leave edits to Python scripts? Current answer: read-only CLI, edits via Python.
