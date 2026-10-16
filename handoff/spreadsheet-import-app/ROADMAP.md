# Spreadsheet import, app and CLI: roadmap

Scope: File > Import spreadsheet (preview dialog) and `kopercad-cli import sheet`, on the engine in `novo_interop::sheets` (#414, new rows from #411 rebased on it).

Done
- Dialog in novo-app (`app/sheet_import.rs`): tick per row, filters All/Changed/Conflicts/Rejected, conflicts with take-sheet/keep-project, rejected rows with sheet, line, reason, warnings and notes; Apply is one undo step.
- Command palette entry `import.spreadsheet`.
- `sheets::read_file` (xlsx, CSV/TSV/TXT, UTF-8 or Windows-1252).
- `kopercad-cli import sheet`: preview (text/JSON), `--output` new project file, `--resolve`, `--skip`.

Next
1. New filter and ticks for new-device rows when #411 lands on #414.
2. Cell-level tick (a single cell of a row), if wanted after hands-on test.
3. Python binding (WS-28) and terminals/cables sheets follow the engine.
