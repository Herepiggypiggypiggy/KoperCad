# WS-28 Python API and CLI: roadmap

Roadmap item: embedded Python console and script runner in the app, a Python
package that opens and edits project files headless, and a command-line tool
for batch export, all through the same command API as the UI. Done when a
Python script places a motor circuit, renames its tags and exports a BOM to
Excel.

## Done
- Phase 1 (PR #56, merged 67c3591): `novo-cli` (`info | check | export json | export pdf`, read-only, exit codes 0-5) and `novo-py` (`ScriptProject` + PyO3 shim behind `python`, abi3 wheel). pytest DOL test; scripts refuse files with crash recovery and existing files unless `overwrite=True`.
- Review follow-ups (PR "WS-28: novo export pdf and novo check on the real engines"): `novo export pdf` on novo-pdf with library artwork and `--page`; `novo check` on novo-checks (severity, kind, page; exit 1 on errors/warnings); exports never land on a project or recovery file and are written atomically; plain clap errors; no panics in `ScriptProject` (`NothingCreated`); venv-first README.

- Reports (same PR #65, on novo-reports #55): `novo export bom|devices|wires|toc` (.xlsx/.csv, CSV to stdout), `novo export reports` (one workbook); Python `add_part`, `assign_part`, `report(id)`, `export_bom`, `export_reports`. `test_dol_bom_to_excel` runs: **the WS-28 done-criterion passes headless.**

## Next, in order
1. Python: `check()` (novo-checks) and `export_pdf()` on `Project`; `batch()` context manager (several calls as one undo step, `Command::Batch`); terminals/cables/parts helpers; type stubs (`.pyi`).
2. Ask WS-06 to put `novo_editor::art` behind a light feature (only `novo-canvas/model`), so `novo` stops pulling egui/wgpu in for PDF artwork.
3. CI job that builds the wheel and runs pytest (ask the release/build thread, which owns `.github/workflows`).
4. Embedded console and script runner in the app (Workbench #46 is on main): same `ScriptProject` over the app's Document.

## Open questions
- Wheel distribution for Windows users without admin: ship the wheel with the installer, or a per-user `pip install --user` from a release asset?
- Should `novo` gain write commands (rename, renumber) or leave edits to Python scripts? Current answer: read-only CLI, edits via Python.
