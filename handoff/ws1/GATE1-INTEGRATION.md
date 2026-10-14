# Gate 1 integration (for the WS-01 thread)

Done by a Gate 1 integration worker (session_01QtW6E2sY3dMnhYDgX3zXge) while WS-01 was paused. Coordinator: session_01DZ68tTULUv7Uhe8ecKbC47.

## Where
- Branch `claude/gate1-integration`, **PR #53**, based on main. Everything it stacked on (#46, #38, #31, #49, #50) has merged; main was merged in at 2026-10-14 19:55 UTC and the PR is marked ready for review (main reviewer A for correctness, the design reviewer for UI).
- Merge main with `scripts/merge-main.sh`; never edit the crate list lines in CLAUDE.md or `[workspace.dependencies]`.
- Once READY, no more pushes: follow-ups go in a new PR from main.

## What is in crates/novo-app (one file per engine, so it splits per dependency)
| File | Engine | What |
| --- | --- | --- |
| `editor_canvas.rs` | novo-editor (#38) | `EditorCanvas: CanvasView` around `SchematicView`; per-frame ChangeSet recorded via `OpenPage::edit`, handed to the shell via `take_changes` (never back to the view); `take_page_request` followed; `install(rs)` = MSAA 1 |
| `canvas.rs` | | `OpenPage::edit`, trait methods `take_changes`, `take_page_request`, `schematic`, `schematic_ref` |
| `keys.rs`, `command_field.rs`, `app/workbench.rs` | novo-workflow (main) | egui key -> `Chord`, focus -> `Focus`, `Keymap::resolve`; the field shows `Palette::search_in` rows (Ctrl+K top, I place mode and Space at the pointer); `run_command(id, arg)` and `run_target`; `type_command` for tests/scripts. app/palette.rs removed |
| `app/xref.rs` | novo-xref (#31) | labels (overlay painter), related highlight, G / Shift+G / Alt+Left/Right, device jump |
| `app/terminals.rs` | novo-terminals (main) | Ctrl+3 drawer (strip of the selection), dashed proposals, Enter/Tab, accept all |
| `app/checks.rs` | novo-checks (#50) | issue pill, problem list, F8 / Shift+F8 |
| `app/pdf.rs` | novo-pdf (#49) | Export PDF (mark menu, `export pdf`) to `Documents\NovoElectric\<project>.pdf` |
| `app/gate1_tests.rs` | all | headless Gate 1: new project, starter over 2 pages via command field + editor, L1-L3, terminals, xref, save/reopen/compare, 2-page PDF |
- Also one commit in novo-editor: `SchematicView::set_style` (54bbc83), needed so symbols show on the dark canvas. #38 merged without it, so it ships in this PR; WS-06 was told.

## Keys
- The shell claims a key only when it runs the command itself (`claims_key`); W, R, X, F2, Delete, Escape, Page Up/Down stay with the editor (`EDITOR_KEYS`). Enter/Tab go to the editor while it has wire proposals, else to terminal proposals.
- Escape for the command field and lists is taken before the canvas (`escape_popups`), else the editor eats it while the pointer is on the drawing.

## Known gaps (reported to owners)
- WS-06: a coil placed as `coil K1` gets a new device (-K2); only contacts take the typed parent. The test places the coil first.
- WS-08: `accept all` parses as `accept` + `all` (the app handles that argument); `no ` lists devices by tag, -F1 before -K1.
- WS-03/WS-06: under Xvfb a single click on a symbol does not select it (box select and headless clicks do). Check on Windows.
- WS-11: terminal numbers follow accept order (Tab then Enter gave -X1:1 = V1).
- Not done: the Cables/Devices drawers, dismissing a terminal proposal, Issues/Changes outline modes, the inspector and tool dock, Space-held pan.

## Run
```
export CARGO_TARGET_DIR=<one dir> CARGO_INCREMENTAL=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
NOVO_GATE1_OUT=/tmp/g1 cargo test -p novo-app gate1   # keeps Starter.novo + Starter.pdf
```
Screenshots: Xvfb :99 + mesa-vulkan-drivers + **openbox** (without a window manager xdotool keys never arrive), then `novoelectric /tmp/g1/Starter.novo`.
