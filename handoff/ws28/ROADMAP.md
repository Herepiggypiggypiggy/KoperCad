# WS-28 Python API and CLI: roadmap

Roadmap item: embedded Python console and script runner inside the app, a
Python package that opens and edits project files headless, and a
command-line tool for batch export, all through the same command API as the
UI. Done when a Python script places a motor circuit, renames its tags and
exports a BOM to Excel.

## Done
- #56 (merged 67c3591): `novo-cli` and `novo-py` (`ScriptProject`, PyO3 shim behind
  `python`, abi3 wheel, `pip install --user` or venv). Crash-recovery guard and
  `overwrite=False` on `create`/`save_as`.
- #65 (READY from B and C at 3354d4e, waiting to merge): `novo export pdf` on novo-pdf
  with library artwork (`--page`), `novo check` on novo-checks, report exports
  (`novo export bom|devices|wires|toc|reports`, Python `add_part`, `assign_part`,
  `report`, `export_bom`, `export_reports`), shared export guard and atomic write,
  plain clap errors, `NothingCreated` instead of panics, venv-first README.
  **The done-criterion passes headless** (`test_dol_bom_to_excel`).

## Next, in order (one new PR from main after #65 merges)
1. Review notes on #65 (all non-blocking):
   - `ScriptProject::execute_one`: on `NothingCreated`, undo the step first
     (`self.doc.undo()` and record the change set) so "a refused call changes
     nothing" holds; pick with `created.into_iter().find_map(pick)`, not `first()`. (B)
   - Long help of `novo`: exit 2 also covers unknown page names and report types. (B)
   - `--delimiter`: accept only `,` `;` tab `|` (quote/newline break CSV). (B)
   - novo-cli `export_reports`: map `ScriptError::Export`/`Io` to exit 4, not 2. (B)
   - Return the `ProjectFileTarget` refusal directly instead of
     `write_output(file, path, &[])`. (B)
   - Python `place(..., device=k)` defaults `main=False`: a coil placed without
     `main=True` gives "No main function". Default `main` from the symbol's class
     (as novo-editor's `classify` does), keep the keyword to override. Predates #65. (C)
   - Docstrings/errors still say "toc, bom, devices or wires": build them from
     `report_ids()`; add generic `novo export report <id>` so WS-13's #68
     `terminals`/`cables` (multi-section; CSV writes section titles) are reachable. (C, WS-13)
2. When #70 merges: switch `novo-cli` to novo-editor's light `art` feature and drop
   `features = ["egui"]`, so `novo` no longer links egui/wgpu. (B, C)
3. Python on `Project`: `check()` (novo-checks problems as dicts), `export_pdf(path,
   pages=None)` (novo-pdf + LibraryArt through `write_export`), `batch(label)` context
   manager (several calls as one undo step via `Command::Batch` or one `transact`),
   helpers for terminals/cables, type stubs (`.pyi`).
4. CI job that builds the wheel and runs pytest (release/build thread owns
   `.github/workflows`; must stay optional so CI without Python stays green).
5. Embedded Python console and script runner in the app on the Workbench (#46 is on
   main): same `ScriptProject` over the app's Document, one undo step per call.

## Open questions
- Wheel distribution for Windows users without admin: ship the wheel with the
  installer, or `pip install --user` from a release asset?
- Should `novo` gain write commands (rename, renumber)? Current answer: no, the CLI
  stays read-only and edits go through Python.
